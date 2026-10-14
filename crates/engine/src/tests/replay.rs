//! Powtórki (`docs/06-silnik-gry.md`, rozdz. 9).

use crate::end::Termination;
use crate::event::EventBuf;
use crate::moves::default_move;
use crate::replay::{ENGINE_VERSION, ReplayError, ReplayRecord, Version, replay};
use crate::round::DuelError;
use crate::testkit::{content, play_random_duel, sample_cards, seeded_rng};
use crate::types::Side;

fn recorded_duel(seed: u64) -> (crate::state::DuelState, ReplayRecord, EventBuf) {
    let cards = sample_cards();
    let content = content(&cards);
    let mut events = EventBuf::new();
    let (state, record) = play_random_duel(&mut seeded_rng(seed), &content, &mut events, |_| {});
    (state, record, events)
}

#[test]
fn replay_reproduces_the_duel_and_its_events() {
    let cards = sample_cards();
    let content = content(&cards);
    let mut early_ends = 0;
    for seed in 0..300 {
        let (state, record, events) = recorded_duel(seed);
        early_ends += usize::from(record.end.is_some());
        let mut replayed_events = EventBuf::new();
        let replayed = replay(&record, &content, &mut replayed_events).expect("poprawny zapis");
        assert_eq!(replayed, state, "ziarno {seed}");
        assert_eq!(replayed_events, events, "ziarno {seed}");
    }
    // Losowe pojedynki obejmują też zakończenia spoza rozgrywki.
    assert!(early_ends > 0);
}

#[test]
fn replay_checks_engine_and_content_versions() {
    let cards = sample_cards();
    let content = content(&cards);
    let (_, record, _) = recorded_duel(1);

    let mut newer = record.clone();
    newer.engine_version = Version {
        major: ENGINE_VERSION.major + 1,
        ..ENGINE_VERSION
    };
    assert_eq!(
        replay(&newer, &content, &mut EventBuf::new()),
        Err(ReplayError::EngineVersion(newer.engine_version))
    );

    let mut other_content = record;
    other_content.content_version = [1; 32];
    assert_eq!(
        replay(&other_content, &content, &mut EventBuf::new()),
        Err(ReplayError::ContentVersion)
    );
}

#[test]
fn version_compatibility_follows_cargo_rules() {
    let v = |major, minor, patch| Version {
        major,
        minor,
        patch,
    };
    assert!(v(0, 1, 0).can_replay(v(0, 1, 7)));
    assert!(!v(0, 1, 0).can_replay(v(0, 2, 0)));
    assert!(v(1, 0, 0).can_replay(v(1, 4, 2)));
    assert!(!v(2, 0, 0).can_replay(v(1, 9, 9)));
}

#[test]
fn replay_rejects_actions_after_the_end_of_the_duel() {
    let cards = sample_cards();
    let content = content(&cards);
    let (state, record, _) = (0..)
        .map(recorded_duel)
        .find(|(_, record, _)| record.end.is_none())
        .expect("pojedynek");
    let rounds = record.moves.len() as u8;

    let mut extra_round = record.clone();
    extra_round
        .moves
        .push(Side::BOTH.map(|side| default_move(&state, side)));
    assert_eq!(
        replay(&extra_round, &content, &mut EventBuf::new()),
        Err(ReplayError::Round {
            round: rounds + 1,
            error: DuelError::Finished
        })
    );

    let mut late_end = record;
    late_end.end = Some((Side::A, Termination::Forfeit));
    assert_eq!(
        replay(&late_end, &content, &mut EventBuf::new()),
        Err(ReplayError::End(DuelError::Finished))
    );
}
