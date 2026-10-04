//! Choosing a bar from graded judgments: where a Noul starts reading as
//! yes, and how sure a Choice or a Score has to be before its answer is
//! acted on.
//!
//! A threshold is the one number in a decision policy that cannot be read
//! off the documentation: it is a property of the model, the questions and
//! the data together, and it moves when any of the three does. These
//! functions turn labelled judgments into the two tables that make the
//! choice visible: a sweep over thresholds for a yes/no question (what each
//! threshold buys in precision, recall and F1) and a gate table for a Choice
//! or a Score (what share of the answers each confidence bar lets through,
//! and how often those are right). [`best_threshold`] and [`lowest_bar`]
//! pick from the tables by a stated rule; the rule is the policy, and it is
//! printed with the result rather than hidden in it.
//!
//! Everything here is pure arithmetic over [`Judgment`]s and leaves the
//! choice of whether to apply a bar to the application; the `jud` module's
//! policy carries the chosen numbers with their provenance.

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
    /// `covered / n`: the share of answers the bar lets through; the rest
    /// go to a person or a fallback.
    pub coverage: f64,
    /// Correct judgments among the covered.
    pub correct: usize,
    /// `correct / covered`; `None` when the bar lets nothing through.
    pub accuracy: Option<f64>,
}

/// Thresholds from 0.05 to 0.95 in steps of 0.05, the sweep a report prints.
pub fn default_thresholds() -> Vec<f64> {
    (1..=19).map(|i| f64::from(i) / 20.0).collect()
}

/// Bars from 0 to 0.95 in steps of 0.05: the first row is "act on
/// everything", the baseline every other row is compared with.
pub fn default_bars() -> Vec<f64> {
    (0..=19).map(|i| f64::from(i) / 20.0).collect()
}

/// Sweep the thresholds over the judgments of one yes/no question.
///
/// A judgment takes part when it is labelled and carries a `yes`
/// probability (as [`Judgment::noul`] and [`Judgment::of_answer`] produce);
/// the others are skipped, so a Choice's judgments handed here by mistake
/// yield rows over zero cases rather than nonsense.
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

/// The threshold with the best F1; among equals, the lowest, because a
/// lower threshold says yes more often and a tie means the extra yeses
/// cost nothing in F1. `None` when no row has an F1 (nothing labelled yes).
pub fn best_threshold(rows: &[ThresholdRow]) -> Option<f64> {
    rows.iter()
        .filter_map(|row| row.f1.map(|f1| (f1, row.threshold)))
        .fold(
            None,
            |best: Option<(f64, f64)>, (f1, threshold)| match best {
                // Keep the incumbent unless this row's F1 is higher, or equal with
                // a lower threshold.
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

/// The gate table over the judgments of one Choice or Score: for each bar,
/// how many labelled judgments have a confidence at or above it and how
/// many of those are right.
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

/// The lowest bar whose covered answers are right at least
/// `target_accuracy` of the time, over at least `min_covered` judgments.
/// Lowest, because every step up hands more of the work to a person.
/// `None` when no bar reaches the target.
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
}
