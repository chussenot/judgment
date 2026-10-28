//! The retry loop against a server scripted byte by byte (`support/raw.rs`):
//! what [`RetryPolicy::conservative`] re-sends, how long a dribbling server
//! can hold a call, what dropping a call leaves behind, and calls on one
//! shared client, `Fake`, `Recorder` and `Replay` kept apart.
//!
//! HTTP/1.1 only, and every scripted connection closes after its script:
//! a keep-alive connection reused for a retry is avoided here, not tested.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use judgment::client::{HeaderName, HeaderValue};
use judgment::http::{Completed, Exhausted};
use judgment::{
    CallOptions, Client, Error, Fake, Observer, Questions, Recorder, Replay, Request, Response,
    RetryPolicy, SystemOne, TransportRetry,
};
use serde_json::json;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[path = "support/raw.rs"]
mod raw;

use raw::{RawServer, Step};

fn one_noul() -> Questions {
    let mut q = Questions::new();
    q.noul("x", "?", None).unwrap();
    q
}

/// A whole response with `status` and `extra` headers. `connection: close`
/// so the client never reuses a connection the script is about to close.
fn response(status: &str, extra: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\nconnection: close\r\n\
         {extra}content-length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn ok() -> Vec<u8> {
    let body = json!({
        "model": "jev-1.13.0",
        "answers": { "x": { "type": "noul", "noul": 0.5 } },
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    });
    response("200 OK", "", &body.to_string())
}

/// A response head that promises 100 bytes and the first of them.
fn cut(status: &str) -> Vec<u8> {
    format!("HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: 100\r\n\r\n{{")
        .into_bytes()
}

fn client_at(url: &str, retry: RetryPolicy, timeout: Duration) -> Client {
    Client::builder()
        .api_key("test-key")
        .base_url(url)
        .retry(retry)
        .timeout(timeout)
        .build()
        .unwrap()
}

/// Every failed attempt a client's own observer is told about.
#[derive(Default)]
struct Failures(Mutex<Vec<String>>);

impl Observer for Failures {
    fn on_failed_attempt(&self, _service: &'static str, status: &str) {
        self.0.lock().unwrap().push(status.to_owned());
    }
}

impl Failures {
    fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

/// Under `conservative()` (and the same statuses with no transport retry), a
/// request the server read whole is sent again only after a complete 408 or
/// 429: a reset, an end of stream, a cut body, a 5xx, a 529 or a timeout
/// after the read may have been processed and billed. A connection never
/// made is the other retry (`client.rs`, `conservative_retries_a_refused_connection`);
/// the raw server cannot cut a request half read.
#[tokio::test]
async fn conservative_never_sends_again_what_the_server_read_and_may_have_billed() {
    let ok_after = |status: &str| vec![Step::Write(response(status, "", "{}"))];
    let cases: Vec<(&str, Vec<Step>, usize)> = vec![
        ("reset after the read", vec![Step::Reset], 1),
        ("end of stream after the read", vec![], 1),
        ("200 cut short", vec![Step::Write(cut("200 OK"))], 1),
        (
            "503 cut short",
            vec![Step::Write(cut("503 Service Unavailable"))],
            1,
        ),
        (
            "200 head then a reset",
            vec![
                Step::Write(cut("200 OK")),
                Step::Stall(Duration::from_millis(50)),
                Step::Reset,
            ],
            1,
        ),
        ("500", ok_after("500 Internal Server Error"), 1),
        ("503", ok_after("503 Service Unavailable"), 1),
        ("529", ok_after("529 Overloaded"), 1),
        (
            "no reply until the timeout",
            vec![Step::Stall(Duration::MAX)],
            1,
        ),
        (
            "200 head then silence",
            vec![Step::Write(cut("200 OK")), Step::Stall(Duration::MAX)],
            1,
        ),
        ("408", ok_after("408 Request Timeout"), 2),
        ("429", ok_after("429 Too Many Requests"), 2),
    ];
    for transport in [TransportRetry::BeforeSend, TransportRetry::Never] {
        for (name, first, expected) in &cases {
            let server = RawServer::start(vec![first.clone(), vec![Step::Write(ok())]]);
            let policy = RetryPolicy {
                transport,
                backoff_initial: Duration::from_millis(5),
                backoff_jitter: 0.0,
                ..RetryPolicy::conservative()
            };
            let result = client_at(&server.url, policy, Duration::from_millis(300))
                .system_one(&"s", &one_noul())
                .await;
            let read = server.stop().iter().filter(|c| c.complete).count();
            assert_eq!(read, *expected, "{transport:?}, {name}: {result:?}");
            assert_eq!(
                result.is_ok(),
                *expected == 2,
                "{transport:?}, {name}: {result:?}"
            );
        }
    }
}

/// `count` steps that write one byte of `bytes` (cycled) every 50 ms.
fn drip(bytes: &[u8], count: usize) -> Vec<Step> {
    bytes
        .iter()
        .cycle()
        .take(count)
        .flat_map(|b| {
            [
                Step::Write(vec![*b]),
                Step::Stall(Duration::from_millis(50)),
            ]
        })
        .collect()
}

/// A server that trickles a body, trickles the head, or never answers holds
/// a call no longer than the budget plus one per-attempt timeout (reqwest's
/// timeout covers the body read), the call ends on a timeout, and nothing
/// reaches the server after it returned. Real clock: a paused one would
/// fire the timeout at once while the runtime waits on the socket.
#[tokio::test]
async fn a_dribbling_server_holds_a_call_no_longer_than_its_budget_and_one_timeout() {
    let head = b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 1000\r\n\r\n";
    let mut body_drip = vec![Step::Write(head.to_vec())];
    body_drip.extend(drip(b" ", 400));
    let cases = [
        ("body dribbled", body_drip),
        ("head dribbled", drip(head, 400)),
        ("nothing written", vec![Step::Stall(Duration::MAX)]),
    ];
    let (budget, timeout) = (Duration::from_secs(1), Duration::from_millis(300));
    for (name, script) in cases {
        let server = RawServer::start(vec![script]);
        let policy = RetryPolicy {
            max_retries: 50,
            backoff_initial: Duration::from_millis(10),
            backoff_jitter: 0.0,
            budget: Some(budget),
            ..RetryPolicy::default()
        };
        let started = Instant::now();
        let err = client_at(&server.url, policy, timeout)
            .system_one(&"s", &one_noul())
            .await
            .unwrap_err();
        let elapsed = started.elapsed();
        // The budget, not the retry count, ended it; one attempt in flight
        // may run past it, plus slack for a loaded machine.
        assert!(elapsed >= budget, "{name}: {elapsed:?}");
        assert!(
            elapsed < budget + timeout + Duration::from_secs(1),
            "{name}: {elapsed:?}"
        );
        assert!(
            matches!(&err, Error::Transport { source, .. } if source.is_timeout()),
            "{name}: {err:?}"
        );
        let after = server.connections().len();
        assert!(after >= 2, "{name}: retried, {after} connections");
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(server.stop().len(), after, "{name}: a request after return");
    }
}

/// A call dropped by `tokio::time::timeout` after `deadline`, against a
/// server whose first connection runs `first` and every later one answers.
/// Nothing reaches the server in `settle` after the drop, the observer has
/// heard of `failed` attempts per connection made, and the same client then
/// answers.
async fn dropped(name: &str, first: Vec<Step>, policy: RetryPolicy, failed: usize) {
    let (deadline, settle) = (Duration::from_millis(250), Duration::from_millis(1200));
    let server = RawServer::start(vec![first, vec![Step::Write(ok())]]);
    let failures = Arc::new(Failures::default());
    let client = Client::builder()
        .api_key("test-key")
        .base_url(&server.url)
        .retry(policy)
        .timeout(Duration::from_secs(5))
        .observer(failures.clone())
        .build()
        .unwrap();
    let q = one_noul();
    let started = Instant::now();
    let call = tokio::time::timeout(deadline, client.system_one(&"s", &q)).await;
    assert!(call.is_err(), "{name}: the call ended on its own: {call:?}");
    assert!(
        started.elapsed() < deadline + Duration::from_millis(500),
        "{name}: {:?}",
        started.elapsed()
    );
    let made = server.connections().len();
    tokio::time::sleep(settle).await;
    assert_eq!(
        server.connections().len(),
        made,
        "{name}: sent after the drop"
    );
    assert!(made <= 1, "{name}: {made} connections before the drop");
    assert_eq!(failures.count(), made * failed, "{name}");
    client.system_one(&"s", &q).await.unwrap();
    assert_eq!(server.stop().len(), made + 1, "{name}");
}

/// Dropping the call future while it waits for a response, reads a body,
/// sleeps a backoff or waits out a `Retry-After` stops the call there: no
/// zombie retry, no report for an attempt that did not finish, and the
/// client stays usable.
#[tokio::test]
async fn a_call_dropped_at_any_wait_sends_nothing_more_and_leaves_the_client_usable() {
    let quick = RetryPolicy {
        backoff_initial: Duration::from_millis(400),
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    };
    tokio::join!(
        dropped(
            "awaiting the response",
            vec![Step::Stall(Duration::MAX)],
            quick.clone(),
            0
        ),
        dropped(
            "reading the body",
            vec![Step::Write(cut("200 OK")), Step::Stall(Duration::MAX)],
            quick.clone(),
            0
        ),
        dropped(
            "sleeping the backoff",
            vec![Step::Write(response("503 Service Unavailable", "", "{}"))],
            quick.clone(),
            1
        ),
        dropped(
            "waiting out retry-after",
            vec![Step::Write(response(
                "429 Too Many Requests",
                "retry-after-ms: 400\r\n",
                "{}"
            ))],
            RetryPolicy {
                backoff_initial: Duration::from_millis(5),
                ..quick
            },
            1
        ),
    );
}

/// A recorded call dropped before its answer came writes no recording.
#[tokio::test]
async fn a_dropped_recorded_call_writes_nothing() {
    let server = RawServer::start(vec![vec![Step::Stall(Duration::MAX)]]);
    let dir = std::env::temp_dir().join(format!("retry-model-drop-{}", std::process::id()));
    let recorder = Recorder::new(
        client_at(&server.url, RetryPolicy::none(), Duration::from_secs(5)),
        &dir,
    );
    let q = one_noul();
    let call = tokio::time::timeout(
        Duration::from_millis(200),
        recorder.answer(&json!("s"), "jev-latest", &q),
    )
    .await;
    assert!(call.is_err(), "{call:?}");
    let written = std::fs::read_dir(&dir).map_or(0, Iterator::count);
    assert_eq!(written, 0, "{}", dir.display());
    drop(server.stop());
}

// The shared types cross threads; a regression is a compile error here.
const _: () = {
    const fn send_sync<T: Send + Sync>() {}
    send_sync::<Client>();
    send_sync::<Fake>();
    send_sync::<Replay>();
    send_sync::<Recorder<Fake>>();
    send_sync::<Response>();
    send_sync::<Error>();
    send_sync::<Completed>();
    send_sync::<Exhausted>();
};

const TASKS: usize = 32;

/// One client shared by 32 tasks on a multi-thread runtime: each call gets
/// the answer to its own questions and its own header back, and each call's
/// retry is its own (every even task's first request gets a 503, so the
/// server sees 48 requests and the observer 16 failures).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_client_shared_by_many_tasks_keeps_their_calls_apart() {
    let server = MockServer::start().await;
    let failed_once = Arc::new(Mutex::new(HashSet::new()));
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(move |req: &wiremock::Request| {
            let task: usize = req.headers["x-task"].to_str().unwrap().parse().unwrap();
            if task.is_multiple_of(2) && failed_once.lock().unwrap().insert(task) {
                return ResponseTemplate::new(503);
            }
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            let answers: serde_json::Map<_, _> = body["questions"]
                .as_object()
                .unwrap()
                .keys()
                .map(|id| (id.clone(), json!({ "type": "noul", "noul": 0.5 })))
                .collect();
            ResponseTemplate::new(200).set_body_json(json!({
                "model": format!("task-{task}-state-{}", body["state"]["task"]),
                "answers": answers,
                "usage": { "input_tokens": 1, "output_tokens": 1 }
            }))
        })
        .expect(u64::try_from(TASKS + TASKS / 2).unwrap())
        .mount(&server)
        .await;
    let failures = Arc::new(Failures::default());
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(RetryPolicy {
            backoff_initial: Duration::from_millis(5),
            ..RetryPolicy::default()
        })
        .observer(failures.clone())
        .build()
        .unwrap();
    let tasks: Vec<_> = (0..TASKS)
        .map(|task| {
            let client = client.clone();
            // `tokio::spawn` is the check that the call's future is `Send`.
            tokio::spawn(async move {
                let mut q = Questions::new();
                q.noul(format!("q{task}"), "?", None).unwrap();
                let state = json!({ "task": task });
                let options = CallOptions::new()
                    .header(
                        HeaderName::from_static("x-task"),
                        HeaderValue::from_str(&task.to_string()).unwrap(),
                    )
                    .unwrap();
                let response = client
                    .evaluate_with(&Request::new(&state, "jev-latest", &q), &options)
                    .await
                    .unwrap();
                assert_eq!(response.model, format!("task-{task}-state-{task}"));
                assert_eq!(
                    response.answers.keys().collect::<Vec<_>>(),
                    [&format!("q{task}")]
                );
            })
        })
        .collect();
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(failures.count(), TASKS / 2);
    server.verify().await;
}

/// A `Fake`, a `Recorder<Fake>` and the `Replay` over what it wrote, each
/// shared by 32 tasks: every call is counted once and answered for its own
/// state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_fake_recorder_and_replay_shared_by_many_tasks_keep_their_calls_apart() {
    let dir = std::env::temp_dir().join(format!("retry-model-shared-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let recorder = Arc::new(Recorder::new(Fake::new().noul("x", 0.5).unwrap(), &dir));
    let q = Arc::new(one_noul());
    let ask = |backend: Arc<dyn SystemOne>| {
        let q = Arc::clone(&q);
        (0..TASKS)
            .map(move |task| {
                let (backend, q) = (Arc::clone(&backend), Arc::clone(&q));
                tokio::spawn(async move {
                    backend
                        .answer(&json!({ "task": task }), "jev-latest", &q)
                        .await
                        .unwrap()
                })
            })
            .collect::<Vec<_>>()
    };
    for task in ask(recorder.clone()) {
        task.await.unwrap();
    }
    let mut asked: Vec<_> = recorder
        .inner()
        .calls()
        .iter()
        .map(|c| c.state["task"].as_u64().unwrap())
        .collect();
    asked.sort_unstable();
    assert_eq!(asked, (0..TASKS as u64).collect::<Vec<_>>());
    let replay = Replay::open(&dir).unwrap();
    assert_eq!(replay.len(), TASKS);
    for task in ask(Arc::new(replay)) {
        task.await.unwrap();
    }
    let _ = std::fs::remove_dir_all(&dir);
}
