//! Umagicznienia (R-CZA-02; `docs/06-silnik-gry.md`, rozdz. 6.3).

use arrayvec::ArrayVec;

use crate::card::{CardDef, CardId, CardKind};
use crate::effect::{Duration, Effect, EffectCtx, Effects, Trigger, multiply_percent, sides};
use crate::event::{Event, EventBuf};
use crate::rules::MAX_ROUNDS;
use crate::state::{DuelState, NORMAL_DAMAGE_PERCENT};
use crate::types::{School, Side, Stat};

/// Najwięcej aktywnych umagicznień: po jednym rzuceniu na stronę w każdej rundzie i dary obu stron.
pub const MAX_ENCHANTMENTS: usize = 2 * MAX_ROUNDS as usize + 2;

/// Aktywne umagicznienie. Lista jest jawna dla obu graczy (R-CZA-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveEnchantment {
    pub(crate) owner: Side,
    pub(crate) card: CardId,
    /// Kolejność rzucenia; przy tym samym wyzwalaczu umagicznienia działają rosnąco (R-CZA-02).
    pub(crate) order: u16,
    pub(crate) trigger: Trigger,
    pub(crate) effects: Effects,
}

/// Rejestruje umagicznienie z karty `def` strony `owner`, z efektami przemnożonymi przez `scale`
/// (sprzężenie, R-CZA-06); zwraca jego wyzwalacz albo `None`, gdy karta nie jest umagicznieniem.
pub(crate) fn register(
    state: &mut DuelState,
    events: &mut EventBuf,
    owner: Side,
    def: &CardDef,
    scale: i16,
) -> Option<Trigger> {
    let CardKind::Enchantment(trigger) = def.kind else {
        return None;
    };
    let order = state.next_order;
    state.next_order = state.next_order.saturating_add(1);
    let active = ActiveEnchantment {
        owner,
        card: def.id,
        order,
        trigger,
        effects: def
            .effects
            .iter()
            .map(|effect| effect.scaled(scale))
            .collect(),
    };
    // Pojemność wystarcza na wszystkie możliwe rzucenia i dary.
    if state.enchantments.try_push(active).is_ok() {
        events.push(Event::EnchantmentAdded {
            side: owner,
            card: def.id,
        });
    }
    Some(trigger)
}

/// Wyzwala umagicznienia z wyzwalaczem `trigger` w kolejności rzucenia: tylko strony `owner` albo
/// obu stron, gdy `owner` to `None`. Efekty umagicznień nie wyzwalają kolejnych (Z-ENG-6).
pub(crate) fn fire(
    state: &mut DuelState,
    events: &mut EventBuf,
    trigger: Trigger,
    owner: Option<Side>,
) {
    let pending: ArrayVec<u16, MAX_ENCHANTMENTS> = state
        .enchantments
        .iter()
        .filter(|active| active.trigger == trigger && owner.is_none_or(|side| side == active.owner))
        .map(|active| active.order)
        .collect();
    for order in pending {
        // Wcześniejszy efekt mógł to umagicznienie rozproszyć.
        let Some(active) = state
            .enchantments
            .iter()
            .find(|active| active.order == order)
        else {
            continue;
        };
        let (caster, source, effects) = (active.owner, active.card, active.effects.clone());
        let mut ctx = EffectCtx {
            state,
            events,
            caster,
            source,
            scale: 1,
            triggers: false,
        };
        for effect in &effects {
            ctx.apply(effect);
        }
    }
}

/// Usuwa umagicznienia strony `side` (rodzina `dispel`).
pub(crate) fn dispel(state: &mut DuelState, events: &mut EventBuf, side: Side, source: CardId) {
    for active in state
        .enchantments
        .iter()
        .filter(|active| active.owner == side)
    {
        events.push(Event::EnchantmentRemoved {
            side,
            card: active.card,
            source,
        });
    }
    state.enchantments.retain(|active| active.owner != side);
}

/// Ciągłe efekty aktywnych umagicznień działające na stronę `side`.
fn continuous(state: &DuelState, side: Side) -> impl Iterator<Item = &Effect> {
    state.enchantments.iter().flat_map(move |active| {
        active.effects.iter().filter(move |effect| {
            let target = match **effect {
                Effect::StatMod {
                    target,
                    duration: Duration::WhileActive,
                    ..
                }
                | Effect::SchoolMod {
                    target,
                    duration: Duration::WhileActive,
                    ..
                }
                | Effect::DamageTakenMult {
                    target,
                    duration: Duration::WhileActive,
                    ..
                } => target,
                _ => return false,
            };
            sides(active.owner, target).any(|affected| affected == side)
        })
    })
}

/// Suma ciągłych zmian statystyki `stat` strony `side`.
pub(crate) fn stat_bonus(state: &DuelState, side: Side, stat: Stat) -> i32 {
    continuous(state, side)
        .map(|effect| match *effect {
            Effect::StatMod {
                stat: changed,
                amount,
                ..
            } if changed == stat => i32::from(amount),
            _ => 0,
        })
        .fold(0, i32::saturating_add)
}

/// Suma ciągłych zmian poziomu szkoły `school` strony `side`.
pub(crate) fn school_bonus(state: &DuelState, side: Side, school: School) -> i16 {
    continuous(state, side)
        .map(|effect| match *effect {
            Effect::SchoolMod {
                school: changed,
                amount,
                ..
            } if changed == school => i16::from(amount),
            _ => 0,
        })
        .fold(0, i16::saturating_add)
}

/// Złożony ciągły mnożnik obrażeń otrzymywanych przez stronę `side`, w procentach.
pub(crate) fn damage_percent(state: &DuelState, side: Side) -> u32 {
    continuous(state, side).fold(NORMAL_DAMAGE_PERCENT, |percent, effect| match *effect {
        Effect::DamageTakenMult {
            percent: factor, ..
        } => multiply_percent(percent, u32::from(factor)),
        _ => percent,
    })
}
