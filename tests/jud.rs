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
const AGENT_MAIL_RECORDINGS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/examples/recordings/agent_mail"
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
    // triage, routing, handoff, agent-mail, shapes, stamps, the guide's
    // screening rubric, the shapes rubric without its Box, for the servers
    // that have none, and the pairwise judge.
    assert_eq!(kinds["Rubric"], 9);
    // Their cases, and the specification's example cases document.
    assert_eq!(kinds["Cases"], 10);
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
    // The pairwise cases bind to their rubric, and half put the better answer
    // first: what makes a judge that always picks one place score 0.5.
    let pairwise = Rubric::parse(&read(EXAMPLES, "pairwise-judge.jud")).unwrap();
    let pairwise_cases = Cases::parse(&read(EXAMPLES, "pairwise-judge-cases.jud")).unwrap();
    pairwise_cases.bind(&pairwise).unwrap();
    assert_eq!(pairwise_cases.cases.len(), 50);
    let first = pairwise_cases
        .cases
        .iter()
        .filter(|c| c.expect["better"] == jud::Expect::Text("a".to_owned()))
        .count();
    assert_eq!(first, 25, "the better answer comes first in half the cases");
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
#[allow(clippy::too_many_lines)]
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
        // `jud/v1` without its minor, with one past this reader's, and the
        // earlier spelling with a minor it did not have.
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 99\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: \"3\"\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\n  minor: 3\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        // A Box with what it does not take: criteria, null instructions,
        // options to supply (minor 4).
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`, criteria: {a: A, b: B}}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: null}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`, options_from: request}\n",
        ),
        // An asset with neither source, with both, with a path that leaves
        // the directory, with a format the API does not take, and with bytes
        // but no format.
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: photo.png, format: png, data_base64: aGk=}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: ../photo.png}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: '..\\photo.png'}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: '\\\\server\\share\\photo.png'}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: C:/photo.png}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      assets:\n        photo: {type: image, path: photo.webp, format: webp}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      assets:\n        photo: {type: image, data_base64: aGk=}\n",
        ),
        // A Box label without one of its edges, and one with a bar over 1.
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      expect: {b: {box: {top: 0.2, bottom: 0.8, left: 0.2}}}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      expect: {b: {box: {top: 0.2, bottom: 0.8, left: 0.2, right: 0.8}, iou_at_least: 2}}\n",
        ),
        // A Box with what it does not take: criteria, null instructions,
        // options to supply (minor 4).
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`, criteria: {a: A, b: B}}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: null}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`, options_from: request}\n",
        ),
        // An asset with neither source, with both, with a path that leaves
        // the directory, with a format the API does not take, and with bytes
        // but no format.
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: photo.png, format: png, data_base64: aGk=}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in `photo`}\n  assets:\n    photo: {type: image, path: ../photo.png}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      assets:\n        photo: {type: image, path: photo.webp, format: webp}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      assets:\n        photo: {type: image, data_base64: aGk=}\n",
        ),
        // A Box label without one of its edges, and one with a bar over 1.
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      expect: {b: {box: {top: 0.2, bottom: 0.8, left: 0.2}}}\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      expect: {b: {box: {top: 0.2, bottom: 0.8, left: 0.2, right: 0.8}, iou_at_least: 2}}\n",
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
        // A number or a boolean where the format takes text, and null where it
        // gives null no meaning: read as text or as absent, each would be a
        // document the schemas refuse.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: 2026\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\n  labels: {run: 3}\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    s: {type: score, instructions: rate, criteria: [low, high]}\n  policy:\n    s: {fallback: 1}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n  policy:\n    n: {threshold: null}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n  policy: null\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - id: 123\n      state: s\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      tags: [1, true]\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      note: 4.5\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  rubric: 7\n  cases:\n    - state: s\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  rubric: null\n  cases:\n    - state: s\n",
        ),
        (
            &*CASES,
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cases\nspec:\n  cases:\n    - state: s\n      expect: null\n",
        ),
        // A recording's field misspelt: read, it would be a recording with no
        // fingerprint, which no replay finds by its request.
        (
            &*RECORDING,
            "apiVersion: jud/v1.3\nkind: Recording\nmetadata:\n  name: c\nspec:\n  elapsed_ms: 1\n  fingreprint: sha256:00\n  response: {model: m, answers: {}, usage: {input_tokens: 1, output_tokens: 1}}\n",
        ),
        // A number or a boolean where the API takes text, an object, an array or
        // null: instructions, a level, an option's description, a Noul's side.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: 42}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    s: {type: score, instructions: how bad?, criteria: [1, 2, 3]}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {a: 1, b: true}}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, criteria: {true: 1, false: false}}\n",
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

