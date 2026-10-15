//! Stan pojedynku (`docs/06-silnik-gry.md`, rozdz. 3).

use arrayvec::ArrayVec;
use rand_chacha::ChaCha8Rng;

use crate::card::{CardId, Condition};
use crate::effect::{AttackFilter, multiply_percent};
use crate::enchant::{self, ActiveEnchantment, MAX_ENCHANTMENTS};
use crate::end::Outcome;
use crate::rules::{MAX_DECK_SIZE, MAX_HAND_SIZE, MAX_ROUNDS, RulesConfig};
use crate::types::{
    Kind, MAX_SCHOOL_LEVEL, SCHOOLS, SLOTS, Schools, Side, Specialization, Stat, Stats,
};

/// Najwięcej wymuszonych wyników ataków w jednej rundzie.
pub(crate) const MAX_OVERRIDES: usize = 32;

/// Najwięcej wygranych warunkowych zgłoszonych w jednej rundzie.
pub(crate) const MAX_WIN_CHECKS: usize = 16;

/// Mnożnik otrzymywanych obrażeń bez efektów, w procentach.
pub(crate) const NORMAL_DAMAGE_PERCENT: u32 = 100;

/// Faza pojedynku.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Gracze wybierają ruchy na bieżącą rundę.
    AwaitingMoves,
    /// Pojedynek zakończony; stan już się nie zmienia.
    Finished(Outcome),
}

/// Stan jednego gracza.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    /// Statystyki ze snapshotu postaci (R-POJ-01).
    pub(crate) base: Stats,
    /// Zmiany statystyk do końca pojedynku (R-CZA-01), w kolejności `Stat::ALL`.
    pub(crate) permanent: [i32; 6],
    /// Zmiany statystyk do końca bieżącej rundy, w kolejności `Stat::ALL`.
    pub(crate) round_delta: [i32; 6],
    /// Poziomy szkół ze snapshotu postaci (R-POJ-01).
    pub(crate) base_schools: Schools,
    /// Specjalizacja ze snapshotu postaci (R-CZA-09).
    pub(crate) specialization: Option<Specialization>,
    /// Zmiany poziomów szkół do końca pojedynku i do końca rundy, w kolejności `School`.
    pub(crate) school_permanent: [i8; 6],
    pub(crate) school_round: [i8; 6],
    /// Mnożniki otrzymywanych obrażeń w procentach: do końca pojedynku i do końca rundy.
    pub(crate) damage_pct: u32,
    pub(crate) round_damage_pct: u32,
    pub(crate) hp: i32,
    pub(crate) hp_start: i32,
    pub(crate) stamina: i32,
    pub(crate) focus: i32,
    pub(crate) attacks: [Kind; SLOTS],
    pub(crate) defenses: [Kind; SLOTS],
    pub(crate) hand: ArrayVec<CardId, MAX_HAND_SIZE>,
    pub(crate) deck: ArrayVec<CardId, MAX_DECK_SIZE>,
    pub(crate) discard: ArrayVec<CardId, MAX_DECK_SIZE>,
    pub(crate) used_interventions: ArrayVec<CardId, { MAX_ROUNDS as usize }>,
}

impl PlayerState {
    /// Statystyki ze snapshotu postaci; wartości efektywne podaje `DuelState::stats`.
    #[must_use]
    pub const fn base_stats(&self) -> &Stats {
        &self.base
    }

    /// Specjalizacja postaci (R-CZA-09).
    #[must_use]
    pub const fn specialization(&self) -> Option<Specialization> {
        self.specialization
    }

    /// Poziomy szkół ze snapshotu postaci; poziomy efektywne podaje `DuelState::schools`.
    #[must_use]
    pub const fn base_schools(&self) -> &Schools {
        &self.base_schools
    }

    /// Punkty życia; mogą być ujemne (R-ATK-07).
    #[must_use]
    pub const fn hp(&self) -> i32 {
        self.hp
    }

