//! Boty L0–L3 (`docs/11-ai-i-symulacje.md`, rozdz. 3). Bot widzi tylko
//! `PlayerView`, czyli to samo co człowiek („Uczciwość botów”).

use std::num::NonZeroU32;

use engine::{
    CardAction, CardDef, CardKind, Condition, Content, Duration, Effect, HandAction, Kind, Move,
    MoveError, PlayerView, SLOTS, Stat, Target, Trigger, uniform_below,
};
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};
use serde::Deserialize;

mod adaptive;
mod profiles;
mod search;

pub use adaptive::OpponentModel;
pub use profiles::{Archetype, BotProfile, Profiles, training_profiles};

/// Poziom bota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Level {
    /// Losuje jeden z dozwolonych ruchów z rozkładu równomiernego.
    L0,
    /// Heurystyki: obrony, ataki, oszczędzanie K, czary o najlepszej wartości na koszt.
    L1,
    /// Model ujawnionych zachowań i odpowiedź na przewidywany rozkład.
    L2,
    /// Próbkowanie kart i strategia mieszana dla macierzy rozstrzygnięć silnika.
    L3,
}

/// Prawdopodobieństwo losowej zmiany jednego slotu przez L1, w procentach (ε z `docs/11`).
const EPSILON_PERCENT: u32 = 15;

/// Bot o stałym ziarnie: te same widoki dają te same ruchy.
#[derive(Debug, Clone)]
pub struct Bot {
    level: Level,
    rng: ChaCha8Rng,
    model: OpponentModel,
    aggression: i32,
    spell_bias: i32,
    epsilon: u32,
    temperature: i32,
}

