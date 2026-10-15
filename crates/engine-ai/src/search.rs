//! Jedna runda przeszukiwania na 8 determinizacjach; regret matching na macierzy 24 × 24.

use engine::{CardId, Content, DuelState, EventBuf, Move, Outcome, PlayerView, Side, view};
use rand_chacha::rand_core::Rng;

use crate::Bot;

const CANDIDATES: usize = 24;
const SAMPLES: usize = 8;
const ITERATIONS: usize = 1000;
const PROBABILITY: i64 = 65_536;

impl Bot {
    pub(crate) fn candidates(&self, seen: &PlayerView, content: &Content<'_>) -> Vec<Move> {
        let ranked = self.ranked_moves(seen, content);
        let mut moves = vec![seen.default_move()];
        // Najlepszy ruch dla każdej akcji kartą: czary nie znikają wśród podobnych ustawień.
        for (mv, _) in &ranked {
            if !moves.iter().any(|old| old.card == mv.card) {
                moves.push(*mv);
            }
        }
        for (mv, _) in &ranked {
            if moves.len() >= CANDIDATES {
                break;
            }
            if !moves.contains(mv) {
                moves.push(*mv);
            }
        }
        moves
    }

    fn sample_cards(
        &mut self,
        seen: &PlayerView,
        content: &Content<'_>,
        own: bool,
        count: usize,
    ) -> Vec<CardId> {
        let side = if own { &seen.me } else { &seen.opponent };
        let mut pool = Vec::new();
        for card in content.cards() {
            if !side.schools.meets(&card.requires) {
                continue;
            }
            let known = side.discard.iter().filter(|&&id| id == card.id).count()
                + if own {
                    seen.hand.iter().filter(|&&id| id == card.id).count()
                } else {
                    0
                };
            for _ in known..usize::from(seen.rules.deck.max_copies) {
                pool.push(card.id);
            }
        }
        let mut result = Vec::with_capacity(count);
        let mut with_replacement = false;
        for _ in 0..count {
            if pool.is_empty() {
                if seen.rules.deck.fill_missing_with_random {
                    // R-TAL-05: uzupełnienia losowane z całej puli ZE ZWRACANIEM.
                    // Limit kopii dotyczy zapisanej talii, nie tych uzupełnień.
                    pool.extend(content.cards().iter().map(|card| card.id));
                    with_replacement = true;
                } else {
                    // R-TAL-04 pozwala zapisać również karty spoza szkół postaci.
                    for card in content.cards() {
                        let known = side.discard.iter().filter(|&&id| id == card.id).count()
                            + result.iter().filter(|&&id| id == card.id).count()
                            + if own {
                                seen.hand.iter().filter(|&&id| id == card.id).count()
                            } else {
                                0
                            };
                        for _ in known..usize::from(seen.rules.deck.max_copies) {
                            pool.push(card.id);
                        }
                    }
                }
            }
            if pool.is_empty() {
                break;
            }
            // Ujawnione karty przybliżają popularność archetypu, nigdy prawdziwą talię.
            let weights: Vec<_> = pool
                .iter()
                .map(|id| 1 + side.discard.iter().filter(|&seen| seen == id).count())
                .collect();
            let mut draw = self.pick(weights.iter().sum());
            let index = weights
                .iter()
                .position(|&weight| {
                    if draw < weight {
                        true
                    } else {
                        draw -= weight;
                        false
                    }
                })
                .unwrap_or(0);
            result.push(if with_replacement {
                pool[index]
            } else {
                pool.swap_remove(index)
            });
        }
        result
    }

    pub(crate) fn sample(&mut self, seen: &PlayerView, content: &Content<'_>) -> Option<DuelState> {
        let own_deck = self.sample_cards(seen, content, true, usize::from(seen.me.deck_len));
        let hidden = self.sample_cards(
            seen,
            content,
            false,
            usize::from(seen.opponent.hand_len) + usize::from(seen.opponent.deck_len),
        );
        let split = usize::from(seen.opponent.hand_len).min(hidden.len());
        let (hand, foe_deck) = hidden.split_at(split);
        let mut decks = [own_deck.as_slice(), foe_deck];
        if seen.side == Side::B {
            decks.swap(0, 1);
        }
        let mut seed = [0; 32];
        self.rng.fill_bytes(&mut seed);
        seen.sample_state(content, hand, decks, seed)
    }

