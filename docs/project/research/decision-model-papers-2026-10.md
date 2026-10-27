---
title: Decision-model papers, September and October 2026
description: A reading, as of 2026-10-26, of the twenty papers paperswithcode lists under "Decision Model" (typed decisions from System One models such as Jev), grouped by what they study; the findings that recur across them (item accuracy does not predict use, in-sample bars, binary probabilities placed off 0.5, option order, mass on an unstated "unknown", decomposition, label leakage); and what each means for this crate, what it already does and the follow-up beads.
status: current
last_reviewed: 2026-10-27
tags: [research, judgment, jev, calibration, evaluation, thresholds, position-bias, literature]
---

# Decision-model papers, September and October 2026

The method page [Decision Model](https://paperswithcode.co/methods/decision-model) on paperswithcode lists twenty papers. It defines a decision model, or System One model, as one that maps unstructured state directly to a typed decision over predefined options, instead of generating text. Nineteen of the papers appeared between 2026-09-21 and 2026-10-20, so none has been peer reviewed and none has been replicated. This page sorts them by what they study, states the findings that recur across several of them, and says what each finding means for a caller of this crate and for the crate itself.

**In short:** the papers agree that the model is rarely what goes wrong. The evaluation is. A model can be accurate on single items and still fail when used, and a bar tuned on the cases it is graded on looks better than it is. Some models answer differently when the options are reordered. The crate already has the tools for most of this (`jud split`, `jud eval --swap`, one bar per question). Three things are missing:

- a warning when the evaluation is in-sample
- a way to grade near-identical cases together
- a clear statement of what "calibrated" means for each primitive

## Sources

All twenty papers were read on 2026-10-26, from their arXiv abstract pages, or from their paperswithcode pages for the two without an arXiv number. **The full text of none was read.** Every number on this page is the authors' own, as the abstract gives it, and none was reproduced here. Bead `judgment-rwh` reads the five papers that bear most on the crate in full.

## The papers

### Jev, evaluated

| Paper | What it does | What it finds |
|---|---|---|
| Deußer, Sparrenberg, Sifa, [Evaluating and Benchmarking the System One Model Jev](https://arxiv.org/abs/2609.37647) | `jev-1.13.0` zero-shot on 37 datasets, 346,009 requests for under USD 10. Qwen3.8-27B and Gemma-4-E4B scored on the same requests from their next-token probabilities over the options. | Jev is ahead of Qwen on 27 of 37 datasets; Qwen's 9 leads are all within bootstrap intervals. Jev is ahead of Gemma on all 37. Choice probabilities are well calibrated and support selective prediction. Binary probabilities rank items well but sit in the wrong place relative to 0.5: tuned bars take UNFAIR-ToS micro-F1 from 0.50 to 0.75. Rotating the options leaves accuracy unchanged. All three models are weak on low-resource languages, noisy labels and rubric scoring. |
| Li, [Fast Models, Slow Evidence](https://arxiv.org/abs/2610.02267) | Laya and Jev compared on 11 decision points an agent harness makes (routing, tool choice, retrieval relevance, injection detection). 7,283 cases and 6,640 robustness variants, byte-identical inputs, paired tests, and an audit of the paper's own pipeline. | Jev is better on 9 of 11 decision points, by 10.8 to 46.0 points. Neither model beats chance at zero-shot model routing. **Laya changes 30% of its answers when the option order is reversed.** The self-audit corrects its own figures: a reported 23.9% saving was 4.3% once a pre-screen's cost was counted, and **thresholds set in-sample for a 5% miss rate missed up to 17% held out**. |
| Porcedda, [Jev thinks "I don't know", but doesn't say it](https://arxiv.org/abs/2609.35342) | Sys1Cal-v1: True/False propositions whose probability is known by construction, asked through Noul, Choice and Score, scored by total variation distance from the true distribution. | Jev's Choice probabilities for A and not-A read as if they sum to 1, while some probability sits on an unstated third value, "unknown". Counting it moves median soft accuracy from 0.771 to 0.978. |
| Zhuang et al., [DecisionPivot](https://arxiv.org/abs/2610.28522) | 48 tasks, 40,033 items. A core track of contrast groups (items that differ by one detail that changes the answer), checked by independent solvers, and a reference track of 35 public test sets. 40 decision models and 14 general models. | Two 27B models 2.6 points apart on the reference track score 70.1% and 44.2% on contrast groups. Perplexity Decider ranks the positive member of a pair above the negative in 92.3% of 2,400 pairs, but answers both right in only 74.1%: ranking the pair correctly does not mean deciding both correctly. |

### New models

| Paper | What it is | Main number |
|---|---|---|
| Zhuang et al., [StartLux-Decision](https://arxiv.org/abs/2610.17309) | 0.8B to 27B. Instant models answer in one pass; AutoThink models reason only when unsure. Trained with RLCD², reinforcement learning against a proper scoring rule. | 73.4 on Decision Index 0.3 (27B AutoThink). See the [StartLux page](startlux-decision-27b-autothink.md). |
| Zhu et al., [Intern-Decision](https://arxiv.org/abs/2610.22131) | 0.8B to 4B, multimodal. One placeholder per field of a JSON skeleton, all fields scored in one causal pass. | 90.02% average over seven suites against Jev 1.13.0's 88.74%. Temperature scaling cut ECE from 0.213 to 0.089 on a 96-case pilot. |
| Li, Wang, [Jeva](https://arxiv.org/abs/2610.18773) | Encodes the request and each option separately, so its probabilities don't depend on option order, and adds an explicit "no answer" outcome. | No flips under any tested permutation and "near-perfect null detection". The abstract gives no figures. |
| Song et al., [LEV](https://arxiv.org/abs/2610.20951) | Thinks deeper by looping its layers rather than decoding tokens; a stopper sets the depth for each question. | 63.2% at depth 1 to 76.2% at depth 4 on JevBench (2.6B). On-demand depth saves 15 to 24% of compute for under a point. |
| Liquid AI, [Open d1](https://paperswithcode.co/paper/123164) | Open weights `d1-3B` and `d1-omni-600M` (text, vision, audio). | 48.57 on Decision Index 0.2.1; 8 ms on an RTX 4090. |
| Link, [Phocinae-Largha-150M](https://paperswithcode.co/paper/123111) | 144M parameters, for routing an agent's decisions locally. | ECE 0.1313 and 3.00% flips under option reordering, both reported as measured. At a 0.6 bar, 82% of decisions stay local. |
| Wang et al., [Chinese-Jev](https://arxiv.org/abs/2609.36965) | Encoder-only model trained on Chinese, with medicine, law and finance fine-tunes. | 1.24 points above Jev on general Chinese, 20.3× faster. |

### Other modalities

| Paper | Answer types | Main number |
|---|---|---|
| Zhu et al., [VideoJev](https://arxiv.org/abs/2610.18615) | Time spans and choices over video | Backbone accuracy at more than 100× the speed on video QA |
| Qin et al., [MJev-Spatial](https://arxiv.org/abs/2610.22276) | Choices, scalars as value bins, 2D points as image regions | First on 7 of 12 spatial benchmarks |
| Jin et al., [Batched Speech Decisions Without Decoding](https://arxiv.org/abs/2610.02638) | Single-token choices over speech, with a frozen LLM | Gender and emotion from 55% and 28% to 90% |

### Decision models inside larger systems

| Paper | Role of the decision model | Main finding |
|---|---|---|
| Zhu et al., [JevAgentHarness](https://arxiv.org/abs/2610.21412) | Makes an agent's multiple-choice decisions and hands control to the LLM below a bar fitted per choice on the agent's own trajectories | The model most accurate on isolated questions solved only 69.5% of agent tasks. Delegating without a fitted bar cost 19.0 points. With the bar, 4B models made agents 81 to 89% cheaper. |
| Jiang et al., [Jev-Mem](https://arxiv.org/abs/2609.23986) | Controls an agent's memory (types, routing, retrieval budget, when to stop) | 6.6× faster memory construction, 36.7% lower query latency |
| Perifanis et al., [SeLMRoute](https://arxiv.org/abs/2609.34736) | Answers questions about a query, kept as probability distributions, which feed a router choosing an LLM | 72.64% against 69.23% for the best single model |
| Liu et al., [vLLM Semantic Router](https://arxiv.org/abs/2603.04444) | One of thirteen signals combined into a Boolean routing policy | The abstract gives no figures |
| Song et al., [PremiseCheck](https://arxiv.org/abs/2610.21046) | Checks single claims an LLM has extracted from a review comment | A comment checked whole defeats a decision model; split into its claims, it matches GPT-5.5 at about half the cost |
| Guo et al., [Just Ask Jev](https://arxiv.org/abs/2609.29429) | Detects alignment failures zero-shot (ten kinds, 44 benchmarks) | Median AUROC 0.886 from one generic question, 63× cheaper than LLM judges. Context matters mostly through input fields that encode the label. |

## What recurs

1. **Accuracy on single items does not predict how a model behaves in use.** The evidence comes from three papers:
   - In JevAgentHarness, the model most accurate on isolated questions solved 69.5% of agent tasks.
   - In DecisionPivot, two models 2.6 points apart score 26 points apart on contrast groups.
   - In Fast Models, Slow Evidence, accuracy at the gate (58% against 98%) had been presented as end-to-end quality.
2. **A bar tuned on the cases it is graded on looks better than it is.** A 5% target miss rate became 17% held out. JevAgentHarness fits a bar for each choice and loses 19 points without one.
3. **A probability can rank items well and still sit in the wrong place against a fixed bar.** Jev's binary probabilities do; Deußer et al. tune each bar. "Calibrated" depends on the primitive.
4. **Option order moves some models.** The rates on reversed or reordered options:

   | Model | Answers changed |
   |---|---|
   | Laya | 30% |
   | Phocinae | 3% |
   | Jev (rotation) | none |

   Jeva is built so that order cannot matter.
5. **Probability goes to "none of these" whether or not the wire names it.** Sys1Cal finds it missing from Jev's Choice answers. StartLux sends it as `none_probability`, and Jeva models it explicitly.
6. **Narrow questions work and compound ones fail.** PremiseCheck splits review comments into single claims before asking. Intern-Decision and the agent harnesses ask one field or one choice at a time.
7. **A high score can come from leaked labels.** In Just Ask Jev, context helped mostly through input fields that encode the label. DecisionPivot and Sys1Cal build their answers by construction to rule this out.

## What it means for this crate

### What the crate already does

- **Held-out cases.** `jud split` writes a held-out half. [Tune thresholds](../../guides/tune-thresholds.md#hold-some-cases-out) says to quote the held-out number, and `/jud:tune ... holdout` does it.
- **Option order.** `jud eval --swap` asks every Choice again with its options reversed. On Jev 1.13 and 1.14 it moved 1 verdict in 50 ([pairwise judge, position and tone](../verification/pairwise-judge-position-and-tone.md)), which agrees with Deußer et al.
- **One bar per question.** A rubric's gates and `jud tune`'s sweep are the bars Deußer et al. and JevAgentHarness argue for. A bar fixed at 0.5 is what fails.
- **Narrow questions.** The crate's rule that the model answers narrow, single questions is PremiseCheck's result.
- **A way to answer "none of these".** Noul exists, and the [StartLux page](startlux-decision-27b-autothink.md) records that `none_probability` reads from the answer's `extra` since judgment 0.16 and a null `choice` still cannot be decoded.

### What it lacks

| Gap | Evidence | Bead |
|---|---|---|
| `jud eval` does not say when it grades the cases the bars were tuned on. The rubric's `tuning.cases` names them, so the check is a comparison of fingerprints. | 5% → 17% (2610.02267) | `judgment-q1l` |
| Calibration is one number across primitives, read from the server's `confidence` rather than from the answer's probability, and a Brier score assumes a distribution that sums to 1. Sys1Cal's missing "unknown" share shows up there as miscalibration. | 2609.35342, 2609.37647 | evidence added to `judgment-bsz.20.14` and to the decision `judgment-bsz.21.4` (HD4) |
| Gate and tune methodology: whether a bar is judged on the cases it was tuned on. | 2610.02267, 2610.21412 | evidence added to `judgment-bsz.21.14` (HD14) |
| Cases cannot be grouped, so a near-identical pair is graded as two independent items. | 70.1% against 44.2% (2610.28522) | `judgment-xiv`, a `.jud` minor and a decision record |
| [Compatible servers and models](compatible-servers-and-models.md) has no order-sensitivity column, and it lists neither d1's open weights nor the new open models. | Laya 30% (2610.02267) | `judgment-xbl` |

### What the crate should not do

- **Recalibrate probabilities** (Intern-Decision's temperature scaling). That changes a model's output, and the policy that reads it belongs to the caller. The crate's job is to measure calibration honestly.
- **Add a backend for every new model.** Each of Intern-Decision, AgentDecision-4B, Phocinae and Chinese-Jev would be a server row when a consumer needs it, not before.
- **Ship DecisionPivot or Sys1Cal as test data.** Their licences and sizes are unchecked, and the crate's examples prove the format, not a model.

## For a caller of the crate

- Grade a rubric on held-out cases, and quote that number.
- Run `jud eval --swap` once against any backend other than Jev before trusting its Choice answers.
- Ask single claims, not compound ones.
- Where a wrong answer is costly, label a few pairs of near-identical cases and check that both are right, not just one.
- Read a Noul's probability against a bar you tuned, never against 0.5.
