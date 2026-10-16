---
title: Export to OpenTelemetry
description: How to send a client's calls to an OpenTelemetry backend: the spans the client already writes, now carrying the GenAI semantic-convention attributes, and the metrics (tokens, duration, failed attempts, cost, optionally the confidence of each answer) that OtelObserver records through the OpenTelemetry API, behind the otel feature, with an OpenTelemetry Collector snippet and what each instrument and attribute means; the conventions are at Development status.
status: current
last_reviewed: 2026-10-16
tags: [judgment, opentelemetry, observability, observer, tracing, how-to, rust]
---

# Export to OpenTelemetry

**You will** see a client's calls in an OpenTelemetry backend: one span per call, and metrics for tokens, duration, failed attempts and cost. **Prerequisites:** [Your first decision in Rust](../start/first-decision-rust.md), and an application that already has an OpenTelemetry SDK or collector.

The crate emits `tracing` spans and reports to an [`Observer`](../reference/crate.md); it picks no metrics backend and links no OpenTelemetry SDK. The `otel` feature adds the one piece that needs the OpenTelemetry **API** (`opentelemetry`, metrics only): an `Observer` that records to a `Meter` you give it. The SDK, the exporter and where the data goes stay yours.

!!! warning "Development status"
    The OpenTelemetry [GenAI semantic conventions](https://opentelemetry.io/docs/specs/semconv/gen-ai/) are at Development status, so an attribute or metric name can still change upstream. Every name this crate writes is a constant in `judgment::semconv`, the one place to edit when one does.

## The spans

The client's `typesafe.evaluate` span is a `client` span named `system_one {model}` for any `tracing` subscriber that reads `otel.kind` and `otel.name`, such as `tracing-opentelemetry`. It carries:

| Attribute | Value |
|---|---|
| `gen_ai.operation.name` | `system_one`: a System One call evaluates typed questions and generates no text, so it has an operation of its own, outside the conventions' `chat` and `embeddings` |
| `gen_ai.provider.name` | `typesafe`, `openrouter`, `ollama` or `other`, from the base URL's host; a Laya or autojev server on a local address reads as `other` |
| `gen_ai.request.model` | the model the request named, alias included |
| `gen_ai.response.model` | the versioned model that answered |
| `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens` | the response's usage, also when the response did not fit the questions: the tokens were spent |
| `server.address` | the base URL's host |
| `error.type` | on failure: an HTTP status as a number (`500`), or a short name (`rate_limited`, `transport`, `decode`, `unfit`, `too_large`, ...) |

The span's `images`, `long_context`, `multipart` and `request_id` fields are as before. The retries of one call are `warn` events inside this span, not spans of their own.

## The metrics

With the `otel` feature, give an `OtelObserver` a meter from your provider and set it on the client:

```rust no_run
use std::sync::Arc;

use judgment::Client;
use judgment::observer::otel::OtelObserver;
use opentelemetry::metrics::MeterProvider;

fn client(provider: &opentelemetry_sdk::metrics::SdkMeterProvider) -> judgment::Result<Client> {
    let observer = OtelObserver::new(provider.meter("judgment"));
    Client::builder().observer(Arc::new(observer)).build()
}
```

| Instrument | Kind, unit | Attributes |
|---|---|---|
| `gen_ai.client.token.usage` | histogram, `{token}` | operation, provider, request model, response model, `gen_ai.token.type` (`input` or `output`) |
| `gen_ai.client.operation.duration` | histogram, `s` | operation, provider, request model, `server.address`, and `error.type` on a failed call |
| `judgment.retry.attempts` | counter, `{attempt}` | `error.type`: the status, or `transport`, `decode`, `unfit`, `too_large` |
| `judgment.usage.cost` | histogram, `USD` | operation, provider, request model; recorded only when the server reports a cost (OpenRouter does, TypeSafe's own API does not) |
| `judgment.answer.confidence` | histogram, `1` | request model, question id; only when asked for |

A call's duration is its wall time with every retry and wait, and a call that fails is recorded too, with its `error.type`. A response that did not fit the questions still records its tokens and cost, since they were billed.

Every attribute has a bounded set of values, so none multiplies series without limit, except the question id on the confidence histogram: it is your own string. `OtelObserver::new(meter).answer_confidence(true)` records the confidence of every Choice and Score answer of a successful call by question id; switch it on when your ids are few and fixed. A request id and the state's content are never an attribute.

!!! note "Failed attempts need the global observer"
    A failed attempt is reported to the process-wide observer, not to a client's own, because the retry loop is shared with clients that have none. For `judgment.retry.attempts` to count, also install the same observer with `judgment::observer::set_global`; the other instruments come from the client's own.

## A collector to receive them

Any OTLP endpoint works. This `OpenTelemetry Collector` configuration receives OTLP over HTTP and prints what arrives, which is enough to see the instruments before pointing an exporter at a real backend:

```yaml
receivers:
  otlp:
    protocols:
      http:
        endpoint: 0.0.0.0:4318
exporters:
  debug:
    verbosity: detailed
service:
  pipelines:
    metrics:
      receivers: [otlp]
      exporters: [debug]
    traces:
      receivers: [otlp]
      exporters: [debug]
```

Point your SDK's OTLP exporter at `http://localhost:4318`. The `jud` command line does not export telemetry yet.
