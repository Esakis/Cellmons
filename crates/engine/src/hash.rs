//! Skrót stanu pojedynku: porównanie natywne = WASM, testy i kontrola powtórek
//! (`docs/06-silnik-gry.md`, rozdz. 7).

use crate::card::{CardId, Condition};
use crate::effect::AttackFilter;
use crate::enchant::ActiveEnchantment;
use crate::end::{Outcome, Reason};
use crate::rules::{CounterResolution, DeckRules, DefaultSettings, DuelRules, RulesConfig};
use crate::state::{AttackOverride, DuelState, Phase, PlayerState};
use crate::types::{Kind, Side, Specialization, Stats};

/// Kontekst BLAKE3 skrótu stanu.
const HASH_CONTEXT: &str = "Spellgambit engine 2026-10-14 duel state hash";

/// 64-bitowy skrót całego stanu, taki sam na każdej platformie: pola są kodowane jawnie, liczby
/// w kolejności little-endian, a długości list jako jeden bajt.
#[must_use]
pub fn state_hash(state: &DuelState) -> u64 {
    // Pełne rozłożenie struktur: nowe pole nie skompiluje się, dopóki nie trafi do skrótu.
    let DuelState {
        rules,
        round,
        phase,
        initiative,
        players,
        overrides,
        rng,
        enchantments,
        next_order,
        win_checks,
    } = state;
    let mut out = Encoder(blake3::Hasher::new_derive_key(HASH_CONTEXT));
    out.rules(rules);
    out.u8(*round);
    out.phase(*phase);
    out.side(*initiative);
    for player in players {
        out.player(player);
    }
    out.u8(overrides.len() as u8);
    for forced in overrides {
        out.attack_override(forced);
    }
    out.0.update(&rng.get_seed());
    out.0.update(&rng.get_stream().to_le_bytes());
    out.0.update(&rng.get_word_pos().to_le_bytes());
    out.u8(enchantments.len() as u8);
    for active in enchantments {
        // Wyzwalacz i efekty wynikają z karty w treści gry o wersji zapisanej w powtórce.
        let ActiveEnchantment {
            owner,
            card,
            order,
            trigger: _,
            effects: _,
        } = active;
        out.side(*owner);
        out.u16(*card);
        out.u16(*order);
    }
    out.u16(*next_order);
    out.u8(win_checks.len() as u8);
    for &(side, condition) in win_checks {
        out.side(side);
        out.condition(condition);
    }
    let hash = out.0.finalize();
    hash.as_bytes()
        .first_chunk::<8>()
        .map_or(0, |bytes| u64::from_le_bytes(*bytes))
}

struct Encoder(blake3::Hasher);

