---
title: Bound a stream of calls
description: How to run many calls at once without outrunning the server, with a Pacer shared by a Paced backend, which holds at most N calls in flight, and by the client, whose 429 or 529 that names a wait pauses every call on it; what pauses and what does not, what a dropped call gives back, how to test it with a Fake on a paused clock, and what a Pacer is not (a rate limiter).
status: current
last_reviewed: 2026-10-16
tags: [judgment, concurrency, rate-limit, retries, client, how-to, rust]
---

# Bound a stream of calls

**You will** run a batch of calls at once, keep at most N of them in flight, and make every one of them wait when the server says to. **Prerequisites:** [Your first decision in Rust](../start/first-decision-rust.md).

## The two things a retry cannot do

The retry loop retries one call. It cannot say "at most eight at a time", and when the server answers 429 with a wait, only the call that got the answer waits: the others, in flight or queued, go on sending and are refused in their turn, each spending an attempt. A replay of history in bulk reaches the account's limit this way first.

A `Pacer` is the two missing parts in one value that you clone and share: a number of places for calls in flight, and a deadline before which no call sends. The `pacer` feature has it, and `http` implies the feature.

## Bound the calls and share the pause

```rust no_run
use std::sync::Arc;

use judgment::{Client, Paced, Pacer, Questions, SystemOne};
use serde_json::Value;

async fn run(states: Vec<Value>) -> judgment::Result<()> {
    let mut questions = Questions::new();
    let urgent = questions.noul("urgent", "Does `message` convey urgency?", None)?;
    let questions = Arc::new(questions);

    // Eight calls at a time; every 429 that names a wait pauses all of them.
    let pacer = Pacer::new(8);
    let client = Client::builder().pacer(pacer.clone()).build()?;
    let backend = Arc::new(Paced::new(client, pacer));

    let mut calls = tokio::task::JoinSet::new();
    for state in states {
        let (backend, questions) = (Arc::clone(&backend), Arc::clone(&questions));
        calls.spawn(async move { backend.answer(&state, "jev-latest", &questions).await });
    }
    while let Some(done) = calls.join_next().await {
        let response = done.expect("a call panicked")?;
        println!("urgent: {}", response.get(&urgent)?.is_yes(0.6));
    }
    Ok(())
}
```

`Paced` makes each call take a place first, wait out any pause, and only then reach the client. Give the same pacer to the client (`ClientBuilder::pacer`) so its retries feed the pause. The two are separate on purpose: the bound is for any backend, a `Fake` in a test included, and the pause is the client's to set.

## What pauses, and what does not

A retryable 429 or 529 that names a wait (`Retry-After` or `retry-after-ms`) pauses the pacer for that wait, the one the retry policy already capped (30 s by default, [`RetryPolicy::retry_after_max`](../reference/crate.md)). Every call on the pacer then holds its next attempt, the first included, until the pause ends. A pause only ever extends: a shorter wait reported later does not shorten the one running, and a pause is never longer than a day.

Nothing else pauses anyone. A 429 with no named wait backs off that call alone, and so does any other status, a 503 `server busy` included, because only a rate limit says the whole stream should slow down. A response that did not fit the questions was billed and is never retried, here or without a pacer.

## What a place is

A call holds its place from the moment it is admitted until its future ends, retries and waits included. Dropping the future, by a timeout or by aborting the task, gives the place back at once, so a cancelled call cannot leak one. A pacer built for zero places has one: a pacer that admitted nothing would hang every call.

A `Pacer` bounds how many calls are in flight. It is not a rate limiter: eight places and fast answers still send as many requests per second as eight places can. If the server's limit is a rate, size the pacer to what the rate allows, or put a rate limiter in front.

## Test it without a server

`Paced` wraps any `SystemOne`, so a `Fake` shows the bound with no network, and tokio's paused clock makes the waits instant and exact:

```rust no_run
use std::time::Duration;

use judgment::{Fake, Paced, Pacer, Questions, SystemOne};
use serde_json::json;

#[tokio::test(start_paused = true)]
async fn a_call_waits_out_a_pause() -> judgment::Result<()> {
    let mut questions = Questions::new();
    questions.noul("x", "?", None)?;
    let pacer = Pacer::new(4);
    let backend = Paced::new(Fake::new().noul("x", 0.5)?, pacer.clone());

    pacer.pause_for(Duration::from_secs(1));
    let started = tokio::time::Instant::now();
    backend.answer(&json!({}), "jev-latest", &questions).await?;
    assert_eq!(started.elapsed(), Duration::from_secs(1));
    Ok(())
}
```

`tokio::time::pause` needs tokio's `test-util` feature in your dev-dependencies.

## Reporting

A client's failed attempts, the 429s that a pacer pauses on included, go to the client's own [`Observer`](../reference/crate.md) when it has one and to the process-wide one otherwise, as its token usage does. [Export to OpenTelemetry](export-to-opentelemetry.md) shows them counted.
