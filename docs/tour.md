---
title: What is in the crate
description: A tour of the crate's modules, question, answer, backend, client, http, error, eval, observer and contract, with the guarantee each one makes and the trade-off behind it, for a reader deciding where a concern belongs before opening the rustdoc.
status: current
last_reviewed: 2026-10-04
tags: [judgment, design, api, modules]
---

# What is in the crate

The rustdoc documents every type; this page says what each module is for and what it promises, so a reader knows where a concern lives before reading the reference. The modules follow the path of a call: a question is built, sent through a backend, answered, checked, read, and counted.

## `question`: the builder and the handles

`Questions` collects the typed questions of one request. Adding one returns a `Handle<A>` that fixes the answer type `A`: `noul` for a yes or no probability, `choice::<T>` for one option out of a closed set `T`, `score` for a position on ordered levels, and `dynamic_choice` for an option set known only at runtime, which reads as `Choice<String>` so the boundary is visible in the type ([decision 0003](decisions/0003-typed-handles-between-questions-and-answers.md)). The `options!` macro writes an enum and its wire keys and descriptions together, so the request criteria and the parser derive from one definition and cannot disagree.

The builder checks the HTTP API reference page's limits before anything is sent: 255 options at most, 2 to 10 levels, non-empty and unique question ids, non-empty option keys, no `null` level. They are stricter than the OpenAPI document in places, and the rustdoc of the module says why each bound is the server's or the crate's own ([hosted API record](verification/hosted-typesafe.md)). A request that would be refused upstream fails here, before it costs a call.

## `answer`: typed reading, tolerant decoding

`Probability` and `Confidence` are newtypes that refuse a value outside `[0, 1]` at deserialisation, because a value no threshold can use is better an error than a number. The wire `Answer` decodes tolerantly: an answer kind this release does not know becomes `Answer::Unknown` (the client logs it at `warn`), a missing `usage` reads as zero, and undocumented top-level fields (a compatible server's `routing`, say) are kept in `Response::extra`. A known answer that breaks its shape is still an error.

`Response::verify(&questions)` holds a response against the questions it was sent for: an answer under every id, of the question's primitive, a Choice naming only options it offered, a Score whose legend parses to the levels sent and whose value is on their scale. The official SDKs check the shape of each answer and stop there; here an answer that names an option nobody offered is an error, never read as a guess. `Response::get(&handle)` then returns `Noul`, `Choice<T>` or `Score`, each with the formulas TypeSafe documents (`confidence_from_probabilities`, `expected_value`) computed from the wire's probabilities, so a server that defines confidence otherwise is a number a caller can log rather than a surprise.

## `backend`: one trait, four implementations

`SystemOne` is the one-method trait every source of answers implements, so the code consuming judgments never knows which. `Client` is one. `Fake` answers from a table and remembers what it was asked. `Recorder` writes another backend's responses to a directory. `Replay` answers from that directory offline, keyed by a content hash of the request. Every one of them verifies its response before returning it, so a response that reaches the caller answers what was asked whichever backend is behind the trait. The client does not retry a response that does not fit, because the call was billed; it still reports the usage and counts the attempt as failed, `unfit`, beside `decode` for a 2xx body that does not decode. [Testing without the model](testing.md) shows the fake and the recordings in use.

## `client` and `http`: the wire, with the SDKs' retries

`Client` (feature `http`, on by default) sends the request with the official SDKs' defaults and a `RetryPolicy`: two retries of 408, 429, 5xx and transport failures, exponential backoff whose jitter only shortens a wait, and the server's wait (`retry-after-ms`, or `Retry-After` in seconds or as an HTTP date) honoured up to a cap. An overall retry budget is available and off by default; `RetryPolicy::conservative()` retries only what cannot have been billed twice. The rustdoc of `RetryPolicy` lists where it matches the SDKs and where it deliberately differs, and [How judgment works](design.md#retries) draws the loop.

`Client::evaluate_with` takes a `CallOptions` for one call's timeout, retry policy, headers and extra body fields, and `ClientBuilder::default_header` sets a header on every call. What the client sets itself (the key, the content type, the user agent, the retry count both SDKs own, and the `state`, `model` and `questions` fields) is refused with an error before anything is sent, where the SDKs silently keep or overwrite it. Options stay off the `SystemOne` trait, so a recording's key is unchanged. The client follows no redirect and caps the body it buffers (8 MiB by default).

`http` is the retry loop behind `Client`, public so another `reqwest` client in the same application can retry the same way and report to the same observer.

## `error`: grouped by what fixes it

One enum, `#[non_exhaustive]`, grouped by remedy: configuration, request, transient (after the retries stopped), transport, decode, and reading an answer. A 400 or a 422 is `InvalidRequest` with the server's message and the fields it names as `ValidationIssue`s with dotted paths (`questions.urgency.score.criteria`), plus the server's machine-readable `kind` when the body carries one; `is_request_too_large` names the refusals whose remedy is a smaller request. A 403 is `PermissionDenied`, apart from a 401, because a new key does not fix it. A malformed API key is `InvalidApiKey` when the client is built, before any request, and no message quotes the key. Every error that came from an HTTP response, and every `Response`, carries TypeSafe's request id when the API sent one, the id its support asks for; the `typesafe.*` spans record it too ([How judgment works](design.md#errors-and-the-request-id)).

## `eval`: what makes a probability trustworthy

Recordings for replay, one `Judgment` per answer and label, and per-question accuracy with a 95% Wilson interval, Brier score, calibration error and confidence when right or wrong. `fingerprint` is the canonical hash that keys a recording, over any JSON value, so a harness can name the questions a run was recorded under and refuse to grade old answers under new questions. The module makes calibrated probabilities a measurement rather than an assumption; the thresholds that act on them stay in the application.

## `observer` and `contract`

`Observer` is the seam an application uses to count tokens and failed attempts in its own metrics; the crate emits `tracing` spans and nothing else, so no telemetry stack is forced on a consumer. `contract::OPENAPI_DOCUMENT` (feature `openapi`, off by default) is the vendored TypeSafe OpenAPI document as text, for an application that validates its own traffic against the contract the crate is tested against ([how the crate is checked](verification/method.md)).

## Without the `http` feature

The crate is the questions, the answers, the fake and replay backends, the metrics and the contract, for a project that brings its own transport. CI builds and checks every test target in that configuration.
