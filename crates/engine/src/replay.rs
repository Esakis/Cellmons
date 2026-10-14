//! Powtórki: zapis wejść pojedynku i ich odtwarzanie (`docs/06-silnik-gry.md`, rozdz. 9).

use std::fmt;

use arrayvec::ArrayVec;

use crate::card::Content;
use crate::end::{Termination, end_by};
use crate::event::EventBuf;
use crate::moves::Move;
use crate::round::{DuelError, resolve_round};
use crate::rules::MAX_ROUNDS;
use crate::setup::{DuelSetup, SetupError, new_duel};
use crate::state::DuelState;
use crate::types::Side;

/// Wersja silnika w zapisie powtórki.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    /// Czy ten silnik odtworzy powtórkę z wersji `recorded`. Zgodność jak w Cargo: dla wersji
    /// 0.x musi się zgadzać część minor, a od 1.0 część major. Zmiana zasad, która zmienia przebieg
    /// zapisanych pojedynków, podbija właśnie tę część.
    #[must_use]
    pub const fn can_replay(self, recorded: Version) -> bool {
        if self.major == 0 {
            recorded.major == 0 && recorded.minor == self.minor
        } else {
            recorded.major == self.major
        }
    }
}

/// Wersja tego silnika (z `Cargo.toml`).
pub const ENGINE_VERSION: Version = Version {
    major: parse_u16(env!("CARGO_PKG_VERSION_MAJOR")),
    minor: parse_u16(env!("CARGO_PKG_VERSION_MINOR")),
    patch: parse_u16(env!("CARGO_PKG_VERSION_PATCH")),
};

/// Liczba dziesiętna z części wersji; błędna wartość zatrzymuje kompilację.
const fn parse_u16(digits: &str) -> u16 {
    let bytes = digits.as_bytes();
    let mut value: u16 = 0;
    let mut index = 0;
    while index < bytes.len() {
        value = value * 10 + (bytes[index] - b'0') as u16;
        index += 1;
    }
    value
}

/// Zapis pojedynku: wejścia, z których silnik odtwarza cały przebieg.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRecord {
    pub engine_version: Version,
    pub content_version: [u8; 32],
    pub setup: DuelSetup,
    /// Ruchy stron A i B w kolejnych rundach.
    pub moves: ArrayVec<[Move; 2], { MAX_ROUNDS as usize }>,
    /// Zakończenie spoza rozgrywki po ostatnim zapisanym ruchu: przegrywająca strona i powód.
    pub end: Option<(Side, Termination)>,
}

impl ReplayRecord {
    /// Pusty zapis pojedynku z bieżącą wersją silnika i treści.
    #[must_use]
    pub fn new(setup: DuelSetup, content: &Content<'_>) -> Self {
        Self {
            engine_version: ENGINE_VERSION,
            content_version: *content.version(),
            setup,
            moves: ArrayVec::new(),
            end: None,
        }
    }
}

/// Powód, dla którego powtórki nie da się odtworzyć.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayError {
    /// Zapis z wersji silnika, której ten silnik nie odtwarza.
    EngineVersion(Version),
    /// Treść gry ma inną wersję niż zapisana.
    ContentVersion,
    Setup(SetupError),
    /// Błąd w rundzie o numerze `round`.
    Round {
        round: u8,
        error: DuelError,
    },
    /// Zakończenie zapisane po końcu pojedynku.
    End(DuelError),
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EngineVersion(version) => write!(
                f,
                "powtórka z niezgodnej wersji silnika {}.{}.{}",
                version.major, version.minor, version.patch
            ),
            Self::ContentVersion => f.write_str("powtórka z innej wersji treści gry"),
            Self::Setup(error) => write!(f, "niepoprawne ustawienia pojedynku: {error}"),
            Self::Round { round, error } => write!(f, "runda {round}: {error}"),
            Self::End(error) => write!(f, "zakończenie pojedynku: {error}"),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Odtwarza pojedynek z zapisu; zdarzenia trafiają do `events`.
pub fn replay(
    record: &ReplayRecord,
    content: &Content<'_>,
    events: &mut EventBuf,
) -> Result<DuelState, ReplayError> {
    if !ENGINE_VERSION.can_replay(record.engine_version) {
        return Err(ReplayError::EngineVersion(record.engine_version));
    }
    if content.version() != &record.content_version {
        return Err(ReplayError::ContentVersion);
    }
    let mut state = new_duel(&record.setup, content).map_err(ReplayError::Setup)?;
    for (round, moves) in (1u8..).zip(&record.moves) {
        resolve_round(&mut state, content, moves, events)
            .map_err(|error| ReplayError::Round { round, error })?;
    }
    if let Some((loser, termination)) = record.end {
        end_by(&mut state, loser, termination, events).map_err(ReplayError::End)?;
    }
    Ok(state)
}