/// Every document is written back under `jud/v1` and the minor this crate
/// implements, with the envelope's keys in manifest order, and validates
/// against its schema; a document in the earlier spelling reads and is
/// moved forward by the write.
#[test]
fn a_document_is_written_back_under_the_one_api_version() {
    assert_eq!(jud::API_VERSION, "jud/v1");
    assert_eq!(jud::MINOR, 4, "the minor this crate reads and writes");
    let minor = jud::MINOR;
    let plain = format!(
        "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: {minor}\nspec:\n  questions:\n    n: {{type: noul, instructions: ok?}}\n"
    );
    let rubric = Rubric::parse(&plain).unwrap();
    let yaml = rubric.to_yaml().unwrap();
    assert!(
        yaml.starts_with(&format!(
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: {minor}\nspec:\n"
        )),
        "{yaml}"
    );
    assert!(RUBRIC.is_valid(&value_of(&plain)));
    assert!(RUBRIC.is_valid(&value_of(&yaml)));
    // The spelling minor 3 was published under: read, valid, and written
    // back as `jud/v1` with the minor; a lower minor is read too.
    let earlier = "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?}\n";
    assert!(RUBRIC.is_valid(&value_of(earlier)));
    assert_eq!(Rubric::parse(earlier).unwrap(), rubric);
    assert_eq!(Rubric::parse(earlier).unwrap().to_yaml().unwrap(), yaml);
    let lower = plain.replace(&format!("minor: {minor}"), "minor: 2");
    assert!(RUBRIC.is_valid(&value_of(&lower)));
    assert_eq!(Rubric::parse(&lower).unwrap(), rubric);
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

#[tokio::test]
async fn the_agent_mail_recordings_replay_under_the_gates_they_tuned() {
    // The live answers `jud record` kept for agent-mail: one per case, each
    // found by fingerprint, so a question edited without re-recording fails
    // here. The counts are the gates' agreement with the labels, which the
    // rubric's notes and the measurement it came from quote.
    let rubric = Rubric::parse(&read(EXAMPLES, "agent-mail.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "agent-mail-cases.jud")).unwrap();
    cases.bind(&rubric).unwrap();
    let tuning = rubric.policy.tuning.as_ref().unwrap();
    assert_eq!(tuning.cases.as_deref(), Some(cases.fingerprint().as_str()));
    let replay = Replay::open(Path::new(AGENT_MAIL_RECORDINGS)).unwrap();
    assert_eq!(replay.len(), cases.cases.len());
    let mut right = std::collections::BTreeMap::<String, usize>::new();
    for case in &cases.cases {
        let asked = case.request(&rubric).unwrap();
        let response = replay.answer(&case.state, "any", &asked).await.unwrap();
        assert_eq!(response.model, "jev-1.13.0");
        for (id, verdict) in rubric.apply(&asked, &response).unwrap() {
            let said = matches!(verdict, jud::Verdict::Yes { .. });
            let expected = matches!(case.expect[&id], jud::Expect::Bool(true));
            *right.entry(id).or_default() += usize::from(said == expected);
        }
    }
    assert_eq!(right["needs_action"], 28);
    assert_eq!(right["breaks_something"], 26);
    assert_eq!(right["status_only"], 32);
}

/// The shapes example: a case names its image once, by a path relative to
/// the document, under the reference its state carries; the rubric's and
/// the case's images load into one request's assets, and a Box label
/// grades by intersection over union against the box the model answers,
/// 0.5 being the bar when the label names none.
#[tokio::test]
async fn the_shapes_cases_load_their_images_and_grade_a_box_by_iou() {
    let rubric = Rubric::parse(&read(EXAMPLES, "shapes.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "shapes-cases.jud")).unwrap();
    cases.bind(&rubric).unwrap();
    assert_eq!(cases.cases.len(), 40);
    assert!(rubric.assets.is_empty(), "the images are the cases'");
    let base = Path::new(EXAMPLES);
    let mut labelled = Vec::new();
    for case in &cases.cases {
        let mut assets = judgment::Assets::new();
        rubric.load_assets(base, &mut assets).unwrap();
        case.load_assets(base, &mut assets).unwrap();
        let reference = case.state["image"].as_str().unwrap();
        assert_eq!(assets.len(), 1);
        let image = assets.get(reference).unwrap();
        assert_eq!((image.width(), image.height()), (320, 320));
        assert_eq!(image.format(), judgment::ImageFormat::Png);
        match case.expect.get("circle") {
            Some(jud::Expect::Box(label)) => {
                assert!(label.iou_at_least.is_none(), "the default bar");
                labelled.push((case, label.edges));
            }
            None => assert_eq!(case.expect["circle_present"], jud::Expect::Bool(false)),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(labelled.len(), 30);
    // A box a little wider than its label is right; one in the wrong corner
    // is wrong, with the overlap it reached beside it.
    let (case, edges) = labelled[0];
    let asked = case.request(&rubric).unwrap();
    let close = Fake::new()
        .noul("circle_present", 0.9)
        .unwrap()
        .r#box("circle", edges.padded(0.02), Some(0.8))
        .unwrap();
    let response = close.answer(&case.state, "m", &asked).await.unwrap();
    let graded: std::collections::BTreeMap<_, _> = jud::grade(&rubric, case, 0, &response)
        .unwrap()
        .into_iter()
        .collect();
    let circle = &graded["circle"];
    assert_eq!(circle.correct, Some(true));
    assert!(circle.iou.unwrap() > 0.5, "{circle:?}");
    assert_eq!(graded["circle_present"].correct, Some(true));
    let far = Fake::new()
        .noul("circle_present", 0.9)
        .unwrap()
        .r#box(
            "circle",
            judgment::BoxEdges::new(0.0, 0.1, 0.0, 0.1),
            Some(0.8),
        )
        .unwrap();
    let response = far.answer(&case.state, "m", &asked).await.unwrap();
    let graded: std::collections::BTreeMap<_, _> = jud::grade(&rubric, case, 0, &response)
        .unwrap()
        .into_iter()
        .collect();
    assert_eq!(graded["circle"].correct, Some(false));
    assert_eq!(graded["circle"].iou, Some(0.0));
}

/// The format page's second known vector: a request with one image
/// fingerprints over the image's digest under its reference, so another
/// implementation can check itself against the same bytes.
#[test]
fn the_request_fingerprint_with_an_image_matches_the_known_vector() {
    let image = judgment::Image::read(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/images/square-300.png"
    )))
    .unwrap();
    assert_eq!(image.len(), 717);
    assert_eq!(
        image.digest(),
        "sha256:d1b456a83498c260c89efd73af6015d7260951224900d1d9c736bcd15d96ceb2"
    );
    let mut assets = judgment::Assets::new();
    assets.insert("photo", image).unwrap();
    let mut questions = judgment::Questions::new();
    questions.r#box("mug", "the mug in `photo`").unwrap();
    let state = json!({ "photo": "photo" });
    assert_eq!(
        judgment::eval::canonical::request_fingerprint_with(&state, &questions, &assets),
        "sha256:7a492d9eb5e50d0413dfc3d3361e06dabc0ca9fe68e31860b923340d4258aa72"
    );
    // The same request without the image is the fingerprint it always was.
    assert_eq!(
        judgment::eval::canonical::request_fingerprint_with(
            &state,
            &questions,
            &judgment::Assets::new()
        ),
        judgment::eval::canonical::request_fingerprint(&state, &questions)
    );
}

/// The same forty images asked the Noul alone, as `shapes-noul.jud` does for
/// a server without the Box, answered by `clef-flash` on a local Ollama over
/// the shared `images` array (`docs/project/research/compatible-servers-and-models.md`):
/// forty of forty, as the hosted model, found by the fingerprint that folds
/// each image in. A re-recording that moves a count fails here.
#[tokio::test]
async fn the_clef_recordings_answer_the_noul_alone() {
    const CLEF: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/recordings/shapes-clef"
    );
    let rubric = Rubric::parse(&read(EXAMPLES, "shapes-noul.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "shapes-noul-cases.jud")).unwrap();
    cases.bind(&rubric).unwrap();
    let replay = Replay::open(Path::new(CLEF)).unwrap();
    assert_eq!(replay.len(), cases.cases.len());
    let base = Path::new(EXAMPLES);
    let mut judgments = Vec::new();
    for (index, case) in cases.cases.iter().enumerate() {
        let asked = case.request(&rubric).unwrap();
        let mut assets = judgment::Assets::new();
        case.load_assets(base, &mut assets).unwrap();
        let response = replay
            .answer_with_assets(&case.state, "any", &asked, &assets)
            .await
            .unwrap();
        assert_eq!(response.model, "clef-flash");
        judgments.extend(jud::grade(&rubric, case, index, &response).unwrap());
    }
    let present = judgment::eval::QuestionMetrics::summarise(
        judgments.iter().map(|(_, j)| j),
        judgment::eval::ECE_BINS,
    );
    assert_eq!((present.labelled, present.correct), (40, 40));
    assert!(present.brier.unwrap() < 0.01, "{present:?}");
}

/// The shapes recordings: `jev-1.14.0`'s live answers to the forty cases,
/// each found by the fingerprint that folds its image in, each verified
/// against the Box question before it is read, and graded as the
/// calibration study tables it (`docs/project/verification/box-calibration-study.md`):
/// every Noul right, twenty-eight of thirty boxes at the 0.5 bar, a mean
/// overlap of 0.80. A re-recording that moves a count fails here.
#[tokio::test]
async fn the_shapes_recordings_replay_under_the_box_gate() {
    const SHAPES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/recordings/shapes");
    let rubric = Rubric::parse(&read(EXAMPLES, "shapes.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "shapes-cases.jud")).unwrap();
    cases.bind(&rubric).unwrap();
    let replay = Replay::open(Path::new(SHAPES)).unwrap();
    assert_eq!(replay.len(), cases.cases.len());
    let base = Path::new(EXAMPLES);
    let mut judgments = Vec::new();
    let mut located = 0;
    for (index, case) in cases.cases.iter().enumerate() {
        let asked = case.request(&rubric).unwrap();
        let mut assets = judgment::Assets::new();
        case.load_assets(base, &mut assets).unwrap();
        // Keyed by the image: without it there is no recording to find.
        let err = replay.answer(&case.state, "any", &asked).await.unwrap_err();
        assert!(matches!(err, judgment::Error::NoRecording(_)), "{err}");
        let response = replay
            .answer_with_assets(&case.state, "any", &asked, &assets)
            .await
            .unwrap();
        assert_eq!(response.model, "jev-1.14.0");
        for (id, verdict) in rubric.apply(&asked, &response).unwrap() {
            if id == "circle" && matches!(verdict, jud::Verdict::Located { .. }) {
                located += 1;
            }
        }
        judgments.extend(jud::grade(&rubric, case, index, &response).unwrap());
    }
    let metrics = |id: &str| {
        judgment::eval::QuestionMetrics::summarise(
            judgments.iter().filter(|(q, _)| q == id).map(|(_, j)| j),
            judgment::eval::ECE_BINS,
        )
    };
    let present = metrics("circle_present");
    assert_eq!((present.labelled, present.correct), (40, 40));
    assert!(present.brier.unwrap() < 0.01, "{present:?}");
    let circle = metrics("circle");
    assert_eq!((circle.labelled, circle.correct), (30, 28));
    assert_eq!(circle.brier, None, "a box has no distribution");
    assert!(
        (circle.mean_iou.unwrap() - 0.797).abs() < 0.005,
        "{circle:?}"
    );
    // The gate at 0.5 acts on 23 of the 40 boxes and defers the rest.
    assert_eq!(located, 23);
}

/// The stamps recordings: `jev-1.14.0`'s live answers to the forty
/// document pages, each found by the fingerprint that folds its page in,
/// and graded as the second calibration study tables it
/// (`docs/project/verification/box-calibration-study-pages.md`): every
/// Noul right, twenty-four of thirty boxes at the 0.5 bar on a target a
/// tenth of the page wide, a mean overlap of 0.65, and a gate at 0.5 that
/// acts on 32 answers because the Box's confidence crosses the bar on
/// three pages with no stamp, where the box sits on the round logo. A
/// re-recording that moves a count fails here.
#[tokio::test]
async fn the_stamps_recordings_replay_under_the_box_gate() {
    const STAMPS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/recordings/stamps");
    let rubric = Rubric::parse(&read(EXAMPLES, "stamps.jud")).unwrap();
    let cases = Cases::parse(&read(EXAMPLES, "stamps-cases.jud")).unwrap();
    cases.bind(&rubric).unwrap();
    let replay = Replay::open(Path::new(STAMPS)).unwrap();
    assert_eq!(replay.len(), cases.cases.len());
    let base = Path::new(EXAMPLES);
    let mut judgments = Vec::new();
    let (mut located, mut located_on_empty) = (0, 0);
    for (index, case) in cases.cases.iter().enumerate() {
        let asked = case.request(&rubric).unwrap();
        let mut assets = judgment::Assets::new();
        case.load_assets(base, &mut assets).unwrap();
        let response = replay
            .answer_with_assets(&case.state, "any", &asked, &assets)
            .await
            .unwrap();
        assert_eq!(response.model, "jev-1.14.0");
        let present = matches!(
            case.expect.get("stamp_present"),
            Some(jud::Expect::Bool(true))
        );
        for (id, verdict) in rubric.apply(&asked, &response).unwrap() {
            if id == "stamp" && matches!(verdict, jud::Verdict::Located { .. }) {
                located += 1;
                if !present {
                    located_on_empty += 1;
                }
            }
        }
        judgments.extend(jud::grade(&rubric, case, index, &response).unwrap());
    }
    let metrics = |id: &str| {
        judgment::eval::QuestionMetrics::summarise(
            judgments.iter().filter(|(q, _)| q == id).map(|(_, j)| j),
            judgment::eval::ECE_BINS,
        )
    };
    let present = metrics("stamp_present");
    assert_eq!((present.labelled, present.correct), (40, 40));
    assert!(present.brier.unwrap() < 0.01, "{present:?}");
    let stamp = metrics("stamp");
    assert_eq!((stamp.labelled, stamp.correct), (30, 24));
    assert!((stamp.mean_iou.unwrap() - 0.645).abs() < 0.005, "{stamp:?}");
    // The Box's confidence does not say "nothing here" when a round logo
    // is on the page: the gate acts on three of the ten empty pages, and
    // only the Noul beside it keeps the application from cropping a logo.
    assert_eq!((located, located_on_empty), (32, 3));
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("judgment-jud-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
