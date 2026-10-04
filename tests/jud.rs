//! The `.jud` format against its own JSON Schemas and its examples: every
//! example document validates, everything the crate writes validates, the
//! parser and the schemas agree on what is refused, and a replay answers
//! from `.jud` recordings by fingerprint.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use judgment::eval::Recording;
use judgment::jud::{self, Cases, Document, Rubric, recording_to_yaml};
use judgment::{Fake, Replay, SystemOne};
use serde_json::{Value, json};

const SCHEMAS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/schemas/jud");
const EXAMPLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud");
const BASE: &str = "https://github.com/chussenot/judgment/schemas/jud/";

fn read(dir: &str, name: &str) -> String {
    let path = Path::new(dir).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// YAML or JSON text as a JSON value, the shape the schemas are written
/// against.
fn value_of(text: &str) -> Value {
    serde_saphyr::from_str(text).expect("the document is YAML")
}

/// Resolves `common.schema.json` (and the other schemas) from the
/// directory, so the cross-file `$ref`s need no network.
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
        Ok(serde_json::from_str(&read(SCHEMAS, name))?)
    }
}

fn validator(kind: &str) -> jsonschema::Validator {
    let schema: Value =
        serde_json::from_str(&read(SCHEMAS, &format!("{kind}.schema.json"))).unwrap();
    jsonschema::options()
        .with_retriever(Local)
        .build(&schema)
        .unwrap_or_else(|e| panic!("{kind}.schema.json does not compile: {e}"))
}

static RUBRIC: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator("rubric"));
static CASES: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator("cases"));
static RECORDING: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator("recording"));

fn assert_valid(validator: &jsonschema::Validator, document: &Value, what: &str) {
    let errors: Vec<String> = validator
        .iter_errors(document)
        .map(|e| format!("{}: {e}", e.instance_path()))
        .collect();
    assert!(
        errors.is_empty(),
        "{what} does not validate:\n{}",
        errors.join("\n")
    );
}

fn example_files(extension: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(EXAMPLES)
        .unwrap()
        .chain(std::fs::read_dir(Path::new(EXAMPLES).join("recordings")).unwrap())
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == extension))
        .collect();
    files.sort();
    files
}

#[test]
fn every_example_document_validates_against_its_schema_and_parses() {
    let files = example_files("jud");
    assert!(files.len() >= 4, "{files:?}");
    let mut kinds = std::collections::BTreeMap::<String, usize>::new();
    for path in &files {
        let text = std::fs::read_to_string(path).unwrap();
        let value = value_of(&text);
        let kind = value["kind"]
            .as_str()
            .unwrap_or_else(|| panic!("{}: no kind", path.display()))
            .to_owned();
        let validator = match kind.as_str() {
            "rubric" => &*RUBRIC,
            "cases" => &*CASES,
            "recording" => &*RECORDING,
            other => panic!("{}: kind {other}", path.display()),
        };
        assert_valid(validator, &value, &path.display().to_string());
        let document = jud::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            match document {
                Document::Rubric(_) => "rubric",
                Document::Cases(_) => "cases",
                Document::Recording(_) => "recording",
                _ => "?",
            },
            kind
        );
        *kinds.entry(kind).or_default() += 1;
    }
    assert_eq!(kinds["rubric"], 2);
    assert_eq!(kinds["cases"], 2);
    assert!(kinds["recording"] >= 10, "{kinds:?}");
}

