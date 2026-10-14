//! Parametry zasad z `content/rules.toml` (`docs/02-zasady-gry.md`, rozdz. 13).

use std::fmt;

use serde::Deserialize;

use crate::types::{MAX_POOL, SLOTS};

/// Najwięcej rund, na ile pozwalają bufory silnika (np. ruchy w powtórce).
pub const MAX_ROUNDS: u8 = 9;

/// Największa ręka, na jaką pozwalają bufory silnika.
pub const MAX_HAND_SIZE: usize = 8;

/// Największa talia, na jaką pozwalają bufory silnika.
pub const MAX_DECK_SIZE: usize = 40;

/// Górna granica parametrów K (przyrost, limit, koszt zmiany).
const MAX_FOCUS_PARAM: i32 = 1_000;

/// Górna granica mnożnika serii.
const MAX_COMBO_MULTIPLIER: i32 = 64;

/// Górna granica limitów czasu w sekundach.
const MAX_TIMEOUT_S: u16 = 3_600;

/// Parametry zasad. Silnik czyta sekcje `[duel]` i `[deck]`; pozostałe sekcje pliku należą
/// do serwera i są tu pomijane.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RulesConfig {
    pub duel: DuelRules,
    pub deck: DeckRules,
}

/// Sekcja `[duel]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuelRules {
    /// Liczba rund (R-RUN).
    pub rounds: u8,
    /// Liczba slotów ataku i obrony (R-RUN-04); silnik obsługuje tylko [`SLOTS`].
    pub attacks_per_round: u8,
    /// Przyrost K na początku rundy (R-RUN-01).
    pub focus_per_round: i32,
    /// Limit K przy przyroście (R-RUN-02); `None` oznacza brak limitu.
    pub focus_cap: Option<i32>,
    /// Koszt K za każdy zmieniony slot (R-RUN-05).
    pub change_cost: i32,
    /// Liczba kart w ręce (R-POJ-04).
    pub hand_size: u8,
    /// Mnożniki obrażeń kolejnych trafień serii (R-ATK-04).
    pub combo_multipliers: [i32; SLOTS],
    /// Najmniejsza W obrońcy, przy której blok działa (R-ATK-02).
    pub block_min_stamina: i32,
    /// Ustawienia odniesienia dla kosztu zmian w 1. rundzie (R-POJ-06).
    pub default_settings: DefaultSettings,
    /// Rozstrzyganie przeciwzaklęć (R-CZA-05), od fazy F2.
    pub counter_resolution: CounterResolution,
    /// Czas na ruch po ruchu przeciwnika w sekundach (R-RUN-08); pilnuje go serwer.
    pub move_timeout_after_opponent_s: u16,
    /// Limit całej fazy decyzji w sekundach (R-RUN-08); pilnuje go serwer.
    pub round_timeout_s: u16,
    /// Liczba kolejnych braków ruchu, po której jest walkower (R-RUN-09); liczy je serwer.
    pub max_consecutive_timeouts: u8,
    /// Czas na powrót po rozłączeniu w sekundach (R-KON-05); pilnuje go serwer.
    pub reconnect_grace_s: u16,
}

/// Sekcja `[deck]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeckRules {
    /// Liczba kart w talii (R-TAL-01).
    pub size: u8,
    /// Najwięcej kopii tej samej karty (R-TAL-02).
    pub max_copies: u8,
    /// Czy brakujące karty uzupełnia się losowymi z puli (R-TAL-05).
    pub fill_missing_with_random: bool,
    /// Najwięcej zapisanych zestawów talii (R-TAL-06); używa go serwer.
    pub saved_presets_max: u8,
}

/// Wybór ustawień odniesienia (R-POJ-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultSettings {
    /// Wszystkie sloty typu z wyższą statystyką; przy remisie F (Z-DEF).
    HighestStat,
}

/// Rozstrzyganie przeciwzaklęć (R-CZA-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterResolution {
    /// W kolejności inicjatywy (Z-CNT).
    InitiativeFirst,
}

/// Parametr zasad poza zakresem, który obsługuje silnik.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RulesError {
    /// Klucz w `content/rules.toml`, np. `duel.rounds`.
    pub key: &'static str,
    pub min: i64,
    pub max: i64,
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parametr {} poza zakresem {}..={}",
            self.key, self.min, self.max
        )
    }
}

impl std::error::Error for RulesError {}

impl RulesConfig {
    /// Sprawdza zakresy parametrów, na których polega silnik.
    pub fn validate(&self) -> Result<(), RulesError> {
        let duel = &self.duel;
        let deck = &self.deck;
        let timeout = (1, i64::from(MAX_TIMEOUT_S));
        let focus = (0, i64::from(MAX_FOCUS_PARAM));

        check("deck.size", deck.size, (1, MAX_DECK_SIZE as i64))?;
        check(
            "deck.max_copies",
            deck.max_copies,
            (1, i64::from(deck.size)),
        )?;
        check(
            "deck.saved_presets_max",
            deck.saved_presets_max,
            (1, i64::from(u8::MAX)),
        )?;
        check("duel.rounds", duel.rounds, (1, i64::from(MAX_ROUNDS)))?;
        check(
            "duel.attacks_per_round",
            duel.attacks_per_round,
            (SLOTS as i64, SLOTS as i64),
        )?;
        check("duel.focus_per_round", duel.focus_per_round, focus)?;
        if let Some(cap) = duel.focus_cap {
            check("duel.focus_cap", cap, focus)?;
        }
        check("duel.change_cost", duel.change_cost, focus)?;
        let max_hand = MAX_HAND_SIZE.min(usize::from(deck.size));
        check("duel.hand_size", duel.hand_size, (1, max_hand as i64))?;
        for multiplier in duel.combo_multipliers {
            check(
                "duel.combo_multipliers",
                multiplier,
                (1, i64::from(MAX_COMBO_MULTIPLIER)),
            )?;
        }
        check(
            "duel.block_min_stamina",
            duel.block_min_stamina,
            (0, i64::from(MAX_POOL)),
        )?;
        check(
            "duel.move_timeout_after_opponent_s",
            duel.move_timeout_after_opponent_s,
            timeout,
        )?;
        check("duel.round_timeout_s", duel.round_timeout_s, timeout)?;
        check(
            "duel.max_consecutive_timeouts",
            duel.max_consecutive_timeouts,
            (1, 255),
        )?;
        check("duel.reconnect_grace_s", duel.reconnect_grace_s, timeout)?;
        Ok(())
    }
}

fn check(
    key: &'static str,
    value: impl Into<i64>,
    (min, max): (i64, i64),
) -> Result<(), RulesError> {
    if (min..=max).contains(&value.into()) {
        Ok(())
    } else {
        Err(RulesError { key, min, max })
    }
}
