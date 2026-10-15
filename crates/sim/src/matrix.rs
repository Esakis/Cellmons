//! `sim matrix`: każda postać startowa przeciw każdej, z talią startową, na botach
//! (`docs/11-ai-i-symulacje.md`, rozdz. 6; kryterium kroku F3.5: żadna postać > 60% wygranych).

use std::io::{self, Write};

use engine::{Content, DuelSetup, DuelState, EventBuf, Outcome, PlayerSetup, Side, new_duel};
use engine_ai::Level;
use engine_content::{AVATARS, STARTERS, StarterDef};

use crate::Duel;

/// Najwyższy dopuszczalny odsetek wygranych postaci startowej (kryterium F3.5).
pub const MAX_WIN_RATE: f64 = 0.60;

/// Wyniki jednej pary z punktu widzenia postaci z wiersza.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cell {
    pub wins: u64,
    pub draws: u64,
    pub losses: u64,
}

impl Cell {
    fn games(self) -> u64 {
        self.wins + self.draws + self.losses
    }

    /// Odsetek wygranych; remis liczy się za pół wygranej.
    #[allow(clippy::cast_precision_loss)]
    fn rate(self) -> f64 {
        if self.games() == 0 {
            return 0.0;
        }
        (self.wins as f64 + self.draws as f64 / 2.0) / self.games() as f64
    }

    fn add(&mut self, other: Cell) {
        self.wins += other.wins;
        self.draws += other.draws;
        self.losses += other.losses;
    }
}

/// Macierz wyników: `cells[i][j]` to wyniki postaci startowej `i` przeciw `j`.
#[derive(Debug, Clone)]
pub struct Matrix {
    pub names: Vec<&'static str>,
    pub cells: Vec<Vec<Cell>>,
}

impl Matrix {
    /// Wyniki postaci przeciw wszystkim pozostałym (bez pojedynków lustrzanych).
    #[must_use]
    pub fn total(&self, row: usize) -> Cell {
        let mut total = Cell::default();
        for (column, cell) in self.cells[row].iter().enumerate() {
            if column != row {
                total.add(*cell);
            }
        }
        total
    }
}

fn player(starter: &StarterDef, level: u8) -> PlayerSetup {
    let avatar = engine_content::avatar(starter.avatar).expect("postać startowa istnieje");
    PlayerSetup {
        avatar: avatar.snapshot(level),
        deck: starter.deck.iter().copied().collect(),
    }
}

/// Indeks postaci startowej o nazwie `name` (klucz bez `avatar.`).
pub fn starter_index(name: &str) -> Option<usize> {
    STARTERS
        .iter()
        .position(|starter| self::name(starter) == name)
}

/// Nazwa postaci startowej `index`.
pub fn starter_name(index: usize) -> &'static str {
    name(&STARTERS[index])
}

/// Stan początkowy pojedynku postaci startowych `pair` (A, B) na poziomie `level`.
pub fn starter_duel(
    pair: [usize; 2],
    level: u8,
    seed: [u8; 32],
    content: &Content<'_>,
) -> DuelState {
    let setup = DuelSetup {
        rules: engine_content::rules(),
        seed,
        players: pair.map(|index| player(&STARTERS[index], level)),
    };
    new_duel(&setup, content).expect("treść startowa jest poprawna")
}

fn name(starter: &StarterDef) -> &'static str {
    AVATARS
        .iter()
        .find(|avatar| avatar.id == starter.avatar)
        .map_or("?", |avatar| avatar.key.trim_start_matches("avatar."))
}

/// Ziarno pojedynku `game` pary (`row`, `column`).
pub fn seed(row: usize, column: usize, game: u64) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&game.to_le_bytes());
    bytes[8] = row as u8;
    bytes[9] = column as u8;
    bytes
}

/// Rozgrywa `games` pojedynków każdej pary postaci startowych na poziomie `level`. Postacie
/// zamieniają się stronami co pojedynek, żeby strona A ani B nie dawała przewagi.
#[must_use]
pub fn play_matrix(games: u64, level: u8, bot: Level) -> Matrix {
    let size = STARTERS.len();
    let cells = (0..size)
        .map(|row| {
            (0..size)
                .map(|column| play_pair(row, column, games, level, bot))
                .collect()
        })
        .collect();
    Matrix {
        names: STARTERS.iter().map(name).collect(),
        cells,
    }
}

