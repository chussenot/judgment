//! Choosing a bar from graded judgments: where a Noul starts reading as yes, and how sure a
//! Choice or a Score must be before its answer is acted on.
//!
//! A threshold depends on the model, the questions and the data together, so it is read off a
//! table ([`threshold_sweep`], [`gate_table`], [`level_sweep`]) rather than guessed, and
//! [`best_threshold`], [`lowest_bar`] and [`best_level`] pick by a stated rule that is printed
//! with the result (`docs/guides/record-replay-and-test.md`). Whether to apply a bar is the application's choice; a
//! `.jud` rubric's policy carries the numbers.

// Counts are small; the casts to f64 are exact in practice.
#![allow(clippy::cast_precision_loss)]

use super::Judgment;

/// One row of a threshold sweep over a yes/no question.
#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdRow {
    /// The threshold: a probability of yes at or above it reads as yes.
    pub threshold: f64,
    /// Labelled judgments the row was computed over.
    pub n: usize,
    /// Predicted yes and labelled yes.
    pub true_positives: usize,
    /// Predicted yes, labelled no.
    pub false_positives: usize,
    /// Predicted no, labelled yes.
    pub false_negatives: usize,
    /// Predicted no and labelled no.
    pub true_negatives: usize,
    /// `(tp + tn) / n`.
    pub accuracy: f64,
    /// `tp / (tp + fp)`; `None` when nothing was predicted yes.
    pub precision: Option<f64>,
    /// `tp / (tp + fn)`; `None` when nothing was labelled yes.
    pub recall: Option<f64>,
    /// Harmonic mean of precision and recall; `None` when either is.
    pub f1: Option<f64>,
}

/// One row of a gate table over a Choice or a Score.
#[derive(Debug, Clone, PartialEq)]
pub struct GateRow {
    /// The bar: an answer whose confidence is at or above it is acted on.
    pub bar: f64,
    /// Labelled judgments the row was computed over.
    pub n: usize,
    /// Judgments at or above the bar.
    pub covered: usize,
    /// `covered / n`: the share the bar lets through; the rest go to a person or a fallback.
    pub coverage: f64,
    /// Correct judgments among the covered.
    pub correct: usize,
    /// `correct / covered`; `None` when the bar lets nothing through.
    pub accuracy: Option<f64>,
}

/// Thresholds from 0.05 to 0.95 in steps of 0.05.
pub fn default_thresholds() -> Vec<f64> {
    (1..=19).map(|i| f64::from(i) / 20.0).collect()
}

/// Bars from 0 to 0.95 in steps of 0.05; the first row, "act on everything", is the baseline.
pub fn default_bars() -> Vec<f64> {
    (0..=19).map(|i| f64::from(i) / 20.0).collect()
}

/// Sweep the thresholds over one yes/no question's judgments. Only labelled
/// judgments with a `yes` probability take part, so a Choice's judgments
/// handed here by mistake yield rows over zero cases rather than nonsense.
pub fn threshold_sweep<'a>(
    judgments: impl IntoIterator<Item = &'a Judgment>,
    thresholds: &[f64],
) -> Vec<ThresholdRow> {
    let pairs: Vec<(f64, bool)> = judgments
        .into_iter()
        .filter_map(|j| {
            let p_yes = *j.probabilities.get("yes")?;
            let yes = j.expected.as_deref()? == "yes";
            Some((p_yes, yes))
        })
        .collect();
    thresholds
        .iter()
        .map(|&threshold| {
            let (mut tp, mut fp, mut fn_, mut tn) = (0usize, 0usize, 0usize, 0usize);
            for (p, yes) in &pairs {
                match (*p >= threshold, *yes) {
                    (true, true) => tp += 1,
                    (true, false) => fp += 1,
                    (false, true) => fn_ += 1,
                    (false, false) => tn += 1,
                }
            }
            let n = pairs.len();
            let ratio = |num: usize, den: usize| (den > 0).then(|| num as f64 / den as f64);
            let precision = ratio(tp, tp + fp);
            let recall = ratio(tp, tp + fn_);
            let f1 = match (precision, recall) {
                (Some(p), Some(r)) if p + r > 0.0 => Some(2.0 * p * r / (p + r)),
                (Some(_), Some(_)) => Some(0.0),
                _ => None,
            };
            ThresholdRow {
                threshold,
                n,
                true_positives: tp,
                false_positives: fp,
                false_negatives: fn_,
                true_negatives: tn,
                accuracy: if n == 0 {
                    0.0
                } else {
                    (tp + tn) as f64 / n as f64
                },
                precision,
                recall,
                f1,
            }
        })
        .collect()
}