    pub(crate) fn search_move(&mut self, seen: &PlayerView, content: &Content<'_>) -> Move {
        let ours = self.candidates(seen, content);
        let mut matrix = vec![vec![0i64; CANDIDATES]; ours.len()];
        let mut events = EventBuf::new();
        let mut samples = 0;
        for _ in 0..SAMPLES {
            let Some(state) = self.sample(seen, content) else {
                continue;
            };
            let foe = view(&state, seen.side.other());
            // Osobny model: własnego modelu przeciwnika nie używamy jako jego wiedzy o nas.
            let predictor = Bot::new(crate::Level::L2, 0);
            let theirs = predictor.candidates(&foe, content);
            if theirs.is_empty() {
                continue;
            }
            for (i, &our_move) in ours.iter().enumerate() {
                for j in 0..CANDIDATES {
                    let their_move = theirs[j % theirs.len()];
                    let mut branch = state.clone();
                    events.clear();
                    let mut moves = [our_move, their_move];
                    if seen.side == Side::B {
                        moves.swap(0, 1);
                    }
                    if engine::resolve_round(&mut branch, content, &moves, &mut events).is_ok() {
                        matrix[i][j] +=
                            evaluate(&branch, seen.side, self.aggression, self.spell_bias);
                    }
                }
            }
            samples += 1;
        }
        if samples == 0 {
            return self.adaptive_move(seen, content);
        }
        let strategy = solve(&matrix, ITERATIONS);
        let mut draw = self.pick(strategy.iter().map(|&weight| weight as usize).sum());
        for (mv, weight) in ours.iter().zip(strategy) {
            if draw < weight as usize {
                return *mv;
            }
            draw -= weight as usize;
        }
        ours[0]
    }
}

fn evaluate(state: &DuelState, side: Side, aggression: i32, spell_bias: i32) -> i64 {
    if let Some(outcome) = state.outcome() {
        return match outcome {
            Outcome::Win { winner, .. } if winner == side => 100_000,
            Outcome::Win { .. } => -100_000,
            Outcome::Draw { .. } => 0,
        };
    }
    let value = |side| {
        let player = state.player(side);
        let stats = state.stats(side);
        let rounds = i64::from(state.rules().duel.rounds.saturating_sub(state.round()) + 1);
        let hp = i64::from(player.hp().max(0)) * 10_000 / i64::from(player.hp_start().max(1));
        let stamina = i64::from(player.stamina()) * 1500 / i64::from(stats.stamina_max.max(1));
        let focus = i64::from(player.focus()) * 60;
        let power = i64::from(stats.phys_atk + stats.mag_atk + stats.phys_def + stats.mag_def)
            * rounds
            * 20;
        let enchantments = state
            .enchantments()
            .filter(|(owner, _)| *owner == side)
            .count() as i64
            * 100;
        let hand = player.hand().len() as i64 * 20;
        hp + stamina * i64::from(200 - aggression) / 100
            + focus
            + power
            + (enchantments + hand) * i64::from(spell_bias) / 100
    };
    (value(side) - value(side.other())).clamp(-90_000, 90_000)
}

fn normalize(weights: &[i64]) -> Vec<i64> {
    let total: i64 = weights.iter().map(|&weight| weight.max(0)).sum();
    if total == 0 {
        return vec![PROBABILITY / weights.len() as i64; weights.len()];
    }
    weights
        .iter()
        .map(|&weight| weight.max(0) * PROBABILITY / total)
        .collect()
}

