//! Narzędzia testowe: zasady klasyczne, karty testowe, konstruktor scenariuszy i losowe pojedynki
//! (`docs/06-silnik-gry.md`, rozdz. 11). Dostępne w testach silnika oraz przez cechę `testkit`.
//! Funkcje tego modułu mogą panikować przy błędnym użyciu, bo służą tylko testom.

use std::num::NonZeroU32;
use std::ops::Range;

use arrayvec::ArrayVec;
use rand_chacha::ChaCha8Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

use crate::card::{CardDef, CardId, Content, Cost, Keywords};
use crate::end::{Termination, end_by};
use crate::event::EventBuf;
use crate::hash::state_hash;
use crate::moves::{CardAction, Move, default_move, validate_move};
use crate::replay::ReplayRecord;
use crate::rng::below;
use crate::round::resolve_round;
use crate::rules::{
    CounterResolution, DeckRules, DefaultSettings, DuelRules, MAX_HAND_SIZE, RulesConfig,
};
use crate::setup::{self, AvatarSnapshot, DuelSetup, PlayerSetup, new_duel};
use crate::state::DuelState;
use crate::types::{Kind, SLOTS, School, Schools, Side, Stats};

/// Wersja treści używana w testach.
pub const TEST_CONTENT_VERSION: [u8; 32] = [0; 32];

/// Zasady klasyczne, takie jak w `content/rules.toml` (zgodność sprawdza test).
#[must_use]
pub fn classic_rules() -> RulesConfig {
    RulesConfig {
        duel: DuelRules {
            rounds: 5,
            attacks_per_round: 3,
            focus_per_round: 4,
            focus_cap: None,
            change_cost: 1,
            hand_size: 5,
            combo_multipliers: [1, 2, 4],
            block_min_stamina: 1,
            default_settings: DefaultSettings::HighestStat,
            counter_resolution: CounterResolution::InitiativeFirst,
            move_timeout_after_opponent_s: 60,
            round_timeout_s: 90,
            max_consecutive_timeouts: 3,
            reconnect_grace_s: 60,
        },
        deck: DeckRules {
            size: 20,
            max_copies: 4,
            fill_missing_with_random: true,
            saved_presets_max: 10,
        },
    }
}

/// Statystyki w kolejności z dokumentacji: AF, OF, AM, OM, Ż, W.
#[must_use]
pub const fn stats(
    phys_atk: i32,
    phys_def: i32,
    mag_atk: i32,
    mag_def: i32,
    hp_max: i32,
    stamina_max: i32,
) -> Stats {
    Stats {
        phys_atk,
        phys_def,
        mag_atk,
        mag_def,
        hp_max,
        stamina_max,
    }
}

/// Karta testowa bez efektu i bez wymagań.
#[must_use]
pub const fn noop_card(id: CardId, focus: u16, stamina: u16) -> CardDef {
    CardDef {
        id,
        requires: Schools::NONE,
        cost: Cost { focus, stamina },
        keywords: Keywords {
            intervention: false,
        },
    }
}

/// Treść z podanych kart (posortowanych rosnąco według `id`).
#[must_use]
pub fn content(cards: &[CardDef]) -> Content<'_> {
    Content::new(TEST_CONTENT_VERSION, cards).expect("karty testowe są posortowane i poprawne")
}

/// Gracz bez biegłości w szkołach, z talią z podanych kart.
#[must_use]
pub fn player(stats: Stats, deck: &[CardId]) -> PlayerSetup {
    PlayerSetup {
        avatar: AvatarSnapshot {
            stats,
            schools: Schools::NONE,
        },
        deck: deck.iter().copied().collect(),
    }
}

/// Ruch w zapisie z dokumentacji, np. `mv("FMF", "MFM", CardAction::None)`: F to typ fizyczny,
/// M magiczny.
#[must_use]
pub fn mv(attacks: &str, defenses: &str, card: CardAction) -> Move {
    Move {
        attacks: kinds(attacks),
        defenses: kinds(defenses),
        card,
    }
}

fn kinds(text: &str) -> [Kind; SLOTS] {
    let kinds: Vec<Kind> = text
        .chars()
        .map(|letter| match letter {
            'F' => Kind::Phys,
            'M' => Kind::Mag,
            other => panic!("nieznany typ {other:?} w {text:?}"),
        })
        .collect();
    kinds
        .try_into()
        .unwrap_or_else(|_| panic!("oczekiwano {SLOTS} znaków F/M: {text:?}"))
}

/// Pojedynek z jawnie podaną inicjatywą, ręką i talią, bez losowania (`docs/06-silnik-gry.md`,
/// rozdz. 11).
#[derive(Debug, Clone)]
pub struct Scenario {
    rules: RulesConfig,
    avatars: [AvatarSnapshot; 2],
    hands: [Vec<CardId>; 2],
    decks: [Vec<CardId>; 2],
    initiative: Side,
}

