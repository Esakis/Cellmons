//! Test scenariuszowy każdej karty z `content/` (`docs/03-tresc-gry.md`, rozdz. 5.4, p. 4;
//! kryterium kroku F3.3). Gracz A rzuca kartę w 1. rundzie; ustawienia domyślne obu stron
//! blokują wszystkie ataki, więc zmiany Ż i W pochodzą tylko z efektów karty, chyba że scenariusz
//! karty ustawia ataki inaczej.

use engine::testkit::{Scenario, stats};
use engine::{
    CardAction, CardDef, CardId, DuelState, Event, EventBuf, Kind, Move, Outcome, Reason, Resource,
    School, Side, Stat, Stats, default_move, resolve_round,
};
use engine_content::{CARD_KEYS, STARTERS, content};

/// Obie postacie: AF 6, OF 3, AM 4, OM 2, Ż 60, W 60. Ustawienia domyślne to F we wszystkich
/// slotach, więc bloki są pełne.
const BASE: Stats = stats(6, 3, 4, 2, 60, 60);

fn card(key: &str) -> &'static CardDef {
    let id = CARD_KEYS
        .iter()
        .find(|(_, card)| *card == key)
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("brak karty {key}"));
    content().get(id).expect("karta w treści")
}

/// Wynik rundy, w której A rzucił kartę.
struct Run {
    card: CardId,
    before: DuelState,
    state: DuelState,
    events: Vec<Event>,
}

impl Run {
    fn has(&self, wanted: impl Fn(&Event) -> bool) -> bool {
        self.events.iter().any(wanted)
    }

    fn count(&self, wanted: impl Fn(&Event) -> bool) -> usize {
        self.events.iter().filter(|event| wanted(event)).count()
    }

    /// Zmiany statystyki z efektów tej karty, po kolei.
    fn stat_changes(&self, side: Side, stat: Stat) -> Vec<i32> {
        self.events
            .iter()
            .filter_map(|event| match *event {
                Event::StatChanged {
                    side: s,
                    stat: t,
                    old,
                    new,
                    source,
                } if s == side && t == stat && source == self.card => Some(new - old),
                _ => None,
            })
            .collect()
    }

    /// Statystyka efektywna po rundzie minus przed rundą.
    fn stat_delta(&self, side: Side, stat: Stat) -> i32 {
        self.state.stats(side).get(stat) - self.before.stats(side).get(stat)
    }

    fn resource(&self, side: Side, resource: Resource) -> i32 {
        self.events
            .iter()
            .filter_map(|event| match *event {
                Event::ResourceChanged {
                    side: s,
                    resource: r,
                    old,
                    new,
                    source,
                } if s == side && r == resource && source == self.card => Some(new - old),
                _ => None,
            })
            .sum()
    }

    fn healed(&self, side: Side) -> i32 {
        self.events
            .iter()
            .filter_map(|event| match *event {
                Event::Healed {
                    side: s,
                    amount,
                    source,
                } if s == side && source == self.card => Some(amount),
                _ => None,
            })
            .sum()
    }

    fn damaged(&self, side: Side) -> i32 {
        self.events
            .iter()
            .filter_map(|event| match *event {
                Event::Damaged {
                    side: s,
                    amount,
                    source,
                } if s == side && source == self.card => Some(amount),
                _ => None,
            })
            .sum()
    }

    fn missed(&self, attacker: Side) -> Vec<u8> {
        self.events
            .iter()
            .filter_map(|event| match *event {
                Event::AttackMissed {
                    attacker: a, slot, ..
                } if a == attacker => Some(slot),
                _ => None,
            })
            .collect()
    }

    fn hit(&self, attacker: Side) -> Vec<u8> {
        self.events
            .iter()
            .filter_map(|event| match *event {
                Event::AttackHit {
                    attacker: a, slot, ..
                } if a == attacker => Some(slot),
                _ => None,
            })
            .collect()
    }

    fn enchanted(&self) -> bool {
        self.has(|event| matches!(*event, Event::EnchantmentAdded { side: Side::A, card } if card == self.card))
    }
}

/// Scenariusz z kartą w ręce A i wymaganiami szkół spełnionymi przez A; A ma 40 z 60 Ż.
fn scenario(card: &CardDef) -> Scenario {
    Scenario::new(BASE, BASE)
        .schools(Side::A, card.requires)
        .hand(Side::A, &[card.id])
        .hp(Side::A, 40)
}

