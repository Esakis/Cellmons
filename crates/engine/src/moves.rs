//! Ruch gracza: walidacja, podgląd kosztów i ruch domyślny (R-RUN-04…06, R-RUN-09).

use std::fmt;

use crate::card::{CardDef, CardId, Content};
use crate::rules::RulesConfig;
use crate::state::{DuelState, PlayerState};
use crate::types::{Kind, SLOTS, Schools, Side, Specialization, Stats};

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
    Mover::of(state, side).validate(content, mv)
}

/// Koszty i skutki ruchu do pokazania przed jego zatwierdzeniem.
#[must_use]
pub fn preview(state: &DuelState, content: &Content<'_>, side: Side, mv: &Move) -> Preview {
    Mover::of(state, side).preview(content, mv)
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
    changes_between(&player.attacks, &player.defenses, mv)
}

fn changes_between(attacks: &[Kind; SLOTS], defenses: &[Kind; SLOTS], mv: &Move) -> u8 {
    let mut changes = 0;
    for slot in 0..SLOTS {
        changes += u8::from(attacks[slot] != mv.attacks[slot]);
        changes += u8::from(defenses[slot] != mv.defenses[slot]);
    }
    changes
}

/// Koszt rzucenia karty przez stronę i mnożnik jej efektów.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpellCost {
    pub(crate) focus: i32,
    pub(crate) stamina: i32,
    /// Poziom szkoły przy sprzężeniu (R-CZA-06), inaczej 1.
    pub(crate) scale: i16,
}

/// Koszt rzucenia karty `card` przez stronę `side` (zob. `Mover::spell_cost`).
pub(crate) fn spell_cost(state: &DuelState, side: Side, card: &CardDef) -> SpellCost {
    Mover::of(state, side).spell_cost(card)
}

/// Dane strony potrzebne do sprawdzenia ruchu: ze stanu pojedynku albo z widoku gracza
/// (`docs/06-silnik-gry.md`, rozdz. 8), więc sprawdzanie nie wymaga ukrytych informacji.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mover<'a> {
    pub(crate) rules: &'a RulesConfig,
    pub(crate) finished: bool,
    /// Statystyki i poziomy szkół efektywne.
    pub(crate) stats: Stats,
    pub(crate) schools: Schools,
    pub(crate) specialization: Option<Specialization>,
    pub(crate) focus: i32,
    pub(crate) stamina: i32,
    pub(crate) attacks: [Kind; SLOTS],
    pub(crate) defenses: [Kind; SLOTS],
    pub(crate) hand: &'a [CardId],
    pub(crate) used_interventions: &'a [CardId],
}

impl<'a> Mover<'a> {
    pub(crate) fn of(state: &'a DuelState, side: Side) -> Self {
        let player = state.player(side);
        Self {
            rules: &state.rules,
            finished: state.is_finished(),
            stats: state.stats(side),
            schools: state.schools(side),
            specialization: player.specialization,
            focus: player.focus,
            stamina: player.stamina,
            attacks: player.attacks,
            defenses: player.defenses,
            hand: &player.hand,
            used_interventions: &player.used_interventions,
        }
    }

    pub(crate) fn validate(&self, content: &Content<'_>, mv: &Move) -> Result<MoveCost, MoveError> {
        let cost = self.cost(content, mv);
        self.check(content, mv, &cost)?;
        Ok(cost)
    }

    pub(crate) fn preview(&self, content: &Content<'_>, mv: &Move) -> Preview {
        let cost = self.cost(content, mv);
        Preview {
            cost,
            focus_after: self.focus - cost.focus,
            max_attack_stamina: mv.attacks.iter().map(|&kind| self.stats.attack(kind)).sum(),
            validity: self.check(content, mv, &cost),
        }
    }

    /// Koszt z karty, dopłata za sprzężenie (R-CZA-06), a na końcu redukcja specjalizacji,
    /// liczona już z dopłatą (R-CZA-09, Z-SPC).
    pub(crate) fn spell_cost(&self, card: &CardDef) -> SpellCost {
        let mut cost = SpellCost {
            focus: i32::from(card.cost.focus),
            stamina: i32::from(card.cost.stamina),
            scale: 1,
        };
        if let Some(coupling) = card.keywords.coupling {
            let level = self.schools.level(coupling.school);
            if level >= coupling.level.max(1) {
                cost.focus += i32::from(level);
                cost.stamina += 5 * i32::from(level);
                cost.scale = i16::from(level);
            }
        }
        let reduce = |value: i32| (2 * value + 1) / 3;
        match self.specialization {
            Some(Specialization::School(school)) if card.requires.level(school) > 0 => {
                cost.stamina = reduce(cost.stamina);
            }
            Some(Specialization::Essence) => cost.stamina = reduce(cost.stamina),
            Some(Specialization::Concentration) => cost.focus = reduce(cost.focus),
            Some(Specialization::School(_)) | None => {}
        }
        cost
    }

    fn cost(&self, content: &Content<'_>, mv: &Move) -> MoveCost {
        let changes = changes_between(&self.attacks, &self.defenses, mv);
        let spell = self
            .cast_card(content, mv.card)
            .ok()
            .flatten()
            .map(|card| self.spell_cost(card));
        MoveCost {
            changes,
            focus: i32::from(changes) * self.rules.duel.change_cost
                + spell.map_or(0, |cost| cost.focus),
            stamina: spell.map_or(0, |cost| cost.stamina),
        }
    }

    fn check(&self, content: &Content<'_>, mv: &Move, cost: &MoveCost) -> Result<(), MoveError> {
        if self.finished {
            return Err(MoveError::WrongPhase);
        }
        if let CardAction::Discard(index) = mv.card {
            // Odrzucenie nic nie kosztuje (R-RUN-06, p. 5; Z-DSC).
            self.hand_card(index)?;
        }
        if let Some(card) = self.cast_card(content, mv.card)? {
            if !self.schools.meets(&card.requires) {
                return Err(MoveError::RequirementsNotMet);
            }
            if card.keywords.intervention && self.used_interventions.contains(&card.id) {
                return Err(MoveError::InterventionUsed);
            }
        }
        if self.focus < cost.focus {
            return Err(MoveError::NotEnoughFocus);
        }
        if self.stamina < cost.stamina {
            return Err(MoveError::NotEnoughStamina);
        }
        Ok(())
    }

    /// Definicja rzucanej karty albo `None`, gdy ruch nie rzuca karty.
    fn cast_card<'c>(
        &self,
        content: &Content<'c>,
        action: CardAction,
    ) -> Result<Option<&'c CardDef>, MoveError> {
        let CardAction::Cast(index) = action else {
            return Ok(None);
        };
        let id = self.hand_card(index)?;
        content.get(id).map(Some).ok_or(MoveError::UnknownCard)
    }

    fn hand_card(&self, index: u8) -> Result<CardId, MoveError> {
        self.hand
            .get(usize::from(index))
            .copied()
            .ok_or(MoveError::InvalidHandIndex)
    }
}
