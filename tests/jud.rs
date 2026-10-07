//! The `.jud` format against its own JSON Schemas and its examples: every
//! example document validates, everything the crate writes validates, the
//! parser and the schemas agree on what is refused, the reading rules
//! hold, and a replay answers from `.jud` recordings by fingerprint.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use judgment::eval::Recording;
use judgment::jud::{self, Cases, Document, Rubric, Supplied, recording_to_yaml};
use judgment::{Fake, Replay, SystemOne};
use serde_json::{Value, json};

const SCHEMAS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/schemas/jud");
const EXAMPLES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud");
const RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/recordings/jud_calibration"
);
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
        .chain(std::fs::read_dir(Path::new(RECORDINGS)).unwrap())
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
            "Rubric" => &*RUBRIC,
            "Cases" => &*CASES,
            "Recording" => &*RECORDING,
            other => panic!("{}: kind {other}", path.display()),
        };
        assert_valid(validator, &value, &path.display().to_string());
        let document = jud::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(
            match document {
                Document::Rubric(_) => "Rubric",
                Document::Cases(_) => "Cases",
                Document::Recording(_) => "Recording",
                _ => "?",
            },
            kind
        );
        *kinds.entry(kind).or_default() += 1;
    }
    // triage, routing, handoff, and the guide's screening rubric.
    assert_eq!(kinds["Rubric"], 4);
    // Their cases, and the specification's example cases document.
    assert_eq!(kinds["Cases"], 5);
    // The calibration recordings, and the specification's example recording.
    assert!(kinds["Recording"] >= 11, "{kinds:?}");
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
    // The routing pair: every case lowers to a request and its labels fit it.
    let routing = Rubric::parse(&read(EXAMPLES, "routing.jud")).unwrap();
    let routing_cases = Cases::parse(&read(EXAMPLES, "routing-cases.jud")).unwrap();
    routing_cases.bind(&routing).unwrap();
    let asked: Vec<Vec<String>> = routing_cases
        .cases
        .iter()
        .map(|c| {
            c.request(&routing)
                .unwrap()
                .ids()
                .map(str::to_owned)
                .collect()
        })
        .collect();
    assert_eq!(asked[0], ["desk", "tone", "refund_request"]);
    assert_eq!(asked[1], ["desk", "tone"]);
    assert_eq!(asked[2], ["desk", "tone", "duplicate_of"]);
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
    let asked = handoff.lower(&json!(["hi"]), &Supplied::new()).unwrap();
    let response = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(fake.answer(&json!(["hi"]), "m", &asked))
        .unwrap();
    let mut recording = Recording::new("one", response, 7);
    recording.fingerprint = Some(judgment::eval::canonical::request_fingerprint(
        &json!(["hi"]),
        &asked,
    ));
    recording.recorded_at = Some(judgment::eval::now_rfc3339());
    let yaml = recording_to_yaml(&recording).unwrap();
    assert_valid(&RECORDING, &value_of(&yaml), "recording_to_yaml");
    assert_eq!(jud::parse_recording(&yaml).unwrap(), recording);
    // JSON is a document too, and the same one.
    let as_json = serde_json::to_string(&value_of(&yaml)).unwrap();
    assert_eq!(jud::parse_recording(&as_json).unwrap(), recording);
}

/// Documents the schemas and the reader must both refuse.
fn refused_documents() -> Vec<(&'static jsonschema::Validator, &'static str)> {
    vec![
        // A misspelt gate field.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n  policy:\n    n: {treshold: 0.5}\n",
        ),
        // A one-option Choice.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {only: null}}\n",
        ),
        // Another apiVersion, the old envelope, and a kind in the wrong case.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.2\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "jud: 1.2\nkind: rubric\nid: r\nquestions:\n  n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        // A metadata field the kind does not take, and a document without metadata.
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\n  version: 2\nspec:\n  cases:\n    - state: s\n",
        ),
        (
            &*RECORDING,
            "apiVersion: jud/v1.3\nkind: Recording\nmetadata:\n  name: c\n  description: d\nspec:\n  response: {model: m, answers: {q: {type: noul, noul: 0.5}}}\n  elapsed_ms: 1\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        // A case without a state.
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - expect: {n: true}\n",
        ),
        // from_turn with a sibling.
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: [a]\n      expect: {n: {from_turn: 0, until: 1}}\n",
        ),
        // A recording without a response.
        (
            &*RECORDING,
            "apiVersion: jud/v1.3\nkind: Recording\nmetadata:\n  name: c\nspec:\n  elapsed_ms: 1\n",
        ),
    ]
}

