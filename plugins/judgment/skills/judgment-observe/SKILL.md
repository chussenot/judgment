---
name: judgment-observe
description: Observe and pace Rust code that calls the judgment crate - the Observer trait (tokens, cost, failed attempts, one CallReport per call), OtelObserver and the GenAI semantic-convention attributes behind the otel feature, the tracing spans the client writes, and Pacer and Paced to bound a stream of calls so one 429's wait pauses them all. Use whenever a task is about metrics, cost, token usage, tracing, OpenTelemetry, dashboards or alerts for judgment calls, or about running many calls at once (a stream, a batch, a fan-out, rate limits, 429s), even when the user does not say "judgment".
---

# Observing and pacing judgment calls

The crate writes `tracing` spans and no metrics of its own. Everything an
application measures about its calls (tokens, cost, failed attempts,
duration, the model that answered, how a call failed) reaches it through
one seam, the `Observer`. And a program that calls more than once at a
time bounds itself with a `Pacer`, which the client feeds: a 429 that
names a wait pauses every call that shares it. The two pages under
`references/` (generated from the crate's guides) are the long form.

## The Observer

```rust
use std::sync::Arc;
use judgment::{Client, Observer};
use judgment::observer::CallReport;

struct Metrics { /* the application's counters */ }

impl Observer for Metrics {
    fn on_call(&self, call: &CallReport<'_>) {
        // call.provider, call.request_model, call.response_model,
        // call.usage (tokens and cost, present for an unfit answer too: it was billed),
        // call.duration (retries and waits included), call.error_type (None on success)
    }
    fn on_failed_attempt(&self, service: &'static str, status: &str) {
        // one per attempt the retry loop gave up on: "429", "503", "transport"
    }
}

let client = Client::builder().observer(Arc::new(Metrics { /* ... */ })).build()?;
```

- Four methods, each with a default that does nothing: `on_usage(model,
  usage)`, `on_assets(model, images, bytes)`, `on_failed_attempt(service,
  status)`, `on_call(&CallReport)`. Implement the ones the application
  uses. `CallReport` is `#[non_exhaustive]`: read its fields, never build
  one.
- `judgment::observer::set_global(Arc<dyn Observer>)` sets one for every
  client the process builds; the builder's `observer(...)` wins for its
  client.
- What to alert on: `error_type` grouped (`unfit` and `decode` are never
  retried and mean the server answered something the questions did not
  allow; a status number is after the retries), failed attempts per
  minute (a server shedding load before it fails a call), cost per
  decision, and `response_model` changing (an alias moved under bars
  tuned on the old version).
- Counting tokens by hand from `Response::usage` beside an observer counts
  them twice; remove the hand count.

## OpenTelemetry

With the `otel` feature, `judgment::observer::otel::OtelObserver` records
the metrics through the OpenTelemetry API with the GenAI semantic
conventions (`gen_ai.client.token.usage`,
`gen_ai.client.operation.duration`, attributes from `judgment::semconv`).
The SDK and the exporter stay the application's: build the meter provider
as the application already does, then pass the observer to the client.
`references/export-to-opentelemetry.md` has the code and the attribute
list. The spans (`typesafe.evaluate`, `typesafe.list_models`) come
through `tracing`; a `tracing-opentelemetry` layer the application already
has exports them. The `jud` command's own export is
`OTEL_EXPORTER_OTLP_ENDPOINT` with a `cli-otel` build, not this.

## A stream of calls

```rust
use std::sync::Arc;
use judgment::{Client, Paced, Pacer, SystemOne};

let pacer = Pacer::new(8);                                   // eight calls in flight
let client = Client::builder().pacer(pacer.clone()).build()?; // a 429's wait pauses the pacer
let backend = Arc::new(Paced::new(client, pacer));            // every call waits its turn
```

- One `Pacer` shared by the client and the `Paced` backend: the client
  sets the pause from the server's `retry-after`, and every call through
  `Paced` waits it out, so a 429 is a pause and not a cascade of failures.
- Size it to what the server takes, not to the work: a hosted API's rate
  limit, a local server's cores. A spawned task per event with no pacer
  outruns the server.
- When states arrive faster than answers, sample or drop at the edge, and
  count what was dropped; an unbounded queue only delays the failure.
- `references/bound-a-stream.md` has the `JoinSet` loop and a test with a
  paused tokio clock.

## Workflow

1. Read the call sites and what the application already uses for metrics
   and tracing (its meter provider, its `tracing` subscriber).
2. Add the observer at the client's construction (or `set_global` when
   clients are built in many places), the pacer where calls fan out.
3. Remove what the observer now does by hand.
4. Write the test that holds it: a `Fake` (no `on_call` from a fake; the
   client reports it) or a wiremock server behind a real `Client`, the
   observer's counts asserted after a success, a 429 and an unfit answer.
5. `cargo check`, `cargo clippy -- -D warnings`, `cargo test`, as run.

## Reply

What was wired where, the metrics or attributes the application now gets
and their names, what was removed, the pacer's size and why, the alerts
proposed, and the checks run with their results.
