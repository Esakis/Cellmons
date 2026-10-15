//! Pomiar silnika (`docs/06-silnik-gry.md`, rozdz. 10): rozstrzygnięcie jednej rundy i cały
//! pojedynek. Cel kroku F1.7 planu: mediana `resolve_round` ≤ 5 µs natywnie.

use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use engine::{
    AvatarSnapshot, CardAction, CardDef, CardKind, Content, Cost, DuelSetup, Duration, Effect,
    EventBuf, Keywords, Kind, Move, PlayerSetup, RulesConfig, Schools, Stat, Stats, Target,
    new_duel, resolve_round,
};

const RULES: &str = include_str!("../../../content/rules.toml");

fn rules() -> RulesConfig {
    toml::from_str(RULES).expect("content/rules.toml jest poprawny")
}

fn player(stats: Stats) -> PlayerSetup {
    PlayerSetup {
        avatar: AvatarSnapshot {
            stats,
            schools: Schools::NONE,
            specialization: None,
            gift: None,
        },
        deck: (1..=10).chain(1..=10).collect(),
    }
}

/// Postacie z dużą liczbą Ż, żeby pojedynek trwał wszystkie 5 rund.
fn setup() -> DuelSetup {
    DuelSetup {
        rules: rules(),
        seed: [7; 32],
        players: [
            player(Stats {
                phys_atk: 8,
                phys_def: 3,
                mag_atk: 6,
                mag_def: 4,
                hp_max: 400,
                stamina_max: 300,
            }),
            player(Stats {
                phys_atk: 5,
                phys_def: 4,
                mag_atk: 9,
                mag_def: 3,
                hp_max: 400,
                stamina_max: 300,
            }),
        ],
    }
}

/// Ruchy z rzuceniem czaru, odrzuceniem, zmianami ustawień, trafieniami i blokami. Po 1. rundzie
/// ustawienia się nie zmieniają, więc te same ruchy są poprawne w każdej rundzie.
fn moves() -> [Move; 2] {
    use Kind::{Mag, Phys};
    [
        Move {
            attacks: [Phys, Mag, Phys],
            defenses: [Mag, Phys, Mag],
            card: CardAction::Cast(0),
        },
        Move {
            attacks: [Mag, Mag, Phys],
            defenses: [Phys, Mag, Phys],
            card: CardAction::Discard(1),
        },
    ]
}

fn bench(c: &mut Criterion) {
    let cards: Vec<CardDef> = (1..=10)
        .map(|id| CardDef {
            id,
            requires: Schools::NONE,
            cost: Cost {
                focus: 1,
                stamina: 4,
            },
            kind: CardKind::Instant,
            condition: None,
            keywords: Keywords::default(),
            effects: [Effect::StatMod {
                target: Target::Caster,
                stat: Stat::PhysAtk,
                amount: 1,
                duration: Duration::Round,
            }]
            .into_iter()
            .collect(),
        })
        .collect();
    let content = Content::new([0; 32], &cards).expect("poprawna treść");
    let setup = setup();
    let moves = moves();
    let start = new_duel(&setup, &content).expect("poprawne wejście");
    let mut events = EventBuf::new();

    c.bench_function("resolve_round", |b| {
        b.iter_batched_ref(
            || start.clone(),
            |state| {
                events.clear();
                black_box(resolve_round(state, &content, &moves, &mut events))
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("full_duel", |b| {
        b.iter(|| {
            let mut state = new_duel(&setup, &content).expect("poprawne wejście");
            events.clear();
            while !state.is_finished() {
                resolve_round(&mut state, &content, &moves, &mut events).expect("poprawne ruchy");
            }
            black_box(state)
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
