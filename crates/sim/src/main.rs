//! Symulator balansu: pojedynki botów i raporty (`docs/11-ai-i-symulacje.md`).
//!
//! `sim duel` opisuje jeden pojedynek botów runda po rundzie, a `sim run` rozgrywa serię pojedynków
//! i podsumowuje wyniki. Postacie i talie są losowane z kart testowych silnika; raporty balansu
//! prawdziwej zawartości dojdą w kroku F3.5 planu.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::process::ExitCode;
use std::time::Instant;

use engine::testkit::{content, random_setup, sample_cards, seeded_rng};
use engine::{
    CardAction, Content, DuelState, Event, EventBuf, Kind, Move, Outcome, PlayerTotals, Reason,
    Resource, School, Side, Stat, new_duel, resolve_round, view,
};
use engine_ai::{Bot, Level};

const USAGE: &str = "Użycie:
  sim duel [ziarno] [--a L0|L1] [--b L0|L1]
      pojedynek botów opisany runda po rundzie (domyślnie ziarno 1, boty L1)
  sim run [--n liczba] [--seed ziarno] [--a L0|L1] [--b L0|L1]
      seria pojedynków z podsumowaniem (domyślnie 1000 pojedynków od ziarna 0, boty L1)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Duel {
        seed: u64,
        levels: [Level; 2],
    },
    Run {
        count: u64,
        seed: u64,
        levels: [Level; 2],
    },
}

fn parse_level(text: &str) -> Option<Level> {
    match text {
        "L0" | "l0" => Some(Level::L0),
        "L1" | "l1" => Some(Level::L1),
        _ => None,
    }
}

