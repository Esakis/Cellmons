//! Podstawowe typy pojedynku (`docs/06-silnik-gry.md`, rozdz. 3).

/// Liczba slotów ataku i obrony w rundzie (R-RUN-04).
pub const SLOTS: usize = 3;

/// Najwyższy poziom biegłości w szkole: Arcymistrz (R-ENT-03).
pub const MAX_SCHOOL_LEVEL: u8 = 4;

/// Górna granica AF, OF, AM i OM w snapshocie postaci. W grze to pojedyncze dziesiątki; granica
/// wyklucza przepełnienie `i32` w obliczeniach obrażeń.
pub const MAX_STAT: i32 = 1_000;

/// Górna granica Ż i W w snapshocie postaci.
pub const MAX_POOL: i32 = 100_000;

/// Strona pojedynku: A to gracz 1, B to gracz 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

impl Side {
    /// Obie strony w stałej kolejności.
    pub const BOTH: [Side; 2] = [Side::A, Side::B];

    /// Przeciwnik.
    #[must_use]
    pub const fn other(self) -> Side {
        match self {
            Side::A => Side::B,
            Side::B => Side::A,
        }
    }

    /// Indeks w tablicach `[_; 2]` indeksowanych stroną.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Side::A => 0,
            Side::B => 1,
        }
    }
}

/// Typ ataku lub obrony: fizyczny (F) albo magiczny (M).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Phys,
    Mag,
}

/// Szkoły Esencji (R-ENT-02), w kolejności poziomów w [`Schools`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum School {
    Combat,
    Divine,
    Element,
    Arcane,
    Necro,
    Tech,
}

/// Poziomy biegłości 0..=4 w 6 szkołach (R-ENT-03), indeksowane przez [`School`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Schools(pub [u8; 6]);

impl Schools {
    /// Brak biegłości (albo brak wymagań karty).
    pub const NONE: Schools = Schools([0; 6]);

    #[must_use]
    pub const fn level(&self, school: School) -> u8 {
        self.0[school as usize]
    }

    /// Kopia z ustawionym poziomem szkoły.
    #[must_use]
    pub const fn with(mut self, school: School, level: u8) -> Self {
        self.0[school as usize] = level;
        self
    }

    /// Czy każdy poziom jest co najmniej taki jak w `required` (R-RUN-06, p. 3).
    #[must_use]
    pub fn meets(&self, required: &Schools) -> bool {
        self.0
            .iter()
            .zip(required.0)
            .all(|(have, need)| *have >= need)
    }

    pub(crate) fn is_valid(&self) -> bool {
        self.0.iter().all(|level| *level <= MAX_SCHOOL_LEVEL)
    }
}

/// Statystyki efektywne postaci (R-ENT-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub phys_atk: i32,
    pub phys_def: i32,
    pub mag_atk: i32,
    pub mag_def: i32,
    pub hp_max: i32,
    pub stamina_max: i32,
}

impl Stats {
    /// Atak danego typu: AF albo AM (R-ATK-04).
    #[must_use]
    pub const fn attack(&self, kind: Kind) -> i32 {
        match kind {
            Kind::Phys => self.phys_atk,
            Kind::Mag => self.mag_atk,
        }
    }

    /// Obrona danego typu: OF albo OM (R-ATK-04).
    #[must_use]
    pub const fn defense(&self, kind: Kind) -> i32 {
        match kind {
            Kind::Phys => self.phys_def,
            Kind::Mag => self.mag_def,
        }
    }
}
