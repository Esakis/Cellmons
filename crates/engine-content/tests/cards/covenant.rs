//! Independent numerical oracles for the first Covenant packet. Keep literal expectations here.
use super::*;
pub(super) fn check(name: &str) -> bool {
    let c = card(&format!("card.{name}"));
    if c.id < 166 || c.id > 198 {
        return false;
    }
    // Effects are tested in round two with the real focus policy and enough stamina for both sides.
    let stats = Stats {
        stamina_max: 120,
        ..BASE
    };
    let setup = Scenario::new(stats, stats)
        .schools(Side::A, c.requires)
        .hand(Side::A, &[c.id])
        .hp(Side::A, 40);
    let run = match name {
        "storm_confluence" => play_as(
            c,
            setup,
            1,
            |m| Move {
                attacks: [Kind::Mag; 3],
                ..m
            },
            b_defends_magic,
        ),
        "sanctum_mirror" => play(c, setup, 1, b_attacks_magic),
        _ => play(c, setup, 1, |m| m),
    };
    match name {
        "final_brand" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 6);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), -2);
        }
        "bastion_oath" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 5);
            assert_eq!(run.stat_delta(Side::A, Stat::HpMax), 14);
            assert_eq!(run.healed(Side::A), 14);
        }
        "red_horizon" => {
            assert_eq!(run.hit(Side::A), vec![0]);
            assert_eq!(run.stat_changes(Side::A, Stat::PhysAtk).first(), Some(&4));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 0);
            assert_eq!(run.resource(Side::A, Resource::Stamina), -6);
        }
        "dawn_covenant" => {
            assert_eq!(run.healed(Side::A), 18);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 2);
        }
        "sanctum_mirror" => {
            assert_eq!(
                run.before.player(Side::A).hp() - run.state.player(Side::A).hp(),
                6 // floor(max(0,4−5)/2) + floor((8−5)/2) + floor((16−5)/2)
            );
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 3);
        }
        "light_balance" => {
            assert_eq!(run.healed(Side::A), 12);
            assert_eq!(run.damaged(Side::B), 8);
        }
        "crucible_sky" => {
            assert_eq!(run.damaged(Side::B), 14);
            assert_eq!(run.resource(Side::B, Resource::Stamina), -6);
        }
        "rime_crown" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 3);
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 3);
        }
        "storm_confluence" => {
            assert_eq!(run.hit(Side::A), vec![0]);
            assert_eq!(run.stat_changes(Side::A, Stat::MagAtk).first(), Some(&5));
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 0);
        }
        "seventh_hour" => {
            assert_eq!(run.resource(Side::A, Resource::Focus), 3);
            assert_eq!(run.resource(Side::A, Resource::Stamina), -4);
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 3);
        }
        "astral_prism" => {
            assert_eq!(run.stat_delta(Side::B, Stat::PhysAtk), -2);
            assert_eq!(run.stat_delta(Side::B, Stat::MagAtk), 2);
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 3);
        }
        "folded_horizon" => {
            assert_eq!(run.resource(Side::B, Resource::Focus), -3);
            assert_eq!(run.resource(Side::B, Resource::Stamina), -6);
            assert_eq!(run.resource(Side::A, Resource::Focus), 2);
        }
        "mourning_throne" => {
            assert_eq!(run.damaged(Side::B), 10);
            assert_eq!(run.healed(Side::A), 10);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 2);
        }
        "last_ember" => {
            assert_eq!(run.damaged(Side::B), 12);
            assert_eq!(run.damaged(Side::A), 4);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 8);
        }
        "night_tithe" => {
            assert_eq!(run.resource(Side::B, Resource::Stamina), -10);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 10);
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 2);
        }
        "aether_reactor" => {
            assert_eq!(run.stat_delta(Side::A, Stat::StaminaMax), 14);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 20);
            assert_eq!(run.resource(Side::A, Resource::Focus), 1);
        }
        "clockwork_aegis" => {
            assert_eq!(run.stat_changes(Side::A, Stat::PhysDef).first(), Some(&4));
            assert_eq!(run.stat_changes(Side::A, Stat::MagDef).first(), Some(&2));
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 0);
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 0);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 8);
        }
        "convergence_engine" => {
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 4);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 2);
            assert_eq!(run.damaged(Side::A), 6);
        }
        "sanctuary_steel" => {
            assert_eq!(run.healed(Side::A), 6);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 2);
        }
        "monsoon_feint" => {
            assert_eq!(run.missed(Side::B), vec![0]);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 5);
        }
        "steel_refrain" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), -1);
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 2);
        }
        "marrow_oath" => {
            assert_eq!(run.stat_delta(Side::A, Stat::PhysAtk), 4);
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 1);
            assert_eq!(run.damaged(Side::A), 3);
        }
        "gyroblade" => {
            assert_eq!(run.hit(Side::A), vec![0]);
            assert_eq!(run.resource(Side::A, Resource::Focus), 1);
        }
        "dew_censer" => {
            assert_eq!(run.healed(Side::A), 10);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 5);
        }
        "prayer_lens" => {
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), 2);
            assert_eq!(run.resource(Side::A, Resource::Focus), 1);
        }
        "twilight_accord" => {
            assert_eq!(run.healed(Side::A), 6);
            assert_eq!(run.damaged(Side::B), 6);
            assert_eq!(run.resource(Side::A, Resource::Focus), -1);
        }
        "dawn_automaton" => {
            assert_eq!(run.healed(Side::A), 4);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 1);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 4);
        }
        "resonant_cloud" => {
            assert_eq!(run.stat_changes(Side::A, Stat::MagAtk).first(), Some(&2));
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 0);
            assert_eq!(run.stat_delta(Side::B, Stat::MagDef), -2);
        }
        "ashroot_pact" => {
            assert_eq!(run.damaged(Side::B), 4);
            assert_eq!(run.healed(Side::A), 4);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 4);
        }
        "thought_gear" => {
            assert_eq!(run.resource(Side::A, Resource::Focus), 2);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 6);
        }
        "thunder_socket" => {
            assert_eq!(run.damaged(Side::B), 6);
            assert_eq!(run.stat_delta(Side::A, Stat::PhysDef), 1);
        }
        "umbral_parallax" => {
            assert_eq!(run.stat_delta(Side::B, Stat::MagDef), -2);
            assert_eq!(run.resource(Side::B, Resource::Focus), -2);
        }
        "mortal_coil" => {
            assert_eq!(run.resource(Side::B, Resource::Stamina), -5);
            assert_eq!(run.resource(Side::A, Resource::Stamina), 5);
            assert_eq!(run.stat_delta(Side::A, Stat::MagAtk), 1);
        }
        _ => panic!("Missing Covenant oracle for {name}"),
    }
    true
}

#[test]
fn capstones_need_four_innate_school_levels_and_each_shade_perfection_has_three() {
    let content = content();
    let cards: Vec<_> = content
        .cards()
        .iter()
        .filter(|c| (166..=183).contains(&c.id))
        .collect();
    assert_eq!(cards.len(), 18);
    for base in engine_content::AVATARS {
        assert!(
            cards
                .iter()
                .all(|c| !base.at_level(20).1.meets(&c.requires))
        );
    }
    for p in engine_content::PATHS {
        let count = cards
            .iter()
            .filter(|c| p.avatar.at_level(20).1.meets(&c.requires))
            .count();
        assert_eq!(
            count,
            if p.avatar.rarity == engine_content::AvatarRarity::Shade && p.variant == 1 {
                3
            } else {
                0
            }
        );
    }
}