/// The threshold with the best F1; among equals, the lowest, because a lower
/// threshold says yes more often and a tie means the extra yeses cost
/// nothing in F1. `None` when no row has an F1 (nothing labelled yes).
pub fn best_threshold(rows: &[ThresholdRow]) -> Option<f64> {
    rows.iter()
        .filter_map(|row| row.f1.map(|f1| (f1, row.threshold)))
        .fold(
            None,
            |best: Option<(f64, f64)>, (f1, threshold)| match best {
                Some((best_f1, best_threshold))
                    if f1 < best_f1 || (f1 <= best_f1 && threshold > best_threshold) =>
                {
                    Some((best_f1, best_threshold))
                }
                _ => Some((f1, threshold)),
            },
        )
        .map(|(_, threshold)| threshold)
}

/// One row of a level sweep over a Score: "the nearest level is `level` or
/// higher" against the label "the expected level is `level` or higher".
#[derive(Debug, Clone, PartialEq)]
pub struct LevelRow {
    /// The level the decision starts at.
    pub level: usize,
    /// Labelled judgments the row was computed over.
    pub n: usize,
    /// At or above the level, and labelled at or above it.
    pub true_positives: usize,
    /// At or above the level, labelled below it.
    pub false_positives: usize,
    /// Below the level, labelled at or above it.
    pub false_negatives: usize,
    /// Below the level and labelled below it.
    pub true_negatives: usize,
    /// `(tp + tn) / n`.
    pub accuracy: f64,
    /// `tp / (tp + fp)`; `None` when nothing reached the level.
    pub precision: Option<f64>,
    /// `tp / (tp + fn)`; `None` when nothing was labelled at or above it.
    pub recall: Option<f64>,
    /// Harmonic mean of precision and recall; `None` when either is.
    pub f1: Option<f64>,
}

/// Sweep "this level or higher" over one Score's judgments, for every level from 1 (level 0
/// is reached by every answer). The level reached is the nearest level, `Σ i · p_i` rounded, as
/// a rubric gate's `level_at_least` reads it ([`crate::jud::Rubric::apply`]), not the most
/// probable level in `predicted`: the two differ on a spread distribution. Recomputed from the
/// probabilities, it can round apart from the wire's `score` at a .5 boundary, and every
/// labelled answer counts, including those a confidence bar would defer.
pub fn level_sweep<'a>(
    judgments: impl IntoIterator<Item = &'a Judgment>,
    levels: usize,
) -> Vec<LevelRow> {
    let pairs: Vec<(usize, usize)> = judgments
        .into_iter()
        .filter_map(|j| {
            let expected: usize = j.expected.as_deref()?.parse().ok()?;
            let mut position = 0.0;
            let mut any = false;
            for (level, p) in &j.probabilities {
                position += level.parse::<usize>().ok()? as f64 * p;
                any = true;
            }
            let nearest = crate::answer::nearest_index(position, levels);
            any.then_some((nearest, expected))
        })
        .collect();
    (1..levels)
        .map(|level| {
            let (mut tp, mut fp, mut fn_, mut tn) = (0usize, 0usize, 0usize, 0usize);
            for (nearest, expected) in &pairs {
                match (*nearest >= level, *expected >= level) {
                    (true, true) => tp += 1,
                    (true, false) => fp += 1,
                    (false, true) => fn_ += 1,
                    (false, false) => tn += 1,
                }
            }
            let n = pairs.len();
            let ratio = |num: usize, den: usize| (den > 0).then(|| num as f64 / den as f64);
            let precision = ratio(tp, tp + fp);
            let recall = ratio(tp, tp + fn_);
            let f1 = match (precision, recall) {
                (Some(p), Some(r)) if p + r > 0.0 => Some(2.0 * p * r / (p + r)),
                (Some(_), Some(_)) => Some(0.0),
                _ => None,
            };
            LevelRow {
                level,
                n,
                true_positives: tp,
                false_positives: fp,
                false_negatives: fn_,
                true_negatives: tn,
                accuracy: if n == 0 {
                    0.0
                } else {
                    (tp + tn) as f64 / n as f64
                },
                precision,
                recall,
                f1,
            }
        })
        .collect()
}

