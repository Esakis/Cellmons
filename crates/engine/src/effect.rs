//! Efekty kart: rodziny sparametryzowane (`docs/03-tresc-gry.md`, rozdz. 5.3) i ich wykonanie
//! (`docs/06-silnik-gry.md`, rozdz. 6).

use std::num::NonZeroU32;

use arrayvec::ArrayVec;

use crate::card::{CardId, Condition};
use crate::enchant;
use crate::event::{Event, EventBuf};
use crate::rng;
use crate::state::{AttackOverride, DuelState};
use crate::types::{Kind, SLOTS, School, Side, Stat};

/// Najwięcej efektów na jednej karcie.
pub const MAX_EFFECTS: usize = 4;

/// Efekty karty, wykonywane po kolei.
pub type Effects = ArrayVec<Effect, MAX_EFFECTS>;

/// Na kogo działa efekt, z punktu widzenia rzucającego.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Caster,
    Opponent,
    Both,
}

/// Jak długo działa zmiana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    /// Do końca pojedynku (R-CZA-01).
    Duel,
    /// Do końca bieżącej rundy.
    Round,
    /// Póki działa umagicznienie, które ma ten efekt (tylko w umagicznieniach; w czarze zwykłym
    /// efekt nic nie robi). Działa od razu, także przy sprawdzaniu ruchów.
    WhileActive,
}

/// Wyzwalacz umagicznienia (`docs/06-silnik-gry.md`, rozdz. 6.3). Wyzwala go to, co dotyczy
/// właściciela umagicznienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// W fazie czarów każdej rundy, także w rundzie rzucenia (R-CZA-02, R-RUN-11).
    OnRound,
    /// Po każdym trafieniu właściciela.
    OnHit,
    /// Po każdym bloku właściciela.
    OnBlocked,
    /// Gdy właściciel otrzyma obrażenia (od ataku albo czaru).
    OnDamageTaken,
    /// Na koniec każdej rundy (R-RUN-13).
    OnRoundEnd,
}

/// Zasób gracza zmieniany przez efekt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resource {
    Stamina,
    Focus,
}

/// Zasób przenoszony przez wysysanie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pool {
    Hp,
    Stamina,
    Focus,
}

/// Pary statystyk zamieniane przez `swap_stats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatPair {
    /// AF ↔ AM.
    Attacks,
    /// OF ↔ OM.
    Defenses,
    /// AF ↔ AM i OF ↔ OM.
    Both,
}

/// Działanie na ręce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandAction {
    /// Odrzucenie losowej karty z ręki.
    DiscardRandom,
    /// Dobranie karty z wierzchu talii, najwyżej do `duel.hand_size` kart w ręce.
    Draw,
}

/// Ataki objęte efektem w bieżącej rundzie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttackFilter {
    /// Tylko ataki tego typu.
    pub kind: Option<Kind>,
    /// Tylko atak w tym slocie (0, 1 albo 2).
    pub slot: Option<u8>,
    /// Tylko pierwszy pasujący atak w rundzie (np. „pierwszy atak fizyczny nie trafia”).
    pub first_only: bool,
}

impl AttackFilter {
    pub(crate) fn matches(&self, slot: usize, kind: Kind) -> bool {
        self.kind.is_none_or(|only| only == kind)
            && self.slot.is_none_or(|only| usize::from(only) == slot)
    }
}

