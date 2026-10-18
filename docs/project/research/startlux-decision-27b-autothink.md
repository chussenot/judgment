---
title: StartLux-Decision v1.1 27B AutoThink
description: A study, as of 2026-10-18, of StartLux Labs' 27B decision model that thinks only when its first read-out is unsure, from its model card, its server's documentation, its technical report and its benchmark; what its published numbers can and cannot be compared with, what its server sends that the crate keeps, drops or cannot decode (none_probability, a null choice, sampled thoughts), what a sampled thought does to recordings and thresholds, and what its non-commercial licence allows.
status: current
last_reviewed: 2026-10-18
tags: [research, judgment, compatibility, startlux, autothink, decision-index, calibration, licence]
---

# StartLux-Decision v1.1 27B AutoThink

[StartLux-Decision-v1.1-27B-AutoThink](https://huggingface.co/startlux-models/StartLux-Decision-v1.1-27B-AutoThink) is a decision model that answers a typed question in one pass when it is sure, and writes a thought first when it is not. Its server, `startlux-decide serve`, speaks `/v1/systemone`, so it is a backend this crate can reach with a base URL. This page says what the model is, what its numbers show, and what it would change for a caller of this crate. It is a reading of published sources: the model was not run, since its weights take about 55 GB at BF16 and need a GPU this repository's environment does not have.

The short answer: by its documentation the wire fits what the crate sends by default, two of the server's additions are lost or break decoding, and a sampled thought makes the model's answers vary between runs in a way the crate's recordings and thresholds have to be read with. The weights are licensed for non-commercial use only.

## Sources

Read on 2026-10-18:

- The [model card](https://huggingface.co/startlux-models/StartLux-Decision-v1.1-27B-AutoThink) and the repository's `decision_config.json`, `config.json`, `NOTICE` and metadata (created 2026-10-10, last modified 2026-10-14; 296 downloads and 6 likes when read).
- The inference package's [API reference](https://github.com/StartLuxLabs/StartLux-Decision/blob/main/docs/api.md) and its [page on `none_probability`](https://github.com/StartLuxLabs/StartLux-Decision/blob/main/docs/none_probability.md).
- The abstract of the technical report, [arXiv 2610.17309](https://arxiv.org/abs/2610.17309) (submitted 2026-10-12; 38 pages). The full text was not read.
- The card of [DecisionPivot](https://huggingface.co/datasets/startlux-models/DecisionPivot) v1.1, the authors' benchmark, formerly named DecideBench.

Every number on this page is the authors' own. None was reproduced here.

## What the model is

A 27.4 billion parameter model (27,356,728,560 at BF16) of the `qwen3_5` architecture, 64 layers, with a native context of 262,144 tokens. Its `NOTICE` says the weights are a modified version of an Apache-2.0 model by Alibaba Cloud. It reads text, JSON and up to 8 images per request.

**How it answers.** Each question is its own prompt holding the whole state. The model reads the logits of the option letters A to Z at the last prompt position, divides them by a temperature per question type (Noul 1.376, Choice 0.9503, Score 1.0108, from `decision_config.json`) and normalises them over the listed options. If the top probability reaches a gate, that read-out is the answer. Otherwise the same weights write a thought of at most 4,096 tokens, sampled at temperature 0.6, top-p 0.95, top-k 20, read the letters again, and answer 0.25 × the first read-out + 0.75 × the second. The gate is 0.6 by default; the published results use 0.9.

**How it was trained.** In three stages: shared decision training, a split into the one-pass Instant models (bidirectional attention in their full-attention layers) and the causal AutoThink ones, then supervised reasoning examples followed by RLCD², reinforcement learning that rewards a better probability under a proper scoring rule, scored by a frozen model, and penalises long thoughts. The abstract reports that this raised the share of thoughts that end within the limit by up to 48 points and cut their length by up to 65%, "while largely preserving decision accuracy".

**Its family.** Instant models at 0.8B, 2B, 4B, 9B and 27B, and AutoThink models at 4B, 9B and 27B, with GGUF and MLX builds of this one.

## What its numbers show

| Model | Decision Index 0.3, public subset | DecisionPivot, pair accuracy |
|---|---|---|
| 27B AutoThink | 73.37 | 86.9 |
| 27B Instant | 72.83 | 75.4 |
| 9B AutoThink | 70.45 | 59.9 |
| 4B AutoThink | 68.21 | 59.9 |

From the card, AutoThink at a gate of 0.9.

- **On the Decision Index the thought buys little; on DecisionPivot it buys a lot.** 27B AutoThink leads 27B Instant by 0.54 points on the index and by 11.5 points of pair accuracy on DecisionPivot. With thinking off, the same model reaches 48.0% on DecisionPivot. The card puts the cost at 304 thought tokens per question there, and 273 on held-out items of the training distribution, where the thought gains 2.5 points.
- **The Decision Index scores are not independent results.** The authors ran the public kit on its public subset themselves, and they say they chose training recipes and seeds by these scores. The scores are not leaderboard submissions, and they measure the model the authors selected on them.
- **DecisionPivot is the authors' own benchmark**, though they say it was used for no data, checkpoint or configuration choice. Its core track is 7,800 generated items in 13 families, mostly pairs that differ in one detail that changes the answer, and a pair counts only when both are right. That design is what rewards a thought: a surface cue gets one member of a pair right and the other wrong.
- **Edition 0.3 is not edition 0.2.1.** [Compatible servers and models](compatible-servers-and-models.md) quotes the index at edition 0.2.1, where Jev 1.13.0 reads 57.9. Scores from different editions are not comparable, and no score for Jev on edition 0.3 was found, so this page does not place the model against Jev.

## On the wire, for this crate

The server answers `POST /v1/systemone`, and `POST /v1/decisions` under OpenRouter's name. Every row below reads the server's documentation against the crate's code; none was run.

| What the server does | What the crate does with it |
|---|---|
| Ignores the request's `model` and answers with its own name | The response's `model` is the server's, as with any server; recordings record it. |
| Takes images as base64 strings or data URIs, at most 8 | Both of the crate's shared-image spellings fit: `shared_images(true)` and `shared_images_as_data_urls(true)`. The `assets` map is not documented, so set one of them. |
| Adds `none_probability` to every Choice answer (`none_check`, on by default) | **Dropped.** `Response::extra` keeps unknown top-level fields, but an answer's own fields are fixed, so the value cannot be read through the crate. |
| With `"allow_none": true`, returns `"choice": null` when "None of the above" is most likely | **Fails to decode.** The crate reads `choice` as a string, so the whole response is an `Error::Decode`, never retried, and its usage, thought tokens included, never reaches the `Observer`. Off by default, but on for every request of a server started with `--allow-none`, and settable per question, so a caller who never sets it can still meet it. Sending `"allow_none": false` with `CallOptions::extra` guards against a server's default. |
| Takes `think_mode`, `gate`, `think_budget`, `mix`, `return_details` as top-level fields | Sent with `CallOptions::extra`. `details` per answer is dropped like `none_probability`; `timing_ms` at the top level is kept in `Response::extra`. |
| Counts thought tokens in `usage.output_tokens` | Reported to the `Observer` as output tokens. A thought is the only cost besides the prompt, so it is the number to watch. |
| Follows TypeSafe's confidence formulas; a Noul answer has no `confidence` | As the crate expects. |
| Refuses with 422 and `{"error", "detail": [{"loc", "msg", "type"}]}`; 401 without a key; 429 with `Retry-After: 1` at its in-flight limit; 502 when its engine fails | The top-level `error` string becomes the `Error::InvalidRequest` message and the located `detail` list its issues; 429 and 502 are retried and `Retry-After` honoured. The server has already tried its engine four times before a 502. |
| Sends `x-typesafe-request-id` | Read into `Response::request_id`. |
| Reads 26 options per pass; longer lists are knocked out in groups of 25, the 3 best of each to a final round, the rest sharing 0.001 | Accepted: the crate allows 255 options. The probabilities of options knocked out in the group rounds are a shared remainder, not estimates, so read only the leaders of a long list. |

Of the two gaps, the null `choice` is the one that fails a call; `none_probability` only goes unread. The authors report it as a strong alarm, not a probability: at 0.2 it caught 66.3% of questions with no right option and flagged 2.2% of well-posed ones (AUROC 0.920, on 51,506 questions with this model). A caller of this crate who wants that signal today has to read the raw response.

## What a thought does to recordings and thresholds

- **The same request can get a different answer.** A thought is sampled, so a question that thinks may answer differently on another run. A recording keeps one answer, and a replay returns that one. The fingerprint is the same for every run of a request, so a second recording into the same directory replaces the first (`Recorder` writes one file per fingerprint, `jud record` one per case). To see how often answers differ, record each run into a directory of its own and compare them.
- **The gate and `think_mode` are not in a recording.** The fingerprint covers the state, the questions and the images, not the model or any extra body field, and per-call options do not reach `Recorder` or `jud record`. Set them on the server instead (`serve --think-mode`, or the `auto_think` block of the model folder's `decision_config.json`) and note them beside the recordings, since a recording made at a gate of 0.6 and one made at 0.9 look the same.
- **`think_mode: never` makes it repeatable, and makes it a different model.** Without a thought its DecisionPivot pair accuracy is 48.0, against 86.9 with one. A threshold tuned with thoughts off does not hold with them on.
- **The gate is part of what is measured.** The default is 0.6 and the published numbers use 0.9. Record and evaluate with the gate the application will run. The card's own advice is the crate's: set thresholds on the answers your own policy returns, because a confidence used to select or route answers overstates their accuracy.
- **Latency depends on the case.** One read-out takes 49 ms at 256 tokens on one H200 (vLLM, BF16, from the card). A thought is generated at about 27 tokens per second on an A800 (BF16, from the server's documentation). The card's 273 thought tokens are an average over all questions, those that did not think included, so they come to about 10 seconds per question on that machine. In the authors' `none_probability` run about half of the well-posed questions thought (51%), which would put a thought itself nearer 20 seconds, and one that runs to its 4,096-token budget at about 150 seconds. They report 4% of questions running to the budget, mostly hard ones. The client's default timeout of 10 seconds is per attempt, and a timed-out attempt is retried, up to 3 attempts by default, so a thinking call can time out three times over. The `jud` command's default is 30 seconds.

## Licence

The weights are released under [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/): research and other non-commercial use, with attribution. Commercial use needs a separate licence from StartLux Labs. The inference package is Apache-2.0, and DecisionPivot's core track is CC BY 4.0.

For this repository: no weights or code of theirs would enter it. Whether recordings of the model's answers may be committed under the crate's MIT licence is a question about the licence of outputs, which the card does not answer, so none should be committed without checking.

## What would settle the open points

- A run of `tests/live.rs` against `startlux-decide serve` on a GPU, under the `generic` profile, with the 27B AutoThink and with a small Instant model, and a profile for what it pins. The 27B needs about 55 GB at BF16, the Q8_0 GGUF on llama.cpp less. That run would test every row of the wire table above, the image spellings first, and could share one recording directory per run with the repeat test below.
- Recording the same cases two or three times, to measure how often a thinking question's answer changes.
- A decision on whether the crate should keep an answer's unknown fields, as `Response::extra` does at the top level, which would make `none_probability` and `details` readable, and whether a null `choice` should decode.
