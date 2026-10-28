//! The wire as a whole, in tables and properties: questions and options go
//! out in insertion order on every path (`judgment-bsz.18.7`); no caller
//! header or field re-aims a call or makes a billed 2xx unreadable
//! (`.18.10`); each departure a real server was seen to make maps to its
//! documented outcome offline (`.18.14`); and every failure class the
//! client can build from a response or a transport is produced by some
//! scenario, with its predicates (`.18.24`).

#![cfg(feature = "http")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::missing_panics_doc)]

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::LazyLock;
use std::time::Duration;

use judgment::client::{HeaderName, HeaderValue, REQUEST_ID_HEADER};
use judgment::{CallOptions, Client, Error, Questions, Request, RetryPolicy, SystemOne};
use proptest::prelude::*;
use serde_json::{Value, json};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

fn client(base_url: &str, retry: RetryPolicy) -> Client {
    Client::builder()
        .api_key("test-key")
        .base_url(base_url)
        .retry(retry)
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap()
}

/// Retries that do not wait.
fn fast_retries() -> RetryPolicy {
    RetryPolicy {
        max_retries: 2,
        backoff_initial: Duration::from_millis(1),
        backoff_max: Duration::from_millis(2),
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    }
}

fn noul_question() -> Questions {
    let mut q = Questions::new();
    q.noul("urgent", "Is `message` urgent?", None).unwrap();
    q
}

fn noul_body() -> Value {
    json!({
        "model": "jev-1.13.0",
        "answers": { "urgent": { "type": "noul", "noul": 0.9 } },
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    })
}

// ---------------------------------------------------------------------------
// 18.7: questions and options reach the wire in insertion order
// ---------------------------------------------------------------------------

/// Words in no sorted order, so a sorted map shows.
const POOL: [&str; 12] = [
    "zulu", "mike", "alpha", "xray", "bravo", "kilo", "echo", "yankee", "delta", "papa", "golf",
    "oscar",
];

/// Ids in a shuffled order, each with option keys in a shuffled order.
fn shuffled_questions() -> impl Strategy<Value = Vec<(String, Vec<String>)>> {
    let words = || Just(POOL.to_vec()).prop_shuffle();
    (
        words(),
        proptest::collection::vec((words(), 2..=12usize), 2..=8),
    )
        .prop_map(|(ids, options)| {
            ids.into_iter()
                .zip(options)
                .map(|(id, (keys, n))| {
                    let keys = keys[..n].iter().map(|k| format!("{id}_{k}")).collect();
                    (format!("q_{id}"), keys)
                })
                .collect()
        })
}