/// Efekt karty; nazwy rodzin jak w `docs/03-tresc-gry.md`, rozdz. 5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// `stat_mod`: zmiana statystyki o `amount`.
    StatMod {
        target: Target,
        stat: Stat,
        amount: i16,
        duration: Duration,
    },
    /// `stat_set`: ustawienie statystyki na `value` do końca pojedynku.
    StatSet {
        target: Target,
        stat: Stat,
        value: i16,
    },
    /// `swap_stats`: zamiana statystyk F ↔ M do końca pojedynku.
    SwapStats { target: Target, pair: StatPair },
    /// `heal`: leczenie; Ż nie rośnie ponad Ż_max (Z-ENG-3).
    Heal { target: Target, amount: u16 },
    /// `damage`: obrażenia od czaru, bez obrony, z mnożnikiem otrzymywanych obrażeń.
    Damage { target: Target, amount: u16 },
    /// `stamina`: zmiana W o `amount`. W nie spada poniżej 0 i nie rośnie ponad W_max (Z-ENG-5).
    Stamina { target: Target, amount: i16 },
    /// `focus`: zmiana K o `amount`. K nie spada poniżej 0, a limit z `duel.focus_cap` dotyczy
    /// tylko przyrostu na początku rundy (Z-ENG-4).
    Focus { target: Target, amount: i16 },
    /// `drain`: przeciwnik traci do `amount` zasobu, a rzucający zyskuje tyle, ile odebrano.
    Drain { pool: Pool, amount: u16 },
    /// `force_hit`: wybrane ataki strony trafiają w tej rundzie.
    ForceHit {
        target: Target,
        filter: AttackFilter,
    },
    /// `force_miss`: wybrane ataki strony chybiają w tej rundzie; chybienie ma pierwszeństwo
    /// przed wymuszonym trafieniem (Z-ENG-1).
    ForceMiss {
        target: Target,
        filter: AttackFilter,
    },
    /// `unblockable`: wybranych ataków strony nie da się zablokować w tej rundzie.
    Unblockable {
        target: Target,
        filter: AttackFilter,
    },
    /// `swap_choices`: zamiana ataków z obronami strony w tej rundzie.
    SwapChoices { target: Target },
    /// `damage_taken_mult`: mnożnik otrzymywanych obrażeń w procentach (200 to podwójne).
    DamageTakenMult {
        target: Target,
        percent: u16,
        duration: Duration,
    },
    /// `school_mod`: zmiana poziomu szkoły; poziom efektywny zostaje w zakresie 0..=4.
    SchoolMod {
        target: Target,
        school: School,
        amount: i8,
        duration: Duration,
    },
    /// `hand`: odrzucenie losowych kart albo dobranie kart.
    Hand {
        target: Target,
        action: HandAction,
        count: u8,
    },
    /// `dispel`: usunięcie umagicznień strony (R-CZA-02).
    Dispel { target: Target },
    /// Wygrana warunkowa: na końcu rundy rzucający wygrywa, jeśli warunek jest spełniony
    /// (R-KON-06; obie strony naraz to remis, Z-IWN).
    ConditionalWin { condition: Condition },
}

impl Effect {
    /// Efekt z wartościami liczbowymi pomnożonymi przez `factor` (sprzężenie, R-CZA-06).
    pub(crate) fn scaled(self, factor: i16) -> Effect {
        let times = |amount: i16| amount.saturating_mul(factor);
        let times_u = |amount: u16| amount.saturating_mul(factor.unsigned_abs());
        match self {
            Effect::StatMod {
                target,
                stat,
                amount,
                duration,
            } => Effect::StatMod {
                target,
                stat,
                amount: times(amount),
                duration,
            },
            Effect::Heal { target, amount } => Effect::Heal {
                target,
                amount: times_u(amount),
            },
            Effect::Damage { target, amount } => Effect::Damage {
                target,
                amount: times_u(amount),
            },
            Effect::Stamina { target, amount } => Effect::Stamina {
                target,
                amount: times(amount),
            },
            Effect::Focus { target, amount } => Effect::Focus {
                target,
                amount: times(amount),
            },
            Effect::Drain { pool, amount } => Effect::Drain {
                pool,
                amount: times_u(amount),
            },
            other => other,
        }
    }

    /// Efekt ciągły umagicznienia: działa, póki umagicznienie jest aktywne, a nie przy wyzwoleniu.
    pub(crate) const fn is_continuous(&self) -> bool {
        matches!(
            self,
            Effect::StatMod {
                duration: Duration::WhileActive,
                ..
            } | Effect::SchoolMod {
                duration: Duration::WhileActive,
                ..
            } | Effect::DamageTakenMult {
                duration: Duration::WhileActive,
                ..
            }
        )
    }
}

/// Strony, na które działa efekt strony `caster`: najpierw rzucający, potem przeciwnik.
pub(crate) fn sides(caster: Side, target: Target) -> impl Iterator<Item = Side> {
    let (first, second) = match target {
        Target::Caster => (caster, None),
        Target::Opponent => (caster.other(), None),
        Target::Both => (caster, Some(caster.other())),
    };
    std::iter::once(first).chain(second)
}