/// The level with the best F1; among equals, the lowest, as
/// [`best_threshold`] takes the lowest threshold. `None` when no row has an F1.
pub fn best_level(rows: &[LevelRow]) -> Option<usize> {
    rows.iter()
        .filter_map(|row| row.f1.map(|f1| (f1, row.level)))
        .fold(None, |best: Option<(f64, usize)>, (f1, level)| match best {
            Some((best_f1, best_level))
                if f1 < best_f1 || (f1 <= best_f1 && level > best_level) =>
            {
                Some((best_f1, best_level))
            }
            _ => Some((f1, level)),
        })
        .map(|(_, level)| level)
}

/// The gate table over one Choice's or Score's judgments: per bar, how many
/// labelled judgments reach it and how many of those are right.
pub fn gate_table<'a>(
    judgments: impl IntoIterator<Item = &'a Judgment>,
    bars: &[f64],
) -> Vec<GateRow> {
    let pairs: Vec<(f64, bool)> = judgments
        .into_iter()
        .filter_map(|j| Some((j.confidence, j.correct?)))
        .collect();
    bars.iter()
        .map(|&bar| {
            let covered: Vec<&(f64, bool)> = pairs.iter().filter(|(c, _)| *c >= bar).collect();
            let correct = covered.iter().filter(|(_, ok)| *ok).count();
            let n = pairs.len();
            GateRow {
                bar,
                n,
                covered: covered.len(),
                coverage: if n == 0 {
                    0.0
                } else {
                    covered.len() as f64 / n as f64
                },
                correct,
                accuracy: (!covered.is_empty()).then(|| correct as f64 / covered.len() as f64),
            }
        })
        .collect()
}

