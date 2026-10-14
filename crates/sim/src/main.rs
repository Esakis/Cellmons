//! Symulator balansu: pojedynki botów i raporty (`docs/11-ai-i-symulacje.md`).
//!
//! Na razie jedno polecenie: `sim duel [ziarno]` rozgrywa losowy pojedynek kartami testowymi
//! i wypisuje jego przebieg. Boty i raporty balansu dojdą w krokach F2.6 i F3.5 planu.

use std::io::{self, Write};
use std::process::ExitCode;

use engine::testkit::{content, play_random_duel, sample_cards, seeded_rng};
use engine::{
    CardAction, DuelState, Event, EventBuf, Kind, Move, Outcome, PlayerTotals, Reason, School,
    Side, end_by, new_duel, resolve_round,
};

const USAGE: &str = "Użycie: sim duel [ziarno]
  duel  losowy pojedynek opisany runda po rundzie (domyślne ziarno: 1)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let seed = match args.as_slice() {
        [command] if command == "duel" => Some(1),
        [command, seed] if command == "duel" => seed.parse::<u64>().ok(),
        _ => None,
    };
    let Some(seed) = seed else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    match duel_report(seed, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sim: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Rozgrywa losowy pojedynek, a potem odtwarza go runda po rundzie z zapisu i opisuje przebieg.
fn duel_report(seed: u64, out: &mut impl Write) -> io::Result<()> {
    let cards = sample_cards();
    let content = content(&cards);
    let (_, record) = play_random_duel(
        &mut seeded_rng(seed),
        &content,
        &mut EventBuf::new(),
        |_| {},
    );
    let mut state = new_duel(&record.setup, &content).expect("zapisane wejście jest poprawne");
    let mut events = EventBuf::new();

    writeln!(
        out,
        "Pojedynek testowy (ziarno {seed}): losowe postacie, talie i ruchy."
    )?;
    writeln!(
        out,
        "Karty nie mają jeszcze efektów (faza F2), tylko koszty."
    )?;
    writeln!(out)?;
    for side in Side::BOTH {
        writeln!(
            out,
            "Gracz {}: {}",
            name(side),
            describe_player(&state, side)
        )?;
    }
    writeln!(out, "Inicjatywa: {}", name(state.initiative()))?;

    for moves in &record.moves {
        let focus = Side::BOTH.map(|side| state.player(side).focus());
        writeln!(out)?;
        writeln!(
            out,
            "Runda {} (K na start: A {}, B {})",
            state.round(),
            focus[0],
            focus[1]
        )?;
        events.clear();
        let summary = resolve_round(&mut state, &content, moves, &mut events)
            .expect("zapisane ruchy są poprawne");
        for event in events.as_slice() {
            describe_event(out, event)?;
        }
        let [a, b] = summary.totals;
        writeln!(out, "  Koniec rundy: A {} | B {}", totals(a), totals(b))?;
    }
    if let Some((loser, termination)) = record.end {
        end_by(&mut state, loser, termination, &mut events).expect("pojedynek trwa");
        writeln!(out)?;
        writeln!(out, "Gracz {} kończy pojedynek przed czasem.", name(loser))?;
    }
    if let Some(outcome) = state.outcome() {
        writeln!(out)?;
        writeln!(out, "Wynik: {}", describe_outcome(outcome))?;
    }
    Ok(())
}

fn name(side: Side) -> &'static str {
    match side {
        Side::A => "A",
        Side::B => "B",
    }
}

fn letter(kind: Kind) -> &'static str {
    match kind {
        Kind::Phys => "F",
        Kind::Mag => "M",
    }
}

