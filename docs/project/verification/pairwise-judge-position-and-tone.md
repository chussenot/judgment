---
title: Position and tone bias of Jev 1.13 and 1.14
description: Fifty LLMBar Natural pairs asked as a pairwise Choice of Jev 1.13.0 and Jev 1.14.0 through jud record on 2026-10-18, each as written, with the two answers' order reversed (jud eval --swap) and with the better answer rewritten in a cheerful, sad, angry or fearful tone (--tone-perturb); the flip rates with their Wilson intervals, the flips among confident verdicts, the change in probability, the accuracy under each form and which cases moved; the recordings are committed, so every number replays with no key.
status: current
last_reviewed: 2026-10-24
tags: [judgment, verification, bias, pairwise, jev, calibration]
---

# Position and tone bias of Jev 1.13 and 1.14

[arXiv 2610.18122](https://arxiv.org/abs/2610.18122) (Lukassen, Weisser and Silbersdorff, 2026) found that every typed decision model it tested changed verdicts when two answers swapped places, and that rewriting the better answer in an emotional tone hurt them most. This page runs the two tests on the pinned Jev versions the hosted API served on 2026-10-18, with the commands of [Test a judge for position and tone bias](../../guides/test-a-judge-for-position-and-tone-bias.md). It is one small set, not a replication of the paper.

| | |
|---|---|
| Date | 2026-10-18 |
| Models | `jev-1.13.0` and `jev-1.14.0`, requested by version, each response naming the version that answered |
| Rubric | `examples/jud/pairwise-judge.jud`: one Choice, the two answers as its options |
| Cases | `examples/jud/pairwise-judge-cases.jud`: 50 pairs of LLMBar's Natural set (Zeng et al., ICLR 2024, MIT), the 25 shortest with the better answer first and the 25 shortest with it second |
| Tone rewrites | `examples/jud/pairwise-judge-tone.yaml`: each better answer kept and put in one of CALM's four tones in turn, 13 cheerful, 13 sad, 12 angry, 12 fearful; written for this example |
| Requests | 150 per model: each case as written, swapped (`--swap`, separate requests) and toned |
| Recordings | `examples/recordings/pairwise-judge-jev-1.13/` and `-jev-1.14/`; `tests/jud_perturb.rs` replays both and fails when a count moves |
| Latency | median 111 ms (1.13) and 122 ms (1.14) a call; 439 input tokens at the median |

## Results

The confident column counts the pairs whose original verdict had a confidence of at least 0.5, 47 of the 50 on both models. Intervals are 95 % Wilson.

| model | test | flip rate | flip rate, confidence ≥ 0.50 | mean \|Δp\| | accuracy, original | accuracy, perturbed |
|---|---|---|---|---:|---|---|
| Jev 1.13 | swap | 0.02 [0.00, 0.10] (1/50) | 0.00 [0.00, 0.08] (0/47) | 0.029 | 0.94 [0.84, 0.98] (47/50) | 0.96 [0.87, 0.99] (48/50) |
| Jev 1.13 | tone | 0.16 [0.08, 0.29] (8/50) | 0.15 [0.07, 0.28] (7/47) | 0.164 | 0.94 [0.84, 0.98] (47/50) | 0.78 [0.65, 0.87] (39/50) |
| Jev 1.14 | swap | 0.02 [0.00, 0.10] (1/50) | 0.00 [0.00, 0.08] (0/47) | 0.031 | 0.94 [0.84, 0.98] (47/50) | 0.96 [0.87, 0.99] (48/50) |
| Jev 1.14 | tone | 0.12 [0.06, 0.24] (6/50) | 0.09 [0.03, 0.20] (4/47) | 0.129 | 0.94 [0.84, 0.98] (47/50) | 0.86 [0.74, 0.93] (43/50) |

- **The swap moved one verdict, an unsure one.** On both models the only flip is `llmbar-natural-042`, a pair whose better answer is `b`, which both picked as `a` at a confidence of 0.36 and 0.32 and as `b` once the order was reversed. No verdict of confidence 0.5 or more moved, and the mean change in probability was three points. The paper's typed models all changed some verdicts under a swap, and these two did, once each; at one flip in fifty, on a verdict a gate would not have acted on, the order is close to irrelevant on these pairs. They are short and most have one clearly better answer, where a judge has little reason to fall back on position, so this says nothing about harder pairs.
- **The tone moved more, and confident verdicts too.** Jev 1.13 flipped 8 of 50, every one a right verdict made wrong, and fell from 0.94 to 0.78; Jev 1.14 flipped 6, five made wrong and one corrected, and fell to 0.86. Seven and four of those flips were verdicts at a confidence of 0.5 or more, so a gate at that bar would have acted on them. The cases that moved: on 1.13, `021`, `038`, `040`, `045`, `067`, `092`, `096` and `099`; on 1.14, `021`, `040`, `045`, `067`, `069` and `092`, of which `069`, a miss as written, became right.
- **No one tone did it.** The 1.13 flips were 3 angry, 2 cheerful, 2 sad and 1 fearful; the 1.14 flips 3 cheerful, 2 angry and 1 sad. Twelve or thirteen cases per tone is too few to rank them.
- **1.14 is less moved by tone than 1.13**, by 4 flips in 50; the intervals overlap, so fifty pairs do not show the difference to be real.

## Agreement between the two versions

Added on 2026-10-24 with `jud eval --against`, from the same recordings, no call:

```sh
jud eval examples/jud/pairwise-judge.jud examples/jud/pairwise-judge-cases.jud \
  --replay examples/recordings/pairwise-judge-jev-1.14 \
  --against examples/recordings/pairwise-judge-jev-1.13
```

| question | pairs | differs | differs, confidence ≥ 0.50 | mean \|Δp\| | accuracy | accuracy, against |
|---|---:|---|---|---:|---|---|
| better | 50 | 0.00 [0.00, 0.07] (0/50) | 0.00 [0.00, 0.08] (0/47) | 0.016 | 0.94 [0.84, 0.98] (47/50) | 0.94 [0.84, 0.98] (47/50) |

Jev 1.14 picks what Jev 1.13 picks on every case as written, and its probability on the pick is within two points of 1.13's on average. They agree on their misses too, the same three, which is why agreement is a screen and not a grade ([Agreement before labels](../../concepts/rubrics-cases-recordings.md#agreement-before-labels)). The two versions differ only once the request is perturbed (the tone rows above). `tests/jud_perturb.rs` replays it.

## What this does not show

- **Fifty pairs.** Every rate above carries an interval ten to twenty points wide. A conclusion about a judge on a task needs that task's cases, in the hundreds.
- **The rewrites are this repository's.** They keep each better answer's content, but some add a hedge ("I hope that's right") that a careful judge could fairly hold against an answer. CALM generated its rewrites with GPT-4 Turbo; these were written by hand for the example.
- **The swap is asked in a separate request** (`--swap=separate`) under the id `better.swapped`, so that it is a request of its own to the recordings. TypeSafe documents that the id is not sent to the model ([Position and tone](../../reference/cli.md#position-and-tone)), so on these two models the two requests differ only in order.
