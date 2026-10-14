//! Ruch gracza: walidacja, podgląd kosztów i ruch domyślny (R-RUN-04…06, R-RUN-09).

use std::fmt;

use crate::card::{CardDef, CardId, Content};
use crate::state::{DuelState, PlayerState};
use crate::types::{Kind, SLOTS, Side};

/// Akcja karty w ruchu (R-RUN-04); liczba to indeks karty w ręce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAction {
    None,
    Cast(u8),
    Discard(u8),
}

/// Ruch na jedną rundę (R-RUN-04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub attacks: [Kind; SLOTS],
    pub defenses: [Kind; SLOTS],
    pub card: CardAction,
}

/// Powód odrzucenia ruchu (kody dla protokołu, `docs/06-silnik-gry.md`, rozdz. 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveError {
    /// Pojedynek jest zakończony.
    WrongPhase,
    /// Indeks karty poza ręką.
    InvalidHandIndex,
    /// Poziomy szkół postaci nie spełniają wymagań karty (R-RUN-06, p. 3).
    RequirementsNotMet,
    /// Ta interwencja była już rzucona w tym pojedynku (R-RUN-06, p. 4).
    InterventionUsed,
    /// Za mało K na zmiany ustawień i czar (R-RUN-06, p. 1).
    NotEnoughFocus,
    /// Za mało W na czar (R-RUN-06, p. 2).
    NotEnoughStamina,
    /// Karty z ręki nie ma w przekazanej treści gry (błąd wywołującego).
    UnknownCard,
}

impl fmt::Display for MoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::WrongPhase => "pojedynek jest zakończony",
            Self::InvalidHandIndex => "nie ma takiej karty w ręce",
            Self::RequirementsNotMet => "postać nie spełnia wymagań karty",
            Self::InterventionUsed => "ta interwencja była już użyta",
            Self::NotEnoughFocus => "za mało punktów koncentracji",
            Self::NotEnoughStamina => "za mało punktów wytrzymałości",
            Self::UnknownCard => "nieznana karta",
        };
        f.write_str(text)
    }
}

impl std::error::Error for MoveError {}

/// Koszt ruchu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MoveCost {
    /// Liczba zmienionych slotów (R-RUN-05).
    pub changes: u8,
    /// K: zmiany razy koszt zmiany plus koszt K czaru.
    pub focus: i32,
    /// W: koszt W czaru.
    pub stamina: i32,
}

/// Podgląd ruchu dla ekranu walki, także dla ruchu niepoprawnego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preview {
    pub cost: MoveCost,
    /// K po opłatach; ujemna, gdy ruch jest za drogi.
    pub focus_after: i32,
    /// Zużycie W przez ataki, gdy wszystkie trafią (R-ATK-04); faktyczny ubytek nie przekroczy
    /// posiadanej W (R-ATK-07).
    pub max_attack_stamina: i32,
    /// Wynik walidacji (R-RUN-06).
    pub validity: Result<(), MoveError>,
}

/// Sprawdza ruch strony (R-RUN-06) i zwraca jego koszt. Przy kilku problemach zwraca pierwszy
/// w kolejności: faza, indeks karty, karta w treści, wymagania szkół, interwencja, K, W.
pub fn validate_move(
    state: &DuelState,
    content: &Content<'_>,
    side: Side,
    mv: &Move,
) -> Result<MoveCost, MoveError> {
    let player = state.player(side);
    let cost = move_cost(state, content, player, mv);
    check_move(state, content, player, mv, &cost)?;
    Ok(cost)
}

/// Koszty i skutki ruchu do pokazania przed jego zatwierdzeniem.
#[must_use]
pub fn preview(state: &DuelState, content: &Content<'_>, side: Side, mv: &Move) -> Preview {
    let player = state.player(side);
    let cost = move_cost(state, content, player, mv);
    Preview {
        cost,
        focus_after: player.focus - cost.focus,
        max_attack_stamina: mv
            .attacks
            .iter()
            .map(|&kind| player.stats.attack(kind))
            .sum(),
        validity: check_move(state, content, player, mv, &cost),
    }
}

/// Ruch „bez zmian”: ustawienia z poprzedniej rundy, bez akcji karty (R-RUN-09).
#[must_use]
pub fn default_move(state: &DuelState, side: Side) -> Move {
    let player = state.player(side);
    Move {
        attacks: player.attacks,
        defenses: player.defenses,
        card: CardAction::None,
    }
}

/// Liczba slotów różnych od ustawień z poprzedniej rundy; liczy się tylko stan końcowy (R-RUN-05).
pub(crate) fn count_changes(player: &PlayerState, mv: &Move) -> u8 {
    let mut changes = 0;
    for slot in 0..SLOTS {
        changes += u8::from(player.attacks[slot] != mv.attacks[slot]);
        changes += u8::from(player.defenses[slot] != mv.defenses[slot]);
    }
    changes
}

fn move_cost(
    state: &DuelState,
    content: &Content<'_>,
    player: &PlayerState,
    mv: &Move,
) -> MoveCost {
    let changes = count_changes(player, mv);
    let spell = cast_card(player, content, mv.card).ok().flatten();
    let spell_focus = spell.map_or(0, |card| i32::from(card.cost.focus));
    MoveCost {
        changes,
        focus: i32::from(changes) * state.rules.duel.change_cost + spell_focus,
        stamina: spell.map_or(0, |card| i32::from(card.cost.stamina)),
    }
}

fn check_move(
    state: &DuelState,
    content: &Content<'_>,
    player: &PlayerState,
    mv: &Move,
    cost: &MoveCost,
) -> Result<(), MoveError> {
    if state.is_finished() {
        return Err(MoveError::WrongPhase);
    }
    if let CardAction::Discard(index) = mv.card {
        // Odrzucenie nic nie kosztuje (R-RUN-06, p. 5; Z-DSC).
        hand_card(player, index)?;
    }
    if let Some(card) = cast_card(player, content, mv.card)? {
        if !player.schools.meets(&card.requires) {
            return Err(MoveError::RequirementsNotMet);
        }
        if card.keywords.intervention && player.used_interventions.contains(&card.id) {
            return Err(MoveError::InterventionUsed);
        }
    }
    if player.focus < cost.focus {
        return Err(MoveError::NotEnoughFocus);
    }
    if player.stamina < cost.stamina {
        return Err(MoveError::NotEnoughStamina);
    }
    Ok(())
}

/// Definicja rzucanej karty albo `None`, gdy ruch nie rzuca karty.
fn cast_card<'a>(
    player: &PlayerState,
    content: &Content<'a>,
    action: CardAction,
) -> Result<Option<&'a CardDef>, MoveError> {
    let CardAction::Cast(index) = action else {
        return Ok(None);
    };
    let id = hand_card(player, index)?;
    content.get(id).map(Some).ok_or(MoveError::UnknownCard)
}

fn hand_card(player: &PlayerState, index: u8) -> Result<CardId, MoveError> {
    player
        .hand
        .get(usize::from(index))
        .copied()
        .ok_or(MoveError::InvalidHandIndex)
}
