//! Independent numeric and boundary scenarios for the final three content packets.
use super::*;

fn fixture(c: &CardDef) -> Scenario {
    let stats = Stats {
        stamina_max: 240,
        ..BASE
    };
    Scenario::new(stats, stats)
        .schools(Side::A, c.requires)
        .hand(Side::A, &[c.id])
        .hp(Side::A, 40)
        .stamina(Side::A, 100)
        .stamina(Side::B, 100)
}
fn stat(run: &Run, side: Side, key: Stat, amount: i32, temporary: bool) {
    assert_eq!(
        run.stat_changes(side, key).first(),
        Some(&amount),
        "{}",
        run.card
    );
    assert_eq!(
        run.stat_delta(side, key),
        if temporary { 0 } else { amount },
        "{}",
        run.card
    );
}
fn resource(run: &Run, side: Side, key: Resource, amount: i32) {
    assert_eq!(run.resource(side, key), amount, "{}", run.card);
}
fn inert(run: &Run) {
    assert!(
        run.has(|e| matches!(*e, Event::SpellFailed { side: Side::A, card } if card == run.card))
    );
    assert_eq!(run.healed(Side::A), 0);
    assert_eq!(run.damaged(Side::B), 0);
    for side in Side::BOTH {
        for key in [Stat::PhysAtk, Stat::PhysDef, Stat::MagAtk, Stat::MagDef] {
            assert!(run.stat_changes(side, key).is_empty());
        }
        for key in [Resource::Focus, Resource::Stamina] {
            resource(run, side, key, 0);
        }
    }
}

