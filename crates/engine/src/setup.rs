//! Przygotowanie pojedynku (R-POJ-01…06, R-TAL-01, R-TAL-02, R-TAL-05).

use std::fmt;
use std::num::NonZeroU32;

use arrayvec::ArrayVec;
use rand_chacha::rand_core::Rng;

use crate::card::{CardId, Content};
use crate::rng::{self, Stream};
use crate::round::begin_round;
use crate::rules::{DefaultSettings, MAX_DECK_SIZE, RulesConfig, RulesError};
use crate::state::{DuelState, Phase, PlayerState};
use crate::types::{Kind, MAX_POOL, MAX_STAT, SLOTS, Schools, Side, Stats};

/// Snapshot postaci z chwili startu pojedynku (R-POJ-01). Specjalizacja i dar dochodzą w fazie F2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarSnapshot {
    /// Statystyki efektywne.
    pub stats: Stats,
    pub schools: Schools,
}

/// Postać i talia gracza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSetup {
    pub avatar: AvatarSnapshot,
    /// Talia z kolekcji gracza; może mieć mniej kart niż `deck.size` (R-TAL-05).
    pub deck: ArrayVec<CardId, MAX_DECK_SIZE>,
}

/// Wejście pojedynku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuelSetup {
    pub rules: RulesConfig,
    /// Ziarno wylosowane przez serwer (ADR-007).
    pub seed: [u8; 32],
    pub players: [PlayerSetup; 2],
}

/// Niepoprawne wejście pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupError {
    Rules(RulesError),
    /// AF, OF, AM lub OM poza 0..=MAX_STAT albo Ż, W poza 1..=MAX_POOL.
    InvalidStats(Side),
    /// Poziom szkoły powyżej 4.
    InvalidSchools(Side),
    /// Talia ma więcej kart niż `deck.size` (R-TAL-01).
    DeckTooLarge(Side),
    /// Więcej kopii karty niż `deck.max_copies` (R-TAL-02).
    TooManyCopies(Side, CardId),
    /// Karty nie ma w treści gry.
    UnknownCard(Side, CardId),
    /// Talię trzeba uzupełnić (R-TAL-05), a treść nie ma kart.
    EmptyCardPool,
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rules(error) => error.fmt(f),
            Self::InvalidStats(side) => write!(f, "gracz {side:?}: statystyki poza zakresem"),
            Self::InvalidSchools(side) => write!(f, "gracz {side:?}: poziom szkoły powyżej 4"),
            Self::DeckTooLarge(side) => write!(f, "gracz {side:?}: za dużo kart w talii"),
            Self::TooManyCopies(side, card) => {
                write!(f, "gracz {side:?}: za dużo kopii karty {card}")
            }
            Self::UnknownCard(side, card) => write!(f, "gracz {side:?}: nieznana karta {card}"),
            Self::EmptyCardPool => write!(f, "brak kart do uzupełnienia talii"),
        }
    }
}

impl std::error::Error for SetupError {}

/// Rozpoczyna pojedynek: zwraca stan na początku 1. rundy, gotowy na ruchy.
pub fn new_duel(setup: &DuelSetup, content: &Content<'_>) -> Result<DuelState, SetupError> {
    let rules = &setup.rules;
    rules.validate().map_err(SetupError::Rules)?;
    for side in Side::BOTH {
        validate_player(rules, content, side, &setup.players[side.index()])?;
    }
    let mut decks = [ArrayVec::new(), ArrayVec::new()];
    for side in Side::BOTH {
        let deck = &setup.players[side.index()].deck;
        decks[side.index()] = prepare_deck(rules, content, &setup.seed, side, deck)?;
    }
    let avatars = Side::BOTH.map(|side| setup.players[side.index()].avatar);
    Ok(start(rules, avatars, decks, draw_initiative(&setup.seed)))
}

/// Stan na początku 1. rundy dla gotowych talii i znanej inicjatywy. `rules` muszą przejść
/// walidację.
pub(crate) fn start(
    rules: &RulesConfig,
    avatars: [AvatarSnapshot; 2],
    decks: [ArrayVec<CardId, MAX_DECK_SIZE>; 2],
    initiative: Side,
) -> DuelState {
    let [deck_a, deck_b] = decks;
    let mut state = DuelState {
        rules: rules.clone(),
        round: 1,
        phase: Phase::AwaitingMoves,
        initiative,
        players: [
            new_player(rules, &avatars[0], deck_a),
            new_player(rules, &avatars[1], deck_b),
        ],
    };
    begin_round(&mut state);
    state
}

