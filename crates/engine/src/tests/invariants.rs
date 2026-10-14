//! Niezmienniki silnika na losowych danych (`docs/14-testy-i-jakosc.md`, rozdz. 2.1). Numery w
//! komentarzach to punkty tej listy; punkt 7 (widoki gracza) dojdzie razem z `view` w fazie F5.

use arrayvec::ArrayVec;
use proptest::prelude::*;

use crate::card::{CardId, Content};
use crate::end::{Outcome, Termination, end_by};
use crate::event::EventBuf;
use crate::moves::{CardAction, Move, default_move, validate_move};
use crate::round::{DuelError, resolve_round};
use crate::rules::{MAX_DECK_SIZE, MAX_ROUNDS, RulesConfig};
use crate::setup::{AvatarSnapshot, DuelSetup, PlayerSetup, new_duel};
use crate::state::DuelState;
use crate::testkit::{Scenario, classic_rules, content, sample_cards};
use crate::types::{Kind, SLOTS, Schools, Side, Stats};

/// Wybór gracza na jedną rundę: bity ustawień i selektor akcji karty.
#[derive(Debug, Clone, Copy)]
struct Choice {
    settings: u8,
    card: u8,
}

/// Poprawny ruch z wyboru: ustawienia z bitów i akcja karty z selektora. Gdy ruch jest za drogi,
/// zmiany są cofane od ostatniego slotu, a w ostateczności znika akcja karty.
fn choose(state: &DuelState, content: &Content<'_>, side: Side, choice: Choice) -> Move {
    let keep = default_move(state, side);
    let options = 1 + 2 * state.player(side).hand().len() as u8;
    let card = match choice.card % options {
        0 => CardAction::None,
        n if n % 2 == 1 => CardAction::Discard((n - 1) / 2),
        n => CardAction::Cast((n - 2) / 2),
    };
    let kind = |bit: usize| {
        if (choice.settings >> bit) & 1 == 0 {
            Kind::Phys
        } else {
            Kind::Mag
        }
    };
    let mut mv = Move { card, ..keep };
    for slot in 0..SLOTS {
        mv.attacks[slot] = kind(slot);
        mv.defenses[slot] = kind(SLOTS + slot);
    }
    for slot in (0..2 * SLOTS).rev() {
        if validate_move(state, content, side, &mv).is_ok() {
            return mv;
        }
        if slot < SLOTS {
            mv.attacks[slot] = keep.attacks[slot];
        } else {
            mv.defenses[slot - SLOTS] = keep.defenses[slot - SLOTS];
        }
    }
    if validate_move(state, content, side, &mv).is_ok() {
        mv
    } else {
        keep
    }
}

fn mirror(outcome: Option<Outcome>) -> Option<Outcome> {
    outcome.map(|outcome| match outcome {
        Outcome::Win { winner, reason } => Outcome::Win {
            winner: winner.other(),
            reason,
        },
        draw @ Outcome::Draw { .. } => draw,
    })
}

/// Karty gracza w ręce, talii i na stosie odrzuconych.
fn card_count(state: &DuelState, side: Side) -> usize {
    let player = state.player(side);
    player.hand().len() + player.deck().len() + player.discard().len()
}

fn arb_stats() -> impl Strategy<Value = Stats> {
    (0..=15, 0..=10, 0..=15, 0..=10, 1..=150, 1..=150).prop_map(
        |(phys_atk, phys_def, mag_atk, mag_def, hp_max, stamina_max)| Stats {
            phys_atk,
            phys_def,
            mag_atk,
            mag_def,
            hp_max,
            stamina_max,
        },
    )
}

fn arb_schools() -> impl Strategy<Value = Schools> {
    prop::array::uniform6(0u8..=3).prop_map(Schools)
}

/// Karty z `sample_cards` (identyfikatory 1..=13).
fn arb_cards(max: usize) -> impl Strategy<Value = Vec<CardId>> {
    prop::collection::vec(1u16..=13, 0..=max)
}

/// Talia z co najwyżej 4 kopiami karty (R-TAL-02).
fn capped_deck(cards: &[CardId]) -> ArrayVec<CardId, MAX_DECK_SIZE> {
    let mut deck = ArrayVec::new();
    for &card in cards {
        if deck.iter().filter(|&&other| other == card).count() < 4 {
            deck.push(card);
        }
    }
    deck
}

