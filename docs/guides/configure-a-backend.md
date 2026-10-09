---
title: Configure a backend
description: How to point the jud command, the examples and the crate's client at the hosted TypeSafe API, at Jev through OpenRouter, at Ollama on your machine, at laya-serve, at Clef on Workers AI, at any other server that speaks the wire, or at a model on Google Cloud; and what each local server does differently from the hosted API.
status: current
last_reviewed: 2026-10-09
tags: [judgment, jud, configuration, typesafe, openrouter, ollama, laya, clef, cloudflare, google-cloud, how-to]
---

# Configure a backend

**You will** have `jud`, the examples or your own code talking to the server of your choice. **Prerequisites:** [Install](../start/install.md). The settings, their precedence and their defaults are in [Configuration](../reference/configuration.md); this page is the procedure per server.

Three settings name a backend: the base URL, the model, and a key. The crate is a client to one wire, `POST /v1/systemone`; any server that speaks it is a base URL and nothing more. There is no provider switch.

| Entry point | Base URL | Model | Key |
|---|---|---|---|
| `jud` | `TYPESAFE_BASE_URL`, or `base_url` in the file | `model` in the file (default `jev-latest`) | `TYPESAFE_API_KEY`, or `api_key` in the file |
| the examples (`--live`) | `TYPESAFE_BASE_URL` | `TYPESAFE_MODEL` | `TYPESAFE_API_KEY` |
| `Client` | `ClientBuilder::base_url` | the `model` argument | `TYPESAFE_API_KEY`, or `ClientBuilder::api_key` |

The file is `~/.config/jud/config.yaml` (`$XDG_CONFIG_HOME` honoured). `jud config` prints what a run would use and where each value came from.

## The hosted TypeSafe API

The default. Export the key and nothing else:

```sh
export TYPESAFE_API_KEY=...        # from https://docs.typesafe.ai
jud config                         # "api_key": "environment", base URL and model from the defaults
```

A key in the file is accepted for a machine where the environment is awkward to set, with the usual caution about a secret on disk. A run with no key is refused before any call, with status 2, naming the variable and the file. In Rust, `Client::from_env()?` reads the same variable.

## Jev through OpenRouter

OpenRouter serves Jev behind TypeSafe's own wire, billed to an OpenRouter account: its System One API is `POST https://openrouter.ai/api/v1/systemone`, so the base URL is `https://openrouter.ai/api` and the key is an [OpenRouter key](https://openrouter.ai/settings/keys).

```sh
export TYPESAFE_BASE_URL=https://openrouter.ai/api
export TYPESAFE_API_KEY=sk-or-...  # an OpenRouter key, not a TypeSafe one
jud rubric.jud < state.json
```

The client appends `v1/systemone` under the base URL's path, with or without a trailing slash, as the official SDKs do. Before that was fixed it dropped the last path segment, so `https://openrouter.ai/api` reached `https://openrouter.ai/v1/systemone`; a gateway under a path is the case it got wrong.

What differs from the hosted API, from [OpenRouter's guide for TypeSafe's SDKs](https://openrouter.ai/docs/guides/community/typesafe-sdk):

- **Model names.** `jev-latest` is routed as `~typesafe/jev-latest` and `jev-1.13` as `typesafe/jev-1.13`; the response's `model` is OpenRouter's dated id, `typesafe/jev-1.13-20260917` say. Log it, as with any server: thresholds are tuned per version.
- **Cost.** Every response carries `usage.cost` in US dollars, read into `Usage::cost`, which the observer's `on_usage` sees.
- **Request id.** There is no `x-typesafe-request-id`; the body's `id` (`gen-dec-…`, OpenRouter's generation id) becomes `Response::request_id`, and stays in `Response::extra` with `provider`.
- **Credit.** A balance that runs out is a 402, `Error::PaymentRequired`, not retried.
- **The model list.** `GET /api/v1/models` is OpenRouter's own catalogue, not TypeSafe's list, so `Client::list_models` fails to decode there. Nothing else in the crate calls it.

