//! Przebieg rundy (R-RUN-01…15, R-ATK-01…07).

use std::fmt;
use std::num::NonZeroU32;

use crate::card::{CardDef, CardId, Content};
use crate::effect::{EffectCtx, Trigger};
use crate::enchant;
use crate::end::{self, Outcome};
use crate::event::{Event, EventBuf};
use crate::moves::{CardAction, Move, MoveError, count_changes, spell_cost, validate_move};
use crate::rng;
use crate::state::{DuelState, NORMAL_DAMAGE_PERCENT, Phase};
use crate::types::{SLOTS, Side};

/// Błąd rozstrzygania rundy albo zakończenia pojedynku; stan się nie zmienia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuelError {
    /// Pojedynek jest już zakończony.
    Finished,
    /// Ruch strony jest niepoprawny.
    InvalidMove { side: Side, error: MoveError },
}

impl fmt::Display for DuelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Finished => f.write_str("pojedynek jest zakończony"),
            Self::InvalidMove { side, error } => write!(f, "ruch gracza {side:?}: {error}"),
        }
    }
}

impl std::error::Error for DuelError {}

/// Ż, W i K gracza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerTotals {
    pub hp: i32,
    pub stamina: i32,
    pub focus: i32,
}

/// Podsumowanie rozegranej rundy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundSummary {
    /// Numer rozegranej rundy.
    pub round: u8,
    /// Wartości stron A i B na koniec rundy, przed przyrostem K w następnej.
    pub totals: [PlayerTotals; 2],
    /// Wynik, jeśli pojedynek się zakończył.
    pub outcome: Option<Outcome>,
}

/// Rozstrzyga rundę dla ruchów stron A i B (`docs/06-silnik-gry.md`, rozdz. 5.1). Najpierw
/// sprawdza oba ruchy; przy błędzie stan zostaje bez zmian.
pub fn resolve_round(
    state: &mut DuelState,
    content: &Content<'_>,
    moves: &[Move; 2],
    events: &mut EventBuf,
) -> Result<RoundSummary, DuelError> {
    if state.is_finished() {
        return Err(DuelError::Finished);
    }
    for side in Side::BOTH {
        validate_move(state, content, side, &moves[side.index()])
            .map_err(|error| DuelError::InvalidMove { side, error })?;
    }
    let played = reveal(state, moves, events);
    spell_phase(state, content, &played, events);
    attack_phase(state, events);
    end_round_effects(state, events);
    draw_replacements(state, &played, events);
    Ok(finish_round(state, events))
}

/// Karta zagrana w tej rundzie. Schodzi z ręki przy ujawnieniu ruchów, więc efekty działające na
/// rękę nie mogą jej już zabrać.
#[derive(Debug, Clone, Copy)]
struct Played {
    card: CardId,
    /// Miejsce w ręce, na które trafi dobrana karta (R-RUN-14).
    index: usize,
    cast: bool,
}

/// Początek rundy: przyrost K (R-RUN-01, R-RUN-02).
pub(crate) fn begin_round(state: &mut DuelState) {
    let income = state.rules.duel.focus_per_round;
    let cap = state.rules.duel.focus_cap;
    for player in &mut state.players {
        let raised = player.focus + income;
        // Limit ogranicza tylko przyrost; K już ponad limitem zostaje.
        player.focus = cap.map_or(raised, |cap| raised.min(cap).max(player.focus));
    }
}

/// Ujawnienie ruchów: opłata za zmiany ustawień (R-RUN-05, R-RUN-10), zapis nowych ustawień
/// i zdjęcie zagranych kart z ręki. Zwraca zagrane karty stron A i B.
fn reveal(state: &mut DuelState, moves: &[Move; 2], events: &mut EventBuf) -> [Option<Played>; 2] {
    let change_cost = state.rules.duel.change_cost;
    let mut paid = [0; 2];
    let mut played = [None; 2];
    for side in Side::BOTH {
        let mv = &moves[side.index()];
        let player = state.player_mut(side);
        let cost = i32::from(count_changes(player, mv)) * change_cost;
        player.focus -= cost;
        player.attacks = mv.attacks;
        player.defenses = mv.defenses;
        paid[side.index()] = cost;
        let (index, cast) = match mv.card {
            CardAction::None => continue,
            CardAction::Cast(index) => (usize::from(index), true),
            CardAction::Discard(index) => (usize::from(index), false),
        };
        if let Some(card) = player.hand.pop_at(index) {
            played[side.index()] = Some(Played { card, index, cast });
        }
    }
    events.push(Event::MovesRevealed {
        moves: *moves,
        change_cost: paid,
    });
    played
}