impl Bot {
    #[must_use]
    pub fn new(level: Level, seed: u64) -> Self {
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&seed.to_le_bytes());
        bytes[8] = match level {
            Level::L0 => 0,
            Level::L1 => 1,
            Level::L2 => 2,
            Level::L3 => 3,
        };
        Self {
            level,
            rng: ChaCha8Rng::from_seed(bytes),
            model: OpponentModel::default(),
            aggression: 100,
            spell_bias: 100,
            epsilon: EPSILON_PERCENT,
            temperature: 300,
        }
    }

    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Ruch na bieżącą rundę; zawsze poprawny dla widoku `view`.
    pub fn choose(&mut self, view: &PlayerView, content: &Content<'_>) -> Move {
        self.observe(view);
        if !matches!(view.phase, engine::Phase::AwaitingMoves) {
            return view.default_move();
        }
        match self.level {
            Level::L0 => self.random_move(view, content),
            Level::L1 => self.heuristic_move(view, content),
            Level::L2 => self.adaptive_move(view, content),
            Level::L3 => self.search_move(view, content),
        }
    }

    /// Przekaż widok po rozstrzygnięciu rundy, także ostatniej. Nie losuje ruchu.
    /// Powtórne przekazanie tego samego widoku nie dopisuje obserwacji.
    pub fn observe(&mut self, view: &PlayerView) {
        self.model.observe(view);
    }

    /// Model można zachować między pojedynkami danego przeciwnika. Nie zawiera kart.
    pub fn model(&self) -> &OpponentModel {
        &self.model
    }

    pub fn set_model(&mut self, mut model: OpponentModel) {
        model.reset_duel();
        self.model = model;
    }

    /// Wywołaj przed kolejnym pojedynkiem, gdy zachowujesz tę samą instancję bota.
    pub fn reset_duel(&mut self) {
        self.model.reset_duel();
    }

    fn pick(&mut self, n: usize) -> usize {
        let Some(bound) = u32::try_from(n).ok().and_then(NonZeroU32::new) else {
            return 0;
        };
        uniform_below(&mut self.rng, bound) as usize
    }

    /// L0: wszystkie ruchy (2⁶ ustawień × akcje kart) i równomierny wybór spośród dozwolonych.
    fn random_move(&mut self, view: &PlayerView, content: &Content<'_>) -> Move {
        let mut actions = vec![CardAction::None];
        for index in 0..view.hand.len() as u8 {
            actions.push(CardAction::Cast(index));
            actions.push(CardAction::Discard(index));
        }
        let mut valid = Vec::new();
        for bits in 0..64u8 {
            for &card in &actions {
                let mv = Move {
                    attacks: kinds(bits),
                    defenses: kinds(bits >> SLOTS),
                    card,
                };
                if view.validate_move(content, &mv).is_ok() {
                    valid.push(mv);
                }
            }
        }
        // Ruch „bez zmian” bez karty jest zawsze dozwolony, więc lista nie jest pusta.
        let index = self.pick(valid.len());
        valid
            .get(index)
            .copied()
            .unwrap_or_else(|| view.default_move())
    }

    /// L1: heurystyki z `docs/11-ai-i-symulacje.md`, rozdz. 3.
    fn heuristic_move(&mut self, view: &PlayerView, content: &Content<'_>) -> Move {
        let keep = view.default_move();
        let mut mv = Move {
            card: best_card(view, content),
            ..keep
        };
        for (slot, attack, gain) in setting_gains(view) {
            // Zmiana slotu tylko, gdy zysk przewyższa koszt K (wartość 1 K to ok. 2 Ż).
            if gain <= 2 * view.rules.duel.change_cost {
                continue;
            }
            let previous = mv;
            if attack {
                mv.attacks[slot] = other(mv.attacks[slot]);
            } else {
                mv.defenses[slot] = other(mv.defenses[slot]);
            }
            if view.validate_move(content, &mv).is_err() {
                mv = previous;
            }
        }
        if self.rng.next_u32() % 100 < self.epsilon {
            let slot = self.pick(2 * SLOTS);
            let previous = mv;
            if slot < SLOTS {
                mv.attacks[slot] = other(mv.attacks[slot]);
            } else {
                mv.defenses[slot - SLOTS] = other(mv.defenses[slot - SLOTS]);
            }
            if view.validate_move(content, &mv).is_err() {
                mv = previous;
            }
        }
        if view.validate_move(content, &mv).is_ok() {
            mv
        } else {
            keep
        }
    }
}

fn kinds(bits: u8) -> [Kind; SLOTS] {
    std::array::from_fn(|slot| {
        if (bits >> slot) & 1 == 0 {
            Kind::Phys
        } else {
            Kind::Mag
        }
    })
}

const fn other(kind: Kind) -> Kind {
    match kind {
        Kind::Phys => Kind::Mag,
        Kind::Mag => Kind::Phys,
    }
}

/// Proponowane zmiany slotów z zyskiem w punktach Ż, od największego: (slot, czy atak, zysk).
fn setting_gains(view: &PlayerView) -> Vec<(usize, bool, i32)> {
    let (me, foe) = (&view.me, &view.opponent);
    let block_min = view.rules.duel.block_min_stamina;
    let mut gains = Vec::new();
    for slot in 0..SLOTS {
        // Ataki: przeciwnik prawdopodobnie zostawi obrony z poprzedniej rundy.
        let damage = |kind: Kind| {
            let blocked = foe.defenses[slot] == kind && foe.stamina >= block_min;
            if blocked {
                0
            } else {
                (me.stats.attack(kind) - foe.stats.defense(kind)).max(0)
            }
        };
        let current = me.attacks[slot];
        let gain = damage(other(current)) - damage(current);
        if gain > 0 {
            gains.push((slot, true, gain));
        }
        // Obrony: blok ma sens tylko z W (R-ATK-03) i przeciw przewidywanemu atakowi, czyli
        // typowi, który zada więcej obrażeń; przy remisie temu z poprzedniej rundy przeciwnika.
        let hurt = |kind: Kind| (foe.stats.attack(kind) - me.stats.defense(kind)).max(0);
        let previous = foe.attacks[slot];
        let threat = if hurt(other(previous)) > hurt(previous) {
            other(previous)
        } else {
            previous
        };
        if me.stamina >= block_min && me.defenses[slot] != threat {
            let gain = (foe.stats.attack(threat) - me.stats.defense(threat)).max(0);
            gains.push((slot, false, gain));
        }
    }
    gains.sort_by_key(|&(slot, attack, gain)| (-gain, slot, attack));
    gains
}

