---
title: Against Ollaya
description: How the judgment crate was run against Ollaya 0.17.0's TypeSafe-compatible API on a CPU-only container with the laya:en model, every status and body the live tests and probes observed, how each departs from the hosted TypeSafe API, and what the ollaya profile pins because of it.
status: experiment
last_reviewed: 2026-10-25
tags: [judgment, ollaya, laya, compatibility, self-hosted]
---

# Against Ollaya

[Ollaya](https://ollaya.dev) serves open decision models on your own machine behind an API it documents as wire-identical to TypeSafe's ([TypeSafe compatibility](https://ollaya.dev/docs/typesafe-compatibility)): `POST /v1/systemone`, its alias `POST /v1/decisions`, and `GET /v1/models`, on `http://127.0.0.1:11435` by default. `mise run live:ollaya` runs the live tests against it, under the `ollaya` profile.

The short answer: the wire holds for text, with a real run behind it. Every success body, the model list and every 422's `detail` conform to the vendored OpenAPI document; the 404 and the 401 are statuses the document does not list. The confidence formulas hold, and repeated requests give the same numbers to the last digit. Ollaya departs from the hosted API in three ways a caller must know: every validation refusal is a 422 and a model it has not pulled a 404; each model's context bounds the options and the state far below TypeSafe's; and its `/v1` API, modelled on typesafe-sdk 0.7.1, has no image field and no Box, so a Box is refused and an image is dropped without a word.

## What was tested, and with what

| | |
|---|---|
| Server | Ollaya 0.17.0 (`linux-amd64`), installed with its script pinned to the version (`OLLAYA_VERSION=0.17.0`, `OLLAYA_NO_SERVICE=1`) and started with `ollaya serve` |
| Model | `laya:en` (ModernBERT-large, released 2026-09-23), an 814 MiB store. Ollaya recommends `laya`, a router, on a machine without an NVIDIA GPU; it sends English to `laya:en`, which the run names directly |
| Machine | a Claude Code cloud container: 4 vCPUs, 15 GiB, no GPU, Ubuntu 24.04 (glibc 2.39; Ollaya needs 2.38). The archive is `.tar.zst`, so `zstd` had to be installed first |
| Reference | Ollaya's [API reference](https://ollaya.dev/docs/api) and [TypeSafe compatibility](https://ollaya.dev/docs/typesafe-compatibility) pages, read on 2026-10-25 |
| Date | 2026-10-25 |
| Tests | `tests/live.rs` as `mise run live:ollaya` builds it: 21 tests, the two new ones included (the `multipart` feature adds a twenty-second). Under `generic`, run first with `--all-features` and before the two new tests existed, 18 of 20 passed, the multipart test among them: `a_model_name_the_server_does_not_know_is_still_answered` failed on the 404 and `a_state_over_the_budget_is_refused_as_too_large` on `is_request_too_large`, both pinned below. Under `ollaya`, 21 of 21, and again 21 of 21 against a second instance on `127.0.0.1:11436` started with `OLLAYA_API_KEY`, which runs the bearer test. The shared-images test skipped itself (`JUDGMENT_LIVE_SHARED_IMAGES` unset) |
| Probes | the rows marked *probe* below were sent by hand with `curl` (one is under [How to repeat it](#how-to-repeat-it)); the others come from the tests |
| Latency | a request with a Choice, a Noul and a Score took 0.65 to 0.71 s warm, median 0.68 s. The first request loads the model: 21.5 s on this machine (below) |

## What the server does

| Request | Ollaya, `laya:en` | The hosted API |
|---|---|---|
| The three primitives, a structured level, an option with no description | 200, conforming to `SystemOneResponse` | 200 |
| An unknown top-level field | 200, the field ignored | 400, `Invalid request.` |
| The same body to `/v1/decisions` (*probe*) | 200 | not in the OpenAPI document |
| `GET /v1/models` | 200, TypeSafe's `ModelMetadataList` of the models pulled to the machine (`laya:en`) | 200, the aliases |
| A model it has not pulled (`jev-latest`) | 404, `{"error": "model \"jev-latest:latest\" not found, try pulling it first", "code": "MODEL_NOT_FOUND"}` | 200 |
| 256 options, 11 levels | 422, `INVALID_REQUEST` with TypeSafe's `detail` (`at most 255`, `at most 10`) | 422 from its schema |
| 1 option, 1 level | 422, `INVALID_REQUEST` (`at least 2`) | 200, probability 1 |
| An empty question id | 200, answered | 400 |
| 125 options (*probe*) | 200 | 200 |
| 126 options (*probe*) | 422, `STATE_TRUNCATED`: the options leave the state no room | 200 |
| 130 options (*probe*), 200 options | 422, `TOO_MANY_OPTIONS`, `130 options do not fit the option budget of laya:en` | 200 by the reference's limits; not run |
| A state of 150 words, 330 input tokens (*probe*) | 200 | 200 |
| A state of 300 to 2,000 words (300 to 450 by *probe*) | 422, `STATE_TRUNCATED`, `part of state was dropped to fit the context of laya:en` | 200 by the reference's limits; not run |
| A state of 162,923 tokens | 422, `INPUT_TOO_LONG`, issue `input_too_long`, `the limit is 65536` | 400, `max_tokens_exceeded` |
| A Box question | 422, `INVALID_REQUEST`, naming the `box` tag against the tags it knows, `choice`, `score`, `noul` | 200 on Jev 1.14 |
| A Noul about an image in `assets`, with the images header | 200, the same answer and the same 42 input tokens as without the image | 200, the image read |
| The same image in Ollama's shared `images` array (*probe*) | 200, the same answer and tokens | not in the OpenAPI document |
| A wrong bearer, no `OLLAYA_API_KEY` set (*probe*) | 200: no key is checked | 401 |
| A wrong bearer, `OLLAYA_API_KEY` set | 401, `UNAUTHORIZED` | 401 |
| Three identical requests | the same probabilities, confidences and score to the fourth decimal | 0.01 to 0.19 apart, wider on ambiguous cases |
| The first request after the model is unloaded, with a 10 s limit (*probe*) | no answer within 10 s; the load carries on, and the next request was answered 9.2 s later | not applicable |

Refusals carry Ollaya's own envelope, `{"error", "code", "detail"}`: a string message, a machine-readable code, and, for `INVALID_REQUEST`, `TOO_MANY_OPTIONS`, `INPUT_TOO_LONG` and `STATE_TRUNCATED`, TypeSafe's `detail` list of located issues. The client reads `error` as the message and `detail` as it does the hosted API's, so `Error::InvalidRequest::issues` holds the issues and each one's `kind` (`too_long`, `state_truncated`, `input_too_long`); the top-level `code` is `kind` (`STATE_TRUNCATED`). A 404 is an `Error::Http`. Every response carries `x-typesafe-request-id`.

The cold load is longer than the crate's default timeout of 10 s. The first attempt times out, and the client's default transport policy retries it; Ollaya keeps loading through the dropped request, as its compatibility page says, so the retry finds the model warm or nearly so. On this machine the retry was answered at 9.2 s, under the limit with little to spare: on a slower one, or a larger model, raise the timeout for the first call or load the model first (`ollaya run`, or Ollaya's `/api/decide` with `keep_alive`). The live suite builds its client with 120 s, so the run did not draw this.

## What the profile pins, and why

- **422 for every limits probe**, and the message naming the bound, as on Liquid.
- **A model it has not pulled is a 404 with `MODEL_NOT_FOUND`**, where the hosted API answers `jev-latest`. A client built for the hosted API fails against Ollaya until it names an Ollaya model; Ollaya's own page says to set `TYPESAFE_DEFAULT_MODEL`.
- **An empty question id is answered**, as autojev-serve does.
- **An over-budget state is a 422 `INPUT_TOO_LONG`, with the issue `input_too_long`**, which `Error::is_request_too_large` reads as too large, as it does `STATE_TRUNCATED`; `TOO_MANY_OPTIONS` it does not, since the fix is fewer options. `STATE_TRUNCATED` also comes from options that leave the state no room (126 on `laya:en`), where fewer options, not less state, is the fix. Ollaya's limit does not move with the long-context header (the live test's long-context request was refused the same way), so a caller that asks for long context on that predicate is refused again and has to send less.
- **Each model's context bounds the options and the state** (`the_model_s_context_bounds_the_options_and_the_state`). On `laya:en`, whose figures were measured, 200 options are `TOO_MANY_OPTIONS` and a 2,000-word state is `STATE_TRUNCATED`, both 422, and both are pinned. Another model may answer either, or refuse the 200 options as `STATE_TRUNCATED` for the room they leave the state, so for it the test asserts only that a refusal is one of the two. The documented figures for `laya:en`, a 512-token context and about 125 options, held: 125 options were answered and 126 were not. A caller must cap options and shorten states per model, far below TypeSafe's 255 options and 65,536 tokens: Ollaya's `/v1` refuses rather than answer from part of the state.
- **A Box is refused**, a 422 naming the `box` tag and the tags Ollaya knows.
- **An image is dropped without a word** (`an_image_the_model_cannot_see_is_answered_without_it`): the answer and the input tokens are those of the request without it. Ollaya's API page says why: `/v1/systemone` stays identical to TypeSafe's API, which has no image field there, and unknown fields are ignored. Its vision models (`decider:2b-vision`, `winnow:e4b-vision` and others) read images only on its native `/api/decide`, which the crate does not speak. So a rubric about an image must not be pointed at Ollaya's `/v1` at all, whatever the model; the test fails the day a release starts reading them, and the profile is revisited then.
- **Success bodies hold to the document**, so the profile asserts it, as `typesafe`, `autojev` and `liquid` do.
- **The confidence formulas hold** (choice 0.64 on the wire against 0.639 from the formula, score confidence 0.93 against 0.926, score 1.95 against an expected level of 1.951), so the profile asserts them; Ollaya's reference documents TypeSafe's formula.

## What Ollaya's reference adds

The [API reference](https://ollaya.dev/docs/api) documents what the run did not exercise:

- 1 to 256 questions per request; a body over 8 MiB is a 413 `REQUEST_TOO_LARGE`; a body that is not a JSON object is a 400 `INVALID_JSON`.
- A 503 `QUEUE_FULL` with `Retry-After: 1` once `OLLAYA_MAX_QUEUE` requests wait; a loaded model answers one request at a time, and requests to it queue. The client retries a 503 and honours `Retry-After`.
- Probabilities, confidences and scores are rounded to four decimals.
- `laya` is a router: the response's `model` names the checkpoint that answered (`laya:en`), not the name requested. The hosted API also answers with a version rather than the alias.
- Models are pulled from Ollaya's registry (`ollaya.dev` by default) and never as a side effect of a request.

## What is still open

The run shows the wire, not the model: accuracy and calibration on real cases are not measured here, and Ollaya's own page puts the base `laya:en` near chance zero-shot on typed decisions (0.357). Other models were not run: `winnow:e4b`, which needs an NVIDIA GPU, and `laya:multilingual`, with a 1,024-token context and about 250 options. The profile's pins are expected to hold for them, since none depends on the model's budget, but that is untested. A 503 was not drawn, nor a 413.

## How to repeat it

```sh
# Ollaya 0.17.0, no service, no GPU libraries; zstd unpacks the archive
curl -fsSL https://ollaya.dev/install.sh -o install.sh   # read it first
OLLAYA_VERSION=0.17.0 OLLAYA_NO_SERVICE=1 OLLAYA_INSTALL_DIR=$HOME/.local sh install.sh
ollaya serve &
ollaya pull laya:en
mise run live:ollaya

# with a key, which also runs the bearer test
OLLAYA_HOST=127.0.0.1:11436 OLLAYA_API_KEY=live-secret ollaya serve &
OLLAYA_URL=http://127.0.0.1:11436 OLLAYA_API_KEY=live-secret mise run live:ollaya

# a probe by hand: 130 options against laya:en's budget
curl -s http://127.0.0.1:11435/v1/systemone -d "{\"model\": \"laya:en\", \"state\": \"Pick one.\",
  \"questions\": {\"c\": {\"type\": \"choice\", \"instructions\": \"Which?\", \"criteria\":
  $(python3 -c 'import json; print(json.dumps({f"o{i}": f"option {i}" for i in range(130)}))')}}}"
```

`OLLAYA_URL` and `OLLAYA_MODEL` point the task at another server or model; it checks that the model is pulled before the suite runs.
