---
title: 0026 A Box is graded by intersection over union
description: How a Box answer is held against a labelled box in a cases document and a report; right when the intersection over union reaches a bar the label may set and that is 0.5 by default, the overlap kept as a number beside accuracy as the mean IoU, no Brier score, the confidence calibrated against hits as for any question, rather than a centre test, a corner distance or a pixel tolerance.
status: accepted
date: 2026-10-15
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-15
tags: [decisions, jud, eval, box, iou, metrics]
---

# 0026 A Box is graded by intersection over union

## Context and problem statement

A Noul, a Choice and a Score answer in a vocabulary a label can be written in: `yes`, an option key, a level. A Box answers with four edges, and no label is the same four numbers; grading it needs a rule for when a box is close enough to count as right, and a report needs a number that says how close the boxes are on average, since a pass rate alone hides whether the misses were near or wild. The rule decides what `jud eval` counts, what `jud tune` can sweep, and what a cases document has to carry.

## Decision drivers

- One rule, stated once, that another implementation of the format computes identically from the same documents.
- A label that fits the case: a crop can tolerate a loose box; an alignment cannot. The bar should be the labeller's to set, with a default most tasks accept.
- A report that keeps the number, not only the verdict, so a threshold can be chosen from the recordings rather than fixed before them.
- The calibration machinery (Brier score, expected calibration error, confidence when right and when wrong) applied only where it means something.

## Considered options

1. **Intersection over union against a bar**, 0.5 by default, settable per label; the overlap kept as a number, averaged into the report; no Brier score; confidence calibrated against hits.
2. **Centre inside the label**: right when the answer's centre falls in the labelled box.
3. **Corner distance under a tolerance**, in fractions of the image.
4. **Exact match with a pixel tolerance**, at a known image size.

## Decision outcome

Chosen option 1.

- A Box label is `{box: {top, bottom, left, right}}` with an optional `iou_at_least`, 0 to 1; `eval::DEFAULT_IOU_AT_LEAST` is 0.5, the bar object detection benchmarks have used since PASCAL VOC. `Judgment::located` computes `BoxEdges::iou` between the answer and the label, sets `correct` by the bar and keeps the number in `Judgment::iou`; `QuestionMetrics::mean_iou` averages it over the labelled cases, and a miss in `jud eval` says the overlap it reached.
- A Box has no distribution, so no Brier score: `QuestionMetrics` skips it where `iou` is set, and the report prints the mean overlap in its place. The confidence is calibrated as any other question's, against hits under the bar, so `ece`, `confidence_when_right` and `confidence_when_wrong` keep their meaning: a model that says 0.9 for a box that misses is miscalibrated in exactly the sense the page on calibration means.
- `jud tune` sweeps the gate's `confidence` for a Box as for a Choice: the bar at which the located boxes it acts on are right often enough. The IoU bar is the label's, not the gate's; it is not swept.
- The rule is in fractions of the image, so it needs no image size, and a label written for a scaled image holds for the original.

### Consequences

- Good: one number everyone computes the same way, with a default that needs no argument for most crops, and a bar a label raises when the task is stricter.
- Good: the mean overlap tells a near miss from a wild one, which the pass rate alone did not.
- Bad: intersection over union is harsh on small boxes, where a few pixels of offset halve it; a label for a small target lowers `iou_at_least` and says why in its `note`.
- Bad: a box that is right in one dimension and wrong in the other scores the same as one wrong in both; the rule does not say which edge was off. A reviewer reads the miss's two boxes for that.

### Confirmation

`src/eval/mod.rs` tests `Judgment::located` and the metrics without a Brier score; `src/jud/cases.rs` tests the label's shape and the bar's range; `tests/jud.rs` grades a shapes case with a padded box (right) and a box in the wrong corner (wrong, overlap 0); `tests/jud_eval.rs` checks the report's `mean_iou`, the miss's `iou` and the calibration error over two recordings.

## Pros and cons of the options

### Option 1: intersection over union against a bar

- Good, because it is the measure the field grades boxes by, with a known default, and scale-free.
- Good, because the number survives into the report and the bar is the label's.
- Bad, because it is harsh on small targets and silent on which edge was off.

### Option 2: centre inside the label

- Good, because it is trivial to compute and forgiving of size.
- Bad, because a box covering the whole image passes every label; it grades the centre, not the box.

### Option 3: corner distance under a tolerance

- Good, because it says which corner was off.
- Bad, because a tolerance in fractions means different things for a small and a large box, and two corners give four numbers to argue about where one would do.

### Option 4: exact match with a pixel tolerance

- Good, because it is what a pixel-perfect task wants.
- Bad, because it needs the image's size in the document, which the request does not carry, and fails every scaled image; it is `iou_at_least: 0.95` written awkwardly.

## More information

[The .jud format](../../reference/jud-format.md) (Cases) holds the label; `src/eval/mod.rs` (`Judgment::located`, `QuestionMetrics`) and `src/bin/jud/eval.rs` compute and print it; [decision 0025](0025-a-document-names-an-image-by-path.md) says how the image a Box is about is named; [decision 0024](0024-images-are-part-of-the-request.md) put the images on the request.
