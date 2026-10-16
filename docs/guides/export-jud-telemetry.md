---
title: Export the jud command's telemetry
description: How to send the traces and metrics of a jud run to an OpenTelemetry collector over OTLP/HTTP with the cli-otel build, switched on by OTEL_EXPORTER_OTLP_ENDPOINT, what is exported, how short the wait is when the collector is down, and what never changes (stdout, exit status).
status: current
last_reviewed: 2026-10-16
tags: [judgment, jud, cli, opentelemetry, observability, otlp, how-to]
---

# Export the jud command's telemetry

**You will** see a `jud` run (one span per call, plus token, duration and retry metrics) in an OpenTelemetry backend. **Prerequisites:** [Install jud](../start/install.md), and an OTLP/HTTP endpoint: an OpenTelemetry Collector, or a backend that takes OTLP directly. The library's side of this, for an application that owns its SDK, is [Export to OpenTelemetry](export-to-opentelemetry.md).

## Build the variant

Export is a separate build, so the default `jud` carries no SDK and no exporter:

```sh
cargo install judgment --features cli-otel
```

`cli-otel` is `cli` plus the OpenTelemetry SDK, the OTLP/HTTP exporter (protobuf bodies, rustls, no gRPC stack) and the `tracing` bridge. The library never links them; only this binary does.

## Switch it on

Set the endpoint of your collector. Nothing starts without it:

```sh
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 jud eval rubric.jud cases.jud
```

Traces go to `/v1/traces` and metrics to `/v1/metrics` under that endpoint. The exporters read the other standard `OTEL_*` variables themselves: `OTEL_EXPORTER_OTLP_HEADERS` for an API key, `OTEL_RESOURCE_ATTRIBUTES`, the per-signal endpoints. The service name is `jud`.

What is exported is what the library records: the `typesafe.evaluate` span with the GenAI attributes and the instruments `OtelObserver` writes, with the same bounded attributes (the state's content and request ids are never one). Spans and metrics exist for calls made through the HTTP client; a run answered from `--replay` makes none.

## What it never changes

- **stdout** is the bytes a run without telemetry prints.
- **The exit status** is the command's own: 0, 1, 2 or 3.
- **A failure of telemetry** is one line on stderr (`jud: telemetry not exported (...)`) and nothing else. A collector that is down delays the exit by at most the export timeout, three seconds unless `OTEL_EXPORTER_OTLP_TIMEOUT` sets another.

The export is flushed when the command finishes, on every exit status, so a run that fails (status 1) is exported too.

## Check it

Point the endpoint at a local collector with a `debug` exporter and run one call; the span `system_one {model}` and `gen_ai.client.token.usage` show up in its log. `cargo tree -d --features cli-otel` shows one copy of each OpenTelemetry crate, which is why they are pinned together.