fn coupled(c: &CardDef, name: &str) {
    let school = match name {
        "tempered_chain" => School::Combat,
        "votive_spring" => School::Divine,
        "tide_filament" => School::Element,
        "memory_lattice" => School::Arcane,
        "ossuary_link" => School::Necro,
        "nested_flywheel" => School::Tech,
        _ => unreachable!(),
    };
    for (level, factor, extra_k, extra_w) in
        [(1, 1, 0, 0), (2, 2, 2, 10), (3, 3, 3, 15), (4, 4, 4, 20)]
    {
        let setup = fixture(c).schools(Side::A, c.requires.with(school, level));
        let run = play(c, setup, 1, |m| m);
        assert!(run.has(
            |e| matches!(*e, Event::SpellCast { side: Side::A, card, focus, stamina }
            if card==c.id && focus==2+extra_k && stamina==i32::from(c.cost.stamina)+extra_w)
        ));
        match name {
            "tempered_chain" => {
                stat(&run, Side::A, Stat::PhysAtk, factor, false);
                resource(&run, Side::A, Resource::Stamina, 2 * factor);
            }
            "votive_spring" => assert_eq!(run.healed(Side::A), 3 * factor),
            "tide_filament" => {
                assert_eq!(run.damaged(Side::B), 2 * factor);
                resource(&run, Side::B, Resource::Stamina, -factor);
            }
            "memory_lattice" => {
                resource(&run, Side::A, Resource::Focus, factor);
                stat(&run, Side::A, Stat::MagDef, factor, false);
            }
            "ossuary_link" => {
                assert_eq!(run.healed(Side::A), 2 * factor);
                assert_eq!(run.damaged(Side::B), 2 * factor);
            }
            "nested_flywheel" => {
                resource(&run, Side::A, Resource::Stamina, 3 * factor);
                stat(&run, Side::A, Stat::MagAtk, factor, true);
            }
            _ => unreachable!(),
        }
    }
}
fn hand_check(c: &CardDef, name: &str) {
    let setup = fixture(c)
        .hand(Side::A, &[c.id, 1, 2])
        .hand(Side::B, &[1, 2, 3, 4, 5])
        .deck(Side::A, &[3, 4, 5, 6, 7]);
    let run = play(c, setup, 0, |m| m);
    let lost = |side| {
        run.count(
            |e| matches!(*e, Event::ForcedDiscard { side:s, source,.. } if s==side && source==c.id),
        )
    };
    let drawn = run.count(|e| matches!(*e, Event::CardDrawn { side: Side::A, .. }));
    match name {
        "field_dispatch" => {
            assert_eq!(drawn, 3);
            stat(&run, Side::A, Stat::PhysAtk, 1, true);
        }
        "pilgrims_satchel" => {
            assert_eq!(drawn, 2);
            assert_eq!(run.healed(Side::A), 3);
        }
        "scattered_petals" => {
            assert_eq!(lost(Side::B), 1);
            resource(&run, Side::B, Resource::Stamina, -3);
        }
        "palimpsest_case" => {
            assert_eq!(lost(Side::A), 1);
            assert_eq!(drawn, 4);
            assert_eq!(run.state.player(Side::A).hand().len(), 5);
        }
        "sealed_testament" => {
            assert_eq!(lost(Side::B), 2);
            assert_eq!(run.damaged(Side::A), 2);
        }
        "parts_drawer" => {
            assert_eq!(drawn, 3);
            resource(&run, Side::A, Resource::Focus, -1);
        }
        "redacted_order" => {
            assert_eq!(lost(Side::B), 1);
            stat(&run, Side::A, Stat::PhysDef, 1, false);
        }
        "charitable_leaf" => {
            assert_eq!(drawn, 3);
            assert_eq!(run.healed(Side::A), 2);
        }
        "whirling_folios" => {
            assert_eq!(drawn, 3);
            resource(&run, Side::A, Resource::Stamina, 3);
        }
        "closed_index" => {
            assert_eq!(lost(Side::B), 1);
            resource(&run, Side::A, Resource::Focus, 1);
        }
        "testament_ashes" => {
            assert_eq!(lost(Side::B), 1);
            assert_eq!(run.healed(Side::A), 2);
            assert_eq!(run.damaged(Side::B), 2);
        }
        "folding_blueprint" => {
            assert_eq!(lost(Side::A), 2);
            assert_eq!(drawn, 4);
            assert_eq!(run.state.player(Side::A).hand().len(), 4);
            resource(&run, Side::A, Resource::Stamina, 4);
        }
        _ => unreachable!(),
    }
    // A forced discard can never create copies or draw above the configured hand limit.
    assert!(run.state.player(Side::A).hand().len() <= 5);
    let full = play(
        c,
        fixture(c)
            .hand(Side::A, &[c.id, 1, 2, 3, 4])
            .deck(Side::A, &[5, 6, 7]),
        0,
        |m| m,
    );
    assert!(full.state.player(Side::A).hand().len() <= 5);
}
fn cleanse(c: &CardDef, name: &str) {
    let target = if matches!(name, "purifying_censer" | "cleansing_rain" | "reset_socket") {
        Side::A
    } else {
        Side::B
    };
    let gift = card("card.dawn_embodiment");
    let run = play(c, fixture(c).gift(target, gift), 1, |m| m);
    assert!(run.has(|e|matches!(*e,Event::EnchantmentRemoved{side,card,source} if side==target && card==gift.id && source==c.id)));
    match name {
        "scouring_sabre" => stat(&run, Side::A, Stat::PhysAtk, 1, true),
        "purifying_censer" => assert_eq!(run.healed(Side::A), 6),
        "cleansing_rain" => resource(&run, Side::A, Resource::Stamina, 8),
        "erasure_quill" => resource(&run, Side::A, Resource::Focus, 1),
        "grave_solvent" => {
            resource(&run, Side::A, Resource::Stamina, 3);
            resource(&run, Side::B, Resource::Stamina, -3);
        }
        "reset_socket" => {
            assert_eq!(run.stat_changes(Side::A, Stat::MagDef).first(), Some(&1));
            assert_eq!(run.stat_delta(Side::A, Stat::MagDef), -1);
            resource(&run, Side::A, Resource::Stamina, 5);
        }
        _ => unreachable!(),
    }
    // Removing one's own continuous gift also removes its defense modifiers.
    assert_eq!(run.state.stats(target).phys_def, BASE.phys_def);
}
fn reactive(c: &CardDef, name: &str) {
    let run = if matches!(name, "impact_rivet" | "echo_abacus") {
        play(c, fixture(c), 1, b_defends_magic)
    } else if matches!(name, "rain_chain" | "mourning_bell") {
        play(c, fixture(c), 1, b_attacks_magic)
    } else {
        play(c, fixture(c), 1, |m| m)
    };
    assert!(run.enchanted());
    match name {
        "impact_rivet" => {
            resource(&run, Side::A, Resource::Stamina, 3);
            assert_eq!(run.healed(Side::A), 3);
        }
        "mercy_chime" => assert_eq!(run.healed(Side::A), 3),
        "rain_chain" => resource(&run, Side::A, Resource::Stamina, 3),
        "echo_abacus" | "guard_escapement" => resource(&run, Side::A, Resource::Focus, 3),
        "mourning_bell" => resource(&run, Side::B, Resource::Stamina, -3),
        _ => unreachable!(),
    }
}
fn counter(c: &CardDef, name: &str) {
    let blade = card("card.bronze_blade");
    let run = play(
        c,
        fixture(c)
            .hand(Side::A, &[c.id, c.id])
            .schools(Side::B, blade.requires)
            .hand(Side::B, &[blade.id]),
        1,
        |m| Move {
            card: CardAction::Cast(0),
            ..m
        },
    );
    assert!(run.has(|e| matches!(*e,Event::SpellCountered{side:Side::B,card,..}if card==blade.id)));
    assert_eq!(run.stat_delta(Side::B, Stat::PhysAtk), 0);
    assert_eq!(run.state.player(Side::A).used_interventions(), &[c.id]);
    let repeat = Move {
        card: CardAction::Cast(0),
        ..default_move(&run.state, Side::A)
    };
    assert!(engine::validate_move(&run.state, &content(), Side::A, &repeat).is_err());
    match name {
        "commanders_silence" => stat(&run, Side::A, Stat::PhysDef, 2, true),
        "sacred_stillness" => assert_eq!(run.healed(Side::A), 6),
        "weather_seal" => resource(&run, Side::B, Resource::Stamina, -7),
        "unwritten_hour" => resource(&run, Side::A, Resource::Focus, 4),
        "last_requiem" => {
            assert_eq!(run.healed(Side::A), 4);
            assert_eq!(run.damaged(Side::B), 4);
        }
        "absolute_interlock" => resource(&run, Side::A, Resource::Stamina, 10),
        _ => unreachable!(),
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn check(name: &str) -> bool {
    let c = card(&format!("card.{name}"));
    if !(232..=330).contains(&c.id) {
        return false;
    }
    match c.id {
        232..=237 => {
            coupled(c, name);
            return true;
        }
        244..=249 | 304..=309 => {
            hand_check(c, name);
            return true;
        }
        250..=255 => {
            cleanse(c, name);
            return true;
        }
        256..=261 => {
            reactive(c, name);
            return true;
        }
        316..=321 => {
            counter(c, name);
            return true;
        }
        _ => {}
    }
    let setup = fixture(c);
    let run = if (238..=243).contains(&c.id) {
        inert(&play(c, setup.clone().hp(Side::B, 50), 1, |m| m));
        // Equal percentages do not satisfy the strict advantage condition.
        inert(&play(c, setup.clone().hp(Side::B, 40), 1, |m| m));
        play(c, setup.hp(Side::B, 20), 1, |m| m)
    } else if (298..=303).contains(&c.id) {
        inert(&play(c, setup.clone().stamina(Side::B, 1), 0, |m| m));
        play(c, setup.stamina(Side::B, 0), 0, |m| m)
    } else if c.id >= 328 {
        inert(&play(c, setup.clone(), 2, |m| m));
        play(c, setup, 3, |m| m)
    } else if (310..=315).contains(&c.id) || name == "cloud_annotation" {
        play(c, setup, 1, b_attacks_magic)
    } else if c.keywords.intervention {
        let run = play(c, setup.hand(Side::A, &[c.id, c.id]), 1, |m| m);
        assert_eq!(run.state.player(Side::A).used_interventions(), &[c.id]);
        let repeat = Move {
            card: CardAction::Cast(0),
            ..default_move(&run.state, Side::A)
        };
        assert!(engine::validate_move(&run.state, &content(), Side::A, &repeat).is_err());
        run
    } else {
        play(c, setup, 1, |m| m)
    };
    match name {
        "victors_standard" => {
            stat(&run, Side::A, Stat::PhysAtk, 3, false);
            stat(&run, Side::A, Stat::PhysDef, 1, false);
        }
        "unbroken_day" => {
            assert_eq!(run.healed(Side::A), 8);
            stat(&run, Side::A, Stat::MagDef, 1, false);
        }
        "high_watermark" => {
            assert_eq!(run.damaged(Side::B), 7);
            resource(&run, Side::A, Resource::Stamina, 4);
        }
        "keepers_footnote" => {
            resource(&run, Side::A, Resource::Focus, 3);
            stat(&run, Side::A, Stat::MagAtk, 2, true);
        }
        "grave_precedence" => {
            assert_eq!(run.healed(Side::A), 5);
            assert_eq!(run.damaged(Side::B), 5);
            resource(&run, Side::B, Resource::Focus, -1);
        }
        "prime_regulator" => {
            stat(&run, Side::A, Stat::PhysDef, 2, false);
            resource(&run, Side::A, Resource::Stamina, 8);
        }
        "last_banner" => {
            stat(&run, Side::A, Stat::PhysAtk, 5, true);
            stat(&run, Side::A, Stat::PhysDef, 3, true);
        }
        "daybreak_reliquary" => assert_eq!(run.healed(Side::A), 16),
        "singularity_battery" => {
            resource(&run, Side::A, Resource::Stamina, 24);
            resource(&run, Side::A, Resource::Focus, 2);
            assert_eq!(run.damaged(Side::A), 4);
        }
        "oath_tether" => {
            stat(&run, Side::A, Stat::PhysAtk, 2, false);
            assert_eq!(run.healed(Side::A), 3);
        }
        "tidal_guard" => {
            stat(&run, Side::A, Stat::PhysDef, 2, false);
            resource(&run, Side::A, Resource::Stamina, 5);
        }
        "scripted_lunge" => {
            assert_eq!(run.hit(Side::A), [1]);
            resource(&run, Side::A, Resource::Focus, 1);
        }
        "bone_measure" => {
            stat(&run, Side::B, Stat::PhysDef, -2, false);
            assert_eq!(run.healed(Side::A), 2);
            assert_eq!(run.damaged(Side::B), 2);
        }
        "ratchet_stance" => {
            assert_eq!(run.hit(Side::A), [2]);
            resource(&run, Side::A, Resource::Stamina, 4);
        }
        "verdant_litany" => {
            assert_eq!(run.healed(Side::A), 5);
            resource(&run, Side::A, Resource::Stamina, 8);
        }
        "radiant_clause" => {
            stat(&run, Side::A, Stat::MagDef, 2, true);
            resource(&run, Side::A, Resource::Focus, 2);
        }
        "twilight_balance" => {
            assert_eq!(run.healed(Side::A), 4);
            assert_eq!(run.damaged(Side::B), 7);
        }
        "sanctified_piston" => {
            stat(&run, Side::A, Stat::HpMax, 8, false);
            assert_eq!(run.healed(Side::A), 8);
            resource(&run, Side::A, Resource::Stamina, 2);
        }
        "cloud_annotation" => {
            assert_eq!(run.missed(Side::B), [0]);
            stat(&run, Side::A, Stat::MagAtk, 1, true);
        }
        "peat_memory" => {
            resource(&run, Side::A, Resource::Stamina, 5);
            resource(&run, Side::B, Resource::Stamina, -5);
            assert_eq!(run.healed(Side::A), 2);
        }
        "storm_servo" => {
            assert_eq!(run.damaged(Side::B), 5);
            stat(&run, Side::A, Stat::PhysDef, 1, false);
        }
        "sepulchral_formula" => {
            resource(&run, Side::A, Resource::Focus, 2);
            resource(&run, Side::B, Resource::Focus, -2);
            stat(&run, Side::B, Stat::MagAtk, -1, true);
        }
        "indexed_gear" => {
            resource(&run, Side::A, Resource::Focus, 2);
            resource(&run, Side::A, Resource::Stamina, 5);
        }
        "blackened_motor" => {
            resource(&run, Side::A, Resource::Stamina, 4);
            resource(&run, Side::B, Resource::Stamina, -4);
            stat(&run, Side::A, Stat::MagAtk, 1, false);
        }
        "masters_muster" | "sacred_aperture" | "primal_conduit" | "fourth_theorem"
        | "ossuary_gate" | "precision_foundry" => {
            let key = match name {
                "masters_muster" => School::Combat,
                "sacred_aperture" => School::Divine,
                "primal_conduit" => School::Element,
                "fourth_theorem" => School::Arcane,
                "ossuary_gate" => School::Necro,
                _ => School::Tech,
            };
            assert_eq!(run.before.schools(Side::A).level(key), 3);
            assert_eq!(run.state.schools(Side::A).level(key), 4);
            let capped = play(
                c,
                fixture(c).schools(Side::A, c.requires.with(key, 4)),
                1,
                |m| m,
            );
            assert_eq!(capped.state.schools(Side::A).level(key), 4);
            match name {
                "masters_muster" => stat(&run, Side::A, Stat::PhysDef, 1, false),
                "sacred_aperture" => assert_eq!(run.healed(Side::A), 4),
                "primal_conduit" => resource(&run, Side::A, Resource::Stamina, 5),
                "fourth_theorem" => resource(&run, Side::A, Resource::Focus, 1),
                "ossuary_gate" => {
                    assert_eq!(run.healed(Side::A), 2);
                    assert_eq!(run.damaged(Side::B), 2);
                }
                _ => stat(&run, Side::A, Stat::MagDef, 1, false),
            }
        }
        "fervent_exchange" => {
            stat(&run, Side::A, Stat::PhysAtk, 4, false);
            stat(&run, Side::A, Stat::PhysDef, -2, false);
        }
        "entrenched_will" => {
            stat(&run, Side::A, Stat::PhysDef, 3, false);
            stat(&run, Side::A, Stat::MagAtk, -2, false);
        }
        "martyrs_measure" => {
            stat(&run, Side::A, Stat::HpMax, 14, false);
            assert_eq!(run.healed(Side::A), 14);
            stat(&run, Side::A, Stat::PhysAtk, -2, false);
        }
        "luminous_burden" => {
            stat(&run, Side::A, Stat::MagDef, 3, false);
            resource(&run, Side::A, Resource::Stamina, -5);
        }
        "undertow_bargain" => {
            stat(&run, Side::A, Stat::MagAtk, 3, false);
            stat(&run, Side::A, Stat::PhysDef, -1, false);
            resource(&run, Side::A, Resource::Stamina, -3);
        }
        "stone_breath" => {
            stat(&run, Side::A, Stat::PhysDef, 3, false);
            stat(&run, Side::A, Stat::MagDef, 1, false);
            stat(&run, Side::A, Stat::StaminaMax, -8, false);
        }
        "lucid_wager" => {
            resource(&run, Side::A, Resource::Focus, 5);
            assert_eq!(run.damaged(Side::A), 5);
        }
        "mirror_debt" => {
            stat(&run, Side::A, Stat::MagAtk, 4, false);
            stat(&run, Side::A, Stat::MagDef, -2, false);
        }
        "blood_inventory" => {
            assert_eq!(run.healed(Side::A), 7);
            assert_eq!(run.damaged(Side::B), 7);
            stat(&run, Side::A, Stat::MagDef, -2, false);
        }
        "hollow_armour" => {
            stat(&run, Side::A, Stat::PhysDef, 4, false);
            assert_eq!(run.damaged(Side::A), 6);
        }
        "overclocked_array" => {
            stat(&run, Side::A, Stat::MagAtk, 3, false);
            stat(&run, Side::A, Stat::PhysAtk, 2, false);
            assert_eq!(run.damaged(Side::A), 6);
        }
        "armoured_accumulator" => {
            stat(&run, Side::A, Stat::PhysDef, 3, false);
            stat(&run, Side::A, Stat::StaminaMax, 12, false);
            resource(&run, Side::A, Resource::Focus, -2);
        }
        "pursuers_mark" => {
            assert_eq!(run.damaged(Side::B), 9);
            stat(&run, Side::A, Stat::PhysAtk, 1, false);
        }
        "quiet_judgement" => {
            assert_eq!(run.healed(Side::A), 10);
            assert_eq!(run.damaged(Side::B), 3);
        }
        "desiccation_orb" => {
            assert_eq!(run.damaged(Side::B), 8);
            resource(&run, Side::A, Resource::Stamina, 6);
        }
        "empty_page" => {
            assert_eq!(run.damaged(Side::B), 4);
            resource(&run, Side::A, Resource::Focus, 4);
        }
        "final_obol" => {
            assert_eq!(run.healed(Side::A), 8);
            assert_eq!(run.damaged(Side::B), 8);
            resource(&run, Side::B, Resource::Focus, -1);
        }
        "vacuum_ratchet" => {
            resource(&run, Side::A, Resource::Stamina, 14);
            stat(&run, Side::A, Stat::MagAtk, 2, true);
        }
        "iron_twilight" | "vesper_aegis" | "mist_bastion" | "refracted_hour" | "grave_screen"
        | "ceramic_shroud" => {
            assert!(run.has(|e|matches!(*e,Event::DamageTakenChanged{side:Side::A,percent:75,source}if source==c.id)));
            let damage = if name == "ceramic_shroud" { 12 } else { 15 };
            let heal = if name == "vesper_aegis" { 3 } else { 0 };
            assert_eq!(run.state.player(Side::A).hp(), 40 + heal - damage);
            match name {
                "iron_twilight" => stat(&run, Side::A, Stat::PhysAtk, 1, true),
                "vesper_aegis" => assert_eq!(run.healed(Side::A), 3),
                "mist_bastion" => resource(&run, Side::A, Resource::Stamina, 4),
                "refracted_hour" => resource(&run, Side::A, Resource::Focus, 1),
                "grave_screen" => assert_eq!(run.damaged(Side::B), 3),
                _ => stat(&run, Side::A, Stat::MagDef, 1, true),
            }
        }
        "rearguard_signal" => {
            stat(&run, Side::A, Stat::PhysDef, 1, true);
            resource(&run, Side::A, Resource::Stamina, 4);
        }
        "candle_reserve" => {
            assert_eq!(run.healed(Side::A), 2);
            stat(&run, Side::A, Stat::MagDef, 1, true);
        }
        "rain_measure" => {
            resource(&run, Side::A, Resource::Stamina, 5);
            stat(&run, Side::A, Stat::MagAtk, 1, true);
        }
        "fractional_second" => {
            resource(&run, Side::A, Resource::Focus, 1);
            stat(&run, Side::A, Stat::PhysDef, 1, true);
        }
        "bone_needle" => {
            assert_eq!(run.damaged(Side::B), 2);
            assert_eq!(run.healed(Side::A), 1);
        }
        "spare_spring" => {
            resource(&run, Side::A, Resource::Stamina, 4);
            stat(&run, Side::A, Stat::MagDef, 1, true);
        }
        "dusk_procession" => {
            assert_eq!(run.healed(Side::A), 6);
            assert_eq!(run.damaged(Side::B), 6);
            stat(&run, Side::B, Stat::PhysDef, -2, false);
        }
        "last_light" => {
            assert_eq!(run.healed(Side::A), 10);
            resource(&run, Side::A, Resource::Stamina, 10);
        }
        "terminal_equation" => {
            assert_eq!(run.damaged(Side::B), 8);
            resource(&run, Side::A, Resource::Focus, 3);
        }
        _ => panic!("Missing final packet oracle {name}"),
    }
    true
}
