---
title: Tune thresholds
description: How to grade a model's recorded answers against labelled cases, read the accuracy, the Brier score and the calibration error per question, sweep each gate's bar and write the tuned policy back into the rubric with its provenance.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, tuning, calibration, metrics, how-to]
---

# Tune thresholds

**You will** replace the guessed bars in a rubric's policy with bars read off a table, and record what they rest on. **Prerequisites:** a rubric and its cases ([Write a rubric](write-a-rubric.md), [Label cases](label-cases.md)); recordings of a model's answers to every case ([Record, replay and test](record-replay-and-test.md)); a checkout and a Rust toolchain for the example. The metrics and the loop are explained in [Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md).

The loop is `examples/jud_calibration.rs`, run here over the repository's own documents with no key and no network:

```sh
cargo run --features jud --example jud_calibration
```

## 1. Read, bind, answer

The example reads `examples/jud/triage.jud` and `examples/jud/triage-cases.jud`, binds the cases to the rubric so a label that does not fit fails before any call, and answers each case from the `.jud` recordings under `examples/recordings/jud_calibration/` through a `Replay`. Against a live model, the `Replay` is a `Recorder` over a `Client`, and nothing else changes.

```text
rubric inbox-triage (questions sha256:fda347c2…, policy sha256:1585094f…), 7 cases (sha256:d752d806…), 13 recordings
```

## 2. Read the verdicts and the metrics

The committed policy's verdicts per case come first, so a surprising bar is seen on the case that trips it; then, per question, the accuracy, the Brier score and, with enough cases, the calibration error:

```text
verdicts through the committed policy:
  refund-angry   actionable=yes(0.97)  desk=billing(0.87)  tone=angry(0.84)
  thanks         actionable=no(0.08)  desk=none_of_these(0.77)  tone=calm(0.93)
  receipt        actionable=no(0.52)  desk=deferred→none_of_these(0.40<0.45)  tone=calm(0.96)
  …

metrics per question:
  actionable   labelled 7  accuracy 0.86  brier 0.090
  desk         labelled 7  accuracy 0.86  brier 0.155
  tone         labelled 6  accuracy 1.00  brier 0.059
```

The example prints the accuracy and the Brier score; `eval::QuestionMetrics::summarise` computes all four below, and a harness prints what it needs.

- **Accuracy** comes with a 95 % Wilson interval (`eval::metrics::wilson_interval`); on seven cases it is wide, and that width is the honest number.
- **The Brier score** (`eval::metrics::brier`) is the multi-class sum of squared differences between the probabilities and the outcome: 0 is perfect, 2 is full confidence in the wrong option. It penalises confident wrong answers most.
- **Expected calibration error** (`eval::metrics::expected_calibration_error`, ten bins) is the gap between how confident the model was and how often it was right. It needs more cases than seven to mean anything.
- **Mean confidence when right and when wrong** says whether the confidence separates the two at all. If it does not, no bar will.

## 3. Sweep each bar

For a Noul, `eval::tuning::threshold_sweep` tables accuracy, precision, recall and F1 at every candidate threshold (`default_thresholds`, 0.05 to 0.95), and `best_threshold` reads off the best F1, ties to the lower bar:

```text
tuning:
  threshold  acc   prec  rec   f1
  0.50       0.86  0.83  1.00  0.91
  0.55       1.00  1.00  1.00  1.00
  …
  0.90       0.86  1.00  0.80  0.89
  best threshold by F1 (ties to the lower): 0.55
```

The one case between 0.50 and 0.55 is the automated receipt, which the model put at 0.52: the threshold moved to keep it out, and the cases document's `note` on that case says why it is there.

For a Choice or a Score, `gate_table` tables accuracy and coverage at every candidate confidence bar (`default_bars`), and `lowest_bar` reads off the lowest bar that keeps a target accuracy over a minimum number of covered cases:

```text
  lowest desk bar with ≥95% accuracy over ≥3 covered: 0.45 (covers 6/7, accuracy 1.00)
```

Coverage is the share of cases the gate acts on rather than defers; the bar trades one for the other, and the policy's `fallback` is what the deferred ones get. Each band of a `bands` gate is read off the same table. For a Score with `level_at_least`, `level_sweep` and `best_level` do the same over the levels.

## 4. Write the policy back

The example puts the gates into the rubric's `policy`, with a `tuning` block naming the cases by fingerprint, the model, the server and the time, and prints the rubric:

```yaml
  policy:
    actionable:
      threshold: 0.55
      note: best F1 on the labelled cases; 0.5 lets the automated receipt through
    desk:
      confidence: 0.45
      fallback: none_of_these
      note: lowest bar at 95% accuracy; covers 6 of 7 labelled cases
  tuning:
    cases: sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
    model: jev-1.13.0
    server: https://api.typesafe.ai
    tuned_at: "2026-10-04T12:00:00Z"
```

The next person to open the file sees what the numbers rest on. The questions' fingerprint is unchanged; the policy fingerprint moved, which is the point.

## When the model moves

A bar is tuned per model version, and a confidence is the model's own formula: a threshold tuned on `jev-1.13.0` says nothing about `tev1:0.8b`. Record the cases again against the new model, run the loop, and read the new bars off the new table; `tuning.model` names which one answered.

## In your own code

The functions above are `judgment::eval::tuning`; `judgment::jud::grade` grades one case against one response into judgments; `eval::QuestionMetrics` summarises them. The example is the reference for wiring them, and `cargo test --all-features` runs its test over the recordings.

## Next

- [Run in CI](run-in-ci.md), so the tuned rubric and its cases are checked on every change.