/// Wyniki postaci startowej `row` przeciw `column`.
fn play_pair(row: usize, column: usize, games: u64, level: u8, bot: Level) -> Cell {
    let content = engine_content::content();
    let mut events = EventBuf::new();
    let mut cell = Cell::default();
    for game in 0..games {
        let swapped = game % 2 == 1;
        let (a, b) = if swapped {
            (column, row)
        } else {
            (row, column)
        };
        let state = starter_duel([a, b], level, seed(row, column, game), &content);
        let bot_seed = game.wrapping_mul(64) + (row * STARTERS.len() + column) as u64;
        let mut duel = Duel::from_state(state, [bot, bot], bot_seed);
        while !duel.state.is_finished() {
            events.clear();
            duel.play_round(&content, &mut events);
        }
        let mine = if swapped { Side::B } else { Side::A };
        match duel.state.outcome().expect("pojedynek się skończył") {
            Outcome::Win { winner, .. } if winner == mine => cell.wins += 1,
            Outcome::Win { .. } => cell.losses += 1,
            Outcome::Draw { .. } => cell.draws += 1,
        }
    }
    cell
}

fn percent(rate: f64) -> String {
    format!("{:.1}%", rate * 100.0).replace('.', ",")
}

/// Połowa szerokości 95% przedziału ufności odsetka (przybliżenie normalne).
#[allow(clippy::cast_precision_loss)]
fn margin(cell: Cell) -> f64 {
    let n = cell.games() as f64;
    if n == 0.0 {
        return 0.0;
    }
    let p = cell.rate();
    1.96 * (p * (1.0 - p) / n).sqrt()
}

/// Wypisuje macierz i podsumowanie; zwraca, czy kryterium F3.5 jest spełnione.
pub fn report(
    matrix: &Matrix,
    games: u64,
    level: u8,
    bot: Level,
    out: &mut impl Write,
) -> io::Result<bool> {
    let bot = match bot {
        Level::L0 => "L0",
        Level::L1 => "L1",
    };
    writeln!(
        out,
        "Macierz postaci startowych: poziom {level}, talie startowe, boty {bot}, {games} pojedynków na parę."
    )?;
    writeln!(
        out,
        "Wiersz: odsetek wygranych postaci z wiersza przeciw postaci z kolumny (remis = ½)."
    )?;
    writeln!(out)?;
    let width = matrix
        .names
        .iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0)
        .max(8);
    write!(out, "{:width$}", "")?;
    for name in &matrix.names {
        write!(out, " {:>14}", short(name))?;
    }
    writeln!(out, " {:>16}", "razem")?;
    let mut ok = true;
    for (row, name) in matrix.names.iter().enumerate() {
        write!(out, "{name:width$}")?;
        for (column, cell) in matrix.cells[row].iter().enumerate() {
            let text = if column == row {
                "—".to_owned()
            } else {
                percent(cell.rate())
            };
            write!(out, " {text:>14}")?;
        }
        let total = matrix.total(row);
        let flag = if total.rate() > MAX_WIN_RATE {
            ok = false;
            " !"
        } else {
            ""
        };
        writeln!(
            out,
            " {:>9} ± {:>4}{flag}",
            percent(total.rate()),
            percent(margin(total))
        )?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "Kryterium F3.5 (żadna postać powyżej {}): {}",
        percent(MAX_WIN_RATE),
        if ok { "spełnione" } else { "NIESPEŁNIONE" }
    )?;
    Ok(ok)
}

fn short(name: &str) -> String {
    name.chars().take(14).collect()
}

#[cfg(test)]
mod tests {
    use engine_ai::Level;

    use super::{play_matrix, report};

    #[test]
    fn every_pair_plays_the_requested_number_of_games() {
        let matrix = play_matrix(4, 1, Level::L1);
        assert_eq!(matrix.names.len(), 6);
        for row in &matrix.cells {
            assert!(
                row.iter()
                    .all(|cell| cell.wins + cell.draws + cell.losses == 4)
            );
        }
        // Wynik pary (i, j) jest lustrzany do (j, i) tylko co do sumy, bo ziarna są różne.
        let total: u64 = (0..6)
            .map(|row| matrix.total(row).wins + matrix.total(row).draws + matrix.total(row).losses)
            .sum();
        assert_eq!(total, 6 * 5 * 4);
        let mut out = Vec::new();
        report(&matrix, 4, 1, Level::L1, &mut out).expect("zapis do pamięci");
        assert!(
            String::from_utf8(out)
                .expect("UTF-8")
                .contains("Kryterium F3.5")
        );
    }
}
