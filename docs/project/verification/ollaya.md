---
title: Against Ollaya
description: How the judgment crate was run against Ollaya 0.17.0's TypeSafe-compatible API on a CPU-only container with the laya:en model, every status and body the live tests and probes observed, how each departs from the hosted TypeSafe API, and what the ollaya profile pins because of it.
status: experiment
last_reviewed: 2026-10-25
tags: [judgment, ollaya, laya, compatibility, self-hosted]
---

# Against Ollaya

[Ollaya](https://ollaya.dev) serves open decision models on your own machine behind an API it documents as wire-identical to TypeSafe's ([TypeSafe compatibility](https://ollaya.dev/docs/typesafe-compatibility)): `POST /v1/systemone`, its alias `POST /v1/decisions`, and `GET /v1/models`, on `http://127.0.0.1:11435` by default. `mise run live:ollaya` runs the live tests against it, under the `ollaya` profile.

The short answer: the wire holds for text, with a real run behind it. Every success body, every refusal and the model list conform to the vendored OpenAPI document, the confidence formulas hold, and repeated requests give the same numbers to the last digit. Ollaya departs from the hosted API in three ways a caller must know: every refusal is a 422 and a model it has not pulled a 404; each model's context bounds the options and the state far below TypeSafe's; and it has no vision, so a Box is refused but an image is ignored without a word.

## What was tested, and with what

| | |
|---|---|
| Server | Ollaya 0.17.0 (`linux-amd64`, released 2026-10-23), installed with its script pinned to the version (`OLLAYA_VERSION=0.17.0`, `OLLAYA_NO_SERVICE=1`) and started with `ollaya serve` |
| Model | `laya:en` (ModernBERT-large, released 2026-09-23), the model Ollaya names for a machine without a GPU; an 814 MB store |
| Machine | a Claude Code cloud container: 4 vCPUs, 15 GiB, no GPU, Ubuntu 24.04 (glibc 2.39; Ollaya needs 2.38). The archive is `.tar.zst`, so `zstd` had to be installed first |
| Reference | Ollaya's [API reference](https://ollaya.dev/docs/api) and [TypeSafe compatibility](https://ollaya.dev/docs/typesafe-compatibility) pages, read on 2026-10-25 |
| Date | 2026-10-25 |
| Profiles | `generic` first (18 of 20 passed; the two failures are the pins below), then `ollaya`: 21 of 21, the bearer test against a second instance on `127.0.0.1:11436` started with `OLLAYA_API_KEY`. The shared-images test does not apply (below) |
| Latency | the first request loads the model (21.5 s); after that a request with a Choice, a Noul and a Score took 0.65 to 0.71 s, median 0.68 s |

## What the server does

| Request | Ollaya, `laya:en` | The hosted API |
|---|---|---|
| The three primitives, a structured level, an option with no description, an unknown extra field | 200, conforming to `SystemOneResponse`; the extra field ignored | 200 |
| The same body to `/v1/decisions` | 200 | not served |
| `GET /v1/models` | 200, TypeSafe's `ModelMetadataList` of the models pulled to the machine (`laya:en`) | 200, the aliases |
| A model it has not pulled (`jev-latest`) | 404, `{"error": "model \"jev-latest:latest\" not found, try pulling it first", "code": "MODEL_NOT_FOUND"}` | 200 |
| 256 options, 11 levels | 422, `INVALID_REQUEST` with TypeSafe's `detail` (`at most 255`, `at most 10`) | 422 from its schema |
| 1 option, 1 level | 422, `INVALID_REQUEST` (`at least 2`) | 200, probability 1 |
| An empty question id | 200, answered | 400 |
| 125 options | 200 | 200 |
| 126 options | 422, `STATE_TRUNCATED`: the options leave the state no room | 200 |
| 130 or 200 options | 422, `TOO_MANY_OPTIONS`, `130 options do not fit the option budget of laya:en` | 200 |
| A state of 150 words (330 input tokens) | 200 | 200 |
| A state of 300 to 2,000 words | 422, `STATE_TRUNCATED`, `part of state was dropped to fit the context of laya:en` | 200 |
| A state of 162,923 tokens | 422, `INPUT_TOO_LONG`, issue `input_too_long`, `the limit is 65536` | 400, `max_tokens_exceeded` |
| A Box question | 422, `INVALID_REQUEST`: the tags it knows are `choice`, `score`, `noul` | 200 on Jev 1.14 |
| A Noul about an image, in `assets` with the images header, or in Ollama's shared `images` array | 200, and the same answer and the same 42 input tokens as without the image | 200, the image read |
| A wrong bearer, no `OLLAYA_API_KEY` set | 200: no key is checked | 401 |
| A wrong bearer, `OLLAYA_API_KEY` set | 401, `UNAUTHORIZED` | 401 |
| Three identical requests | the same probabilities, confidences and score to the fourth decimal | a spread in the hundredths |

Refusals carry Ollaya's own envelope, `{"error", "code", "detail"}`: a string message, a machine-readable code, and for a 422 TypeSafe's `detail` list of located issues. The client reads `detail` as it does the hosted API's, so `Error::InvalidRequest::issues` holds the issues and each one's `kind` (`too_long`, `state_truncated`, `input_too_long`); the `code` is not kept. A 404 is an `Error::Http`. Every response carries `x-typesafe-request-id`.

## What the profile pins, and why

- **422 for every limits probe**, and the message naming the bound, as on Liquid.
- **A model it has not pulled is a 404 with `MODEL_NOT_FOUND`**, where the hosted API answers `jev-latest`. A client built for the hosted API fails against Ollaya until it names an Ollaya model; Ollaya's own page says to set `TYPESAFE_DEFAULT_MODEL`.
- **An empty question id is answered**, as autojev-serve does.
- **An over-budget state is a 422 whose issue says `input_too_long`.** `Error::is_request_too_large` reads a 413 or `kind == "max_tokens_exceeded"`, so it is false here, as on Liquid; the test asserts the issue instead.
- **Each model's context bounds the options and the state** (`the_model_s_context_bounds_the_options_and_the_state`): 200 options are `TOO_MANY_OPTIONS` and a 2,000-word state is `STATE_TRUNCATED`, both 422. The documented figures for `laya:en`, a 512-token context and about 125 options, held: 125 options were answered and 126 were not. A model with a longer context may answer either, so the test asserts the code only when the server refuses. A caller must cap options and shorten states per model, far below TypeSafe's 255 options and 65,536 tokens: Ollaya refuses rather than answer from part of the state on `/v1/*`.
- **A Box is refused**, a 422 naming the tags it knows.
- **An image is answered without being seen** (`an_image_the_model_cannot_see_is_answered_without_it`): the answer and the input tokens are those of the request without it. This is the trap: the hosted API reads the image, and Ollaya with a text-only model answers without it in either shape and says nothing. A rubric about an image must not be pointed at a text-only Ollaya model, and the test fails the day a release starts reading images, so the profile is revisited then.
- **Every body holds to the document**, so the profile asserts it, as `typesafe`, `autojev` and `liquid` do.
- **The confidence formulas hold** (choice 0.64 on the wire against 0.639 from the formula, score confidence 0.93 against 0.926, score 1.95 against an expected level of 1.951), so the profile asserts them, as Ollaya's reference documents TypeSafe's formula.

## What is still open

The run shows the wire, not the model: accuracy and calibration on real cases are not measured here, and Ollaya's own page puts the base `laya:en` near chance zero-shot on typed decisions (0.357). Other models (`winnow:e4b`, which needs an NVIDIA GPU; `laya:multilingual`, with a 1,024-token context and about 250 options) were not run; the profile's pins hold for them as written, since none depends on the model's budget. A 503 `QUEUE_FULL` was not drawn.

## How to repeat it

```sh
# Ollaya 0.17.0, no service, no GPU libraries; zstd unpacks the archive
curl -fsSL https://ollaya.dev/install.sh -o install.sh   # read it first
OLLAYA_VERSION=0.17.0 OLLAYA_NO_SERVICE=1 OLLAYA_INSTALL_DIR=$HOME/.local sh install.sh
ollaya serve &
ollaya pull laya:en
mise run live:ollaya

# the bearer test as well, against a second instance that checks a key
OLLAYA_HOST=127.0.0.1:11436 OLLAYA_API_KEY=live-secret ollaya serve &
JUDGMENT_LIVE_AUTH_BASE_URL=http://127.0.0.1:11436 JUDGMENT_LIVE_AUTH_API_KEY=live-secret \
  mise run live:ollaya
```

`OLLAYA_URL` and `OLLAYA_MODEL` point the task at another server or model.
