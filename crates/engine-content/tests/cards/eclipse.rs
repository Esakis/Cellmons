//! Numeric intent for Eclipse utility, counters and conditional finishers.
use super::*;
fn fixture(c: &CardDef) -> Scenario {
    let stats = Stats {
        stamina_max: 120,
        ..BASE
    };
    Scenario::new(stats, stats)
        .schools(Side::A, c.requires)
        .hand(Side::A, &[c.id])
        .hp(Side::A, 40)
        .stamina(Side::A, 60)
        .stamina(Side::B, 60)
}
pub(super) fn check(name: &str) -> bool {
    let c = card(&format!("card.{name}"));
    if !(199..=231).contains(&c.id) {
        return false;
    }
    let setup = fixture(c);
    let run = if c.keywords.counter {
        let blade = card("card.bronze_blade");
        let run = play(
            c,
            setup
                .schools(Side::B, blade.requires)
                .hand(Side::B, &[blade.id]),
            1,
            |m| Move {
                card: CardAction::Cast(0),
                ..m
            },
        );
        assert!(
            run.has(|e| matches!(*e,Event::SpellCountered{side:Side::B,card,..}if card==blade.id))
        );
        assert_eq!(run.stat_delta(Side::B, Stat::PhysAtk), 0);
        run
    } else if c.id >= 229 {
        let early = play(c, setup.clone(), 1, |m| m);
        assert!(early.has(|e| matches!(*e,Event::SpellFailed{side:Side::A,card}if card==c.id)));
        assert_eq!(early.healed(Side::A), 0);
        assert_eq!(early.damaged(Side::B), 0);
        assert_eq!(early.resource(Side::A, Resource::Focus), 0);
        play(c, setup, 2, |m| m)
    } else {
        play(c, setup, 1, |m| m)
    };
    match name {
        "marchers_canteen" => {
            assert_eq!(run.resource(Side::A, Resource::Stamina), 9);
            assert_eq!(
                run.state.player(Side::A).stamina() - run.before.player(Side::A).stamina(),
                6
            );
        }
        "split_guard" => {
            assert_eq!(run.stat_changes(Side::A, Stat::PhysDef).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 0);
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 1);
        }
        "rusted_edge" => {
            assert_eq!(run.stat_delta(Side::B, Stat::PhysDef), -1);
            assert_eq!(run.stat_changes(Side::A, Stat::PhysAtk).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 0);
        }
        "vigilant_step" => {
            assert_eq!(run.missed(Side::B), vec![0]);
            assert_eq!(run.resource(Side::A, Resource::Focus), -1);
        }
        "votive_thread" => {
            assert_eq!(run.stat_changes(Side::B, Stat::MagAtk).first(), Some(&-1));
            assert_eq!(run.stat_delta(Side::B, Stat::MagAtk), 0);
            assert_eq!(run.healed(Side::A), 2);
        }
        "quiet_toll" => {
            assert_eq!(run.resource(Side::B, Resource::Stamina), -5);
            assert_eq!(run.healed(Side::A), 1);
        }
        "shelter_pin" => {
            assert_eq!(run.stat_changes(Side::A, Stat::PhysDef).first(), Some(&2));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 0);
            assert_eq!(run.resource(Side::A, Resource::Focus), 1);
        }
        "courage_token" => {
            assert_eq!(run.stat_changes(Side::A, Stat::PhysAtk).first(), Some(&2));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 0);
            assert_eq!(run.healed(Side::A), 2);
        }
        "moss_band" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 1);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 4);
        }
        "cinder_fleck" => {
            assert_eq!(run.damaged(Side::B), 3);
            assert_eq!(run.damaged(Side::A), 1);
        }
        "dew_pocket" => {
            assert_eq!(run.healed(Side::A), 3);
            assert_eq!(run.resource(Side::A, Resource::Focus), 1);
        }
        "silt_veil" => {
            assert_eq!(run.stat_delta(Side::B, Stat::MagAtk), -1);
            assert_eq!(run.resource(Side::B, Resource::Stamina), -2);
        }
        "ink_reservoir" => {
            assert_eq!(run.resource(Side::A, Resource::Focus), 2);
            assert_eq!(run.damaged(Side::A), 1);
        }
        "glass_rune" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 1);
            assert_eq!(run.stat_changes(Side::A, Stat::MagDef).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 0);
        }
        "margin_note" => {
            assert_eq!(run.stat_changes(Side::A, Stat::MagAtk).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 0);
            assert_eq!(run.resource(Side::A, Resource::Focus), 1);
        }
        "pale_index" => {
            assert_eq!(run.resource(Side::B, Resource::Focus), -1);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 3);
        }
        "grave_ration" => {
            assert_eq!(run.damaged(Side::B), 2);
            assert_eq!(run.healed(Side::A), 2);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 3);
        }
        "sable_splinter" => {
            assert_eq!(run.stat_delta(Side::B, Stat::PhysDef), -1);
            assert_eq!(run.healed(Side::A), 1);
        }
        "funeral_knot" => {
            assert_eq!(run.resource(Side::B, Resource::Focus), -1);
            assert_eq!(run.stat_changes(Side::A, Stat::MagDef).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 0);
        }
        "cold_obol" => {
            assert_eq!(run.resource(Side::B, Resource::Stamina), -3);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 3);
            assert_eq!(run.damaged(Side::A), 1);
        }
        "copper_switch" => {
            assert_eq!(run.stat_changes(Side::A, Stat::PhysAtk).first(), Some(&1));
            assert_eq!(run.stat_changes(Side::A, Stat::MagAtk).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 0);
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 0);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 2);
        }
        "spare_capacitor" => {
            assert_eq!(run.resource(Side::A, Resource::Stamina), 8);
            assert_eq!(run.resource(Side::A, Resource::Focus), -1);
        }
        "washer_ward" => {
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 1);
            assert_eq!(run.stat_delta(Side::A, Stat::StaminaMax), 7);
        }
        "winding_key" => {
            assert_eq!(run.resource(Side::A, Resource::Focus), 3);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 2);
        }
        "silence_of_iron" => {
            assert_eq!(run.stat_changes(Side::A, Stat::PhysAtk).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 0);
        }
        "hushed_anthem" => assert_eq!(run.healed(Side::A), 2),
        "still_weather" => assert_eq!(run.resource(Side::B, Resource::Stamina), -3),
        "blank_margin" => assert_eq!(run.resource(Side::A, Resource::Focus), 2),
        "wordless_dirge" => assert_eq!(run.resource(Side::B, Resource::Focus), -2),
        "null_spindle" => assert_eq!(run.resource(Side::A, Resource::Stamina), 6),
        "eclipsed_verdict" => {
            assert_eq!(run.healed(Side::A), 8);
            assert_eq!(run.damaged(Side::B), 4);
        }
        "grave_clock" => {
            assert_eq!(run.damaged(Side::B), 6);
            assert_eq!(run.healed(Side::A), 6);
            assert_eq!(run.resource(Side::B, Resource::Focus), -2);
            assert_eq!(run.resource(Side::A, Resource::Focus), 2);
        }
        "zenith_pulse" => {
            assert_eq!(run.resource(Side::A, Resource::Focus), 3);
            assert_eq!(run.damaged(Side::B), 6);
        }
        _ => panic!("Missing Eclipse oracle for {name}"),
    }
    true
}