impl Encoder {
    fn u8(&mut self, value: u8) {
        self.0.update(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.0.update(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.0.update(&value.to_le_bytes());
    }

    fn side(&mut self, side: Side) {
        self.u8(match side {
            Side::A => 0,
            Side::B => 1,
        });
    }

    fn kind(&mut self, kind: Kind) {
        self.u8(match kind {
            Kind::Phys => 0,
            Kind::Mag => 1,
        });
    }

    /// Listy kart mają najwyżej kilkadziesiąt elementów, więc długość mieści się w bajcie.
    fn cards(&mut self, cards: &[CardId]) {
        self.u8(cards.len() as u8);
        for &card in cards {
            self.u16(card);
        }
    }

    fn condition(&mut self, condition: Condition) {
        match condition {
            Condition::RoundAtLeast(round) => {
                self.u8(0);
                self.u8(round);
            }
            Condition::HpPercentAhead => self.u8(1),
            Condition::OpponentExhausted => self.u8(2),
        }
    }

    fn attack_override(&mut self, forced: &AttackOverride) {
        let AttackOverride {
            attacker,
            hit,
            filter,
            used,
        } = *forced;
        let AttackFilter {
            kind,
            slot,
            first_only,
        } = filter;
        self.side(attacker);
        self.u8(u8::from(hit));
        match kind {
            None => self.u8(0),
            Some(kind) => {
                self.u8(1);
                self.kind(kind);
            }
        }
        self.u8(slot.unwrap_or(u8::MAX));
        self.u8(u8::from(first_only));
        self.u8(u8::from(used));
    }

    fn rules(&mut self, rules: &RulesConfig) {
        let RulesConfig { duel, deck } = rules;
        let DuelRules {
            rounds,
            attacks_per_round,
            focus_per_round,
            focus_cap,
            change_cost,
            hand_size,
            combo_multipliers,
            block_min_stamina,
            default_settings,
            counter_resolution,
            move_timeout_after_opponent_s,
            round_timeout_s,
            max_consecutive_timeouts,
            reconnect_grace_s,
        } = duel;
        self.u8(*rounds);
        self.u8(*attacks_per_round);
        self.i32(*focus_per_round);
        match focus_cap {
            None => self.u8(0),
            Some(cap) => {
                self.u8(1);
                self.i32(*cap);
            }
        }
        self.i32(*change_cost);
        self.u8(*hand_size);
        for multiplier in combo_multipliers {
            self.i32(*multiplier);
        }
        self.i32(*block_min_stamina);
        self.u8(match default_settings {
            DefaultSettings::HighestStat => 0,
        });
        self.u8(match counter_resolution {
            CounterResolution::InitiativeFirst => 0,
        });
        self.u16(*move_timeout_after_opponent_s);
        self.u16(*round_timeout_s);
        self.u8(*max_consecutive_timeouts);
        self.u16(*reconnect_grace_s);

        let DeckRules {
            size,
            max_copies,
            fill_missing_with_random,
            saved_presets_max,
        } = deck;
        self.u8(*size);
        self.u8(*max_copies);
        self.u8(u8::from(*fill_missing_with_random));
        self.u8(*saved_presets_max);
    }

    fn phase(&mut self, phase: Phase) {
        match phase {
            Phase::AwaitingMoves => self.u8(0),
            Phase::Finished(Outcome::Win { winner, reason }) => {
                self.u8(1);
                self.side(winner);
                self.reason(reason);
            }
            Phase::Finished(Outcome::Draw { reason }) => {
                self.u8(2);
                self.reason(reason);
            }
        }
    }

    fn reason(&mut self, reason: Reason) {
        self.u8(match reason {
            Reason::Knockout => 0,
            Reason::Rounds => 1,
            Reason::Forfeit => 2,
            Reason::Disconnect => 3,
            Reason::Timeout => 4,
            Reason::ConditionalWin => 5,
        });
    }

    fn player(&mut self, player: &PlayerState) {
        let PlayerState {
            base,
            permanent,
            round_delta,
            base_schools,
            specialization,
            school_permanent,
            school_round,
            damage_pct,
            round_damage_pct,
            hp,
            hp_start,
            stamina,
            focus,
            attacks,
            defenses,
            hand,
            deck,
            discard,
            used_interventions,
        } = player;
        let Stats {
            phys_atk,
            phys_def,
            mag_atk,
            mag_def,
            hp_max,
            stamina_max,
        } = *base;
        for value in [phys_atk, phys_def, mag_atk, mag_def, hp_max, stamina_max] {
            self.i32(value);
        }
        for value in permanent.iter().chain(round_delta) {
            self.i32(*value);
        }
        self.0.update(&base_schools.0);
        match specialization {
            None => self.u8(0),
            Some(Specialization::School(school)) => {
                self.u8(1);
                self.u8(*school as u8);
            }
            Some(Specialization::Essence) => self.u8(2),
            Some(Specialization::Concentration) => self.u8(3),
        }
        for delta in school_permanent.iter().chain(school_round) {
            self.0.update(&delta.to_le_bytes());
        }
        self.0.update(&damage_pct.to_le_bytes());
        self.0.update(&round_damage_pct.to_le_bytes());
        for value in [*hp, *hp_start, *stamina, *focus] {
            self.i32(value);
        }
        for kind in attacks.iter().chain(defenses) {
            self.kind(*kind);
        }
        self.cards(hand);
        self.cards(deck);
        self.cards(discard);
        self.cards(used_interventions);
    }
}