    /// Ż w chwili startu (R-POJ-02), podstawa procentu życia (R-KON-03).
    #[must_use]
    pub const fn hp_start(&self) -> i32 {
        self.hp_start
    }

    #[must_use]
    pub const fn stamina(&self) -> i32 {
        self.stamina
    }

    #[must_use]
    pub const fn focus(&self) -> i32 {
        self.focus
    }

    /// Ataki z poprzedniej rundy, przed 1. rundą ustawienia domyślne (R-POJ-06).
    #[must_use]
    pub const fn attacks(&self) -> &[Kind; SLOTS] {
        &self.attacks
    }

    /// Obrony z poprzedniej rundy, przed 1. rundą ustawienia domyślne (R-POJ-06).
    #[must_use]
    pub const fn defenses(&self) -> &[Kind; SLOTS] {
        &self.defenses
    }

    /// Karty w ręce; indeksy jak w `CardAction`.
    #[must_use]
    pub fn hand(&self) -> &[CardId] {
        &self.hand
    }

    /// Pozostałe karty talii; wierzch na końcu.
    #[must_use]
    pub fn deck(&self) -> &[CardId] {
        &self.deck
    }

    /// Karty rzucone i odrzucone.
    #[must_use]
    pub fn discard(&self) -> &[CardId] {
        &self.discard
    }

    /// Interwencje rzucone w tym pojedynku (R-CZA-03).
    #[must_use]
    pub fn used_interventions(&self) -> &[CardId] {
        &self.used_interventions
    }
}

/// Wymuszony wynik ataków strony `attacker` w bieżącej rundzie (`force_hit`, `force_miss`,
/// `unblockable`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttackOverride {
    pub(crate) attacker: Side,
    /// `true`: trafienie, `false`: chybienie.
    pub(crate) hit: bool,
    pub(crate) filter: AttackFilter,
    /// Efekt „tylko pierwszy atak” już zadziałał.
    pub(crate) used: bool,
}

/// Stan pojedynku.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuelState {
    pub(crate) rules: RulesConfig,
    pub(crate) round: u8,
    pub(crate) phase: Phase,
    pub(crate) initiative: Side,
    pub(crate) players: [PlayerState; 2],
    /// Wymuszone wyniki ataków w bieżącej rundzie.
    pub(crate) overrides: ArrayVec<AttackOverride, MAX_OVERRIDES>,
    /// Strumień losowości efektów kart (`docs/06-silnik-gry.md`, rozdz. 7).
    pub(crate) rng: ChaCha8Rng,
    /// Aktywne umagicznienia w kolejności rzucenia (R-CZA-02).
    pub(crate) enchantments: ArrayVec<ActiveEnchantment, MAX_ENCHANTMENTS>,
    /// Numer kolejnego rzuconego umagicznienia.
    pub(crate) next_order: u16,
    /// Wygrane warunkowe do sprawdzenia na końcu bieżącej rundy (R-KON-06).
    pub(crate) win_checks: ArrayVec<(Side, Condition), MAX_WIN_CHECKS>,
}

impl DuelState {
    #[must_use]
    pub const fn rules(&self) -> &RulesConfig {
        &self.rules
    }

    /// Bieżąca runda, od 1. Po zakończeniu pojedynku: runda, w której się zakończył; przy
    /// ucieczce, rozłączeniu i walkowerze (`end_by`) to runda, na którą czekano na ruchy.
    #[must_use]
    pub const fn round(&self) -> u8 {
        self.round
    }

    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<Outcome> {
        match self.phase {
            Phase::AwaitingMoves => None,
            Phase::Finished(outcome) => Some(outcome),
        }
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        matches!(self.phase, Phase::Finished(_))
    }

    /// Strona z inicjatywą (R-POJ-05).
    #[must_use]
    pub const fn initiative(&self) -> Side {
        self.initiative
    }

