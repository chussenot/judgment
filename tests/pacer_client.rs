//! A client that shares a `Pacer`: a 429 or 529 that names a wait pauses
//! every call on the pacer, a status that is not one does not, and a
//! client's failed attempts reach its own observer.
//!
//! Real time over a wiremock server, so every assertion is a lower bound
//! on a wait (a sleep is never shorter than asked) except the controls,
//! which are well inside the wait they are compared with. No test here sets
//! the process-wide observer.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use judgment::{Client, Observer, Paced, Pacer, Questions, RetryPolicy, SystemOne};
use serde_json::{Value, json};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// The server's named wait, and how long after the first request the second
/// call starts: well inside the wait.
const WAIT: Duration = Duration::from_millis(600);
const SECOND_CALL_AT: Duration = Duration::from_millis(50);

fn one_noul() -> Questions {
    let mut q = Questions::new();
    q.noul("x", "?", None).unwrap();
    q
}

fn ok() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "model": "jev-1.13.0",
        "answers": { "x": { "type": "noul", "noul": 0.5 } },
        "usage": { "input_tokens": 3, "output_tokens": 1 }
    }))
}

fn quick_retry() -> RetryPolicy {
    RetryPolicy {
        max_retries: 1,
        backoff_initial: Duration::from_millis(1),
        backoff_max: Duration::from_millis(1),
        backoff_jitter: 0.0,
        ..RetryPolicy::default()
    }
}

/// A server whose first request is answered with `first` and every later
/// one with a 200, noting when each arrived.
async fn serve(first: ResponseTemplate) -> (MockServer, Arc<Mutex<Vec<Instant>>>) {
    let server = MockServer::start().await;
    let arrivals = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&arrivals);
    Mock::given(method("POST"))
        .respond_with(move |_: &Request| {
            let mut seen = seen.lock().unwrap();
            seen.push(Instant::now());
            if seen.len() == 1 { first.clone() } else { ok() }
        })
        .mount(&server)
        .await;
    (server, arrivals)
}

fn client(server: &MockServer, pacer: Option<&Pacer>) -> Client {
    let mut builder = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(quick_retry())
        .timeout(Duration::from_secs(5));
    if let Some(pacer) = pacer {
        builder = builder.pacer(pacer.clone());
    }
    builder.build().unwrap()
}

/// Two calls, the second starting `SECOND_CALL_AT` after the first, through
/// `backend`; the time after the first request at which the earliest later
/// request arrived.
async fn earliest_follow_up(backend: &dyn SystemOne, arrivals: &Mutex<Vec<Instant>>) -> Duration {
    let state = Value::Null;
    let questions = one_noul();
    let first = backend.answer(&state, "jev-latest", &questions);
    let second = async {
        tokio::time::sleep(SECOND_CALL_AT).await;
        backend.answer(&state, "jev-latest", &questions).await
    };
    let (a, b) = tokio::join!(first, second);
    a.unwrap();
    b.unwrap();
    let arrivals = arrivals.lock().unwrap();
    assert!(arrivals.len() >= 3, "expected a retry and a second call");
    arrivals[1..]
        .iter()
        .map(|t| t.duration_since(arrivals[0]))
        .min()
        .unwrap()
}

#[tokio::test]
async fn a_429s_wait_holds_every_call_on_the_pacer() {
    let pacer = Pacer::new(2);
    let (server, arrivals) = serve(
        ResponseTemplate::new(429).insert_header("retry-after-ms", WAIT.as_millis().to_string()),
    )
    .await;
    let bounded = Paced::new(client(&server, Some(&pacer)), pacer);
    let gap = earliest_follow_up(&bounded, &arrivals).await;
    assert!(
        gap >= WAIT,
        "a call reached the server {gap:?} after the 429, inside its {WAIT:?} wait"
    );
}

/// No `Paced` here, so only the retry loop's own wait before each attempt
/// holds the second call.
#[tokio::test]
async fn a_client_sharing_a_pacer_holds_its_own_calls_without_paced() {
    let pacer = Pacer::new(2);
    let (server, arrivals) = serve(
        ResponseTemplate::new(429).insert_header("retry-after-ms", WAIT.as_millis().to_string()),
    )
    .await;
    let bare = client(&server, Some(&pacer));
    let gap = earliest_follow_up(&bare, &arrivals).await;
    assert!(
        gap >= WAIT,
        "a call reached the server {gap:?} after the 429, inside its {WAIT:?} wait"
    );
}