/// Ids that are not names: a path, an absolute path, a blank, a
/// space, a leading dot, on a rubric, a cases document, a case and a
/// recording; refused by the schemas and the reader alike.
fn refused_names() -> Vec<(&'static jsonschema::Validator, &'static str)> {
    vec![
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: ../x\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: \"\"\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: a b\nspec:\n  cases:\n    - state: s\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: /etc/passwd\n      state: s\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: .hidden\n      state: s\n",
        ),
        (
            &*RECORDING,
            "apiVersion: jud/v1.3\nkind: Recording\nmetadata:\n  name: ../c\nspec:\n  response: {model: m, answers: {q: {type: noul, noul: 0.5}}}\n  elapsed_ms: 1\n",
        ),
        // A spec field the kind does not define, and a top-level key beyond the envelope.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  comment: hi\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  notes: hi\n  cases:\n    - state: s\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nx-tool: 1\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n",
        ),
        // An unknown source of options.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {a: A}, options_from: database}\n",
        ),
        // Null where a declaration takes a value; an empty band list; a blank
        // band name; a supplied option that is a number.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?, when: null}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {a: A, b: B}}\n  policy:\n    c: {bands: []}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {a: A, b: B}}\n  policy:\n    c: {bands: [{at_least: 0.5, verdict: \" \"}]}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      options: {c: {a: 1}}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      options: null\n",
        ),
        // A band with a field bands do not have.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {a: A, b: B}}\n  policy:\n    c: {bands: [{at_least: 0.5, verdict: go, colour: red}]}\n",
        ),
        // A state path with an empty segment.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?, when: \"message..x\"}\n",
        ),
    ]
}

#[test]
fn the_schemas_and_the_parser_refuse_the_same_documents() {
    for (validator, text) in refused_documents().into_iter().chain(refused_names()) {
        assert!(
            !validator.is_valid(&value_of(text)),
            "the schema accepts:\n{text}"
        );
        assert!(jud::parse(text).is_err(), "the parser accepts:\n{text}");
    }
}

/// What a reviewer cannot see is refused. A tag the core schema does
/// not define and a merge key are errors with a position; a `!!binary`
/// scalar is its text, never what the base64 encodes, so the model reads
/// what the reviewer read.
#[test]
fn what_a_reviewer_cannot_see_is_refused() {
    let tagged = "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: !secret hidden}\n";
    let err = jud::parse(tagged).unwrap_err();
    assert!(
        matches!(&err, jud::Error::Syntax(m) if m.contains("tag") && m.contains("line 7")),
        "{err}"
    );
    let merged = "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {<<: *b}\n";
    let err = jud::parse(merged).unwrap_err();
    assert!(
        matches!(&err, jud::Error::Syntax(m) if m.contains("merge") && m.contains("line 7")),
        "{err}"
    );
    let binary = "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: !!binary SWdub3JlIHRoaXM=}\n";
    let rubric = Rubric::parse(binary).unwrap();
    let instructions = match &rubric.questions.get("n").unwrap().question {
        judgment::Question::Noul { instructions, .. } => instructions.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        instructions,
        json!("SWdub3JlIHRoaXM="),
        "the text, not `Ignore this`"
    );
}

/// A syntax error names a line and a column and quotes nothing, because
/// a cases document's state can be someone's data and the message lands in
/// a log.
#[test]
fn a_syntax_error_quotes_no_part_of_the_document() {
    let broken = "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: {account: TOPSECRET-4411}\n      expect: [not a map\n";
    let err = Cases::parse(broken).unwrap_err();
    let shown = err.to_string();
    assert!(shown.contains("line"), "{shown}");
    assert!(!shown.contains("TOPSECRET"), "{shown}");
}

