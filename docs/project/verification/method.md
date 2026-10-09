---
title: How the crate is checked
description: The three checks that stand between the crate's mocks and a real server, the ignored live tests, the benchmark replay and the contract test against the vendored OpenAPI document, what each one can and cannot establish, and how to run them against the hosted API, Laya, Ollama or Clef on Workers AI.
status: current
last_reviewed: 2026-10-06
tags: [judgment, verification, contract, openapi, live-tests]
---

# How the crate is checked

The unit and integration tests never leave the process: wiremock for the client, `Fake` and recordings for everything else. That keeps the gate fast and free, and it means the gate cannot tell whether a server speaks the wire the way the mocks assume. Three further checks can, each answering a different question. The records of what they found are the other pages under [Verification](hosted-typesafe.md).

## Does a real server speak the wire this way?

`tests/live.rs` holds fifteen `#[ignore]` tests that run the three primitives, a structured Score level (and print how the server echoes it), the model list, an unknown model name, an unknown extra body field, the server's own limits, the stability of repeated calls, the confidence formulas, the token budget, a bearer check and a record-then-replay against whatever `JUDGMENT_LIVE_BASE_URL` points at. `cargo test` skips them. Five tasks run them:

| Task | Against | Needs |
|---|---|---|
| `mise run live:typesafe` | the hosted API, `jev-latest` by default | `TYPESAFE_API_KEY` in `.env`; spends a handful of model calls |
| `mise run live:laya` | `laya-serve` on `LAYA_URL` (default `http://127.0.0.1:8000`) | a running server; [the Laya record](laya-typed-decisions.md) says how to start one |
| `mise run live:ollama` | Ollama's `/v1/systemone` on `OLLAMA_URL` (default `http://127.0.0.1:11434`) with `tev1:0.8b`, or `OLLAMA_MODEL=clef` or `clef-flash` for Cloudflare's models | Ollama 0.35 or later and `ollama pull tev1:0.8b`; that model runs on a CPU, Clef wants a GPU (0.35.1 for `ollama pull clef`) |
| `mise run live:pplx` | Perplexity's [pplx-decider-v1.1-27b](https://huggingface.co/perplexity-ai/pplx-decider-v1.1-27b) through the `autojev-serve` its checkpoint ships, on `PPLX_URL` (default `http://127.0.0.1:8010`), model `autojev-qwen3.8-27b` by default | a CUDA GPU with about 49 GiB for the weights; the snapshot's `source/` installed with its `requirements.txt`, then `AUTOJEV_CHECKPOINT=<snapshot> PORT=8010 autojev-serve`. `PPLX_API_KEY` when the server sets `AUTOJEV_API_KEY`, which also runs the bearer test |
| `mise run live:clef` | Cloudflare's Clef on Workers AI through `tools/systemone/serve.py`, `clef` by default and `CLEF_MODEL=clef-flash` for the 9B | `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN` in `.env`; spends a handful of model calls |

A run that passes, or that finds a departure, becomes a page under `docs/project/verification/` naming the server, its release and the date. None of these runs in CI: the gate stays offline, and a key in CI would be a secret the repository does not need.

## Does it hold at scale?