/// Wykonanie efektów karty strony `caster`.
pub(crate) struct EffectCtx<'a> {
    pub(crate) state: &'a mut DuelState,
    pub(crate) events: &'a mut EventBuf,
    pub(crate) caster: Side,
    /// Karta, z której pochodzą efekty (pole `source` zdarzeń).
    pub(crate) source: CardId,
    /// Mnożnik wartości efektów: poziom szkoły przy sprzężeniu (R-CZA-06), inaczej 1.
    pub(crate) scale: i16,
    /// Czy efekty mogą wyzwalać umagicznienia. Efekty samych umagicznień nie wyzwalają kolejnych,
    /// co wyklucza nieskończone łańcuchy (Z-ENG-6).
    pub(crate) triggers: bool,
}

impl EffectCtx<'_> {
    pub(crate) fn apply(&mut self, effect: &Effect) {
        let caster = self.caster;
        if effect.is_continuous() {
            return;
        }
        match effect.scaled(self.scale) {
            Effect::StatMod {
                target,
                stat,
                amount,
                duration,
            } => {
                for side in sides(caster, target) {
                    self.modify_stat(side, stat, amount, duration);
                }
            }
            Effect::StatSet {
                target,
                stat,
                value,
            } => {
                for side in sides(caster, target) {
                    self.set_stat(side, stat, i32::from(value));
                }
            }
            Effect::SwapStats { target, pair } => {
                for side in sides(caster, target) {
                    self.swap_stats(side, pair);
                }
            }
            Effect::Heal { target, amount } => {
                for side in sides(caster, target) {
                    self.heal(side, i32::from(amount));
                }
            }
            Effect::Damage { target, amount } => {
                for side in sides(caster, target) {
                    self.damage(side, i32::from(amount));
                }
            }
            Effect::Stamina { target, amount } => {
                for side in sides(caster, target) {
                    self.change_resource(side, Resource::Stamina, i32::from(amount));
                }
            }
            Effect::Focus { target, amount } => {
                for side in sides(caster, target) {
                    self.change_resource(side, Resource::Focus, i32::from(amount));
                }
            }
            Effect::Drain { pool, amount } => self.drain(pool, i32::from(amount)),
            Effect::ForceHit { target, filter } | Effect::Unblockable { target, filter } => {
                for side in sides(caster, target) {
                    self.add_override(side, true, filter);
                }
            }
            Effect::ForceMiss { target, filter } => {
                for side in sides(caster, target) {
                    self.add_override(side, false, filter);
                }
            }
            Effect::SwapChoices { target } => {
                for side in sides(caster, target) {
                    let player = self.state.player_mut(side);
                    std::mem::swap(&mut player.attacks, &mut player.defenses);
                    self.events.push(Event::ChoicesSwapped {
                        side,
                        source: self.source,
                    });
                }
            }
            Effect::DamageTakenMult {
                target,
                percent,
                duration,
            } => {
                for side in sides(caster, target) {
                    self.multiply_damage_taken(side, percent, duration);
                }
            }
            Effect::SchoolMod {
                target,
                school,
                amount,
                duration,
            } => {
                for side in sides(caster, target) {
                    self.modify_school(side, school, amount, duration);
                }
            }
            Effect::Hand {
                target,
                action,
                count,
            } => {
                for side in sides(caster, target) {
                    for _ in 0..count {
                        self.hand(side, action);
                    }
                }
            }
            Effect::Dispel { target } => {
                for side in sides(caster, target) {
                    enchant::dispel(self.state, self.events, side, self.source);
                }
            }
            Effect::ConditionalWin { condition } => {
                // Numer rundy nie zmienia się przed sprawdzeniem końca. Nie rejestrujemy
                // niemożliwych warunków, a duplikaty nie zmieniają wyniku (Z-IWN).
                if matches!(condition, Condition::RoundAtLeast(round) if round > self.state.round) {
                    return;
                }
                let check = (
                    caster,
                    match condition {
                        Condition::RoundAtLeast(_) => Condition::RoundAtLeast(self.state.round),
                        other => other,
                    },
                );
                if !self.state.win_checks.contains(&check) {
                    self.state.win_checks.push(check);
                }
            }
        }
    }

    fn modify_stat(&mut self, side: Side, stat: Stat, amount: i16, duration: Duration) {
        let old = self.state.stats(side).get(stat);
        let player = self.state.player_mut(side);
        let layer = match duration {
            Duration::Duel => &mut player.permanent,
            Duration::Round => &mut player.round_delta,
            Duration::WhileActive => return,
        };
        let delta = &mut layer[stat as usize];
        *delta = delta.saturating_add(i32::from(amount));
        self.stat_changed(side, stat, old);
    }

    /// Statystyka efektywna równa `value` do końca pojedynku; znika też zmiana z bieżącej rundy.
    fn set_stat(&mut self, side: Side, stat: Stat, value: i32) {
        let old = self.state.stats(side).get(stat);
        let bonus = enchant::stat_bonus(self.state, side, stat);
        let player = self.state.player_mut(side);
        let index = stat as usize;
        player.permanent[index] = value
            .saturating_sub(player.base.get(stat))
            .saturating_sub(bonus);
        player.round_delta[index] = 0;
        self.stat_changed(side, stat, old);
    }

    fn swap_stats(&mut self, side: Side, pair: StatPair) {
        let pairs: &[(Stat, Stat)] = match pair {
            StatPair::Attacks => &[(Stat::PhysAtk, Stat::MagAtk)],
            StatPair::Defenses => &[(Stat::PhysDef, Stat::MagDef)],
            StatPair::Both => &[(Stat::PhysAtk, Stat::MagAtk), (Stat::PhysDef, Stat::MagDef)],
        };
        for &(first, second) in pairs {
            let stats = self.state.stats(side);
            let (first_value, second_value) = (stats.get(first), stats.get(second));
            self.set_stat(side, first, second_value);
            self.set_stat(side, second, first_value);
        }
    }

    fn stat_changed(&mut self, side: Side, stat: Stat, old: i32) {
        let new = self.state.stats(side).get(stat);
        self.events.push(Event::StatChanged {
            side,
            stat,
            old,
            new,
            source: self.source,
        });
    }

    /// Leczy o `amount`; Ż nie przekracza Ż_max, ale Ż już wyższe zostaje (Z-ENG-3).
    fn heal(&mut self, side: Side, amount: i32) {
        let cap = self.state.stats(side).hp_max;
        let player = self.state.player_mut(side);
        let old = player.hp;
        player.hp = old.saturating_add(amount.max(0)).min(cap.max(old));
        let healed = player.hp - old;
        self.events.push(Event::Healed {
            side,
            amount: healed,
            source: self.source,
        });
    }

    /// Zadaje obrażenia od czaru (bez obrony) i zwraca ich faktyczną wartość.
    fn damage(&mut self, side: Side, amount: i32) -> i32 {
        let dealt = self.state.scale_damage(side, amount.max(0));
        let player = self.state.player_mut(side);
        player.hp = player.hp.saturating_sub(dealt);
        self.events.push(Event::Damaged {
            side,
            amount: dealt,
            source: self.source,
        });
        if self.triggers && dealt > 0 {
            enchant::fire(self.state, self.events, Trigger::OnDamageTaken, Some(side));
        }
        dealt
    }

    fn change_resource(&mut self, side: Side, resource: Resource, amount: i32) {
        let stamina_max = self.state.stats(side).stamina_max;
        let player = self.state.player_mut(side);
        let (value, cap) = match resource {
            Resource::Stamina => (&mut player.stamina, Some(stamina_max)),
            Resource::Focus => (&mut player.focus, None),
        };
        let old = *value;
        *value = add_bounded(old, amount, cap);
        let new = *value;
        self.events.push(Event::ResourceChanged {
            side,
            resource,
            old,
            new,
            source: self.source,
        });
    }

    fn drain(&mut self, pool: Pool, amount: i32) {
        let (caster, victim) = (self.caster, self.caster.other());
        let resource = match pool {
            Pool::Hp => {
                let dealt = self.damage(victim, amount);
                self.heal(caster, dealt);
                return;
            }
            Pool::Stamina => Resource::Stamina,
            Pool::Focus => Resource::Focus,
        };
        let available = match resource {
            Resource::Stamina => self.state.player(victim).stamina,
            Resource::Focus => self.state.player(victim).focus,
        };
        let taken = amount.clamp(0, available.max(0));
        self.change_resource(victim, resource, -taken);
        self.change_resource(caster, resource, taken);
    }

    /// Identyczne wymuszenia działają wspólnie, także przy `first_only`: zużywa je ten sam
    /// atak. Nowe wyzwolenie już zużytego wymuszenia uzbraja je ponownie.
    fn add_override(&mut self, attacker: Side, hit: bool, filter: AttackFilter) {
        if filter.slot.is_some_and(|slot| usize::from(slot) >= SLOTS) {
            return; // Taki filtr nigdy nie pasuje do żadnego ataku.
        }
        let added = AttackOverride {
            attacker,
            hit,
            filter,
            used: false,
        };
        if let Some(existing) = self.state.overrides.iter_mut().find(|forced| {
            forced.attacker == attacker && forced.hit == hit && forced.filter == filter
        }) {
            existing.used = false;
        } else {
            // MAX_OVERRIDES obejmuje wszystkie kombinacje strony, wyniku i filtra.
            self.state.overrides.push(added);
        }
        self.events.push(Event::AttackOverride {
            attacker,
            hit,
            filter,
            source: self.source,
        });
    }

    fn multiply_damage_taken(&mut self, side: Side, percent: u16, duration: Duration) {
        let player = self.state.player_mut(side);
        let layer = match duration {
            Duration::Duel => &mut player.damage_pct,
            Duration::Round => &mut player.round_damage_pct,
            Duration::WhileActive => return,
        };
        *layer = multiply_percent(*layer, u32::from(percent));
        let percent = self.state.damage_taken_pct(side);
        self.events.push(Event::DamageTakenChanged {
            side,
            percent,
            source: self.source,
        });
    }

    fn modify_school(&mut self, side: Side, school: School, amount: i8, duration: Duration) {
        let old = self.state.schools(side).level(school);
        let player = self.state.player_mut(side);
        let layer = match duration {
            Duration::Duel => &mut player.school_permanent,
            Duration::Round => &mut player.school_round,
            Duration::WhileActive => return,
        };
        let delta = &mut layer[school as usize];
        *delta = delta.saturating_add(amount);
        let new = self.state.schools(side).level(school);
        self.events.push(Event::SchoolChanged {
            side,
            school,
            old,
            new,
            source: self.source,
        });
    }

    fn hand(&mut self, side: Side, action: HandAction) {
        match action {
            HandAction::DiscardRandom => {
                let held = self.state.player(side).hand.len();
                let Some(bound) = u32::try_from(held).ok().and_then(NonZeroU32::new) else {
                    return;
                };
                let index = rng::below(&mut self.state.rng, bound) as usize;
                let player = self.state.player_mut(side);
                if let Some(card) = player.hand.pop_at(index) {
                    player.discard.push(card);
                    self.events.push(Event::ForcedDiscard {
                        side,
                        card,
                        source: self.source,
                    });
                }
            }
            HandAction::Draw => {
                let limit = usize::from(self.state.rules.duel.hand_size);
                let player = self.state.player_mut(side);
                if player.hand.len() >= limit {
                    return;
                }
                if let Some(card) = player.deck.pop() {
                    player.hand.push(card);
                    self.events.push(Event::CardDrawn { side, card });
                }
            }
        }
    }
}

/// `value + amount`, nie mniej niż 0. Wzrost nie przekracza `cap`, ale wartość już wyższa od `cap`
/// zostaje.
fn add_bounded(value: i32, amount: i32, cap: Option<i32>) -> i32 {
    let raised = value.saturating_add(amount).max(0);
    match cap {
        Some(cap) if amount > 0 => raised.min(cap.max(value)),
        _ => raised,
    }
}

/// Najwyższy mnożnik otrzymywanych obrażeń w procentach (×100).
pub(crate) const MAX_DAMAGE_PERCENT: u32 = 10_000;

/// Złożenie mnożników w procentach, np. 200 % i 200 % to 400 %.
pub(crate) fn multiply_percent(percent: u32, factor: u32) -> u32 {
    let product = u64::from(percent) * u64::from(factor) / 100;
    u32::try_from(product)
        .unwrap_or(MAX_DAMAGE_PERCENT)
        .min(MAX_DAMAGE_PERCENT)
}