fn arb_rules() -> impl Strategy<Value = RulesConfig> {
    (
        1..=6u8,
        0..=6,
        prop::option::of(0..=12),
        0..=2,
        1..=5u8,
        0..=3,
        any::<bool>(),
    )
        .prop_map(
            |(rounds, income, cap, change_cost, hand_size, block_min_stamina, fill)| {
                let mut rules = classic_rules();
                rules.duel.rounds = rounds;
                rules.duel.focus_per_round = income;
                rules.duel.focus_cap = cap;
                rules.duel.change_cost = change_cost;
                rules.duel.hand_size = hand_size;
                rules.duel.block_min_stamina = block_min_stamina;
                rules.deck.fill_missing_with_random = fill;
                rules
            },
        )
}

fn arb_player() -> impl Strategy<Value = PlayerSetup> {
    (arb_stats(), arb_schools(), arb_cards(20)).prop_map(|(stats, schools, cards)| PlayerSetup {
        avatar: AvatarSnapshot { stats, schools },
        deck: capped_deck(&cards),
    })
}

fn arb_setup() -> impl Strategy<Value = DuelSetup> {
    (arb_rules(), any::<[u8; 32]>(), arb_player(), arb_player()).prop_map(|(rules, seed, a, b)| {
        DuelSetup {
            rules,
            seed,
            players: [a, b],
        }
    })
}

fn arb_choice() -> impl Strategy<Value = Choice> {
    (any::<u8>(), any::<u8>()).prop_map(|(settings, card)| Choice { settings, card })
}

fn arb_choices() -> impl Strategy<Value = Vec<[Choice; 2]>> {
    prop::collection::vec([arb_choice(), arb_choice()], usize::from(MAX_ROUNDS))
}

