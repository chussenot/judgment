//! Measuring judgments: recordings for replay, one graded [`Judgment`] per
//! answer and label, and per-question metrics for accuracy and calibration
//! (`docs/guides/record-replay-and-test.md` says what the module is for and what it leaves to the
//! application). A response recorded once is graded again under every
//! candidate policy without a model call; [`Recording`] is the file format.

pub mod canonical;
pub mod metrics;
pub mod tuning;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::answer::{Answer, BoxEdges, Confidence, Response, sanitize_server_str};
use crate::asset::Assets;
use crate::question::Questions;

/// Bins for expected calibration error.
pub const ECE_BINS: usize = 10;

/// The intersection over union a Box answer must reach against its
/// labelled box to count as right when the label sets no bar of its own
/// ([`Judgment::located`]): half, the detection benchmarks' usual bar
/// (PASCAL VOC), loose enough that a box a few pixels off is right and
/// strict enough that a box around the wrong thing is not.
pub const DEFAULT_IOU_AT_LEAST: f64 = 0.5;

/// A raw model response kept for replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    /// The case it answers, or the request hash under a [`crate::backend::Recorder`].
    pub case: String,
    /// The response as received.
    pub response: Response,
    /// Wall-clock time of the call.
    pub elapsed_ms: u64,
    /// [`request_hash`] of the request, set by a [`crate::backend::Recorder`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_hash: Option<String>,
    /// The request's [`canonical::request_fingerprint`], model excluded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    /// The rubric the questions came from, by id or fingerprint, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubric: Option<String>,
    /// The server that answered, as a base URL: two servers on one wire answer differently.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// When the call was made, RFC 3339 UTC: a `jev-latest` alias moves; the date says which.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
}

impl Recording {
    /// A recording of `response` for `case`, every optional field unset.
    pub fn new(case: impl Into<String>, response: Response, elapsed_ms: u64) -> Self {
        Self {
            case: case.into(),
            response,
            elapsed_ms,
            request_hash: None,
            fingerprint: None,
            rubric: None,
            server: None,
            recorded_at: None,
        }
    }
}

/// The current time as RFC 3339 UTC to the second, for [`Recording::recorded_at`].
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    rfc3339_from_unix(secs)
}

/// RFC 3339 for a UNIX timestamp in seconds, UTC, without a date dependency.
pub fn rfc3339_from_unix(secs: u64) -> String {
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // Howard Hinnant's `civil_from_days` (proleptic Gregorian, era from 0000-03-01).
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Reading or writing recordings.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file could not be read or written.
    #[error("cannot access {path}: {source}")]
    Io {
        /// The path.
        path: String,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
    /// A recording that does not parse.
    #[error("invalid recording {path}: {source}")]
    Json {
        /// The path.
        path: String,
        /// Cause.
        #[source]
        source: serde_json::Error,
    },
    /// No recording for the case.
    #[error("no recording for {case} (expected {path})")]
    MissingRecording {
        /// The case id.
        case: String,
        /// Where it was expected.
        path: String,
    },
    /// A case id that is not a name ([`is_name`]), so it cannot name a file
    /// under the directory; a path or a blank is refused before any access.
    #[error(
        "case id {case} is not a name (letters, digits, `.`, `_` and `-`, starting with a letter or a digit), so it cannot name a file under {dir}"
    )]
    NotAName {
        /// The id, escaped and cut.
        case: String,
        /// The directory it would have named a file in.
        dir: String,
    },
}

/// Result alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Where a recording lives: `dir/<case>.json`.
pub fn recording_path(dir: &Path, case: &str) -> PathBuf {
    dir.join(format!("{case}.json"))
}

