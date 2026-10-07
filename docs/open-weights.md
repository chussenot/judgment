---
title: Open-weight models without an account
description: How to use the crate, its examples and the jud binary with no TypeSafe account, against an open-weight decision model that speaks the same wire; the first example is tev1 on Ollama 0.40.0, a 0.8B model that runs on a CPU, with Clef-flash as the step up for a machine that holds it, how each entry point is pointed at a local server, what Ollama does differently from the hosted API (its limits, its model list, its errors, the meaning of confidence) and what was checked on 2026-10-07.
status: current
last_reviewed: 2026-10-07
tags: [judgment, ollama, tev1, clef, open-weights, compatibility, getting-started]
---

# Open-weight models without an account

The crate is a client to one wire, `POST /v1/systemone`, and TypeSafe's hosted API is the default server, not the only one. Several open-weight decision models now speak the same wire, and Ollama serves them on a laptop. Nothing in the crate changes between servers: the typed questions, the verified answers, the `Fake`, the recordings, the `.jud` reader and the `jud` binary all work against a local model as they work against the hosted API. What changes is the model behind the wire: its limits, how fast it answers, how its probabilities are calibrated. This page is the walk-through for someone with no key; [Compatible servers and models](research/compatible-servers-and-models.md) is the survey behind it.

The first example is [tev1](https://ollama.com/library/tev1) at 0.8B parameters on Ollama: 811 MB to download, runs on a CPU, answers a request in a second or two. It is the model `mise run live:ollama` defaults to and the one every command on this page was run against on 2026-10-07, with Ollama 0.40.0. Its answers are a 0.8B model's, enough to see the crate work and to write tests against, not a measure of what a larger model would say; [Clef-flash](#a-larger-model-clef-flash) is the step up once the machine allows it.

## Ollama and the model

Install Ollama 0.40.0 or later: the [installer](https://ollama.com/download), or `mise use -g ollama@0.40.0` for the same binary through mise. This repository pins `ollama = "0.40.0"` in `mise.toml`, so `mise install` in a checkout provides it too. Ollama's `/v1/systemone` arrived in 0.35 and the decision models in 0.35.1; 0.40.0 is the release this page was checked with, and the one that runs them on MLX on Apple silicon.

```sh
ollama serve &            # unless the installer registered a service, as it does on Linux
ollama pull tev1:0.8b     # 811 MB
ollama list               # tev1:0.8b
```

The model loads on the first request and stays loaded for five minutes after the last one (Ollama's `OLLAMA_KEEP_ALIVE`), so the first call of a session pays the load, a few seconds, and the rest do not.

## Pointing the crate at it

Three settings name the server: the base URL (`http://127.0.0.1:11434`, Ollama's default), the model (`tev1:0.8b`, the name `ollama list` shows), and a key. The key is the one surprise. Ollama ignores the bearer, but the client refuses to build without one, because for the hosted API a missing key is the most common misconfiguration and refusing it before any call is the right default ([`ClientBuilder::build`](tour.md)). Set `TYPESAFE_API_KEY` to any non-blank word; `unused` is what the live tests send.

### The `jud` binary

[The jud command line](cli.md) evaluates JSON input against a `.jud` rubric. It reads the base URL and the key from the environment and the model from its configuration file; there is no environment variable for the model, so the file is the way to name one. `examples/jud/config-tev1.yaml` is a ready-made file for this page's setup, the four fields commented:

```yaml
base_url: http://127.0.0.1:11434   # Ollama's default address
model: tev1:0.8b                   # the name `ollama list` shows
api_key: unused                    # Ollama ignores it; the client refuses a blank one; not a secret
timeout_secs: 60                   # the first call of a session loads the model
```

```sh
mkdir -p ~/.config/jud && cp examples/jud/config-tev1.yaml ~/.config/jud/config.yaml
echo '{"message": "My payouts have been failing for 3 days."}' | jud examples/jud/triage.jud
```

What came back on 2026-10-07, in 8 s including the model's load:

```json
{
  "actionable": { "verdict": "yes", "probability": 0.966 },
  "desk": { "verdict": "option", "key": "technical", "confidence": 0.644 },
  "tone": { "verdict": "level", "index": 1, "label": "annoyed", "value": 1.211, "confidence": 0.357 }
}
```

The rubric's policy, tuned on cases answered by the hosted model, sends the message to the technical desk; Jev sends it to billing. That is the difference between models made visible, and the reason the last row of the table below exists. `jud config` prints what a run will use and where each value came from, with `config_file_present: true` and every `_from` reading `config_file` once the file is in place. Without the `model:` line the same command fails with Ollama's 404, `model "jev-latest" not found, try pulling it first`.

### The examples

The examples under `examples/` replay committed recordings by default and take `--live` to ask a server. They read `TYPESAFE_BASE_URL` for the server and `TYPESAFE_MODEL` for the model:

```sh
TYPESAFE_BASE_URL=http://127.0.0.1:11434 TYPESAFE_API_KEY=unused TYPESAFE_MODEL=tev1:0.8b \
  cargo run --example intent_routing -- --live
```

The seven messages were routed in a run of 41 s including the build, one request each. `--record` instead of `--live` writes the answers over the example's recordings, which is how a project keeps a local model's answers for its tests ([Testing without the model](testing.md)).

### In code

```rust
use std::time::Duration;
use judgment::Client;

let client = Client::builder()
    .base_url("http://127.0.0.1:11434")
    .api_key("unused")          // Ollama ignores the bearer; the builder refuses a blank one
    .model("tev1:0.8b")         // what `Client::system_one` sends
    .timeout(Duration::from_secs(60))   // a model on a CPU answers in seconds, not milliseconds
    .build()?;
```

Everything above the builder is unchanged: `client.answer(&state, "tev1:0.8b", &questions)` returns a `Response` verified against the questions, read through the same typed handles as a hosted answer. A `Client` built this way satisfies the `SystemOne` trait, so it goes wherever a `Fake` or a `Replay` went.

## What Ollama does differently

The wire is the same; the server and the model behind it are not the hosted API's, and the differences a consumer meets are these. Each was observed on 2026-10-07 against Ollama 0.40.0 with `tev1:0.8b`; the limits are Ollama's, so they hold for every decision model it serves.

| | Hosted API (`jev-latest`) | Ollama 0.40.0 | What the crate does |
|---|---|---|---|
| Options per Choice, levels per Score | up to 255 options, 2 to 10 levels | 2 to 26 (`criteria must contain 2–26 candidates`, a 400) | The builder's own cap is 255, so a question with 27 or more options is refused by Ollama, not by the crate: [`Error::InvalidRequest`](tour.md), not retried. Cap the list in code before the wire. |
| Questions per request | the hosted budget | 1 to 64 (`questions must contain 1–64 fields`, a 400) | The same: one request per 64 questions. |
| A Score level that is a JSON object or array | accepted and echoed verbatim | refused (`score criteria must be an array of descriptions`, a 400) | The crate accepts structured levels because the hosted API does. Against Ollama, describe each level in one string. |
| `GET /v1/models` | TypeSafe's list | the OpenAI-shaped list (`{"object": "list", "data": [{"id": "tev1:0.8b", ...}]}`) | `Client::list_models` cannot decode it ([`Error::Decode`](tour.md), `missing field models`). Use `ollama list`; nothing else in the crate depends on the call. |
| A model that is not there | a 400 naming the model | a 404, `model "x" not found, try pulling it first` | [`Error::Http`](tour.md) with the status and Ollama's body, which is `{"error": "<text>"}` rather than the wire's `{"error": {"message", "type"}}`; the message is still shown. |
| The bearer | checked; a wrong key is a 401 | ignored | Send any non-blank key. A test that expects a 401 from a wrong key (`tests/live.rs` has one) is skipped against Ollama. |
| Request id | `x-typesafe-request-id` on every response | none | `Response::request_id` is `None`; it is optional everywhere. |
| `Response::model` | the alias resolved, `jev-1.13.0` | the name sent, `tev1:0.8b` | Record it beside every decision either way. |
| Latency | hundreds of milliseconds | one to two seconds for `tev1:0.8b` on an 8-core CPU, after the load | Raise the per-attempt timeout above the 10 s default; the `jud` binary's default is 30 s. |
| `confidence` | TypeSafe's spread | the model's own formula | A threshold tuned on one model is meaningless on another. Refit it on labelled cases with `judgment::eval` or a `.jud` Cases document ([What cases are](jud/cases.md)) before a policy ships against a new model. |

Score is the weakest primitive on every open model that publishes per-primitive numbers; the research page says why, and a Score that can be written as a Choice over the same described levels gets a better answer from a small model and a `Confidence` the policy can use.

## A larger model: Clef-flash

[Clef-flash](https://ollama.com/library/clef-flash) is Cloudflare's 9B decision model, Apache 2.0, fine-tuned from Qwen3.5-9B; it sits within a point of Jev on the Decision Index and answers in one pass. On Ollama it is the same three settings with `clef-flash` for the model:

```sh
ollama pull clef-flash    # 10 GB, Q8_0, context 256K; `ollama show clef-flash` says "requires 0.35.1"
```

What it needs from the machine is the reason it is not this page's first example. Clef-flash loads 9 GB of weights plus its context, on the GPU when there is one and otherwise in RAM. A machine with 16 GB of RAM and little else running holds it; a 13 GB laptop does not, and the failure is not one the model reports. The Linux kernel kills the loader (`journalctl -u ollama` shows `oom-kill`, `ollama ps` shows nothing), the service restarts, and the crate sees the connection drop: `transport error after 3 attempts` ([`Error::Transport`](tour.md)), since the client retries a request that got no response. A transport error from a local server that answered `GET /api/version` a moment earlier means the model did not fit. `OLLAMA_MODEL=clef-flash mise run live:ollama` is the run that would verify it on a machine that holds it, and a page under `docs/verification/` is where the result goes.

## Other servers

Ollama is the first example because it is the runtime most people already have. The same three settings reach the others the research page lists: `laya-serve` for Laya's small encoders ([Against Laya typed-decisions](verification/laya-typed-decisions.md) is a full run), Decider and OpenDecider for their families, and Cloudflare's Workers AI for the same Clef weights hosted, which needs `tools/systemone/serve.py cloudflare` in front because Workers AI serves the body at its own URL inside its own envelope. Each one's limits and confidence semantics are its own, and the table above is the shape of what to check.

## What was checked

On 2026-10-07, on an 8-core laptop with 13 GB of RAM and no usable GPU, Ollama 0.40.0 as a systemd service, `tev1:0.8b` pulled:

- `jud examples/jud/triage.jud` through the configuration file above answered in 8 s including the model's load, with the verdicts shown; `cargo run --example intent_routing -- --live` routed its seven messages in a run of 41 s including the build.
- `mise run live:ollama`, the crate's ignored live tests (`tests/live.rs`), finished in 13 s: 11 passed and 4 recorded departures from the hosted wire, every one in the table above. The three primitives round-trip through their typed handles; the response carries no undocumented field and no request id; a Choice with 256 or 1 options and a Score with 1 level are refused as 2 to 26 candidates, and a Score with 11 levels is answered. The departures: a model name the server does not know is a 404 where the hosted API answers; a Score whose level is a JSON object is a 400; the model list decodes as neither TypeSafe's shape nor an absence. The limits and error shapes in the table came from these tests and from direct requests to `/v1/systemone`.
- The `jud` binary refuses to run without a key (`no API key: set TYPESAFE_API_KEY, or api_key in ~/.config/jud/config.yaml`, exit 2) and runs with any word in it.
- `ollama pull clef-flash` succeeded; loading it was killed by the kernel at a 6.7 GB peak and the crate reported `transport error after 3 attempts`. Nothing on this page was verified against Clef-flash itself.
