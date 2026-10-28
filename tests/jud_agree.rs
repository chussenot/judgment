//! The reader, the JSON Schemas and the writer accept and refuse the same
//! documents (`docs/reference/jud-format.md`, Schemas and conformance).
//!
//! Every committed document is edited one node at a time, every way a hand
//! or another tool edits one: a key deleted, a value nulled, retyped,
//! emptied, reordered or repeated, an unknown key inserted, a string's
//! bytes changed, a key written as a number. Each edit is read by the
//! reader (`R`) and validated by the schema for its kind (`S`); the two
//! agree, except where the page says a schema cannot state the check (the
//! cross-checks, named in [`cross_check`]). What both accept is written
//! back (`W`), and that validates, reads back as the same document and
//! keeps its fingerprints.
//!
//! The sweep is exhaustive over single edits rather than sampled: the
//! space is finite and a disagreement is a class, so every member is
//! listed. The classes it found are each a bead, pinned by an ignored test
//! below ([`FOUND`]); a new class fails the main test.

#![cfg(feature = "jud")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use judgment::jud::{self, Document, recording_to_yaml};
use serde_json::{Map, Value, json};

const SCHEMAS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/schemas/jud");
const EXAMPLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
const BASE: &str = "https://github.com/chussenot/judgment/schemas/jud/";

struct Local;

impl jsonschema::Retrieve for Local {
    fn retrieve(
        &self,
        uri: &jsonschema::Uri<String>,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let name = uri
            .as_str()
            .strip_prefix(BASE)
            .ok_or_else(|| format!("unexpected $ref target {uri}"))?;
        Ok(serde_json::from_str(&std::fs::read_to_string(
            Path::new(SCHEMAS).join(name),
        )?)?)
    }
}

fn validator(kind: &str) -> jsonschema::Validator {
    let schema: Value = serde_json::from_str(
        &std::fs::read_to_string(Path::new(SCHEMAS).join(format!("{kind}.schema.json"))).unwrap(),
    )
    .unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap()
}

static RUBRIC: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator("rubric"));
static CASES: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator("cases"));
static RECORDING: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator("recording"));

/// `S(d)`: the schema of the kind `d` names accepts it; no kind, no schema.
fn schema_accepts(d: &Value) -> bool {
    match d.get("kind").and_then(Value::as_str) {
        Some("Rubric") => RUBRIC.is_valid(d),
        Some("Cases") => CASES.is_valid(d),
        Some("Recording") => RECORDING.is_valid(d),
        _ => false,
    }
}

/// `W(d)`: the document written back.
fn write(document: &Document) -> String {
    match document {
        Document::Rubric(r) => r.to_yaml().unwrap(),
        Document::Cases(c) => c.to_yaml().unwrap(),
        Document::Recording(r) => recording_to_yaml(r).unwrap(),
        _ => unreachable!("three kinds"),
    }
}

fn fingerprints(document: &Document) -> Vec<String> {
    match document {
        Document::Rubric(r) => vec![r.fingerprint(), r.policy_fingerprint()],
        Document::Cases(c) => vec![c.fingerprint()],
        _ => vec![],
    }
}

/// Every `.jud` under `examples/jud/`, and the first recording of each
/// directory under `examples/recordings/` (the rest of a directory has the
/// same shape), as JSON values.
static SEEDS: LazyLock<Vec<(String, Value)>> = LazyLock::new(|| {
    let sorted = |dir: &Path| {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        entries
    };
    let is_jud = |p: &PathBuf| p.extension().is_some_and(|e| e == "jud");
    let mut files: Vec<PathBuf> = sorted(&Path::new(EXAMPLES).join("jud"))
        .into_iter()
        .filter(is_jud)
        .collect();
    for dir in sorted(&Path::new(EXAMPLES).join("recordings")) {
        files.extend(sorted(&dir).into_iter().find(is_jud));
    }
    files
        .into_iter()
        .map(|p| {
            let value = serde_saphyr::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            let name = p.strip_prefix(EXAMPLES).unwrap().display().to_string();
            (name, value)
        })
        .collect()
});