/// Whether `id` is a name as the `.jud` format defines one (`docs/reference/jud-format.md`,
/// Names): ASCII letters, digits, `.`, `_` and `-`, starting with a letter or
/// a digit. A name is never a path, so a case id can name a file under a
/// directory and nothing else; [`write_recording`] and [`read_recording`]
/// refuse any other id.
pub fn is_name(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn not_a_name(dir: &Path, case: &str) -> Error {
    Error::NotAName {
        case: format!("{:?}", crate::answer::sanitize_server_str(case)),
        dir: dir.display().to_string(),
    }
}

/// Write `recording` under `dir` (created if needed), pretty-printed and newline-terminated,
/// whole: a temporary file beside it, renamed onto the name once written.
pub fn write_recording(dir: &Path, recording: &Recording) -> Result<PathBuf> {
    if !is_name(&recording.case) {
        return Err(not_a_name(dir, &recording.case));
    }
    std::fs::create_dir_all(dir).map_err(|source| Error::Io {
        path: dir.display().to_string(),
        source,
    })?;
    let path = recording_path(dir, &recording.case);
    let text = serde_json::to_string_pretty(recording).map_err(|source| Error::Json {
        path: path.display().to_string(),
        source,
    })?;
    write_atomically(&path, &(text + "\n")).map_err(|source| Error::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(path)
}

/// Write `path` so that no reader ever sees half of it: a [`Replay`] opened
/// over the directory while a [`Recorder`] writes into it, or after a crash
/// mid-write. The text goes to a temporary file beside it (one file system,
/// so the rename is atomic), is flushed, then takes the name. The temporary
/// name ends in `.tmp`, which a replay skips, and carries the process id and
/// a counter, so two writers of one request never share it. A failed write
/// removes its temporary file.
///
/// [`Replay`]: crate::backend::Replay
/// [`Recorder`]: crate::backend::Recorder
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temporary = path.with_file_name(format!(
        ".{name}.{}.{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let written = std::fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(text.as_bytes())?;
        file.sync_all()
    });
    let renamed = written.and_then(|()| std::fs::rename(&temporary, path));
    if renamed.is_err() {
        // Best effort: the error being returned is the one that matters.
        let _ = std::fs::remove_file(&temporary);
    }
    renamed
}

/// Read the recording for `case` under `dir`.
pub fn read_recording(dir: &Path, case: &str) -> Result<Recording> {
    if !is_name(case) {
        return Err(not_a_name(dir, case));
    }
    let path = recording_path(dir, case);
    let text = std::fs::read_to_string(&path).map_err(|_| Error::MissingRecording {
        case: case.to_owned(),
        path: path.display().to_string(),
    })?;
    serde_json::from_str(&text).map_err(|source| Error::Json {
        path: path.display().to_string(),
        source,
    })
}

/// The `z` of a 95% interval, the one the reports print.
pub const Z_95: f64 = 1.96;

/// The crate's own fingerprint of any JSON value, [`request_hash`]'s hash; it still names the
/// committed recordings, but a new tool should use [`canonical::fingerprint`].
pub fn fingerprint(value: &Value) -> String {
    let mut canonical = String::new();
    write_canonical(&mut canonical, value);
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

/// A content hash of `{"questions": …, "state": …}`, model excluded, that
/// names recording files: FNV-1a over key-sorted JSON, because it must not
/// change across runs, machines or toolchains and `DefaultHasher` promises
/// none of that. Not collision-resistant against an adversary.
/// [`request_hash_with`] for a request without images.
pub fn request_hash(state: &Value, questions: &Questions) -> String {
    request_hash_with(state, questions, &Assets::new())
}

/// [`request_hash`] of a request with its images, over the value
/// [`canonical::request_fingerprint_with`] fingerprints: the images by the
/// SHA-256 of their bytes under `assets`, the key left out without any, so
/// a request without images hashes as before.
pub fn request_hash_with(state: &Value, questions: &Questions, assets: &Assets) -> String {
    let mut canonical = String::new();
    write_canonical(
        &mut canonical,
        &canonical::request_value(state, questions, assets),
    );
    format!("{:016x}", fnv1a64(canonical.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Keys sorted by scalar value, numbers as `serde_json` prints them: not RFC 8785.
fn write_canonical(out: &mut String, value: &Value) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                write_canonical(out, &map[*key]);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(out, item);
            }
            out.push(']');
        }
        scalar => out.push_str(&serde_json::to_string(scalar).unwrap_or_default()),
    }
}

/// One graded judgment: what the model said, what the label said, how sure it was.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Judgment {
    /// What the model chose (or the application's reading of it).
    pub predicted: String,
    /// The label, when given.
    pub expected: Option<String>,
    /// `predicted == expected`, when labelled.
    pub correct: Option<bool>,
    /// Confidence in `predicted`: the Choice or Score confidence, `max(p, 1 - p)` for a Noul.
    pub confidence: f64,
    /// Probability the model put on the expected option, when labelled.
    pub p_expected: Option<f64>,
    /// The full distribution, keyed by option.
    pub probabilities: BTreeMap<String, f64>,
    /// False when the question was not asked and the answer is the implied one: graded but marked.
    pub asked: bool,
    /// For a Box graded against a labelled box ([`Judgment::located`]): the
    /// intersection over union, which decided `correct`. `None` for every
    /// other primitive and for an unlabelled Box.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iou: Option<f64>,
}

