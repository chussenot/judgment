//! The process-wide observer: installed first-wins under racing threads,
//! deaf to what was reported before it, and the one place usage goes for a
//! client without its own, never both.
//!
//! A target of its own, with one test: `observer::set_global` succeeds once
//! per process, and any other test in the binary would race it.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use judgment::{
    Client, Fake, Observer, Questions, Recorder, Replay, RetryPolicy, SystemOne, Usage, observer,
};
use serde_json::json;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Every report one observer receives, in order.
#[derive(Default)]
struct Seen(Mutex<Vec<String>>);

impl Observer for Seen {
    fn on_usage(&self, _model: &str, usage: &Usage) {
        self.push(format!("usage {}", usage.input_tokens));
    }
    fn on_failed_attempt(&self, _service: &'static str, status: &str) {
        self.push(format!("failed {status}"));
    }
}

impl Seen {
    fn push(&self, report: String) {
        self.0.lock().unwrap().push(report);
    }
    /// What arrived since the last call.
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

fn answered(id: &str) -> serde_json::Value {
    json!({
        "model": "jev-1.13.0",
        "answers": { id: { "type": "noul", "noul": 0.5 } },
        "usage": { "input_tokens": 7, "output_tokens": 1 }
    })
}

/// `server` answers `first` once, then `then` for good.
async fn script(server: &MockServer, first: ResponseTemplate, then: ResponseTemplate) {
    server.reset().await;
    Mock::given(method("POST"))
        .respond_with(first)
        .up_to_n_times(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .respond_with(then)
        .mount(server)
        .await;
}

/// `n` installers released together by a barrier: exactly one wins, and the
/// global is that one. Every candidate, and the winner.
fn race_to_install(n: usize) -> (Vec<Arc<Seen>>, Arc<Seen>) {
    let candidates: Vec<Arc<Seen>> = (0..n).map(|_| Arc::default()).collect();
    let barrier = Arc::new(Barrier::new(candidates.len()));
    let won: Vec<bool> = candidates
        .iter()
        .map(|seen| {
            let (seen, barrier) = (Arc::clone(seen), Arc::clone(&barrier));
            std::thread::spawn(move || {
                barrier.wait();
                observer::set_global(seen)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect();
    assert_eq!(won.iter().filter(|w| **w).count(), 1, "{won:?}");
    let winner = Arc::clone(&candidates[won.iter().position(|w| *w).unwrap()]);
    assert!(std::ptr::addr_eq(
        Arc::as_ptr(&observer::global()),
        Arc::as_ptr(&winner)
    ));
    (candidates, winner)
}

#[tokio::test]
async fn the_global_observer_is_first_wins_and_usage_reaches_exactly_one_observer() {
    let server = MockServer::start().await;
    let retrying = RetryPolicy {
        backoff_initial: Duration::from_millis(5),
        ..RetryPolicy::default()
    };
    let own = Arc::new(Seen::default());
    let with_own = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(retrying.clone())
        .observer(own.clone())
        .build()
        .unwrap();
    let without = Client::builder()
        .api_key("test-key")
        .base_url(server.uri())
        .retry(retrying)
        .build()
        .unwrap();
    let mut q = Questions::new();
    q.noul("x", "?", None).unwrap();

    // Reported before any install: to the no-op, and lost by design.
    script(
        &server,
        ResponseTemplate::new(503),
        ResponseTemplate::new(200).set_body_json(answered("x")),
    )
    .await;
    without.system_one(&"s", &q).await.unwrap();

    let (candidates, winner) = race_to_install(16);
    assert!(!observer::set_global(Arc::new(Seen::default())), "too late");
    assert!(
        winner.take().is_empty(),
        "nothing before the install replayed"
    );

    // A good answer after a failed attempt: usage once, to the client's own
    // observer when it has one, else to the global; never both.
    for (client, to, other) in [(&with_own, &own, &winner), (&without, &winner, &own)] {
        script(
            &server,
            ResponseTemplate::new(503).set_body_json(answered("x")),
            ResponseTemplate::new(200).set_body_json(answered("x")),
        )
        .await;
        client.system_one(&"s", &q).await.unwrap();
        assert_eq!(to.take(), ["failed 503", "usage 7"]);
        assert!(other.take().is_empty());
    }

    // Billed but unfit: usage once and one `unfit`, never retried. A body that
    // does not decode: one `decode` and no usage.
    for (body, expected) in [
        (
            ResponseTemplate::new(200).set_body_json(answered("y")),
            vec!["usage 7", "failed unfit"],
        ),
        (
            ResponseTemplate::new(200).set_body_string("<html>"),
            vec!["failed decode"],
        ),
    ] {
        script(&server, body.clone(), body).await;
        with_own.system_one(&"s", &q).await.unwrap_err();
        assert_eq!(own.take(), expected);
        assert!(winner.take().is_empty());
    }

    // A Fake, a Recorder over it and a Replay of what it wrote report nothing.
    let dir = std::env::temp_dir().join(format!("observer-global-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let recorder = Recorder::new(Fake::new().noul("x", 0.5).unwrap(), &dir);
    recorder
        .answer(&json!("s"), "jev-latest", &q)
        .await
        .unwrap();
    recorder
        .inner()
        .answer(&json!("s"), "jev-latest", &q)
        .await
        .unwrap();
    Replay::open(&dir)
        .unwrap()
        .answer(&json!("s"), "jev-latest", &q)
        .await
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(winner.take().is_empty());

    // No loser ever heard anything.
    for seen in &candidates {
        assert!(Arc::ptr_eq(seen, &winner) || seen.take().is_empty());
    }
}
