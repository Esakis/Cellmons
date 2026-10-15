//! Koniec pojedynku (R-KON-01…05).

use std::cmp::Ordering;

use crate::card::Condition;
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
    /// Wygrana warunkowa z efektu karty (R-KON-06).
    ConditionalWin,
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

/// Simultaneous walkovers are draws; the server supplies the termination cause.
pub fn end_draw(
    state: &mut DuelState,
    termination: Termination,
    events: &mut EventBuf,
) -> Result<Outcome, DuelError> {
    if state.is_finished() {
        return Err(DuelError::Finished);
    }
    let outcome = Outcome::Draw {
        reason: termination.reason(),
    };
    state.phase = Phase::Finished(outcome);
    events.push(Event::DuelEnded { outcome });
    Ok(outcome)
}

/// Czy warunek karty jest spełniony dla strony `side` (R-CZA-08, R-KON-06).
pub(crate) fn holds(state: &DuelState, side: Side, condition: Condition) -> bool {
    match condition {
        Condition::RoundAtLeast(round) => state.round >= round,
        Condition::HpPercentAhead => {
            let (me, foe) = (state.player(side), state.player(side.other()));
            let ahead = compare_hp_percent([me.hp, foe.hp], [me.hp_start, foe.hp_start]);
            ahead
                == Outcome::Win {
                    winner: Side::A,
                    reason: Reason::Rounds,
                }
        }
        Condition::OpponentExhausted => state.player(side.other()).stamina == 0,
    }
}

/// Wygrane warunkowe zgłoszone w tej rundzie, sprawdzane przed nokautem (R-KON-06). Gdy warunek
/// spełniają obie strony, jest remis (Z-IWN).
pub(crate) fn conditional_win(state: &DuelState) -> Option<Outcome> {
    let won = Side::BOTH.map(|side| {
        state
            .win_checks
            .iter()
            .any(|&(owner, condition)| owner == side && holds(state, side, condition))
    });
    let reason = Reason::ConditionalWin;
    match won {
        [true, true] => Some(Outcome::Draw { reason }),
        [true, false] => Some(Outcome::Win {
            winner: Side::A,
            reason,
        }),
        [false, true] => Some(Outcome::Win {
            winner: Side::B,
            reason,
        }),
        [false, false] => None,
    }
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