impl Judgment {
    /// Grade a prediction against a label over its distribution.
    pub fn new(
        predicted: String,
        expected: Option<String>,
        confidence: f64,
        probabilities: BTreeMap<String, f64>,
        asked: bool,
    ) -> Self {
        let correct = expected.as_ref().map(|e| *e == predicted);
        let p_expected = expected
            .as_ref()
            .map(|e| probabilities.get(e).copied().unwrap_or(0.0));
        Self {
            predicted,
            expected,
            correct,
            confidence,
            p_expected,
            probabilities,
            asked,
            iou: None,
        }
    }

    /// Grade a Box answer against a labelled box by intersection over union
    /// ([`BoxEdges::iou`]): right when it reaches `iou_at_least`
    /// ([`DEFAULT_IOU_AT_LEAST`] when the label sets none). The prediction
    /// and the label both read `box`, so the outcome counts stay one
    /// vocabulary per question; `iou` carries the number. A Box's
    /// confidence is the model's own and may be absent, which grades as 0:
    /// the model said nothing about its certainty. No distribution, so no
    /// Brier score.
    pub fn located(
        edges: &BoxEdges,
        expected: Option<&BoxEdges>,
        iou_at_least: f64,
        confidence: Option<f64>,
    ) -> Self {
        let iou = expected.map(|label| edges.iou(label));
        Self {
            predicted: "box".to_owned(),
            expected: expected.map(|_| "box".to_owned()),
            correct: iou.map(|iou| iou >= iou_at_least),
            confidence: confidence.unwrap_or(0.0),
            p_expected: None,
            probabilities: BTreeMap::new(),
            asked: true,
            iou,
        }
    }

    /// Grade a yes/no probability against a bool label: `yes` from 0.5, confidence `max(p, 1 - p)`.
    pub fn noul(p_yes: f64, expected: Option<bool>, asked: bool) -> Self {
        let predicted = if p_yes >= 0.5 { "yes" } else { "no" };
        Self::new(
            predicted.to_owned(),
            expected.map(|b| if b { "yes".to_owned() } else { "no".to_owned() }),
            p_yes.max(1.0 - p_yes),
            BTreeMap::from([("yes".to_owned(), p_yes), ("no".to_owned(), 1.0 - p_yes)]),
            asked,
        )
    }