#[test]
fn the_example_cases_bind_to_their_rubric_and_the_tuning_names_them() {
    let rubric = Rubric::parse(&read(EXAMPLES, "triage.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "triage-cases.jud")).unwrap();
    cases.bind(&rubric).unwrap();
    // The committed rubric was tuned on exactly these cases.
    let tuning = rubric.policy.tuning.as_ref().unwrap();
    assert_eq!(tuning.cases.as_deref(), Some(cases.fingerprint().as_str()));
    let handoff = Rubric::parse(&read(EXAMPLES, "handoff.jud")).unwrap();
    Cases::parse(&read(EXAMPLES, "handoff-cases.jud"))
        .unwrap()
        .bind(&handoff)
        .unwrap();
    // The recordings name the rubric they answer.
    for path in example_files("jud") {
        if let Document::Recording(recording) =
            jud::parse(&std::fs::read_to_string(&path).unwrap()).unwrap()
        {
            assert!(
                matches!(
                    recording.rubric.as_deref(),
                    Some("inbox-triage" | "handoff")
                ),
                "{}: rubric {:?}",
                path.display(),
                recording.rubric
            );
            assert!(recording.fingerprint.is_some(), "{}", path.display());
        }
    }
}

#[test]
fn what_the_crate_writes_validates() {
    let rubric = Rubric::parse(&read(EXAMPLES, "triage.jud")).unwrap();
    assert_valid(
        &RUBRIC,
        &value_of(&rubric.to_yaml().unwrap()),
        "Rubric::to_yaml",
    );
    let cases = Cases::parse(&read(EXAMPLES, "handoff-cases.jud")).unwrap();
    assert_valid(
        &CASES,
        &value_of(&cases.to_yaml().unwrap()),
        "Cases::to_yaml",
    );
    let fake = Fake::new().noul("wants_human", 0.3).unwrap();
    let handoff = Rubric::parse(&read(EXAMPLES, "handoff.jud")).unwrap();
    let response = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(fake.answer(&json!(["hi"]), "m", &handoff.questions))
        .unwrap();
    let mut recording = Recording::new("one", response, 7);
    recording.fingerprint = Some(judgment::eval::canonical::request_fingerprint(
        &json!(["hi"]),
        &handoff.questions,
    ));
    recording.recorded_at = Some(judgment::eval::now_rfc3339());
    let yaml = recording_to_yaml(&recording).unwrap();
    assert_valid(&RECORDING, &value_of(&yaml), "recording_to_yaml");
    assert_eq!(jud::parse_recording(&yaml).unwrap(), recording);
    // JSON is a document too, and the same one.
    let as_json = serde_json::to_string(&value_of(&yaml)).unwrap();
    assert_eq!(jud::parse_recording(&as_json).unwrap(), recording);
}

#[test]
fn the_schemas_and_the_parser_refuse_the_same_documents() {
    let refused = [
        // A misspelt gate field.
        (
            &*RUBRIC,
            "jud: 1\nkind: rubric\nid: r\nquestions:\n  n: {type: noul, instructions: ok?}\npolicy:\n  n: {treshold: 0.5}\n",
        ),
        // A one-option Choice.
        (
            &*RUBRIC,
            "jud: 1\nkind: rubric\nid: r\nquestions:\n  c: {type: choice, instructions: pick, criteria: {only: null}}\n",
        ),
        // Another format version.
        (
            &*RUBRIC,
            "jud: 2\nkind: rubric\nid: r\nquestions:\n  n: {type: noul, instructions: ok?}\n",
        ),
        // A case without a state.
        (
            &*CASES,
            "jud: 1\nkind: cases\ncases:\n  - expect: {n: true}\n",
        ),
        // from_turn with a sibling.
        (
            &*CASES,
            "jud: 1\nkind: cases\ncases:\n  - state: [a]\n    expect: {n: {from_turn: 0, until: 1}}\n",
        ),
        // A recording without a response.
        (
            &*RECORDING,
            "jud: 1\nkind: recording\ncase: c\nelapsed_ms: 1\n",
        ),
    ];
    for (validator, text) in refused {
        assert!(
            !validator.is_valid(&value_of(text)),
            "the schema accepts:\n{text}"
        );
        assert!(jud::parse(text).is_err(), "the parser accepts:\n{text}");
    }
}

#[test]
fn yes_is_a_string_not_a_boolean() {
    // YAML 1.2 core schema: only `true` and `false` are booleans, so an
    // option called `yes` is the string, and a Noul labelled `yes` is a
    // label that does not fit, not `true`.
    let rubric = Rubric::parse(
        "jud: 1\nkind: rubric\nid: r\nquestions:\n  n: {type: noul, instructions: ok?}\n  c: {type: choice, instructions: pick, criteria: {yes: null, no: null, maybe: null}}\n",
    )
    .unwrap();
    let keys: Vec<&str> = match rubric.questions.get("c").unwrap() {
        judgment::Question::Choice { criteria, .. } => {
            criteria.keys().map(String::as_str).collect()
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(keys, ["yes", "no", "maybe"]);
    let cases =
        Cases::parse("jud: 1\nkind: cases\ncases:\n  - state: s\n    expect: {c: yes}\n").unwrap();
    cases.bind(&rubric).unwrap();
    let cases =
        Cases::parse("jud: 1\nkind: cases\ncases:\n  - state: s\n    expect: {n: yes}\n").unwrap();
    let err = cases.bind(&rubric).unwrap_err();
    assert!(
        matches!(&err, jud::Error::Case { reason, .. } if reason.contains("a Noul expects")),
        "{err}"
    );
}

#[tokio::test]
async fn a_replay_answers_from_jud_recordings_by_fingerprint() {
    let replay = Replay::open(&Path::new(EXAMPLES).join("recordings")).unwrap();
    assert_eq!(
        replay.len(),
        example_files("jud").len() - 4,
        "one recording per file"
    );
    let rubric = Rubric::parse(&read(EXAMPLES, "triage.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "triage-cases.jud")).unwrap();
    for case in &cases.cases {
        let response = replay
            .answer(&case.state, "any", &rubric.questions)
            .await
            .unwrap();
        assert_eq!(response.model, "jev-1.13.0");
        rubric.apply(&response).unwrap();
    }
    // Another state has no recording.
    let err = replay
        .answer(&json!({"message": "unseen"}), "any", &rubric.questions)
        .await
        .unwrap_err();
    assert!(matches!(err, judgment::Error::NoRecording(_)), "{err}");
    // A `.jud` file that is not a recording is an error, named.
    let dir = tempdir();
    std::fs::write(dir.join("rubric.jud"), read(EXAMPLES, "triage.jud")).unwrap();
    let err = Replay::open(&dir).unwrap_err();
    assert!(
        matches!(&err, judgment::Error::InvalidRecording { path, .. } if path.ends_with("rubric.jud")),
        "{err}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("judgment-jud-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