/// Karta do rzucenia o najlepszej wartości na koszt, z rezerwą W (R-ATK-03); bez dobrego czaru
/// odrzuca kartę, której postać nie może rzucić, żeby dobrać inną.
fn best_card(view: &PlayerView, content: &Content<'_>) -> CardAction {
    let keep = view.default_move();
    let reserve = view.me.stats.stamina_max / 4;
    let mut best: Option<(i32, CardAction)> = None;
    let mut unusable = None;
    for (index, &card) in view.hand.iter().enumerate() {
        let index = index as u8;
        let cast = Move {
            card: CardAction::Cast(index),
            ..keep
        };
        let cost = match view.validate_move(content, &cast) {
            Ok(cost) => cost,
            Err(MoveError::RequirementsNotMet | MoveError::InterventionUsed) => {
                unusable.get_or_insert(CardAction::Discard(index));
                continue;
            }
            Err(_) => continue,
        };
        let Some(def) = content.get(card) else {
            continue;
        };
        let value = card_value(view, def);
        if value <= 0 || view.me.stamina - cost.stamina < reserve {
            continue;
        }
        let score = value * 10 / (cost.focus + cost.stamina / 5 + 1);
        if best.is_none_or(|(top, _)| score > top) {
            best = Some((score, cast.card));
        }
    }
    best.map(|(_, action)| action)
        .or(unusable)
        .unwrap_or(CardAction::None)
}

/// Przybliżona wartość karty w punktach Ż z punktu widzenia rzucającego.
fn card_value(view: &PlayerView, def: &CardDef) -> i32 {
    if !card_ready(view, def) {
        return 0;
    }
    let remaining = i32::from(view.rules.duel.rounds.saturating_sub(view.round)) + 1;
    let repeats = match def.kind {
        CardKind::Enchantment(Trigger::OnRound | Trigger::OnRoundEnd) => remaining,
        CardKind::Enchantment(_) => 2,
        CardKind::Instant => 1,
    };
    def.effects
        .iter()
        .map(|effect| effect_value(view, effect, remaining))
        .sum::<i32>()
        * repeats
}

/// Warunki karty są sprawdzane przed jej efektami; warunki `ConditionalWin` dopiero
/// na końcu rundy, dlatego nie odrzucamy ich tutaj na podstawie bieżącego stanu.
fn card_ready(view: &PlayerView, def: &CardDef) -> bool {
    def.condition.is_none_or(|condition| match condition {
        Condition::RoundAtLeast(round) => view.round >= round,
        Condition::OpponentExhausted => view.opponent.stamina == 0,
        Condition::HpPercentAhead => {
            i64::from(view.me.hp.max(0)) * i64::from(view.opponent.hp_start)
                > i64::from(view.opponent.hp.max(0)) * i64::from(view.me.hp_start)
        }
    })
}