impl Scenario {
    /// Zasady klasyczne, inicjatywa A, brak biegłości, puste ręce i talie.
    #[must_use]
    pub fn new(a: Stats, b: Stats) -> Self {
        Self {
            rules: classic_rules(),
            avatars: [a, b].map(|stats| AvatarSnapshot {
                stats,
                schools: Schools::NONE,
            }),
            hands: [Vec::new(), Vec::new()],
            decks: [Vec::new(), Vec::new()],
            initiative: Side::A,
        }
    }

    #[must_use]
    pub fn rules(mut self, rules: RulesConfig) -> Self {
        self.rules = rules;
        self
    }

    #[must_use]
    pub fn initiative(mut self, side: Side) -> Self {
        self.initiative = side;
        self
    }

    #[must_use]
    pub fn schools(mut self, side: Side, schools: Schools) -> Self {
        self.avatars[side.index()].schools = schools;
        self
    }

    /// Ręka; indeksy jak w `CardAction`.
    #[must_use]
    pub fn hand(mut self, side: Side, cards: &[CardId]) -> Self {
        self.hands[side.index()] = cards.to_vec();
        self
    }

    /// Talia; ostatnia karta to wierzch.
    #[must_use]
    pub fn deck(mut self, side: Side, cards: &[CardId]) -> Self {
        self.decks[side.index()] = cards.to_vec();
        self
    }

    /// Stan na początku 1. rundy.
    #[must_use]
    pub fn build(&self) -> DuelState {
        self.rules
            .validate()
            .expect("zasady scenariusza są poprawne");
        let empty = [ArrayVec::new(), ArrayVec::new()];
        let mut state = setup::start(&self.rules, self.avatars, empty, self.initiative);
        for side in Side::BOTH {
            let player = state.player_mut(side);
            player.hand = self.hands[side.index()].iter().copied().collect();
            player.deck = self.decks[side.index()].iter().copied().collect();
        }
        state
    }
}

/// Karty do losowych pojedynków: różne koszty, wymagania szkół i interwencje.
#[must_use]
pub fn sample_cards() -> Vec<CardDef> {
    let requires = |levels: &[(School, u8)]| {
        levels
            .iter()
            .fold(Schools::NONE, |schools, &(school, level)| {
                schools.with(school, level)
            })
    };
    let card = |id, focus, stamina, levels: &[(School, u8)], intervention| CardDef {
        id,
        requires: requires(levels),
        cost: Cost { focus, stamina },
        keywords: Keywords { intervention },
    };
    vec![
        card(1, 0, 0, &[], false),
        card(2, 1, 4, &[], false),
        card(3, 2, 8, &[], false),
        card(4, 1, 12, &[], false),
        card(5, 3, 6, &[], false),
        card(6, 4, 24, &[], false),
        card(7, 2, 6, &[(School::Combat, 1)], false),
        card(8, 2, 12, &[(School::Divine, 2)], false),
        card(9, 1, 8, &[(School::Element, 1), (School::Arcane, 1)], false),
        card(10, 3, 18, &[(School::Necro, 3)], false),
        card(11, 2, 12, &[(School::Tech, 2)], false),
        card(12, 2, 10, &[], true),
        card(13, 1, 4, &[(School::Arcane, 1)], true),
    ]
}

/// Generator testowy z ziarna liczbowego (bez `seed_from_u64`, którego wynik może się zmienić
/// między wersjami biblioteki).
#[must_use]
pub fn seeded_rng(seed: u64) -> ChaCha8Rng {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    ChaCha8Rng::from_seed(bytes)
}

/// Liczba z przedziału `0..n`, `n` > 0.
fn pick(rng: &mut ChaCha8Rng, n: usize) -> usize {
    let bound = u32::try_from(n)
        .ok()
        .and_then(NonZeroU32::new)
        .expect("dodatni przedział");
    below(rng, bound) as usize
}

/// Liczba z przedziału `low..=high`.
fn between(rng: &mut ChaCha8Rng, low: i32, high: i32) -> i32 {
    let span = usize::try_from(high - low + 1).expect("poprawny przedział");
    low + i32::try_from(pick(rng, span)).expect("mały przedział")
}

/// Losowe, poprawne wejście pojedynku z kart `content`: postacie, talie (także niepełne, R-TAL-05),
/// ziarno i drobne zmiany zasad.
#[must_use]
pub fn random_setup(rng: &mut ChaCha8Rng, content: &Content<'_>) -> DuelSetup {
    let mut rules = classic_rules();
    rules.duel.change_cost = between(rng, 1, 2);
    rules.duel.block_min_stamina = between(rng, 1, 2);
    if pick(rng, 4) == 0 {
        rules.duel.focus_cap = Some(10);
    }
    if pick(rng, 8) == 0 {
        rules.deck.fill_missing_with_random = false;
    }
    let mut seed = [0; 32];
    rng.fill_bytes(&mut seed);
    let players = [(); 2].map(|()| random_player(rng, content, &rules));
    DuelSetup {
        rules,
        seed,
        players,
    }
}

