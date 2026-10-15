//! Widok gracza: to, co gracz może wiedzieć o pojedynku (`docs/06-silnik-gry.md`, rozdz. 8).
//! Na nim działają boty (`docs/11-ai-i-symulacje.md`, „Uczciwość botów”), a w przyszłości także
//! przeglądarka w pojedynkach online.

use arrayvec::ArrayVec;

use crate::card::{CardId, Condition, Content};
use crate::enchant::{ActiveEnchantment, MAX_ENCHANTMENTS};
use crate::moves::{CardAction, Move, MoveCost, MoveError, Mover, Preview};
use crate::rules::{MAX_DECK_SIZE, MAX_HAND_SIZE, MAX_ROUNDS, RulesConfig};
use crate::state::{AttackOverride, DuelState, MAX_OVERRIDES, MAX_WIN_CHECKS, Phase, PlayerState};
use crate::types::{Kind, SLOTS, Schools, Side, Specialization, Stats};

/// Jawne dane jednej strony, widoczne dla obu graczy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideView {
    /// Statystyki i poziomy szkół efektywne.
    pub stats: Stats,
    pub schools: Schools,
    pub hp: i32,
    pub hp_start: i32,
    pub stamina: i32,
    pub focus: i32,
    /// Ustawienia z poprzedniej rundy (R-RUN-07).
    pub attacks: [Kind; SLOTS],
    pub defenses: [Kind; SLOTS],
    /// Mnożnik otrzymywanych obrażeń w procentach.
    pub damage_taken_pct: u32,
    /// Liczba kart w ręce i w talii; same karty i kolejność talii są ukryte.
    pub hand_len: u8,
    pub deck_len: u8,
    /// Karty rzucone i odrzucone (jawne).
    pub discard: ArrayVec<CardId, MAX_DECK_SIZE>,
    /// Interwencje rzucone w tym pojedynku (jawne).
    pub used_interventions: ArrayVec<CardId, { MAX_ROUNDS as usize }>,
}

/// Wszystko, co gracz strony `side` wie o pojedynku: bez ręki przeciwnika, kolejności talii
/// i ziarna losowania.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerView {
    pub side: Side,
    pub round: u8,
    pub phase: Phase,
    pub initiative: Side,
    pub rules: RulesConfig,
    pub me: SideView,
    pub opponent: SideView,
    /// Własna ręka; indeksy jak w `CardAction`.
    pub hand: ArrayVec<CardId, MAX_HAND_SIZE>,
    pub specialization: Option<Specialization>,
    /// Aktywne umagicznienia obu stron w kolejności rzucenia (lista jawna, R-CZA-02).
    pub enchantments: ArrayVec<(Side, CardId), MAX_ENCHANTMENTS>,
    // Tylko dane jawne: bez rąk, talii i RNG. Zachowuje m.in. skalowanie sprzężeń,
    // kolejność umagicznień i modyfikatory sprzed przycięcia statystyk do zera.
    simulation: SimulationSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SimulationSnapshot {
    players: [PlayerState; 2],
    enchantments: ArrayVec<ActiveEnchantment, MAX_ENCHANTMENTS>,
    overrides: ArrayVec<AttackOverride, MAX_OVERRIDES>,
    win_checks: ArrayVec<(Side, Condition), MAX_WIN_CHECKS>,
    next_order: u16,
}

/// Widok pojedynku od strony `side`.
#[must_use]
pub fn view(state: &DuelState, side: Side) -> PlayerView {
    PlayerView {
        side,
        round: state.round,
        phase: state.phase,
        initiative: state.initiative,
        rules: state.rules.clone(),
        me: side_view(state, side),
        opponent: side_view(state, side.other()),
        hand: state.player(side).hand.clone(),
        specialization: state.player(side).specialization,
        enchantments: state.enchantments().collect(),
        simulation: SimulationSnapshot {
            players: state.players.clone().map(|mut player| {
                player.hand.clear();
                player.deck.clear();
                player
            }),
            enchantments: state.enchantments.clone(),
            overrides: state.overrides.clone(),
            win_checks: state.win_checks.clone(),
            next_order: state.next_order,
        },
    }
}