proptest! {
    /// Punkty 1–4, 8 i 9: zasoby, karty, liczba rund i koszt K na losowych pojedynkach.
    #[test]
    fn resources_cards_and_costs_stay_consistent(setup in arb_setup(), choices in arb_choices()) {
        let cards = sample_cards();
        let content = content(&cards);
        let mut state = new_duel(&setup, &content).expect("poprawne wejście");
        let cards_owned = Side::BOTH.map(|side| card_count(&state, side));
        let mut events = EventBuf::new();
        for round in choices {
            if state.is_finished() {
                break;
            }
            let moves = Side::BOTH.map(|side| choose(&state, &content, side, round[side.index()]));
            let costs = Side::BOTH.map(|side| validate_move(&state, &content, side, &moves[side.index()]));
            let focus_before = Side::BOTH.map(|side| state.player(side).focus());
            let summary = resolve_round(&mut state, &content, &moves, &mut events);
            // 8: poprawny ruch nigdy nie daje błędu rozstrzygania.
            prop_assert!(summary.is_ok(), "{summary:?}");
            let summary = summary.expect("sprawdzone wyżej");
            for side in Side::BOTH {
                let player = state.player(side);
                let cost = costs[side.index()].expect("ruch z `choose` jest poprawny");
                // 9: K spada dokładnie o zmiany razy koszt zmiany plus koszt K czaru.
                prop_assert_eq!(focus_before[side.index()] - summary.totals[side.index()].focus, cost.focus);
                // 1: W i K nie są ujemne.
                prop_assert!(player.stamina() >= 0 && player.focus() >= 0);
                // 2: statystyki efektywne w zakresie (Z-ENG-2).
                let stats = player.stats();
                prop_assert!([stats.phys_atk, stats.phys_def, stats.mag_atk, stats.mag_def].iter().all(|v| *v >= 0));
                prop_assert!(stats.hp_max >= 1 && stats.stamina_max >= 1);
                // 3: ręka nie rośnie ponad limit, a karty nie znikają ani nie przybywają.
                prop_assert!(player.hand().len() <= usize::from(state.rules().duel.hand_size));
                prop_assert_eq!(card_count(&state, side), cards_owned[side.index()]);
            }
            // 4: liczba rund nie przekracza limitu.
            prop_assert!(summary.round <= state.rules().duel.rounds);
            prop_assert!(state.round() <= state.rules().duel.rounds);
        }
    }

    /// Punkt 4: po zakończeniu pojedynku żadna operacja nie zmienia stanu.
    #[test]
    fn finished_duel_never_changes(setup in arb_setup(), choices in arb_choices(), loser in any::<bool>()) {
        let cards = sample_cards();
        let content = content(&cards);
        let mut state = new_duel(&setup, &content).expect("poprawne wejście");
        let mut events = EventBuf::new();
        for round in &choices {
            if state.is_finished() {
                break;
            }
            let moves = Side::BOTH.map(|side| choose(&state, &content, side, round[side.index()]));
            resolve_round(&mut state, &content, &moves, &mut events).expect("poprawne ruchy");
        }
        let loser = if loser { Side::A } else { Side::B };
        if !state.is_finished() {
            end_by(&mut state, loser, Termination::Forfeit, &mut events).expect("pojedynek trwa");
        }
        let finished = state.clone();
        events.clear();
        let moves = Side::BOTH.map(|side| default_move(&state, side));
        prop_assert_eq!(resolve_round(&mut state, &content, &moves, &mut events), Err(DuelError::Finished));
        prop_assert_eq!(end_by(&mut state, loser, Termination::Timeout, &mut events), Err(DuelError::Finished));
        prop_assert_eq!(&state, &finished);
        prop_assert!(events.is_empty());
    }

    /// Punkt 5: te same wejścia dają ten sam stan i te same zdarzenia.
    #[test]
    fn same_input_gives_the_same_duel(setup in arb_setup(), choices in arb_choices()) {
        let cards = sample_cards();
        let content = content(&cards);
        let run = || {
            let mut state = new_duel(&setup, &content).expect("poprawne wejście");
            let mut events = EventBuf::new();
            let mut states = vec![state.clone()];
            for round in &choices {
                if state.is_finished() {
                    break;
                }
                let moves = Side::BOTH.map(|side| choose(&state, &content, side, round[side.index()]));
                resolve_round(&mut state, &content, &moves, &mut events).expect("poprawne ruchy");
                states.push(state.clone());
            }
            (states, events)
        };
        prop_assert_eq!(run(), run());
    }

    /// Punkt 6: zamiana graczy miejscami (razem z inicjatywą) daje lustrzany przebieg.
    #[test]
    fn mirrored_duel_gives_a_mirrored_result(
        stats in [arb_stats(), arb_stats()],
        schools in [arb_schools(), arb_schools()],
        hands in [arb_cards(5), arb_cards(5)],
        decks in [arb_cards(10), arb_cards(10)],
        initiative_a in any::<bool>(),
        choices in arb_choices(),
    ) {
        let cards = sample_cards();
        let content = content(&cards);
        let initiative = if initiative_a { Side::A } else { Side::B };
        let build = |first: usize, initiative: Side| {
            let second = 1 - first;
            Scenario::new(stats[first], stats[second])
                .schools(Side::A, schools[first])
                .schools(Side::B, schools[second])
                .hand(Side::A, &hands[first])
                .hand(Side::B, &hands[second])
                .deck(Side::A, &decks[first])
                .deck(Side::B, &decks[second])
                .initiative(initiative)
                .build()
        };
        let mut duel = build(0, initiative);
        let mut mirrored = build(1, initiative.other());
        let mut events = EventBuf::new();
        for round in choices {
            if duel.is_finished() {
                break;
            }
            let moves = Side::BOTH.map(|side| choose(&duel, &content, side, round[side.index()]));
            let mirrored_moves = [
                choose(&mirrored, &content, Side::A, round[1]),
                choose(&mirrored, &content, Side::B, round[0]),
            ];
            prop_assert_eq!(moves, [mirrored_moves[1], mirrored_moves[0]]);
            resolve_round(&mut duel, &content, &moves, &mut events).expect("poprawne ruchy");
            resolve_round(&mut mirrored, &content, &mirrored_moves, &mut events).expect("poprawne ruchy");
            prop_assert_eq!(duel.player(Side::A), mirrored.player(Side::B));
            prop_assert_eq!(duel.player(Side::B), mirrored.player(Side::A));
            prop_assert_eq!(mirror(duel.outcome()), mirrored.outcome());
        }
    }

    /// Punkt 8 od drugiej strony: ruch odrzucony przez `validate_move` daje błąd rozstrzygania
    /// i nie zmienia stanu.
    #[test]
    fn invalid_move_never_changes_the_state(setup in arb_setup(), settings in any::<u8>(), card in 0u8..12) {
        let cards = sample_cards();
        let content = content(&cards);
        let mut state = new_duel(&setup, &content).expect("poprawne wejście");
        let kind = |bit: u8| if (settings >> bit) & 1 == 0 { Kind::Phys } else { Kind::Mag };
        let action = match card % 3 {
            0 => CardAction::Discard(card / 3),
            1 => CardAction::Cast(card / 3),
            _ => CardAction::None,
        };
        let mv = Move { attacks: [kind(0), kind(1), kind(2)], defenses: [kind(3), kind(4), kind(5)], card: action };
        let other = default_move(&state, Side::B);
        let before = state.clone();
        let mut events = EventBuf::new();
        let result = resolve_round(&mut state, &content, &[mv, other], &mut events);
        match validate_move(&before, &content, Side::A, &mv) {
            Ok(_) => prop_assert!(result.is_ok()),
            Err(error) => {
                prop_assert_eq!(result, Err(DuelError::InvalidMove { side: Side::A, error }));
                prop_assert_eq!(&state, &before);
                prop_assert!(events.is_empty());
            }
        }
    }
}