fn random_player(rng: &mut ChaCha8Rng, content: &Content<'_>, rules: &RulesConfig) -> PlayerSetup {
    let stats = Stats {
        phys_atk: between(rng, 0, 12),
        phys_def: between(rng, 0, 8),
        mag_atk: between(rng, 0, 12),
        mag_def: between(rng, 0, 8),
        hp_max: between(rng, 20, 120),
        stamina_max: between(rng, 5, 120),
    };
    let mut schools = Schools::NONE;
    for level in &mut schools.0 {
        // Poziomy 0..=2, najczęściej 0.
        *level = (pick(rng, 5) as u8).saturating_sub(2);
    }
    let cards = content.cards();
    // Talia nie może mieć więcej kart, niż pozwala limit kopii (R-TAL-02).
    let length = pick(rng, usize::from(rules.deck.size) + 1)
        .min(cards.len() * usize::from(rules.deck.max_copies));
    let mut deck = ArrayVec::new();
    while deck.len() < length {
        let card = cards[pick(rng, cards.len())].id;
        let copies = deck.iter().filter(|&&other| other == card).count();
        if copies < usize::from(rules.deck.max_copies) {
            deck.push(card);
        }
    }
    PlayerSetup {
        avatar: AvatarSnapshot { stats, schools },
        deck,
    }
}

/// Losowy poprawny ruch: akcja karty losowana spośród możliwych, potem losowe ustawienia, z których
/// cofa się zmiany od końca, dopóki ruch nie zmieści się w K.
#[must_use]
pub fn random_move(
    state: &DuelState,
    content: &Content<'_>,
    side: Side,
    rng: &mut ChaCha8Rng,
) -> Move {
    let keep = default_move(state, side);
    let mut actions: ArrayVec<CardAction, { 1 + 2 * MAX_HAND_SIZE }> = ArrayVec::new();
    actions.push(CardAction::None);
    for index in 0..state.player(side).hand().len() as u8 {
        actions.push(CardAction::Discard(index));
        let cast = Move {
            card: CardAction::Cast(index),
            ..keep
        };
        if validate_move(state, content, side, &cast).is_ok() {
            actions.push(cast.card);
        }
    }
    let mut mv = Move {
        card: actions[pick(rng, actions.len())],
        ..keep
    };
    let bits = rng.next_u32();
    let kind = |bit: usize| {
        if (bits >> bit) & 1 == 0 {
            Kind::Phys
        } else {
            Kind::Mag
        }
    };
    for slot in 0..SLOTS {
        mv.attacks[slot] = kind(slot);
        mv.defenses[slot] = kind(SLOTS + slot);
    }
    for slot in (0..2 * SLOTS).rev() {
        if validate_move(state, content, side, &mv).is_ok() {
            break;
        }
        if slot < SLOTS {
            mv.attacks[slot] = keep.attacks[slot];
        } else {
            mv.defenses[slot - SLOTS] = keep.defenses[slot - SLOTS];
        }
    }
    mv
}

/// Rozgrywa losowy pojedynek do końca i zwraca stan końcowy z zapisem powtórki. Co dwudziesty
/// pojedynek kończy się wcześniej ucieczką, rozłączeniem albo walkowerem. `after_round` dostaje
/// stan po każdej rundzie i po zakończeniu spoza rozgrywki.
pub fn play_random_duel(
    rng: &mut ChaCha8Rng,
    content: &Content<'_>,
    events: &mut EventBuf,
    mut after_round: impl FnMut(&DuelState),
) -> (DuelState, ReplayRecord) {
    let setup = random_setup(rng, content);
    let mut state = new_duel(&setup, content).expect("losowe wejście jest poprawne");
    let mut record = ReplayRecord::new(setup, content);
    let early_end = (pick(rng, 20) == 0).then(|| {
        let round = 1 + pick(rng, usize::from(state.rules().duel.rounds)) as u8;
        let loser = Side::BOTH[pick(rng, 2)];
        let termination = [
            Termination::Forfeit,
            Termination::Disconnect,
            Termination::Timeout,
        ][pick(rng, 3)];
        (round, loser, termination)
    });
    while !state.is_finished() {
        if let Some((round, loser, termination)) = early_end
            && state.round() == round
        {
            end_by(&mut state, loser, termination, events).expect("pojedynek trwa");
            record.end = Some((loser, termination));
        } else {
            let moves = Side::BOTH.map(|side| random_move(&state, content, side, rng));
            resolve_round(&mut state, content, &moves, events).expect("losowe ruchy są poprawne");
            record.moves.push(moves);
        }
        after_round(&state);
    }
    (state, record)
}

/// Skrót z `state_hash` po każdej rundzie losowych pojedynków o ziarnach `seeds`, rozgrywanych
/// kartami z [`sample_cards`]. Podstawa testu natywne = WASM (`docs/06-silnik-gry.md`, rozdz. 7).
#[must_use]
pub fn determinism_digest(seeds: Range<u64>) -> u64 {
    let cards = sample_cards();
    let content = content(&cards);
    let mut digest = blake3::Hasher::new();
    let mut events = EventBuf::new();
    for seed in seeds {
        play_random_duel(&mut seeded_rng(seed), &content, &mut events, |state| {
            digest.update(&state_hash(state).to_le_bytes());
        });
        events.clear();
    }
    let hash = digest.finalize();
    hash.as_bytes()
        .first_chunk::<8>()
        .map_or(0, |bytes| u64::from_le_bytes(*bytes))
}
