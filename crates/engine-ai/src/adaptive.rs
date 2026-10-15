//! Dirichlet dla przejść ustawień, warunkowany fazą pojedynku i poprzednim wyborem.

use engine::{CardAction, Content, Kind, Move, PlayerView, SLOTS};

use crate::{Bot, best_card, card_ready, card_value, kinds, other};

const SCALE: i64 = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpponentModel {
    // [wczesna/środkowa/późna runda][atak/obrona × slot][poprzedni][ujawniony]
    counts: [[[[u32; 2]; 2]; 2 * SLOTS]; 3],
    previous: Option<(u8, engine::Side, [Kind; SLOTS], [Kind; SLOTS])>,
    observations: u32,
}

impl OpponentModel {
    pub fn observations(&self) -> u32 {
        self.observations
    }

    /// Nowa walka z tym samym przeciwnikiem: zachowuje statystyki, usuwa przejście między walkami.
    pub fn reset_duel(&mut self) {
        self.previous = None;
    }

    pub(crate) fn observe(&mut self, view: &PlayerView) {
        let finished = matches!(view.phase, engine::Phase::Finished(_));
        if let Some((round, side, attacks, defenses)) = self.previous {
            let resolved_final = matches!(
                view.phase,
                engine::Phase::Finished(
                    engine::Outcome::Win {
                        reason: engine::Reason::Knockout
                            | engine::Reason::Rounds
                            | engine::Reason::ConditionalWin,
                        ..
                    } | engine::Outcome::Draw {
                        reason: engine::Reason::Knockout
                            | engine::Reason::Rounds
                            | engine::Reason::ConditionalWin
                    }
                )
            );
            if side == view.side && round == view.round && !finished {
                return;
            }
            if side == view.side
                && (round + 1 == view.round || (round == view.round && resolved_final))
            {
                let bucket = bucket(round);
                for slot in 0..SLOTS {
                    for (index, old, new) in [
                        (slot, attacks[slot], view.opponent.attacks[slot]),
                        (SLOTS + slot, defenses[slot], view.opponent.defenses[slot]),
                    ] {
                        let count = &mut self.counts[bucket][index][bit(old)][bit(new)];
                        *count = count.saturating_add(1);
                    }
                }
                self.observations = self.observations.saturating_add(1);
            }
        }
        self.previous = (!finished).then_some((
            view.round,
            view.side,
            view.opponent.attacks,
            view.opponent.defenses,
        ));
    }

    /// Rozkład na CAŁYCH ustawieniach; niewykonalne przy jawnym K mają wagę zero.
    pub(crate) fn distribution(&self, view: &PlayerView) -> Vec<(Move, i64)> {
        let foe = &view.opponent;
        let mut prior = Move {
            attacks: foe.attacks,
            defenses: foe.defenses,
            card: CardAction::None,
        };
        let mut changes = Vec::new();
        for slot in 0..SLOTS {
            let damage = |kind| {
                if view.me.defenses[slot] == kind
                    && view.me.stamina >= view.rules.duel.block_min_stamina
                {
                    0
                } else {
                    (foe.stats.attack(kind) - view.me.stats.defense(kind)).max(0)
                }
            };
            let current = prior.attacks[slot];
            let gain = damage(other(current)) - damage(current);
            if gain > 2 * view.rules.duel.change_cost {
                changes.push((gain, slot, true));
            }
            let hurt = |kind| (view.me.stats.attack(kind) - foe.stats.defense(kind)).max(0);
            let previous = view.me.attacks[slot];
            let threat = if hurt(other(previous)) > hurt(previous) {
                other(previous)
            } else {
                previous
            };
            if foe.stamina >= view.rules.duel.block_min_stamina
                && prior.defenses[slot] != threat
                && hurt(threat) > 2 * view.rules.duel.change_cost
            {
                changes.push((hurt(threat), slot, false));
            }
        }
        changes.sort_by_key(|&(gain, slot, attack)| (-gain, slot, attack));
        let mut focus = foe.focus;
        for (_, slot, attack) in changes {
            if focus >= view.rules.duel.change_cost {
                focus -= view.rules.duel.change_cost;
                if attack {
                    prior.attacks[slot] = other(prior.attacks[slot]);
                } else {
                    prior.defenses[slot] = other(prior.defenses[slot]);
                }
            }
        }
        let mut result = Vec::new();
        for bits in 0..64 {
            let mv = Move {
                attacks: kinds(bits),
                defenses: kinds(bits >> SLOTS),
                card: CardAction::None,
            };
            let cost = (0..SLOTS)
                .map(|s| {
                    i32::from(mv.attacks[s] != foe.attacks[s])
                        + i32::from(mv.defenses[s] != foe.defenses[s])
                })
                .sum::<i32>()
                * view.rules.duel.change_cost;
            if cost > foe.focus {
                continue;
            }
            let mut weight = 1_000_000i64;
            for slot in 0..SLOTS {
                for (index, old, chosen, predicted) in [
                    (
                        slot,
                        foe.attacks[slot],
                        mv.attacks[slot],
                        prior.attacks[slot],
                    ),
                    (
                        SLOTS + slot,
                        foe.defenses[slot],
                        mv.defenses[slot],
                        prior.defenses[slot],
                    ),
                ] {
                    let counts = self.counts[bucket(view.round)][index][bit(old)];
                    // A priori: 85% heurystyka, 15% eksploracja. Obserwacje stopniowo ją zastępują.
                    let alpha = if chosen == predicted { 17 } else { 3 };
                    weight = weight * (i64::from(counts[bit(chosen)].min(1000)) + alpha)
                        / (i64::from(counts[0].min(1000) + counts[1].min(1000)) + 20);
                }
            }
            result.push((mv, weight.max(1)));
        }
        result
    }
}

