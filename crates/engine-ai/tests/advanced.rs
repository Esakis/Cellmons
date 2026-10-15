use engine::testkit::{Scenario, content, random_setup, sample_cards, seeded_rng, stats};
use engine::{EventBuf, Side, new_duel, resolve_round, state_hash, view};
use engine_ai::{Bot, Level};

#[test]
fn advanced_bots_are_legal_and_deterministic_through_whole_duels() {
    let cards = sample_cards();
    let content = content(&cards);
    for seed in 0..16 {
        let mut state = new_duel(&random_setup(&mut seeded_rng(seed), &content), &content).unwrap();
        let mut bots = [Bot::new(Level::L2, seed), Bot::new(Level::L3, seed + 1)];
        let mut twins = bots.clone();
        let mut events = EventBuf::new();
        while !state.is_finished() {
            let before = state_hash(&state);
            let moves = Side::BOTH.map(|side| {
                let seen = view(&state, side);
                let mv = bots[side.index()].choose(&seen, &content);
                assert_eq!(mv, twins[side.index()].choose(&seen, &content));
                assert!(seen.validate_move(&content, &mv).is_ok());
                mv
            });
            assert_eq!(before, state_hash(&state));
            events.clear();
            resolve_round(&mut state, &content, &moves, &mut events).unwrap();
            for side in Side::BOTH {
                let seen = view(&state, side);
                bots[side.index()].observe(&seen);
                twins[side.index()].observe(&seen);
            }
        }
        for bot in &bots {
            assert_eq!(bot.model().observations(), u32::from(state.round()));
        }
    }
}

#[test]
fn conditional_interventions_are_saved_until_the_card_can_act() {
    use engine::{CardAction, CardDef, Condition, Effect, Keywords, Target};
    let card = CardDef {
        condition: Some(Condition::RoundAtLeast(4)),
        keywords: Keywords {
            intervention: true,
            ..Default::default()
        },
        ..engine::testkit::spell(
            1,
            4,
            12,
            &[Effect::ConditionalWin {
                condition: Condition::OpponentExhausted,
            }],
        )
    };
    let cards = [card];
    let content = content(&cards);
    let mut state = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80))
        .stamina(Side::B, 0)
        .hand(Side::A, &[1])
        .build();
    for round in 1..=4 {
        for level in [Level::L1, Level::L2, Level::L3] {
            for seed in 0..4 {
                let choice = Bot::new(level, seed).choose(&view(&state, Side::A), &content);
                if round < 4 {
                    assert!(!matches!(choice.card, CardAction::Cast(_)));
                } else {
                    assert!(matches!(choice.card, CardAction::Cast(0)));
                }
            }
        }
        let moves = Side::BOTH.map(|side| engine::default_move(&state, side));
        resolve_round(&mut state, &content, &moves, &mut EventBuf::new()).unwrap();
    }
    // Pozostałe warunki karty też nie mogą być mylone z warunkami wygranej na koniec rundy.
    let damage = engine::testkit::spell(
        2,
        1,
        0,
        &[Effect::Damage {
            target: Target::Opponent,
            amount: 50,
        }],
    );
    for condition in [Condition::HpPercentAhead, Condition::OpponentExhausted] {
        let cards = [CardDef {
            condition: Some(condition),
            ..damage.clone()
        }];
        let content = engine::testkit::content(&cards);
        let state = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80))
            .hand(Side::A, &[2])
            .build();
        assert!(!matches!(
            Bot::new(Level::L2, 0)
                .choose(&view(&state, Side::A), &content)
                .card,
            CardAction::Cast(_)
        ));
    }
}

#[test]
fn finished_round_is_observed_once_but_forfeit_is_not_a_revealed_move() {
    let content = content(&[]);
    let mut rules = engine::testkit::classic_rules();
    rules.duel.rounds = 1;
    for finish_round in [true, false] {
        let mut state = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80))
            .rules(rules.clone())
            .build();
        let mut bot = Bot::new(Level::L2, 0);
        let mv = bot.choose(&view(&state, Side::A), &content);
        if finish_round {
            let moves = [mv, engine::default_move(&state, Side::B)];
            resolve_round(&mut state, &content, &moves, &mut EventBuf::new()).unwrap();
        } else {
            engine::end_by(
                &mut state,
                Side::B,
                engine::Termination::Forfeit,
                &mut EventBuf::new(),
            )
            .unwrap();
        }
        for _ in 0..3 {
            bot.observe(&view(&state, Side::A));
        }
        assert_eq!(bot.model().observations(), u32::from(finish_round));
        let mut next = Bot::new(Level::L2, 1);
        next.set_model(bot.model().clone());
        assert_eq!(next.model().observations(), u32::from(finish_round));
    }
}

#[test]
fn hidden_cards_deck_order_and_engine_seed_cannot_influence_a_decision() {
    let cards = sample_cards();
    let content = content(&cards);
    let make = |hidden: &[u16], deck: &[u16], seed| {
        Scenario::new(stats(8, 2, 6, 4, 100, 80), stats(7, 3, 6, 4, 100, 80))
            .hand(Side::A, &[1, 2, 3])
            .hand(Side::B, hidden)
            .deck(Side::A, deck)
            .deck(Side::B, deck)
            .seed(seed)
            .build()
    };
    let a = make(&[1, 2, 3], &[4, 5, 6], [1; 32]);
    let b = make(&[4, 5, 6], &[6, 4, 5], [2; 32]);
    let (first, second) = (view(&a, Side::A), view(&b, Side::A));
    assert_eq!(first, second);
    for level in [Level::L0, Level::L1, Level::L2, Level::L3] {
        assert_eq!(
            Bot::new(level, 7).choose(&first, &content),
            Bot::new(level, 7).choose(&second, &content)
        );
    }
}