OpenRouter also serves the same body at `POST /api/alpha/decisions`, its Decisions API, which it marks alpha; the System One path is the one TypeSafe's SDKs, and this client, are pointed at. `mise run live:openrouter` runs the live tests against it with `OPENROUTER_API_KEY` from `.env`; no run is recorded yet ([Compatible servers and models](../project/research/compatible-servers-and-models.md)).

## Ollama on your machine

Open-weight decision models speak the wire, and Ollama 0.35 or later serves them; the first example is [tev1](https://ollama.com/library/tev1) at 0.8B parameters, 811 MB, which runs on a CPU.

```sh
ollama serve &            # unless the installer registered a service
ollama pull tev1:0.8b
```

Ollama ignores the bearer, but the client refuses to build without one, because for the hosted API a missing key is the most common misconfiguration. Set `TYPESAFE_API_KEY` to any non-blank word. [`examples/jud/config-tev1.yaml`](../../examples/jud/config-tev1.yaml) is the configuration file for this setup:

<!-- file: examples/jud/config-tev1.yaml -->
```yaml
# A `jud` configuration for tev1 on a local Ollama (docs/guides/configure-a-backend.md).
# Copy it to ~/.config/jud/config.yaml (or $XDG_CONFIG_HOME/jud/config.yaml):
#
#   mkdir -p ~/.config/jud && cp examples/jud/config-tev1.yaml ~/.config/jud/config.yaml
#   ollama pull tev1:0.8b
#   echo '{"message": "My payouts have been failing for 3 days."}' | jud examples/jud/triage.jud
#
# Every field is optional; the environment (TYPESAFE_BASE_URL, TYPESAFE_API_KEY)
# overrides the file, and `jud config` prints what a run will use.

# Ollama's default address; `/v1/systemone` is appended by the client.
base_url: http://127.0.0.1:11434

# The name `ollama list` shows. There is no environment variable for the
# model, so the file is where a local model is named. `clef-flash` is the
# step up on a machine that holds it.
model: tev1:0.8b

# Ollama ignores the bearer, but the client refuses to build without one.
# Any word will do; it is not a secret, so the file may hold it.
api_key: unused

# A model on a CPU answers in seconds, the first call of a session in more
# (it loads the model). The default is 30.
timeout_secs: 60
```

```sh
mkdir -p ~/.config/jud && cp examples/jud/config-tev1.yaml ~/.config/jud/config.yaml
echo '{"message": "My payouts have been failing for 3 days."}' | jud examples/jud/triage.jud
```

Without the `model:` line the same command fails with Ollama's 404, `model "jev-latest" not found, try pulling it first`. The model loads on the first request and stays loaded for five minutes after the last one, so the first call of a session pays the load.

For the examples: `TYPESAFE_BASE_URL=http://127.0.0.1:11434 TYPESAFE_API_KEY=unused TYPESAFE_MODEL=tev1:0.8b cargo run --example intent_routing -- --live`. In Rust:

```rust no_run
use std::time::Duration;

use judgment::Client;

fn main() -> judgment::Result<()> {
    let client = Client::builder()
        .base_url("http://127.0.0.1:11434")
        .api_key("unused") // Ollama ignores the bearer; the builder refuses a blank one
        .model("tev1:0.8b") // what `Client::system_one` sends
        .timeout(Duration::from_secs(60)) // a model on a CPU answers in seconds
        .build()?;
    let _ = client;
    Ok(())
}
```

### What Ollama does differently

Observed on 2026-10-07 against Ollama 0.40.0 with `tev1:0.8b`; the limits are Ollama's, so they hold for every decision model it serves.

| | Hosted API (`jev-latest`) | Ollama 0.40.0 | What the crate does |
|---|---|---|---|
| Options per Choice, levels per Score | up to 255 options, 2 to 10 levels | 2 to 26 (`criteria must contain 2–26 candidates`, a 400) | The builder's cap is 255, so 27 or more options are refused by Ollama: `Error::InvalidRequest`, not retried. Cap the list in code. |
| Questions per request | the hosted budget | 1 to 64 (a 400) | One request per 64 questions. |
| A Score level that is a JSON object | accepted | refused (a 400) | Describe each level in one string. |
| `GET /v1/models` | TypeSafe's list | the OpenAI-shaped list | `Client::list_models` cannot decode it (`Error::Decode`). Use `ollama list`. |
| A model that is not there | a 400 naming the model | a 404, `model "x" not found` | `Error::Http` with the status and Ollama's message. |
| The bearer | checked; a wrong key is a 401 | ignored | Send any non-blank key. |
| Request id | on every response | none | `Response::request_id` is `None`. |
| Latency | hundreds of milliseconds | one to two seconds for `tev1:0.8b` on an 8-core CPU, after the load | Raise the timeout: 60 s in the file above; the crate's default is 10 s, the command's 30 s. |
| `confidence` | TypeSafe's spread | the model's own formula | A threshold tuned on one model is meaningless on another. [Tune thresholds](tune-thresholds.md) again. |

A rubric's policy tuned on the hosted model sends the message above to the technical desk on `tev1`; Jev sends it to billing. That is the difference between models made visible, and the reason the last row exists.

### A larger model: Clef-flash

[Clef-flash](https://ollama.com/library/clef-flash) is Cloudflare's 9B decision model, Apache 2.0; `ollama pull clef-flash` (10 GB) and `model: clef-flash`. It loads 9 GB of weights plus its context, on the GPU when there is one and otherwise in RAM. A machine with 16 GB holds it; one with 13 GB does not, and the failure is the kernel killing the loader: the crate sees the connection drop, `transport error after 3 attempts`. A transport error from a local server that answered `GET /api/version` a moment earlier means the model did not fit.

## laya-serve

[Laya](https://huggingface.co/convaiinnovations/laya)'s small encoders serve the wire through `laya-serve` (`pip install "laya[serve]"`), on `http://127.0.0.1:8000` by default; the model is `typed-decisions`. [Against Laya typed-decisions](../project/verification/laya-typed-decisions.md) records a full run and how to start the server; `mise run live:laya` runs the crate's live tests against it. Laya sends no request id, and the record lists what else differs.

## Clef on Workers AI

Cloudflare serves Clef and Clef-flash on Workers AI at one exact URL per model, with no `/v1/systemone` path, inside its own envelope. `tools/systemone/serve.py cloudflare` is a shim in front of it that serves the wire on a local port, maps the path, unwraps the envelope and forwards `Retry-After` and the `cf-ray` id as the request id. It reads `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN` from the environment, never from the client's bearer:

```sh
python3 tools/systemone/serve.py cloudflare --port 8099 &
TYPESAFE_BASE_URL=http://127.0.0.1:8099 TYPESAFE_API_KEY=unused jud rubric.jud < state.json
```

with `model: clef` or `clef-flash` in the file. `mise run live:clef` runs the live tests this way.

## Any other server

A server that serves `POST /v1/systemone` is a base URL: `TYPESAFE_BASE_URL=https://...`, a key if it checks one and any word if it does not, and the model name it knows. [Compatible servers and models](../project/research/compatible-servers-and-models.md) lists what has been run and what each does differently; the table above is the shape of what to check. A server that serves the body at another path needs a shim like the one above.

## Google Cloud

A compatible server on Cloud Run (Ollama with a decision model in a container, say) is a base URL like any other, with the service's ID token in `TYPESAFE_API_KEY` for the hour it is valid. A model on a Vertex AI endpoint is reached through `rawPredict`, whose path is not the wire's, so today it takes a shim in front; [System One models on Google Cloud](../project/research/system-one-on-google-cloud.md) studies each path, how request labels go in a header with the client as it is, and the changes to the crate that would make a Vertex endpoint a base URL and nothing more.

## Inside the container

The same three settings, passed as `-e` variables or a mounted file; the host's Ollama needs `--network host`. [Run in a container](run-in-a-container.md).

## Next

- [Record, replay and test](record-replay-and-test.md): keep a local model's answers for your tests.
- [Tune thresholds](tune-thresholds.md) before a policy ships against a new model.
