//! Przebieg rundy (R-RUN-01…15, R-ATK-01…07).

use std::fmt;

use crate::card::Content;
use crate::end::{self, Outcome};
use crate::event::{Event, EventBuf};
use crate::moves::{CardAction, Move, MoveError, count_changes, validate_move};
use crate::state::{DuelState, Phase};
use crate::types::{SLOTS, Side};

/// Błąd rozstrzygania rundy albo zakończenia pojedynku; stan się nie zmienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuelError {
    /// Pojedynek jest już zakończony.
    Finished,
    /// Ruch strony jest niepoprawny.
    InvalidMove { side: Side, error: MoveError },
}

impl fmt::Display for DuelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Finished => f.write_str("pojedynek jest zakończony"),
            Self::InvalidMove { side, error } => write!(f, "ruch gracza {side:?}: {error}"),
        }
    }
}

impl std::error::Error for DuelError {}

/// Ż, W i K gracza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerTotals {
    pub hp: i32,
    pub stamina: i32,
    pub focus: i32,
}

/// Podsumowanie rozegranej rundy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundSummary {
    /// Numer rozegranej rundy.
    pub round: u8,
    /// Wartości stron A i B na koniec rundy, przed przyrostem K w następnej.
    pub totals: [PlayerTotals; 2],
    /// Wynik, jeśli pojedynek się zakończył.
    pub outcome: Option<Outcome>,
}

/// Rozstrzyga rundę dla ruchów stron A i B (`docs/06-silnik-gry.md`, rozdz. 5.1). Najpierw
/// sprawdza oba ruchy; przy błędzie stan zostaje bez zmian.
pub fn resolve_round(
    state: &mut DuelState,
    content: &Content<'_>,
    moves: &[Move; 2],
    events: &mut EventBuf,
) -> Result<RoundSummary, DuelError> {
    if state.is_finished() {
        return Err(DuelError::Finished);
    }
    for side in Side::BOTH {
        validate_move(state, content, side, &moves[side.index()])
            .map_err(|error| DuelError::InvalidMove { side, error })?;
    }
    reveal(state, moves, events);
    let played = spell_phase(state, content, moves, events);
    attack_phase(state, events);
    draw_replacements(state, played, events);
    Ok(finish_round(state, events))
}

/// Początek rundy: przyrost K (R-RUN-01, R-RUN-02) i wyzerowanie serii trafień (R-RUN-03).
pub(crate) fn begin_round(state: &mut DuelState) {
    let income = state.rules.duel.focus_per_round;
    let cap = state.rules.duel.focus_cap;
    for player in &mut state.players {
        let raised = player.focus + income;
        // Limit ogranicza tylko przyrost; K już ponad limitem zostaje.
        player.focus = cap.map_or(raised, |cap| raised.min(cap).max(player.focus));
        player.combo = 0;
    }
}

/// Ujawnienie ruchów, opłata za zmiany ustawień (R-RUN-05, R-RUN-10) i zapis nowych ustawień.
fn reveal(state: &mut DuelState, moves: &[Move; 2], events: &mut EventBuf) {
    let change_cost = state.rules.duel.change_cost;
    let mut paid = [0; 2];
    for side in Side::BOTH {
        let mv = &moves[side.index()];
        let player = state.player_mut(side);
        let cost = i32::from(count_changes(player, mv)) * change_cost;
        player.focus -= cost;
        player.attacks = mv.attacks;
        player.defenses = mv.defenses;
        paid[side.index()] = cost;
    }
    events.push(Event::MovesRevealed {
        moves: *moves,
        change_cost: paid,
    });
}

/// Faza czarów (R-RUN-11): rzucenia i odrzucenia w kolejności inicjatywy. Zwraca indeks w ręce,
/// z którego każda strona (A, B) zagrała kartę, do dobrania na jej miejsce (R-RUN-14).
fn spell_phase(
    state: &mut DuelState,
    content: &Content<'_>,
    moves: &[Move; 2],
    events: &mut EventBuf,
) -> [Option<usize>; 2] {
    let mut played = [None; 2];
    for side in state.initiative_order() {
        let (index, cast) = match moves[side.index()].card {
            CardAction::None => continue,
            CardAction::Cast(index) => (usize::from(index), true),
            CardAction::Discard(index) => (usize::from(index), false),
        };
        let player = state.player_mut(side);
        let Some(card) = player.hand.pop_at(index) else {
            continue;
        };
        played[side.index()] = Some(index);
        player.discard.push(card);
        if !cast {
            events.push(Event::CardDiscarded { side, card });
            continue;
        }
        // R-RUN-12: rzucający płaci W i K, o ile nadal może; inaczej czar przepada (Z-SPW).
        // Efekty kart dochodzą w fazie F2.
        match content.get(card) {
            Some(def)
                if player.focus >= i32::from(def.cost.focus)
                    && player.stamina >= i32::from(def.cost.stamina) =>
            {
                let (focus, stamina) = (i32::from(def.cost.focus), i32::from(def.cost.stamina));
                player.focus -= focus;
                player.stamina -= stamina;
                if def.keywords.intervention {
                    player.used_interventions.push(card);
                }
                events.push(Event::SpellCast {
                    side,
                    card,
                    focus,
                    stamina,
                });
            }
            _ => events.push(Event::SpellFizzled { side, card }),
        }
    }
    played
}