/// Faza czarów (R-RUN-11): najpierw aktywne umagicznienia w kolejności rzucenia, potem
/// przeciwzaklęcia (R-CZA-05), a na końcu pozostałe rzucenia i odrzucenia, zawsze w kolejności
/// inicjatywy.
fn spell_phase(
    state: &mut DuelState,
    content: &Content<'_>,
    played: &[Option<Played>; 2],
    events: &mut EventBuf,
) {
    enchant::fire(state, events, Trigger::OnRound, None);
    let mut countered = [false; 2];
    let mut resolved = [false; 2];
    // Z-CNT: gdy obaj rzucają przeciwzaklęcia, działa tylko to gracza z inicjatywą.
    for side in state.initiative_order() {
        let Some(Played { card, .. }) = played[side.index()].filter(|played| played.cast) else {
            continue;
        };
        if !content.get(card).is_some_and(|def| def.keywords.counter) {
            continue;
        }
        resolved[side.index()] = true;
        state.player_mut(side).discard.push(card);
        if countered[side.index()] {
            cancel_spell(state, content, side, card, events);
        } else if cast_spell(state, content, side, card, events) {
            countered[side.other().index()] = true;
        }
    }
    for side in state.initiative_order() {
        let Some(Played { card, cast, .. }) = played[side.index()] else {
            continue;
        };
        if resolved[side.index()] {
            continue;
        }
        state.player_mut(side).discard.push(card);
        if !cast {
            events.push(Event::CardDiscarded { side, card });
        } else if countered[side.index()] {
            cancel_spell(state, content, side, card, events);
        } else {
            cast_spell(state, content, side, card, events);
        }
    }
}

/// Pobiera koszty, jeśli strona nadal może zapłacić (Z-SPW).
fn pay(state: &mut DuelState, side: Side, focus: i32, stamina: i32) -> bool {
    let player = state.player_mut(side);
    if player.focus < focus || player.stamina < stamina {
        return false;
    }
    player.focus -= focus;
    player.stamina -= stamina;
    true
}

/// Czar anulowany przeciwzaklęciem: rzucający płaci koszty, efektu nie ma (R-CZA-05).
fn cancel_spell(
    state: &mut DuelState,
    content: &Content<'_>,
    side: Side,
    card: CardId,
    events: &mut EventBuf,
) {
    let cost = content.get(card).map(|def| spell_cost(state, side, def));
    match cost {
        Some(cost) if pay(state, side, cost.focus, cost.stamina) => {
            events.push(Event::SpellCountered {
                side,
                card,
                focus: cost.focus,
                stamina: cost.stamina,
            });
        }
        _ => events.push(Event::SpellFizzled { side, card }),
    }
}