/// Rozgrywa `skip` rund bez kart, a potem rundę, w której A rzuca kartę z pozycji 0. `b` zmienia
/// ruch B z domyślnego.
fn play(card: &CardDef, scenario: Scenario, skip: u8, b: impl Fn(Move) -> Move) -> Run {
    let content = content();
    let mut state = scenario.build();
    let mut events = EventBuf::new();
    for _ in 0..skip {
        let moves = Side::BOTH.map(|side| default_move(&state, side));
        resolve_round(&mut state, &content, &moves, &mut events).expect("runda bez kart");
    }
    let before = state.clone();
    events.clear();
    let a = Move {
        card: CardAction::Cast(0),
        ..default_move(&state, Side::A)
    };
    let moves = [a, b(default_move(&state, Side::B))];
    resolve_round(&mut state, &content, &moves, &mut events)
        .unwrap_or_else(|error| panic!("karta {}: {error:?}", card.id));
    let cast = events.as_slice().iter().any(|event| {
        matches!(*event, Event::SpellCast { side: Side::A, card: c, .. } if c == card.id)
            || matches!(*event, Event::RandomSpell { side: Side::A, card: c, .. } if c == card.id)
    });
    assert!(
        cast,
        "karta {} nie została rzucona: {:?}",
        card.id,
        events.as_slice()
    );
    Run {
        card: card.id,
        before,
        state,
        events: events.as_slice().to_vec(),
    }
}

fn cast(card: &CardDef) -> Run {
    play(card, scenario(card), 0, |mv| mv)
}

/// B atakuje magicznie we wszystkich slotach (3 K za zmiany), więc trafia w obrony F gracza A.
fn b_attacks_magic(mv: Move) -> Move {
    Move {
        attacks: [Kind::Mag; 3],
        ..mv
    }
}

/// B broni się magicznie, więc fizyczne ataki A trafiają.
fn b_defends_magic(mv: Move) -> Move {
    Move {
        defenses: [Kind::Mag; 3],
        ..mv
    }
}

fn id(key: &str) -> CardId {
    card(key).id
}

