//! Live checks against a real System One server: the wire as a server
//! actually speaks it, not as the mocks in `client.rs` assume it does.
//!
//! Every test is `#[ignore]`, so `cargo test` stays offline; run them by
//! hand against a server:
//!
//! ```sh
//! JUDGMENT_LIVE_BASE_URL=http://127.0.0.1:8000 JUDGMENT_LIVE_MODEL=typed-decisions \
//!   cargo test -p judgment --test live -- --ignored --nocapture
//! ```
//!
//! `JUDGMENT_LIVE_PROFILE` names the server (`typesafe`, `laya`, `autojev`
//! or `generic`, the default): where servers differ, a status code or a
//! limit, the behaviour is asserted for the server it was observed on and
//! printed for the others ([`Profile`]). Every body a test reads raw is
//! checked against the vendored OpenAPI document, and the answer is printed
//! for any server ([`check_body`]).
//!
//! `JUDGMENT_LIVE_API_KEY` defaults to `unused`, which a server that does not
//! check keys accepts. The bearer test needs a second server that does:
//! `JUDGMENT_LIVE_AUTH_BASE_URL` and `JUDGMENT_LIVE_AUTH_API_KEY`; it skips
//! when they are unset.
//!
//! What was learned running these against Laya is written up in
//! `docs/project/verification/laya-typed-decisions.md`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

use std::sync::LazyLock;
use std::time::Duration;

use judgment::{
    Answer, CallOptions, Client, Error, NoulCriteria, Questions, Recorder, Replay, Request,
    SystemOne, options,
};
use serde_json::{Value, json};

options! {
    enum Department {
        Billing = "billing" => "Payments, invoicing, refunds",
        Technical = "technical" => "Bugs, outages, integrations",
        NoneOfThese = "none_of_these" => "No team above fits",
    }
}

const STATE_PAYOUTS: &str =
    "My payouts have been failing for 3 days and nobody answers my tickets.";

/// `JUDGMENT_LIVE_BASE_URL`, read only after the profile is: every request
/// starts here, so a run the profile stops (the hosted API with none set)
/// stops before it has sent anything.
fn base_url() -> String {
    LazyLock::force(&PROFILE);
    std::env::var("JUDGMENT_LIVE_BASE_URL")
        .expect("set JUDGMENT_LIVE_BASE_URL to a System One server (see the file comment)")
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

/// The client under test, or a message that says how to point it somewhere.
fn client() -> Client {
    let base_url = base_url();
    Client::builder()
        .base_url(base_url)
        .api_key(env_or("JUDGMENT_LIVE_API_KEY", "unused"))
        .model(env_or("JUDGMENT_LIVE_MODEL", "typed-decisions"))
        // CPU inference answers in seconds; the default assumes a hosted API.
        .timeout(Duration::from_secs(120))
        .build()
        .unwrap()
}

fn sum(values: impl IntoIterator<Item = f64>) -> f64 {
    values.into_iter().sum()
}

// ---------------------------------------------------------------------------
// Which server: what is pinned, and where
// ---------------------------------------------------------------------------

/// The server under test, named by `JUDGMENT_LIVE_PROFILE`. Servers that
/// speak the wire still differ where the contract is silent (a refusal's
/// status, the limits, the budget), so a difference is pinned for the
/// server it was observed on and printed for the others. The model name
/// cannot say which server it is: `autojev-serve` answers to `jev-latest`
/// too, and was once pinned to the hosted API's status codes for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    /// The hosted TypeSafe API (`docs/project/verification/hosted-typesafe.md`).
    Typesafe,
    /// `laya-serve` 0.3.24 (`docs/project/verification/laya-typed-decisions.md`).
    Laya,
    /// `autojev-serve`, the server pplx-decider-v1.1-27b's checkpoint ships
    /// (`mise run live:pplx`).
    Autojev,
    /// Jev through `OpenRouter`'s System One API, base URL
    /// `https://openrouter.ai/api` (`mise run live:openrouter`). Its model
    /// list is `OpenRouter`'s own catalogue, which the client cannot decode;
    /// nothing else of it has been observed yet, so every other difference
    /// is printed.
    OpenRouter,
    /// Any other server: outcomes are printed, and only what the crate needs
    /// of every server is asserted.
    Generic,
}

/// The four limits probes of [`the_server_s_limits_match_the_reference_page`].
#[derive(Debug, Clone, Copy)]
enum Limit {
    Options256,
    Levels11,
    Options1,
    Levels1,
}

