---
title: Against Liquid's System One API
description: How the judgment crate was run against Liquid's hosted /decisions/v1/systemone with the d1 and d1-ultra decision models, every status and body shape the live tests observed, how each departs from the hosted TypeSafe API, and what the liquid profile pins because of it.
status: experiment
last_reviewed: 2026-10-18
tags: [judgment, liquid, d1, d1-ultra, compatibility]
---

# Against Liquid's System One API

Liquid serves TypeSafe's System One API for its decision models `d1` and `d1-ultra` at `https://api.liquid.ai/decisions`: the client reaches `POST /decisions/v1/systemone` and `GET /decisions/v1/models` with that as its base URL and no other change. `mise run live:liquid` runs the live tests against it with `JUDGMENT_LIVE_PROFILE=liquid`; this page is what that profile pins and where each pin comes from.

The short answer: the wire holds for text. Every success body conforms to the vendored OpenAPI document, and the confidence formulas hold within rounding. Liquid differs from the hosted API where the document is silent: it refuses with 422 where the hosted API answers 400 or 200, it refuses a model name it does not serve, and it has no Box question.

## What was tested, and with what

| | |
|---|---|
| Server | `https://api.liquid.ai/decisions`, Liquid's hosted API |
| Model | `d1`, then `d1-ultra` |
| Reference | Liquid's API reference ("Liquid inference API" 1.0.0), the decision models' section |
| Date | 2026-10-18 |
| Profiles | `generic` first on `d1`, then `liquid` on both models; all 19 tests pass under `liquid` on each, the shared-images test included |

## What the server does

| Request | Liquid, `d1` | The hosted API |
|---|---|---|
| The three primitives, a structured level, an option with no description | 200, conforming to `SystemOneResponse`, plus `usage.cost` in US dollars (read into `Usage::cost`) | 200 |
| 256 options | 422, `A Choice accepts at most 255 options.` | 400, `at most 255` |
| 11 levels | 422, `A Score accepts at most 10 levels.` | 400 |
| One option | 422, `A Choice needs at least 2 options.` | 200, probability 1 |
| One level | 422, `A Score needs at least 2 levels.` | 200 |
| An empty question id | 422, `Invalid key in record` | 400, `cannot be empty` |
| A state of about 40,000 tokens in about 500 KB of text, with or without long context asked for | 422, `over the model's input limit of 65536 tokens, images included, or 250000 bytes of text`: the bytes, not the tokens | 400, `max_tokens_exceeded`; answered under long context |
| An unknown extra body field | answered, the field ignored | 400, `Invalid request.` |
| The model name `jev-latest` | 404, `not_found_error`, `The model `jev-latest` does not exist.` | 200 |
| An image with a Box question | 422, `Invalid discriminator value. Expected 'noul' \| 'choice' \| 'score'` | 200 on `jev-preview` |
| An image as a shared `images` array (`CallOptions::shared_images`) | 422, `An image must be a base64 `data:` URL or a `{ content_type, base64 }` object.` | not served |
| `GET /v1/models` | 200, conforming to `ModelMetadataList`: `d1`, `d1-ultra` | 200 |
| A wrong bearer token | 401 | 401 |

Refusals come in OpenAI's envelope, `{"error": {"message", "type", "param", "code"}}`, with `type` `invalid_request_error` or `not_found_error` and `code` null. The client keeps `type` as `kind`, and the message as the detail. There is no request id header.

## What the profile pins, and why

- **422 for every refusal** the limits probes, the empty id and the over-budget state draw. Liquid's reference says validation errors return 422, and the run agrees.
- **A model name it does not serve is a 404**, an `Error::Http`, where every server run before answered it. A client built for the hosted API, sending `jev-latest`, fails against Liquid until it names `d1`.
- **An over-budget state is not recognisable as one.** `Error::is_request_too_large` reads a 413 or `kind == "max_tokens_exceeded"`, and Liquid sends neither: only the message says the state is over the limit. The over-budget test asserts the 422 on Liquid and leaves the predicate out; a caller that needs to tell this refusal from others has only the message to go on.
- **Success bodies hold to the document**, so the profile asserts it, as `typesafe` and `autojev` do.
- **The confidence formulas hold** within the hosted API's tolerances (on the run, choice 0.85 against 0.846, score 0.55 against 0.547, score 1.70 against an expected level of 1.698), so the profile asserts them, as `typesafe` does.

Repeated identical requests agree on the answer but not on the hundredths (on `d1`, a spread of 0.032 in probability and 0.006 in score over three calls; on `d1-ultra`, 0.017 and 0.022), as on the hosted API.

## `d1-ultra`

`d1-ultra` behaved as `d1` did on every row above, with its own limit in the over-budget message (`32768 tokens`) and its own numbers in the confidence check (choice 0.82 against 0.824, score 0.92 against 0.919, score 1.95 against 1.946). One profile covers both.

## What Liquid's reference adds

The decision models' section documents what the run did not exercise:

- An `images` array of up to 8 images, each a `data:` URL or a `{ content_type, base64 }` object, on models whose `input_modalities` include `image` (`d1`). It is Liquid's extension; the crate's `shared_images` sends bare base64 strings, which Liquid refuses (judgment-05p).
- Images may also ride in an array state as OpenAI `image_url` parts.
- Up to 128 questions per request; `d1-ultra` allows 32,768 tokens; both allow 250 KB of text.
- 429 with `Retry-After` when the organisation's per-model limit is spent, and with code `model_unavailable` when the model has no healthy target. The client retries a 429 and honours `Retry-After`; neither was observed.

## What is still open

Images through `images` once the crate can send them in Liquid's shape, and a 429.

## How to repeat it

```sh
# LIQUID_API_KEY in .env; spends a handful of model calls on the Liquid account
mise run live:liquid

# the shared-images test as well
JUDGMENT_LIVE_SHARED_IMAGES=1 mise run live:liquid

# another model
LIQUID_MODEL=d1-ultra mise run live:liquid
```