/// Gracz na starcie: pełne Ż i W, K = 0 (R-POJ-02), ustawienia domyślne (R-POJ-06) i ręka
/// z wierzchu talii (R-POJ-04).
fn new_player(
    rules: &RulesConfig,
    avatar: &AvatarSnapshot,
    mut deck: ArrayVec<CardId, MAX_DECK_SIZE>,
) -> PlayerState {
    let (attacks, defenses) = default_settings(rules.duel.default_settings, &avatar.stats);
    let mut hand = ArrayVec::new();
    for _ in 0..rules.duel.hand_size {
        let Some(card) = deck.pop() else { break };
        hand.push(card);
    }
    PlayerState {
        stats: avatar.stats,
        schools: avatar.schools,
        hp: avatar.stats.hp_max,
        hp_start: avatar.stats.hp_max,
        stamina: avatar.stats.stamina_max,
        focus: 0,
        attacks,
        defenses,
        hand,
        deck,
        discard: ArrayVec::new(),
        used_interventions: ArrayVec::new(),
    }
}

/// Ustawienia odniesienia dla kosztu zmian w 1. rundzie (R-POJ-06).
pub(crate) fn default_settings(
    choice: DefaultSettings,
    stats: &Stats,
) -> ([Kind; SLOTS], [Kind; SLOTS]) {
    match choice {
        DefaultSettings::HighestStat => {
            // Przy remisie statystyk F (Z-DEF).
            let attack = if stats.mag_atk > stats.phys_atk {
                Kind::Mag
            } else {
                Kind::Phys
            };
            let defense = if stats.mag_def > stats.phys_def {
                Kind::Mag
            } else {
                Kind::Phys
            };
            ([attack; SLOTS], [defense; SLOTS])
        }
    }
}

/// Sprawdza snapshot postaci i talię gracza (R-POJ-01, R-TAL-01, R-TAL-02).
fn validate_player(
    rules: &RulesConfig,
    content: &Content<'_>,
    side: Side,
    player: &PlayerSetup,
) -> Result<(), SetupError> {
    if !valid_stats(&player.avatar.stats) {
        return Err(SetupError::InvalidStats(side));
    }
    if !player.avatar.schools.is_valid() {
        return Err(SetupError::InvalidSchools(side));
    }
    if player.deck.len() > usize::from(rules.deck.size) {
        return Err(SetupError::DeckTooLarge(side));
    }
    for &card in &player.deck {
        if content.get(card).is_none() {
            return Err(SetupError::UnknownCard(side, card));
        }
        let copies = player.deck.iter().filter(|&&other| other == card).count();
        if copies > usize::from(rules.deck.max_copies) {
            return Err(SetupError::TooManyCopies(side, card));
        }
    }
    Ok(())
}

fn valid_stats(stats: &Stats) -> bool {
    let stat = |value: i32| (0..=MAX_STAT).contains(&value);
    let pool = |value: i32| (1..=MAX_POOL).contains(&value);
    [stats.phys_atk, stats.phys_def, stats.mag_atk, stats.mag_def]
        .into_iter()
        .all(stat)
        && pool(stats.hp_max)
        && pool(stats.stamina_max)
}

/// Talia gotowa do gry: uzupełniona losowymi kartami z puli wszystkich kart, ze zwracaniem
/// (R-TAL-05), i potasowana (R-POJ-03). Oba kroki używają strumienia talii gracza.
fn prepare_deck(
    rules: &RulesConfig,
    content: &Content<'_>,
    seed: &[u8; 32],
    side: Side,
    deck: &ArrayVec<CardId, MAX_DECK_SIZE>,
) -> Result<ArrayVec<CardId, MAX_DECK_SIZE>, SetupError> {
    let mut rng = rng::stream(seed, Stream::deck(side));
    let mut deck = deck.clone();
    let size = usize::from(rules.deck.size);
    if rules.deck.fill_missing_with_random && deck.len() < size {
        let pool = content.cards();
        let count = u32::try_from(pool.len())
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(SetupError::EmptyCardPool)?;
        while deck.len() < size {
            deck.push(pool[rng::below(&mut rng, count) as usize].id);
        }
    }
    rng::shuffle(&mut rng, &mut deck);
    Ok(deck)
}

/// Inicjatywa losowana raz na cały pojedynek (R-POJ-05).
fn draw_initiative(seed: &[u8; 32]) -> Side {
    let mut rng = rng::stream(seed, Stream::Initiative);
    if rng.next_u32() & 1 == 0 {
        Side::A
    } else {
        Side::B
    }
}