`examples/typed_decisions.rs` replays the [typed-decisions](https://huggingface.co/datasets/LocalLLaMA/typed-decisions) benchmark (400 cases, 2,000 typed decisions) through the crate and scores it with `judgment::eval`, live or from recordings. `examples/typed-decisions/` holds a 40-case sample and `tools/typed-decisions/export.py` exports the full split. The example is a compatibility test at scale before it is an evaluation: every combination of primitive and criteria shape the benchmark uses goes through the builder, the client and the decoder. It was run against Laya's `typed-decisions` checkpoint, and the decoding bug it caught is in [the Laya record](laya-typed-decisions.md#the-bug-the-run-caught). `tools/systemone/serve.py` serves the wire over open-weight models that do not serve it at the standard path themselves: Laya in this process, for when `laya-serve` is not wanted and as the one Laya server that answers `GET /v1/models`, and Cloudflare's Clef on Workers AI, which takes the body at one URL per model inside Cloudflare's envelope.

## Does the crate match the published contract?

TypeSafe publishes an OpenAPI document for the System One API at <https://api.typesafe.ai/openapi.json>. A copy is vendored at `tests/fixtures/typesafe-openapi.json` (OpenAPI 3.1.0, API version 0.2.0), and `tests/contract.rs` validates against it as JSON Schema 2020-12, offline and in every `cargo test`:

- every request shape the builders produce (each primitive, string, object, array and null instructions, one-sided and structured Noul criteria, undescribed options, 255 options, 2 and 10 levels of every level shape), sent through each client entry point, with the method, path, content type and bearer scheme the document names, and also against a closed copy of the request components, so a renamed or misspelt field fails even where the published schema, which closes no object, would take it as an extra key;
- every response a `Fake` builds, and all 40 committed recordings under `examples/recordings/typed_decisions`, as committed and after a decode and re-serialise;
- the document's own examples, which decode through `Response` and read through typed handles, and its model list and validation error, through `list_models` and `Error::InvalidRequest`.

Where the crate and the schema disagree the test pins the difference, each at its own path, so a refreshed document that closes one fails loudly:

- **The crate sends what the schema refuses**: a null or numeric state, numeric instructions, a boolean Noul criterion, a numeric Score level, an empty question set. The builders take any JSON value there and the client any `Serialize` state, so such a request reaches the server, which is expected to refuse it with a 422.
- **The builder refuses what the schema allows**: 1 or 256 options, 1 or 11 levels, an empty question id, an empty option key. It follows the HTTP API reference page, which is stricter than the schema. The hosted API was probed past those limits on 2026-10-03: it refuses 256 options and 11 levels with a 400, refuses an empty question id with a 400, and answers one option or one level with probability 1 and accepts an empty option key, so the upper bounds are the server's and the lower ones this crate's alone (the rustdoc of `question` says why).
- **The crate decodes differently**: it refuses a probability or confidence outside `[0, 1]` and a negative token count, which the schema types as bare numbers, because a value no threshold can use is better an error; and it accepts a response without `usage`, with `usage` or a token count null, with no answers, or with a legend entry that is null or a scalar, which the schema refuses, because decoding is tolerant and `Response::verify` is what holds a response to its questions. A `Fake` asked nothing answers `answers: {}`, which the schema refuses.

An answer of a kind the document does not name decodes as `Answer::Unknown` and is refused by the schema. That case is not run through the validator: one test compares the document's discriminator mappings with the crate's kinds, so a document that adds a kind fails there.

An application that validates its own traffic against the same document reads it as `judgment::contract::OPENAPI_DOCUMENT` with the `openapi` feature, so its checks and the crate's cannot drift apart.

### Refreshing the copy

The copy is refreshed only through `tests/openapi_drift.rs`, an ignored test that needs the network and no key. It compares the copy with the live document and names what differs; with `JUDGMENT_OPENAPI_WRITE` set it rewrites the copy canonically instead:

```sh
cargo test --test openapi_drift -- --ignored          # stale?
JUDGMENT_OPENAPI_WRITE=1 cargo test --test openapi_drift -- --ignored
cargo test --test contract                            # review the refresh
```

A refreshed document is a contract change: `tests/contract.rs`, the CHANGELOG and the README's guarantees move together, and the committed recordings must still replay.

## What each check cannot establish

The live tests show that one server, on one day, accepted and answered what the crate sends; they say nothing about accuracy on your data. The benchmark shows the decoder and the metrics on real answers; it is one checkpoint on the task it was tuned for. The contract test checks the published schema, not a live account: a server can accept or refuse what its schema does not say, which is what the live tests are for. What all three leave open is calibration on your own questions, which only labelled history replayed through `judgment::eval` can measure ([Testing without the model](../../guides/record-replay-and-test.md)).