/// The lowest bar whose covered answers are right at least `target_accuracy`
/// of the time over at least `min_covered` judgments; lowest, because every
/// step up hands more work to a person. `None` when no bar reaches the target.
pub fn lowest_bar(rows: &[GateRow], target_accuracy: f64, min_covered: usize) -> Option<f64> {
    rows.iter()
        .filter(|r| r.covered >= min_covered.max(1))
        .filter(|r| r.accuracy.is_some_and(|a| a >= target_accuracy))
        .map(|r| r.bar)
        .fold(None, |low: Option<f64>, b| {
            Some(low.map_or(b, |l| l.min(b)))
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]

    use super::*;

    fn noul(p: f64, yes: bool) -> Judgment {
        Judgment::noul(p, Some(yes), true)
    }

    #[test]
    fn a_sweep_counts_each_cell_and_derives_the_rates() {
        let js = [
            noul(0.9, true),
            noul(0.7, true),
            noul(0.6, false),
            noul(0.2, false),
        ];
        let rows = threshold_sweep(&js, &[0.5, 0.65, 0.95]);
        assert_eq!(rows.len(), 3);
        let r = &rows[0];
        assert_eq!(
            (
                r.true_positives,
                r.false_positives,
                r.false_negatives,
                r.true_negatives
            ),
            (2, 1, 0, 1)
        );
        assert_eq!(r.accuracy, 0.75);
        assert_eq!(r.precision, Some(2.0 / 3.0));
        assert_eq!(r.recall, Some(1.0));
        let r = &rows[1];
        assert_eq!((r.true_positives, r.false_positives), (2, 0));
        assert_eq!(r.f1, Some(1.0));
        let r = &rows[2];
        assert_eq!(r.precision, None, "nothing predicted yes");
        assert_eq!(r.f1, None);
        assert_eq!(best_threshold(&rows), Some(0.65));
    }

    #[test]
    fn ties_in_f1_go_to_the_lower_threshold() {
        let js = [noul(0.9, true), noul(0.1, false)];
        let rows = threshold_sweep(&js, &default_thresholds());
        assert_eq!(best_threshold(&rows), Some(0.15));
    }

    #[test]
    fn unlabelled_and_non_noul_judgments_are_skipped() {
        let js = [
            Judgment::noul(0.9, None, true),
            Judgment::new(
                "a".into(),
                Some("a".into()),
                0.8,
                std::collections::BTreeMap::default(),
                true,
            ),
        ];
        let rows = threshold_sweep(&js, &[0.5]);
        assert_eq!(rows[0].n, 0);
        assert_eq!(best_threshold(&rows), None);
    }

    #[test]
    fn a_gate_table_trades_coverage_for_accuracy() {
        let j = |c: f64, ok: bool| {
            Judgment::new(
                if ok { "a" } else { "b" }.into(),
                Some("a".into()),
                c,
                std::collections::BTreeMap::default(),
                true,
            )
        };
        let js = [j(0.95, true), j(0.8, true), j(0.7, false), j(0.3, false)];
        let rows = gate_table(&js, &[0.0, 0.75, 0.9, 0.99]);
        assert_eq!(rows[0].coverage, 1.0);
        assert_eq!(rows[0].accuracy, Some(0.5));
        assert_eq!(rows[1].covered, 2);
        assert_eq!(rows[1].accuracy, Some(1.0));
        assert_eq!(rows[3].accuracy, None, "nothing clears 0.99");
        assert_eq!(lowest_bar(&rows, 0.9, 1), Some(0.75));
        assert_eq!(
            lowest_bar(&rows, 0.9, 3),
            None,
            "too few covered at every bar that reaches 0.9"
        );
        assert_eq!(lowest_bar(&rows, 0.5, 1), Some(0.0));
    }

    fn score(probabilities: [f64; 4], expected: usize) -> Judgment {
        Judgment::new(
            String::new(),
            Some(expected.to_string()),
            0.9,
            probabilities
                .iter()
                .enumerate()
                .map(|(i, p)| (i.to_string(), *p))
                .collect(),
            true,
        )
    }

    #[test]
    fn a_level_sweep_reads_the_nearest_level_as_the_gate_does() {
        let judgments = [
            // Most probable level 3, but the weighted position 1.8 rounds to 2.
            score([0.3, 0.0, 0.1, 0.6], 2),
            score([0.0, 0.0, 0.0, 1.0], 3),
            score([0.0, 1.0, 0.0, 0.0], 1),
            score([1.0, 0.0, 0.0, 0.0], 0),
        ];
        let rows = level_sweep(&judgments, 4);
        assert_eq!(rows.iter().map(|r| r.level).collect::<Vec<_>>(), [1, 2, 3]);
        let at_2 = &rows[1];
        assert_eq!(
            (
                at_2.true_positives,
                at_2.false_positives,
                at_2.false_negatives,
                at_2.true_negatives
            ),
            (2, 0, 0, 2)
        );
        assert_eq!(at_2.f1, Some(1.0));
        // Levels 1 and 2 both separate perfectly; the lower wins the tie.
        assert_eq!(best_level(&rows), Some(1));
        let skipped = [Judgment::noul(0.9, Some(true), true)];
        assert!(level_sweep(&skipped, 4).iter().all(|r| r.n == 0));
        assert_eq!(best_level(&level_sweep(&skipped, 4)), None);
    }

    // ---- the sweeps' laws ----

    use crate::answer::{Answer, Confidence, FromAnswer, Probability, Score};
    use proptest::prelude::*;

    /// The documented selection rule, written out apart from the fold: the
    /// greatest F1, and among equal F1s the lowest key.
    fn pick<K: Copy + PartialOrd>(rows: impl Iterator<Item = (Option<f64>, K)>) -> Option<K> {
        let rows: Vec<(f64, K)> = rows.filter_map(|(f1, key)| Some((f1?, key))).collect();
        let top = rows
            .iter()
            .map(|(f1, _)| *f1)
            .fold(f64::NEG_INFINITY, f64::max);
        rows.iter()
            .filter(|(f1, _)| *f1 == top)
            .map(|(_, key)| *key)
            .fold(None, |low: Option<K>, key| match low {
                Some(l) if l <= key => Some(l),
                _ => Some(key),
            })
    }

    fn sorted(mut bars: Vec<f64>) -> Vec<f64> {
        bars.sort_by(f64::total_cmp);
        bars
    }

    /// A Score distribution over four levels, normalised, and its label.
    fn distribution() -> impl Strategy<Value = ([f64; 4], usize)> {
        (prop::array::uniform4(0.0_f64..1.0), 0_usize..4).prop_map(|(w, expected)| {
            let total: f64 = w.iter().sum::<f64>() + 1e-3;
            (w.map(|x| x / total), expected)
        })
    }

    proptest! {
        /// Every bar sees every labelled judgment exactly once, and a higher bar never
        /// says yes more often.
        #[test]
        fn a_threshold_sweep_conserves_the_cases_and_is_monotone(
            cases in prop::collection::vec((0.0_f64..=1.0, prop::option::of(any::<bool>())), 0..30),
            bars in prop::collection::vec(0.0_f64..=1.0, 0..12),
        ) {
            let js: Vec<Judgment> = cases.iter().map(|(p, l)| Judgment::noul(*p, *l, true)).collect();
            let labelled = cases.iter().filter(|(_, l)| l.is_some()).count();
            let rows = threshold_sweep(&js, &sorted(bars));
            for row in &rows {
                prop_assert_eq!(row.n, labelled);
                prop_assert_eq!(
                    row.true_positives + row.false_positives + row.false_negatives + row.true_negatives,
                    labelled
                );
            }
            for pair in rows.windows(2) {
                prop_assert!(pair[1].true_positives <= pair[0].true_positives);
                prop_assert!(pair[1].false_positives <= pair[0].false_positives);
                prop_assert!(pair[1].false_negatives >= pair[0].false_negatives);
            }
            prop_assert_eq!(best_threshold(&rows), pick(rows.iter().map(|r| (r.f1, r.threshold))));
        }

        /// Covered never exceeds the labelled count, never grows with the bar, and
        /// `lowest_bar` is the least bar whose row is big enough and accurate enough.
        #[test]
        fn a_gate_table_is_monotone_and_lowest_bar_is_the_least_qualifying_bar(
            cases in prop::collection::vec((0.0_f64..=1.0, any::<bool>()), 0..30),
            bars in prop::collection::vec(0.0_f64..=1.0, 0..12),
            target in 0.0_f64..=1.0,
            min_covered in 0_usize..6,
        ) {
            let js: Vec<Judgment> = cases
                .iter()
                .map(|(c, ok)| {
                    Judgment::new(
                        if *ok { "a" } else { "b" }.into(),
                        Some("a".into()),
                        *c,
                        std::collections::BTreeMap::default(),
                        true,
                    )
                })
                .collect();
            let rows = gate_table(&js, &sorted(bars));
            for row in &rows {
                prop_assert_eq!(row.n, cases.len());
                prop_assert!(row.correct <= row.covered && row.covered <= row.n);
            }
            for pair in rows.windows(2) {
                prop_assert!(pair[1].covered <= pair[0].covered);
            }
            let want = rows
                .iter()
                .filter(|r| r.covered >= min_covered.max(1) && r.accuracy.is_some_and(|a| a >= target))
                .map(|r| r.bar)
                .reduce(f64::min);
            prop_assert_eq!(lowest_bar(&rows, target, min_covered), want);
        }

        /// The same two laws for a level sweep, and `best_level` follows the same rule.
        #[test]
        fn a_level_sweep_conserves_the_cases_and_is_monotone(
            cases in prop::collection::vec(distribution(), 0..30),
        ) {
            let js: Vec<Judgment> = cases.iter().map(|(p, e)| score(*p, *e)).collect();
            let rows = level_sweep(&js, 4);
            for row in &rows {
                prop_assert_eq!(row.n, cases.len());
                prop_assert_eq!(
                    row.true_positives + row.false_positives + row.false_negatives + row.true_negatives,
                    cases.len()
                );
            }
            // The label moves with the level, so only what was predicted is monotone.
            for pair in rows.windows(2) {
                prop_assert!(
                    pair[1].true_positives + pair[1].false_positives
                        <= pair[0].true_positives + pair[0].false_positives
                );
            }
            prop_assert_eq!(best_level(&rows), pick(rows.iter().map(|r| (r.f1, r.level))));
        }

        /// Where the wire `score` is the probability-weighted position its own
        /// probabilities give, the level the sweep reads is the level the gate reads.
        #[test]
        fn the_sweep_and_the_gate_read_the_same_level_when_the_wire_score_agrees(
            (p, expected) in distribution(),
        ) {
            let position: f64 = p.iter().enumerate().fold(0.0, |sum, (i, p)| sum + i as f64 * p);
            let (gate, sweep) = gate_and_sweep_levels(position, p, expected);
            prop_assert_eq!(gate, sweep);
        }
    }

    /// `(level the gate reads from the wire score, level the sweep reads)` for one answer.
    fn gate_and_sweep_levels(wire_score: f64, p: [f64; 4], expected: usize) -> (usize, usize) {
        let answer = Answer::Score {
            score: wire_score,
            legend: (0..4)
                .map(|i| (i.to_string(), serde_json::json!(format!("level {i}"))))
                .collect(),
            probabilities: p
                .iter()
                .enumerate()
                .map(|(i, p)| (i.to_string(), Probability::new(p.min(1.0)).unwrap()))
                .collect(),
            confidence: Confidence::new(0.9).unwrap(),
            extra: std::collections::BTreeMap::new(),
        };
        let gate = Score::from_answer("q", &answer).unwrap().nearest_level();
        let judgment = Judgment::of_answer(&answer, Some(&expected.to_string()));
        // One judgment: row `level` says yes exactly when it reached `level`.
        let sweep = level_sweep([&judgment], 4)
            .iter()
            .filter(|r| r.true_positives + r.false_positives == 1)
            .count();
        (gate, sweep)
    }

    /// Deliberate: the sweep reads the probabilities, never the wire `score`, so a
    /// server whose `score` disagrees with its own distribution is graded by the
    /// distribution (the rubric gate, which reads `score`, may then read another level).
    #[test]
    fn the_sweep_reads_the_probabilities_not_a_disagreeing_wire_score() {
        // All the weight on level 0; the server claims 3.0.
        assert_eq!(gate_and_sweep_levels(3.0, [1.0, 0.0, 0.0, 0.0], 0), (3, 0));
        // At a .5 boundary: the distribution's position is exactly 0.5, which
        // rounds up to level 1; a score a hair under it stays at 0.
        assert_eq!(gate_and_sweep_levels(0.5, [0.5, 0.5, 0.0, 0.0], 0), (1, 1));
        assert_eq!(
            gate_and_sweep_levels(0.499_999, [0.5, 0.5, 0.0, 0.0], 0),
            (0, 1)
        );
    }
}