/// Faza ataków: 6 ataków naprzemiennie, zaczyna strona z inicjatywą (R-ATK-01). Wykonują się
/// zawsze wszystkie, także gdy Ż spadnie do 0 lub niżej (R-ATK-06).
fn attack_phase(state: &mut DuelState, events: &mut EventBuf) {
    let order = state.initiative_order();
    for slot in 0..SLOTS {
        for attacker in order {
            attack(state, attacker, slot, events);
        }
    }
}

/// Atak strony `attacker` w slocie `slot` (R-ATK-02…05, R-ATK-07).
fn attack(state: &mut DuelState, attacker: Side, slot: usize, events: &mut EventBuf) {
    let block_min_stamina = state.rules.duel.block_min_stamina;
    let multipliers = state.rules.duel.combo_multipliers;
    let (striker, target) = state.pair_mut(attacker);
    let kind = striker.attacks[slot];
    let slot_number = slot as u8;

    // R-ATK-02: blok, gdy obrona ma ten sam typ, a obrońca ma dość W. Przy mniejszej W atak
    // trafia niezależnie od obrony (R-ATK-03).
    if target.defenses[slot] == kind && target.stamina >= block_min_stamina {
        // R-ATK-05: blok zeruje serię i nic nie kosztuje.
        striker.combo = 0;
        events.push(Event::AttackBlocked {
            attacker,
            slot: slot_number,
            kind,
        });
        return;
    }

    // R-ATK-04: seria liczy trafienia atakującego w tej rundzie, niezależnie od typu ataku.
    striker.combo += 1;
    let multiplier = multipliers[usize::from(striker.combo).min(SLOTS) - 1];
    let value = striker.stats.attack(kind);
    let damage = (value * multiplier - target.stats.defense(kind)).max(0);
    // R-ATK-07: Ż może spaść poniżej 0, W nie.
    target.hp -= damage;
    let stamina_cost = value.min(striker.stamina);
    striker.stamina -= stamina_cost;
    events.push(Event::AttackHit {
        attacker,
        slot: slot_number,
        kind,
        multiplier,
        damage,
        stamina_cost,
    });
}

/// Dobranie z wierzchu talii na miejsce zagranej lub odrzuconej karty (R-RUN-14). Gdy talia
/// jest pusta, gracz nie dobiera (Z-EMP).
fn draw_replacements(state: &mut DuelState, played: [Option<usize>; 2], events: &mut EventBuf) {
    for side in state.initiative_order() {
        let Some(index) = played[side.index()] else {
            continue;
        };
        let player = state.player_mut(side);
        // Po zagraniu karty ręka nie jest pełna; warunek tylko chroni bufor o stałej pojemności.
        if player.hand.is_full() {
            continue;
        }
        let Some(card) = player.deck.pop() else {
            continue;
        };
        player.hand.insert(index.min(player.hand.len()), card);
        events.push(Event::CardDrawn { side, card });
    }
}

/// Koniec rundy: warunki końca pojedynku (R-RUN-15), a jeśli trwa dalej, początek następnej rundy.
fn finish_round(state: &mut DuelState, events: &mut EventBuf) -> RoundSummary {
    let round = state.round;
    let totals = Side::BOTH.map(|side| {
        let player = state.player(side);
        PlayerTotals {
            hp: player.hp,
            stamina: player.stamina,
            focus: player.focus,
        }
    });
    let outcome = end::knockout(state)
        .or_else(|| (round >= state.rules.duel.rounds).then(|| end::by_hp_percent(state)));
    match outcome {
        Some(outcome) => {
            state.phase = Phase::Finished(outcome);
            events.push(Event::DuelEnded { outcome });
        }
        None => {
            state.round += 1;
            begin_round(state);
            let focus = Side::BOTH.map(|side| state.player(side).focus);
            events.push(Event::RoundStarted {
                round: state.round,
                focus,
            });
        }
    }
    RoundSummary {
        round,
        totals,
        outcome,
    }
}