fn parse(args: &[String]) -> Option<Command> {
    let (command, mut rest) = args.split_first()?;
    let mut levels = [Level::L1; 2];
    let mut seed = None;
    let mut count = None;
    while let Some((flag, tail)) = rest.split_first() {
        if command == "duel" && seed.is_none() && !flag.starts_with("--") {
            seed = Some(flag.parse().ok()?);
            rest = tail;
            continue;
        }
        let value = tail.first()?;
        match flag.as_str() {
            "--a" => levels[0] = parse_level(value)?,
            "--b" => levels[1] = parse_level(value)?,
            "--seed" => seed = Some(value.parse().ok()?),
            "--n" if command == "run" => count = Some(value.parse().ok()?),
            _ => return None,
        }
        rest = &tail[1..];
    }
    match command.as_str() {
        "duel" => Some(Command::Duel {
            seed: seed.unwrap_or(1),
            levels,
        }),
        "run" => Some(Command::Run {
            count: count.unwrap_or(1_000),
            seed: seed.unwrap_or(0),
            levels,
        }),
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = parse(&args) else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let out = &mut io::stdout().lock();
    let result = match command {
        Command::Duel { seed, levels } => duel_report(seed, levels, out),
        Command::Run {
            count,
            seed,
            levels,
        } => run_report(count, seed, levels, out),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sim: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Pojedynek o ziarnie `seed`: losowe wejście z kart testowych i boty o poziomach `levels`.
struct Duel {
    state: DuelState,
    bots: [Bot; 2],
}

impl Duel {
    fn new(seed: u64, levels: [Level; 2], content: &Content<'_>) -> Self {
        let setup = random_setup(&mut seeded_rng(seed), content);
        let state = new_duel(&setup, content).expect("losowe wejście jest poprawne");
        // Każdy bot losuje z własnego strumienia.
        let bots = [0, 1].map(|index| Bot::new(levels[index], seed.wrapping_mul(2) + index as u64));
        Self { state, bots }
    }

    /// Rozgrywa rundę ruchami botów; zdarzenia trafiają do `events`.
    fn play_round(&mut self, content: &Content<'_>, events: &mut EventBuf) -> [PlayerTotals; 2] {
        let moves = Side::BOTH
            .map(|side| self.bots[side.index()].choose(&view(&self.state, side), content));
        resolve_round(&mut self.state, content, &moves, events)
            .expect("boty dają tylko poprawne ruchy")
            .totals
    }
}

const fn level_name(level: Level) -> &'static str {
    match level {
        Level::L0 => "L0",
        Level::L1 => "L1",
    }
}

/// Rozgrywa pojedynek botów i opisuje jego przebieg runda po rundzie.
fn duel_report(seed: u64, levels: [Level; 2], out: &mut impl Write) -> io::Result<()> {
    let cards = sample_cards();
    let content = content(&cards);
    let mut duel = Duel::new(seed, levels, &content);
    let mut events = EventBuf::new();

    writeln!(
        out,
        "Pojedynek testowy (ziarno {seed}): bot {} (A) przeciw botowi {} (B).",
        level_name(levels[0]),
        level_name(levels[1])
    )?;
    writeln!(
        out,
        "Losowe postacie i talie z kart testowych z efektami podstawowych rodzin i umagicznieniami."
    )?;
    writeln!(out)?;
    for side in Side::BOTH {
        writeln!(
            out,
            "Gracz {}: {}",
            name(side),
            describe_player(&duel.state, side)
        )?;
    }
    writeln!(out, "Inicjatywa: {}", name(duel.state.initiative()))?;

    while !duel.state.is_finished() {
        let focus = Side::BOTH.map(|side| duel.state.player(side).focus());
        writeln!(out)?;
        writeln!(
            out,
            "Runda {} (K na start: A {}, B {})",
            duel.state.round(),
            focus[0],
            focus[1]
        )?;
        events.clear();
        let [a, b] = duel.play_round(&content, &mut events);
        for event in events.as_slice() {
            describe_event(out, event)?;
        }
        writeln!(out, "  Koniec rundy: A {} | B {}", totals(a), totals(b))?;
    }
    if let Some(outcome) = duel.state.outcome() {
        writeln!(out)?;
        writeln!(out, "Wynik: {}", describe_outcome(outcome))?;
    }
    Ok(())
}

/// Wyniki serii pojedynków.
#[derive(Debug, Default)]
struct RunStats {
    duels: u64,
    wins: [u64; 2],
    draws: u64,
    reasons: BTreeMap<&'static str, u64>,
    rounds: u64,
    /// Pojedynki wygrane przez stronę z inicjatywą.
    initiative_wins: u64,
}

impl RunStats {
    fn record(&mut self, outcome: Outcome, initiative: Side, rounds: u64) {
        self.duels += 1;
        self.rounds += rounds;
        let reason = match outcome {
            Outcome::Win { winner, reason } => {
                self.wins[winner.index()] += 1;
                if winner == initiative {
                    self.initiative_wins += 1;
                }
                reason
            }
            Outcome::Draw { reason } => {
                self.draws += 1;
                reason
            }
        };
        *self.reasons.entry(reason_name(reason)).or_default() += 1;
    }
}

fn play_series(count: u64, seed: u64, levels: [Level; 2]) -> RunStats {
    let cards = sample_cards();
    let content = content(&cards);
    let mut events = EventBuf::new();
    let mut stats = RunStats::default();
    for index in 0..count {
        let mut duel = Duel::new(seed.wrapping_add(index), levels, &content);
        let mut rounds = 0;
        while !duel.state.is_finished() {
            events.clear();
            duel.play_round(&content, &mut events);
            rounds += 1;
        }
        let outcome = duel.state.outcome().expect("pojedynek się skończył");
        stats.record(outcome, duel.state.initiative(), rounds);
    }
    stats
}

/// Procent `part` z `whole` z jednym miejscem po przecinku.
fn percent(part: u64, whole: u64) -> String {
    if whole == 0 {
        return "0,0%".to_owned();
    }
    let tenths = (part * 1000 + whole / 2) / whole;
    format!("{},{}%", tenths / 10, tenths % 10)
}

/// Rozgrywa serię pojedynków botów i wypisuje podsumowanie.
fn run_report(count: u64, seed: u64, levels: [Level; 2], out: &mut impl Write) -> io::Result<()> {
    let started = Instant::now();
    let stats = play_series(count, seed, levels);
    let seconds = started.elapsed().as_secs_f64();
    let [a, b] = stats.wins;
    let duels = stats.duels;
    writeln!(
        out,
        "Seria {duels} pojedynków od ziarna {seed}: bot {} (A) przeciw botowi {} (B).",
        level_name(levels[0]),
        level_name(levels[1])
    )?;
    writeln!(out, "  Wygrane A: {a} ({})", percent(a, duels))?;
    writeln!(out, "  Wygrane B: {b} ({})", percent(b, duels))?;
    writeln!(
        out,
        "  Remisy: {} ({})",
        stats.draws,
        percent(stats.draws, duels)
    )?;
    writeln!(out, "  Powody zakończenia:")?;
    for (reason, &number) in &stats.reasons {
        writeln!(out, "    {reason}: {number} ({})", percent(number, duels))?;
    }
    let per_duel = |total: f64| {
        if duels == 0 {
            0.0
        } else {
            total / duels as f64
        }
    };
    writeln!(
        out,
        "  Średnia liczba rund: {:.2}",
        per_duel(stats.rounds as f64)
    )?;
    writeln!(
        out,
        "  Strona z inicjatywą wygrywa {} rozstrzygniętych pojedynków",
        percent(stats.initiative_wins, a + b)
    )?;
    writeln!(
        out,
        "  Czas: {seconds:.2} s ({:.1} µs na pojedynek)",
        per_duel(seconds * 1e6)
    )?;
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

fn stat_name(stat: Stat) -> &'static str {
    match stat {
        Stat::PhysAtk => "AF",
        Stat::PhysDef => "OF",
        Stat::MagAtk => "AM",
        Stat::MagDef => "OM",
        Stat::HpMax => "Ż_max",
        Stat::StaminaMax => "W_max",
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

const SCHOOLS: [School; 6] = [
    School::Combat,
    School::Divine,
    School::Element,
    School::Arcane,
    School::Necro,
    School::Tech,
];

fn school_name(school: School) -> &'static str {
    match school {
        School::Combat => "Walka",
        School::Divine => "Kapłaństwo",
        School::Element => "Elementalizm",
        School::Arcane => "Magia",
        School::Necro => "Nekromancja",
        School::Tech => "Technologia",
    }
}

fn describe_player(state: &DuelState, side: Side) -> String {
    let player = state.player(side);
    let stats = state.stats(side);
    let schools_now = state.schools(side);
    let schools: Vec<String> = SCHOOLS
        .iter()
        .filter(|school| schools_now.level(**school) > 0)
        .map(|&school| format!("{} {}", school_name(school), schools_now.level(school)))
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
        Event::StatChanged {
            side,
            stat,
            old,
            new,
            source,
        } => writeln!(
            out,
            "  {}: {} {old} → {new} (karta {source})",
            name(side),
            stat_name(stat)
        ),
        Event::ResourceChanged {
            side,
            resource,
            old,
            new,
            source,
        } => {
            let resource = match resource {
                Resource::Stamina => "W",
                Resource::Focus => "K",
            };
            writeln!(
                out,
                "  {}: {resource} {old} → {new} (karta {source})",
                name(side)
            )
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
        Event::AttackMissed {
            attacker,
            slot,
            kind,
        } => writeln!(
            out,
            "  {}{} {}: chybienie wymuszone efektem",
            name(attacker),
            slot + 1,
            letter(kind)
        ),
        Event::Healed {
            side,
            amount,
            source,
        } => writeln!(out, "  {}: +{amount} Ż (karta {source})", name(side)),
        Event::Damaged {
            side,
            amount,
            source,
        } => writeln!(
            out,
            "  {}: {amount} {} od czaru (karta {source})",
            name(side),
            plural(amount, "obrażenie", "obrażenia", "obrażeń")
        ),
        Event::ChoicesSwapped { side, source } => writeln!(
            out,
            "  {}: ataki zamienione z obronami (karta {source})",
            name(side)
        ),
        Event::AttackOverride {
            attacker,
            hit,
            source,
            ..
        } => writeln!(
            out,
            "  {}: wybrane ataki w tej rundzie {} (karta {source})",
            name(attacker),
            if hit { "trafiają" } else { "chybiają" }
        ),
        Event::DamageTakenChanged {
            side,
            percent,
            source,
        } => writeln!(
            out,
            "  {}: otrzymywane obrażenia {percent}% (karta {source})",
            name(side)
        ),
        Event::SchoolChanged {
            side,
            school,
            old,
            new,
            source,
        } => writeln!(
            out,
            "  {}: {} {old} → {new} (karta {source})",
            name(side),
            school_name(school)
        ),
        Event::SpellCountered {
            side,
            card,
            focus,
            stamina,
        } => writeln!(
            out,
            "  {}: karta {card} anulowana przeciwzaklęciem (−{focus} K, −{stamina} W)",
            name(side)
        ),
        Event::RandomSpell { side, card, drawn } => writeln!(
            out,
            "  {}: karta losowa {card} działa jak karta {drawn}",
            name(side)
        ),
        Event::SpellFailed { side, card } => writeln!(
            out,
            "  {}: warunek karty {card} niespełniony, brak efektu",
            name(side)
        ),
        Event::EnchantmentAdded { side, card } => {
            writeln!(out, "  {}: umagicznienie z karty {card} działa", name(side))
        }
        Event::EnchantmentRemoved { side, card, source } => writeln!(
            out,
            "  {}: umagicznienie z karty {card} rozproszone (karta {source})",
            name(side)
        ),
        Event::ForcedDiscard { side, card, source } => writeln!(
            out,
            "  {} traci kartę {card} z ręki (karta {source})",
            name(side)
        ),
        // Początek rundy i wynik opisuje `duel_report`.
        Event::RoundStarted { .. } | Event::DuelEnded { .. } => Ok(()),
    }
}

fn reason_name(reason: Reason) -> &'static str {
    match reason {
        Reason::Knockout => "nokaut",
        Reason::Rounds => "wyższy procent życia po ostatniej rundzie",
        Reason::Forfeit => "ucieczka przeciwnika",
        Reason::Disconnect => "rozłączenie przeciwnika",
        Reason::Timeout => "walkower",
        Reason::ConditionalWin => "wygrana warunkowa z efektu karty",
    }
}

fn describe_outcome(outcome: Outcome) -> String {
    match outcome {
        Outcome::Win { winner, reason } => {
            format!("wygrywa {} ({})", name(winner), reason_name(reason))
        }
        Outcome::Draw {
            reason: Reason::Knockout,
        } => "remis (obaj gracze bez życia)".to_owned(),
        Outcome::Draw {
            reason: Reason::ConditionalWin,
        } => "remis (obaj spełnili warunek wygranej)".to_owned(),
        Outcome::Draw { .. } => "remis (równy procent życia)".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use engine_ai::Level;

    use super::{Command, duel_report, parse, play_series, plural};

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(ToOwned::to_owned).collect()
    }

    #[test]
    fn duel_report_describes_every_duel_to_the_end() {
        for seed in 0..50 {
            let first = [Level::L0, Level::L1][seed as usize % 2];
            let mut out = Vec::new();
            duel_report(seed, [first, Level::L1], &mut out).expect("zapis do pamięci");
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
    fn series_counts_every_duel_once() {
        let stats = play_series(40, 7, [Level::L1, Level::L0]);
        assert_eq!(stats.duels, 40);
        assert_eq!(stats.wins[0] + stats.wins[1] + stats.draws, 40);
        assert_eq!(stats.reasons.values().sum::<u64>(), 40);
        assert!(stats.rounds >= 40);
    }

    #[test]
    fn arguments_are_parsed_with_defaults() {
        let l1 = [Level::L1; 2];
        assert_eq!(
            parse(&args("duel")),
            Some(Command::Duel {
                seed: 1,
                levels: l1
            })
        );
        assert_eq!(
            parse(&args("duel 5 --b L0")),
            Some(Command::Duel {
                seed: 5,
                levels: [Level::L1, Level::L0]
            })
        );
        assert_eq!(
            parse(&args("run --n 10 --a L0 --seed 3")),
            Some(Command::Run {
                count: 10,
                seed: 3,
                levels: [Level::L0, Level::L1]
            })
        );
        assert_eq!(parse(&args("run 5")), None);
        assert_eq!(parse(&args("duel --a L2")), None);
        assert_eq!(parse(&args("run --n")), None);
        assert_eq!(parse(&args("")), None);
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