fn side_view(state: &DuelState, side: Side) -> SideView {
    let player = state.player(side);
    SideView {
        stats: state.stats(side),
        schools: state.schools(side),
        hp: player.hp,
        hp_start: player.hp_start,
        stamina: player.stamina,
        focus: player.focus,
        attacks: player.attacks,
        defenses: player.defenses,
        damage_taken_pct: state.damage_taken_pct(side),
        hand_len: player.hand.len() as u8,
        deck_len: player.deck.len() as u8,
        discard: player.discard.clone(),
        used_interventions: player.used_interventions.clone(),
    }
}

impl PlayerView {
    /// Tworzy hipotetyczny stan do przeszukiwania, wyłącznie z jawnego snapshotu i kart
    /// dostarczonych przez wywołującego. Nie odzyskuje prawdziwej ręki, talii ani RNG.
    /// `decks` są w kolejności stron A, B (wierzch na końcu); długości muszą zgadzać się
    /// z widokiem. Rozstrzyganie takiego stanu nie zmienia prawdziwego pojedynku.
    pub fn sample_state(
        &self,
        content: &Content<'_>,
        opponent_hand: &[CardId],
        decks: [&[CardId]; 2],
        seed: [u8; 32],
    ) -> Option<DuelState> {
        let mut players = self.simulation.players.clone();
        for side in Side::BOTH {
            let seen = if side == self.side {
                &self.me
            } else {
                &self.opponent
            };
            let hand = if side == self.side {
                self.hand.as_slice()
            } else {
                opponent_hand
            };
            let deck = decks[side.index()];
            if hand.len() != usize::from(seen.hand_len)
                || deck.len() != usize::from(seen.deck_len)
                || hand.iter().chain(deck).any(|&id| content.get(id).is_none())
            {
                return None;
            }
            let player = &mut players[side.index()];
            player.hand.try_extend_from_slice(hand).ok()?;
            player.deck.try_extend_from_slice(deck).ok()?;
        }
        Some(DuelState {
            rules: self.rules.clone(),
            round: self.round,
            phase: self.phase,
            initiative: self.initiative,
            players,
            enchantments: self.simulation.enchantments.clone(),
            overrides: self.simulation.overrides.clone(),
            win_checks: self.simulation.win_checks.clone(),
            next_order: self.simulation.next_order,
            rng: crate::rng::stream(&seed, crate::rng::Stream::Effects),
        })
    }

    /// Sprawdza własny ruch tak samo jak `validate_move` na pełnym stanie.
    pub fn validate_move(&self, content: &Content<'_>, mv: &Move) -> Result<MoveCost, MoveError> {
        self.mover().validate(content, mv)
    }

    /// Koszty i skutki własnego ruchu, jak `preview` na pełnym stanie.
    #[must_use]
    pub fn preview(&self, content: &Content<'_>, mv: &Move) -> Preview {
        self.mover().preview(content, mv)
    }

    /// Ruch „bez zmian” (R-RUN-09).
    #[must_use]
    pub fn default_move(&self) -> Move {
        Move {
            attacks: self.me.attacks,
            defenses: self.me.defenses,
            card: CardAction::None,
        }
    }

    fn mover(&self) -> Mover<'_> {
        Mover {
            rules: &self.rules,
            finished: matches!(self.phase, Phase::Finished(_)),
            stats: self.me.stats,
            schools: self.me.schools,
            specialization: self.specialization,
            focus: self.me.focus,
            stamina: self.me.stamina,
            attacks: self.me.attacks,
            defenses: self.me.defenses,
            hand: &self.hand,
            used_interventions: &self.me.used_interventions,
        }
    }
}
