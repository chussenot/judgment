---
title: The crate
description: The judgment crate as a dependency; its features and what each compiles, its modules with a link to the rustdoc of each, the error groups, the limits and the defaults.
status: current
last_reviewed: 2026-10-07
tags: [judgment, crate, features, modules, errors, reference]
---

# The crate

`judgment` on [crates.io](https://crates.io/crates/judgment); the rustdoc on [docs.rs](https://docs.rs/judgment/latest/judgment/) is the reference for every type, method and error. This page is the map: what each feature compiles, what each module is for, and the numbers. The minimum supported Rust is 1.91 (`rust-version` in `Cargo.toml`).

## Features

| Feature | Default | Compiles | Pulls in |
|---|---|---|---|
| `http` | yes | `Client`, `ClientBuilder`, `CallOptions`, `RetryPolicy`, the `http` module (the retry loop), `Client::list_models` | `reqwest` (rustls), `tokio`, `fastrand`, `httpdate` |
| `jud` | no | The `jud` module: the `.jud` reader and writer, `Rubric`, `Cases`, `Verdict`, `Replay` reading `.jud` recordings | `serde-saphyr` |
| `openapi` | no | `contract::OPENAPI_DOCUMENT`, the vendored TypeSafe OpenAPI document as text, about 25 KB | nothing |
| `cli` | no | The `jud` binary (`src/bin/jud/`); implies `jud` and `http` | `clap`, `clap_complete` |

Without `http` the crate is the questions, the answers, `Fake`, `Recorder`, `Replay`, `eval`, `observer` and, with `jud`, the format: everything a project with its own transport needs. CI builds and tests that configuration (`cargo check --no-default-features --all-targets`).

```toml
[dependencies]
judgment = "0.11"                                   # the client; pin the minor, 0.x may break between minors
judgment = { version = "0.11", features = ["jud"] } # with the .jud format
judgment = { version = "0.11", default-features = false }   # no HTTP client
```

## Modules

In the order of one call: a question is built, sent through a backend, answered, checked, read, and counted.

| Module | What it is for | Reference |
|---|---|---|
| `question` | `Questions` collects the typed questions of one request; adding one returns a `Handle<A>` that fixes the answer type (`noul`, `choice::<T>`, `score`, `dynamic_choice`). The `options!` macro writes an enum and its wire keys together. The builder checks the limits below before anything is sent. | [docs.rs](https://docs.rs/judgment/latest/judgment/question/index.html) |
| `answer` | `Response` and the wire `Answer`; `Probability` and `Confidence`, which refuse a value outside `[0, 1]`; `Response::verify` holds a response against the questions; `Response::get(&handle)` reads `Noul`, `Choice<T>` or `Score`. | [docs.rs](https://docs.rs/judgment/latest/judgment/answer/index.html) |
| `backend` | The `SystemOne` trait and three implementations beside the client: `Fake`, `Recorder`, `Replay`. | [docs.rs](https://docs.rs/judgment/latest/judgment/backend/index.html) |
| `client` (`http`) | `Client`, `ClientBuilder`, `CallOptions`: the wire with the official SDKs' retries. | [docs.rs](https://docs.rs/judgment/latest/judgment/client/index.html) |
| `http` (`http`) | `RetryPolicy` and `send_with_retries`, the retry loop behind the client, public so another `reqwest` client can retry the same way. | [docs.rs](https://docs.rs/judgment/latest/judgment/http/index.html) |
| `error` | `Error`, `Result`, `ValidationIssue`. | [docs.rs](https://docs.rs/judgment/latest/judgment/error/index.html) |
| `eval` | `Recording`, `Judgment`, the metrics (`metrics`), the fingerprints (`canonical`) and the sweeps (`tuning`). | [docs.rs](https://docs.rs/judgment/latest/judgment/eval/index.html) |
| `jud` (`jud`) | `parse`, `Rubric`, `Cases`, `Case`, `Verdict`, `parse_recording`, `recording_to_yaml`, `present`, `API_VERSION`. | [docs.rs](https://docs.rs/judgment/latest/judgment/jud/index.html) |
| `observer` | `Observer`, the seam through which token usage and failed attempts reach the application's metrics. The crate emits `tracing` spans (`typesafe.evaluate`, `typesafe.list_models`) and no metrics of its own. | [docs.rs](https://docs.rs/judgment/latest/judgment/observer/index.html) |
| `contract` (`openapi`) | `OPENAPI_DOCUMENT`. | [docs.rs](https://docs.rs/judgment/latest/judgment/contract/index.html) |

The crate root re-exports `Questions`, `Handle`, `Question`, `Options`, `NoulCriteria`, the answer types, `SystemOne`, `Fake`, `Recorder`, `Replay`, `Error`, `Result`, `ValidationIssue`, `Observer` and, with `http`, the client types.

### The `jud` feature

`judgment::jud` implements [The .jud format](jud-format.md) at `API_VERSION` (`jud/v1.3`), the one apiVersion it reads and writes. `Rubric` has `parse`, `to_yaml`, `fingerprint`, `policy_fingerprint`, `gate`, `lower` and `apply`; `Cases` has `parse`, `to_yaml`, `fingerprint` and `bind`, with `Case::request` and `Case::per_turn`; `grade` grades one case against one response; `present` is the state-path test; `parse_recording` and `recording_to_yaml` read and write the third kind into `eval::Recording`. `Questions::handle` gives a typed handle to a question read from a file. `Replay` reads `.jud` recordings beside its `.json` ones.

## Errors

`Error` is one enum, `#[non_exhaustive]`, grouped by what fixes it. The variants and their remedies are on [docs.rs](https://docs.rs/judgment/latest/judgment/enum.Error.html).

| Group | Variants | Remedy |
|---|---|---|
| Configuration | `MissingApiKey`, `InvalidApiKey`, `Url`, `ReservedHeader`, `ReservedField` | fix the builder's input; refused before any call |
| Request refused | `Unauthorized`, `PermissionDenied`, `InvalidRequest`, `InvalidQuestion`, `DuplicateQuestionId` | fix the key, the permission or the question |
| Transient, after the retries | `RateLimited`, `Overloaded`, `Http` | retry later, fall back, or raise the policy |
| Transport | `Transport`, `ResponseTooLarge` | the network, the server, or the body cap |
| Decode | `Decode`, `Io`, `InvalidRecording`, `NoRecording` | a body or a file the crate cannot read, or a request nobody recorded |
| Reading an answer | `MissingAnswer`, `AnswerTypeMismatch`, `UnknownOption`, `InvalidAnswer`, `NotAProbability` | the response does not fit the questions; the call was billed and is not retried |

Every error that came from an HTTP response carries the server's request id (`Error::request_id()`), a 2xx whose body does not decode included.

## Limits

Checked by the builder before a request is sent, and by the `.jud` reader:

| Limit | Value | Constant |
|---|---|---|
| Options per Choice | 2 to 255 | `question::MAX_CHOICE_OPTIONS` |
| Levels per Score | 2 to 10 | `question::MAX_SCORE_LEVELS` |
| Question ids | non-empty, unique | |
| Option keys | non-empty | |
| Response body | 8 MiB by default | `RetryPolicy::max_body_bytes` |

## Defaults

`RetryPolicy::default()`, the official SDKs' values:

| | Value |
|---|---|
| Retries | 2 |
| Backoff | 500 ms, doubled each retry, capped at 5 s |
| Jitter | up to 25 % off a wait, never added |
| Statuses retried | 408, 429, every 5xx; transport failures |
| Server's wait honoured | `retry-after-ms`, or `Retry-After` in seconds or as an HTTP date ([RFC 9110](https://www.rfc-editor.org/rfc/rfc9110#field.retry-after)), up to 30 s |
| Overall budget | none |

`RetryPolicy::conservative()` retries only 408, 429 and a connection that was never made. The client's own defaults are in [Configuration](configuration.md#the-crates-client); `eval` uses 10 calibration bins (`eval::ECE_BINS`) and `z = 1.96` for the 95 % interval (`eval::Z_95`).
