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
        // A gate with `confidence` and `bands`: one band is a confidence bar.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    c: {type: choice, instructions: pick, criteria: {a: A, b: B}}\n  policy:\n    c: {confidence: 0.5, bands: [{at_least: 0.5, verdict: go}]}\n",
        ),
        // A blank reference.
        (
            &*RUBRIC,
            "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    b: {type: box, instructions: the mug in ` `}\n  assets:\n    \" \": {type: image, path: photo.png}\n",
        ),
        // `part_when` beside instructions that are text, empty or not.
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?, part_when: {}}\n",
        ),
        (
            &*RUBRIC,
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    n: {type: noul, instructions: ok?, part_when: {question: a.b}}\n",
        ),
        // A Box label with an edge out of range, refused when read, not
        // only when bound.
        (
            &*CASES,
            "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: cases\n  minor: 4\nspec:\n  cases:\n    - state: {photo: photo}\n      expect: {b: {box: {top: 0.2, bottom: 1.5, left: 0.2, right: 0.8}}}\n",
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

/// A fingerprint is over the value as read, in the form the writer writes:
/// spelling a default another way moves none, and the format page's vector
/// for a question without instructions holds.
#[test]
fn fingerprints_are_over_the_normal_form() {
    let rubric = |questions: &str, policy: &str| {
        Rubric::parse(&format!(
            "apiVersion: jud/v1.3\nkind: Rubric\nmetadata:\n  name: r\nspec:\n  questions:\n    {questions}\n{policy}"
        ))
        .unwrap()
    };
    for (one, other) in [
        // Instructions absent or null; a strict gate's default.
        (
            rubric(
                "c: {type: choice, criteria: {a: A, b: B}}",
                "  policy:\n    c: {confidence: 0.5}\n",
            ),
            rubric(
                "c: {type: choice, instructions: null, criteria: {a: A, b: B}}",
                "  policy:\n    c: {confidence: 0.5, strict: false}\n",
            ),
        ),
        // An empty part_when.
        (
            rubric("n: {type: noul, instructions: {q: ok?}}", ""),
            rubric("n: {type: noul, instructions: {q: ok?}, part_when: {}}", ""),
        ),
    ] {
        assert_eq!(one.fingerprint(), other.fingerprint());
        assert_eq!(one.policy_fingerprint(), other.policy_fingerprint());
    }
    let without = rubric("c: {type: choice, criteria: {a: A, b: B}}", "");
    assert_eq!(
        without.fingerprint(),
        "sha256:d10bb58f36f7e4cd6be5887cefaddad376348591fad70bf73fec962fcabf910b"
    );
    // Every option from the request: the Choice's criteria are `{}`.
    let supplied = rubric(
        "c: {type: choice, instructions: pick, options_from: request}",
        "",
    );
    assert_eq!(
        supplied.fingerprint(),
        judgment::eval::canonical::fingerprint(&json!({
            "c": { "type": "choice", "instructions": "pick", "criteria": {}, "options_from": "request" }
        }))
    );
    let cases = |case: &str| {
        Cases::parse(&format!(
            "apiVersion: jud/v1.3\nkind: Cases\nmetadata:\n  name: cs\nspec:\n  cases:\n    - {case}\n"
        ))
        .unwrap()
        .fingerprint()
    };
    assert_eq!(
        cases("{id: k, state: s, tags: [], expect: {}, options: {}, assets: {}}"),
        cases("{id: k, state: s}")
    );
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

/// Typed documents, generated over the shapes the format takes rather than
/// over the space of strings: at most four questions, cases, answers or
/// entries anywhere, short strings drawn mostly from the ones YAML reads as
/// something else (`yes`, `null`, `1`, `0x1`, `- a`, a line break, a BOM),
/// and maps built in insertion order, the order the model sees and the
/// writer keeps. A state is arbitrary JSON, which has no NaN, so no `.nan`
/// is generated (nor can a state hold one).
mod generated {
    use std::collections::BTreeMap;

    use indexmap::IndexMap;
    use judgment::eval::Recording;
    use judgment::jud::{
        Band, BoxLabel, Case, Cases, Expect, FromTurn, Gate, LevelRef, OptionsFrom, Policy, Rubric,
        RubricQuestion, Tuning,
    };
    use judgment::{
        Answer, BoxEdges, Confidence, NoulCriteria, Probability, Question, Response, Usage,
    };
    use proptest::collection::vec;
    use proptest::option;
    use proptest::prelude::*;
    use proptest::sample::{Index, select};
    use proptest::test_runner::FileFailurePersistence;
    use serde_json::{Value, json};

    /// Failures saved at the crate root, as `tests/properties.rs` does.
    pub fn config() -> ProptestConfig {
        ProptestConfig {
            failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
                "proptest-regressions/jud.txt",
            ))),
            ..ProptestConfig::default()
        }
    }

    /// What a YAML reader could take for something other than the text.
    #[rustfmt::skip]
    const AWKWARD: &[&str] = &[
        "", "yes", "No", "true", "null", "~", "1", "-0", "0x1", "0o7", "1e3", "1_000", ".inf",
        ".nan", "2024-01-01", "- a", "a: b", "a #b", "#", "'", "\"", " x", "x ", "\n", "a\nb",
        "\r\n", "\t", "\u{2028}", "\u{feff}x", "\u{85}", "é", "🙂", "*a", "&a", "!t", "%x",
        "@x", "`", "{", "[a]", ">", "|", "? x", "<<", "\\", "---", "...", "=",
    ];

    pub fn text() -> impl Strategy<Value = String> {
        prop_oneof![
            3 => select(AWKWARD).prop_map(str::to_owned),
            1 => "[a-z ]{1,5}",
        ]
    }

    fn filled() -> impl Strategy<Value = String> {
        text().prop_filter("not blank", |t| !t.trim().is_empty())
    }

    /// A name (`docs/reference/jud-format.md`, Names).
    pub fn name() -> impl Strategy<Value = String> {
        "[a-z0-9][a-z0-9._-]{0,5}"
    }

    fn path() -> impl Strategy<Value = String> {
        vec("[a-z0-9_]{1,3}", 1..3).prop_map(|segments| segments.join("."))
    }

    fn unit() -> impl Strategy<Value = f64> {
        prop_oneof![select(&[0.0, 1.0, 0.5][..]), 0.0..=1.0f64]
    }

    /// Any JSON: every scalar, any finite float, nested to depth three.
    pub fn json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(Value::from),
            any::<u64>().prop_map(Value::from),
            any::<f64>()
                .prop_filter("JSON has no NaN or infinity", |f| f.is_finite())
                .prop_map(Value::from),
            text().prop_map(Value::String),
        ];
        leaf.prop_recursive(3, 16, 4, |inner| {
            prop_oneof![
                vec(inner.clone(), 0..4).prop_map(Value::Array),
                vec((text(), inner), 0..4).prop_map(|kv| Value::Object(kv.into_iter().collect())),
            ]
        })
    }

    /// Text, an object, an array or null: what instructions and a
    /// description may be.
    fn described() -> impl Strategy<Value = Value> {
        prop_oneof![
            text().prop_map(Value::String),
            Just(Value::Null),
            vec(json(), 0..3).prop_map(Value::Array),
            vec((text(), json()), 1..3).prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ]
    }

    fn map<V>(entries: Vec<(String, V)>) -> IndexMap<String, V> {
        entries.into_iter().collect()
    }

    /// One question with its declarations, valid as the builder checks it.
    fn question() -> impl Strategy<Value = RubricQuestion> {
        let noul = (
            described(),
            option::of((option::of(described()), option::of(described()))),
        )
            .prop_map(|(instructions, criteria)| {
                let criteria = criteria.map(|(yes, no)| NoulCriteria { yes, no });
                let silent = criteria.as_ref().is_none_or(|c| {
                    c.yes.as_ref().is_none_or(Value::is_null)
                        && c.no.as_ref().is_none_or(Value::is_null)
                });
                let instructions = if instructions.is_null() && silent {
                    json!("ok?")
                } else {
                    instructions
                };
                (
                    Question::Noul {
                        instructions,
                        criteria,
                    },
                    false,
                )
            });
        let choice = (
            described(),
            vec((filled(), described()), 0..5),
            any::<bool>(),
        )
            .prop_map(|(instructions, options, from_request)| {
                let criteria = map(options);
                let from_request = from_request || criteria.len() < 2;
                (
                    Question::Choice {
                        instructions,
                        criteria,
                    },
                    from_request,
                )
            });
        let level = prop_oneof![
            text().prop_map(Value::String),
            vec(json(), 0..2).prop_map(Value::Array),
            vec((text(), json()), 0..2).prop_map(|kv| Value::Object(kv.into_iter().collect())),
        ];
        let score = (described(), vec(level, 2..5)).prop_map(|(instructions, criteria)| {
            (
                Question::Score {
                    instructions,
                    criteria,
                },
                false,
            )
        });
        let boxed = described().prop_map(|instructions| {
            let instructions = if instructions.is_null() {
                json!("the logo")
            } else {
                instructions
            };
            (Question::Box { instructions }, false)
        });
        (
            prop_oneof![noul, choice, score, boxed],
            option::of(path()),
            vec(path(), 4),
            any::<u8>(),
        )
            .prop_map(|((question, from_request), when, paths, mask)| {
                // Some of the instructions' parts, each under a path.
                let part_when = question
                    .instructions()
                    .as_object()
                    .map(|parts| {
                        parts
                            .keys()
                            .zip(&paths)
                            .enumerate()
                            .filter(|(i, _)| mask & (1 << i) != 0)
                            .map(|(_, (part, path))| (part.clone(), path.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                RubricQuestion {
                    question,
                    when,
                    part_when,
                    options_from: from_request.then_some(OptionsFrom::Request),
                }
            })
    }

    /// The raw draws a gate is built from, fitted to its question by [`gate`].
    type GateSeed = (
        Option<f64>,
        bool,
        Vec<(f64, String)>,
        Option<Index>,
        Option<(bool, Index)>,
        bool,
        Option<String>,
    );

    fn gate_seed() -> impl Strategy<Value = GateSeed> {
        (
            option::of(unit()),
            any::<bool>(),
            vec((unit(), filled()), 1..4),
            option::of(any::<Index>()),
            option::of((any::<bool>(), any::<Index>())),
            any::<bool>(),
            option::of(text()),
        )
    }

    /// A gate that fits `question` (`docs/reference/jud-format.md`, Policy).
    fn gate(question: &Question, seed: GateSeed) -> Gate {
        let (bar, banded, draws, fallback, level, strict, note) = seed;
        let mut gate = Gate {
            strict,
            note,
            ..Gate::default()
        };
        if let Question::Noul { .. } = question {
            gate.threshold = bar;
            return gate;
        }
        if banded {
            // Highest bar first, each lower and each named once.
            let mut draws = draws;
            draws.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (at_least, verdict) in draws {
                let lower = gate
                    .bands
                    .last()
                    .is_none_or(|b: &Band| at_least < b.at_least);
                if lower && gate.bands.iter().all(|b| b.verdict != verdict) {
                    gate.bands.push(Band { at_least, verdict });
                }
            }
        } else {
            gate.confidence = bar;
        }
        match question {
            Question::Choice { criteria, .. } if !criteria.is_empty() => {
                gate.fallback = fallback.map(|i| {
                    criteria
                        .get_index(i.index(criteria.len()))
                        .unwrap()
                        .0
                        .clone()
                });
            }
            Question::Score { criteria, .. } => {
                let pick = |i: &Index| i.index(criteria.len());
                gate.fallback = fallback.map(|i| match &criteria[pick(&i)] {
                    Value::String(text) => text.clone(),
                    _ => pick(&i).to_string(),
                });
                gate.level_at_least = level.map(|(by_index, i)| {
                    if by_index {
                        LevelRef::Index(pick(&i))
                    } else {
                        LevelRef::Text(pick(&i).to_string())
                    }
                });
            }
            _ => {}
        }
        gate
    }

    fn labels() -> impl Strategy<Value = IndexMap<String, String>> {
        vec((text(), text()), 0..3).prop_map(map)
    }

    fn extra(reserved: &'static [&'static str]) -> impl Strategy<Value = Vec<(String, Value)>> {
        vec(
            (
                text().prop_filter("a field of its own", move |k| {
                    !reserved.contains(&k.as_str())
                }),
                json(),
            ),
            0..3,
        )
    }

    pub fn rubric() -> impl Strategy<Value = Rubric> {
        (
            (
                name(),
                option::of(text()),
                option::of(text()),
                labels(),
                labels(),
            ),
            vec((filled(), question(), option::of(gate_seed())), 1..5),
            option::of((
                option::of(text()),
                option::of(text()),
                option::of(text()),
                option::of(text()),
                extra(&["cases", "model", "server", "tuned_at"]),
            )),
        )
            .prop_map(
                |((name, version, description, labels, annotations), questions, tuning)| {
                    let mut rubric = Rubric::new(name, judgment::Questions::new());
                    rubric.version = version;
                    rubric.description = description;
                    rubric.labels = labels;
                    rubric.annotations = annotations;
                    let mut gates = IndexMap::new();
                    for (id, question, seed) in questions {
                        // A repeated id replaces the question, and its gate with it.
                        gates.shift_remove(&id);
                        if let Some(seed) = seed {
                            gates.insert(id.clone(), gate(&question.question, seed));
                        }
                        rubric.questions.insert(id, question);
                    }
                    rubric.policy = Policy {
                        gates,
                        tuning: tuning.map(|(cases, model, server, tuned_at, extra)| Tuning {
                            cases,
                            model,
                            server,
                            tuned_at,
                            extra: map(extra),
                        }),
                    };
                    rubric
                },
            )
    }

    /// Two distinct fractions, lowest first.
    fn span() -> impl Strategy<Value = (f64, f64)> {
        (unit(), unit())
            .prop_filter("a region has width", |(a, b)| a.total_cmp(b).is_ne())
            .prop_map(|(a, b)| (a.min(b), a.max(b)))
    }

    fn region() -> impl Strategy<Value = BoxEdges> {
        (span(), span())
            .prop_map(|((top, bottom), (left, right))| BoxEdges::new(top, bottom, left, right))
    }

    fn expect() -> impl Strategy<Value = Expect> {
        prop_oneof![
            any::<bool>().prop_map(Expect::Bool),
            any::<u64>().prop_map(Expect::Index),
            text().prop_map(Expect::Text),
            option::of(any::<u64>()).prop_map(|from_turn| Expect::FromTurn(FromTurn { from_turn })),
            (region(), option::of(unit())).prop_map(|(edges, iou_at_least)| Expect::Box(
                BoxLabel {
                    edges,
                    iou_at_least
                }
            )),
        ]
    }

    fn case() -> impl Strategy<Value = (bool, Case)> {
        (
            any::<bool>(),
            json(),
            vec((text(), expect()), 0..4),
            vec((text(), vec((filled(), described()), 0..3)), 0..2),
            vec(text(), 0..3),
            option::of(text()),
        )
            .prop_map(|(has_id, state, expect, options, tags, note)| {
                let mut case = Case::new(state);
                case.expect = map(expect);
                case.options = options.into_iter().map(|(q, o)| (q, map(o))).collect();
                case.tags = tags;
                case.note = note;
                (has_id, case)
            })
    }

    /// Cases with and without ids (each id once), states that may repeat,
    /// and every form of label.
    pub fn cases() -> impl Strategy<Value = Cases> {
        (
            (
                name(),
                option::of(text()),
                labels(),
                labels(),
                option::of(text()),
            ),
            vec(case(), 1..5),
            any::<bool>(),
        )
            .prop_map(
                |((name, description, labels, annotations, rubric), drawn, repeat)| {
                    let mut cases: Vec<Case> = drawn
                        .into_iter()
                        .enumerate()
                        .map(|(i, (has_id, mut case))| {
                            case.id = has_id.then(|| format!("{name}-{i}"));
                            case
                        })
                        .collect();
                    if repeat && cases.len() > 1 {
                        let state = cases[0].state.clone();
                        cases.last_mut().unwrap().state = state;
                    }
                    Cases {
                        name,
                        description,
                        labels,
                        annotations,
                        rubric,
                        cases,
                    }
                },
            )
    }

    fn prob() -> impl Strategy<Value = Probability> {
        unit().prop_map(|p| Probability::new(p).unwrap())
    }

    fn conf() -> impl Strategy<Value = Confidence> {
        unit().prop_map(|c| Confidence::new(c).unwrap())
    }

    fn probabilities() -> impl Strategy<Value = BTreeMap<String, Probability>> {
        vec((text(), prob()), 0..4).prop_map(|kv| kv.into_iter().collect())
    }

    /// An answer of each kind a server may send, its unknown fields kept in
    /// `extra` under any key but the kind's own fields, and a kind this
    /// release does not know.
    fn answer() -> impl Strategy<Value = Answer> {
        let extras =
            |reserved| extra(reserved).prop_map(|kv| kv.into_iter().collect::<BTreeMap<_, _>>());
        prop_oneof![
            (prob(), extras(&["type", "noul"]))
                .prop_map(|(noul, extra)| Answer::Noul { noul, extra }),
            (
                text(),
                probabilities(),
                conf(),
                extras(&["type", "choice", "probabilities", "confidence"])
            )
                .prop_map(|(choice, probabilities, confidence, extra)| {
                    Answer::Choice {
                        choice,
                        probabilities,
                        confidence,
                        extra,
                    }
                }),
            (
                any::<f64>().prop_filter("finite", |f| f.is_finite()),
                vec((text(), json()), 0..4),
                probabilities(),
                conf(),
                extras(&["type", "score", "legend", "probabilities", "confidence"]),
            )
                .prop_map(|(score, legend, probabilities, confidence, extra)| {
                    Answer::Score {
                        score,
                        legend: legend.into_iter().collect(),
                        probabilities,
                        confidence,
                        extra,
                    }
                }),
            (
                region(),
                option::of(conf()),
                extras(&["type", "box", "confidence"])
            )
                .prop_map(|(edges, confidence, extra)| Answer::Box {
                    edges,
                    confidence,
                    extra
                }),
            (
                select(&["polygon", "Noul", "x-1", ""][..]),
                extras(&["type"])
            )
                .prop_map(|(kind, extra)| {
                    let mut raw: serde_json::Map<String, Value> = extra.into_iter().collect();
                    raw.insert("type".to_owned(), json!(kind));
                    Answer::Unknown(Value::Object(raw))
                }),
        ]
    }

    /// Whatever shape a server's response took: any model string, answers
    /// under any id, the usage, and top-level fields the crate does not know.
    fn response() -> impl Strategy<Value = Response> {
        (
            text(),
            vec((text(), answer()), 0..4),
            (
                any::<u64>(),
                any::<u64>(),
                option::of(any::<f64>().prop_filter("finite", |f| f.is_finite())),
            ),
            option::of(text()),
            extra(&["model", "answers", "usage", "request_id"]),
        )
            .prop_map(
                |(model, answers, (input_tokens, output_tokens, cost), request_id, extra)| {
                    Response {
                        model,
                        answers: answers.into_iter().collect(),
                        usage: Usage {
                            input_tokens,
                            output_tokens,
                            cost,
                        },
                        request_id,
                        extra: extra.into_iter().collect(),
                    }
                },
            )
    }

    pub fn recording() -> impl Strategy<Value = Recording> {
        (
            name(),
            response(),
            any::<u64>(),
            option::of(text()),
            option::of(text()),
            option::of(text()),
            option::of(text()),
            option::of(text()),
        )
            .prop_map(
                |(
                    case,
                    response,
                    elapsed_ms,
                    request_hash,
                    fingerprint,
                    rubric,
                    server,
                    recorded_at,
                )| {
                    let mut recording = Recording::new(case, response, elapsed_ms);
                    recording.request_hash = request_hash;
                    recording.fingerprint = fingerprint;
                    recording.rubric = rubric;
                    recording.server = server;
                    recording.recorded_at = recorded_at;
                    recording
                },
            )
    }
}

proptest::proptest! {
    #![proptest_config(generated::config())]

    /// A rubric written and read back is the rubric, writing it again gives
    /// the same text, and neither fingerprint moves: what a reviewer reads
    /// is what is asked and how it is gated.
    #[test]
    fn a_rubric_written_reads_back_as_itself_with_its_fingerprints(rubric in generated::rubric()) {
        let text = rubric.to_yaml().unwrap();
        let read = Rubric::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        proptest::prop_assert_eq!(&read, &rubric, "{}", text);
        proptest::prop_assert_eq!(read.to_yaml().unwrap(), text);
        proptest::prop_assert_eq!(read.fingerprint(), rubric.fingerprint());
        proptest::prop_assert_eq!(read.policy_fingerprint(), rubric.policy_fingerprint());
    }

    /// The same of a cases document, ids or none, repeated states, every
    /// form of label.
    #[test]
    fn cases_written_read_back_as_themselves_with_their_fingerprint(cases in generated::cases()) {
        let text = cases.to_yaml().unwrap();
        let read = Cases::parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        proptest::prop_assert_eq!(&read, &cases, "{}", text);
        proptest::prop_assert_eq!(read.to_yaml().unwrap(), text);
        proptest::prop_assert_eq!(read.fingerprint(), cases.fingerprint());
    }

    /// Whatever a server answered, the recording the writer makes of it
    /// reads back as that recording and writes back as the same text: an
    /// odd model name, an answer of a kind not known yet, fields nobody
    /// documented, at the top level or inside an answer.
    #[test]
    fn whatever_a_server_answered_the_recording_written_reads_back(recording in generated::recording()) {
        let text = recording_to_yaml(&recording).unwrap();
        let read = jud::parse_recording(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        proptest::prop_assert_eq!(&read, &recording, "{}", text);
        proptest::prop_assert_eq!(recording_to_yaml(&read).unwrap(), text);
    }
}

/// The `n`th line (one-based) of `base` replaced by `line`.
fn with_line(base: &str, n: usize, line: &str) -> String {
    let mut lines: Vec<&str> = base.lines().collect();
    lines[n - 1] = line;
    lines.join("\n") + "\n"
}

/// `line` put after the `n`th line (one-based) of `base`, so it is line `n + 1`.
fn with_line_after(base: &str, n: usize, line: &str) -> String {
    let mut lines: Vec<&str> = base.lines().collect();
    lines.insert(n, line);
    lines.join("\n") + "\n"
}

const DUP_RUBRIC: &str = "apiVersion: jud/v1\nkind: Rubric\nmetadata:\n  name: r\n  minor: 4\nspec:\n  questions:\n    q:\n      type: choice\n      instructions: pick in logo\n      criteria: {a: null, b: null}\n  assets:\n    logo: {path: logo.png}\n  policy:\n    q: {confidence: 0.5}\n";
const DUP_CASES: &str = "apiVersion: jud/v1\nkind: Cases\nmetadata:\n  name: c\n  minor: 4\nspec:\n  cases:\n    - id: one\n      state: {text: see logo}\n      expect:\n        q: a\n      assets:\n        logo: {path: logo.png}\n      options:\n        q: {x: null}\n";
const DUP_RECORDING: &str = "apiVersion: jud/v1\nkind: Recording\nmetadata:\n  name: c\n  minor: 4\nspec:\n  elapsed_ms: 1\n  response:\n    model: m\n    answers:\n      q: {type: noul, noul: 0.5}\n";

/// A duplicate key is an error (`docs/reference/jud-format.md`, Reading
/// rules) at every level of every kind, and the error names the line of
/// the second one: a reviewer reads the first, a lenient reader the last.
#[test]
fn a_duplicate_key_is_refused_at_every_level_by_its_line() {
    for base in [DUP_RUBRIC, DUP_CASES, DUP_RECORDING] {
        jud::parse(base).unwrap_or_else(|e| panic!("{e}\n{base}"));
    }
    let after = |base, n, line| (with_line_after(base, n, line), n + 1);
    let at = |base, n, line| (with_line(base, n, line), n);
    for (what, (text, line)) in [
        ("the envelope", after(DUP_RUBRIC, 2, "kind: Cases")),
        ("metadata", after(DUP_RUBRIC, 4, "  name: s")),
        ("spec", after(DUP_RUBRIC, 15, "  policy: {}")),
        (
            "spec.questions",
            after(DUP_RUBRIC, 11, "    q: {type: noul, instructions: ok?}"),
        ),
        (
            "a question's fields",
            after(DUP_RUBRIC, 10, "      instructions: again"),
        ),
        (
            "a Choice's options",
            at(
                DUP_RUBRIC,
                11,
                "      criteria: {a: null, a: null, b: null}",
            ),
        ),
        (
            "spec.assets",
            after(DUP_RUBRIC, 13, "    logo: {path: other.png}"),
        ),
        (
            "an asset's fields",
            at(DUP_RUBRIC, 13, "    logo: {path: logo.png, path: x.png}"),
        ),
        (
            "spec.policy",
            after(DUP_RUBRIC, 15, "    q: {confidence: 0.6}"),
        ),
        (
            "a gate's fields",
            at(DUP_RUBRIC, 15, "    q: {confidence: 0.5, confidence: 0.6}"),
        ),
        ("a case's fields", after(DUP_CASES, 8, "      id: two")),
        (
            "a state",
            at(DUP_CASES, 9, "      state: {text: see logo, text: again}"),
        ),
        ("expect", after(DUP_CASES, 11, "        q: b")),
        (
            "a case's assets",
            after(DUP_CASES, 13, "        logo: {path: x.png}"),
        ),
        (
            "a case's options",
            after(DUP_CASES, 15, "        q: {y: null}"),
        ),
        (
            "a recording's spec",
            after(DUP_RECORDING, 7, "  elapsed_ms: 2"),
        ),
        ("the response", after(DUP_RECORDING, 9, "    model: n")),
        (
            "the answers",
            after(DUP_RECORDING, 11, "      q: {type: noul, noul: 0.6}"),
        ),
        (
            "an answer's fields",
            at(
                DUP_RECORDING,
                11,
                "      q: {type: noul, noul: 0.5, noul: 0.6}",
            ),
        ),
    ] {
        let err = jud::parse(&text).unwrap_err();
        assert!(
            matches!(&err, jud::Error::Syntax(m) if m.contains("duplicate") && m.contains(&format!("line {line},"))),
            "{what}: {err}\n{text}"
        );
    }
}

/// A key is text whatever YAML reads it as (`docs/reference/jud-format.md`,
/// Reading rules), so `2024` and `"2024"` are one key, written twice: a
/// duplicate, to be refused. The YAML layer sees an integer and a string,
/// two keys, and the reader keeps the last under the one text, silently
/// dropping a label, a question, an option or an answer.
#[test]
#[ignore = "judgment-l47 (found by judgment-bsz.18.20): a key spelled as a number and as text collapses to one"]
fn a_key_spelled_as_a_number_and_as_text_is_a_duplicate() {
    let read_as_one = [
        ("metadata.labels", with_line_after(DUP_RUBRIC, 4, "  labels: {2024: a, \"2024\": b}")),
        (
            "spec.questions",
            with_line_after(DUP_RUBRIC, 11, "    1: {type: noul, instructions: one}\n    \"1\": {type: noul, instructions: two}"),
        ),
        ("a Choice's options", with_line(DUP_RUBRIC, 11, "      criteria: {1: null, \"1\": null, b: null}")),
        ("a state", with_line(DUP_CASES, 9, "      state: {1: see logo, \"1\": logo again}")),
        ("expect", with_line_after(DUP_CASES, 11, "        1: a\n        \"1\": b")),
        (
            "the answers",
            with_line_after(DUP_RECORDING, 11, "      1: {type: noul, noul: 0.1}\n      \"1\": {type: noul, noul: 0.9}"),
        ),
    ]
    .into_iter()
    .filter(|(_, text)| jud::parse(text).is_ok())
    .map(|(what, _)| what)
    .collect::<Vec<_>>();
    assert!(read_as_one.is_empty(), "read as one key: {read_as_one:?}");
}

/// A key is text whatever YAML reads it as, inside a question as in a
/// state: a Choice rated `1` to `3`, instructions in numbered parts. The
/// schema reads the keys as JSON does and takes these; the reader buffers a
/// question by its `type` and then refuses the integer key ("expected a
/// string key", at the question's line).
#[test]
#[ignore = "judgment-pry (found by judgment-bsz.18.20): a number key inside a question is refused"]
fn a_number_key_inside_a_question_is_read_as_text() {
    let refused = [
        "{type: choice, instructions: rate it, criteria: {1: poor, 2: fair, 3: good}}",
        "{type: noul, instructions: {1: first, 2: second}}",
        "{type: noul, instructions: {\"1\": first}, part_when: {1: x.y}}",
        "{type: score, instructions: rate it, criteria: [{1: low}, high]}",
        "{type: noul, instructions: x, criteria: {true: {1: yes}}}",
    ]
    .into_iter()
    .map(|q| {
        format!("apiVersion: jud/v1\nkind: Rubric\nmetadata: {{name: r, minor: 4}}\nspec:\n  questions:\n    q: {q}\n")
    })
    .filter(|text| {
        assert!(RUBRIC.is_valid(&value_of(text)), "the schema refuses:\n{text}");
        Rubric::parse(text).is_err()
    })
    .collect::<Vec<_>>();
    assert!(refused.is_empty(), "{refused:#?}");
}

/// The options a case supplies are asked in the order written, before the
/// static ones (`docs/reference/jud-format.md`, Declarations): the order
/// the model sees. Read through a `serde_json::Value` they were sorted
/// unless `preserve_order` was on (the `mcp` feature), so this holds under
/// `--features jud` as well as `--all-features`.
#[test]
fn supplied_options_are_read_in_the_order_written() {
    let text = with_line(DUP_CASES, 15, "        q: {zebra: null, apple: null}");
    let cases = Cases::parse(&text).unwrap();
    let order: Vec<&String> = cases.cases[0].options["q"].keys().collect();
    assert_eq!(order, ["zebra", "apple"]);
}

/// The example documents, each read once.
static EXAMPLE_TEXTS: LazyLock<Vec<String>> = LazyLock::new(|| {
    example_files("jud")
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
});

/// One edit a hand, a merge or a truncated write makes to a document.
#[derive(Debug, Clone)]
enum Mutation {
    Truncate(proptest::sample::Index),
    DuplicateLine(proptest::sample::Index),
    DeleteLine(proptest::sample::Index),
    SwapLines(proptest::sample::Index, proptest::sample::Index),
    SwapChars(proptest::sample::Index, proptest::sample::Index),
    Insert(proptest::sample::Index, char),
}

fn mutation() -> impl proptest::strategy::Strategy<Value = Mutation> {
    use proptest::prelude::*;
    use proptest::sample::{Index, select};
    prop_oneof![
        any::<Index>().prop_map(Mutation::Truncate),
        any::<Index>().prop_map(Mutation::DuplicateLine),
        any::<Index>().prop_map(Mutation::DeleteLine),
        (any::<Index>(), any::<Index>()).prop_map(|(a, b)| Mutation::SwapLines(a, b)),
        (any::<Index>(), any::<Index>()).prop_map(|(a, b)| Mutation::SwapChars(a, b)),
        (
            any::<Index>(),
            select(
                &[
                    ':', '-', '{', '}', '[', ']', '"', '\'', '\n', ' ', '\t', '#', '&', '*', '!',
                    '|', '>', '\0', '\u{feff}', 'é'
                ][..]
            ),
        )
            .prop_map(|(a, c)| Mutation::Insert(a, c)),
    ]
}

fn mutate(text: &str, mutation: &Mutation) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let (nc, nl) = (chars.len().max(1), lines.len().max(1));
    match mutation {
        Mutation::Truncate(i) => chars.truncate(i.index(nc)),
        Mutation::SwapChars(a, b) if !chars.is_empty() => chars.swap(a.index(nc), b.index(nc)),
        Mutation::Insert(a, c) => chars.insert(a.index(nc + 1).min(chars.len()), *c),
        Mutation::DuplicateLine(i) if !lines.is_empty() => {
            let i = i.index(nl);
            lines.insert(i, lines[i]);
            return lines.concat();
        }
        Mutation::DeleteLine(i) if !lines.is_empty() => {
            lines.remove(i.index(nl));
            return lines.concat();
        }
        Mutation::SwapLines(a, b) if !lines.is_empty() => {
            lines.swap(a.index(nl), b.index(nl));
            return lines.concat();
        }
        _ => {}
    }
    chars.into_iter().collect()
}

proptest::proptest! {
    #![proptest_config(generated::config())]

    /// Every reader is total over what a hand or a broken write makes of a
    /// valid document: a truncation, a line repeated, dropped or moved, two
    /// characters swapped, a stray one inserted. The answer is a document or
    /// an error, never a panic.
    #[test]
    fn a_mangled_document_is_read_or_refused_never_a_panic(
        which in proptest::prelude::any::<proptest::sample::Index>(),
        mutations in proptest::collection::vec(mutation(), 1..4),
    ) {
        let mut text = EXAMPLE_TEXTS[which.index(EXAMPLE_TEXTS.len())].clone();
        for m in &mutations {
            text = mutate(&text, m);
        }
        let _ = jud::parse(&text);
        let _ = Rubric::parse(&text);
        let _ = Cases::parse(&text);
        let _ = jud::parse_recording(&text);
    }
}

/// bsz.19.7: an error from reading a rubric, cases or recording file names
/// where (a path, a line, a field) and never what the document held. Every
/// slot of the example documents, as a value and as a key, is made to hold a
/// sentinel in a shape that is wrong for it; whatever the readers refuse, the
/// error text does not carry the sentinel.
mod untrusted_text {
    use super::*;

    const S: &str = "ZQ9SENTINELZQ9";

    /// Every document `seed` becomes with the sentinel in one slot.
    fn mutants(seed: &Value) -> Vec<Value> {
        fn paths(v: &Value, at: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
            out.push(at.clone());
            match v {
                Value::Object(m) => {
                    for (k, child) in m {
                        at.push(k.clone());
                        paths(child, at, out);
                        at.pop();
                    }
                }
                Value::Array(a) => {
                    for (i, child) in a.iter().enumerate() {
                        at.push(i.to_string());
                        paths(child, at, out);
                        at.pop();
                    }
                }
                _ => {}
            }
        }
        fn at_mut<'a>(v: &'a mut Value, p: &[String]) -> &'a mut Value {
            p.iter().fold(v, |v, k| match v {
                Value::Array(a) => &mut a[k.parse::<usize>().unwrap()],
                _ => &mut v[k.as_str()],
            })
        }
        let mut all = Vec::new();
        paths(seed, &mut Vec::new(), &mut all);
        let mut out = Vec::new();
        for p in &all {
            for wrong in [json!(S), json!([S]), json!({ S: S }), json!({ "x": S })] {
                let mut d = seed.clone();
                *at_mut(&mut d, p) = wrong;
                out.push(d);
            }
            let mut d = seed.clone();
            let slot = at_mut(&mut d, p);
            if let Value::Object(m) = slot {
                // As an unknown key, and as every known key renamed.
                m.insert(S.to_owned(), json!(1));
                out.push(d.clone());
                let mut d = seed.clone();
                if let Value::Object(m) = at_mut(&mut d, p) {
                    let keys: Vec<String> = m.keys().cloned().collect();
                    for k in keys {
                        let v = m.remove(&k).unwrap();
                        m.insert(format!("{S}{k}"), v);
                    }
                }
                out.push(d);
            }
        }
        out
    }

    fn seeds() -> Vec<Value> {
        [
            (EXAMPLES, "triage.jud"),
            (EXAMPLES, "triage-cases.jud"),
            (RECORDINGS, "close-account.jud"),
        ]
        .iter()
        .map(|(dir, name)| value_of(&read(dir, name)))
        .collect()
    }

    fn leaks(what: &str, doc: &Value, text: &str, found: &mut Vec<String>) {
        if text.contains(S) {
            found.push(format!("{what}: {text}  <= {doc}"));
        }
    }

    #[test]
    #[ignore = "judgment-ic8: found"]
    fn no_reader_error_quotes_the_document() {
        let mut found = Vec::new();
        for seed in seeds() {
            for doc in mutants(&seed) {
                let text = doc.to_string();
                if let Err(e) = jud::parse(&text) {
                    leaks("parse", &doc, &e.to_string(), &mut found);
                }
                if let Err(e) = Rubric::parse(&text) {
                    leaks("Rubric::parse", &doc, &e.to_string(), &mut found);
                }
                if let Err(e) = Cases::parse(&text) {
                    leaks("Cases::parse", &doc, &e.to_string(), &mut found);
                }
                if let Err(e) = jud::parse_recording(&text) {
                    leaks("parse_recording", &doc, &e.to_string(), &mut found);
                }
            }
        }
        found.sort();
        found.dedup_by_key(|f| f.split("  <=").next().unwrap().to_owned());
        assert!(
            found.is_empty(),
            "{} kinds of leak:\n{}",
            found.len(),
            found.join("\n")
        );
    }

    /// The `.json` recording a `Replay` directory holds: serde's own message.
    #[test]
    #[ignore = "judgment-ic8: found"]
    fn a_json_recording_error_quotes_nothing() {
        let seed = serde_json::to_value(
            jud::parse_recording(&read(RECORDINGS, "close-account.jud")).unwrap(),
        )
        .unwrap();
        let dir = tempdir().join("untrusted-json");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut found = Vec::new();
        for doc in mutants(&seed) {
            std::fs::write(dir.join("r.json"), doc.to_string()).unwrap();
            if let Err(e) = Replay::open(&dir) {
                leaks("Replay::open", &doc, &e.to_string(), &mut found);
            }
        }
        found.sort();
        found.dedup_by_key(|f| f.split("  <=").next().unwrap().to_owned());
        assert!(
            found.is_empty(),
            "{} kinds of leak:\n{}",
            found.len(),
            found.join("\n")
        );
    }
}
