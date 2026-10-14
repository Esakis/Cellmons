//! Przykłady z dokumentacji: runda z `docs/02-zasady-gry.md` (rozdz. 14) i rundy 1–2 testu
//! referencyjnego TR-01, czyli walki #26519 z archiwum (`docs/14-testy-i-jakosc.md`, rozdz. 3).

use super::{NO_CARD, attack_log, play};
use crate::moves::CardAction::Cast;
use crate::moves::Move;
use crate::round::PlayerTotals;
use crate::state::DuelState;
use crate::testkit::{Scenario, content, mv, noop_card, stats};
use crate::types::{Kind, Side};

/// Postacie z rozdz. 14. Czar „+2 do AM” postaci B dostanie efekt w fazie F2, więc tu jest kartą
/// bez efektu (K 1, W 4), a B ma od razu AM 7 zamiast 5.
fn chapter_14() -> DuelState {
    Scenario::new(stats(6, 1, 3, 2, 60, 50), stats(4, 2, 7, 1, 55, 45))
        .initiative(Side::A)
        .hand(Side::B, &[1])
        .deck(Side::B, &[2])
        .build()
}

fn chapter_14_moves() -> [Move; 2] {
    [mv("FMF", "MFM", NO_CARD), mv("MMF", "FMF", Cast(0))]
}

#[test]
fn rules_chapter_14_example_round() {
    let cards = [noop_card(1, 1, 4), noop_card(2, 1, 4)];
    let content = content(&cards);
    let mut state = chapter_14();
    let (summary, events) = play(&mut state, &content, chapter_14_moves());

    let (a, b) = (Side::A, Side::B);
    assert_eq!(
        attack_log(&events),
        [
            (a, 0, None),
            (b, 0, None),
            (a, 1, None),
            (b, 1, Some(1)),
            (a, 2, None),
            (b, 2, Some(2))
        ]
    );
    assert_eq!(
        summary.totals,
        [
            PlayerTotals {
                hp: 48,
                stamina: 50,
                focus: 2
            },
            PlayerTotals {
                hp: 55,
                stamina: 30,
                focus: 1
            },
        ]
    );
    // B dobiera kartę w miejsce rzuconej.
    assert_eq!(state.player(Side::B).hand(), [2]);
}

#[test]
fn r_atk_03_chapter_14_variant_without_defender_stamina() {
    let cards = [noop_card(1, 1, 4), noop_card(2, 1, 4)];
    let content = content(&cards);
    let mut state = chapter_14();
    state.player_mut(Side::A).stamina = 0;
    let (_, events) = play(&mut state, &content, chapter_14_moves());
    // B1 trafia mimo obrony M, a B2 dostaje już mnożnik ×2.
    let log = attack_log(&events);
    assert_eq!(log[1], (Side::B, 0, Some(1)));
    assert_eq!(log[3], (Side::B, 1, Some(2)));
}

#[test]
fn tr_01_rounds_1_and_2() {
    // Karty testowe „bez efektu” (K 2, W 8). Runda 3 wymaga leczenia, więc dojdzie w kroku F2.5.
    let cards = [noop_card(1, 2, 8)];
    let content = content(&cards);
    // Inicjatywa nie wpływa na wyniki rund 1–3 tej walki, więc test sprawdza obie.
    for initiative in Side::BOTH {
        let mut state = Scenario::new(stats(4, 2, 9, 3, 75, 74), stats(8, 3, 7, 4, 66, 101))
            .initiative(initiative)
            .hand(Side::B, &[1, 1])
            .build();
        assert_eq!(state.player(Side::A).attacks(), &[Kind::Mag; 3]);
        assert_eq!(state.player(Side::A).defenses(), &[Kind::Mag; 3]);
        assert_eq!(state.player(Side::B).attacks(), &[Kind::Phys; 3]);
        assert_eq!(state.player(Side::B).defenses(), &[Kind::Mag; 3]);

        let rounds = [
            (
                [mv("MFF", "MMF", NO_CARD), mv("FFF", "MMM", Cast(0))],
                [(55, 66, 1), (60, 77, 2)],
            ),
            (
                [mv("FMF", "MFF", NO_CARD), mv("FFF", "MFF", Cast(0))],
                [(49, 53, 2), (45, 61, 2)],
            ),
        ];
        for (moves, expected) in rounds {
            let (summary, _) = play(&mut state, &content, moves);
            let totals = summary.totals.map(|t| (t.hp, t.stamina, t.focus));
            assert_eq!(
                totals, expected,
                "runda {}, inicjatywa {initiative:?}",
                summary.round
            );
        }
    }
}