/// The offsets of `needles` in `body`, each found once, strictly increasing.
fn in_order(body: &str, needles: impl IntoIterator<Item = String>) -> bool {
    let mut last = None;
    needles.into_iter().all(|n| {
        let at = body.find(&n);
        let ok = at.is_some() && at > last && body.rfind(&n) == at;
        last = at;
        ok
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    /// For any order of ids and option keys, the bytes sent list the
    /// questions and each Choice's options in the order inserted, through
    /// `evaluate`, `evaluate_with` with an extra field and the `SystemOne`
    /// trait. Raw bytes: comparing parsed values is blind to order, and
    /// `serde_json`'s `preserve_order` (the `mcp` feature) would hide a
    /// sorted map, so this runs under the default features too.
    #[test]
    fn questions_and_options_reach_the_wire_in_insertion_order(drawn in shuffled_questions()) {
        let mut q = Questions::new();
        for (id, keys) in &drawn {
            q.dynamic_choice(id.as_str(), "Which?", keys.iter().map(|k| (k.clone(), None)))
                .unwrap();
        }
        let bodies = RUNTIME.block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(noul_body()))
                .mount(&server)
                .await;
            let c = client(&server.uri(), RetryPolicy::none());
            let state = json!({ "message": "hi" });
            // The answers do not fit these questions; the bytes are the point.
            let _ = c.evaluate(&Request::new(&state, "m", &q)).await;
            let extra = CallOptions::default().extra("zz_extra", 1).unwrap();
            let _ = c.evaluate_with(&Request::new(&state, "m", &q), &extra).await;
            let _ = SystemOne::answer(&c, &state, "m", &q).await;
            server.received_requests().await.unwrap()
        });
        prop_assert_eq!(bodies.len(), 3);
        for sent in bodies {
            let body = String::from_utf8(sent.body).unwrap();
            prop_assert!(in_order(&body, drawn.iter().map(|(id, _)| format!("\"{id}\":{{"))), "questions: {}", body);
            for (_, keys) in &drawn {
                prop_assert!(in_order(&body, keys.iter().map(|k| format!("\"{k}\":"))), "options: {}", body);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 18.10: reserved names cover what breaks the call
// ---------------------------------------------------------------------------

/// The names the client refuses: the seven it sets and the six HTTP owns.
const RESERVED: [&str; 13] = [
    "authorization",
    "content-type",
    "content-encoding",
    "user-agent",
    "x-typesafe-retry-count",
    "typesafe-danger-zone-images",
    "typesafe-danger-zone-long-context",
    "content-length",
    "transfer-encoding",
    "host",
    "connection",
    "te",
    "upgrade",
];

proptest! {
    /// A reserved name in any ASCII case is refused at both entry points.
    #[test]
    fn a_reserved_header_is_refused_in_any_case(
        which in 0..RESERVED.len(),
        upper in proptest::collection::vec(any::<bool>(), 40),
    ) {
        let cased: String = RESERVED[which]
            .chars()
            .zip(upper.iter().cycle())
            .map(|(c, &u)| if u { c.to_ascii_uppercase() } else { c })
            .collect();
        let name = HeaderName::from_bytes(cased.as_bytes()).unwrap();
        let value = HeaderValue::from_static("x");
        prop_assert!(matches!(
            CallOptions::default().header(name.clone(), value.clone()),
            Err(Error::ReservedHeader(_))
        ));
        prop_assert!(matches!(
            Client::builder().api_key("k").default_header(name, value).build(),
            Err(Error::ReservedHeader(_))
        ));
    }
}

/// An extra field spelled like a reserved one in another case is its own
/// field: the request's `state` and `model` go out as they are.
#[tokio::test]
async fn an_extra_field_in_another_case_does_not_touch_the_reserved_ones() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(noul_body()))
        .expect(1)
        .mount(&server)
        .await;
    let options = CallOptions::default()
        .extra("State", "caller")
        .unwrap()
        .extra("MODEL", "caller")
        .unwrap()
        .extra("Questions", "caller")
        .unwrap();
    let q = noul_question();
    client(&server.uri(), RetryPolicy::none())
        .evaluate_with(&Request::new(&"the state", "jev-1.13.0", &q), &options)
        .await
        .unwrap();
    let sent: Value =
        serde_json::from_slice(&server.received_requests().await.unwrap()[0].body).unwrap();
    assert_eq!(sent["state"], "the state");
    assert_eq!(sent["model"], "jev-1.13.0");
    assert_eq!(sent["questions"]["urgent"]["type"], "noul");
    assert_eq!(sent["State"], "caller");
}

/// CRC-32 (IEEE), for the gzip trailer.
fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |mut crc, &b| {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
        crc
    })
}

/// `data` as a gzip member of one stored deflate block (RFC 1951/1952).
fn gzip(data: &[u8]) -> Vec<u8> {
    let len = u16::try_from(data.len()).unwrap();
    let mut out = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff, 1];
    out.extend(len.to_le_bytes());
    out.extend((!len).to_le_bytes());
    out.extend(data);
    out.extend(crc32(data).to_le_bytes());
    out.extend(u32::from(len).to_le_bytes());
    out
}

#[test]
fn the_gzip_helper_matches_the_known_crc() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

/// `Accept-Encoding: gzip` as a default header: a server that honours it
/// gzips the billed answer. The call must be refused at `build()` or read;
/// today the client has no decompression and the paid answer is `Decode`.
#[test]
#[ignore = "judgment-2fj: found"]
fn an_accept_encoding_header_cannot_make_a_billed_answer_unreadable() {
    RUNTIME.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("accept-encoding", "gzip"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-encoding", "gzip")
                    .set_body_raw(gzip(noul_body().to_string().as_bytes()), "application/json"),
            )
            .mount(&server)
            .await;
        let built = Client::builder()
            .api_key("test-key")
            .base_url(server.uri())
            .retry(RetryPolicy::none())
            .default_header(
                HeaderName::from_static("accept-encoding"),
                HeaderValue::from_static("gzip"),
            )
            .build();
        let Ok(c) = built else {
            assert!(matches!(built, Err(Error::ReservedHeader(_))));
            return;
        };
        let answered = c.system_one(&"hi", &noul_question()).await;
        assert!(answered.is_ok(), "{answered:?}");
    });
}

// ---------------------------------------------------------------------------
// 18.14: each observed server departure, offline
// ---------------------------------------------------------------------------

