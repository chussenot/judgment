---
title: Against Liquid's System One API
description: How the judgment crate was run against Liquid's hosted /decisions/v1/systemone with the d1 and d1-ultra decision models, every status and body shape the live tests observed, how each departs from the hosted TypeSafe API, and what the liquid profile pins because of it.
status: experiment
last_reviewed: 2026-10-25
tags: [judgment, liquid, d1, d1-ultra, compatibility]
---

# Against Liquid's System One API

Liquid serves TypeSafe's System One API for its decision models `d1` and `d1-ultra` at `https://api.liquid.ai/decisions`: the client reaches `POST /decisions/v1/systemone` and `GET /decisions/v1/models` with that as its base URL and no other change. `mise run live:liquid` runs the live tests against it with `JUDGMENT_LIVE_PROFILE=liquid`; this page is what that profile pins and where each pin comes from.

The short answer: the wire holds, for text and, sent as Liquid spells them, for images. Every success body conforms to the vendored OpenAPI document, and the confidence formulas hold within rounding. Liquid differs from the hosted API in its refusals: it refuses with 422 where the hosted API answers 400 or 200, in OpenAI's error envelope rather than the document's `HTTPValidationError`; it refuses a model name it does not serve; and it has no Box question.

## What was tested, and with what

| | |
|---|---|
| Server | `https://api.liquid.ai/decisions`, Liquid's hosted API |
| Model | `d1`, then `d1-ultra` |
| Reference | Liquid's API reference ("Liquid inference API" 1.0.0), the decision models' section, read from the Liquid console (`https://console.liquid.ai/docs`, behind a sign-in) |
| Date | 2026-10-18 |
| Profiles | `generic` first on `d1`, then `liquid` on both models; all 19 tests pass under `liquid` on each, the shared-images test (`JUDGMENT_LIVE_SHARED_IMAGES=1`) included |

## What the server does

| Request | Liquid, `d1` | The hosted API |
|---|---|---|
| The three primitives, a structured level, an option with no description | 200, conforming to `SystemOneResponse`; a first request by hand also carried `usage.cost` in US dollars, which `Usage::cost` reads | 200 |
| 256 options | 422, `A Choice accepts at most 255 options.` | 400, `at most 255` |
| 11 levels | 422, `A Score accepts at most 10 levels.` | 400 |
| One option | 422, `A Choice needs at least 2 options.` | 200, probability 1 |
| One level | 422, `A Score needs at least 2 levels.` | 200 |
| An empty question id | 422, `Invalid key in record` | 400, `cannot be empty` |
| A state of about 40,000 tokens in about 500 KB of text, with or without long context asked for | 422, `over the model's input limit of 65536 tokens, images included, or 250000 bytes of text`: the bytes, not the tokens | 400, `max_tokens_exceeded`; answered under long context |
| An unknown extra body field | answered, the field ignored | 400, `Invalid request.` |
| The model name `jev-latest` | 404, `not_found_error`, `The model `jev-latest` does not exist.` | 200 |
| An image with a Box question | 422, `Invalid discriminator value. Expected 'noul' \| 'choice' \| 'score'` | 200 on `jev-preview` |
| An image as a shared `images` array of bare base64 (`CallOptions::shared_images`) | 422, `An image must be a base64 `data:` URL or a `{ content_type, base64 }` object.` | not served |
| The same array of `data:` URLs (`CallOptions::shared_images_as_data_urls`) | 200 on both models: `circle_present` 1.00 for the drawn circle, against about 0.5 for the same question with no image | not served |
| `GET /v1/models` | 200, conforming to `ModelMetadataList`: `d1`, `d1-ultra` | 200 |
| A wrong bearer token | 401 | 401 |

Refusals come in OpenAI's envelope, `{"error": {"message", "type", "param", "code"}}`, with `type` `invalid_request_error` or `not_found_error` and `code` null. The client keeps `type` as `kind`, and the message as the detail. This is not the document's `HTTPValidationError` for a 422, a `detail` list of located issues: `Error::InvalidRequest::issues` is always empty on Liquid, and the schema check's "conforms" on a refusal means nothing here, since every field of `HTTPValidationError` is optional. The profile checks each refusal's message instead. There is no request id header.

## What the profile pins, and why

- **422 for every refusal** the limits probes, the empty id and the over-budget state draw. Liquid's reference says validation errors return 422, and the run agrees.
- **A model name it does not serve is a 404**, an `Error::Http`, where the hosted API, Laya and autojev-serve answered it (Ollama 0.40.5 also answers 404). A client built for the hosted API, sending `jev-latest`, fails against Liquid until it names `d1`.
- **An over-budget state is not recognisable as one.** `Error::is_request_too_large` reads a 413 or a code that names the budget (`max_tokens_exceeded`, Ollaya's `INPUT_TOO_LONG` and `STATE_TRUNCATED`), and Liquid sends neither: only the message says the state is over the limit. The over-budget test asserts the 422 on Liquid and leaves the predicate out; a caller that needs to tell this refusal from others has only the message to go on. The crate does not read the message, because the wording is Liquid's to change without notice and a predicate that silently stops matching is worse than one that never did.
- **Success bodies hold to the document**, so the profile asserts it, as `typesafe` and `autojev` do.
- **The confidence formulas hold** within the hosted API's tolerances (over three runs on `d1`, the wire's choice confidence 0.84 to 0.85 against 0.844 to 0.846 from the formula, score confidence 0.55 to 0.57 against 0.547 to 0.570, and the score 1.70 to 1.71 against an expected level of 1.698 to 1.713), so the profile asserts them, as `typesafe` does.

Repeated identical requests agree on the answer but not on the hundredths (each run makes three calls; on `d1` the spread was 0.020 to 0.036 in probability and 0 to 0.039 in score over four runs, on `d1-ultra` 0.017 and 0.022), as on the hosted API.

## `d1-ultra`

`d1-ultra` behaved as `d1` did on every row above, with its own limit in the over-budget message (`32768 tokens`) and its own numbers in the confidence check (choice 0.82 against 0.824, score 0.92 against 0.919, score 1.95 against 1.946). One profile covers both.

## What Liquid's reference adds

The decision models' section documents what the runs did not exercise, and the image rules the shared-images rows above follow:

- An `images` array of up to 8 images, each a `data:` URL or a `{ content_type, base64 }` object, on models whose `input_modalities` include `image`; `GET /v1/models` lists `text` and `image` for both `d1` and `d1-ultra` on 2026-10-18. It is Liquid's extension. Ollama's array takes only bare base64 ([what Ollama does differently](../../guides/configure-a-backend.md#what-ollama-does-differently)), so no one spelling serves both, and the crate sends Liquid's with `shared_images_as_data_urls` (judgment-05p).
- Images may also ride in an array state as OpenAI `image_url` parts.
- Up to 128 questions per request; `d1-ultra` allows 32,768 tokens; both allow 250 KB of text.
- 429 with `Retry-After` when the organisation's per-model limit is spent, and with code `model_unavailable` when the model has no healthy target. The client retries a 429 and honours `Retry-After`; neither was observed.

## What is still open

The runs show the wire, not the models: accuracy and calibration on real cases are not measured here. Images were answered for one drawn circle on each model, not measured on cases. A 429 was not drawn.

## How to repeat it

```sh
# LIQUID_API_KEY in .env; spends a handful of model calls on the Liquid account
mise run live:liquid

# the shared-images test as well
JUDGMENT_LIVE_SHARED_IMAGES=1 mise run live:liquid

# another model
LIQUID_MODEL=d1-ultra mise run live:liquid
```
