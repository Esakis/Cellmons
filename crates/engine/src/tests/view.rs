//! Widok gracza (`docs/06-silnik-gry.md`, rozdz. 8; niezmiennik 7 z `docs/14-testy-i-jakosc.md`,
//! rozdz. 2.1).

use crate::event::EventBuf;
use crate::moves::{CardAction, Move, preview, validate_move};
use crate::round::resolve_round;
use crate::setup::new_duel;
use crate::testkit::{
    Scenario, content, random_move, random_setup, sample_cards, seeded_rng, stats,
};
use crate::types::{Kind, Side, Stats};
use crate::view::view;

const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

#[test]
fn view_hides_the_opponents_hand_deck_order_and_seed() {
    let one = Scenario::new(A, B)
        .hand(Side::A, &[1, 2])
        .hand(Side::B, &[3, 4, 5])
        .deck(Side::B, &[6, 7, 8])
        .seed([1; 32])
        .build();
    let other = Scenario::new(A, B)
        .hand(Side::A, &[1, 2])
        .hand(Side::B, &[9, 10, 11])
        .deck(Side::B, &[8, 7, 6])
        .seed([2; 32])
        .build();
    assert_eq!(view(&one, Side::A), view(&other, Side::A));
    assert_eq!(view(&one, Side::A).opponent.hand_len, 3);
    assert_ne!(view(&one, Side::B), view(&other, Side::B));
}

/// Wszystkie ruchy z daną akcją karty i losowymi ustawieniami.
fn candidate_moves(hand: usize, bits: u32) -> Vec<Move> {
    let kind = |bit: u32| {
        if (bits >> bit) & 1 == 0 {
            Kind::Phys
        } else {
            Kind::Mag
        }
    };
    let settings = Move {
        attacks: [kind(0), kind(1), kind(2)],
        defenses: [kind(3), kind(4), kind(5)],
        card: CardAction::None,
    };
    let mut moves = vec![settings];
    for index in 0..=hand as u8 {
        moves.push(Move {
            card: CardAction::Cast(index),
            ..settings
        });
        moves.push(Move {
            card: CardAction::Discard(index),
            ..settings
        });
    }
    moves
}

#[test]
fn checking_moves_on_the_view_gives_the_same_answers_as_on_the_full_state() {
    let cards = sample_cards();
    let content = content(&cards);
    let mut checked = 0;
    for seed in 0..60 {
        let mut rng = seeded_rng(seed);
        let setup = random_setup(&mut rng, &content);
        let mut state = new_duel(&setup, &content).expect("poprawne wejście");
        let mut events = EventBuf::new();
        while !state.is_finished() {
            for side in Side::BOTH {
                let seen = view(&state, side);
                let hand = state.player(side).hand().len();
                for mv in candidate_moves(hand, rand_bits(seed, state.round())) {
                    assert_eq!(
                        seen.validate_move(&content, &mv),
                        validate_move(&state, &content, side, &mv)
                    );
                    assert_eq!(
                        seen.preview(&content, &mv),
                        preview(&state, &content, side, &mv)
                    );
                    checked += 1;
                }
            }
            let moves = Side::BOTH.map(|side| random_move(&state, &content, side, &mut rng));
            resolve_round(&mut state, &content, &moves, &mut events).expect("poprawne ruchy");
        }
    }
    assert!(checked > 1_000);
}

fn rand_bits(seed: u64, round: u8) -> u32 {
    (seed as u32)
        .wrapping_mul(2_654_435_761)
        .rotate_left(u32::from(round))
}