fn effect_value(view: &PlayerView, effect: &Effect, remaining: i32) -> i32 {
    // Korzyść z efektu na siebie liczy się dodatnio, a na przeciwnika ujemnie (i odwrotnie).
    let sign = |target: Target| match target {
        Target::Caster => 1,
        Target::Opponent => -1,
        Target::Both => 0,
    };
    let (me, foe) = (&view.me, &view.opponent);
    match *effect {
        Effect::StatMod {
            target,
            stat,
            amount,
            duration,
        } => {
            // Punkt ataku albo obrony to w przybliżeniu 1 Ż na rundę (większość ataków jest
            // blokowana), a punkt Ż_max albo W_max działa jak jednorazowy przyrost.
            let lasting = match (stat, duration) {
                (Stat::HpMax | Stat::StaminaMax, _) | (_, Duration::Round) => 1,
                (_, Duration::Duel | Duration::WhileActive) => remaining,
            };
            sign(target) * i32::from(amount) * lasting
        }
        Effect::Heal { target, amount } => {
            let missing = (me.stats.hp_max - me.hp).max(0);
            sign(target) * i32::from(amount).min(missing)
        }
        Effect::Damage { target, amount } => -sign(target) * i32::from(amount),
        Effect::Stamina { target, amount } | Effect::Focus { target, amount } => {
            sign(target) * i32::from(amount) / 2
        }
        Effect::Drain { amount, .. } => i32::from(amount),
        Effect::ForceHit { target, .. } | Effect::Unblockable { target, .. } => sign(target) * 8,
        Effect::ForceMiss { target, .. } => -sign(target) * 6,
        Effect::SwapChoices { target } => -sign(target) * 4,
        Effect::DamageTakenMult {
            target, percent, ..
        } => -sign(target) * (i32::from(percent) - 100) / 10,
        Effect::SchoolMod { target, amount, .. } => sign(target) * i32::from(amount) * 2,
        Effect::Hand {
            target,
            action,
            count,
        } => {
            let per_card = match action {
                HandAction::Draw => 2,
                HandAction::DiscardRandom => -2,
            };
            sign(target) * per_card * i32::from(count)
        }
        Effect::Dispel { target } => {
            let owned = |side| {
                view.enchantments
                    .iter()
                    .filter(|(owner, _)| *owner == side)
                    .count()
            };
            let mine = owned(view.side) as i32;
            let theirs = owned(view.side.other()) as i32;
            match target {
                Target::Caster => -6 * mine,
                Target::Opponent => 6 * theirs,
                Target::Both => 6 * (theirs - mine),
            }
        }
        Effect::ConditionalWin { condition } => match condition {
            Condition::OpponentExhausted if foe.stamina == 0 => 100,
            Condition::RoundAtLeast(round) if view.round >= round => 20,
            Condition::HpPercentAhead
                if i64::from(me.hp.max(0)) * i64::from(foe.hp_start)
                    > i64::from(foe.hp.max(0)) * i64::from(me.hp_start) =>
            {
                50
            }
            _ => 0,
        },
        Effect::StatSet { .. } | Effect::SwapStats { .. } => 1,
    }
}

#[cfg(test)]
mod tests {
    use engine::testkit::{content, random_setup, sample_cards, seeded_rng};
    use engine::{EventBuf, Side, new_duel, resolve_round, view};

    use super::{Bot, Level};

    #[test]
    fn bots_always_choose_valid_moves_and_repeat_them_for_the_same_seed() {
        let cards = sample_cards();
        let content = content(&cards);
        for seed in 0..200 {
            let setup = random_setup(&mut seeded_rng(seed), &content);
            let mut state = new_duel(&setup, &content).expect("poprawne wejście");
            let level = if seed % 2 == 0 { Level::L0 } else { Level::L1 };
            let mut bots = [Bot::new(level, seed), Bot::new(Level::L1, seed + 1)];
            let mut twins = bots.clone();
            let mut events = EventBuf::new();
            while !state.is_finished() {
                let moves = Side::BOTH.map(|side| {
                    let seen = view(&state, side);
                    let mv = bots[side.index()].choose(&seen, &content);
                    assert!(seen.validate_move(&content, &mv).is_ok(), "ziarno {seed}");
                    assert_eq!(twins[side.index()].choose(&seen, &content), mv);
                    mv
                });
                resolve_round(&mut state, &content, &moves, &mut events).expect("poprawne ruchy");
            }
        }
    }
}