fn bucket(round: u8) -> usize {
    usize::from(round.saturating_sub(1) / 3).min(2)
}
fn bit(kind: Kind) -> usize {
    usize::from(kind == Kind::Mag)
}

pub(crate) fn legal_moves(view: &PlayerView, content: &Content<'_>) -> Vec<Move> {
    let mut actions = vec![CardAction::None];
    for index in 0..view.hand.len() as u8 {
        actions.extend([CardAction::Cast(index), CardAction::Discard(index)]);
    }
    let mut moves = Vec::new();
    for bits in 0..64 {
        for &card in &actions {
            let mv = Move {
                attacks: kinds(bits),
                defenses: kinds(bits >> SLOTS),
                card,
            };
            if view.validate_move(content, &mv).is_ok() {
                moves.push(mv);
            }
        }
    }
    moves
}

impl Bot {
    pub(crate) fn ranked_moves(
        &self,
        view: &PlayerView,
        content: &Content<'_>,
    ) -> Vec<(Move, i64)> {
        let distribution = self.model.distribution(view);
        let total: i64 = distribution.iter().map(|(_, weight)| weight).sum();
        let mut attacks = [[0i64; 2]; SLOTS];
        let mut defenses = [[0i64; 2]; SLOTS];
        for (mv, weight) in distribution {
            for slot in 0..SLOTS {
                attacks[slot][bit(mv.attacks[slot])] += weight * SCALE / total.max(1);
                defenses[slot][bit(mv.defenses[slot])] += weight * SCALE / total.max(1);
            }
        }
        let preferred = best_card(view, content);
        let mut ranked: Vec<_> = legal_moves(view, content)
            .into_iter()
            .filter(|mv| match mv.card {
                CardAction::Cast(index) => content
                    .get(view.hand[usize::from(index)])
                    .is_some_and(|def| card_ready(view, def)),
                _ => true,
            })
            .map(|mv| {
                let cost = view.validate_move(content, &mv).expect("legal move");
                let mut score = 0i64;
                for slot in 0..SLOTS {
                    let kind = mv.attacks[slot];
                    let damage =
                        (view.me.stats.attack(kind) - view.opponent.stats.defense(kind)).max(0);
                    let hit = if view.opponent.stamina >= view.rules.duel.block_min_stamina {
                        SCALE - defenses[slot][bit(kind)]
                    } else {
                        SCALE
                    };
                    let stamina_factor =
                        if view.me.stamina - cost.stamina >= view.rules.duel.block_min_stamina {
                            100
                        } else {
                            33
                        };
                    score += i64::from(damage) * hit * i64::from(self.aggression) * stamina_factor
                        / i64::from(view.opponent.hp_start.max(1))
                        / 100;
                    for kind in [Kind::Phys, Kind::Mag] {
                        if mv.defenses[slot] != kind
                            || view.me.stamina - cost.stamina < view.rules.duel.block_min_stamina
                        {
                            let damage = (view.opponent.stats.attack(kind)
                                - view.me.stats.defense(kind))
                            .max(0);
                            score -= i64::from(damage) * attacks[slot][bit(kind)] * 100
                                / i64::from(view.me.hp_start.max(1));
                        }
                    }
                }
                // K zachowuje wartość na kolejne rundy, ale nie na końcu pojedynku.
                let remaining = view.rules.duel.rounds.saturating_sub(view.round);
                score -= i64::from(cost.focus)
                    * if remaining == 0 {
                        0
                    } else {
                        5 * SCALE / i64::from(view.me.hp_start.max(1))
                    };
                if let CardAction::Cast(index) = mv.card
                    && let Some(card) = content.get(view.hand[usize::from(index)])
                {
                    score += i64::from(card_value(view, card)) * SCALE * i64::from(self.spell_bias)
                        / i64::from(view.me.hp_start.max(1));
                    score -= i64::from(cost.stamina) * SCALE * 12
                        / i64::from(view.me.stats.stamina_max.max(1));
                }
                if matches!(mv.card, CardAction::Discard(_)) {
                    score += if mv.card == preferred {
                        5 * SCALE
                    } else {
                        -SCALE
                    };
                }
                (mv, score)
            })
            .collect();
        ranked.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        ranked
    }

