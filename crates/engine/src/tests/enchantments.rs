//! Umagicznienia (R-CZA-02, R-RUN-11, R-RUN-13, R-POJ-07, Z-ENG-6).

use super::{NO_CARD, idle, play};
use crate::card::CardDef;
use crate::effect::{Duration, Effect, Resource, Target, Trigger};
use crate::event::{Event, EventBuf};
use crate::moves::CardAction::Cast;
use crate::moves::{Move, validate_move};
use crate::setup::{DuelSetup, SetupError, new_duel};
use crate::testkit::{
    Scenario, classic_rules, content, enchantment, mv, noop_card, player, spell, stats,
};
use crate::types::{School, Schools, Side, Stats};

// Ustawienia domyślne: A ataki FFF, obrony MMM; B ataki MMM, obrony FFF (wszystko blokowane).
const A: Stats = stats(6, 1, 3, 2, 100, 50);
const B: Stats = stats(4, 2, 7, 1, 100, 45);

fn a_casts() -> [Move; 2] {
    [mv("FFF", "MMM", Cast(0)), mv("MMM", "FFF", NO_CARD)]
}

fn b_casts() -> [Move; 2] {
    [mv("FFF", "MMM", NO_CARD), mv("MMM", "FFF", Cast(0))]
}

fn while_active(stat: crate::types::Stat, amount: i16) -> Effect {
    Effect::StatMod {
        target: Target::Caster,
        stat,
        amount,
        duration: Duration::WhileActive,
    }
}

fn sources(events: &EventBuf, wanted: fn(&Event) -> Option<u16>) -> Vec<u16> {
    events.as_slice().iter().filter_map(wanted).collect()
}

#[test]
fn r_cza_02_enchantment_acts_in_the_round_it_is_cast_and_every_round_after() {
    let heal = Effect::Heal {
        target: Target::Caster,
        amount: 5,
    };
    let cards = [enchantment(1, 1, 0, Trigger::OnRound, &[heal])];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    state.player_mut(Side::A).hp = 85;
    let (summary, _) = play(&mut state, &content, a_casts());
    assert_eq!(summary.totals[0].hp, 90);
    assert_eq!(state.enchantments().collect::<Vec<_>>(), [(Side::A, 1)]);
    for expected in [95, 100] {
        let moves = idle(&state);
        let (summary, _) = play(&mut state, &content, moves);
        assert_eq!(summary.totals[0].hp, expected);
    }
}

#[test]
fn r_cza_02_continuous_change_lasts_until_the_enchantment_is_dispelled() {
    let cards = [
        enchantment(
            1,
            1,
            0,
            Trigger::OnRound,
            &[while_active(crate::types::Stat::PhysAtk, 2)],
        ),
        spell(
            2,
            1,
            0,
            &[Effect::Dispel {
                target: Target::Opponent,
            }],
        ),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    play(&mut state, &content, a_casts());
    assert_eq!(state.stats(Side::A).phys_atk, 8);

    let (_, events) = play(&mut state, &content, b_casts());
    assert!(events.as_slice().contains(&Event::EnchantmentRemoved {
        side: Side::A,
        card: 1,
        source: 2
    }));
    assert_eq!(state.stats(Side::A).phys_atk, 6);
    assert_eq!(state.enchantments().count(), 0);
}

#[test]
fn r_cza_02_continuous_school_bonus_counts_when_checking_moves() {
    let bonus = Effect::SchoolMod {
        target: Target::Caster,
        school: School::Combat,
        amount: 1,
        duration: Duration::WhileActive,
    };
    let cards = [
        enchantment(1, 1, 0, Trigger::OnRound, &[bonus]),
        CardDef {
            requires: Schools::NONE.with(School::Combat, 1),
            ..noop_card(2, 1, 0)
        },
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[2, 1]).build();
    let cast_first = mv("FFF", "MMM", Cast(0));
    assert!(validate_move(&state, &content, Side::A, &cast_first).is_err());
    play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(1)), mv("MMM", "FFF", NO_CARD)],
    );
    assert!(validate_move(&state, &content, Side::A, &cast_first).is_ok());
}

