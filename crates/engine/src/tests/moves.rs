//! Walidacja ruchu (R-RUN-06), kody błędów, podgląd kosztów i ruch domyślny (R-RUN-09).

use super::{NO_CARD, play};
use crate::card::{CardDef, Keywords};
use crate::end::{Termination, end_by};
use crate::event::EventBuf;
use crate::moves::CardAction::{Cast, Discard};
use crate::moves::{MoveCost, MoveError, Preview, default_move, preview, validate_move};
use crate::round::{DuelError, resolve_round};
use crate::testkit::{Scenario, content, mv, noop_card, stats};
use crate::types::{School, Schools, Side, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF.
const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

fn intervention(id: u16) -> CardDef {
    CardDef {
        keywords: Keywords { intervention: true },
        ..noop_card(id, 1, 4)
    }
}

#[test]
fn r_run_06_focus_must_cover_changes_and_the_spell() {
    let cards = [noop_card(1, 1, 4)];
    let content = content(&cards);
    let state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let check = |attacks, defenses, card| {
        validate_move(&state, &content, Side::A, &mv(attacks, defenses, card))
    };

    // 3 zmiany i czar za 1 K: dokładnie 4 K.
    assert_eq!(
        check("MMM", "MMM", Cast(0)),
        Ok(MoveCost {
            changes: 3,
            focus: 4,
            stamina: 4
        })
    );
    assert_eq!(check("MMM", "FMM", Cast(0)), Err(MoveError::NotEnoughFocus));
    assert_eq!(
        check("MMM", "FMM", NO_CARD),
        Ok(MoveCost {
            changes: 4,
            focus: 4,
            stamina: 0
        })
    );
    assert_eq!(check("MMM", "FFM", NO_CARD), Err(MoveError::NotEnoughFocus));
}

#[test]
fn r_run_06_stamina_must_cover_the_spell() {
    let cards = [noop_card(1, 1, 50), noop_card(2, 1, 51)];
    let content = content(&cards);
    let state = Scenario::new(A, B).hand(Side::A, &[1, 2]).build();
    let check = |card| validate_move(&state, &content, Side::A, &mv("FFF", "MMM", card));
    assert!(check(Cast(0)).is_ok());
    assert_eq!(check(Cast(1)), Err(MoveError::NotEnoughStamina));
}

#[test]
fn r_run_06_schools_must_meet_the_card_requirements() {
    let card = CardDef {
        requires: Schools::NONE
            .with(School::Combat, 2)
            .with(School::Divine, 1),
        ..noop_card(1, 1, 4)
    };
    let content = content(std::slice::from_ref(&card));
    let check = |schools: Schools| {
        let state = Scenario::new(A, B)
            .schools(Side::A, schools)
            .hand(Side::A, &[1])
            .build();
        validate_move(&state, &content, Side::A, &mv("FFF", "MMM", Cast(0))).map(|_| ())
    };
    assert_eq!(
        check(Schools::NONE.with(School::Combat, 1)),
        Err(MoveError::RequirementsNotMet)
    );
    assert_eq!(
        check(Schools::NONE.with(School::Combat, 4)),
        Err(MoveError::RequirementsNotMet)
    );
    assert_eq!(
        check(
            Schools::NONE
                .with(School::Combat, 2)
                .with(School::Divine, 1)
        ),
        Ok(())
    );
}

#[test]
fn r_cza_03_intervention_can_be_cast_once_per_duel_by_each_player() {
    let cards = [intervention(1), intervention(2)];
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1, 1, 2])
        .hand(Side::B, &[1])
        .build();
    // Runda 1: interwencję 1 rzuca tylko A.
    play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    assert_eq!(state.player(Side::A).used_interventions(), [1]);

    // Ręka A: [1, 2]. Druga kopia karty 1 jest zablokowana, inna interwencja nie (Z-INT).
    let a = |card| validate_move(&state, &content, Side::A, &mv("FFF", "MMM", card));
    assert_eq!(a(Cast(0)), Err(MoveError::InterventionUsed));
    assert!(a(Cast(1)).is_ok());
    // Limit dotyczy gracza: B nadal może rzucić tę samą interwencję.
    let b = validate_move(&state, &content, Side::B, &mv("MMM", "FFF", Cast(0)));
    assert!(b.is_ok());
}

#[test]
fn r_run_06_discarding_is_free() {
    let cards = [noop_card(1, 9, 99)];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let discard = mv("FFF", "MMM", Discard(0));
    assert_eq!(
        validate_move(&state, &content, Side::A, &discard),
        Ok(MoveCost::default())
    );

    let (summary, _) = play(&mut state, &content, [discard, mv("MMM", "FFF", NO_CARD)]);
    assert_eq!(summary.totals[0].focus, 4);
    assert_eq!(summary.totals[0].stamina, 50);
    assert_eq!(state.player(Side::A).discard(), [1]);
}