fn letters(kinds: &[Kind; 3]) -> String {
    kinds
        .iter()
        .map(|&kind| letter(kind))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Polska odmiana po liczebniku: 1 obrażenie, 2 obrażenia, 5 obrażeń.
fn plural(count: i32, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let (last, last_two) = (count.abs() % 10, count.abs() % 100);
    if count.abs() == 1 {
        one
    } else if (2..=4).contains(&last) && !(12..=14).contains(&last_two) {
        few
    } else {
        many
    }
}

fn totals(totals: PlayerTotals) -> String {
    format!("Ż {}, W {}, K {}", totals.hp, totals.stamina, totals.focus)
}

fn describe_player(state: &DuelState, side: Side) -> String {
    let player = state.player(side);
    let stats = player.stats();
    let school_names = [
        (School::Combat, "Walka"),
        (School::Divine, "Kapłaństwo"),
        (School::Element, "Elementalizm"),
        (School::Arcane, "Magia"),
        (School::Necro, "Nekromancja"),
        (School::Tech, "Technologia"),
    ];
    let schools: Vec<String> = school_names
        .iter()
        .filter(|(school, _)| player.schools().level(*school) > 0)
        .map(|(school, name)| format!("{name} {}", player.schools().level(*school)))
        .collect();
    let hand: Vec<String> = player.hand().iter().map(ToString::to_string).collect();
    format!(
        "AF {}, OF {}, AM {}, OM {}, Ż {}, W {}; szkoły: {}; karty w ręce: {}",
        stats.phys_atk,
        stats.phys_def,
        stats.mag_atk,
        stats.mag_def,
        stats.hp_max,
        stats.stamina_max,
        if schools.is_empty() {
            "brak".to_owned()
        } else {
            schools.join(", ")
        },
        hand.join(", "),
    )
}

fn describe_move(side: Side, mv: &Move, change_cost: i32) -> String {
    let card = match mv.card {
        CardAction::None => String::new(),
        CardAction::Cast(index) => format!(", rzuca kartę z pozycji {}", index + 1),
        CardAction::Discard(index) => format!(", odrzuca kartę z pozycji {}", index + 1),
    };
    format!(
        "  {}: ataki {}, obrony {}, zmiany za {change_cost} K{card}",
        name(side),
        letters(&mv.attacks),
        letters(&mv.defenses),
    )
}

fn describe_event(out: &mut impl Write, event: &Event) -> io::Result<()> {
    match *event {
        Event::MovesRevealed { moves, change_cost } => {
            for side in Side::BOTH {
                let index = side.index();
                writeln!(
                    out,
                    "{}",
                    describe_move(side, &moves[index], change_cost[index])
                )?;
            }
            Ok(())
        }
        Event::SpellCast {
            side,
            card,
            focus,
            stamina,
        } => {
            writeln!(
                out,
                "  {} rzuca kartę {card}: −{focus} K, −{stamina} W",
                name(side)
            )
        }
        Event::SpellFizzled { side, card } => {
            writeln!(
                out,
                "  {}: karta {card} przepada, bo nie da się jej opłacić",
                name(side)
            )
        }
        Event::CardDiscarded { side, card } => {
            writeln!(out, "  {} odrzuca kartę {card}", name(side))
        }
        Event::AttackHit {
            attacker,
            slot,
            kind,
            multiplier,
            damage,
            stamina_cost,
        } => {
            let damage_word = plural(damage, "obrażenie", "obrażenia", "obrażeń");
            let stamina = if stamina_cost > 0 {
                format!("−{stamina_cost} W")
            } else {
                "W bez zmian".to_owned()
            };
            writeln!(
                out,
                "  {}{} {}: trafienie ×{multiplier}, {damage} {damage_word}, {stamina}",
                name(attacker),
                slot + 1,
                letter(kind),
            )
        }
        Event::AttackBlocked {
            attacker,
            slot,
            kind,
        } => {
            writeln!(
                out,
                "  {}{} {}: blok",
                name(attacker),
                slot + 1,
                letter(kind)
            )
        }
        Event::CardDrawn { side, card } => writeln!(out, "  {} dobiera kartę {card}", name(side)),
        // Początek rundy i wynik opisuje `duel_report`.
        Event::RoundStarted { .. } | Event::DuelEnded { .. } => Ok(()),
    }
}

fn describe_outcome(outcome: Outcome) -> String {
    let reason = |reason: Reason| match reason {
        Reason::Knockout => "nokaut",
        Reason::Rounds => "wyższy procent życia po ostatniej rundzie",
        Reason::Forfeit => "ucieczka przeciwnika",
        Reason::Disconnect => "rozłączenie przeciwnika",
        Reason::Timeout => "walkower",
    };
    match outcome {
        Outcome::Win {
            winner,
            reason: why,
        } => {
            format!("wygrywa {} ({})", name(winner), reason(why))
        }
        Outcome::Draw {
            reason: Reason::Knockout,
        } => "remis (obaj gracze bez życia)".to_owned(),
        Outcome::Draw { .. } => "remis (równy procent życia)".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{duel_report, plural};

    #[test]
    fn duel_report_describes_every_duel_to_the_end() {
        for seed in 0..50 {
            let mut out = Vec::new();
            duel_report(seed, &mut out).expect("zapis do pamięci");
            let text = String::from_utf8(out).expect("UTF-8");
            assert!(text.contains("Inicjatywa: "), "ziarno {seed}");
            assert!(
                text.trim_end()
                    .lines()
                    .last()
                    .is_some_and(|line| line.starts_with("Wynik: "))
            );
        }
    }

    #[test]
    fn plural_follows_polish_grammar() {
        let word = |count| plural(count, "obrażenie", "obrażenia", "obrażeń");
        assert_eq!(word(1), "obrażenie");
        assert_eq!(word(4), "obrażenia");
        assert_eq!(word(22), "obrażenia");
        assert_eq!(word(0), "obrażeń");
        assert_eq!(word(5), "obrażeń");
        assert_eq!(word(12), "obrażeń");
        assert_eq!(word(112), "obrażeń");
    }
}
