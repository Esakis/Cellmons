//! Koniec pojedynku (R-KON-01…05).

use std::cmp::Ordering;

use crate::event::{Event, EventBuf};
use crate::round::DuelError;
use crate::state::{DuelState, Phase};
use crate::types::Side;

/// Wynik pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Win { winner: Side, reason: Reason },
    Draw { reason: Reason },
}

/// Powód zakończenia pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Nokaut na końcu rundy (R-KON-01).
    Knockout,
    /// Procent życia po ostatniej rundzie (R-KON-02, R-KON-03).
    Rounds,
    /// Ucieczka (R-KON-04).
    Forfeit,
    /// Rozłączenie bez powrotu (R-KON-05).
    Disconnect,
    /// Walkower po kolejnych brakach ruchu (R-RUN-09).
    Timeout,
}

/// Zakończenie spoza rozgrywki, zgłaszane przez serwer. Czasy i liczniki z zasad
/// (`duel.reconnect_grace_s`, `duel.max_consecutive_timeouts`) pilnuje serwer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// Ucieczka (R-KON-04).
    Forfeit,
    /// Brak powrotu po rozłączeniu (R-KON-05).
    Disconnect,
    /// Kolejne braki ruchu (R-RUN-09).
    Timeout,
}

impl Termination {
    const fn reason(self) -> Reason {
        match self {
            Termination::Forfeit => Reason::Forfeit,
            Termination::Disconnect => Reason::Disconnect,
            Termination::Timeout => Reason::Timeout,
        }
    }
}

/// Kończy pojedynek przegraną strony `loser` z powodu ucieczki, rozłączenia albo walkowera
/// (R-KON-04, R-KON-05, R-RUN-09).
pub fn end_by(
    state: &mut DuelState,
    loser: Side,
    termination: Termination,
    events: &mut EventBuf,
) -> Result<Outcome, DuelError> {
    if state.is_finished() {
        return Err(DuelError::Finished);
    }
    let outcome = Outcome::Win {
        winner: loser.other(),
        reason: termination.reason(),
    };
    state.phase = Phase::Finished(outcome);
    events.push(Event::DuelEnded { outcome });
    Ok(outcome)
}

/// Nokaut sprawdzany raz, na końcu rundy (R-KON-01): obie strony z Ż ≤ 0 to remis.
pub(crate) fn knockout(state: &DuelState) -> Option<Outcome> {
    let reason = Reason::Knockout;
    match Side::BOTH.map(|side| state.player(side).hp <= 0) {
        [true, true] => Some(Outcome::Draw { reason }),
        [true, false] => Some(Outcome::Win {
            winner: Side::B,
            reason,
        }),
        [false, true] => Some(Outcome::Win {
            winner: Side::A,
            reason,
        }),
        [false, false] => None,
    }
}

/// Wynik po ostatniej rundzie: wyższy procent życia (R-KON-02).
pub(crate) fn by_hp_percent(state: &DuelState) -> Outcome {
    let [a, b] = &state.players;
    compare_hp_percent([a.hp, b.hp], [a.hp_start, b.hp_start])
}

/// Porównanie Ż / Ż_start bez zaokrągleń: A wygrywa, gdy Ż_A × Ż_start_B > Ż_B × Ż_start_A;
/// ujemne Ż liczy się jako 0, a równość daje remis (R-KON-03).
pub(crate) fn compare_hp_percent(hp: [i32; 2], hp_start: [i32; 2]) -> Outcome {
    let a = i64::from(hp[0].max(0)) * i64::from(hp_start[1]);
    let b = i64::from(hp[1].max(0)) * i64::from(hp_start[0]);
    let reason = Reason::Rounds;
    match a.cmp(&b) {
        Ordering::Greater => Outcome::Win {
            winner: Side::A,
            reason,
        },
        Ordering::Less => Outcome::Win {
            winner: Side::B,
            reason,
        },
        Ordering::Equal => Outcome::Draw { reason },
    }
}