/// A step into a value: a key of an object or an index of an array.
#[derive(Debug, Clone)]
enum Step {
    Key(String),
    At(usize),
}

/// Every node's path, the root's included, save what lies past an array's
/// second item: the third case or turn has the shape of the second.
fn paths(value: &Value) -> Vec<Vec<Step>> {
    let mut out = vec![vec![]];
    let mut i = 0;
    while i < out.len() {
        let here = out[i].clone();
        let children: Vec<Step> = match node(value, &here) {
            Value::Object(m) => m.keys().cloned().map(Step::Key).collect(),
            Value::Array(a) => (0..a.len().min(2)).map(Step::At).collect(),
            _ => vec![],
        };
        out.extend(children.into_iter().map(|s| {
            let mut p = here.clone();
            p.push(s);
            p
        }));
        i += 1;
    }
    out
}

fn node<'v>(value: &'v Value, path: &[Step]) -> &'v Value {
    path.iter().fold(value, |v, s| match s {
        Step::Key(k) => &v[k.as_str()],
        Step::At(n) => &v[*n],
    })
}

fn node_mut<'v>(value: &'v mut Value, path: &[Step]) -> &'v mut Value {
    path.iter().fold(value, |v, s| match s {
        Step::Key(k) => &mut v[k.as_str()],
        Step::At(n) => &mut v[*n],
    })
}

/// The key a number-key edit writes, as YAML reads it: an integer.
const NUMBER_KEY: &str = "7";

