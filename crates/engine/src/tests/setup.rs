//! Przygotowanie pojedynku (R-POJ-01…06, R-TAL-01, R-TAL-02, R-TAL-05).

use crate::card::{CardDef, CardId, Content};
use crate::rules::RulesError;
use crate::setup::{DuelSetup, SetupError, new_duel};
use crate::state::DuelState;
use crate::testkit::{TEST_CONTENT_VERSION, classic_rules, content, noop_card, player, stats};
use crate::types::{Kind, School, Schools, Side, Stats};

const A: Stats = stats(6, 1, 3, 2, 60, 50);
const B: Stats = stats(4, 2, 5, 1, 55, 45);

/// 20 różnych kart o identyfikatorach 1..=20.
fn cards() -> Vec<CardDef> {
    (1..=20).map(|id| noop_card(id, 1, 4)).collect()
}

fn all_ids() -> Vec<CardId> {
    (1..=20).collect()
}

fn setup(seed: u8, deck_a: &[CardId], deck_b: &[CardId]) -> DuelSetup {
    DuelSetup {
        rules: classic_rules(),
        seed: [seed; 32],
        players: [player(A, deck_a), player(B, deck_b)],
    }
}

fn start(setup: &DuelSetup) -> DuelState {
    let cards = cards();
    new_duel(setup, &content(&cards)).expect("poprawne ustawienia")
}

/// Karty gracza w kolejności dobierania: ręka, potem talia od wierzchu.
fn draw_order(state: &DuelState, side: Side) -> Vec<CardId> {
    let player = state.player(side);
    player
        .hand()
        .iter()
        .chain(player.deck().iter().rev())
        .copied()
        .collect()
}

fn sorted(mut cards: Vec<CardId>) -> Vec<CardId> {
    cards.sort_unstable();
    cards
}

#[test]
fn r_poj_01_snapshot_does_not_follow_later_changes() {
    let mut setup = setup(1, &all_ids(), &all_ids());
    setup.players[0].avatar.schools = Schools::NONE.with(School::Combat, 2);
    let state = start(&setup);

    setup.players[0].avatar.stats.phys_atk = 99;
    setup.players[0].avatar.schools = Schools::NONE;

    assert_eq!(state.player(Side::A).base_stats(), &A);
    assert_eq!(state.stats(Side::A), A);
    assert_eq!(
        state.player(Side::A).base_schools(),
        &Schools::NONE.with(School::Combat, 2)
    );
}

#[test]
fn r_poj_02_players_start_with_full_hp_and_stamina_and_no_focus() {
    let mut setup = setup(1, &all_ids(), &all_ids());
    // Bez przyrostu z R-RUN-01 widać K z chwili startu.
    setup.rules.duel.focus_per_round = 0;
    let state = start(&setup);

    for (side, stats) in [(Side::A, A), (Side::B, B)] {
        let player = state.player(side);
        assert_eq!(player.hp(), stats.hp_max);
        assert_eq!(player.hp_start(), stats.hp_max);
        assert_eq!(player.stamina(), stats.stamina_max);
        assert_eq!(player.focus(), 0);
        assert!(player.discard().is_empty());
        assert!(player.used_interventions().is_empty());
    }
    assert_eq!(state.round(), 1);
    assert_eq!(state.outcome(), None);
}

#[test]
fn r_poj_03_decks_are_shuffled_with_the_duel_seed() {
    let first = start(&setup(1, &all_ids(), &all_ids()));
    let again = start(&setup(1, &all_ids(), &all_ids()));
    let other_seed = start(&setup(2, &all_ids(), &all_ids()));

    assert_eq!(first, again);
    assert_ne!(draw_order(&first, Side::A), all_ids());
    assert_ne!(
        draw_order(&first, Side::A),
        draw_order(&other_seed, Side::A)
    );
    // Każdy gracz ma własny strumień: ta sama talia, inna kolejność.
    assert_ne!(draw_order(&first, Side::A), draw_order(&first, Side::B));
    assert_eq!(sorted(draw_order(&first, Side::A)), all_ids());
}

#[test]
fn r_poj_04_players_draw_hand_size_cards() {
    let state = start(&setup(3, &all_ids(), &all_ids()));
    for side in Side::BOTH {
        assert_eq!(state.player(side).hand().len(), 5);
        assert_eq!(state.player(side).deck().len(), 15);
    }
}

#[test]
fn r_poj_04_smaller_deck_without_filling_gives_smaller_hand() {
    let mut setup = setup(3, &[1, 2, 3], &[]);
    setup.rules.deck.fill_missing_with_random = false;
    let state = start(&setup);
    assert_eq!(sorted(state.player(Side::A).hand().to_vec()), [1, 2, 3]);
    assert!(state.player(Side::A).deck().is_empty());
    assert!(state.player(Side::B).hand().is_empty());
}

