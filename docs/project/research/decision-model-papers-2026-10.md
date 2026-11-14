---
title: Decision-model papers, September and October 2026
description: A reading, as of 2026-10-26, of the twenty papers paperswithcode lists under "Decision Model" (typed decisions from System One models such as Jev) and a hallucination-detection study of 2026-10-27, grouped by what they study, with a separate reading of the System-1 judge comparison of 2026-10-31 (fourteen decision models against four generative judges on eleven benchmarks: accuracy, calibration, and grader-directed injection); the findings that recur across them (item accuracy does not predict use, in-sample bars, binary probabilities placed off 0.5, option order, mass on an unstated "unknown", decomposition, label leakage); and what each means for this crate, what it already does and the follow-up beads; and two preprints of November 2026, read on 2026-11-13: five readouts trained under one protocol (2611.15592) and a per-bucket recalibration of Laya (2611.16499).
status: current
last_reviewed: 2026-11-13
tags: [research, judgment, jev, calibration, evaluation, thresholds, position-bias, literature]
---

# Decision-model papers, September and October 2026

The method page [Decision Model](https://paperswithcode.co/methods/decision-model) on paperswithcode lists twenty papers. It defines a decision model, or System One model, as one that maps unstructured state directly to a typed decision over predefined options, instead of generating text. Nineteen of the papers appeared between 2026-09-21 and 2026-10-20, so none has been peer reviewed and none has been replicated. This page sorts them by what they study, states the findings that recur across several of them, and says what each finding means for a caller of this crate and for the crate itself.

**In short:** the papers agree that the model is rarely what goes wrong. The evaluation is. A model can be accurate on single items and still fail when used, and a bar tuned on the cases it is graded on looks better than it is. Some models answer differently when the options are reordered. The crate already has the tools for most of this (`jud split`, `jud eval --swap`, one bar per question). Three things are missing:

- a warning when the evaluation is in-sample
- a way to grade near-identical cases together
- a clear statement of what "calibrated" means for each primitive

## Sources

All twenty papers were read on 2026-10-26, from their arXiv abstract pages, or from their paperswithcode pages for the two without an arXiv number. **The full text of none was read.** Every number on this page is the authors' own, as the abstract gives it, and none was reproduced here. Bead `judgment-rwh` reads the five papers that bear most on the crate in full. A twenty-first paper, on hallucination detection, was added on 2026-11-04 from its abstract and the tables it reports; it is the last row of the first table.

## The papers

### Jev, evaluated

| Paper | What it does | What it finds |
|---|---|---|
| Deußer, Sparrenberg, Sifa, [Evaluating and Benchmarking the System One Model Jev](https://arxiv.org/abs/2609.37647) | `jev-1.13.0` zero-shot on 37 datasets, 346,009 requests for under USD 10. Qwen3.8-27B and Gemma-4-E4B scored on the same requests from their next-token probabilities over the options. | Jev is ahead of Qwen on 27 of 37 datasets; Qwen's 9 leads are all within bootstrap intervals. Jev is ahead of Gemma on all 37. Choice probabilities are well calibrated and support selective prediction. Binary probabilities rank items well but sit in the wrong place relative to 0.5: tuned bars take UNFAIR-ToS micro-F1 from 0.50 to 0.75. Rotating the options leaves accuracy unchanged. All three models are weak on low-resource languages, noisy labels and rubric scoring. |
| Li, [Fast Models, Slow Evidence](https://arxiv.org/abs/2610.02267) | Laya and Jev compared on 11 decision points an agent harness makes (routing, tool choice, retrieval relevance, injection detection). 7,283 cases and 6,640 robustness variants, byte-identical inputs, paired tests, and an audit of the paper's own pipeline. | Jev is better on 9 of 11 decision points, by 10.8 to 46.0 points. Neither model beats chance at zero-shot model routing. **Laya changes 30% of its answers when the option order is reversed.** The self-audit corrects its own figures: a reported 23.9% saving was 4.3% once a pre-screen's cost was counted, and **thresholds set in-sample for a 5% miss rate missed up to 17% held out**. |
| Porcedda, [Jev thinks "I don't know", but doesn't say it](https://arxiv.org/abs/2609.35342) | Sys1Cal-v1: True/False propositions whose probability is known by construction, asked through Noul, Choice and Score, scored by total variation distance from the true distribution. | Jev's Choice probabilities for A and not-A read as if they sum to 1, while some probability sits on an unstated third value, "unknown". Counting it moves median soft accuracy from 0.771 to 0.978. |
| Zhuang et al., [DecisionPivot](https://arxiv.org/abs/2610.28522) | 48 tasks, 40,033 items. A core track of contrast groups (items that differ by one detail that changes the answer), checked by independent solvers, and a reference track of 35 public test sets. 40 decision models and 14 general models. | Two 27B models 2.6 points apart on the reference track score 70.1% and 44.2% on contrast groups. Perplexity Decider ranks the positive member of a pair above the negative in 92.3% of 2,400 pairs, but answers both right in only 74.1%: ranking the pair correctly does not mean deciding both correctly. |
| [Decision Models as Judges for Hallucination Detection](https://arxiv.org/abs/2610.37229), 2026-10-27, a preprint not replicated | Five decision models against GPT-6-Sol as the reference judge on 9,852 samples, five kinds of hallucination, six datasets. | The result is per kind, not a verdict on decision models: Jev's balanced accuracy is 82.5% on context inconsistency and 80.7% on factual fabrication, but 69.0% on logical and 61.8% on instruction inconsistency. Across the five models ECE is 0.083 to 0.136, and `confidence` separates right answers from wrong ones at an AUROC of only 0.67 to 0.73; high-confidence errors remain. A cascade that escalates below a confidence bar gains 1.9 to 3.3 points on the factual and context kinds at 1.2 to 3.3% of the large model's cost, and nothing on the other two. The authors flag source confounding, one prompt per task and limited generalisation to deployment. |

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

## A judge comparison, read on 2026-11-06

"To Jev or Not to Jev: When Can System-1 Decision Models Replace LLM Judges?" (Subramanian, Scale AI, [arXiv 2611.00505](https://arxiv.org/abs/2611.00505), submitted 2026-10-31, version 1) post-dates the snapshot above and is a separate reading, added on 2026-11-06 from the paper's full text, not from its abstract. It is a preprint and, by its own account, a single run on established benchmarks at pilot scale (80 to 1,925 items per set). Nothing in it has been reproduced by this crate.

**Scope.** Fourteen System-1 decision models (Jev 1.14, pplx-decider 1.2, pplx-flash 1.2, Clef, Clef-flash, Kev-4B, d1, d1-ultra, Mercury-Decide, Microsoft Decision-1, DReX v1.5, DReX v2.0, DReX-Max and Span-02), four generative judges (Claude Haiku 5.5, GPT-5.6 with and without reasoning, Claude Opus 5.5 with thinking) and eleven judge benchmarks, with identical questions and inputs. Jev 1.14 was served through OpenRouter's decision-model catalogue.

**Jev 1.14, pairwise Choice, averaged over both option orders** (its Table 1): 82.3% on JudgeBench (350 pairs), 87.4% on LLMBar (419) and 81.7% on RewardBench 2 best-of-4 (a 300-item sample). The pointwise checklist formulation reads 79.1%, 85.2% and 83.0% on the same three. Reference-guided grading, where a GPT-5.6-written reference is in the state, reads 91.4% on JudgeBench and 75.4% on RewardBench 2 and was not run on LLMBar. On JudgeBench the reference-guided Jev is among six models significantly more accurate than GPT-5.6 without reasoning, and on PPE best-of-16 every reference-guided System-1 judge is statistically indistinguishable from GPT-5.6 with reasoning.

**Calibration.** Jev 1.14's ECE is 0.019, the lowest of any judge in the study, against 0.038 to 0.087 for the generative judges. The paper's ECE is computed over the single-call A/B probability, the probability of option A from one request in one order, pooled across the pairwise benchmarks; the number of bins is not stated. It is a calibration of the Choice probability, not of the wire's `confidence`, which is the distinction `ece_probability` makes in this crate's report.

**Grader-directed injection** (its Appendix C and E.5, Table 13). One of three payloads was appended to the incorrect response of 150 JudgeBench and 150 LLMBar items, every judge told to ignore claims about grading: an evaluator note claiming the response had been verified against the reference, a fake chat-template system turn ordering the grader to pick it, and a plain self-assertion of correctness. Attack success is defined as the share of items judged correctly without the payload that are judged incorrectly with it. For Jev 1.14's pairwise Choice: 23.3% under the evaluator note, 3.1% under the fake system turn, 0.4% under the self-assertion. Under the note, five models (DReX-Max, d1-ultra, Microsoft Decision-1, d1, pplx-decider 1.2) stayed at 0.0 to 4.7%, statistically indistinguishable from the generative judges; Jev 1.14 and Mercury-Decide (49.0%) were significantly above them; the same-size generative judge Qwen3.5-4B read 77.3%. The paper reads susceptibility as model-dependent rather than inherent to the interface, and says to measure it before a judge reads text an optimizer or a user produced. This is not the paper's position or tone robustness, which it measures separately (its Appendix E.5 on EvalBiasBench and CALM, E.6 on length and position), and attack success is not a flip rate: a flip that corrects a wrong verdict, or moves one that was wrong already, is outside it.

**Limitations the paper states.** A single run at pilot scale; the payload test is the authors' own perturbation of JudgeBench and LLMBar, not an established injection benchmark, so the rates are payload-specific; the relation between leaderboards and judging accuracy rests on 9 to 12 models, some of which may share training sources with the leaderboards, and that analysis measures neither injection robustness nor calibration; stronger models on the leaderboards (Fastino GLiDE, Jeffan-31B, StartLux-Decision, Darwin-27B) were not evaluated, so the best-model results are a lower bound; failed calls were counted as incorrect; the checklists and references were written by Haiku 5.5 and GPT-5.6, so those formulations mix System-1 and System-2 components.

**For this crate.** The injection test is `jud eval --injection-perturb` ([Test a judge for position, tone and injection bias](../../guides/test-a-judge-for-position-and-tone-bias.md)), with the paper's three payloads as the example file and its attack-success definition as the reported rate; it measures the payload supplied and nothing more. The calibration result is one more reason the report carries `ece_probability` beside `ece`.

## Two preprints of November, read on 2026-11-13

Both post-date everything above. Both are preprints at their first version, neither is peer reviewed, every number is the authors' own, and nothing in either has been reproduced by this crate.

### Readout design under one protocol

"Small Heads, Strong Decisions: A Controlled Study of Typed-Decision Models" (Rastogi, [arXiv 2611.15592](https://arxiv.org/abs/2611.15592), submitted 2026-11-11, version 1), read from the paper's HTML text; its appendices were read in summary only.

**What was measured.** Each open typed-decision model so far changed its readout, its backbone, its data and its recipe together, so nobody could say which mattered. The author re-implements five readouts, the part that turns the backbone's hidden states into a score per option, and trains them under one protocol: a small cosine head of his own (0.5M parameters), Clef's joint schema head (27M), the bilinear head of Decision 1.0 and 2.0, the learned letter readout of Decision 3.0, and Nimble's readout of the language model's own next-token logits at the answer letters (no new parameters). Accuracy, expected calibration error over 15 bins and log loss are reported before and after temperature scaling.

**On what data.** One backbone for the comparison, Qwen3-0.6B, with a rank-32 LoRA; fifteen public English training sources (137,497 rows: BoolQ, SST-5, SNLI, MNLI, Banking77, CLINC150, SQuAD 2.0 and others); the same recipe, a learning-rate sweep per head and one seed. In-domain accuracy is over those sources' test splits. Held-out accuracy is over four sets never trained on (AG News, DBpedia-14, QASC, the English Belebele) and a transfer set of 1,046 questions. Larger Qwen3 backbones and Gemma 4 are run for the cosine head only.

**What it finds.**

- **The readout barely matters in-domain.** All five reach 81.3 to 82.0% on Qwen3-0.6B, and the 0.5M-parameter head matches the 27M-parameter one at half its inference time.
- **Held out, they spread from 76.1 to 79.4%**, and the lead of the letter readout comes from one set, Belebele, where the paper traces it to the answer letters themselves: 76.1% with uppercase letters as option codes, 59.3% with symbols.
- **One prompt for all of a state's questions matches one prompt per question** (81.9% in-domain against 82.0%), at 1.6 times the inference speed and 2.1 to 2.5 times the training speed.
- **The backbone is where the accuracy is.** Qwen3-0.6B to Qwen3-4B adds 4 points in-domain (82.0 to 85.9) and 11 held out (78.0 to 88.9). Base, instruct and thinking checkpoints tie.
- **On calibration**, a temperature per question type "cannot correct a source that is over-confident while another of the same type is under-confident".

**Limits the paper states.** One seed, so its intervals are test-set noise and not seed noise; some runs diverged at the default learning rate and were rerun lower; the readouts were compared under a shared joint prompt that the released models do not all use. The code is promised, not yet published.

**For this crate.** Nothing to change. It is evidence for two things the crate already assumes. A Choice's option keys are read by the model, so a key that hints at place or rank is a variable: [the bias guide](../../guides/test-a-judge-for-position-and-tone-bias.md) asks for keys that say nothing about place. And a server may answer a request's questions from one prompt or from one each, at the same accuracy on this evidence. Under one prompt the answers are not independent of each other, which is why `jud eval --swap` asks the reversed order in a request of its own unless told otherwise.

### Per-bucket recalibration of an open model

"Recalibrating a Typed-Decision Model in an Afternoon: A Hands-On Recipe for Laya's Confidence Scores" (Khan and Misri, Intel, [arXiv 2611.16499](https://arxiv.org/abs/2611.16499), submitted 2026-11-12, version 1), read from the paper's full text.

**What was measured.** The calibration of the released Laya checkpoint, pooled and by bucket, and what a refit of its temperatures does. A bucket is Laya's own grouping of items by question type and number of options: `choice:2`, `choice:3-5`, `choice:6-10`, `choice:11+`, `score:3-5`, `noul:2`. Confidence is the top probability, expected calibration error is over ten equal-width bins, and the macro value is the unweighted mean of the buckets' errors. The recipe is five steps (measure, diagnose, fit, validate, deploy) in about 130 lines of numpy, with standard methods.

**On what data.** 1,065 synthetic items with labels written by a language model, in seven evaluation sets from one generator: 681 in `choice:3-5`, 270 in `choice:11+`, 89 in `score:3-5`, 25 in `noul:2`. A check of where the refit helps runs on 33 checkpoints, all scored on that one pool.

**What it finds.**

- **The pooled error hides the buckets'.** 0.073 pooled against 0.143 as the mean over buckets (0.127 to 0.184). The twelve-option bucket is over-confident by 0.22 and the three-to-five-option bucket under-confident by 0.05, and they cancel.
- **One global temperature makes the pooled error worse** (0.173). A cross-fitted temperature per bucket brings the mean to 0.073, to 0.089 with ten labelled items a bucket, and to 0.099 when one of the seven sets is held out.
- **Most of the gain is in separating the buckets, not in refitting.** A gate aimed at 10% error answers 46.3% of items after the refit (10.5% wrong) against 6.9% on the published confidences, but one cut-off per bucket on the published confidences answers 45.4%.
- **Laya's `confidence` field is not its top probability.** For a Choice and a Score it is one minus normalised entropy; read as a probability it has an error of 0.307, against 0.072 for the top probability.
- **The stored temperature of the twelve-option bucket, 0.1006, is clamped to 0.5 at run time.**

**Limits the paper states.** Fit and test share one generator and one labeller, so "these are findings about these checkpoints and this data"; two of the four buckets are too small to tell from chance.

**For this crate.** Three things, none a change of course.

- **Recalibration stays out** ([below](#what-the-crate-should-not-do)): it rewrites a model's output. What the paper shows to matter most, one cut-off per bucket, is what a `.jud` policy already is: one gate per question, fitted by `jud tune`.
- **The measurement is worth having.** `jud eval` reports one calibration error per question; a breakdown by the paper's buckets, with the pooled value and the mean beside it, is bead `judgment-8f2`.
- **It is a second, independent measurement of why the report carries `ece_probability` beside `ece`**: on Laya the wire's `confidence` has an error of 0.307 where the answer's probability has 0.072.

## What recurs

1. **Accuracy on single items does not predict how a model behaves in use.** The evidence comes from three papers:
   - In JevAgentHarness, the model most accurate on isolated questions solved 69.5% of agent tasks.
   - In DecisionPivot, two models 2.6 points apart score 26 points apart on contrast groups.
   - In Fast Models, Slow Evidence, accuracy at the gate (58% against 98%) had been presented as end-to-end quality.
2. **A bar tuned on the cases it is graded on looks better than it is.** A 5% target miss rate became 17% held out. JevAgentHarness fits a bar for each choice and loses 19 points without one.
3. **A probability can rank items well and still sit in the wrong place against a fixed bar.** Jev's binary probabilities do; Deußer et al. tune each bar. "Calibrated" depends on the primitive. The hallucination study puts a number on confidence as a gate: an AUROC of 0.67 to 0.73 across five models, with ECE 0.083 to 0.136, so a confidence bar sorts right from wrong only weakly, whichever model answers.
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
| `jud eval` does not say when it grades the cases the bars were tuned on. The rubric's `tuning.cases` names them, so the check is a comparison of fingerprints. | 5% → 17% (2610.02267); accuracy on one task says little about another, 61.8% to 82.5% across five kinds of hallucination (2610.37229) | `judgment-q1l` |
| Calibration is one number across primitives, read from the server's `confidence` rather than from the answer's probability, and a Brier score assumes a distribution that sums to 1. Sys1Cal's missing "unknown" share shows up there as miscalibration. | 2609.35342, 2609.37647 | evidence added to `judgment-bsz.20.14` and to the decision `judgment-bsz.21.4` (HD4) |
| Gate and tune methodology: whether a bar is judged on the cases it was tuned on. | 2610.02267, 2610.21412 | evidence added to `judgment-bsz.21.14` (HD14) |
| Cases cannot be grouped, so a near-identical pair is graded as two independent items. | 70.1% against 44.2% (2610.28522) | `judgment-xiv`, a `.jud` minor and a decision record |

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