/// Every document is written back under the one apiVersion, with the
/// envelope's keys in manifest order, and validates against its schema.
#[test]
fn a_document_is_written_back_under_the_one_api_version() {
    let plain = "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n";
    let rubric = Rubric::parse(plain).unwrap();
    let yaml = rubric.to_yaml().unwrap();
    assert!(
        yaml.starts_with("apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n"),
        "{yaml}"
    );
    assert!(RUBRIC.is_valid(&value_of(plain)));
    assert!(RUBRIC.is_valid(&value_of(&yaml)));
    assert_eq!(jud::API_VERSION, "jud/v1.3");
}

/// The policy fingerprint is the gates alone, so a moved bar is as
/// visible as a changed question; `tuning`, the id and the questions are
/// not part of it, and an empty policy is the fingerprint of `{}`.
#[test]
fn the_policy_fingerprint_is_the_gates_alone() {
    let rubric = Rubric::parse(&read(EXAMPLES, "triage.jud")).unwrap();
    let pinned = rubric.policy_fingerprint();
    assert!(pinned.starts_with("sha256:"));
    let mut moved = rubric.clone();
    moved.policy.gates.get_mut("actionable").unwrap().threshold = Some(0.95);
    assert_ne!(
        moved.policy_fingerprint(),
        pinned,
        "a moved threshold shows"
    );
    assert_eq!(
        moved.fingerprint(),
        rubric.fingerprint(),
        "and the questions' fingerprint cannot show it"
    );
    let mut other = rubric.clone();
    other.policy.tuning = None;
    other.name = "renamed".to_owned();
    assert_eq!(
        other.policy_fingerprint(),
        pinned,
        "tuning and the id are not part of it"
    );
    let mut none = rubric.clone();
    none.policy = jud::Policy::default();
    assert_eq!(
        none.policy_fingerprint(),
        "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        "the SHA-256 of the two bytes `{{}}`"
    );
}

#[test]
fn yes_is_a_string_not_a_boolean() {
    // YAML 1.2 core schema: only `true` and `false` are booleans, so an
    // option called `yes` is the string, and a Noul labelled `yes` is a
    // label that does not fit, not `true`.
    let rubric = Rubric::parse(
        "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n    c: {type: choice, instructions: pick, criteria: {yes: null, no: null, maybe: null}}\n",
    )
    .unwrap();
    let keys: Vec<&str> = match &rubric.questions.get("c").unwrap().question {
        judgment::Question::Choice { criteria, .. } => {
            criteria.keys().map(String::as_str).collect()
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(keys, ["yes", "no", "maybe"]);
    let cases =
        Cases::parse("apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      expect: {c: yes}\n").unwrap();
    cases.bind(&rubric).unwrap();
    let cases =
        Cases::parse("apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      expect: {n: yes}\n").unwrap();
    let err = cases.bind(&rubric).unwrap_err();
    assert!(
        matches!(&err, jud::Error::Case { reason, .. } if reason.contains("a Noul expects")),
        "{err}"
    );
}

#[tokio::test]
async fn a_replay_answers_from_jud_recordings_by_fingerprint() {
    let replay = Replay::open(Path::new(RECORDINGS)).unwrap();
    let recordings = std::fs::read_dir(Path::new(RECORDINGS))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "jud"))
        .count();
    assert_eq!(replay.len(), recordings, "one recording per file");
    let rubric = Rubric::parse(&read(EXAMPLES, "triage.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "triage-cases.jud")).unwrap();
    for case in &cases.cases {
        let asked = case.request(&rubric).unwrap();
        let response = replay.answer(&case.state, "any", &asked).await.unwrap();
        assert_eq!(response.model, "jev-1.13.0");
        rubric.apply(&asked, &response).unwrap();
    }
    // Another state has no recording.
    let err = replay
        .answer(
            &json!({"message": "unseen"}),
            "any",
            &rubric.lower(&json!({}), &Supplied::new()).unwrap(),
        )
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
