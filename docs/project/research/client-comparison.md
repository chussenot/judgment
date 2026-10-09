---
title: Compared with the other Rust clients
description: A feature-by-feature comparison of judgment with the nine most-downloaded Rust crates for the System One wire, each read from its published source on 2026-10-04 and measured for dependency footprint and build time, in five grids (the wire, reading answers, resilience, testing and evaluation, observability and footprint), with what judgment should take from the others next.
status: current
last_reviewed: 2026-10-04
tags: [research, judgment, typesafe, rust, sdk, comparison]
---

# Compared with the other Rust clients

A consumer choosing a client for the System One wire has about thirty crates to pick from, most of them a few weeks old. The question this page answers is not which is biggest but which one lets a decision made on a probability be trusted: does a wrong answer become a type error or a guess, does a transient failure become a retry or an outage, can the decision be tested without a key, and what does the dependency cost. The [earlier survey](system-one-client-libraries.md) read the field on 2026-09-25 to decide what judgment 0.2 should become; this page reads it again on 2026-10-04, after judgment 0.3.0, against the crates people download.

## Method

- **Which crates.** The nine most-downloaded client crates on crates.io on 2026-10-04 (totals in the last grid), plus `typesafe-client`, the one closest to judgment in architecture. Crates that are engines or servers rather than clients (`kime`, `laya-*`, `jigor`), adapters inside a larger framework (`everruns-integrations-typesafe`, `gaise-provider-typesafe`) and command-line tools are out. `kunobi-jev`, the most-downloaded name, is a three-line shim over `kunobi-decision` since 0.3.0 and is counted with it. `typesafe-sdk-rust` (zchee), in the earlier survey, is no longer on crates.io.
- **How.** Every crate's published source was downloaded from crates.io at the version named and read, not its README; each cell below is backed by a file and line in that source, listed in the evidence section. A README claim counts only where the code backs it. judgment was read the same way, by a reviewer told not to flatter it.
- **Footprint.** A fresh library crate depending on each one with default features, on one machine (4 CPUs, rustc 1.94.1 for judgment's own toolchain, 1.97.0 stable for the measurement, cold `target`, warm registry): transitive dependency count from `cargo tree`, wall time of a clean `cargo build` in the dev profile. One run each; treat differences under ten seconds as noise. Build time tracks the TLS stack more than the crate: the two fastest use reqwest 0.12 with the older rustls, the rest reqwest 0.13 with aws-lc.

Legend: ✓ present as described; ◐ partial, the note says how; ✗ absent.

## 1. The wire and the questions

| Crate | Structured Score levels | Option sets | Limits checked before sending | `GET /v1/models` | Per-call overrides | Builds without the HTTP client |
|---|---|---|---|---|---|---|
| **judgment 0.3.0** | ✓ objects and arrays, `null` refused | ✓ `options!` enum or `dynamic_choice` | ✓ 255 options, 2–10 levels, empty or duplicate id, empty key, null level | ✓ | ✓ timeout, retry, headers, extra body; model on the request | ✓ `http` feature |
| kunobi-decision 0.3.0 | ✓ | ✓ `labels!` enum or runtime strings | ◐ 255 and 2–10; a duplicate id silently replaces | ✓ plus per-provider catalogs | ✓ | ✗ |
| typesafe-sdk 0.2.0 | ◐ no `null` level | ◐ strings only | ✗ a one-level Score and an empty option map are sent | ✓ | ✓ but `extra_body` may overwrite `state` and `questions` | ✗ |
| typesafeai-sdk 0.4.1 | ✓ | ◐ strings in a `HashMap`: wire order is random | ◐ blank names and minimum two; no caps | ✓ | ✓ | ✗ |
| typesafe-sdk-* 0.6.2 | ✓ | ◐ strings; the derive the README shows is not published | ◐ at least one question, at least two levels | ✓ | ✓ | ◐ by taking only the sibling crates without `-http` |
| jev-client 0.2.0 | ✓ | ◐ strings | ◐ complete, but opt-in: the transport never runs it | ✓ | ✗ extra body only | ✗ |
| jev 0.1.2 | ✗ strings only | ◐ strings in a `BTreeMap`: alphabetical order | ✗ | ✗ | ✗ | ✗ |
| typesafe-ai-sdk 0.5.0 | ✓ | ✓ `RubricChoice` derive or runtime | ◐ no caps at runtime | ✓ | ✓ | ✗ |
| typesafeai-sdk-community 0.5.0 | ✓ | ◐ `ChoiceLabels` derive; `BTreeMap` order on the wire | ◐ no caps | ✓ | ✓ `extra_body` may overwrite `state` | ✗ |
| typesafe-client 0.1.0 | ✓ | ✓ `choice_options!` or `ValueChoice<T>` | ✓ 255, 2–10, duplicate id; empty id unchecked | ✓ | ◐ model, timeout, retry; no headers or extra body | ✓ `http` feature |

What the column means for a caller: a limit checked before sending is a 400 that never costs a call; an option set kept in authored order is what the model and the caller both see; building without the client is what lets a project with its own transport, or a test target, avoid compiling reqwest.

## 2. Reading answers safely

| Crate | Typed handle | Probabilities | Response verified against the questions | Unknown answer kind | Missing `usage` | Request id | Errors |
|---|---|---|---|---|---|---|---|
| **judgment** | ✓ `Handle<A>` | ✓ newtypes refusing values outside [0, 1] | ✓ by every backend: ids, kinds, off-list options, legend equals the levels sent | ✓ kept as `Unknown`, warned | ✓ reads as zero | ✓ response, every error, the spans | ✓ `#[non_exhaustive]`, grouped by remedy, 422 issues as paths |
| kunobi-decision | ✓ `AnswerKey<A>` | ✗ bare `f64` | ✗ only when a key is read; an untyped choice is never checked against the options | ✓ kept | ✓ | ✓ with `cf-ray` and `x-request-id` fallbacks | ✓ `#[non_exhaustive]`; 422 flattened to text |
| typesafe-sdk | ✗ by name | ✗ | ✗ | ◐ dropped with a warning | ✓ | ✓ | ✗ not `#[non_exhaustive]`; flattened |
| typesafeai-sdk | ✗ | ✗ | ✗ | ◐ unit variant, payload lost | ✗ required; counts default to 0 | ✓ | ✓ `#[non_exhaustive]`; 422 list not parsed |
| typesafe-sdk-* | ✗ | ✗ | ✗ | ✗ fails the response | ✗ fails the response | ✓ | ✗ not `#[non_exhaustive]`; parsed to text |
| jev-client | ✗ | ✗ | ✗ | ✓ kept | ✓ | ✓ with latency and attempts | ✓ `#[non_exhaustive]`; 422 parsed |
| jev | ✗ | ✗ | ✗ | ◐ tag optional, kind inferred | ✓ | ✗ | ✗ |
| typesafe-ai-sdk | ◐ `Rubric` derive on a struct | ✗ | ✗ | ◐ skipped, kept in `raw` | ✓ | ✓ | ✓ `#[non_exhaustive]`; flattened |
| typesafeai-sdk-community | ◐ `Questions` derive | ✗ | ✗ an absent `answers` object decodes | ◐ skipped, warned | ✗ required object | ✓ | ✓ `#[non_exhaustive]`; flattened |
| typesafe-client | ✓ `QuestionKey<A>` | ✗ | ◐ ids, kinds, options; legend not compared | ✗ hard decode error | ✗ fails | ✓ | ✓ `#[non_exhaustive]`; structured field errors |

The row that matters most is the verification one. Only two crates hold a response against the questions that were sent before any answer is read; everywhere else an option the model was never offered is read as a choice, and a decision made on it is a guess with a confidence attached. judgment and `typesafe-client` arrived at the same mechanism independently; judgment also runs it in the fake and the replay backends, and compares the Score legend.

## 3. Resilience

| Crate | Retry defaults as the SDKs' | Jitter | Server's wait | Budget | Conservative preset | Body cap | Redirects | Key hygiene | Concurrency |
|---|---|---|---|---|---|---|---|---|---|
| **judgment** | ✓ 2 retries, 0.5 s to 5 s, 408, 429, 5xx, transport | ✓ one-sided | ✓ ms, seconds, date; capped at 30 s | ◐ available, off by default | ✓ `conservative()` | ✓ 8 MiB | ✓ none followed | ✓ validated, `set_sensitive`, redacted | ✗ |
| kunobi-decision | ✓ | ✓ | ✓ capped at 60 s | ✓ 10 s by default | ✗ `none()` only | ✗ | ✓ | ✓ `SecretString` | ✓ semaphore |
| typesafe-sdk | ✓ | ✓ | ✓ uncapped | ✓ 30 s | ✗ | ✗ | ✗ followed, body and key re-sent | ✗ `Debug` prints the key | ✗ |
| typesafeai-sdk | ◐ 5xx subset, 10 s cap | ✗ symmetric | ◐ no HTTP date | ✓ 30 s | ✗ | ✓ 1 MiB | ✓ | ✓ | ✗ |
| typesafe-sdk-* | ✓ | ✓ from the clock | ✓ capped at 60 s | ✗ | ✗ | ✗ | ✗ followed | ✓ | ✗ |
| jev-client | ✓ | ✓ | ✓ capped at 60 s | ✗ | ✗ `none()` | ✓ 32 MiB | ✓ | ✓ zeroised | ✓ shared throttle |
| jev | ✗ 429 and 529 only, fixed | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ `Debug` prints the key | ✗ |
| typesafe-ai-sdk | ✓ | ✓ | ✓ uncapped | ✓ 30 s | ✗ | ✗ | ✗ default | ✓ | ✗ |
| typesafeai-sdk-community | ✓ | ✓ | ✓ uncapped | ✓ 30 s | ✗ | ✗ | ✗ default | ◐ no `set_sensitive` | ✓ batch pacer |
| typesafe-client | ✓ | ✓ | ✓ capped at 60 s | ✓ 30 s | ✗ | ✗ | ✗ default | ✓ | ✗ |

Two choices on this grid are judgment's alone and deliberate. The budget is off by default because a budget that cuts a retry short changes the timing of every existing consumer, so it is a caller's decision. Redirects are not followed because the API never redirects its two paths and following one would send the key, and on a 307 the state, to wherever the redirect names. Three crates do better than judgment on one column each: a concurrency limiter or pacer (`kunobi-decision`, `jev-client`, the community SDK), which judgment leaves to the caller today.

## 4. Testing and evaluation

| Crate | Fake behind a trait | Fake refuses an unfit answer | Record and replay | Contract test against the OpenAPI document | Evaluation metrics | Live tests and servers verified |
|---|---|---|---|---|---|---|
| **judgment** | ✓ | ✓ | ✓ keyed by a content hash | ✓ requests, fakes, 61 recordings, examples | ✓ accuracy with Wilson interval, Brier, ECE | ✓ 16 ignored; hosted API and Laya recorded, Ollama task |
| kunobi-decision | ✓ behind a feature | ✓ | ✗ | ✗ | ✗ | ✗ none shipped in the package |
| typesafe-sdk | ✗ a wiremock helper | – | ✗ | ✗ | ✗ | ✗ examples run live behind an env var |
| typesafeai-sdk | ✗ | – | ✗ | ✗ | ✗ | ✗ |
| typesafe-sdk-* | ✓ mock transport | ✗ | ✗ | ✗ | ✗ | ✗ |
| jev-client | ◐ trait, no fake shipped | – | ✗ | ◐ its own JSON Schema against captured bodies | ✗ | ✗ |
| jev | ✗ | – | ✗ | ✗ | ✗ | ◐ four backends measured once in a document |
| typesafe-ai-sdk | ✗ | – | ✓ SHA-256 cassettes | ✗ | ✗ | ✗ |
| typesafeai-sdk-community | ✓ mock transport | ✗ | ◐ cassettes replayed in order | ✗ | ✓ Brier, ECE, AUC, F1; no intervals | ◐ two live tests that skip without a key |
| typesafe-client | ✓ | ✓ | ✗ | ✓ requests and documented responses | ✗ | ◐ four ignored live tests against the hosted API |

A fake that refuses a scripted answer the question could not produce is what keeps a test suite honest after a question changes; three crates have it. Recordings keyed by the request's content are what let a question change fail a test instead of grading old answers; two crates have them, with different keys. Metrics with an interval are what stop a ratio on three cases reading like one on three hundred; one crate has them.

## 5. Observability and footprint

| Crate | `tracing` | Metrics hook | Secrets in logs | Transitive deps | Clean build | Rust lines, src / tests | MSRV | Licence | Downloads |
|---|---|---|---|---|---|---|---|---|---|
| **judgment** | ✓ spans with the request id | ✓ `Observer` | ✓ never; server strings cut to 64 | 100 | 49 s | 8.2k / 5.3k | 1.91 | MIT | 12 |
| kunobi-decision (+ kunobi-jev) | ✓ span with OpenTelemetry fields | ✗ | ✓ | 97 | 44 s | 5.4k / none shipped | 1.94 | Apache-2.0 | 45 + 1,753 |
| typesafe-sdk | ◐ `log`; sets the process-wide level | ✗ | ✓ headers | 104 | 25 s | 3.0k / 1.7k | 1.85 | MIT | 1,211 |
| typesafeai-sdk | ✗ | ✗ | ◐ `Debug` only | 91 | 23 s | 3.2k / 2.4k | 1.88 | MIT | 214 |
| typesafe-sdk-* | ✗ own logger | ✗ | ◐ bodies at debug | 110 | not on stable (MSRV 1.98) | 3.2k / 1.3k | 1.98 | MIT | about 210 each |
| jev-client | ◐ events | ✗ | ✓ | 107 | 27 s | 6.5k / 1.8k | 1.96 | MIT or Apache-2.0 | 208 |
| jev | ✗ | ✗ | ✗ | 91 | 40 s | 1.4k / 0.8k | none | MIT or Apache-2.0 | 205 |
| typesafe-ai-sdk | ◐ events | ✗ | ◐ bodies at trace | 113 | 43 s | 3.9k / 1.2k | 1.88 | MIT | 129 |
| typesafeai-sdk-community | ◐ events | ✗ | ◐ bodies at debug | 110 | 47 s | 8.4k / 2.1k | 1.88 | MIT | 77 |
| typesafe-client | ✗ | ✗ | ◐ `Debug` only | 99 | 45 s | 4.5k / 1.6k | 1.87 | MIT or Apache-2.0 | 74 |

Downloads are crates.io totals on 2026-10-04, and judgment's twelve are two days old; the column says who people have tried, not what works. "Bodies at debug" means a `state`, which carries whatever the caller put in it, reaches the log at that level.

## Where judgment is behind

An honest comparison names the columns judgment loses, because they are the next release's list.

- **No concurrency limiter or pacer.** Three crates bound in-flight calls or pause every task on one 429. judgment retries with the server's wait and leaves the fan-out to the caller; a consumer replaying history in bulk hits the account limit first.
- **No provider presets.** `kunobi-decision` knows five hosts by name, with their default model and key variable; judgment knows a `base_url`. The presets are a convenience the compatible-servers page shows is now worth having.
- **Fewer distribution helpers.** `ranked()`, `top(n)` and `margin()` exist in four crates; judgment offers the probability of an option, the documented confidence formulas and the expected value, and leaves sorting to the caller.
- **Verification is strict on shape, not on arithmetic.** No crate checks that a distribution sums to one or that the choice is its argmax; judgment is the one that could, since it already walks every answer.
- **The model is not a per-call option across the trait.** It lives on the request, so a harness over `&dyn SystemOne` cannot vary it; the recording key excludes it by design, which is also why.
- **The slowest clean build of the set**, by a few seconds, for the same reason as the others on reqwest 0.13: the TLS stack. The `http` feature is the remedy for a consumer that does not want it.

## What to take from the others

In order of value for a consumer, each with where it was seen:

1. **A shared pacer fed by `Retry-After`** (`typesafeai-sdk-community`, `jev-client`): one "do not send before" instant that every in-flight call honours, so a 429 pauses the pool rather than the one call that saw it. Small, and the first thing bulk replay needs.
2. **Provider presets as data** (`kunobi-decision`): host, default model, key variable, and where the response needs unwrapping. Together with limits as data, it makes the compatible-servers table executable.
3. **`ranked()`, `top(n)`, `margin()`** (`kunobi-decision`, `typesafe-client`, the community SDK): pure, cheap, and what every policy re-derives.
4. **Arithmetic in `verify`**: a distribution that does not sum to about one, a choice that is not the argmax. judgment is the only crate positioned to add it without a new pass.
5. **The merge order of headers** (`typesafe-sdk-*`): caller defaults, then per-call, then the client's own written last. judgment refuses reserved headers instead, which is stricter and noisier; the order rule is friendlier for the same guarantee.

## Evidence

Each crate's source is the crates.io package at the version named, unpacked and read on 2026-10-04; pointers are file and line in that package.

- **kunobi-decision 0.3.0**: typed key `src/questions.rs:332-389`; limits `src/questions.rs:427-462`; bare `f64` `src/answers.rs:18-80`; no whole-response check `src/answers.rs:281-291`; retries and 10 s budget `src/retry.rs:33,120-135`, `src/client/transport.rs:285-325`; semaphore `src/client/builder.rs:154-157`; fake fit checks `src/testing.rs:192-293`; span `src/client/transport.rs:61-71`; providers `src/client/provider.rs:11-80`; package ships no tests or examples (`Cargo.toml.orig` `include`).
- **typesafe-sdk 0.2.0**: one-level Score accepted `src/question.rs:160,397`; `extra_body` over `state` `src/wire.rs:127-139`; unknown kind dropped `src/answer.rs:572-576`; `Debug` with the key `src/config.rs:10-12`, `src/client.rs:20-25`; redirects re-send the body `tests/contract.rs:613-648`; uncapped `Retry-After` `src/retry.rs:98-104`; global log level `src/logging.rs:9-22`.
- **typesafeai-sdk 0.4.1**: `HashMap` questions and options `src/types.rs:32`, `src/questions.rs:100`; `usage` required `src/types.rs:197`; unit `Unknown` `src/types.rs:135-136`; retried statuses `src/client.rs:925-955`; symmetric jitter `src/retry.rs:127-136`; 1 MiB cap `src/client.rs:52,1005-1026`; no `tracing` or `log` (grep).
- **typesafe-sdk-* 0.6.2** (douglance workspace): by-name access `typesafe-sdk-answers/src/response.rs:152-172`; two checks only `typesafe-sdk-client/src/validate.rs:248-270`; unknown kind fails `typesafe-sdk-answers/src/answer.rs:38-47`; `usage` required `response.rs:146`; clock jitter `typesafe-sdk-client/src/attempt.rs:152-161`; bodies logged `send.rs:203-213`; header merge order `headers.rs:231-289`; `rust-version = "1.98"` in every manifest.
- **jev-client 0.2.0**: opt-in validation `src/validate/mod.rs:50-57` and no call from `src/http.rs`; `Answer::Unknown` `src/answer.rs:39-41`; errors `src/error.rs:19-123`; retries `src/retry.rs:58-116`; 32 MiB cap `src/http.rs:29,466-482`; throttle `src/throttle.rs:363-459`; zeroised key `src/secret.rs:26-94`; no features in `Cargo.toml`.
- **jev 0.1.2**: strings-only questions `src/lib.rs:154-179`; `BTreeMap` options `src/lib.rs:212-220`; string legend `src/lib.rs:272`; retry 429 and 529 only `src/lib.rs:1042-1048`; `Debug` with the key `src/lib.rs:637-644`; no request id, no `User-Agent` (grep).
- **typesafe-ai-sdk 0.5.0**: `Rubric` derive `src/rubric.rs:76-90`; no response verification `src/response.rs:450-484`; cassettes `src/cassette.rs:40-81`; `extra_body` over standard fields `src/client.rs:555-575`; uncapped `Retry-After` `src/error.rs:400-440`; events only `src/client.rs:401-478`.
- **typesafeai-sdk-community 0.5.0**: `BTreeMap` order `src/question.rs:115`, `src/client.rs:388`; absent `answers` decodes `src/transport.rs:244-296`; `usage` required (test near `tests/integration.rs:630`); pacer `src/batch.rs:50-118`; eval `src/eval.rs`; in-order cassettes `src/testing.rs:530-680`; mock serves bytes unchecked `src/testing.rs:385-480`.
- **typesafe-client 0.1.0**: `QuestionKey<A>` `src/request.rs:464-478`, `src/answer.rs:301-312`; `verify` `src/answer.rs:317-360` run by the client `src/client/mod.rs:171` and the fake `src/fake.rs:195`; limits `src/request.rs:503-530`; unknown kind fails `src/answer.rs:7-9`; `usage` required `src/answer.rs:210-243`; contract test `tests/openapi_contract.rs:81-207`; no `tracing` (grep).
- **judgment 0.3.0**: handles `src/question.rs:265-280`, `src/answer.rs:628-635`; newtypes `src/answer.rs:198-289`; `verify` `src/answer.rs:654-792` run at `src/client.rs:856`, `src/backend.rs:375,440,534`; limits `src/question.rs:432-501`; retries `src/http.rs:262-396`; body cap `src/http.rs:565-579`; no redirects `src/client.rs:698`; key hygiene `src/client.rs:1162-1196`; fake `src/backend.rs:178-386`; recordings `src/backend.rs:408-538`; contract test `tests/contract.rs`; metrics `src/eval/metrics.rs`; spans `src/client.rs:807-815`; observer `src/observer.rs:27-70`; no limiter (grep).