impl Profile {
    const NAMES: [(&'static str, Self); 5] = [
        ("typesafe", Self::Typesafe),
        ("laya", Self::Laya),
        ("autojev", Self::Autojev),
        ("openrouter", Self::OpenRouter),
        ("generic", Self::Generic),
    ];

    /// The hosts whose calls are billed, so that a run against one without
    /// a profile stops before it asks, and the profile each takes.
    const BILLED: [(&'static str, &'static str); 2] = [
        ("api.typesafe.ai", "typesafe"),
        ("openrouter.ai", "openrouter"),
    ];

    /// `JUDGMENT_LIVE_PROFILE`, or [`Profile::Generic`] when it is unset,
    /// except against the hosted API, where an unset profile stops the run.
    /// A name it does not know is a mistake to stop on, not a server to
    /// guess.
    fn from_env() -> Self {
        let Ok(name) = std::env::var("JUDGMENT_LIVE_PROFILE") else {
            // A billed server's pins are what a hand run against it is for;
            // falling to `generic` there would assert less without a word.
            let urls: Vec<String> = ["JUDGMENT_LIVE_BASE_URL", "JUDGMENT_LIVE_AUTH_BASE_URL"]
                .iter()
                .filter_map(|name| std::env::var(name).ok())
                .collect();
            if let Some((host, profile)) = Self::BILLED
                .iter()
                .find(|(host, _)| urls.iter().any(|url| url.contains(host)))
            {
                panic!(
                    "the server is {host}, whose calls are billed: set \
                     JUDGMENT_LIVE_PROFILE={profile} (or `generic` to assert only what every \
                     server owes)"
                );
            }
            eprintln!(
                "JUDGMENT_LIVE_PROFILE is not set: the generic profile, which prints where \
                 servers differ, schema verdicts included, and asserts what every server owes"
            );
            return Self::Generic;
        };
        if let Some((_, profile)) = Self::NAMES.iter().find(|(known, _)| *known == name) {
            return *profile;
        }
        let known: Vec<_> = Self::NAMES.iter().map(|(n, _)| *n).collect();
        panic!("JUDGMENT_LIVE_PROFILE={name}: one of {}", known.join(", "))
    }

    /// Whether `GET /v1/models` answers with something other than the
    /// documented list, so that the client's decode error is the expected
    /// outcome and the body is not held to the document. `OpenRouter` serves
    /// its own model catalogue at that path, as its TypeSafe-SDK guide says.
    fn model_list_is_foreign(self) -> bool {
        self == Self::OpenRouter
    }

    /// Whether a body this server sends is asserted to conform to the
    /// OpenAPI document, or only checked and printed. Asserted where a run
    /// found it held: the document is the hosted API's own, and
    /// autojev-serve's bodies held on 2026-10-09
    /// (`docs/project/verification/autojev-serve.md`). laya-serve's have not been
    /// checked yet, so they are reported until a run shows they hold.
    fn holds_to_the_schema(self) -> bool {
        matches!(self, Self::Typesafe | Self::Autojev)
    }

    /// The status of a limits probe on this server, where one was observed.
    fn limit_status(self, limit: Limit) -> Option<u16> {
        match (self, limit) {
            (Self::Typesafe, Limit::Options256 | Limit::Levels11) => Some(400),
            (Self::Laya, Limit::Options256) => Some(413),
            (Self::Autojev, Limit::Options256 | Limit::Levels11 | Limit::Levels1) => Some(422),
            (Self::Typesafe | Self::Laya | Self::Autojev, _) => Some(200),
            (Self::OpenRouter | Self::Generic, _) => None,
        }
    }

    /// The status of a request with an empty question id, where observed.
    fn empty_id_status(self) -> Option<u16> {
        match self {
            Self::Typesafe => Some(400),
            Self::Laya => Some(422),
            Self::Autojev => Some(200),
            Self::OpenRouter | Self::Generic => None,
        }
    }

    /// The status the over-budget state is refused with, where observed.
    /// autojev-serve has no budget of its own in front of the model, and
    /// what its tokenizer does past its window was not observed.
    fn over_budget_status(self) -> Option<u16> {
        match self {
            Self::Typesafe => Some(400),
            Self::Laya => Some(413),
            Self::Autojev | Self::OpenRouter | Self::Generic => None,
        }
    }
}

/// The profile of this run, read once.
static PROFILE: LazyLock<Profile> = LazyLock::new(Profile::from_env);

// ---------------------------------------------------------------------------
// The published contract, held against what the server actually sent
// ---------------------------------------------------------------------------

/// The vendored OpenAPI document (`tests/contract.rs` says how it is kept).
static SPEC: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("fixtures/typesafe-openapi.json")).unwrap());

/// The component the document gives for `status` on `method path`, or
/// `None` where it does not list that status: the hosted API's 400 and
/// laya-serve's 413 are not in it, only 200 and 422 are.
fn documented_schema(method: &str, path: &str, status: u16) -> Option<String> {
    SPEC["paths"][path][method]["responses"][status.to_string()]["content"]["application/json"]
        ["schema"]["$ref"]
        .as_str()
        .and_then(|r| r.strip_prefix("#/components/schemas/"))
        .map(str::to_owned)
}