/// Rzucenie czaru (R-RUN-12): rzucający płaci W i K, o ile nadal może, a potem działa karta. Gdy
/// nie może zapłacić (efekt przeciwnika zabrał mu K lub W), czar przepada bez kosztów (Z-SPW).
/// Zwraca, czy czar został opłacony.
fn cast_spell(
    state: &mut DuelState,
    content: &Content<'_>,
    side: Side,
    card: CardId,
    events: &mut EventBuf,
) -> bool {
    let Some(def) = content.get(card) else {
        events.push(Event::SpellFizzled { side, card });
        return false;
    };
    let mut cost = spell_cost(state, side, def);
    // R-CZA-07: karta losowa działa jak wylosowana karta; rzucający płaci jej koszt W, a jej
    // wymagania szkół są pomijane.
    let acting = if def.keywords.random {
        let Some(drawn) = draw_random(state, content) else {
            events.push(Event::SpellFizzled { side, card });
            return false;
        };
        let drawn_cost = spell_cost(state, side, drawn);
        cost.stamina = drawn_cost.stamina;
        cost.scale = drawn_cost.scale;
        drawn
    } else {
        def
    };
    if !pay(state, side, cost.focus, cost.stamina) {
        events.push(Event::SpellFizzled { side, card });
        return false;
    }
    if def.keywords.intervention {
        state.player_mut(side).used_interventions.push(card);
    }
    events.push(Event::SpellCast {
        side,
        card,
        focus: cost.focus,
        stamina: cost.stamina,
    });
    if acting.id != card {
        events.push(Event::RandomSpell {
            side,
            card,
            drawn: acting.id,
        });
    }
    // R-CZA-08: niespełniony warunek oznacza pobrane koszty i brak efektu (Z-CND).
    if let Some(condition) = acting.condition
        && !end::holds(state, side, condition)
    {
        events.push(Event::SpellFailed { side, card });
        return true;
    }
    let immediate = match enchant::register(state, events, side, acting, cost.scale) {
        // Czar zwykły działa raz (R-CZA-01).
        None => Some(true),
        // Umagicznienie „co rundę” działa już w rundzie rzucenia (R-CZA-02).
        Some(Trigger::OnRound) => Some(false),
        Some(_) => None,
    };
    if let Some(triggers) = immediate {
        let mut ctx = EffectCtx {
            state,
            events,
            caster: side,
            source: acting.id,
            scale: cost.scale,
            triggers,
        };
        for effect in &acting.effects {
            ctx.apply(effect);
        }
    }
    true
}

/// Losowa karta z puli wszystkich kart gry, bez kart losowych i przeciwzaklęć, które działają
/// tylko jako rzucona karta (R-CZA-07).
fn draw_random<'c>(state: &mut DuelState, content: &Content<'c>) -> Option<&'c CardDef> {
    let eligible = |def: &&CardDef| !def.keywords.random && !def.keywords.counter;
    let count = content.cards().iter().filter(eligible).count();
    let bound = u32::try_from(count).ok().and_then(NonZeroU32::new)?;
    let pick = rng::below(&mut state.rng, bound) as usize;
    content.cards().iter().filter(eligible).nth(pick)
}

/// Koniec rundy (R-RUN-13): działają umagicznienia „na koniec rundy”, a potem wygasają zmiany
/// „do końca rundy” i wymuszone wyniki ataków.
fn end_round_effects(state: &mut DuelState, events: &mut EventBuf) {
    enchant::fire(state, events, Trigger::OnRoundEnd, None);
    for player in &mut state.players {
        player.round_delta = [0; 6];
        player.school_round = [0; 6];
        player.round_damage_pct = NORMAL_DAMAGE_PERCENT;
    }
    state.overrides.clear();
}

/// Faza ataków: 6 ataków naprzemiennie, zaczyna strona z inicjatywą (R-ATK-01). Wykonują się
/// zawsze wszystkie, także gdy Ż spadnie do 0 lub niżej (R-ATK-06). Seria trafień każdej strony
/// zaczyna się od zera w każdej rundzie (R-RUN-03).
fn attack_phase(state: &mut DuelState, events: &mut EventBuf) {
    let order = state.initiative_order();
    let mut combo = [0u8; 2];
    for slot in 0..SLOTS {
        for attacker in order {
            attack(state, attacker, slot, &mut combo[attacker.index()], events);
        }
    }
}