    pub(crate) fn adaptive_move(&mut self, view: &PlayerView, content: &Content<'_>) -> Move {
        let ranked = self.ranked_moves(view, content);
        let Some(&(_, top)) = ranked.first() else {
            return view.default_move();
        };
        // Softmax w stałym przecinku: (1 + x/32)^32, x <= 0. Brak floatów w WASM.
        let temperature = i64::from(self.temperature.max(1));
        let weights: Vec<_> = ranked
            .iter()
            .map(|(_, score)| {
                let gap = (top - score) * 1000 / temperature;
                let base = (SCALE - gap / 32).max(0);
                let mut weight = SCALE;
                for _ in 0..32 {
                    weight = weight * base / SCALE;
                }
                weight as usize
            })
            .collect();
        let mut draw = self.pick(weights.iter().sum());
        for ((mv, _), weight) in ranked.iter().zip(weights) {
            if draw < weight {
                return *mv;
            }
            draw -= weight;
        }
        ranked[0].0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::Side;
    use engine::testkit::{Scenario, stats};

    #[test]
    fn model_learns_only_revealed_rounds_and_does_not_count_repeated_decisions() {
        let state = Scenario::new(stats(8, 2, 7, 3, 100, 60), stats(8, 2, 7, 3, 100, 60)).build();
        let mut seen = engine::view(&state, Side::A);
        let mut model = OpponentModel::default();
        model.observe(&seen);
        model.observe(&seen);
        assert_eq!(model.observations(), 0);
        seen.round = 2;
        seen.opponent.attacks = [Kind::Mag; SLOTS];
        model.observe(&seen);
        model.observe(&seen);
        assert_eq!(model.observations(), 1);
        assert_eq!(model.counts[0][0][0][1], 1);
        model.reset_duel();
        seen.round = 1;
        model.observe(&seen);
        assert_eq!(model.observations(), 1);
    }

    #[test]
    fn predictions_assign_zero_probability_to_unaffordable_joint_settings() {
        let state = Scenario::new(stats(8, 2, 7, 3, 100, 60), stats(8, 2, 7, 3, 100, 60)).build();
        let mut seen = engine::view(&state, Side::A);
        let model = OpponentModel::default();
        for focus in 0..=6 {
            seen.opponent.focus = focus;
            let predicted = model.distribution(&seen);
            assert!(!predicted.is_empty());
            for (mv, weight) in predicted {
                assert!(weight > 0);
                let changes = (0..SLOTS)
                    .map(|slot| {
                        i32::from(mv.attacks[slot] != seen.opponent.attacks[slot])
                            + i32::from(mv.defenses[slot] != seen.opponent.defenses[slot])
                    })
                    .sum::<i32>();
                assert!(changes * seen.rules.duel.change_cost <= focus);
                if focus == 0 {
                    assert_eq!(changes, 0);
                }
            }
        }
    }

    #[test]
    fn repeated_revealed_choices_shift_the_prediction_and_survive_a_new_duel() {
        let state = Scenario::new(stats(8, 2, 7, 3, 100, 60), stats(8, 2, 7, 3, 100, 60)).build();
        let mut seen = engine::view(&state, Side::A);
        seen.opponent.focus = 6;
        let mut model = OpponentModel::default();
        let chance = |model: &OpponentModel, seen: &PlayerView| {
            let distribution = model.distribution(seen);
            let mag: i64 = distribution
                .iter()
                .filter(|(mv, _)| mv.attacks[0] == Kind::Mag)
                .map(|(_, weight)| weight)
                .sum();
            mag * 1000 / distribution.iter().map(|(_, weight)| weight).sum::<i64>()
        };
        let before = chance(&model, &seen);
        for _ in 0..80 {
            model.reset_duel();
            model.observe(&seen);
            let mut revealed = seen.clone();
            revealed.round = 2;
            revealed.opponent.attacks = [Kind::Mag; SLOTS];
            model.observe(&revealed);
        }
        model.reset_duel();
        assert!(chance(&model, &seen) > before + 300);
        assert_eq!(model.observations(), 80);
    }
}
