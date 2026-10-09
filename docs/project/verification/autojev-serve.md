---
title: Against autojev-serve
description: How the judgment crate was tested against autojev-serve, the server pplx-decider-v1.1-27b's checkpoint ships, with the model replaced by a stand-in network, what that can and cannot show, every status and body shape the live tests observed, and how they differ from the hosted API.
status: experiment
last_reviewed: 2026-10-09
tags: [judgment, typesafe, pplx-decider, autojev, compatibility]
---

# Against autojev-serve

Perplexity's [pplx-decider-v1.1-27b](https://huggingface.co/perplexity-ai/pplx-decider-v1.1-27b) is a decision model, a classifier with a 255-row decision head, not a chat model. Its checkpoint ships its own HTTP server, `autojev-serve`, which serves `POST /v1/systemone` and `GET /v1/models`. `mise run live:pplx` runs the live tests against it with `JUDGMENT_LIVE_PROFILE=autojev`; this page is what that profile pins and where each pin comes from.

The short answer: the wire holds. Every body the server sent conforms to the vendored OpenAPI document, refusals included. Where it differs from the hosted API is where the document is silent: which requests it refuses, and with which status.

## What was tested, and with what

| | |
|---|---|
| Server | `autojev/server.py` from the checkpoint's `source/` (the FastAPI app reports version 0.2.0), run unchanged |
| Model | Not the real one: it needs a CUDA GPU with about 49 GiB for its weights, and the run had none. `answer()`, `options()` and `describe()` from the checkpoint's `model.py` ran unchanged; only the network was replaced, by deterministic logits hashed from each row |
| Machine | 4 CPUs, 15 GB, no GPU, in the development container |
| Date | 2026-10-09 |
| Profiles | `autojev` and `generic`, model `autojev-qwen3.8-27b`; `autojev` again with the model named `jev-latest` |

Everything the server decides before the model runs is the real server's: request validation, the status of each refusal, the body shapes, the aliases, the model list, the bearer check. What depends on the model is not shown here: accuracy, calibration, the confidence formulas, and what the real tokenizer does past its window.

## What the server does

| Request | autojev-serve | The hosted API |
|---|---|---|
| The three primitives, a structured level, an option with no description | 200, conforming to `SystemOneResponse` | 200 |
| 256 options | 422, `HTTPValidationError` (`Dictionary should have at most 255 items`) | 400, `at most 255` |
| 11 levels | 422, `HTTPValidationError` (`List should have at most 10 items`) | 400 |
| One option | 200, probability 1 | 200, probability 1 |
| One level | 422, `HTTPValidationError` (`List should have at least 2 items`) | 200 |
| An empty question id | 200, answered under the key `""` | 400, `cannot be empty` |
| A state of about 40,000 tokens | answered: no budget is checked before the model | 400, `max_tokens_exceeded` |
| An unknown extra body field | 422, `Extra inputs are not permitted` | 400, `Invalid request.` |
| `GET /v1/models` | 200, conforming to `ModelMetadataList`: `autojev`, `autojev-qwen3.8-27b`, `jev-1.13.0`, `jev-latest`, `jev-preview` | 200 |
| A wrong bearer token, with `AUTOJEV_API_KEY` set | 401 | 401 |

Two things the hosted API sends and this server does not are both allowed by the document: a Noul answer with no `confidence` (`NoulAnswer` requires only `noul` and `type`) and `output_tokens` always 0.

## Why the profile, and not the model name

The server answers to `jev-latest`, `jev-preview` and `jev-1.13.0` as well as its own names. The live tests used to take any model whose name starts with `jev` for the hosted API and assert its status codes, so a run under an alias failed on 422 against 400. The tests now take the server from `JUDGMENT_LIVE_PROFILE` ([How the crate is checked](method.md)), and the same run passes under `jev-latest`. Name the model `autojev-qwen3.8-27b` anyway, so a recording or a report does not say Jev.

## What is still open

A run on a GPU with the real weights: the decisions themselves, the confidence the model gives, and the over-budget state, which the real tokenizer may truncate or refuse. That run belongs on this page, with the checkpoint's revision.

## How to repeat it

```sh
# on a CUDA host, with the snapshot's source/ installed with its requirements.txt
AUTOJEV_CHECKPOINT=<snapshot> PORT=8010 autojev-serve

# from the crate's root: the live tests under the autojev profile
PPLX_URL=http://127.0.0.1:8010 mise run live:pplx
```