/// Every way `body` fails `schema`, one line each. A failing `oneOf` over
/// the answer kinds fails at the answer as a whole, so the failures of the
/// branch for the answer's own `type` follow it: they say which field made
/// it none of the kinds.
fn violations(schema: &str, body: &Value) -> Vec<String> {
    fn flatten(error: &jsonschema::ValidationError<'_>, out: &mut Vec<String>) {
        use jsonschema::error::ValidationErrorKind as Kind;
        let at = error.instance_path().to_string();
        let mut message = error.to_string();
        if let Some((cut, _)) = message.char_indices().nth(160) {
            message.truncate(cut);
        }
        out.push(format!(
            "{}: {message}",
            if at.is_empty() { "(root)" } else { &at }
        ));
        if let Kind::AnyOf { context }
        | Kind::OneOfNotValid { context }
        | Kind::OneOfMultipleValid { context } = error.kind()
        {
            // A branch for another kind fails on `type` too; only the
            // branch for the kind the answer says it is has news.
            for branch in context {
                let mut lines = Vec::new();
                for nested in branch {
                    flatten(nested, &mut lines);
                }
                if !lines
                    .iter()
                    .any(|l| l.contains("/type: ") && l.ends_with(" was expected"))
                {
                    out.extend(lines);
                }
            }
        }
    }
    let validator = jsonschema::draft202012::new(&json!({
        "$ref": format!("#/components/schemas/{schema}"),
        "components": SPEC["components"],
    }))
    .unwrap();
    let mut out = Vec::new();
    for error in validator.iter_errors(body) {
        flatten(&error, &mut out);
    }
    out
}