/// Dwustronny regret matching dla gry o sumie zero; zwraca średnią strategię wierszy.
pub(crate) fn solve(matrix: &[Vec<i64>], iterations: usize) -> Vec<i64> {
    if matrix.is_empty() || matrix[0].is_empty() {
        return Vec::new();
    }
    let (rows, columns) = (matrix.len(), matrix[0].len());
    let (mut row_regret, mut col_regret) = (vec![0i64; rows], vec![0i64; columns]);
    let mut average = vec![0; rows];
    for _ in 0..iterations {
        let (p, q) = (normalize(&row_regret), normalize(&col_regret));
        let row_values: Vec<i64> = matrix
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&q)
                    .map(|(payoff, prob)| payoff * prob)
                    .sum::<i64>()
                    / PROBABILITY
            })
            .collect();
        let col_values: Vec<i64> = (0..columns)
            .map(|j| {
                matrix
                    .iter()
                    .zip(&p)
                    .map(|(row, prob)| -row[j] * prob)
                    .sum::<i64>()
                    / PROBABILITY
            })
            .collect();
        let value = row_values
            .iter()
            .zip(&p)
            .map(|(payoff, prob)| payoff * prob)
            .sum::<i64>()
            / PROBABILITY;
        for i in 0..rows {
            row_regret[i] += row_values[i] - value;
            average[i] += p[i];
        }
        for j in 0..columns {
            col_regret[j] += col_values[j] + value;
        }
    }
    normalize(&average)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_fill_can_exceed_copy_limit_and_still_be_sampled() {
        use engine::testkit::{classic_rules, content, player, spell, stats};
        let cards = [spell(
            1,
            0,
            0,
            &[engine::Effect::Heal {
                target: engine::Target::Caster,
                amount: 1,
            }],
        )];
        let content = content(&cards);
        let players = Side::BOTH.map(|_| player(stats(0, 0, 0, 0, 100, 80), &[]));
        let state = engine::new_duel(
            &engine::DuelSetup {
                rules: classic_rules(),
                seed: [0; 32],
                players,
            },
            &content,
        )
        .unwrap();
        for side in Side::BOTH {
            let seen = view(&state, side);
            let sampled = Bot::new(crate::Level::L3, 0)
                .sample(&seen, &content)
                .unwrap();
            for side in Side::BOTH {
                assert_eq!(
                    sampled.player(side).hand().len() + sampled.player(side).deck().len(),
                    20
                );
                assert!(
                    sampled
                        .player(side)
                        .hand()
                        .iter()
                        .chain(sampled.player(side).deck())
                        .all(|&id| id == 1)
                );
            }
            assert!(
                seen.validate_move(
                    &content,
                    &Bot::new(crate::Level::L3, 0).choose(&seen, &content)
                )
                .is_ok()
            );
        }
    }

    #[test]
    fn sampling_without_random_fill_preserves_copy_limits() {
        use engine::testkit::{Scenario, classic_rules, content, noop_card, stats};
        let content_cards = [noop_card(1, 0, 0), noop_card(2, 0, 0)];
        let content = content(&content_cards);
        let mut rules = classic_rules();
        rules.deck.fill_missing_with_random = false;
        let state = Scenario::new(stats(0, 0, 0, 0, 100, 80), stats(0, 0, 0, 0, 100, 80))
            .rules(rules)
            .hand(Side::A, &[1, 1])
            .hand(Side::B, &[2, 2])
            .deck(Side::A, &[1, 1, 2, 2])
            .deck(Side::B, &[1, 1, 2, 2])
            .build();
        for seed in 0..20 {
            for side in Side::BOTH {
                let sampled = Bot::new(crate::Level::L3, seed)
                    .sample(&view(&state, side), &content)
                    .unwrap();
                for side in Side::BOTH {
                    for id in [1, 2] {
                        assert!(
                            sampled
                                .player(side)
                                .hand()
                                .iter()
                                .chain(sampled.player(side).deck())
                                .filter(|&&card| card == id)
                                .count()
                                <= 4
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn solver_finds_a_mixed_strategy_and_a_dominant_move() {
        let rps = vec![
            vec![0, -1000, 1000],
            vec![1000, 0, -1000],
            vec![-1000, 1000, 0],
        ];
        let weights = solve(&rps, 1000);
        assert!(
            weights
                .iter()
                .all(|&weight| (21_000..23_000).contains(&weight))
        );
        let dominant = solve(&[vec![1000, 1000], vec![-1000, 0]], 1000);
        assert!(dominant[0] > PROBABILITY * 99 / 100);
    }

    #[test]
    fn candidate_set_covers_available_cards() {
        let cards = engine::testkit::sample_cards();
        let content = engine::testkit::content(&cards);
        let setup = engine::testkit::random_setup(&mut engine::testkit::seeded_rng(7), &content);
        let state = engine::new_duel(&setup, &content).unwrap();
        let seen = view(&state, Side::A);
        let bot = Bot::new(crate::Level::L3, 0);
        let candidates = bot.candidates(&seen, &content);
        assert!(candidates.len() <= CANDIDATES);
        for mv in crate::adaptive::legal_moves(&seen, &content) {
            assert!(candidates.iter().any(|candidate| candidate.card == mv.card));
        }
    }
}