    /// Grade a wire answer as returned (a Choice by option key, a Score by level index as a
    /// string, a Noul as `yes` or `no`); an application that names its levels uses [`Self::new`].
    /// A Box has no label in words: it reads `box`, ungraded (`correct` is
    /// `None`), and is graded by [`Judgment::located`] against a labelled box.
    ///
    /// An [`Answer::Unknown`] is a miss, not dropped, since dropping it would
    /// raise the accuracy of a model whose answers cannot be read: predicted
    /// `<kind>`, confidence 0.0, no distribution. Its (0.0, wrong) pair pulls
    /// the ECE toward zero; accepted because the `<kind>` stands out in the
    /// confusion matrix and only an unverified response can bring one here.
    pub fn of_answer(answer: &Answer, expected: Option<&str>) -> Self {
        match answer {
            Answer::Unknown(_) => Self {
                predicted: format!("<{}>", sanitize_server_str(answer.kind())),
                expected: expected.map(str::to_owned),
                correct: expected.map(|_| false),
                confidence: 0.0,
                p_expected: expected.map(|_| 0.0),
                probabilities: BTreeMap::new(),
                asked: true,
                iou: None,
            },
            Answer::Box { edges, confidence } => {
                let mut graded = Self::located(
                    edges,
                    None,
                    DEFAULT_IOU_AT_LEAST,
                    confidence.map(Confidence::value),
                );
                graded.expected = expected.map(str::to_owned);
                graded
            }
            Answer::Noul { noul } => Self::noul(noul.value(), expected.map(|e| e == "yes"), true),
            Answer::Choice {
                choice,
                probabilities,
                confidence,
            } => Self::new(
                choice.clone(),
                expected.map(str::to_owned),
                confidence.value(),
                probabilities
                    .iter()
                    .map(|(k, p)| (k.clone(), p.value()))
                    .collect(),
                true,
            ),
            Answer::Score {
                probabilities,
                confidence,
                ..
            } => {
                let probs: BTreeMap<String, f64> = probabilities
                    .iter()
                    .map(|(k, p)| (k.clone(), p.value()))
                    .collect();
                let predicted = probs
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(k, _)| k.clone())
                    .unwrap_or_default();
                Self::new(
                    predicted,
                    expected.map(str::to_owned),
                    confidence.value(),
                    probs,
                    true,
                )
            }
        }
    }
}

/// Metrics for one question over its labelled judgments.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct QuestionMetrics {
    /// Judgments with a label.
    pub labelled: usize,
    /// Correct predictions.
    pub correct: usize,
    /// `correct / labelled`.
    pub accuracy: Option<f64>,
    /// 95% [Wilson interval](metrics::wilson_interval) around `accuracy`, `(low, high)`.
    pub accuracy_interval95: Option<(f64, f64)>,
    /// Mean multi-class [Brier score](metrics::brier) (0 perfect, 2 worst).
    pub brier: Option<f64>,
    /// [Expected calibration error](metrics::expected_calibration_error) of the confidence.
    pub ece: Option<f64>,
    /// Mean confidence when right.
    pub confidence_when_right: Option<f64>,
    /// Mean confidence when wrong.
    pub confidence_when_wrong: Option<f64>,
    /// `expected -> predicted -> count`.
    pub confusion: BTreeMap<String, BTreeMap<String, usize>>,
    /// Mean intersection over union of the labelled Box judgments
    /// ([`Judgment::iou`]); `None` for any other primitive.
    pub mean_iou: Option<f64>,
}

impl QuestionMetrics {
    /// Aggregate one question's labelled judgments. A label the model never
    /// offered adds 1.0 to that Brier score (the missing `(1 - 0)²` term), so
    /// it hurts rather than vanishes. `ece_bins` is usually [`ECE_BINS`].
    #[allow(clippy::cast_precision_loss)] // counts, far below 2^52
    pub fn summarise<'a>(
        judgments: impl IntoIterator<Item = &'a Judgment>,
        ece_bins: usize,
    ) -> Self {
        let mut m = Self::default();
        let mut brier = Vec::new();
        let mut calib = Vec::new();
        let mut right = Vec::new();
        let mut wrong = Vec::new();
        let mut ious = Vec::new();
        for j in judgments {
            let (Some(expected), Some(correct)) = (&j.expected, j.correct) else {
                continue;
            };
            m.labelled += 1;
            if let Some(iou) = j.iou {
                ious.push(iou);
            }
            if correct {
                m.correct += 1;
                right.push(j.confidence);
            } else {
                wrong.push(j.confidence);
            }
            *m.confusion
                .entry(expected.clone())
                .or_default()
                .entry(j.predicted.clone())
                .or_default() += 1;
            let pairs: Vec<(bool, f64)> = j
                .probabilities
                .iter()
                .map(|(k, p)| (k == expected, *p))
                .collect();
            // A Box is graded by its IoU and has no distribution to score.
            if j.iou.is_none() {
                let mut b = metrics::brier(&pairs);
                if !j.probabilities.contains_key(expected) {
                    b += 1.0;
                }
                brier.push(b);
            }
            calib.push((j.confidence, correct));
        }
        if m.labelled > 0 {
            m.accuracy = Some(m.correct as f64 / m.labelled as f64);
            m.accuracy_interval95 = metrics::wilson_interval(m.correct, m.labelled, Z_95);
        }
        m.brier = metrics::mean(&brier);
        m.ece = (!calib.is_empty()).then(|| metrics::expected_calibration_error(&calib, ece_bins));
        m.confidence_when_right = metrics::mean(&right);
        m.confidence_when_wrong = metrics::mean(&wrong);
        m.mean_iou = metrics::mean(&ious);
        m
    }
}