/// One edit to one node.
#[derive(Debug, Clone)]
enum Edit {
    /// Remove the node from its object or array.
    Delete,
    /// Put this value in its place: null, another type, an emptied container.
    Replace(Value),
    /// Add a key no kind defines to an object.
    Unknown,
    /// Reverse an object's keys or an array's items.
    Reverse,
    /// Append a copy of an array's first item.
    Repeat,
    /// A string with a byte changed.
    Bytes(&'static str),
    /// The node's key renamed to [`NUMBER_KEY`] and written unquoted, so
    /// YAML reads an integer key: a key is its written text.
    NumberKey,
}

fn edits() -> Vec<Edit> {
    let mut edits = vec![Edit::Delete, Edit::Unknown, Edit::Reverse, Edit::Repeat];
    edits.extend(
        [
            Value::Null,
            json!("text"),
            json!(7),
            json!(-1),
            json!(1.5),
            json!(4.0),
            json!(true),
            json!([]),
            json!({"k": "v"}),
        ]
        .into_iter()
        .map(Edit::Replace),
    );
    edits.extend(
        ["suffix", "space", "empty", "case"]
            .into_iter()
            .map(Edit::Bytes),
    );
    edits
}

fn changed(s: &str, how: &str) -> String {
    match how {
        "suffix" => format!("{s}x"),
        "space" => format!("{s} "),
        "empty" => String::new(),
        _ => {
            let mut c = s.chars();
            c.next().map_or_else(String::new, |f| {
                let flipped: String = if f.is_uppercase() {
                    f.to_lowercase().collect()
                } else {
                    f.to_uppercase().collect()
                };
                flipped + c.as_str()
            })
        }
    }
}

/// The parent object of `path` with `f` applied to its entries, order kept.
fn rebuild(
    out: &mut Value,
    path: &[Step],
    f: impl FnMut((String, Value)) -> Option<(String, Value)>,
) -> Option<()> {
    let (Step::Key(_), parent) = path.split_last()? else {
        return None;
    };
    let m = node_mut(out, parent).as_object_mut()?;
    *m = std::mem::take(m).into_iter().filter_map(f).collect();
    Some(())
}

/// `edit` applied at `path` as the text a reader is given and the value the
/// schema is given, or `None` when it does not apply there.
fn apply(value: &Value, path: &[Step], edit: &Edit) -> Option<(String, Value)> {
    let mut out = value.clone();
    match edit {
        Edit::Delete => match path.last()? {
            Step::Key(k) => rebuild(&mut out, path, |(key, v)| (&key != k).then_some((key, v)))?,
            Step::At(n) => {
                node_mut(&mut out, &path[..path.len() - 1])
                    .as_array_mut()?
                    .remove(*n);
            }
        },
        Edit::Replace(with) => *node_mut(&mut out, path) = with.clone(),
        Edit::Unknown => {
            node_mut(&mut out, path)
                .as_object_mut()?
                .insert("x_unknown".to_owned(), json!("v"));
        }
        Edit::Reverse => match node_mut(&mut out, path) {
            Value::Object(m) => *m = std::mem::take(m).into_iter().rev().collect::<Map<_, _>>(),
            Value::Array(a) => a.reverse(),
            _ => return None,
        },
        Edit::Repeat => {
            let a = node_mut(&mut out, path).as_array_mut()?;
            let first = a.first()?.clone();
            a.push(first);
        }
        Edit::Bytes(how) => {
            let here = node_mut(&mut out, path);
            *here = Value::String(changed(here.as_str()?, how));
        }
        Edit::NumberKey => {
            let Some(Step::Key(k)) = path.last() else {
                return None;
            };
            rebuild(&mut out, path, |(key, v)| {
                Some((
                    if &key == k {
                        NUMBER_KEY.to_owned()
                    } else {
                        key
                    },
                    v,
                ))
            })?;
        }
    }
    if &out == value {
        return None;
    }
    let mut text = serde_json::to_string(&out).unwrap();
    if let Edit::NumberKey = edit {
        // JSON text is YAML; an unquoted key is a plain scalar, a number here,
        // and a plain key needs a space after its colon.
        let quoted = format!("\"{NUMBER_KEY}\":");
        if serde_json::to_string(value).unwrap().contains(&quoted) {
            return None;
        }
        text = text.replace(&quoted, &format!("{NUMBER_KEY}: "));
    }
    Some((text, out))
}

/// The checks the page says a schema cannot state, by the reader's refusal:
/// "a gate against its question's primitive, a label against the options
/// offered, `from_turn` against the state's shape, a reference against the
/// state or the questions, a labelled box being a region" (Schemas and
/// conformance); the relations between items no schema states (bands
/// highest first and each named once, case ids unique); and `metadata.minor:
/// 4.0`, "the one place where a document the schemas accept is one a reader
/// refuses" (The apiVersion and the minor). Reviewed like code: a line added
/// here is a check the schemas give up on.
fn cross_check(error: &jud::Error, path: &str) -> Option<&'static str> {
    let message = error.to_string();
    let has = |s: &str| message.contains(s);
    Some(match error {
        jud::Error::Policy { .. } => "a gate against its question",
        _ if has("the instructions have no") => "a part_when name against its instructions",
        _ if has("is referenced nowhere") => "a reference against the state or the questions",
        _ if has("is not a region") => "a labelled box being a region",
        _ if has("the same id as case") => "case ids unique",
        _ if path == "metadata.minor" && has("is not a minor version") => "minor 4.0",
        _ => return None,
    })
}

/// One edited document and how each side judged it.
struct Sample {
    seed: &'static str,
    path: String,
    edit: Edit,
    /// The reader's refusal, `None` when it read the document.
    refused: Option<String>,
    /// The page's name for a refusal the schema cannot state.
    cross_check: Option<&'static str>,
    schema: bool,
    /// What went wrong writing back a document both accepted.
    written: Option<String>,
}

impl Sample {
    fn agree(&self) -> bool {
        self.refused.is_some() != self.schema || self.cross_check.is_some()
    }