    /// Kolejność rozpatrywania czarów i ataków: najpierw strona z inicjatywą (R-POJ-05).
    #[must_use]
    pub const fn initiative_order(&self) -> [Side; 2] {
        [self.initiative, self.initiative.other()]
    }

    #[must_use]
    pub const fn player(&self, side: Side) -> &PlayerState {
        &self.players[side.index()]
    }

    /// Statystyki efektywne strony: snapshot plus zmiany trwałe i zmiany z bieżącej rundy, nie
    /// mniejsze niż `Stat::floor` (Z-ENG-2).
    #[must_use]
    pub fn stats(&self, side: Side) -> Stats {
        let player = self.player(side);
        Stats::from_values(Stat::ALL.map(|stat| {
            let index = stat as usize;
            player
                .base
                .get(stat)
                .saturating_add(player.permanent[index])
                .saturating_add(player.round_delta[index])
                .saturating_add(enchant::stat_bonus(self, side, stat))
                .max(stat.floor())
        }))
    }

    /// Aktywne umagicznienia w kolejności rzucenia: właściciel i karta (lista jawna, R-CZA-02).
    pub fn enchantments(&self) -> impl Iterator<Item = (Side, CardId)> + '_ {
        self.enchantments
            .iter()
            .map(|active| (active.owner, active.card))
    }

    /// Poziomy szkół efektywne: snapshot plus zmiany, w zakresie 0..=4.
    #[must_use]
    pub fn schools(&self, side: Side) -> Schools {
        let player = self.player(side);
        let mut schools = player.base_schools;
        for (index, level) in schools.0.iter_mut().enumerate() {
            let changed = i16::from(*level)
                + i16::from(player.school_permanent[index])
                + i16::from(player.school_round[index])
                + enchant::school_bonus(self, side, SCHOOLS[index]);
            *level = changed.clamp(0, i16::from(MAX_SCHOOL_LEVEL)) as u8;
        }
        schools
    }

    /// Mnożnik obrażeń otrzymywanych przez stronę, w procentach.
    #[must_use]
    pub fn damage_taken_pct(&self, side: Side) -> u32 {
        let player = self.player(side);
        let percent = multiply_percent(player.damage_pct, player.round_damage_pct);
        multiply_percent(percent, enchant::damage_percent(self, side))
    }

    /// Obrażenia po uwzględnieniu mnożnika strony `side`, zaokrąglone w dół.
    pub(crate) fn scale_damage(&self, side: Side, damage: i32) -> i32 {
        let scaled = i64::from(damage) * i64::from(self.damage_taken_pct(side)) / 100;
        i32::try_from(scaled).unwrap_or(i32::MAX)
    }

    /// Wymuszony wynik ataku: `Some(false)` to chybienie, które ma pierwszeństwo przed trafieniem
    /// (Z-ENG-1), `Some(true)` trafienie mimo obrony. Pasujące efekty „tylko pierwszy atak” się
    /// zużywają.
    pub(crate) fn take_override(
        &mut self,
        attacker: Side,
        slot: usize,
        kind: Kind,
    ) -> Option<bool> {
        let (mut hit, mut miss) = (false, false);
        for forced in &mut self.overrides {
            if forced.attacker != attacker || forced.used || !forced.filter.matches(slot, kind) {
                continue;
            }
            forced.used = forced.filter.first_only;
            if forced.hit {
                hit = true;
            } else {
                miss = true;
            }
        }
        if miss {
            Some(false)
        } else if hit {
            Some(true)
        } else {
            None
        }
    }

    pub(crate) fn player_mut(&mut self, side: Side) -> &mut PlayerState {
        &mut self.players[side.index()]
    }

    /// Strona i jej przeciwnik jednocześnie do zmiany.
    pub(crate) fn pair_mut(&mut self, side: Side) -> (&mut PlayerState, &mut PlayerState) {
        let [a, b] = &mut self.players;
        match side {
            Side::A => (a, b),
            Side::B => (b, a),
        }
    }
}