/// The control: the same scenario with no pacer, where the second call goes
/// straight out, so the assertion above can fail.
#[tokio::test]
async fn without_a_pacer_the_other_call_is_not_held() {
    let (server, arrivals) = serve(
        ResponseTemplate::new(429).insert_header("retry-after-ms", WAIT.as_millis().to_string()),
    )
    .await;
    let alone = client(&server, None);
    // The retry itself waits the 600 ms; the other call does not.
    let state = Value::Null;
    let questions = one_noul();
    let first = alone.answer(&state, "jev-latest", &questions);
    let second = async {
        tokio::time::sleep(SECOND_CALL_AT).await;
        alone.answer(&state, "jev-latest", &questions).await
    };
    let (a, b) = tokio::join!(first, second);
    a.unwrap();
    b.unwrap();
    let arrivals = arrivals.lock().unwrap();
    let second_call = arrivals[1].duration_since(arrivals[0]);
    assert!(
        second_call < WAIT,
        "the second call reached the server {second_call:?} after the first"
    );
}

#[tokio::test]
async fn a_status_that_is_not_429_or_529_pauses_nobody() {
    let pacer = Pacer::new(2);
    let (server, arrivals) = serve(
        ResponseTemplate::new(500).insert_header("retry-after-ms", WAIT.as_millis().to_string()),
    )
    .await;
    let bounded = Paced::new(client(&server, Some(&pacer)), pacer);
    // The 500 retries after its own wait, the second call is not held.
    let state = Value::Null;
    let questions = one_noul();
    let first = bounded.answer(&state, "jev-latest", &questions);
    let second = async {
        tokio::time::sleep(SECOND_CALL_AT).await;
        bounded.answer(&state, "jev-latest", &questions).await
    };
    let (a, b) = tokio::join!(first, second);
    a.unwrap();
    b.unwrap();
    let arrivals = arrivals.lock().unwrap();
    let second_call = arrivals[1].duration_since(arrivals[0]);
    assert!(
        second_call < WAIT,
        "a 500 held the other call for {second_call:?}"
    );
}

#[tokio::test]
async fn a_429_without_a_named_wait_pauses_nobody() {
    let pacer = Pacer::new(2);
    let (server, arrivals) = serve(ResponseTemplate::new(429)).await;
    let bounded = Paced::new(client(&server, Some(&pacer)), pacer);
    let state = Value::Null;
    let questions = one_noul();
    let first = bounded.answer(&state, "jev-latest", &questions);
    let second = async {
        tokio::time::sleep(SECOND_CALL_AT).await;
        bounded.answer(&state, "jev-latest", &questions).await
    };
    let (a, b) = tokio::join!(first, second);
    a.unwrap();
    b.unwrap();
    let arrivals = arrivals.lock().unwrap();
    // Backoff is a millisecond: both calls are done long before WAIT.
    assert!(arrivals[arrivals.len() - 1].duration_since(arrivals[0]) < WAIT);
}

/// Every failed attempt the observer is told about.
#[derive(Default)]
struct Failures(Mutex<Vec<(&'static str, String)>>);

impl Observer for Failures {
    fn on_failed_attempt(&self, service: &'static str, status: &str) {
        self.0.lock().unwrap().push((service, status.to_owned()));
    }
}

#[tokio::test]
async fn a_clients_failed_attempts_go_to_its_own_observer() {
    let own = Arc::new(Failures::default());
    let (server, _) = serve(ResponseTemplate::new(429)).await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(quick_retry())
        .timeout(Duration::from_secs(5))
        .observer(Arc::clone(&own) as Arc<dyn Observer>)
        .build()
        .unwrap();
    client.system_one(&Value::Null, &one_noul()).await.unwrap();
    // The 429 that was retried, reported though the call succeeded.
    assert_eq!(*own.0.lock().unwrap(), [("typesafe", "429".to_owned())]);
}

#[tokio::test]
async fn an_unfit_200_reaches_the_clients_own_observer_too() {
    let own = Arc::new(Failures::default());
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {},
            "usage": { "input_tokens": 1, "output_tokens": 0 }
        })))
        .mount(&server)
        .await;
    let client = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(RetryPolicy::none())
        .timeout(Duration::from_secs(5))
        .observer(Arc::clone(&own) as Arc<dyn Observer>)
        .build()
        .unwrap();
    client
        .system_one(&Value::Null, &one_noul())
        .await
        .unwrap_err();
    assert_eq!(*own.0.lock().unwrap(), [("typesafe", "unfit".to_owned())]);
}
