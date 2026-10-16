//! `Paced` over a backend that needs no network: the bound on calls in
//! flight, the pause every call waits out, and the place a dropped call
//! gives back. All on tokio's paused clock, so "100 ms" costs nothing and an
//! elapsed time is exact.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use judgment::backend::BoxFuture;
use judgment::{Fake, Paced, Pacer, Questions, Response, SystemOne};
use serde_json::{Value, json};
use tokio::task::JoinSet;
use tokio::time::{Instant, timeout};

fn one_noul() -> Questions {
    let mut q = Questions::new();
    q.noul("x", "?", None).unwrap();
    q
}

fn fake() -> Fake {
    Fake::new().noul("x", 0.5).unwrap()
}

/// A backend that holds each call for `delay` and counts how many are
/// inside it at once.
struct Gauge {
    inner: Fake,
    delay: Duration,
    now: AtomicUsize,
    peak: AtomicUsize,
}

impl SystemOne for Gauge {
    fn answer<'a>(
        &'a self,
        state: &'a Value,
        model: &'a str,
        questions: &'a Questions,
    ) -> BoxFuture<'a, judgment::Result<Response>> {
        Box::pin(async move {
            let inside = self.now.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(inside, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            self.now.fetch_sub(1, Ordering::SeqCst);
            self.inner.answer(state, model, questions).await
        })
    }
}

/// A backend whose call never finishes.
struct Hang;

impl SystemOne for Hang {
    fn answer<'a>(
        &'a self,
        _: &'a Value,
        _: &'a str,
        _: &'a Questions,
    ) -> BoxFuture<'a, judgment::Result<Response>> {
        Box::pin(std::future::pending())
    }
}

#[tokio::test(start_paused = true)]
async fn at_most_the_pacers_number_of_calls_are_in_flight() {
    let gauge = Gauge {
        inner: fake(),
        delay: Duration::from_millis(100),
        now: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
    };
    let bounded = Arc::new(Paced::new(gauge, Pacer::new(2)));
    let started = Instant::now();
    let mut calls = JoinSet::new();
    for _ in 0..10 {
        let bounded = Arc::clone(&bounded);
        calls.spawn(async move {
            bounded
                .answer(&json!({}), "jev-latest", &one_noul())
                .await
                .map(|_| ())
        });
    }
    while let Some(done) = calls.join_next().await {
        done.unwrap().unwrap();
    }
    // Two at a time, not one at a time and not all ten: ten calls of 100 ms
    // in pairs take five rounds.
    assert_eq!(bounded.inner().peak.load(Ordering::SeqCst), 2);
    assert_eq!(started.elapsed(), Duration::from_millis(500));
}

#[tokio::test(start_paused = true)]
async fn a_call_waits_out_a_pause_before_it_starts() {
    let pacer = Pacer::new(4);
    let bounded = Paced::new(fake(), pacer.clone());
    pacer.pause_for(Duration::from_secs(1));
    let started = Instant::now();
    bounded
        .answer(&json!({}), "jev-latest", &one_noul())
        .await
        .unwrap();
    assert_eq!(started.elapsed(), Duration::from_secs(1));

    // The pause is over: the next call does not wait.
    let started = Instant::now();
    bounded
        .answer(&json!({}), "jev-latest", &one_noul())
        .await
        .unwrap();
    assert_eq!(started.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn a_dropped_call_gives_its_place_back() {
    let pacer = Pacer::new(1);
    let hanging = Paced::new(Hang, pacer.clone());
    let fine = Paced::new(fake(), pacer);
    let state = json!({});
    let questions = one_noul();

    // While the first call holds the only place, the second cannot start.
    let mut first = Box::pin(hanging.answer(&state, "jev-latest", &questions));
    tokio::select! {
        _ = &mut first => unreachable!("a hanging call finished"),
        () = tokio::time::sleep(Duration::from_millis(1)) => {}
    }
    timeout(
        Duration::from_secs(1),
        fine.answer(&state, "jev-latest", &questions),
    )
    .await
    .expect_err("the second call ran while the place was held");

    // Dropping the first (a cancelled call) frees it.
    drop(first);
    timeout(
        Duration::from_secs(1),
        fine.answer(&state, "jev-latest", &questions),
    )
    .await
    .expect("the place was not given back")
    .unwrap();
}