/// Checks a body the server sent for `method path` against the document and
/// prints the verdict. A success body the profile holds to the schema fails
/// the test when it does not conform: the crate's decoding is written
/// against it, so a difference there is a contract break, not a quirk. An
/// error body is checked where its status is documented (422) and printed;
/// a status the document does not list is named as such.
fn check_body(profile: Profile, method: &str, path: &str, status: u16, text: &str) {
    let what = format!("{} {path} {status}", method.to_uppercase());
    let Some(schema) = documented_schema(method, path, status) else {
        eprintln!("schema: {what} is a status the document does not list");
        return;
    };
    let failures = match serde_json::from_str::<Value>(text) {
        Ok(body) => violations(&schema, &body),
        Err(e) => vec![format!("not JSON: {e}")],
    };
    if failures.is_empty() {
        eprintln!("schema: {what} conforms to {schema}");
        return;
    }
    eprintln!(
        "schema: {what} does not conform to {schema}:\n  {}",
        failures.join("\n  ")
    );
    // Only a success is held: the crate decodes it against the document,
    // where a refusal's body is read for its message and nothing more.
    assert!(
        !(200..300).contains(&status) || !profile.holds_to_the_schema(),
        "{what}: the {profile:?} profile holds this server to the document"
    );
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn the_three_primitives_round_trip_through_typed_handles() {
    let mut q = Questions::new();
    let dept = q
        .choice::<Department>("department", "Which team should handle `message`?")
        .unwrap();
    let urgent = q
        .noul(
            "urgent",
            "Does `message` convey urgency?",
            Some(NoulCriteria::new(
                "The customer needs an answer today",
                "It can wait",
            )),
        )
        .unwrap();
    let severity = q
        .score(
            "severity",
            "How severe is the problem in `message`?",
            ["cosmetic", "degraded", "blocked"],
        )
        .unwrap();

    let response = client()
        .system_one(&json!({ "message": STATE_PAYOUTS }), &q)
        .await
        .unwrap();

    assert!(!response.model.is_empty(), "the response names a model");
    assert!(
        response.usage.input_tokens > 0,
        "input tokens are counted: {:?}",
        response.usage
    );
    assert_eq!(response.answers.len(), 3);
    // Every answer is of a kind this release knows: an `Answer::Unknown`
    // here means the server has a primitive the crate cannot read yet.
    for (id, answer) in &response.answers {
        assert!(
            !matches!(answer, Answer::Unknown(_)),
            "{id}: an answer of a kind this build does not know: {answer:?}"
        );
    }

    let dept = response.get(&dept).unwrap();
    assert_eq!(dept.probabilities.len(), 3, "one probability per option");
    assert!(
        (sum(dept.probabilities.values().map(|p| p.value())) - 1.0).abs() < 0.01,
        "the choice distribution sums to 1 within rounding: {:?}",
        dept.probabilities
    );
    let top = dept
        .probabilities
        .iter()
        .max_by(|a, b| a.1.value().total_cmp(&b.1.value()))
        .map(|(k, _)| *k)
        .unwrap();
    assert_eq!(
        dept.chosen, top,
        "chosen is the arg max of the distribution"
    );
    assert!((0.0..=1.0).contains(&dept.confidence.value()));

    let urgent = response.get(&urgent).unwrap();
    assert!((0.0..=1.0).contains(&urgent.yes.value()));

    let severity = response.get(&severity).unwrap();
    assert_eq!(
        severity.levels,
        vec!["cosmetic", "degraded", "blocked"],
        "the legend echoes the levels in order"
    );
    assert!(
        (0.0..=2.0).contains(&severity.value),
        "the score lies on the level line: {}",
        severity.value
    );
    assert!((sum(severity.probabilities.iter().map(|p| p.value())) - 1.0).abs() < 0.01);
    assert!(severity.nearest_level() <= 2);

    eprintln!("request id: {:?}", response.request_id);
    eprintln!(
        "undocumented top-level fields: {:?}",
        response.extra.keys().collect::<Vec<_>>()
    );
    eprintln!(
        "model {}: department {:?} p={:.3} conf={:.3}; urgent {:.3}; severity {:.2} ({})",
        response.model,
        dept.chosen,
        dept.probabilities[&dept.chosen].value(),
        dept.confidence.value(),
        urgent.yes.value(),
        severity.value,
        severity.nearest_label()
    );
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn a_structured_score_level_comes_back_decoded() {
    // The API allows a level described as an object. A server echoes it in
    // the legend as the object (the hosted API) or as its JSON text
    // (laya-serve 0.3.22 and later); the first would fail a legend typed as
    // strings, the second a comparison on the text, so the label is checked
    // by what it parses to.
    let mut q = Questions::new();
    let severity = q
        .score(
            "severity",
            "How severe is the problem in `message`?",
            [
                json!({ "what": "cosmetic", "examples": ["a typo in the invoice footer"] }),
                json!("degraded"),
                json!("blocked"),
            ],
        )
        .unwrap();
    let response = client()
        .system_one(&json!({ "message": STATE_PAYOUTS }), &q)
        .await
        .unwrap();
    let severity = response.get(&severity).unwrap();
    assert_eq!(severity.levels.len(), 3);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&severity.levels[0]).unwrap(),
        json!({ "what": "cosmetic", "examples": ["a typo in the invoice footer"] }),
        "a structured level is labelled by its JSON: {:?}",
        severity.levels
    );
    assert_eq!(severity.levels[1], "degraded");
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn a_structured_score_level_is_echoed() {
    // `Response::verify` accepts a structured level echoed as itself (the
    // hosted API, laya-serve to 0.3.21) or as a string that parses to it
    // (laya-serve 0.3.22 and later, with Python's separators). This sends
    // one, prints the legend exactly as the server echoed it, and checks it
    // passes, so a third form shows up here before it fails a caller.
    // The body is fetched directly, not through the client, so the echo is
    // printed even when it does not verify.
    let mut q = Questions::new();
    q.score(
        "severity",
        "How severe is the problem in `message`?",
        [
            json!({ "what": "cosmetic", "examples": ["a typo in the invoice footer"] }),
            json!(["degraded", "some users cannot pay"]),
            json!("blocked"),
        ],
    )
    .unwrap();
    let base_url = base_url();
    let state = json!({ "message": STATE_PAYOUTS });
    let model = env_or("JUDGMENT_LIVE_MODEL", "typed-decisions");
    let body = serde_json::to_vec(&Request {
        state: &state,
        model: &model,
        questions: &q,
    })
    .unwrap();
    let sent = reqwest::Client::new()
        .post(format!("{}/v1/systemone", base_url.trim_end_matches('/')))
        .bearer_auth(env_or("JUDGMENT_LIVE_API_KEY", "unused"))
        .header("content-type", "application/json")
        .timeout(Duration::from_secs(120))
        .body(body)
        .send()
        .await
        .unwrap();
    let status = sent.status().as_u16();
    let text = sent.text().await.unwrap();
    check_body(*PROFILE, "post", "/v1/systemone", status, &text);
    assert_eq!(status, 200, "{text}");
    let response: judgment::Response = serde_json::from_str(&text).unwrap();
    match &response.answers["severity"] {
        Answer::Score { legend, .. } => {
            eprintln!("echoed legend: {}", serde_json::to_string(legend).unwrap());
        }
        other => eprintln!("not a score: {}", serde_json::to_string(other).unwrap()),
    }
    response.verify(&q).unwrap();
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn every_body_the_server_sends_holds_to_the_published_document() {
    // The client decodes into the crate's types, which tolerate more than
    // the document allows (`tests/contract.rs`, "The pinned gaps"), so a
    // server can pass every other test here and still send what the
    // contract does not. This sends the request shapes the builders produce
    // and checks the bodies themselves, plus the model list where it is
    // served. How each check ends is printed for every server; the profiles
    // that hold their server to the document fail on a difference.
    let mut q = Questions::new();
    q.choice::<Department>("department", "Which team should handle `message`?")
        .unwrap();
    q.noul(
        "urgent",
        "Does `message` convey urgency?",
        Some(NoulCriteria::new(
            "The customer needs an answer today",
            "It can wait",
        )),
    )
    .unwrap();
    q.noul("angry", "Is the writer of `message` angry?", None)
        .unwrap();
    q.score(
        "severity",
        "How severe is the problem in `message`?",
        ["cosmetic", "degraded", "blocked"],
    )
    .unwrap();
    let state = json!({ "message": STATE_PAYOUTS });
    let model = requested_model();
    let body = serde_json::to_value(Request {
        state: &state,
        model: &model,
        questions: &q,
    })
    .unwrap();
    let (status, text) = post_raw(&body).await;
    assert_eq!(status, 200, "{text}");

    let base_url = base_url();
    let listed = reqwest::Client::new()
        .get(format!("{}/v1/models", base_url.trim_end_matches('/')))
        .bearer_auth(env_or("JUDGMENT_LIVE_API_KEY", "unused"))
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .unwrap();
    let status = listed.status().as_u16();
    let text = listed.text().await.unwrap();
    // A 404 is a list not served (`the_model_list_is_either_served_or_absent`),
    // and a foreign list is a catalogue the document does not describe.
    if status != 404 && !PROFILE.model_list_is_foreign() {
        check_body(*PROFILE, "get", "/v1/models", status, &text);
    }
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn a_choice_option_without_a_description_is_accepted() {
    let mut q = Questions::new();
    let team = q
        .dynamic_choice(
            "team",
            "Which team should handle `message`?",
            [
                (
                    "billing".to_owned(),
                    Some("Payments and refunds".to_owned()),
                ),
                ("technical".to_owned(), None),
                ("none_of_these".to_owned(), None),
            ],
        )
        .unwrap();
    let response = client()
        .system_one(&json!({ "message": STATE_PAYOUTS }), &q)
        .await
        .unwrap();
    let team = response.get(&team).unwrap();
    assert_eq!(team.probabilities.len(), 3);
    assert!(["billing", "technical", "none_of_these"].contains(&team.chosen.as_str()));
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn the_model_list_is_either_served_or_absent() {
    // `GET /v1/models` is in the OpenAPI document and both SDKs call it, but
    // the HTTP API reference page leaves it out and a compatible server may
    // not serve it (laya-serve does not, 0.3.24 included). Either outcome is
    // acceptable; what is not is anything other than a clean success or a
    // clean 404, apart from a gateway that serves its own catalogue at the
    // path (OpenRouter), whose list the client cannot decode by design.
    match client().list_models().await {
        Ok(models) => {
            assert!(!models.is_empty());
            eprintln!(
                "models: {}",
                models
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Err(Error::Http { status: 404, .. }) => eprintln!("GET /v1/models: 404, not served"),
        Err(err @ Error::Decode { .. }) if PROFILE.model_list_is_foreign() => {
            eprintln!("GET /v1/models: the server's own catalogue, as expected: {err}");
        }
        Err(other) => panic!("unexpected: {other}"),
    }
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn a_model_name_the_server_does_not_know_is_still_answered() {
    // A client built for the hosted API sends `jev-latest`; a server that
    // routes by name must not fail the request for it. Which checkpoint
    // answered is the server's business; the response says so in `model`.
    let base_url = base_url();
    let client = Client::builder()
        .base_url(base_url)
        .api_key(env_or("JUDGMENT_LIVE_API_KEY", "unused"))
        .model("jev-latest")
        .timeout(Duration::from_secs(300))
        .build()
        .unwrap();
    let mut q = Questions::new();
    let urgent = q
        .noul("urgent", "Does `message` convey urgency?", None)
        .unwrap();
    let response = client
        .system_one(&json!({ "message": STATE_PAYOUTS }), &q)
        .await
        .unwrap();
    response.get(&urgent).unwrap();
    eprintln!("jev-latest was answered by model {:?}", response.model);
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn a_live_answer_replays_offline_from_its_recording() {
    let dir = std::env::temp_dir().join(format!("judgment-live-{}", std::process::id()));
    let mut q = Questions::new();
    let dept = q
        .choice::<Department>("department", "Which team should handle `message`?")
        .unwrap();
    let state = json!({ "message": STATE_PAYOUTS });
    let model = env_or("JUDGMENT_LIVE_MODEL", "typed-decisions");

    let recorder = Recorder::new(client(), &dir);
    let live = recorder.answer(&state, &model, &q).await.unwrap();

    let replay = Replay::open(&dir).unwrap();
    assert_eq!(replay.len(), 1);
    let replayed = replay.answer(&state, &model, &q).await.unwrap();
    assert_eq!(
        replayed, live,
        "the replay is the recorded response, byte for byte"
    );
    assert_eq!(
        replayed.get(&dept).unwrap().chosen,
        live.get(&dept).unwrap().chosen
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Records the policy of the server under test for a top-level body field
/// it does not document: it may answer as if the field were absent, use
/// it, or refuse the request with a 400 or 422 naming it. The crate sends
/// extra fields as given and leaves the choice to the server, so any of
/// those is a pass; the outcome is printed so a run says which one this
/// server takes. Anything else (a 5xx, a transport failure, a decode
/// error) fails.
#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn an_unknown_extra_field_is_answered_or_refused_by_name() {
    let client = client();
    let mut q = Questions::new();
    let urgent = q
        .noul("urgent", "Does `message` convey urgency?", None)
        .unwrap();
    let state = json!({ "message": STATE_PAYOUTS });
    let options = CallOptions::new()
        .extra("judgment_live_unknown_field", 4)
        .unwrap();
    let request = Request {
        state: &state,
        model: client.model(),
        questions: &q,
    };
    match client.evaluate_with(&request, &options).await {
        Ok(response) => {
            response.get(&urgent).unwrap();
            eprintln!(
                "an unknown extra field was answered (ignored or used) by model {:?}",
                response.model
            );
        }
        Err(Error::InvalidRequest {
            status: status @ (400 | 422),
            detail,
            issues,
            kind,
            request_id,
        }) => eprintln!(
            "an unknown extra field was refused with {status}: {detail} \
             (kind {kind:?}, issues {:?}, request id {request_id:?})",
            issues.iter().map(ToString::to_string).collect::<Vec<_>>()
        ),
        Err(other) => panic!("unexpected: {other}"),
    }
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn the_builder_refuses_what_the_reference_page_forbids() {
    // Checked before any request: no server involved, but listed here so a
    // live run shows the limits next to the behaviour they guard. The limits
    // are the HTTP API reference page's (255 options, 2 to 10 levels); the
    // OpenAPI document states only `minItems: 1` for levels, and what a
    // server does past either limit is unverified. `tests/contract.rs` pins
    // the difference between the builder and the schema.
    let mut q = Questions::new();
    let too_many = (0..256).map(|i| (format!("opt{i}"), None));
    let err = q.dynamic_choice("c", "pick", too_many).unwrap_err();
    assert!(matches!(err, Error::InvalidQuestion { .. }), "{err}");
    let err = q.score("s", "rate", ["only one"]).unwrap_err();
    assert!(matches!(err, Error::InvalidQuestion { .. }), "{err}");
    let err = q
        .score("s", "rate", [json!("low"), json!(null)])
        .unwrap_err();
    assert!(matches!(err, Error::InvalidQuestion { .. }), "{err}");
}

#[tokio::test]
#[ignore = "needs an authenticating server: JUDGMENT_LIVE_AUTH_BASE_URL"]
async fn a_wrong_bearer_token_is_unauthorized_and_the_right_one_is_not() {
    LazyLock::force(&PROFILE);
    let Ok(base_url) = std::env::var("JUDGMENT_LIVE_AUTH_BASE_URL") else {
        eprintln!("skipped: JUDGMENT_LIVE_AUTH_BASE_URL is not set");
        return;
    };
    let key = std::env::var("JUDGMENT_LIVE_AUTH_API_KEY")
        .expect("JUDGMENT_LIVE_AUTH_API_KEY goes with JUDGMENT_LIVE_AUTH_BASE_URL");
    let build = |key: &str| {
        Client::builder()
            .base_url(base_url.clone())
            .api_key(key)
            .model(env_or("JUDGMENT_LIVE_MODEL", "typed-decisions"))
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap()
    };
    let mut q = Questions::new();
    let urgent = q
        .noul("urgent", "Does `message` convey urgency?", None)
        .unwrap();
    let state = json!({ "message": STATE_PAYOUTS });

    let err = build("not-the-key")
        .system_one(&state, &q)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Unauthorized { .. }), "{err}");
    eprintln!("401 request id: {:?}", err.request_id());

    let response = build(&key).system_one(&state, &q).await.unwrap();
    response.get(&urgent).unwrap();
}

// ---------------------------------------------------------------------------
// Past the builder: what the hosted API does with what the crate refuses, and
// what repeats and formulas show. Added after the 2026-10-03 probe run
// (docs/project/verification/hosted-typesafe.md, "Beyond the test file").
// A behaviour is pinned for the profiles it was observed on ([`Profile`])
// and printed for the others.
// ---------------------------------------------------------------------------

/// Posts `body` to the live server's `/v1/systemone` exactly as given,
/// bypassing the builder, for the shapes the builder refuses. Returns the
/// status and the body text.
async fn post_raw(body: &serde_json::Value) -> (u16, String) {
    let base_url = base_url();
    let response = reqwest::Client::new()
        .post(format!("{}/v1/systemone", base_url.trim_end_matches('/')))
        .bearer_auth(env_or("JUDGMENT_LIVE_API_KEY", "unused"))
        .header("content-type", "application/json")
        .timeout(Duration::from_secs(120))
        .body(serde_json::to_vec(body).unwrap())
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    check_body(*PROFILE, "post", "/v1/systemone", status, &text);
    (status, text)
}

fn requested_model() -> String {
    env_or("JUDGMENT_LIVE_MODEL", "typed-decisions")
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn the_server_s_limits_match_the_reference_page() {
    // The builder stops at 255 options and 10 levels and starts at 2 of
    // each. The hosted API refuses what is above (a 400 with a sentence)
    // and answers what is below with probability 1, so the upper bounds are
    // the server's and the lower ones the crate's alone (the rustdoc of
    // `question`, `# Limits are checked here`). laya-serve's own upper
    // limits are lower (100 options, as a 413) and it answers one option or
    // one level too; autojev-serve refuses 11 levels and one level as a 422.
    // Each outcome is printed, and asserted where its profile observed it.
    let model = requested_model();
    let profile = *PROFILE;
    let state = json!({ "message": STATE_PAYOUTS });
    let options = |n: usize| -> serde_json::Value {
        (0..n)
            .map(|i| (format!("opt_{i:03}"), serde_json::Value::Null))
            .collect::<serde_json::Map<_, _>>()
            .into()
    };
    let levels = |n: usize| -> serde_json::Value {
        (0..n)
            .map(|i| format!("level {i}"))
            .collect::<Vec<_>>()
            .into()
    };
    let cases = [
        (
            "256 options",
            json!({ "c": { "type": "choice", "instructions": "Which?", "criteria": options(256) } }),
            Limit::Options256,
        ),
        (
            "11 levels",
            json!({ "s": { "type": "score", "instructions": "How bad?", "criteria": levels(11) } }),
            Limit::Levels11,
        ),
        (
            "1 option",
            json!({ "c": { "type": "choice", "instructions": "Which?", "criteria": options(1) } }),
            Limit::Options1,
        ),
        (
            "1 level",
            json!({ "s": { "type": "score", "instructions": "How bad?", "criteria": levels(1) } }),
            Limit::Levels1,
        ),
    ];
    for (what, questions, limit) in cases {
        let (status, text) =
            post_raw(&json!({ "model": model, "state": state, "questions": questions })).await;
        eprintln!(
            "{what}: {status} {}",
            text.chars().take(160).collect::<String>()
        );
        let Some(expected) = profile.limit_status(limit) else {
            continue;
        };
        assert_eq!(status, expected, "{what} on {profile:?}: {text}");
        if profile != Profile::Typesafe {
            continue;
        }
        if status == 400 {
            assert!(text.contains("at most"), "{what}: {text}");
        } else {
            let response: judgment::Response = serde_json::from_str(&text).unwrap();
            let answer = response.answers.values().next().unwrap();
            // Two arms with one body: the bindings differ in type.
            #[allow(clippy::match_same_arms)]
            let (count, confidence) = match answer {
                Answer::Choice {
                    probabilities,
                    confidence,
                    ..
                } => (probabilities.len(), confidence.value()),
                Answer::Score {
                    probabilities,
                    confidence,
                    ..
                } => (probabilities.len(), confidence.value()),
                other => panic!("{what}: {other:?}"),
            };
            assert_eq!(count, 1, "{what}: {answer:?}");
            assert!((confidence - 1.0).abs() < 1e-9, "{what}: {answer:?}");
        }
    }
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn an_empty_question_id_is_refused_by_the_server_too() {
    // The builder refuses it (`question` unit tests); this is the hosted
    // API's half of the reason: a 400 that says `Question key cannot be
    // empty.` laya-serve refuses it as a 422; autojev-serve answers it.
    let model = requested_model();
    let (status, text) = post_raw(&json!({
        "model": model,
        "state": { "message": STATE_PAYOUTS },
        "questions": { "": { "type": "noul", "instructions": "Does `message` convey urgency?" } },
    }))
    .await;
    eprintln!("empty id: {status} {text}");
    let profile = *PROFILE;
    if let Some(expected) = profile.empty_id_status() {
        assert_eq!(status, expected, "empty id on {profile:?}: {text}");
    }
    if profile == Profile::Typesafe {
        assert!(text.contains("cannot be empty"), "{text}");
    }
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn repeated_calls_agree_on_the_answer_but_not_on_the_hundredths() {
    // The hosted API is not deterministic at the two decimals it sends.
    // On 2026-10-03, three identical requests gave 0.92, 0.93, 0.93 for the
    // same option in one run and a spread of 0.05 (confidence 0.89, 0.83,
    // 0.90) in the next; the chosen option, the nearest level and the
    // expected score held every time. This pins what a caller may rely on,
    // the decision, prints the spread so a run that widens shows, and fails
    // only on a spread no rounding or batching explains.
    let mut q = Questions::new();
    let dept = q
        .choice::<Department>("department", "Which team should handle `message`?")
        .unwrap();
    let severity = q
        .score(
            "severity",
            "How severe is the problem in `message`?",
            ["cosmetic", "degraded", "blocked"],
        )
        .unwrap();
    let client = client();
    let state = json!({ "message": STATE_PAYOUTS });
    let mut responses = Vec::new();
    for _ in 0..3 {
        responses.push(client.system_one(&state, &q).await.unwrap());
    }
    let depts: Vec<_> = responses.iter().map(|r| r.get(&dept).unwrap()).collect();
    let severities: Vec<_> = responses
        .iter()
        .map(|r| r.get(&severity).unwrap())
        .collect();
    assert!(
        depts.iter().all(|d| d.chosen == depts[0].chosen),
        "the chosen option moved between identical requests: {:?}",
        depts.iter().map(|d| d.chosen).collect::<Vec<_>>()
    );
    assert!(
        severities
            .iter()
            .all(|s| s.nearest_level() == severities[0].nearest_level()),
        "the nearest level moved between identical requests"
    );
    let range = |values: &[f64]| {
        values.iter().copied().fold(f64::MIN, f64::max)
            - values.iter().copied().fold(f64::MAX, f64::min)
    };
    let probability_spread = depts[0]
        .probabilities
        .keys()
        .map(|option| {
            range(
                &depts
                    .iter()
                    .map(|d| d.probability_of(option))
                    .collect::<Vec<_>>(),
            )
        })
        .fold(0.0, f64::max);
    let score_spread = range(&severities.iter().map(|s| s.value).collect::<Vec<_>>());
    eprintln!(
        "spread over 3 identical requests: probability {probability_spread:.3}, score \
         {score_spread:.3}, confidence {:?}",
        depts
            .iter()
            .map(|d| d.confidence.value())
            .collect::<Vec<_>>()
    );
    assert!(
        probability_spread <= 0.15,
        "probabilities spread by {probability_spread}"
    );
    assert!(score_spread <= 0.15, "the score spread by {score_spread}");
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn confidence_follows_the_documented_formulas_on_jev() {
    // The confidence page gives the formulas; the OpenAPI document defines
    // `score` as the expected level. On the hosted API all three held
    // within rounding on 2026-10-03. Laya defines confidence otherwise, so
    // on any other profile the differences are printed and not asserted; they are
    // what `Choice::confidence_from_probabilities` and its Score siblings
    // exist to show.
    let mut q = Questions::new();
    let dept = q
        .choice::<Department>("department", "Which team should handle `message`?")
        .unwrap();
    let severity = q
        .score(
            "severity",
            "How severe is the problem in `message`?",
            ["cosmetic", "degraded", "blocked"],
        )
        .unwrap();
    let response = client()
        .system_one(&json!({ "message": STATE_PAYOUTS }), &q)
        .await
        .unwrap();
    let d = response.get(&dept).unwrap();
    let s = response.get(&severity).unwrap();
    let choice_gap = (d.confidence.value() - d.confidence_from_probabilities()).abs();
    let score_gap = (s.confidence.value() - s.confidence_from_probabilities()).abs();
    let value_gap = (s.value - s.expected_value()).abs();
    eprintln!(
        "model {}: choice confidence wire {:.2} formula {:.3}; score confidence wire {:.2} \
         formula {:.3}; score wire {:.2} expected value {:.3}",
        response.model,
        d.confidence.value(),
        d.confidence_from_probabilities(),
        s.confidence.value(),
        s.confidence_from_probabilities(),
        s.value,
        s.expected_value()
    );
    if *PROFILE == Profile::Typesafe {
        assert!(choice_gap <= 0.015, "choice confidence off by {choice_gap}");
        assert!(score_gap <= 0.02, "score confidence off by {score_gap}");
        assert!(
            value_gap <= 0.011,
            "score off its expected value by {value_gap}"
        );
    }
}

#[tokio::test]
#[ignore = "needs a live server: JUDGMENT_LIVE_BASE_URL"]
async fn a_state_over_the_budget_is_refused_as_too_large() {
    // About 40,000 tokens of state against a 32,000-token budget: the hosted
    // API answers 400 `{"detail": {"error_type": "max_tokens_exceeded"}}`,
    // no message, which the client keeps as `kind`; laya-serve answers 413
    // `{"detail": "state too large (50600 > 50000 chars)"}` at its 50,000
    // characters. Both are `Error::is_request_too_large`; autojev-serve
    // answered it, having no budget in front of its model. The request is
    // about 500 KB; nothing is billed for a refused body.
    let noise = vec!["lorem ipsum dolor sit amet consectetur adipiscing elit ".repeat(20); 450];
    let state = json!({ "message": STATE_PAYOUTS, "noise": noise });
    let mut q = Questions::new();
    q.noul("urgent", "Does `message` convey urgency?", None)
        .unwrap();
    let profile = *PROFILE;
    let expected = profile.over_budget_status();
    match client().system_one(&state, &q).await {
        Err(err @ Error::InvalidRequest { .. }) => {
            eprintln!("over budget: {err}");
            if let Some(expected) = expected {
                assert!(
                    matches!(&err, Error::InvalidRequest { status, .. } if *status == expected),
                    "{profile:?} refuses it with {expected}: {err:?}"
                );
            }
            if profile == Profile::Typesafe {
                assert!(
                    matches!(&err, Error::InvalidRequest { kind: Some(kind), .. } if kind == "max_tokens_exceeded"),
                    "{err:?}"
                );
            }
            assert!(err.is_request_too_large(), "{err:?}");
        }
        Ok(response) => {
            eprintln!("over budget: answered by {}", response.model);
            assert!(
                expected.is_none(),
                "{profile:?} refuses it with {expected:?}, and {} answered it",
                response.model
            );
        }
        Err(other) => {
            eprintln!("over budget: {other}");
            assert!(
                expected.is_none(),
                "{profile:?} refuses it with {expected:?}: {other}"
            );
        }
    }
}