/// Atak strony `attacker` w slocie `slot` (R-ATK-02…05, R-ATK-07); `combo` to jej seria
/// trafień w tej rundzie.
fn attack(
    state: &mut DuelState,
    attacker: Side,
    slot: usize,
    combo: &mut u8,
    events: &mut EventBuf,
) {
    let block_min_stamina = state.rules.duel.block_min_stamina;
    let multipliers = state.rules.duel.combo_multipliers;
    let defender = attacker.other();
    let kind = state.player(attacker).attacks[slot];
    let value = state.stats(attacker).attack(kind);
    let defense = state.stats(defender).defense(kind);
    let slot_number = slot as u8;

    // Efekty kart mogą wymusić wynik; chybienie ma pierwszeństwo (Z-ENG-1). Bez nich decyduje
    // blok: obrona tego samego typu i dość W obrońcy (R-ATK-02), a przy mniejszej W atak trafia
    // niezależnie od obrony (R-ATK-03).
    let forced = state.take_override(attacker, slot, kind);
    let target = state.player(defender);
    let blocks = target.defenses[slot] == kind && target.stamina >= block_min_stamina;
    let failed = match forced {
        Some(false) => Some(Event::AttackMissed {
            attacker,
            slot: slot_number,
            kind,
        }),
        None if blocks => Some(Event::AttackBlocked {
            attacker,
            slot: slot_number,
            kind,
        }),
        Some(true) | None => None,
    };
    if let Some(event) = failed {
        // R-ATK-05: blok (albo wymuszone chybienie) zeruje serię i nic nie kosztuje.
        *combo = 0;
        let blocked = matches!(event, Event::AttackBlocked { .. });
        events.push(event);
        if blocked {
            enchant::fire(state, events, Trigger::OnBlocked, Some(defender));
        }
        return;
    }

    // R-ATK-04: seria liczy trafienia atakującego w tej rundzie, niezależnie od typu ataku.
    *combo += 1;
    let multiplier = multipliers[usize::from(*combo).min(SLOTS) - 1];
    let raw = value
        .saturating_mul(multiplier)
        .saturating_sub(defense)
        .max(0);
    let damage = state.scale_damage(defender, raw);
    let (striker, target) = state.pair_mut(attacker);
    // R-ATK-07: Ż może spaść poniżej 0, W nie.
    target.hp = target.hp.saturating_sub(damage);
    let stamina_cost = value.min(striker.stamina);
    striker.stamina -= stamina_cost;
    events.push(Event::AttackHit {
        attacker,
        slot: slot_number,
        kind,
        multiplier,
        damage,
        stamina_cost,
    });
    enchant::fire(state, events, Trigger::OnHit, Some(attacker));
    if damage > 0 {
        enchant::fire(state, events, Trigger::OnDamageTaken, Some(defender));
    }
}

/// Dobranie z wierzchu talii na miejsce zagranej lub odrzuconej karty (R-RUN-14). Gdy talia
/// jest pusta, gracz nie dobiera (Z-EMP).
fn draw_replacements(state: &mut DuelState, played: &[Option<Played>; 2], events: &mut EventBuf) {
    for side in state.initiative_order() {
        let Some(Played { index, .. }) = played[side.index()] else {
            continue;
        };
        let limit = usize::from(state.rules.duel.hand_size);
        let player = state.player_mut(side);
        // Ręka mogła się już zapełnić kartami dobranymi przez efekty (R-POJ-04).
        if player.hand.len() >= limit {
            continue;
        }
        let Some(card) = player.deck.pop() else {
            continue;
        };
        player.hand.insert(index.min(player.hand.len()), card);
        events.push(Event::CardDrawn { side, card });
    }
}

/// Koniec rundy: warunki końca pojedynku (R-RUN-15), a jeśli trwa dalej, początek następnej rundy.
fn finish_round(state: &mut DuelState, events: &mut EventBuf) -> RoundSummary {
    let round = state.round;
    let totals = Side::BOTH.map(|side| {
        let player = state.player(side);
        PlayerTotals {
            hp: player.hp,
            stamina: player.stamina,
            focus: player.focus,
        }
    });
    // R-KON-06: wygrane warunkowe przed nokautem (R-KON-01).
    let outcome = end::conditional_win(state)
        .or_else(|| end::knockout(state))
        .or_else(|| (round >= state.rules.duel.rounds).then(|| end::by_hp_percent(state)));
    state.win_checks.clear();
    match outcome {
        Some(outcome) => {
            state.phase = Phase::Finished(outcome);
            events.push(Event::DuelEnded { outcome });
        }
        None => {
            state.round += 1;
            begin_round(state);
            let focus = Side::BOTH.map(|side| state.player(side).focus);
            events.push(Event::RoundStarted {
                round: state.round,
                focus,
            });
        }
    }
    RoundSummary {
        round,
        totals,
        outcome,
    }
}