    fn reader(&self) -> bool {
        self.refused.is_none()
    }
}

impl std::fmt::Display for Sample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:?} at {}: ", self.seed, self.edit, self.path)?;
        match (&self.refused, &self.written) {
            (_, Some(w)) => write!(f, "written back, {w}"),
            (None, None) => write!(f, "the reader accepts, the schema refuses"),
            (Some(e), None) => write!(f, "the reader refuses ({e}), the schema accepts"),
        }
    }
}

fn dotted(path: &[Step]) -> String {
    path.iter()
        .map(|s| match s {
            Step::Key(k) => k.clone(),
            Step::At(n) => n.to_string(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn judge(seed: &'static str, path: &[Step], edit: Edit, text: &str, d: &Value) -> Sample {
    let dotted = dotted(path);
    let r = jud::parse(text);
    let schema = schema_accepts(d);
    let mut written = None;
    if let (Ok(document), true) = (&r, schema) {
        let w = write(document);
        written = match jud::parse(&w) {
            _ if !schema_accepts(&serde_saphyr::from_str(&w).unwrap()) => {
                Some(format!("the schema refuses:\n{w}"))
            }
            Err(e) => Some(format!("the reader refuses ({e}):\n{w}")),
            Ok(again) if &again != document => Some(format!("reads back as another:\n{w}")),
            Ok(again) if fingerprints(&again) != fingerprints(document) => {
                Some(format!("the fingerprints move:\n{w}"))
            }
            Ok(_) => None,
        };
    }
    Sample {
        seed,
        cross_check: r.as_ref().err().and_then(|e| cross_check(e, &dotted)),
        refused: r.err().map(|e| e.to_string()),
        path: dotted,
        edit,
        schema,
        written,
    }
}

/// Every single edit of every seed, judged.
static SWEEP: LazyLock<Vec<Sample>> = LazyLock::new(|| {
    let mut out = Vec::new();
    for (seed, value) in SEEDS.iter() {
        assert!(schema_accepts(value), "{seed}");
        jud::parse(&serde_json::to_string(value).unwrap())
            .unwrap_or_else(|e| panic!("{seed}: {e}"));
        for path in paths(value) {
            for edit in edits() {
                if let Some((text, d)) = apply(value, &path, &edit) {
                    out.push(judge(seed, &path, edit, &text, &d));
                }
            }
        }
    }
    out
});

/// The same, with each key in turn written as a number.
static NUMBER_KEYS: LazyLock<Vec<Sample>> = LazyLock::new(|| {
    let mut out = Vec::new();
    for (seed, value) in SEEDS.iter() {
        for path in paths(value) {
            if let Some((text, d)) = apply(value, &path, &Edit::NumberKey) {
                out.push(judge(seed, &path, Edit::NumberKey, &text, &d));
            }
        }
    }
    out
});

fn ends(path: &str, last: &[&str]) -> bool {
    last.iter().any(|l| path.ends_with(&format!(".{l}")))
}

/// The disagreements the sweep found, each a bead: (bead, the class).
type Found = (&'static str, fn(&Sample) -> bool);

const FOUND: &[Found] = &[
    // An integral float where the reader wants an integer, outside the minor.
    ("judgment-ogv", |s| {
        !s.reader() && matches!(&s.edit, Edit::Replace(v) if *v == json!(4.0))
    }),
    // A Noul with nothing to ask.
    ("judgment-6bl", |s| {
        s.refused
            .as_ref()
            .is_some_and(|e| e.contains("a Noul needs instructions or criteria"))
    }),
    // A recorded answer of a known type, out of its shape.
    ("judgment-j7x", |s| {
        !s.reader() && s.path.starts_with("spec.response.answers.")
    }),
    // A box label's edges with a field they do not define.
    ("judgment-0dn", |s| {
        s.reader() && s.path.contains(".expect.") && s.path.rsplit('.').next() == Some("box")
    }),
    // Any text as a fingerprint or a timestamp.
    ("judgment-3dt", |s| {
        s.reader() && ends(&s.path, &["fingerprint", "recorded_at", "tuned_at"])
    }),
    // A request_id that is not a string, a usage that is a list.
    ("judgment-ap8", |s| {
        s.reader() && ["spec.response.request_id", "spec.response.usage"].contains(&s.path.as_str())
    }),
    // A number key in what a tuning block keeps.
    ("judgment-p8g", |s| {
        !s.reader() && matches!(s.edit, Edit::NumberKey) && s.path.starts_with("spec.tuning.")
    }),
];

fn report<'s>(samples: impl Iterator<Item = &'s Sample>) -> Vec<String> {
    samples.map(ToString::to_string).collect()
}

/// `R(d) <=> S(d)` for every single edit of every committed document, save
/// the cross-checks and the classes already filed; and what both accept
/// writes back to a document the schema accepts, that reads back as itself,
/// fingerprints and all.
#[test]
fn every_single_edit_of_every_document_is_judged_alike() {
    assert!(SWEEP.len() > 10_000, "{}", SWEEP.len());
    let unexplained = report(
        SWEEP
            .iter()
            .filter(|s| !s.agree() && !FOUND.iter().any(|(_, f)| f(s)))
            .chain(SWEEP.iter().filter(|s| s.written.is_some())),
    );
    assert!(
        unexplained.is_empty(),
        "{} of {} edits:\n{}",
        unexplained.len(),
        SWEEP.len(),
        unexplained.join("\n")
    );
    // Each cross-check the page names is met, so the list holds no dead line.
    let met: BTreeSet<&str> = SWEEP.iter().filter_map(|s| s.cross_check).collect();
    assert_eq!(met.len(), 6, "{met:?}");
}

fn assert_none_found(bead: &str) {
    assert_none_found_in(&SWEEP, bead);
}

fn assert_none_found_in(samples: &[Sample], bead: &str) {
    let (_, class) = FOUND.iter().find(|(b, _)| *b == bead).unwrap();
    let members = report(samples.iter().filter(|s| !s.agree() && class(s)));
    assert!(
        members.is_empty(),
        "{} edits:\n{}",
        members.len(),
        members.join("\n")
    );
}

/// `elapsed_ms`, an expect index, `from_turn` and `level_at_least` written
/// `4.0`: the schema's `integer` takes it, the reader refuses it, and the
/// page names the minor as the one such place.
#[test]
#[ignore = "judgment-ogv: found"]
fn an_integral_float_is_judged_alike_outside_the_minor() {
    assert_none_found("judgment-ogv");
}

#[test]
#[ignore = "judgment-6bl: found"]
fn a_noul_with_nothing_to_ask_is_judged_alike() {
    assert_none_found("judgment-6bl");
}

#[test]
#[ignore = "judgment-j7x: found"]
fn a_recorded_answer_is_judged_alike() {
    assert_none_found("judgment-j7x");
}

#[test]
#[ignore = "judgment-0dn: found"]
fn a_box_label_with_an_unknown_edge_is_judged_alike() {
    assert_none_found("judgment-0dn");
}

#[test]
#[ignore = "judgment-3dt: found"]
fn a_fingerprint_and_a_timestamp_are_judged_alike() {
    assert_none_found("judgment-3dt");
}

#[test]
#[ignore = "judgment-ap8: found"]
fn a_recorded_request_id_and_usage_are_judged_alike() {
    assert_none_found("judgment-ap8");
}

#[test]
#[ignore = "judgment-p8g: found"]
fn a_number_key_in_tuning_is_judged_as_its_text() {
    assert_none_found_in(&NUMBER_KEYS, "judgment-p8g");
}

/// A key is its written text (Reading rules): every key of every document
/// written as a number is judged as the schema judges its text, save the
/// classes already filed.
#[test]
#[ignore = "judgment-pry: lands with w2-reader"]
fn a_key_written_as_a_number_is_judged_as_its_text() {
    let unexplained = report(
        NUMBER_KEYS
            .iter()
            .filter(|s| !s.agree() && !FOUND.iter().any(|(_, f)| f(s)))
            .chain(NUMBER_KEYS.iter().filter(|s| s.written.is_some())),
    );
    assert!(
        unexplained.is_empty(),
        "{} of {} keys:\n{}",
        unexplained.len(),
        NUMBER_KEYS.len(),
        unexplained.join("\n")
    );
}

/// The same key written as text and as a number, in any map of any
/// document: one key twice, which the reader refuses (the schema, given
/// JSON, never sees the pair).
#[test]
#[ignore = "judgment-l47: lands with w2-reader"]
fn a_key_written_as_text_and_as_a_number_is_refused_in_every_map() {
    let mut read = Vec::new();
    for (seed, value) in SEEDS.iter() {
        for path in paths(value) {
            let mut d = value.clone();
            let Some(m) = node_mut(&mut d, &path).as_object_mut() else {
                continue;
            };
            m.insert(NUMBER_KEY.to_owned(), json!("v"));
            let text = serde_json::to_string(&d).unwrap().replace(
                &format!("\"{NUMBER_KEY}\":\"v\""),
                &format!("\"{NUMBER_KEY}\":\"v\",{NUMBER_KEY}: \"v\""),
            );
            if jud::parse(&text).is_ok() {
                read.push(format!("{seed}: {}", dotted(&path)));
            }
        }
    }
    assert!(read.is_empty(), "read as one key:\n{}", read.join("\n"));
}

// ---------------------------------------------------------------------------
// judgment-bsz.18.22: what the reader accepts lowers, and is answerable
// ---------------------------------------------------------------------------

/// A response that answers every question of `asked` within its shape.
fn answering(asked: &judgment::Questions) -> judgment::Response {
    use judgment::Question;
    let answers: Map<String, Value> = asked
        .iter()
        .map(|(id, q)| {
            let answer = match q {
                Question::Noul { .. } => json!({ "type": "noul", "noul": 0.6 }),
                Question::Choice { criteria, .. } => {
                    let first = criteria.keys().next().unwrap();
                    let p: Map<String, Value> = criteria
                        .keys()
                        .map(|k| (k.clone(), json!(f64::from(k == first))))
                        .collect();
                    json!({ "type": "choice", "choice": first, "probabilities": p, "confidence": 1.0 })
                }
                Question::Score { criteria, .. } => {
                    let legend: Map<String, Value> = criteria
                        .iter()
                        .enumerate()
                        .map(|(i, l)| (i.to_string(), l.clone()))
                        .collect();
                    let p: Map<String, Value> = (0..criteria.len())
                        .map(|i| (i.to_string(), json!(f64::from(i == 0))))
                        .collect();
                    json!({ "type": "score", "score": 0.0, "legend": legend,
                            "probabilities": p, "confidence": 1.0 })
                }
                Question::Box { .. } => json!({ "type": "box", "confidence": 0.5,
                             "box": { "top": 0.1, "bottom": 0.9, "left": 0.1, "right": 0.9 } }),
            };
            (id.to_owned(), answer)
        })
        .collect();
    serde_json::from_value(json!({
        "model": "m", "answers": answers, "usage": { "input_tokens": 1, "output_tokens": 1 }
    }))
    .unwrap()
}

/// For every rubric the reader accepts among the single edits of the
/// committed rubrics, over the empty state, a text and the states of the
/// rubric's own first cases, with none and with two options supplied, out
/// of sorted order, to each question that takes them: `lower` is `Ok` or a typed
/// error, never a panic, and the same bytes twice; on `Ok` it asks exactly
/// the questions whose `when` holds, in the rubric's order, each part of
/// the instructions exactly when its `part_when` holds, the supplied
/// options before the static ones, and a response that answers it is read
/// through the policy into one verdict per question asked, in that order.
/// The oracle is the page's Declarations paragraph, not `lower`.
#[test]
fn what_the_reader_accepts_lowers_and_is_answerable() {
    use judgment::Question;
    use judgment::jud::{Rubric, Supplied, present};
    let (mut lowered, mut refused) = (0usize, 0usize);
    for (seed, value) in SEEDS.iter() {
        if value["kind"] != "Rubric" {
            continue;
        }
        let cases = seed.replace(".jud", "-cases.jud");
        let mut states = vec![json!({}), json!("text")];
        if let Some((_, c)) = SEEDS.iter().find(|(s, _)| *s == cases) {
            states.extend(
                c["spec"]["cases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .take(2)
                    .map(|c| c["state"].clone()),
            );
        }
        for path in paths(value) {
            for edit in edits() {
                let Some((text, _)) = apply(value, &path, &edit) else {
                    continue;
                };
                let Ok(rubric) = Rubric::parse(&text) else {
                    continue;
                };
                let supplied: Supplied = rubric
                    .questions
                    .iter()
                    .filter(|(_, q)| q.options_from.is_some())
                    .map(|(id, _)| {
                        let options = [
                            ("zz_supplied".to_owned(), Value::Null),
                            ("aa_supplied".to_owned(), json!("A")),
                        ];
                        (id.clone(), options.into_iter().collect())
                    })
                    .collect();
                let none = Supplied::new();
                for (state, supplied) in states.iter().flat_map(|s| [(s, &supplied), (s, &none)]) {
                    let what = || {
                        format!(
                            "{seed}: {edit:?} at {}, state {state}, {} supplied",
                            dotted(&path),
                            supplied.len()
                        )
                    };
                    let Ok(asked) = rubric.lower(state, supplied) else {
                        refused += 1;
                        continue;
                    };
                    lowered += 1;
                    let again = rubric.lower(state, supplied).unwrap();
                    assert_eq!(
                        serde_json::to_string(&asked).unwrap(),
                        serde_json::to_string(&again).unwrap(),
                        "{}",
                        what()
                    );
                    let holds = |p: &str| present(state, p);
                    let expected: Vec<&str> = rubric
                        .questions
                        .iter()
                        .filter(|(_, q)| q.when.as_deref().is_none_or(holds))
                        .map(|(id, _)| id.as_str())
                        .collect();
                    let ids: Vec<&str> = asked.iter().map(|(id, _)| id).collect();
                    assert_eq!(ids, expected, "{}", what());
                    for (id, q) in asked.iter() {
                        let declared = &rubric.questions[id];
                        if let Some(parts) = q.instructions().as_object() {
                            for (part, p) in &declared.part_when {
                                assert_eq!(
                                    parts.contains_key(part),
                                    holds(p),
                                    "{}: {id}.{part}",
                                    what()
                                );
                            }
                        }
                        if let (
                            Some(_),
                            Question::Choice { criteria, .. },
                            Question::Choice { criteria: own, .. },
                        ) = (&declared.options_from, q, &declared.question)
                        {
                            let keys: Vec<&String> = criteria.keys().collect();
                            let want: Vec<&String> = supplied
                                .get(id)
                                .into_iter()
                                .flat_map(|o| o.keys())
                                .chain(own.keys())
                                .collect();
                            assert_eq!(keys, want, "{}: {id}", what());
                        }
                    }
                    let verdicts = rubric
                        .apply(&asked, &answering(&asked))
                        .unwrap_or_else(|e| panic!("{}: {e}", what()));
                    assert!(
                        verdicts.keys().map(String::as_str).eq(ids.iter().copied()),
                        "{}",
                        what()
                    );
                }
            }
        }
    }
    assert!(
        lowered > 1000 && refused > 0,
        "{lowered} lowered, {refused} refused"
    );
}
