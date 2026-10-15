//! Słowa kluczowe i warunki (R-CZA-04…09, R-KON-06, Z-CNT, Z-CND, Z-SPC, Z-IWN).

use super::{NO_CARD, play};
use crate::card::{CardDef, Condition, Coupling, Keywords};
use crate::effect::{Duration, Effect, Target};
use crate::end::{Outcome, Reason};
use crate::event::Event;
use crate::moves::CardAction::Cast;
use crate::moves::{Move, validate_move};
use crate::testkit::{Scenario, content, mv, noop_card, spell, stats};
use crate::types::{School, Schools, Side, Specialization, Stat, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF (wszystko blokowane).
const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

fn both_cast() -> [Move; 2] {
    [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", Cast(0))]
}

fn plus_two_attack() -> Effect {
    Effect::StatMod {
        target: Target::Caster,
        stat: Stat::PhysAtk,
        amount: 2,
        duration: Duration::Duel,
    }
}

fn counter(id: u16) -> CardDef {
    CardDef {
        keywords: Keywords {
            counter: true,
            ..Keywords::default()
        },
        ..noop_card(id, 1, 2)
    }
}

fn has(events: &crate::event::EventBuf, wanted: &Event) -> bool {
    events.as_slice().contains(wanted)
}

#[test]
fn r_cza_04_counter_cancels_the_opponents_spell_this_round() {
    let cards = [
        counter(1),
        spell(
            2,
            1,
            4,
            &[Effect::StatMod {
                target: Target::Caster,
                stat: Stat::MagAtk,
                amount: 2,
                duration: Duration::Duel,
            }],
        ),
    ];
    let content = content(&cards);
    // B ma inicjatywę, ale przeciwzaklęcie A rozpatruje się przed innymi czarami.
    let mut state = Scenario::new(A, B)
        .initiative(Side::B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    let (summary, events) = play(&mut state, &content, both_cast());
    let countered = Event::SpellCountered {
        side: Side::B,
        card: 2,
        focus: 1,
        stamina: 4,
    };
    assert!(has(&events, &countered));
    assert_eq!(state.stats(Side::B).mag_atk, 7);
    // Anulowany czar i tak kosztuje (R-CZA-05, p. 2).
    assert_eq!(summary.totals[1].focus, 3);
    assert_eq!(summary.totals[1].stamina, 41);
}

#[test]
fn r_cza_05_when_both_counter_only_the_initiative_counter_works() {
    let cards = [counter(1), counter(2)];
    let content = content(&cards);
    for first in Side::BOTH {
        let mut state = Scenario::new(A, B)
            .initiative(first)
            .hand(Side::A, &[1])
            .hand(Side::B, &[2])
            .build();
        let (_, events) = play(&mut state, &content, both_cast());
        let card_of = |side: Side| if side == Side::A { 1 } else { 2 };
        let second = first.other();
        assert!(
            events
                .as_slice()
                .iter()
                .any(|event| matches!(event, Event::SpellCast { side, .. } if *side == first))
        );
        let countered = Event::SpellCountered {
            side: second,
            card: card_of(second),
            focus: 1,
            stamina: 2,
        };
        assert!(has(&events, &countered), "{first:?}");
    }
}

#[test]
fn r_cza_06_coupling_costs_more_per_level_and_scales_the_effect() {
    let coupled = CardDef {
        keywords: Keywords {
            coupling: Some(Coupling {
                school: School::Combat,
                level: 2,
            }),
            ..Keywords::default()
        },
        ..spell(
            1,
            0,
            0,
            &[Effect::StatMod {
                target: Target::Caster,
                stat: Stat::PhysAtk,
                amount: 1,
                duration: Duration::Duel,
            }],
        )
    };
    let content = content(std::slice::from_ref(&coupled));
    let cast = mv("FFF", "MMM", Cast(0));

    // Poziom 4 przy sprzężeniu do poziomu 2: +4 K i +20 W, efekt ×4.
    let master = Schools::NONE.with(School::Combat, 4);
    let mut state = Scenario::new(A, B)
        .schools(Side::A, master)
        .hand(Side::A, &[1])
        .build();
    let cost = validate_move(&state, &content, Side::A, &cast).expect("stać go na czar");
    assert_eq!((cost.focus, cost.stamina), (4, 20));
    play(&mut state, &content, [cast, mv("MMM", "FFF", NO_CARD)]);
    assert_eq!(state.stats(Side::A).phys_atk, 6 + 4);

    // Poziom 1 nie sięga sprzężenia: zwykły koszt i zwykły efekt.
    let novice = Schools::NONE.with(School::Combat, 1);
    let mut state = Scenario::new(A, B)
        .schools(Side::A, novice)
        .hand(Side::A, &[1])
        .build();
    let cost = validate_move(&state, &content, Side::A, &cast).expect("czar jest darmowy");
    assert_eq!((cost.focus, cost.stamina), (0, 0));
    play(&mut state, &content, [cast, mv("MMM", "FFF", NO_CARD)]);
    assert_eq!(state.stats(Side::A).phys_atk, 7);
}

#[test]
fn r_cza_07_random_card_acts_as_a_random_card_from_the_pool() {
    let random = CardDef {
        keywords: Keywords {
            random: true,
            ..Keywords::default()
        },
        ..noop_card(1, 1, 0)
    };
    // Jedyna karta, którą można wylosować (karta losowa i przeciwzaklęcie są wykluczone). Postać
    // A nie spełnia jej wymagań, ale przy losowaniu się ich nie sprawdza.
    let drawn = CardDef {
        requires: Schools::NONE.with(School::Necro, 3),
        ..spell(2, 3, 7, &[plus_two_attack()])
    };
    let cards = [random, drawn, counter(3)];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    assert!(has(
        &events,
        &Event::RandomSpell {
            side: Side::A,
            card: 1,
            drawn: 2
        }
    ));
    assert_eq!(state.stats(Side::A).phys_atk, 8);
    // K z karty losowej, W z wylosowanej.
    assert_eq!(summary.totals[0].focus, 4 - 1);
    assert_eq!(summary.totals[0].stamina, 50 - 7);
}

#[test]
fn r_cza_08_unmet_condition_takes_the_costs_but_has_no_effect() {
    let late = CardDef {
        condition: Some(Condition::RoundAtLeast(3)),
        ..spell(1, 1, 4, &[plus_two_attack()])
    };
    let content = content(std::slice::from_ref(&late));
    let mut state = Scenario::new(A, B).hand(Side::A, &[1, 1]).build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    assert!(has(
        &events,
        &Event::SpellFailed {
            side: Side::A,
            card: 1
        }
    ));
    assert_eq!(summary.totals[0].focus, 3);
    assert_eq!(summary.totals[0].stamina, 46);
    assert_eq!(state.stats(Side::A).phys_atk, 6);
}

#[test]
fn r_cza_09_specialization_reduces_costs_by_a_third_rounding_up() {
    let combat = CardDef {
        requires: Schools::NONE.with(School::Combat, 1),
        ..noop_card(1, 4, 16)
    };
    let plain = noop_card(2, 4, 16);
    let coupled = CardDef {
        keywords: Keywords {
            coupling: Some(Coupling {
                school: School::Combat,
                level: 1,
            }),
            ..Keywords::default()
        },
        ..noop_card(3, 0, 0)
    };
    let cards = [combat, plain, coupled];
    let content = content(&cards);
    let cost = |specialization, card| {
        let state = Scenario::new(A, B)
            .schools(Side::A, Schools::NONE.with(School::Combat, 4))
            .specialization(Side::A, specialization)
            .hand(Side::A, &[card])
            .build();
        let mut state = state;
        state.player_mut(Side::A).focus = 50;
        validate_move(&state, &content, Side::A, &mv("FFF", "MMM", Cast(0)))
            .map(|cost| (cost.focus, cost.stamina))
            .expect("stać go na czar")
    };
    // Z-SPC: (2 × koszt + 1) div 3, czyli 4 → 3 i 16 → 11.
    assert_eq!(cost(Specialization::Essence, 2), (4, 11));
    assert_eq!(cost(Specialization::Concentration, 2), (3, 16));
    assert_eq!(cost(Specialization::School(School::Combat), 1), (4, 11));
    assert_eq!(cost(Specialization::School(School::Combat), 2), (4, 16));
    // Sprzężenie liczy się przed redukcją: 4 K i 20 W → 3 K przy „Koncentracji”.
    assert_eq!(cost(Specialization::Concentration, 3), (3, 20));
}

#[test]
fn r_kon_06_conditional_win_is_checked_at_the_end_of_the_round_before_knockout() {
    let win = |id| {
        spell(
            id,
            1,
            0,
            &[Effect::ConditionalWin {
                condition: Condition::OpponentExhausted,
            }],
        )
    };
    let cards = [win(1), win(2)];
    let content = content(&cards);

    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    state.player_mut(Side::B).stamina = 0;
    // A wygrywa warunkowo, choć pojedynek nie skończyłby się nokautem.
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    let reason = Reason::ConditionalWin;
    assert_eq!(
        summary.outcome,
        Some(Outcome::Win {
            winner: Side::A,
            reason
        })
    );

    // Obie strony spełniają warunek: remis (Z-IWN), nawet przy nokaucie A.
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    for side in Side::BOTH {
        state.player_mut(side).stamina = 0;
    }
    state.player_mut(Side::A).hp = 1;
    let (summary, _) = play(&mut state, &content, both_cast());
    assert_eq!(summary.outcome, Some(Outcome::Draw { reason }));

    // Niespełniony warunek nie kończy pojedynku.
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)],
    );
    assert_eq!(summary.outcome, None);
}
