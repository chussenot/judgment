//! Scoring rules for calibrated judgments: pure functions over probabilities and outcomes.

// Counts and bin indices are small; the casts to and from f64 are exact in
// practice and truncation is the intended bucketing.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

/// Multi-class Brier score (Brier 1950): `Σ (p_i − y_i)²` over the options against the
/// one-hot truth, `0.0` perfect, `2.0` full confidence in the wrong option. Other tools halve
/// it or report the binary form, so figures are not comparable without the definition.
pub fn brier(probabilities: &[(bool, f64)]) -> f64 {
    probabilities
        .iter()
        .map(|(truth, p)| {
            let y = if *truth { 1.0 } else { 0.0 };
            (p - y).powi(2)
        })
        .sum()
}

/// Expected calibration error (Naeini et al. 2015) over `(confidence,
/// correct)` pairs: confidence bucketed into `bins` equal-width bins over
/// `[0, 1]`, `Σ_b (n_b / n) · |mean confidence_b − accuracy_b|` over the
/// non-empty bins. `0.0` means a 0.8 confidence was right 80% of the time.
pub fn expected_calibration_error(pairs: &[(f64, bool)], bins: usize) -> f64 {
    if pairs.is_empty() || bins == 0 {
        return 0.0;
    }
    let mut sum_conf = vec![0.0; bins];
    let mut sum_correct = vec![0.0; bins];
    let mut count = vec![0usize; bins];
    for (conf, correct) in pairs {
        let c = conf.clamp(0.0, 1.0);
        let b = bin_index(c, bins);
        sum_conf[b] += c;
        sum_correct[b] += if *correct { 1.0 } else { 0.0 };
        count[b] += 1;
    }
    let n = pairs.len() as f64;
    (0..bins)
        .filter(|b| count[*b] > 0)
        .map(|b| {
            let k = count[b] as f64;
            (k / n) * ((sum_conf[b] / k) - (sum_correct[b] / k)).abs()
        })
        .sum()
}

/// `floor(c × bins)`, with 1.0 in the last bin and NaN or a negative value
/// in the first; always below `bins` (bounded proof below).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn bin_index(c: f64, bins: usize) -> usize {
    ((c * bins as f64).floor() as usize).min(bins.saturating_sub(1))
}

/// Wilson score interval (Wilson 1927) for `correct / n` at `z` (1.96 for 95%), `(low, high)`
/// in `[0, 1]`; `None` at `n = 0`. Chosen over the normal approximation because it stays in
/// `[0, 1]` and does not collapse to a point at 0 of 3 or 3 of 3, the extremes a small
/// labelled set produces; it assumes independent observations.
pub fn wilson_interval(correct: usize, n: usize, z: f64) -> Option<(f64, f64)> {
    if n == 0 {
        return None;
    }
    let n = n as f64;
    let p = correct as f64 / n;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let centre = (p + z2 / (2.0 * n)) / denominator;
    let half = z * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt() / denominator;
    Some(((centre - half).max(0.0), (centre + half).min(1.0)))
}

/// Arithmetic mean; `None` when empty.
pub fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

/// Nearest-rank percentile (`0.0..=1.0`) of unsorted values; `None` when empty.
pub fn percentile(values: &[f64], q: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(v[nearest_rank(q, v.len())])
}

/// Rank `ceil(q × len)` clamped to `1..=len`, minus one; `q` clamped to
/// `[0, 1]` and NaN read as 0. Always below `len` (bounded proof below).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn nearest_rank(q: f64, len: usize) -> usize {
    let rank = ((q.clamp(0.0, 1.0) * len as f64).ceil() as usize).clamp(1, len.max(1));
    rank - 1
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn brier_is_zero_when_certain_and_right_and_two_when_certain_and_wrong() {
        assert!((brier(&[(true, 1.0), (false, 0.0)]) - 0.0).abs() < 1e-12);
        assert!((brier(&[(true, 0.0), (false, 1.0)]) - 2.0).abs() < 1e-12);
        // Uniform over two options: (0.5-1)^2 + (0.5-0)^2 = 0.5
        assert!((brier(&[(true, 0.5), (false, 0.5)]) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn ece_is_zero_for_a_calibrated_model_and_large_for_an_overconfident_one() {
        // 80% confidence, right 4 times out of 5: a perfectly calibrated bin.
        let calibrated = [
            (0.8, true),
            (0.8, true),
            (0.8, true),
            (0.8, true),
            (0.8, false),
        ];
        assert!(expected_calibration_error(&calibrated, 10).abs() < 1e-12);
        // 95% confidence, right half the time.
        let over = [(0.95, true), (0.95, false)];
        assert!((expected_calibration_error(&over, 10) - 0.45).abs() < 1e-12);
        assert!(expected_calibration_error(&[], 10).abs() < 1e-12);
    }

    #[test]
    fn percentiles_use_nearest_rank() {
        let v = [5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!(percentile(&v, 0.5), Some(3.0));
        assert_eq!(percentile(&v, 0.95), Some(5.0));
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(mean(&v), Some(3.0));
    }

    #[test]
    fn wilson_stays_inside_the_unit_interval_at_the_extremes() {
        assert_eq!(wilson_interval(0, 0, 1.96), None);
        let (l, h) = wilson_interval(0, 3, 1.96).unwrap();
        assert!(l.abs() < 1e-12, "{l}");
        assert!((0.56..0.57).contains(&h), "{h}");
        let (l, h) = wilson_interval(3, 3, 1.96).unwrap();
        assert!((0.43..0.44).contains(&l), "{l}");
        assert!((h - 1.0).abs() < 1e-12, "{h}");
        let (l, h) = wilson_interval(50, 100, 1.96).unwrap();
        assert!(l < 0.5 && 0.5 < h && h - l < 0.21, "{l} {h}");
    }
}

/// Bounded proofs of the indexing, run with `cargo kani` (docs/testing.md, "Bounded proofs").
#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Below `bins` for any `f64` and any `bins >= 1`; 0 at or below 0 or
    /// NaN; the last bin from 1 up while `bins` is exact as an `f64` (2^53).
    #[kani::proof]
    fn bin_index_is_a_bin() {
        let c: f64 = kani::any();
        let bins: usize = kani::any();
        kani::assume(bins >= 1);

        let b = bin_index(c, bins);

        assert!(b < bins);
        if c.is_nan() || c <= 0.0 {
            assert_eq!(b, 0);
        }
        // Past 2^53 `bins as f64` can round down; Kani finds a counterexample.
        if c >= 1.0 && bins <= 1 << 53 {
            assert_eq!(b, bins - 1);
        }
        kani::cover!(bins == 10 && c == 1.0);
        kani::cover!(bins == 10 && b == 4);
    }

    /// Below `len` for any `q` and any `len >= 1`; 0 at `q <= 0` or NaN;
    /// the largest at `q >= 1` while `len` is exact as an `f64` (2^53).
    #[kani::proof]
    fn nearest_rank_is_an_index_into_the_values() {
        let q: f64 = kani::any();
        let len: usize = kani::any();
        kani::assume(len >= 1);

        let index = nearest_rank(q, len);

        assert!(index < len);
        if q.is_nan() || q <= 0.0 {
            assert_eq!(index, 0);
        }
        // Past 2^53 `len as f64` can round down; Kani finds a counterexample.
        if q >= 1.0 && len <= 1 << 53 {
            assert_eq!(index, len - 1);
        }
        kani::cover!(len == 20 && q == 0.95 && index == 18);
    }
}
