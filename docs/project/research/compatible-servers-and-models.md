---
title: Compatible servers and models
description: The servers and models that speak the System One wire as of 2026-10-06, hosted and local, what the Jev Decision Index 0.2.1 says about how close the open ones are to Jev, what each one's limits and confidence semantics mean for a consumer of this crate, and which the crate has been run against or should be next.
status: current
last_reviewed: 2026-10-09
tags: [research, judgment, typesafe, jev, compatibility, calibration, ollama, laya, cloudflare, clef]
---

# Compatible servers and models

This crate is a client to one wire, `POST /v1/systemone`, and its value is that any server speaking it works unchanged: the same typed questions, the same verified answers, a `base_url` and a `model` apart. That claim is only as good as the list of servers it has been checked against and the knowledge of where each one departs from the hosted API. This page is that list, with the state of the field behind it. It is a research note: it records what was published and measured on the dates given, and what follows for the crate. What a particular application should do with a model (which questions, which thresholds) is the application's own concern and not covered here. The [client libraries survey](system-one-client-libraries.md) covers the other clients; this page covers the servers.

Sources, read on 2026-10-03 and 2026-10-04: the [Decision Index build repository](https://github.com/apolinario/decision-index) (edition 0.2.1, 2026-09-27) and a [reproduction kit](https://github.com/tic-top/decision-index) for it; the repositories of [Decider](https://github.com/Mapika/decider), [Nimble](https://github.com/bespokelabsai/nimble), [OpenDecider](https://github.com/manjunathshiva/opendecider), [openJev-verdict 2.0](https://github.com/Heman10x-NGU/openJev-verdict-2.0), [Laya](https://github.com/NandhaKishorM/laya/releases) and [Ollama](https://github.com/ollama/ollama/releases/tag/v0.35.0); and the vendor announcements of Cloudflare's Clef (2026-10-01), Perplexity's Decisions API (2026-10-01) and Fastino's GLiDE (2026-10-01) as reported by secondary sources. The index's own leaderboard page on Hugging Face was not reachable from the environment this page was written in, so the index scores below are the ones the model authors report for their own entries; rankings within two or three points are not distinguishable, and the index publishes no confidence intervals. The crate's own measurements are in [Verification](../verification/hosted-typesafe.md). Read on 2026-10-06, through search summaries because the pages themselves were not reachable from the environment either: Cloudflare's Workers AI pages for [Clef](https://developers.cloudflare.com/workers-ai/models/clef/) and [Clef-flash](https://developers.cloudflare.com/workers-ai/models/clef-flash/), its [changelog entry](https://developers.cloudflare.com/changelog/post/2026-10-01-clef-workers-ai/), and the [Ollama 0.35.1](https://github.com/ollama/ollama/releases/tag/v0.35.1) release that added both.

## The field in one paragraph

Jev's interface is public and simple: a state, typed questions, one pass, a probability per option. Its weights and training are not. Within three weeks of its release (2026-09-15) the interface was rebuilt three ways: parallel constrained decoding over a stock language model (no training), full or LoRA fine-tunes of 2B to 35B decoders with a readout over answer codes instead of a language-model head, and small encoders trained from scratch or fine-tuned for one distribution. In the first days of October 2026 two large vendors shipped open weights on the wire (Cloudflare, Perplexity), one shipped a closed hosted model that claims to lead the index (Fastino), and Ollama added the endpoint to the most common local runtime. The wire is now a commodity; the models behind it differ in accuracy, calibration, limits and the meaning of `confidence`, which is what a consumer of this crate has to know.

## The Decision Index, edition 0.2.1

The [Decision Index](https://huggingface.co/spaces/multimodalart/jev-decision-index) is a community leaderboard: every entrant answers byte-identical requests from a frozen suite on one GPU, Jev over its hosted API, and the score is chance-corrected (0 is guessing, 100 is perfect) so a yes/no benchmark hands out no free points. Edition 0.2.1 (2026-09-27) rescored edition 0.2 on the same suite files: 38 benchmarks in five areas weighted by the square root of their size with "Arts & Human Taste" fixed at 10%, and a request the model did not answer counts as wrong. Scores from the two editions are not comparable; Jev 1.13.0 reads 57.9 on 0.2.1 where it read 51.7 on 0.2.

The top of the board on 0.2.1, as the authors of each entry report it:

| Model | Skill | ECE | Kind | Weights |
|---|---|---|---|---|
| Clef 27B (Cloudflare) | 61.2 | | full fine-tune | Apache 2.0 |
| Jev 1.13.0 (TypeSafe) | 57.9 | 0.074 (edition 0.2) | hosted | closed |
| Surogate Rune 26B-A4B v3 | 57.4 | | MoE fine-tune | open |
| decider-chat on Gemma4-31B | 57.3 | 0.047 | constrained decoding, no training | stock model |
| Clef-flash 9B (Cloudflare) | 57.1 | | full fine-tune | Apache 2.0 |
| pplx-decider-v1-27b (Perplexity; the entry formerly named AutoJev-27B) | 56.4 | 0.018 | full fine-tune, multimodal | Apache 2.0 |
| Qwen3.8-27B, stock | 55.7 | | baseline | open |
| decider-35b-a3b | 47.1 | 0.023 | MoE fine-tune | Apache 2.0 |
| decider-4b | 40.7 | 0.084 | fine-tune | Apache 2.0 |
| decider-2b | 29.0 | 0.077 | fine-tune, calibration-trained | Apache 2.0 |

Fastino reports 64.8 for GLiDE, above every row, but GLiDE is closed and API-only, its "thinking" mode adds latency and tokens, and the figure is the vendor's own run; it is a hosted competitor to Jev rather than an open model.

Three readings for a consumer of this crate:

1. **Jev is no longer the ceiling.** A 27B open model leads it by about three points, and a 9B one (Clef-flash) sits within a point. A 9B model fits one 24 GB GPU. Whether to self-host is still a question of data residency, provider risk and operating cost against an API that charges a fraction of a cent per request; it is no longer a question of accuracy.
2. **The split by benchmark matters more than the headline.** Clef wins classification, routing and tool selection (BANKING77 94.2 against Jev's 79.7) and Jev wins the reasoning-heavy sets (GPQA Diamond 78.3 against 48.0, MMLU-Pro 82.7 against 65.9). A consumer whose questions classify and route a text has more to gain from the open models than one whose questions need the state reasoned about.
3. **Calibration depends on the suite as much as the model.** Jev's ECE is 0.074 on the index and 0.164 on OpenDecider's 200 general decisions; the fine-tunes that train on calibration (pplx-decider, the Decider family) report 0.02 to 0.05. A `Confidence` is only comparable across models after each has been fit on the consumer's own labelled data, which is what `judgment::eval` is for.

## Servers on the wire

What a consumer can point the crate at, as of 2026-10-06. "Verified" means this crate's ignored live tests (`tests/live.rs`) have passed against it and a page under `docs/project/verification/` records the run.

| Server | Where it runs | Models | Limits and departures from the hosted API | Verified |
|---|---|---|---|---|
| TypeSafe hosted API | `https://api.typesafe.ai` | `jev-latest`, `jev-preview` | 255 options, 2 to 10 levels; the reference | yes, [2026-10-03](../verification/hosted-typesafe.md) |
| `laya-serve` (Laya 0.3.24 to 0.3.26) | local, CPU or GPU | three 421M to 322M encoder checkpoints | no `GET /v1/models`; `model` is `laya-rl-agent` for every checkpoint, the checkpoint in `routing`; `confidence` is one minus normalised entropy, not TypeSafe's spread; 413 past 100 options or 50,000 characters of state; seconds per request on CPU | yes, [2026-10-03](../verification/laya-typed-decisions.md) |
| Ollama 0.35 and later | local | `nimble` (9B), `tev1` (4B), `tev1:0.8b`; `clef` (27B) and `clef-flash` (9B) since 0.35.1 (2026-10-03) | `/v1/systemone` added 2026-09-28 with the three primitives; its `GET /v1/models` is the OpenAI-shaped list, so `list_models` is expected to fail to decode; bearer ignored; option caps and `confidence` semantics not yet observed | no: the next target, `mise run live:ollama` |
| `autojev-serve`, pplx-decider-v1.1-27b's own server | local, CUDA GPU with about 49 GiB | `autojev-qwen3.8-27b`, also answering to `jev-latest`, `jev-preview` and `jev-1.13.0` | 422 for 256 options, 11 levels or one level where the hosted API answers 400 or 200; an empty question id answered; no token budget in front of the model; Noul answers carry no `confidence`, `output_tokens` is 0 | partly: the server, with a stand-in for the network, [2026-10-09](../verification/autojev-serve.md); the real model not yet |
| Decider 1.8 (`pip install decider-ai`) | local, GPU | decider-2b, 2b-vision, 4b, 12b, 35b-a3b | serves `/v1/systemone` with continuous batching; temperature scaled by option count since 1.8.0; no byte-identical reproduction across runs | no |
| OpenDecider 0.6 | local, CPU, CUDA or Apple Silicon | nano (400M encoder), small (4B), medium-td (30B), large-td (80B) | serves `/v1/systemone`; 26 options per question; nano scores 0.796 on typed-decisions against Laya's fine-tuned 0.766 | no |
| OpenRouter System One API | hosted, `https://openrouter.ai/api` | `jev-latest` (`~typesafe/jev-latest`), `typesafe/jev-1.13` | TypeSafe's body and answers plus `id`, `provider` and `usage.cost`; the response names the dated model (`typesafe/jev-1.13-20260917`); no `x-typesafe-request-id`; 402 when the credit runs out; its `GET /v1/models` is OpenRouter's catalogue; the same body is also served at `/api/alpha/decisions` | partly: the client against a stand-in built from the documented shapes, 2026-10-09 ([Configure a backend](../../guides/configure-a-backend.md#jev-through-openrouter)); `mise run live:openrouter` not yet run |
| Perplexity Decisions API | hosted | `pplx-decider-v1-27b` | TypeSafe's primitives (choice, noul, score), 255 options, 250k context, images in the state, $0.04 per million input tokens, weights Apache 2.0 | no; the strongest candidate for a second hosted provider |
| Cloudflare Workers AI | hosted | `clef` (27B), `clef-flash` (9B) | TypeSafe's body and answers, `model` set to `clef` or `clef-flash`, 64 questions per request, images and video in the state; but at one exact URL per model (`/accounts/{account}/ai/run/@cf/cloudflare/{model}`, no `/v1/systemone`, no model list) and inside Cloudflare's `{"result", "success", "errors"}` envelope, so a `base_url` alone does not reach it: `tools/systemone/serve.py cloudflare` maps the path and unwraps the envelope; weights Apache 2.0, self-hosted with Transformers, vLLM or SGLang at about 54 GB (Clef) or 18 GB (Clef-flash) of GPU memory | no: `mise run live:clef`, or `OLLAMA_MODEL=clef mise run live:ollama` for the same weights locally |
| Fastino GLiDE | hosted, closed | GLiDE | 40k context, optional reasoning mode; wire compatibility not confirmed | no |
| meraGPT Decider 1 | hosted, closed | state-decider-1 | `/v1/systemone`, $0.03 per million input tokens; tops the typed-decisions dataset's leaderboard | no |
| stuntdouble | local proxy | shadows Jev with Kev or Laya | a `/v1/systemone` proxy that answers from Jev and compares a local model's answer, reporting whether a swap is safe | no |

Two small models without a server yet are worth knowing: openJev-verdict 2.0 (150M, ModernBERT-base, Apache 2.0) reports 77.1% on typed-decisions with ECE 0.014 and a separate confidence head for escalation, trained in nine hours on a 6 GB GPU; GLiNER2.5-Decide (340M, Apache 2.0, from Fastino) leads Fastino's own 17-dataset suite at 60.1% and runs on CPU at about 170 ms. Both show that a fine-tuned encoder can beat Jev on one distribution; neither is a general engine, and the index places the general encoders below 10.

## What this means for a consumer of the crate

- **The crate does not change between servers; the policy must.** `confidence` means TypeSafe's spread on the hosted API, one minus entropy on Laya, a temperature-scaled softmax on most fine-tunes. A threshold tuned on one is meaningless on another until refit. Record the model a response names (`Response::model`, and `Response::extra["routing"]` on Laya) beside every decision, and treat a change of model as a change of policy that needs a replay (`Recorder`, `Replay`, `judgment::eval`) before it ships.
- **Option caps are the most common failure.** The hosted API and the Perplexity model take 255 options; OpenDecider takes 26; several index entrants refuse 12% to 23% of the suite because they answer with one of 16 to 26 letters; Laya degrades past about 20 options on its token budget. A question whose option list can grow has to be capped in code before the wire, whichever server answers.
- **Score is the weakest primitive everywhere** the open models report per-primitive numbers. A consumer that can express a Score as a Choice over the same described levels gets a `Confidence` the policy can use and often a better answer from a small model.
- **A second provider on the same wire is cheap resilience.** Two hosted servers now publish open weights on the wire. A fallback is a `base_url`, a `model` and a threshold profile; it is worth more than a self-hosted model to a path that must not stall when one API is down.

## What this means for the crate

- **Verify against a third server, and keep doing it on every release.** The live tests passed against two independent implementations; Ollama's endpoint is the cheapest third (`tev1:0.8b` runs on CPU) and exercises a runtime many consumers already have. `mise run live:ollama` runs them; a run that passes, or that finds a departure, becomes a page under `docs/project/verification/`.
- **Expose backend limits as data.** Maximum options, maximum levels and a context budget on the backend, with the hosted API's 255 and 10 as the default, would let a caller shortlist before the wire and let a harness report a refusal as a gap rather than an error. Every server in the table above has different numbers; the crate's are constants today.
- **Keep the answer's probability a first-class accessor.** The index, the typed-decisions leaderboard and every open model grade on the probability of the chosen option; `Choice::confidence_from_probabilities` and `Score::expected_value` exist, and a method for the chosen option's own probability would make the calibration numbers above comparable from inside the crate.
- **Expect `GET /v1/models` to differ.** The hosted API serves TypeSafe's shape, Laya nothing, Ollama the OpenAI shape. `list_models` is the one call on the wire that is not standard; a consumer that lists models should treat a decode failure there as "this server does not say", not as a broken server.
- **Do not add batching, images or streaming on speculation.** Perplexity accepts images in the state and `laya-serve` has a batch endpoint; neither is on the wire every server speaks, and no consumer of this crate has asked.

## Open questions

- **Ollama's departures are unobserved.** Option caps, `confidence` semantics and the error bodies of its endpoint are known only from its release notes until the live tests run. Confidence that the three primitives round-trip: high; on everything else: none.
- **Cloudflare's API speaks the wire's body but not its path.** Confirmed on 2026-10-06 from its model pages; the adapter is `tools/systemone/serve.py`, the first time the crate meets a model it cannot reach with a `base_url`. Whether the client should take an exact endpoint instead, as the other clients are being asked to (litellm's issue 44149), is open; the envelope would still need unwrapping, and that is Cloudflare's shape, not the wire's. Whether Fastino's API speaks the wire was not confirmed.
- **The index's rows above are vendor-reported** and the index publishes no intervals. Re-read the board itself before acting on a difference of under three points.