/// Sprawdza działanie karty `key`; każda karta z treści musi mieć tu swój przypadek.
#[allow(clippy::too_many_lines)]
fn check(key: &str) {
    let c = card(key);
    let name = key.trim_start_matches("card.");
    match name {
        // Walka
        "bronze_blade" | "tempered_blade" => {
            let amount = if name == "bronze_blade" { 2 } else { 3 };
            assert_eq!(cast(c).stat_delta(Side::A, Stat::PhysAtk), amount);
        }
        "recruit_pauldron" => assert_eq!(cast(c).stat_delta(Side::A, Stat::PhysDef), 2),
        "herbal_draught" => assert_eq!(cast(c).healed(Side::A), 6),
        "nimble_step" => {
            let run = play(c, scenario(c), 0, b_attacks_magic);
            assert!(
                run.missed(Side::B).is_empty(),
                "atak magiczny nie jest objęty kartą"
            );
            let run = cast(c);
            assert_eq!(run.missed(Side::B), vec![0]);
        }
        "beginners_luck" => assert_eq!(cast(c).hit(Side::A), vec![0]),
        "brawl" => {
            let run = cast(c);
            assert_eq!((run.damaged(Side::B), run.damaged(Side::A)), (6, 2));
        }
        "curved_blade" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysAtk),
                    run.stat_delta(Side::A, Stat::PhysDef)
                ),
                (2, 1)
            );
        }
        "relentlessness" => {
            let run = play(c, scenario(c), 0, b_defends_magic);
            assert!(run.enchanted());
            assert_eq!(run.hit(Side::A).len(), 3);
            assert_eq!(run.count(|e| matches!(*e, Event::ResourceChanged { side: Side::A, resource: Resource::Stamina, source, .. } if source == c.id)), 3);
        }
        "buckler" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysDef),
                    run.stat_delta(Side::A, Stat::MagDef)
                ),
                (1, 1)
            );
        }
        "truce" => {
            let run = cast(c);
            assert_eq!(
                (run.missed(Side::A).len(), run.missed(Side::B).len()),
                (3, 3)
            );
        }
        "armor_breaker" => assert_eq!(cast(c).stat_delta(Side::B, Stat::PhysDef), -2),
        "preemptive_strike" => assert_eq!(cast(c).hit(Side::A), vec![0, 1, 2]),
        "pitfall" => {
            let run = cast(c);
            assert_eq!(run.missed(Side::B), vec![1]);
            assert_eq!(run.resource(Side::B, Resource::Stamina), -4);
        }
        // Kapłaństwo
        "hearts_gift" => {
            let run = play(c, scenario(c).hp(Side::B, 50), 0, |mv| mv);
            assert_eq!((run.healed(Side::A), run.healed(Side::B)), (8, 8));
        }
        "spirit_shield" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::MagDef),
                    run.stat_delta(Side::A, Stat::PhysDef)
                ),
                (2, 1)
            );
        }
        "spirit_sword" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::MagAtk),
                    run.stat_delta(Side::A, Stat::PhysAtk)
                ),
                (2, 1)
            );
        }
        "shackles_of_truth" => assert_eq!(cast(c).resource(Side::B, Resource::Focus), -2),
        "acolyte" => {
            let run = play(c, scenario(c).stamina(Side::A, 30), 0, |mv| mv);
            assert!(run.enchanted());
            assert_eq!(
                (
                    run.healed(Side::A),
                    run.resource(Side::A, Resource::Stamina)
                ),
                (1, 2)
            );
        }
        "veterans_hymn" => {
            let run = play(c, scenario(c).stamina(Side::A, 30), 0, |mv| mv);
            assert_eq!(
                (
                    run.healed(Side::A),
                    run.resource(Side::A, Resource::Stamina)
                ),
                (6, 6)
            );
        }
        "sheltering_grace" => {
            let run = play(c, scenario(c), 0, b_attacks_magic);
            assert!(run.has(|e| matches!(
                *e,
                Event::DamageTakenChanged {
                    side: Side::A,
                    percent: 50,
                    ..
                }
            )));
            assert_eq!(
                run.state.damage_taken_pct(Side::A),
                100,
                "działa tylko w tej rundzie"
            );
        }
        "morning_hymn" => {
            let run = play(c, scenario(c).stamina(Side::A, 30), 0, |mv| mv);
            assert_eq!(
                (
                    run.healed(Side::A),
                    run.resource(Side::A, Resource::Stamina)
                ),
                (2, 2)
            );
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysDef),
                    run.stat_delta(Side::A, Stat::MagDef)
                ),
                (1, 1)
            );
        }
        "gauntlet_of_light" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysAtk),
                    run.stat_delta(Side::A, Stat::MagAtk)
                ),
                (1, 1)
            );
        }
        "soothing_touch" => assert_eq!(cast(c).healed(Side::A), 8),
        // Elementalizm
        "feral_might" | "treacherous_dagger" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysAtk),
                    run.stat_delta(Side::A, Stat::PhysDef)
                ),
                (3, -1)
            );
        }
        "sandstorm" => {
            let run = cast(c);
            assert_eq!(run.stat_changes(Side::B, Stat::PhysAtk).first(), Some(&-2));
            assert_eq!(run.stat_changes(Side::B, Stat::MagAtk).first(), Some(&-2));
            assert_eq!(
                run.stat_delta(Side::B, Stat::PhysAtk),
                0,
                "działa tylko w tej rundzie"
            );
        }
        "healing_mud" => assert_eq!(cast(c).healed(Side::A), 5),
        "thornhide" | "apprentices_retort" => {
            let run = play(c, scenario(c), 0, b_attacks_magic);
            assert!(run.enchanted());
            let per_hit = if name == "thornhide" { 1 } else { 2 };
            let hits = i32::try_from(run.hit(Side::B).len()).expect("mała liczba");
            assert!(hits > 0);
            assert_eq!(run.damaged(Side::B), per_hit * hits);
        }
        "toxic_fumes" => assert_eq!(cast(c).resource(Side::B, Resource::Stamina), -12),
        "leeching_vine" => {
            let run = cast(c);
            assert_eq!((run.damaged(Side::B), run.healed(Side::A)), (4, 4));
        }
        "stone_spike" => assert_eq!(cast(c).damaged(Side::B), 4),
        "desperate_blaze" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::MagAtk),
                    run.stat_delta(Side::A, Stat::MagDef)
                ),
                (4, -2)
            );
        }
        "oakbark" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysDef),
                    run.stat_delta(Side::A, Stat::MagDef)
                ),
                (2, 1)
            );
        }
        "venom" => {
            let run = cast(c);
            assert!(run.enchanted());
            assert_eq!(run.damaged(Side::B), 1);
        }
        "storm_spark" => assert_eq!(cast(c).damaged(Side::B), 2),
        "surge_of_power" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::MagAtk),
                    run.stat_delta(Side::A, Stat::MagDef)
                ),
                (2, 2)
            );
        }
        // Magia
        "unweave" => {
            let blade = card("card.bronze_blade");
            let scenario = scenario(c)
                .schools(Side::B, blade.requires)
                .hand(Side::B, &[blade.id]);
            let run = play(c, scenario, 0, |mv| Move {
                card: CardAction::Cast(0),
                ..mv
            });
            assert!(run.has(|e| matches!(*e, Event::SpellCountered { side: Side::B, card, .. } if card == blade.id)));
            assert_eq!(run.stat_delta(Side::B, Stat::PhysAtk), 0);
        }
        "sages_hood" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::MagAtk),
                    run.stat_delta(Side::A, Stat::MagDef)
                ),
                (2, 1)
            );
        }
        "mana_elixir" => assert_eq!(cast(c).stat_delta(Side::A, Stat::MagAtk), 2),
        "minor_ward" => assert_eq!(cast(c).stat_delta(Side::A, Stat::MagDef), 2),
        "wild_weave" => {
            let run = cast(c);
            assert!(run.has(
                |e| matches!(*e, Event::RandomSpell { side: Side::A, card, .. } if card == c.id)
            ));
        }
        "pain_echo" => assert_eq!(cast(c).damaged(Side::B), 3),
        "arcane_flick" => {
            let run = cast(c);
            assert_eq!(
                (run.damaged(Side::B), run.resource(Side::B, Resource::Focus)),
                (2, -1)
            );
        }
        "mind_over_steel" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.state.stats(Side::A).phys_atk,
                    run.state.stats(Side::A).mag_atk
                ),
                (4, 6)
            );
        }
        "sour_wine" | "dark_herald" => {
            let filler = id("card.storm_spark");
            let run = play(
                c,
                scenario(c).hand(Side::B, &[filler, filler, filler]),
                0,
                |mv| mv,
            );
            assert!(run.has(|e| matches!(*e, Event::ForcedDiscard { side: Side::B, source, .. } if source == c.id)));
        }
        // Nekromancja
        "despair" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.resource(Side::B, Resource::Focus),
                    run.resource(Side::B, Resource::Stamina)
                ),
                (-1, -4)
            );
        }
        "blight" => {
            let run = cast(c);
            assert!(run.enchanted());
            assert_eq!(
                (
                    run.damaged(Side::B),
                    run.resource(Side::B, Resource::Stamina)
                ),
                (1, -2)
            );
        }
        "grim_revival" => {
            let run = cast(c);
            assert_eq!(run.healed(Side::A), 12);
            assert_eq!(run.state.player(Side::A).used_interventions(), &[c.id]);
        }
        "cultist" => {
            let run = cast(c);
            assert!(run.enchanted());
            assert_eq!((run.damaged(Side::B), run.healed(Side::A)), (1, 1));
        }
        "frailty" => assert_eq!(cast(c).stat_delta(Side::B, Stat::PhysAtk), -2),
        "stupor" => assert_eq!(cast(c).stat_delta(Side::B, Stat::MagAtk), -2),
        "exposure" => {
            let run = cast(c);
            assert!(run.has(|e| matches!(
                *e,
                Event::DamageTakenChanged {
                    side: Side::B,
                    percent: 150,
                    ..
                }
            )));
        }
        "doomhelm" => {
            let run = cast(c);
            assert_eq!(
                (
                    run.stat_delta(Side::A, Stat::PhysDef),
                    run.stat_delta(Side::A, Stat::MagAtk)
                ),
                (3, -1)
            );
        }
        "shadow_pact" => {
            let run = play(c, scenario(c).stamina(Side::A, 30), 0, |mv| mv);
            assert_eq!(
                (
                    run.resource(Side::A, Resource::Stamina),
                    run.damaged(Side::A)
                ),
                (10, 4)
            );
        }
        "dread" => assert_eq!(cast(c).missed(Side::B), vec![0]),
        "soul_vortex" => {
            let run = play(c, scenario(c).stamina(Side::A, 30), 0, |mv| mv);
            assert_eq!(
                (
                    run.resource(Side::B, Resource::Stamina),
                    run.resource(Side::A, Resource::Stamina)
                ),
                (-6, 6)
            );
        }
        // Technologia
        "short_circuit" => {
            let acolyte = card("card.acolyte");
            let run = play(c, scenario(c).gift(Side::B, acolyte), 0, |mv| mv);
            assert!(run.has(|e| matches!(*e, Event::EnchantmentRemoved { side: Side::B, source, .. } if source == c.id)));
            assert_eq!(run.state.enchantments().count(), 0);
        }
        "paradox" => {
            // Działa od 4. rundy, gdy przeciwnik nie ma W. B bez W nie blokuje, więc dostaje
            // więcej Ż, żeby przetrwać 3 rundy trafień, a A więcej W na koszty trafień i karty.
            let tough = Scenario::new(stats(6, 3, 4, 2, 60, 200), stats(6, 3, 4, 2, 200, 60))
                .schools(Side::A, c.requires)
                .hand(Side::A, &[c.id])
                .stamina(Side::B, 0);
            let run = play(c, tough, 3, |mv| mv);
            assert_eq!(
                run.state.outcome(),
                Some(Outcome::Win {
                    winner: Side::A,
                    reason: Reason::ConditionalWin
                })
            );
            let early = play(c, scenario(c).stamina(Side::B, 0), 0, |mv| mv);
            assert!(early.has(|e| matches!(*e, Event::SpellFailed { side: Side::A, .. })));
            assert_eq!(early.state.outcome(), None);
        }
        "attack_lattice" | "defense_lattice" => {
            let run = cast(c);
            assert!(run.enchanted());
            let (first, second) = if name == "attack_lattice" {
                (Stat::PhysAtk, Stat::MagAtk)
            } else {
                (Stat::PhysDef, Stat::MagDef)
            };
            assert_eq!(
                (
                    run.stat_delta(Side::A, first),
                    run.stat_delta(Side::A, second)
                ),
                (1, 1)
            );
        }
        "phase_shift" => {
            let run = cast(c);
            assert!(run.has(|e| matches!(*e, Event::ChoicesSwapped { side: Side::B, .. })));
        }
        "attack_calibration" => assert_eq!(cast(c).hit(Side::A), vec![0, 1, 2]),
        "defense_calibration" => {
            let run = cast(c);
            let (first, second) = (Stat::PhysDef, Stat::MagDef);
            assert_eq!(run.stat_changes(Side::A, first).first(), Some(&2));
            assert_eq!(run.stat_changes(Side::A, second).first(), Some(&2));
            assert_eq!(
                run.stat_delta(Side::A, first),
                0,
                "działa tylko w tej rundzie"
            );
        }
        amplifier if amplifier.ends_with("_amplifier") => {
            let school = match amplifier.trim_end_matches("_amplifier") {
                "combat" => School::Combat,
                "divine" => School::Divine,
                "element" => School::Element,
                "arcane" => School::Arcane,
                "necro" => School::Necro,
                "tech" => School::Tech,
                other => panic!("nieznana szkoła {other}"),
            };
            let run = cast(c);
            assert_eq!(
                run.state.schools(Side::A).level(school),
                run.before.schools(Side::A).level(school) + 1
            );
        }
        _ => panic!("karta {key} nie ma testu scenariuszowego w tests/cards.rs"),
    }
}

#[test]
fn every_card_works_as_its_text_says() {
    for (_, key) in CARD_KEYS {
        check(key);
    }
}

#[test]
fn starter_decks_are_playable_from_level_one() {
    let content = content();
    for starter in STARTERS {
        let avatar = engine_content::avatar(starter.avatar).expect("postać startowa");
        let (_, schools) = avatar.at_level(1);
        for card in starter.deck {
            let def = content.get(*card).expect("karta talii");
            assert!(
                schools.meets(&def.requires),
                "karta {card} talii {:?} wymaga wyższych szkół",
                starter.school
            );
        }
        let total: u32 = starter
            .cards
            .iter()
            .map(|(_, count)| u32::from(*count))
            .sum();
        assert!(
            (36..=40).contains(&total),
            "zestaw {:?} ma {total} kart",
            starter.school
        );
    }
    assert_eq!(STARTERS.len(), 6);
}