#[test]
fn r_run_11_enchantments_act_before_new_spells() {
    let drain = Effect::Focus {
        target: Target::Opponent,
        amount: -2,
    };
    let cards = [
        enchantment(1, 1, 0, Trigger::OnRound, &[drain]),
        noop_card(2, 3, 0),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .initiative(Side::B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    play(&mut state, &content, a_casts());
    // Runda 2: B ma inicjatywę, ale umagicznienie A działa przed jego czarem.
    let (_, events) = play(&mut state, &content, b_casts());
    let position = |wanted: fn(&Event) -> bool| events.as_slice().iter().position(wanted);
    let drained = position(|event| {
        matches!(
            event,
            Event::ResourceChanged {
                side: Side::B,
                resource: Resource::Focus,
                ..
            }
        )
    });
    let cast = position(|event| matches!(event, Event::SpellCast { side: Side::B, .. }));
    assert!(drained.is_some() && drained < cast, "{drained:?} {cast:?}");
}

#[test]
fn triggers_fire_on_hits_blocks_and_damage_without_chains() {
    let heal = Effect::Heal {
        target: Target::Caster,
        amount: 2,
    };
    let rest = Effect::Stamina {
        target: Target::Caster,
        amount: 3,
    };
    let sting = Effect::Damage {
        target: Target::Opponent,
        amount: 1,
    };
    let cards = [
        enchantment(1, 1, 0, Trigger::OnHit, &[heal]),
        enchantment(2, 1, 0, Trigger::OnBlocked, &[rest]),
        enchantment(3, 0, 0, Trigger::OnDamageTaken, &[sting]),
    ];
    let content = content(&cards);

    // A trafia trzy razy (obrony B: M M M) i za każdym razem leczy się o 2.
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    state.player_mut(Side::A).hp = 90;
    let (summary, _) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", Cast(0)), mv("MMM", "MMM", NO_CARD)],
    );
    assert_eq!(summary.totals[0].hp, 96);

    // B blokuje trzy ataki A i za każdy blok odzyskuje 3 W.
    let mut state = Scenario::new(A, B).hand(Side::B, &[2]).build();
    state.player_mut(Side::B).stamina = 30;
    let (summary, _) = play(&mut state, &content, b_casts());
    assert_eq!(summary.totals[1].stamina, 39);

    // Obaj oddają 1 obrażenie za każde otrzymane, ale oddane obrażenia nie wyzwalają kolejnych
    // (Z-ENG-6): trzy trafienia A dają dokładnie trzy obrażenia od czaru.
    let mut state = Scenario::new(A, B)
        .gift(Side::A, &cards[2])
        .gift(Side::B, &cards[2])
        .build();
    let (summary, events) = play(
        &mut state,
        &content,
        [mv("FFF", "MMM", NO_CARD), mv("MMM", "MMM", NO_CARD)],
    );
    let stung = sources(&events, |event| match *event {
        Event::Damaged { source, .. } => Some(source),
        _ => None,
    });
    assert_eq!(stung, [3, 3, 3]);
    assert_eq!(summary.totals.map(|t| t.hp), [97, 100 - 4 - 10 - 22]);
}

#[test]
fn r_run_13_end_of_round_effects_happen_after_the_attacks() {
    let rest = Effect::Stamina {
        target: Target::Caster,
        amount: 10,
    };
    let cards = [enchantment(1, 1, 0, Trigger::OnRoundEnd, &[rest])];
    let content = content(&cards);
    let mut state = Scenario::new(A, B).hand(Side::A, &[1]).build();
    state.player_mut(Side::A).stamina = 20;
    let (summary, events) = play(&mut state, &content, a_casts());
    assert_eq!(summary.totals[0].stamina, 30);
    let last_attack = events
        .as_slice()
        .iter()
        .rposition(|event| matches!(event, Event::AttackBlocked { .. }));
    let rested = events
        .as_slice()
        .iter()
        .position(|event| matches!(event, Event::ResourceChanged { side: Side::A, .. }));
    assert!(last_attack < rested);
}

#[test]
fn enchantments_with_the_same_trigger_act_in_casting_order() {
    let gain = Effect::Focus {
        target: Target::Caster,
        amount: 1,
    };
    let cards = [
        enchantment(1, 1, 0, Trigger::OnRound, &[gain]),
        enchantment(2, 1, 0, Trigger::OnRound, &[gain]),
    ];
    let content = content(&cards);
    let mut state = Scenario::new(A, B)
        .hand(Side::A, &[1])
        .hand(Side::B, &[2])
        .build();
    // B rzuca w rundzie 1, A w rundzie 2; A ma inicjatywę, ale umagicznienie B jest starsze.
    play(&mut state, &content, b_casts());
    play(&mut state, &content, a_casts());
    let moves = idle(&state);
    let (_, events) = play(&mut state, &content, moves);
    let order = sources(&events, |event| match *event {
        Event::ResourceChanged { source, .. } => Some(source),
        _ => None,
    });
    assert_eq!(order, [2, 1]);
}

#[test]
fn r_poj_07_gift_works_from_the_start_like_a_free_enchantment() {
    let cards = [
        enchantment(
            1,
            3,
            30,
            Trigger::OnRound,
            &[while_active(crate::types::Stat::PhysDef, 3)],
        ),
        noop_card(2, 0, 0),
    ];
    let content = content(&cards);
    let mut a = player(A, &[2]);
    a.avatar.gift = Some(1);
    let mut setup = DuelSetup {
        rules: classic_rules(),
        seed: [1; 32],
        players: [a, player(B, &[2])],
    };
    let state = new_duel(&setup, &content).expect("poprawne wejście");
    assert_eq!(state.stats(Side::A).phys_def, 1 + 3);
    assert_eq!(state.enchantments().collect::<Vec<_>>(), [(Side::A, 1)]);
    // Dar nic nie kosztuje.
    assert_eq!(state.player(Side::A).focus(), 4);

    setup.players[1].avatar.gift = Some(2);
    assert_eq!(
        new_duel(&setup, &content).err(),
        Some(SetupError::InvalidGift(Side::B))
    );
}