#[test]
fn r_run_06_invalid_move_is_rejected_and_the_state_stays_unchanged() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    let before = state.clone();
    let mut events = EventBuf::new();
    // Ustawienia domyślne jednej strony to dla drugiej 6 zmian, czyli więcej niż 4 K.
    let a_defaults = mv("FFF", "MMM", NO_CARD);
    let b_defaults = mv("MMM", "FFF", NO_CARD);

    let result = resolve_round(&mut state, &content, &[a_defaults, a_defaults], &mut events);
    assert_eq!(
        result,
        Err(DuelError::InvalidMove {
            side: Side::B,
            error: MoveError::NotEnoughFocus
        })
    );
    let result = resolve_round(&mut state, &content, &[b_defaults, b_defaults], &mut events);
    assert_eq!(
        result,
        Err(DuelError::InvalidMove {
            side: Side::A,
            error: MoveError::NotEnoughFocus
        })
    );
    assert_eq!(state, before);
    assert!(events.is_empty());
}

#[test]
fn r_run_09_default_move_repeats_previous_settings_without_a_card() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    // Przed 1. rundą: ustawienia domyślne (R-POJ-06).
    assert_eq!(default_move(&state, Side::A), mv("FFF", "MMM", NO_CARD));

    let a = mv("FMF", "MFM", NO_CARD);
    let b = default_move(&state, Side::B);
    play(&mut state, &content, [a, b]);
    assert_eq!(default_move(&state, Side::A), a);
    assert_eq!(
        validate_move(&state, &content, Side::A, &a),
        Ok(MoveCost::default())
    );
}

#[test]
fn finished_duel_rejects_moves_with_wrong_phase() {
    let content = content(&[]);
    let mut state = Scenario::new(A, B).build();
    end_by(
        &mut state,
        Side::B,
        Termination::Forfeit,
        &mut EventBuf::new(),
    )
    .expect("pojedynek trwa");
    let check = validate_move(&state, &content, Side::A, &default_move(&state, Side::A));
    assert_eq!(check, Err(MoveError::WrongPhase));
}

#[test]
fn hand_index_outside_the_hand_is_rejected() {
    let cards = [noop_card(1, 1, 4)];
    let content = content(&cards);
    let state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    for card in [Cast(1), Discard(1), Cast(u8::MAX)] {
        let check = validate_move(&state, &content, Side::A, &mv("FFF", "MMM", card));
        assert_eq!(check, Err(MoveError::InvalidHandIndex), "{card:?}");
    }
}

#[test]
fn card_missing_from_the_content_cannot_be_cast() {
    let content = content(&[]);
    let state = Scenario::new(A, B).hand(Side::A, &[99]).build();
    let check = |card| validate_move(&state, &content, Side::A, &mv("FFF", "MMM", card));
    assert_eq!(check(Cast(0)), Err(MoveError::UnknownCard));
    // Odrzucenie nie potrzebuje definicji karty.
    assert!(check(Discard(0)).is_ok());
}

#[test]
fn errors_are_reported_in_a_fixed_order() {
    let card = CardDef {
        requires: Schools::NONE.with(School::Arcane, 1),
        ..noop_card(1, 9, 99)
    };
    let content = content(std::slice::from_ref(&card));
    let state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    // Brak wymagań, za mało K i za mało W naraz: najpierw wymagania, potem K, na końcu W.
    let check = validate_move(&state, &content, Side::A, &mv("FFF", "MMM", Cast(0)));
    assert_eq!(check, Err(MoveError::RequirementsNotMet));

    let state = Scenario::new(A, B)
        .schools(Side::A, Schools::NONE.with(School::Arcane, 1))
        .hand(Side::A, &[1])
        .build();
    let check = validate_move(&state, &content, Side::A, &mv("FFF", "MMM", Cast(0)));
    assert_eq!(check, Err(MoveError::NotEnoughFocus));
}

#[test]
fn preview_shows_costs_of_a_valid_move() {
    let cards = [noop_card(1, 1, 4)];
    let content = content(&cards);
    let state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    // Ataki F M F: AF 6 + AM 3 + AF 6.
    let shown = preview(&state, &content, Side::A, &mv("FMF", "MMM", Cast(0)));
    assert_eq!(
        shown,
        Preview {
            cost: MoveCost {
                changes: 1,
                focus: 2,
                stamina: 4
            },
            focus_after: 2,
            max_attack_stamina: 15,
            validity: Ok(()),
        }
    );
}

#[test]
fn preview_shows_costs_of_an_invalid_move() {
    let cards = [noop_card(1, 1, 4)];
    let content = content(&cards);
    let state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let shown = preview(&state, &content, Side::A, &mv("MMM", "FFF", Cast(0)));
    assert_eq!(
        shown.cost,
        MoveCost {
            changes: 6,
            focus: 7,
            stamina: 4
        }
    );
    assert_eq!(shown.focus_after, -3);
    assert_eq!(shown.max_attack_stamina, 9);
    assert_eq!(shown.validity, Err(MoveError::NotEnoughFocus));
}
