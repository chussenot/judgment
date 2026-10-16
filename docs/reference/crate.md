---
title: The crate
description: The judgment crate as a dependency; its features and what each compiles, its modules with a link to the rustdoc of each, the error groups, the limits and the defaults.
status: current
last_reviewed: 2026-10-16
tags: [judgment, crate, features, modules, errors, reference]
---

# The crate

`judgment` on [crates.io](https://crates.io/crates/judgment); the rustdoc on [docs.rs](https://docs.rs/judgment/latest/judgment/) is the reference for every type, method and error. This page is the map: what each feature compiles, what each module is for, and the numbers. The minimum supported Rust is 1.91 (`rust-version` in `Cargo.toml`).

## Features

| Feature | Default | Compiles | Pulls in |
|---|---|---|---|
| `http` | yes | `Client`, `ClientBuilder`, `CallOptions`, `RetryPolicy`, the `http` module (the retry loop), `Client::list_models`; implies `pacer` | `reqwest` (rustls), `tokio`, `fastrand`, `httpdate` |
| `pacer` | no (implied by `http`) | `Pacer` and `Paced`, a bound on calls in flight and one pause every call honours; with `http`, `ClientBuilder::pacer` and the pause a 429's named wait sets ([Bound a stream of calls](../guides/bound-a-stream.md)) | `tokio` (sync and time) |
| `jud` | no | The `jud` module: the `.jud` reader and writer, `Rubric`, `Cases`, `Verdict`, `Replay` reading `.jud` recordings | `serde-saphyr` |
| `openapi` | no | `contract::OPENAPI_DOCUMENT`, the vendored TypeSafe OpenAPI document as text, about 25 KB | nothing |
| `otel` | no | `observer::otel::OtelObserver`, an `Observer` that records OpenTelemetry metrics through the API crate ([Export to OpenTelemetry](../guides/export-to-opentelemetry.md)); the SDK and exporters stay the application's | `opentelemetry` (API, metrics only) |
| `cli` | no | The `jud` binary (`src/bin/jud/`); implies `jud` and `http` | `clap`, `clap_complete` |
| `multipart` | no | `CallOptions::multipart` and `ClientBuilder::multipart`: a request's images sent as `multipart/form-data` parts beside a JSON `request` part instead of base64 in the JSON body, the transport TypeSafe recommends for images; implies `http` ([decision 0028](../project/decisions/0028-multipart-is-a-feature-and-a-switch.md)) | `reqwest`'s multipart encoding, `mime_guess`, `bytes` |

Without `http` the crate is the questions, the answers, `Fake`, `Recorder`, `Replay`, `eval`, `observer` and, with `jud`, the format: everything a project with its own transport needs. CI builds and tests that configuration (`cargo check --no-default-features --all-targets`).

```toml
[dependencies]
judgment = "0.14"                                   # the client; pin the minor, 0.x may break between minors
judgment = { version = "0.14", features = ["jud"] } # with the .jud format
judgment = { version = "0.14", default-features = false }   # no HTTP client
```

## Modules

In the order of one call: a question is built, sent through a backend, answered, checked, read, and counted.

| Module | What it is for | Reference |
|---|---|---|
| `question` | `Questions` collects the typed questions of one request; adding one returns a `Handle<A>` that fixes the answer type (`noul`, `choice::<T>`, `score`, `dynamic_choice`, `r#box`). The `options!` macro writes an enum and its wire keys together. The builder checks the limits below before anything is sent. | [docs.rs](https://docs.rs/judgment/latest/judgment/question/index.html) |
| `answer` | `Response` and the wire `Answer`; `Probability` and `Confidence`, which refuse a value outside `[0, 1]`; `Response::verify` holds a response against the questions; `Response::get(&handle)` reads `Noul`, `Choice<T>`, `Score` or `BoundingBox` (`BoxEdges` as fractions of the image, `PixelBox` at a size). | [docs.rs](https://docs.rs/judgment/latest/judgment/answer/index.html) |
| `asset` | `Image` (a PNG or a JPEG, its format and size read from the bytes, the hosted API's limits checked), `ImageFormat`, and `Assets`, the request's images by reference. |  [docs.rs](https://docs.rs/judgment/latest/judgment/asset/index.html) |
| `backend` | The `SystemOne` trait (`answer`, and `answer_with_assets` for a request with images) and three implementations beside the client: `Fake`, `Recorder`, `Replay`; with `pacer`, `Pacer` and `Paced`. | [docs.rs](https://docs.rs/judgment/latest/judgment/backend/index.html) |
| `client` (`http`) | `Client`, `ClientBuilder`, `CallOptions` (a timeout, a retry policy, headers, extra fields, long context and, with `multipart`, the images as parts, for one call): the wire with the official SDKs' retries, and the two danger-zone headers, sent for images and Box questions on their own and for long context when asked. | [docs.rs](https://docs.rs/judgment/latest/judgment/client/index.html) |
| `http` (`http`) | `RetryPolicy`, `send_with_retries` and `send_with_retries_paced` (which also shares a `Pacer` and reports to an observer of its own), the retry loop behind the client, public so another `reqwest` client can retry the same way. | [docs.rs](https://docs.rs/judgment/latest/judgment/http/index.html) |
| `error` | `Error`, `Result`, `ValidationIssue`. | [docs.rs](https://docs.rs/judgment/latest/judgment/error/index.html) |
| `eval` | `Recording`, `Judgment`, the metrics (`metrics`), the fingerprints (`canonical`) and the sweeps (`tuning`). | [docs.rs](https://docs.rs/judgment/latest/judgment/eval/index.html) |
| `jud` (`jud`) | `parse`, `Rubric`, `Cases`, `Case`, `Verdict`, `AssetSource`, `load_assets`, `BoxLabel`, `parse_recording`, `recording_to_yaml`, `present`, `API_VERSION`, `MINOR`. | [docs.rs](https://docs.rs/judgment/latest/judgment/jud/index.html) |
| `observer` | `Observer`, the seam through which token usage, failed attempts and each finished call (`CallReport`) reach the application's metrics; with `otel`, `otel::OtelObserver`. The crate emits `tracing` spans (`typesafe.evaluate`, `typesafe.list_models`) and no metrics of its own. | [docs.rs](https://docs.rs/judgment/latest/judgment/observer/index.html) |
| `semconv` | The OpenTelemetry attribute and metric names the crate writes, as constants: the `gen_ai.*`, `server.address` and `error.type` ones. |  [docs.rs](https://docs.rs/judgment/latest/judgment/semconv/index.html) |
| `contract` (`openapi`) | `OPENAPI_DOCUMENT`. | [docs.rs](https://docs.rs/judgment/latest/judgment/contract/index.html) |

The crate root re-exports `Questions`, `Handle`, `Question`, `Options`, `NoulCriteria`, the answer types, `Assets`, `Image`, `ImageFormat`, `SystemOne`, `Fake`, `Recorder`, `Replay`, `Error`, `Result`, `ValidationIssue`, `Observer`, with `pacer` `Pacer` and `Paced`, and with `http` the client types.

### The `jud` feature

`judgment::jud` implements [The .jud format](jud-format.md) at `API_VERSION` (`jud/v1`) up to `MINOR` (4), the line it reads and writes; the earlier spelling of minor 3, `jud/v1.3`, is read as it stands (`EARLIER_SPELLINGS`). `Rubric` has `parse`, `to_yaml`, `fingerprint`, `policy_fingerprint`, `gate`, `lower` and `apply`; `Cases` has `parse`, `to_yaml`, `fingerprint` and `bind`, with `Case::request` and `Case::per_turn`; `grade` grades one case against one response (a Box label by intersection over union); `Rubric::load_assets` and `Case::load_assets` read a document's declared images into a request's `Assets`; `present` is the state-path test; `parse_recording` and `recording_to_yaml` read and write the third kind into `eval::Recording`. `Questions::handle` gives a typed handle to a question read from a file. `Replay` reads `.jud` recordings beside its `.json` ones.

## Errors

`Error` is one enum, `#[non_exhaustive]`, grouped by what fixes it. The variants and their remedies are on [docs.rs](https://docs.rs/judgment/latest/judgment/enum.Error.html).

| Group | Variants | Remedy |
|---|---|---|
| Configuration | `MissingApiKey`, `InvalidApiKey`, `Url`, `ReservedHeader`, `ReservedField` | fix the builder's input; refused before any call |
| Request refused | `Unauthorized`, `PermissionDenied`, `InvalidRequest`, `InvalidQuestion`, `DuplicateQuestionId`, `InvalidImage`, `InvalidAsset`, `RequestTooLarge` | fix the key, the permission, the question or the image; an image outside the limits is refused before any call |
| Account out of credit | `PaymentRequired` (a 402, OpenRouter's "Insufficient credits") | add credit; a retry or a new key does not help |
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
| An image's side | 256 to 1568 pixels | `asset::MIN_SIDE_PX`, `asset::MAX_SIDE_PX` |
| An image's pixels | 78,400 to 2,458,624 | `asset::MIN_PIXELS`, `asset::MAX_PIXELS` |
| An image's bytes | 16 MiB | `asset::MAX_IMAGE_BYTES` |
| A request's images, and its body | 32 MiB | `asset::MAX_REQUEST_BYTES` |
| A reference | 1 to 1024 bytes, 65,536 bytes for all of a request's | `asset::MAX_REFERENCE_BYTES`, `asset::MAX_REFERENCES_BYTES` |

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