/// Latency summary in milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct Latency {
    /// Median.
    pub p50_ms: Option<f64>,
    /// 95th percentile.
    pub p95_ms: Option<f64>,
    /// Mean.
    pub mean_ms: Option<f64>,
}

impl Latency {
    /// Summarise call times in milliseconds.
    pub fn of(elapsed_ms: &[f64]) -> Self {
        Self {
            p50_ms: metrics::percentile(elapsed_ms, 0.5),
            p95_ms: metrics::percentile(elapsed_ms, 0.95),
            mean_ms: metrics::mean(elapsed_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::answer::{Confidence, Probability, Usage};
    use serde_json::json;

    #[test]
    fn fingerprint_is_canonical_and_shares_the_recording_hash() {
        let one = json!({ "x": 1, "y": { "b": 2, "a": 1 } });
        let two = json!({ "y": { "a": 1, "b": 2 }, "x": 1 });
        assert_eq!(fingerprint(&one), fingerprint(&two));
        assert_ne!(fingerprint(&one), fingerprint(&json!({ "x": 2 })));
        assert_eq!(fingerprint(&one).len(), 16);
        let mut q = Questions::new();
        q.noul("a", "Is `x` set?", None).unwrap();
        let questions = serde_json::to_value(&q).unwrap();
        assert_eq!(
            request_hash(&one, &q),
            fingerprint(&json!({ "questions": questions, "state": one }))
        );
    }

    /// A replay opened while recordings are being written reads each one
    /// whole: before the write or after it, never half of it.
    #[test]
    fn write_recording_never_exposes_a_partial_file() {
        let dir = std::env::temp_dir().join(format!("judgment-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // A model name long enough that a write takes more than one syscall.
        let response = Response {
            model: "m".repeat(256 * 1024),
            answers: BTreeMap::new(),
            usage: Usage::default(),
            request_id: None,
            extra: BTreeMap::new(),
        };
        let mut recording = Recording::new("case", response, 1);
        recording.request_hash = Some("0123456789abcdef".to_owned());
        write_recording(&dir, &recording).unwrap();
        let done = std::sync::atomic::AtomicBool::new(false);
        let failures = std::thread::scope(|scope| {
            let reader = scope.spawn(|| {
                let mut failures = 0;
                while !done.load(std::sync::atomic::Ordering::Relaxed) {
                    failures += usize::from(crate::backend::Replay::open(&dir).is_err());
                }
                failures
            });
            for elapsed_ms in 0..100 {
                recording.elapsed_ms = elapsed_ms;
                write_recording(&dir, &recording).unwrap();
            }
            done.store(true, std::sync::atomic::Ordering::Relaxed);
            reader.join().unwrap()
        });
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(failures, 0, "a replay read half a recording");
        assert_eq!(left, ["case.json"], "no temporary file is left behind");
    }

    #[test]
    fn summarise_reports_a_wilson_interval_around_accuracy() {
        let js: Vec<Judgment> = (0..3)
            .map(|_| Judgment::noul(0.9, Some(true), true))
            .collect();
        let m = QuestionMetrics::summarise(&js, ECE_BINS);
        assert_eq!(m.accuracy, Some(1.0));
        let (low, high) = m.accuracy_interval95.unwrap();
        assert!((0.43..0.44).contains(&low), "{low}");
        assert!((high - 1.0).abs() < 1e-12, "{high}");
        assert_eq!(
            QuestionMetrics::summarise(&[], ECE_BINS).accuracy_interval95,
            None
        );
    }

    #[test]
    fn request_hash_ignores_key_order_and_sees_content() {
        let mut q = Questions::new();
        q.noul("a", "Is `x` set?", None).unwrap();
        let one = json!({ "x": 1, "y": { "b": 2, "a": 1 } });
        let two = json!({ "y": { "a": 1, "b": 2 }, "x": 1 });
        assert_eq!(request_hash(&one, &q), request_hash(&two, &q));
        assert_ne!(request_hash(&json!({ "x": 2 }), &q), request_hash(&one, &q));
        let mut q2 = Questions::new();
        q2.noul("a", "Is `x` unset?", None).unwrap();
        assert_ne!(request_hash(&one, &q2), request_hash(&one, &q));
        assert_eq!(request_hash(&one, &q).len(), 16);
    }

    #[test]
    fn recordings_round_trip_with_and_without_a_hash() {
        let dir = std::env::temp_dir().join(format!("judgment-rec-{}", std::process::id()));
        let response = Response {
            model: "m".into(),
            answers: BTreeMap::from([
                (
                    "a".to_owned(),
                    Answer::Noul {
                        noul: Probability::new(0.7).unwrap(),
                    },
                ),
                // An unknown kind and an undocumented field both survive.
                (
                    "b".to_owned(),
                    Answer::Unknown(json!({ "type": "rank", "ranking": ["x", "y"] })),
                ),
            ]),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 2,
                cost: None,
            },
            request_id: None,
            extra: BTreeMap::from([("routing".to_owned(), json!({ "model": "typed-decisions" }))]),
        };
        let keyed = Recording::new("case-1", response.clone(), 5);
        write_recording(&dir, &keyed).unwrap();
        let text = std::fs::read_to_string(recording_path(&dir, "case-1")).unwrap();
        assert!(!text.contains("request_hash"));
        assert!(text.ends_with('\n'));
        assert!(text.contains(r#""routing": {"#), "{text}");
        assert!(text.contains(r#""type": "rank""#), "{text}");
        assert_eq!(read_recording(&dir, "case-1").unwrap(), keyed);
        assert!(matches!(
            read_recording(&dir, "ghost"),
            Err(Error::MissingRecording { case, .. }) if case == "ghost"
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 1.2 (decision 0017): a case id that is not a name never reaches the
    /// file system, so a document cannot name a file outside the directory.
    #[test]
    fn a_case_id_that_is_a_path_is_refused_before_any_access() {
        let dir = std::env::temp_dir().join(format!("judgment-name-{}", std::process::id()));
        let response = Response {
            model: "m".into(),
            answers: BTreeMap::new(),
            usage: Usage::default(),
            request_id: None,
            extra: BTreeMap::new(),
        };
        for bad in [
            "../escape",
            "/etc/passwd",
            "a/b",
            "",
            ".hidden",
            "a b",
            "tab\t",
        ] {
            assert!(!is_name(bad), "{bad:?}");
            let err = write_recording(&dir, &Recording::new(bad, response.clone(), 1)).unwrap_err();
            assert!(matches!(err, Error::NotAName { .. }), "{bad:?}: {err}");
            let err = read_recording(&dir, bad).unwrap_err();
            assert!(matches!(err, Error::NotAName { .. }), "{bad:?}: {err}");
        }
        assert!(!dir.exists(), "nothing was created");
        for good in ["refund-angry", "T-98423", "2c8d503947d3289b", "v1.2_final"] {
            assert!(is_name(good), "{good}");
        }
    }

    #[test]
    fn a_recording_made_before_0_2_reads_with_no_request_id() {
        let text = r#"{
          "case": "0123456789abcdef",
          "response": {
            "model": "typed-decisions",
            "answers": { "a": { "type": "noul", "noul": 0.25 } },
            "usage": { "input_tokens": 7, "output_tokens": 1 }
          },
          "elapsed_ms": 12,
          "request_hash": "0123456789abcdef"
        }"#;
        let recording: Recording = serde_json::from_str(text).unwrap();
        assert_eq!(recording.response.request_id, None);
        let again = serde_json::to_string(&recording).unwrap();
        assert!(!again.contains("request_id"), "{again}");
    }

    #[test]
    fn judgments_grade_each_primitive_and_metrics_summarise_them() {
        let choice = Answer::Choice {
            choice: "billing".into(),
            probabilities: BTreeMap::from([
                ("billing".to_owned(), Probability::new(0.8).unwrap()),
                ("technical".to_owned(), Probability::new(0.2).unwrap()),
            ]),
            confidence: Confidence::new(0.7).unwrap(),
        };
        let right = Judgment::of_answer(&choice, Some("billing"));
        assert_eq!(right.correct, Some(true));
        assert_eq!(right.p_expected, Some(0.8));
        let wrong = Judgment::of_answer(&choice, Some("sales"));
        assert_eq!(wrong.correct, Some(false));
        assert_eq!(wrong.p_expected, Some(0.0));
        let unlabelled = Judgment::of_answer(&choice, None);
        assert_eq!(unlabelled.correct, None);

        let score = Answer::Score {
            score: 1.9,
            legend: BTreeMap::new(),
            probabilities: BTreeMap::from([
                ("0".to_owned(), Probability::new(0.1).unwrap()),
                ("1".to_owned(), Probability::new(0.2).unwrap()),
                ("2".to_owned(), Probability::new(0.7).unwrap()),
            ]),
            confidence: Confidence::new(0.6).unwrap(),
        };
        assert_eq!(Judgment::of_answer(&score, Some("2")).predicted, "2");
        let noul = Answer::Noul {
            noul: Probability::new(0.3).unwrap(),
        };
        let n = Judgment::of_answer(&noul, Some("no"));
        assert_eq!(n.predicted, "no");
        assert!((n.confidence - 0.7).abs() < 1e-12);

        let m = QuestionMetrics::summarise([&right, &wrong, &unlabelled], ECE_BINS);
        assert_eq!(m.labelled, 2);
        assert_eq!(m.correct, 1);
        assert!((m.accuracy.unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(m.confusion["sales"]["billing"], 1);
        // `wrong` expected an option never offered: plain Brier plus one.
        let plain = metrics::brier(&[(false, 0.8), (false, 0.2)]);
        let expected_brier = (metrics::brier(&[(true, 0.8), (false, 0.2)]) + plain + 1.0) / 2.0;
        assert!((m.brier.unwrap() - expected_brier).abs() < 1e-12);
        assert_eq!(m.confidence_when_right, Some(0.7));
        assert_eq!(m.confidence_when_wrong, Some(0.7));

        let l = Latency::of(&[10.0, 30.0, 20.0]);
        assert_eq!(l.p50_ms, Some(20.0));
        assert_eq!(l.mean_ms, Some(20.0));
    }

    #[test]
    fn a_box_is_graded_by_iou_and_summarised_without_a_brier_score() {
        let label = BoxEdges::new(0.2, 0.8, 0.2, 0.8);
        let close = BoxEdges::new(0.25, 0.85, 0.25, 0.85);
        let far = BoxEdges::new(0.0, 0.3, 0.0, 0.3);
        let hit = Judgment::located(&close, Some(&label), DEFAULT_IOU_AT_LEAST, Some(0.9));
        assert_eq!(hit.predicted, "box");
        assert_eq!(hit.expected.as_deref(), Some("box"));
        assert_eq!(hit.correct, Some(true));
        assert!((hit.iou.unwrap() - close.iou(&label)).abs() < 1e-12);
        assert!((hit.confidence - 0.9).abs() < f64::EPSILON);
        assert!(hit.probabilities.is_empty());
        let miss = Judgment::located(&far, Some(&label), DEFAULT_IOU_AT_LEAST, None);
        assert_eq!(miss.correct, Some(false));
        assert!(
            miss.confidence.abs() < f64::EPSILON,
            "no confidence grades as none"
        );
        let strict = Judgment::located(&close, Some(&label), 0.9, Some(0.9));
        assert_eq!(strict.correct, Some(false), "the label's own bar");
        let unlabelled = Judgment::located(&close, None, DEFAULT_IOU_AT_LEAST, Some(0.9));
        assert_eq!((unlabelled.correct, unlabelled.iou), (None, None));
        let shown = serde_json::to_value(&hit).unwrap();
        assert!(shown["iou"].is_number());
        assert!(
            serde_json::to_value(&unlabelled)
                .unwrap()
                .get("iou")
                .is_none()
        );

        let m = QuestionMetrics::summarise([&hit, &miss, &unlabelled], ECE_BINS);
        assert_eq!((m.labelled, m.correct), (2, 1));
        assert_eq!(m.brier, None, "no distribution to score");
        let mean = f64::midpoint(close.iou(&label), far.iou(&label));
        assert!((m.mean_iou.unwrap() - mean).abs() < 1e-12);
        assert_eq!(m.confusion["box"]["box"], 2);
        assert_eq!(m.confidence_when_right, Some(0.9));
        assert_eq!(m.confidence_when_wrong, Some(0.0));
        // `of_answer` cannot grade a Box: no label in words.
        let answer = Answer::Box {
            edges: close,
            confidence: Some(Confidence::new(0.7).unwrap()),
        };
        let graded = Judgment::of_answer(&answer, Some("box"));
        assert_eq!(
            (graded.predicted.as_str(), graded.correct, graded.iou),
            ("box", None, None)
        );
        assert!((graded.confidence - 0.7).abs() < f64::EPSILON);
        let other = QuestionMetrics::summarise([&Judgment::noul(0.9, Some(true), true)], ECE_BINS);
        assert_eq!(other.mean_iou, None);
    }

    #[test]
    fn an_unknown_answer_is_graded_as_a_miss_not_dropped() {
        let rank = Answer::Unknown(json!({ "type": "rank", "ranking": ["billing"] }));
        for label in ["", "billing"] {
            let j = Judgment::of_answer(&rank, Some(label));
            assert_eq!(j.predicted, "<rank>");
            assert_eq!(j.expected.as_deref(), Some(label));
            assert_eq!(j.correct, Some(false), "{label:?}");
            assert_eq!(j.p_expected, Some(0.0), "{label:?}");
            assert!(j.confidence.abs() < f64::EPSILON);
            assert!(j.probabilities.is_empty());
            assert!(j.asked);
        }
        let unlabelled = Judgment::of_answer(&rank, None);
        assert_eq!(unlabelled.correct, None);
        assert_eq!(unlabelled.p_expected, None);
        // A hostile kind is escaped in the prediction, as in an error.
        let hostile = Answer::Unknown(json!({ "type": format!("a\nb{}", "x".repeat(100)) }));
        let predicted = Judgment::of_answer(&hostile, None).predicted;
        assert!(!predicted.contains('\n'), "{predicted:?}");
        assert!(predicted.chars().count() <= 66, "{predicted:?}");

        let m =
            QuestionMetrics::summarise([&Judgment::of_answer(&rank, Some("billing"))], ECE_BINS);
        assert_eq!((m.labelled, m.correct), (1, 0));
        assert_eq!(m.accuracy, Some(0.0));
        assert_eq!(m.brier, Some(1.0));
        assert_eq!(m.ece, Some(0.0));
        assert_eq!(m.confidence_when_wrong, Some(0.0));
        assert_eq!(m.confidence_when_right, None);
        assert_eq!(m.confusion["billing"]["<rank>"], 1);
    }
}