/// What a departure must come out as.
enum Outcome {
    /// `InvalidRequest` with this status, whether it is the size refusal,
    /// and the detail it reads.
    Refused(u16, bool, &'static str),
    /// `PaymentRequired`.
    Payment,
    /// `Http` with this status.
    Http(u16),
    /// `Decode`.
    Decode,
    /// A response with this request id and, when named, this extra field
    /// and this cost.
    Answered(Option<&'static str>, Option<&'static str>, Option<f64>),
}

/// One row per departure in `docs/project/verification/`: the page, the
/// path, the status and body the server sent, and the outcome. The three
/// `OpenRouter` rows have no verification record yet — they come from the
/// documented shapes in `docs/project/research/compatible-servers-and-models.md`
/// (`mise run live:openrouter` not yet run).
#[allow(clippy::too_many_lines)]
fn departures() -> Vec<(&'static str, &'static str, u16, Value, Outcome)> {
    use Outcome::{Answered, Decode, Http, Payment, Refused};
    let answered = |extra: Value| {
        let mut body = noul_body();
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        body
    };
    vec![
        (
            "laya-typed-decisions: 256 options",
            "/v1/systemone",
            413,
            json!({ "detail": "too many choice options for 'c' (256 > 100)" }),
            Refused(413, true, "too many choice options"),
        ),
        (
            "laya-typed-decisions: over budget",
            "/v1/systemone",
            413,
            json!({ "detail": "state too large (50600 > 50000 chars)" }),
            Refused(413, true, "state too large"),
        ),
        (
            "laya-typed-decisions: malformed body, FastAPI detail",
            "/v1/systemone",
            400,
            json!({ "detail": "There was an error parsing the body" }),
            Refused(400, false, "error parsing the body"),
        ),
        (
            "laya-typed-decisions: the shim's error",
            "/v1/systemone",
            400,
            json!({ "error": "only 255 of its 256 option markers fit in max_len=1024" }),
            Refused(400, false, "option markers"),
        ),
        (
            "hosted-typesafe: over budget",
            "/v1/systemone",
            400,
            json!({ "detail": { "error_type": "max_tokens_exceeded" } }),
            Refused(400, true, "max_tokens_exceeded"),
        ),
        (
            "autojev-serve: 256 options",
            "/v1/systemone",
            422,
            json!({ "detail": [{ "type": "too_long", "loc": ["body", "questions", "c", "criteria"],
                                 "msg": "Dictionary should have at most 255 items", "input": {} }] }),
            Refused(422, false, "at most 255 items"),
        ),
        (
            "liquid: 256 options, OpenAI's envelope",
            "/v1/systemone",
            422,
            json!({ "error": { "message": "A Choice accepts at most 255 options.",
                               "type": "invalid_request_error", "param": null, "code": null } }),
            Refused(422, false, "at most 255 options"),
        ),
        (
            "ollaya: state truncated",
            "/v1/systemone",
            422,
            json!({ "error": "state: part of state was dropped to fit the context of laya:en",
                    "code": "STATE_TRUNCATED" }),
            Refused(422, true, "part of state was dropped"),
        ),
        (
            "ollaya: too many options",
            "/v1/systemone",
            422,
            json!({ "error": "130 options do not fit the option budget of laya:en",
                    "code": "TOO_MANY_OPTIONS" }),
            Refused(422, false, "option budget"),
        ),
        (
            "openrouter: out of credit",
            "/v1/systemone",
            402,
            json!({ "error": { "message": "Insufficient credits", "code": 402 } }),
            Payment,
        ),
        (
            "laya-typed-decisions: no model list",
            "/v1/models",
            404,
            json!({ "detail": "Not Found" }),
            Http(404),
        ),
        (
            "openrouter: its own catalogue as the model list",
            "/v1/models",
            200,
            json!({ "data": [{ "id": "typesafe/jev-1.13", "name": "Jev", "pricing": {} }] }),
            Decode,
        ),
        (
            "laya-typed-decisions: no request id, routing beside the answers",
            "/v1/systemone",
            200,
            answered(json!({ "routing": { "checkpoint": "typed-decisions" } })),
            Answered(None, Some("routing"), None),
        ),
        (
            "openrouter: the body's id and the cost",
            "/v1/systemone",
            200,
            answered(json!({ "id": "gen-1", "provider": "TypeSafe",
                             "usage": { "input_tokens": 1, "output_tokens": 1, "cost": 0.00003 } })),
            Answered(Some("gen-1"), Some("provider"), Some(0.00003)),
        ),
    ]
}

/// Each departure is the documented outcome, after one attempt: none of
/// them is retried, under a policy that would retry.
#[tokio::test]
async fn each_observed_server_departure_maps_to_its_outcome_offline() {
    for (what, route, status, body, outcome) in departures() {
        let server = MockServer::start().await;
        Mock::given(path(route))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .expect(1)
            .mount(&server)
            .await;
        let c = client(&server.uri(), fast_retries());
        let result = if route == "/v1/models" {
            c.list_models().await.map(|_| None)
        } else {
            c.system_one(&"hi", &noul_question()).await.map(Some)
        };
        match (outcome, result) {
            (
                Outcome::Refused(want, too_large, detail),
                Err(e @ Error::InvalidRequest { status, .. }),
            ) => {
                assert_eq!(status, want, "{what}");
                assert_eq!(e.is_request_too_large(), too_large, "{what}: {e}");
                assert!(e.to_string().contains(detail), "{what}: {e}");
            }
            (Outcome::Payment, Err(Error::PaymentRequired { .. }))
            | (Outcome::Decode, Err(Error::Decode { .. })) => {}
            (Outcome::Http(want), Err(Error::Http { status, .. })) => assert_eq!(status, want),
            (Outcome::Answered(id, extra, cost), Ok(Some(response))) => {
                assert_eq!(response.request_id.as_deref(), id, "{what}");
                if let Some(extra) = extra {
                    assert!(response.extra.contains_key(extra), "{what}");
                }
                assert_eq!(response.usage.cost, cost, "{what}");
            }
            (_, other) => panic!("{what}: {other:?}"),
        }
        server.verify().await;
    }
}

/// No code in `src/` branches on a server's name: a departure is a status
/// and a body, read the same from any server. The one name allowed is the
/// telemetry provider label, which names a host and changes no behaviour.
#[test]
fn no_code_branches_on_a_server_name() {
    const NAMES: [&str; 5] = ["laya", "ollaya", "liquid", "autojev", "openrouter"];
    const ALLOWED: [&str; 2] = ["} else if on(\"openrouter.ai\") {", "\"openrouter\""];
    let word = |line: &str, name: &str| {
        let lower = line.to_ascii_lowercase();
        lower.match_indices(name).any(|(at, _)| {
            let before = lower[..at].chars().next_back();
            let after = lower[at + name.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
    };
    let mut found = Vec::new();
    let mut dirs = vec![std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src"
    ))];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                dirs.push(p);
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            for (n, line) in text
                .lines()
                .take_while(|l| l.trim() != "#[cfg(test)]")
                .enumerate()
            {
                let code = line.trim();
                if code.starts_with("//") || ALLOWED.contains(&code) {
                    continue;
                }
                if NAMES.iter().any(|name| word(code, name)) {
                    found.push(format!("{}:{}: {code}", p.display(), n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

// ---------------------------------------------------------------------------
// 18.24: every wire-constructible failure class is reachable
// ---------------------------------------------------------------------------

/// The variants `Client` builds from a response or a transport failure
/// (`src/error.rs`, the variant table): each must come out of a scenario.
const WIRE_VARIANTS: [&str; 14] = [
    "Unauthorized",
    "PermissionDenied",
    "PaymentRequired",
    "InvalidRequest",
    "RateLimited",
    "Overloaded",
    "Http",
    "Transport",
    "ResponseTooLarge",
    "Decode",
    "MissingAnswer",
    "AnswerTypeMismatch",
    "UnknownOption",
    "InvalidAnswer",
];

/// A server on a raw socket that answers each connection with `response`
/// and closes: a body shorter than its `Content-Length` is a cut body.
fn raw_server(response: &'static [u8]) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().take(4) {
            let mut stream = stream.unwrap();
            let mut buf = vec![0u8; 65536];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(response);
        }
    });
    url
}

/// The questions every scenario asks: a Noul, a Choice and a Score.
fn three_questions() -> Questions {
    let mut q = Questions::new();
    q.noul("n", "Is it?", None).unwrap();
    q.dynamic_choice(
        "c",
        "Which?",
        [("a".to_owned(), None), ("b".to_owned(), None)],
    )
    .unwrap();
    q.score("s", "How much?", ["low", "high"]).unwrap();
    q
}

/// The answers to [`three_questions`], with `n` replaced by `noul`.
fn three_answers(noul: &Value) -> Value {
    json!({
        "model": "m",
        "answers": {
            "n": noul,
            "c": { "type": "choice", "choice": "a", "probabilities": { "a": 0.6, "b": 0.4 }, "confidence": 0.2 },
            "s": { "type": "score", "score": 0.5, "legend": { "0": "low", "1": "high" },
                   "probabilities": { "0": 0.5, "1": 0.5 }, "confidence": 0.0 }
        },
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    })
}

/// Every wire-constructible variant comes out of a scenario, after one
/// attempt under a policy that retries nothing; a response's request id is
/// kept by every variant built from a response and quoted at the end of
/// its message, and the group predicates agree with the variant.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn every_wire_failure_class_is_produced_by_a_scenario() {
    let rid = "rid-1";
    let ok = |body: Value| ResponseTemplate::new(200).set_body_json(body);
    let status = |s: u16| ResponseTemplate::new(s).set_body_json(json!({ "detail": "no" }));
    let mut cut = three_answers(&json!({ "type": "noul", "noul": 0.5 }));
    cut["answers"]["c"]["choice"] = json!("zz");
    let mut legend = three_answers(&json!({ "type": "noul", "noul": 0.5 }));
    legend["answers"]["s"]["legend"] = json!({ "0": "x", "1": "y" });
    let scenarios: Vec<(&str, ResponseTemplate)> = vec![
        ("Unauthorized", status(401)),
        ("PermissionDenied", status(403)),
        ("PaymentRequired", status(402)),
        ("InvalidRequest", status(400)),
        ("InvalidRequest", status(413)),
        ("InvalidRequest", status(415)),
        ("InvalidRequest", status(422)),
        ("RateLimited", status(429)),
        ("Overloaded", status(529)),
        ("Http", status(500)),
        ("Http", status(404)),
        (
            "Decode",
            ResponseTemplate::new(200).set_body_string("not json"),
        ),
        (
            "MissingAnswer",
            ok(json!({ "model": "m", "answers": {}, "usage": {} })),
        ),
        (
            "AnswerTypeMismatch",
            ok(three_answers(&json!({ "type": "choice", "choice": "a",
                                     "probabilities": { "a": 1.0 }, "confidence": 1.0 }))),
        ),
        ("UnknownOption", ok(cut)),
        ("InvalidAnswer", ok(legend)),
        // A probability out of range is `Decode`: `NotAProbability` is the
        // constructor's, never the wire's.
        (
            "Decode",
            ok(three_answers(&json!({ "type": "noul", "noul": 1.5 }))),
        ),
    ];
    let mut produced = BTreeSet::new();
    let q = three_questions();
    for (want, template) in scenarios {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(template.insert_header(REQUEST_ID_HEADER, rid))
            .expect(1)
            .mount(&server)
            .await;
        let err = client(&server.uri(), RetryPolicy::none())
            .system_one(&"hi", &q)
            .await
            .unwrap_err();
        let name = format!("{err:?}");
        assert!(name.starts_with(want), "{want}: {name}");
        produced.insert(want);
        let from_response = err.request_id() == Some(rid);
        assert!(from_response, "{want}: {err:?}");
        assert!(
            err.to_string().ends_with(&format!(" [request_id {rid}]")),
            "{err}"
        );
        let unfit = [
            "MissingAnswer",
            "AnswerTypeMismatch",
            "UnknownOption",
            "InvalidAnswer",
        ]
        .contains(&want);
        assert_eq!(err.is_unfit(), unfit, "{want}");
        assert_eq!(
            err.is_request_too_large(),
            matches!(err, Error::InvalidRequest { status: 413, .. }),
            "{want}"
        );
        server.verify().await;
    }

    // What comes back with no response: nobody listening, a timeout, a body
    // cut short, a body over the cap.
    let refused = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", listener.local_addr().unwrap())
    };
    let slow = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ok(noul_body()).set_delay(Duration::from_secs(5)))
        .mount(&slow)
        .await;
    let large = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ok(noul_body()))
        .mount(&large)
        .await;
    let short = raw_server(
        b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 500\r\n\r\n{\"model\":",
    );
    let small = RetryPolicy {
        max_body_bytes: 8,
        ..RetryPolicy::none()
    };
    for (want, base, policy, timeout) in [
        ("Transport", refused, RetryPolicy::none(), 2),
        ("Transport", slow.uri(), RetryPolicy::none(), 1),
        ("Transport", short, RetryPolicy::none(), 2),
        ("ResponseTooLarge", large.uri(), small, 2),
    ] {
        let c = Client::builder()
            .api_key("test-key")
            .base_url(base)
            .retry(policy)
            .timeout(Duration::from_secs(timeout))
            .build()
            .unwrap();
        let err = c.system_one(&"hi", &noul_question()).await.unwrap_err();
        let name = format!("{err:?}");
        assert!(name.starts_with(want), "{want}: {name}");
        assert_eq!(err.request_id(), None, "{want}");
        assert!(!err.is_unfit() && !err.is_request_too_large(), "{want}");
        produced.insert(want);
    }
    assert_eq!(produced, WIRE_VARIANTS.into_iter().collect::<BTreeSet<_>>());
}