#[test]
fn r_poj_05_initiative_depends_only_on_the_seed() {
    let sides: Vec<Side> = (0..32)
        .map(|seed| start(&setup(seed, &all_ids(), &all_ids())).initiative())
        .collect();
    assert!(sides.contains(&Side::A));
    assert!(sides.contains(&Side::B));

    let other_decks = start(&setup(7, &[1, 2], &[3, 4]));
    assert_eq!(other_decks.initiative(), sides[7]);
    assert_eq!(other_decks.initiative_order(), [sides[7], sides[7].other()]);
}

#[test]
fn r_poj_06_default_settings_use_the_higher_stats() {
    use Kind::{Mag, Phys};
    let tie = stats(5, 3, 5, 3, 40, 40);
    let mut setup = setup(4, &all_ids(), &all_ids());
    let state = start(&setup);
    // A: AF 6 > AM 3, OM 2 > OF 1. B: AM 5 > AF 4, OF 2 > OM 1.
    assert_eq!(state.player(Side::A).attacks(), &[Phys; 3]);
    assert_eq!(state.player(Side::A).defenses(), &[Mag; 3]);
    assert_eq!(state.player(Side::B).attacks(), &[Mag; 3]);
    assert_eq!(state.player(Side::B).defenses(), &[Phys; 3]);

    // Remis statystyk: F (Z-DEF).
    setup.players[0].avatar.stats = tie;
    let state = start(&setup);
    assert_eq!(state.player(Side::A).attacks(), &[Phys; 3]);
    assert_eq!(state.player(Side::A).defenses(), &[Phys; 3]);
}

#[test]
fn r_tal_01_deck_above_the_size_limit_is_rejected() {
    let cards = cards();
    let too_many: Vec<CardId> = (1..=20).chain([1]).collect();
    assert_eq!(
        new_duel(&setup(1, &too_many, &all_ids()), &content(&cards)),
        Err(SetupError::DeckTooLarge(Side::A))
    );
}

#[test]
fn r_tal_02_more_copies_than_allowed_are_rejected() {
    let cards = cards();
    let content = content(&cards);
    assert!(new_duel(&setup(1, &all_ids(), &[7, 7, 7, 7]), &content).is_ok());
    assert_eq!(
        new_duel(&setup(1, &all_ids(), &[7, 7, 7, 7, 7]), &content),
        Err(SetupError::TooManyCopies(Side::B, 7))
    );
}

#[test]
fn r_tal_05_missing_cards_are_filled_from_the_card_pool() {
    let cards = cards();
    let content = content(&cards);
    let state = new_duel(&setup(5, &[1, 2, 3], &[]), &content).expect("poprawne ustawienia");
    let a = draw_order(&state, Side::A);
    assert_eq!(a.len(), 20);
    assert!([1, 2, 3].iter().all(|id| a.contains(id)));
    assert!(a.iter().all(|&id| content.get(id).is_some()));
    assert_eq!(draw_order(&state, Side::B).len(), 20);

    let mut no_fill = setup(5, &[1, 2, 3], &[]);
    no_fill.rules.deck.fill_missing_with_random = false;
    let state = new_duel(&no_fill, &content).expect("poprawne ustawienia");
    assert_eq!(draw_order(&state, Side::A).len(), 3);

    let empty = Content::new(TEST_CONTENT_VERSION, &[]).expect("pusta treść jest poprawna");
    assert_eq!(
        new_duel(&setup(5, &[], &[]), &empty),
        Err(SetupError::EmptyCardPool)
    );
}

#[test]
fn invalid_setup_is_rejected() {
    let cards = cards();
    let content = content(&cards);
    let check = |change: fn(&mut DuelSetup)| {
        let mut setup = setup(1, &all_ids(), &all_ids());
        change(&mut setup);
        new_duel(&setup, &content).err()
    };

    assert_eq!(
        check(|s| s.players[0].avatar.stats.hp_max = 0),
        Some(SetupError::InvalidStats(Side::A))
    );
    assert_eq!(
        check(|s| s.players[1].avatar.stats.phys_atk = -1),
        Some(SetupError::InvalidStats(Side::B))
    );
    assert_eq!(
        check(|s| s.players[1].avatar.schools = Schools::NONE.with(School::Tech, 5)),
        Some(SetupError::InvalidSchools(Side::B))
    );
    assert_eq!(
        check(|s| s.players[0].deck[0] = 99),
        Some(SetupError::UnknownCard(Side::A, 99))
    );
    assert_eq!(
        check(|s| s.rules.duel.rounds = 0),
        Some(SetupError::Rules(RulesError {
            key: "duel.rounds",
            min: 1,
            max: 9
        }))
    );
}
