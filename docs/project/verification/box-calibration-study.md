---
title: A Box calibration study on synthetic shapes
description: Forty synthetic 320 by 320 images, thirty with one filled circle beside a distractor square and ten without, asked a Noul and a Box by Jev 1.14.0 through jud record on 2026-10-15; the Noul's accuracy and calibration, the Box's accuracy at the 0.5 intersection-over-union bar, the distribution of the overlap, the confidence against the overlap and what a confidence gate at 0.5 acts on, the same image at four scales against latency, input tokens and the box, and which requests the hosted API refuses; the recordings are committed, so every number replays with no key.
status: current
last_reviewed: 2026-10-15
tags: [judgment, verification, box, images, calibration, iou, jev]
---

# A Box calibration study on synthetic shapes

[The .jud format](../../reference/jud-format.md) grades a Box by intersection over union against a labelled box ([decision 0026](../decisions/0026-a-box-is-graded-by-intersection-over-union.md)) and reports the confidence the model gives beside it. Whether the two numbers mean anything, and what a gate on the confidence buys, is a question for recordings, not for a page. This is the first such run: small, synthetic and committed, so that it replays with no key, and so that the next run has a baseline to be set against. [The second run](box-calibration-study-pages.md), on generated document pages with a stamp a tenth of the page wide among a logo, a signature and a barcode, is set against it.

## What was run

| | |
|---|---|
| Images | `examples/jud/images/shape-00.png` to `shape-39.png`: forty 320 by 320 PNGs drawn with Pillow from one seed, each a flat background with, in thirty of them, one filled circle of random size, colour and position and one distractor square; in ten, squares only. The generator's own coordinates are the labels, so no labeller's judgment is in them. |
| Rubric | `examples/jud/shapes.jud`: `circle_present`, a Noul with its two criteria, and `circle`, a Box asking for the filled circle in `image`; the policy gates the Noul at 0.5 and the Box at a confidence of 0.5 |
| Cases | `examples/jud/shapes-cases.jud`: forty cases, each naming its image by a path relative to the document and labelled `true` with the circle's box, or `false` with no box |
| Server, model | `https://api.typesafe.ai`, `jev-preview`, answered by `jev-1.14.0` |
| Run | `jud record examples/jud/shapes.jud examples/jud/shapes-cases.jud --out examples/recordings/shapes` on 2026-10-15; forty requests in about seven seconds, 93 to 532 ms each, median 119 ms, 625 input tokens and 37 output tokens a request |
| Report | `jud eval examples/jud/shapes.jud examples/jud/shapes-cases.jud --replay examples/recordings/shapes`, replayed in `tests/jud.rs`, which pins the counts below |

## The Noul: forty of forty, calibrated

| | |
|---|---|
| Labelled, right | 40, 40 (accuracy 1.00, 95 % interval 0.91 to 1.00) |
| Brier score, calibration error | 0.001, 0.015 |
| Probability of yes | 0.99 or 1.00 on every image with a circle; 0.03 or 0.04 on every image without |
| Gate at 0.5 | acts on 40, defers 0, accuracy when acted 1.00 |

A filled circle on a flat background is the easiest question the model can be asked, and it answers it as a calibrated model should: the probability sits at the ends, and the ten negatives are as confidently no as the thirty positives are yes. The number to carry forward is not the accuracy but the Brier score: on a target this clean, 0.001 is the floor a harder set is measured from.

## The Box: twenty-eight of thirty at the 0.5 bar

| | |
|---|---|
| Labelled, right | 30, 28 (accuracy 0.93, 95 % interval 0.79 to 0.98) |
| Mean intersection over union | 0.80 |
| Confidence when right, when wrong | 0.75, 0.49 |
| Calibration error, over the confidence against hits | 0.21 |
| Brier score | none: a Box has no distribution to score |

Where the overlap fell, with the model's mean confidence in each band:

| Intersection over union | Cases | Mean confidence | Which |
|---|---|---|---|
| 0.0 up to 0.5 | 2 | 0.49 | 00, 10 |
| 0.5 up to 0.7 | 2 | 0.86 | 06, 34 |
| 0.7 up to 0.8 | 4 | 0.73 | 18, 22, 36, 37 |
| 0.8 up to 0.9 | 14 | 0.73 | 05, 08, 09, 12, 14, 16, 17, 21, 24, 28, 29, 30, 32, 33 |
| 0.9 to 1.0 | 8 | 0.77 | 01, 02, 04, 13, 20, 25, 26, 38 |

The two misses are worth reading. `shape-00` (overlap 0.00, confidence 0.29) is the case where the model took the circle's rows and the distractor square's columns: the box spans the circle's height at the square's position, a box over nothing. `shape-10` (overlap 0.24, confidence 0.70) is a box on the right circle, too large on one side. The first is the kind of miss a confidence gate catches and the second is the kind it does not: at the 0.5 bar the gate defers the first and acts on the second.

## Confidence against the overlap

| Confidence | Cases | Right at 0.5 | Mean overlap |
|---|---|---|---|
| 0.0 up to 0.3 | 3 | 2 | 0.58 |
| 0.3 up to 0.5 | 4 | 4 | 0.83 |
| 0.5 up to 0.7 | 2 | 2 | 0.80 |
| 0.7 up to 0.9 | 12 | 11 | 0.79 |
| 0.9 to 1.0 | 9 | 9 | 0.86 |

On the ten images with no circle, the Box's confidence was 0.02 to 0.04: the box it returns there (it always returns one) comes with a confidence that says, by itself, that there was nothing to find. In this set the Box's confidence alone separates present from absent as cleanly as the Noul does; a harder set may not, and the rubric keeps the Noul beside the Box because the format's rule is that the Box answers where, never whether.

What the policy's gate at 0.5 does with the forty answers: it acts on 23 (22 right, accuracy when acted 0.96) and defers 17: the ten empty images, one miss and 6 hits the model was unsure of (overlap 0.77 to 0.92 at confidence 0.27 to 0.48). The calibration error of 0.21 is the model's under-confidence on those: it was right at 0.3 to 0.5 more often than the number says. A caller that pads a deferred box and crops anyway loses little here; one that asks again on a larger crop spends a call on an answer that was already right in six cases of seven. `jud tune` would move the bar down on these recordings; the rubric's note says the bar is a starting point, and thirty cases are too few to move it on.

## Scale against latency, tokens and the box

The same image, `shape-01.png`, sent raw at five sizes and as a JPEG, with the same Box question, one request each:

| Image | Bytes | Status | Latency (ms) | Input tokens | Overlap with the label | Confidence |
|---|---|---|---|---|---|---|
| 256x256 png | 1294 | 400 | 424 | - | - | - |
| 320x320 png | 1645 | 200 | 194 | 860 | 0.93 | 0.81 |
| 640x640 png | 3232 | 200 | 234 | 2400 | 0.93 | 0.82 |
| 1024x1024 png | 6130 | 200 | 221 | 5760 | 0.92 | 0.69 |
| 1568x1568 png | 11676 | 200 | 298 | 12828 | 0.91 | 0.69 |
| 320x320 jpg q90 | 5530 | 200 | 196 | 860 | 0.95 | 0.80 |

Input tokens grow with the pixels, about one token per 120 pixels at 320 by 320 and one per 190 at 1568 by 1568, so the largest image costs fifteen times the smallest; latency barely moves, since the answer is twenty tokens whatever the image. The box is as good at 320 as at 1568 on a target this large, and the confidence falls as the image grows. 256 by 256 is refused: each side is at the minimum but the product, 65,536 pixels, is under the 78,400 the API requires, which the crate's `Image` checks before anything is sent. The one PNG that stays within the pixel limits is at most a few megabytes, so the 16 MiB file limit and the 32 MiB request limit cannot be reached with a valid image; nothing was sent that would have been a 413.

## What the hosted API refused, and what it did not

| Request | Status | Body |
|---|---|---|
| A side under 256, a side over 1568, more than 2,458,624 pixels | 400 | `api_usage_error`, "Invalid image dimensions." |
| An asset nothing in the state or the questions refers to | 400 | `api_usage_error`, "Invalid image request." |
| An image without the `TypeSafe-Danger-Zone-Images` header | 400 | `api_usage_error`, "Images are experimental. Send the header …" |
| A Box on `jev-latest` (Jev 1.13.0) | 400 | `api_usage_error`, "Invalid request." |
| A Box with no image in the request | 200 | a box over nothing at confidence 0.03 |
| A Box with `instructions: null` | 200 | a box, at confidence 0.75 |
| A Box with `criteria` | 200 | the criteria ignored |
| A PNG declared `format: jpg` | 200 | the bytes sniffed |
| An image referenced by a Noul alone, on `jev-latest` | 200 | answered by `jev-1.13.0` |

The crate refuses before the wire what the API refuses (the dimensions, the missing reference), and also what the API lets through but cannot mean anything (`null` instructions, criteria on a Box, a format that is not the bytes'); it sends a Box without an image, since the image may come with the request, and the gate catches the answer. [Jev 1.14 on the hosted API](hosted-typesafe-jev-1-14.md) is the verification record these probes belong to.

## What to take from it, and what not

- A Box on a clear target is accurate at the 0.5 bar and its overlap is high (0.80 on average); the bar is a floor, not a target, and a crop that needs the box tight sets `iou_at_least` higher in the label.
- The confidence is informative in both directions: near zero when there is nothing, and lower on the boxes that were off. It is under-confident in the middle here, and `jud eval`'s calibration error is the number to watch when the target gets harder.
- Send the smallest image that keeps the target legible: the tokens are the cost, the latency is not, and the box did not improve with size.
- Thirty labelled boxes give an interval of 0.79 to 0.98 on the accuracy: wide. The set is a baseline and a regression check (`tests/jud.rs` fails if a re-recording changes the counts), not a measurement of the model.

To repeat it against the live API: `TYPESAFE_API_KEY` in `.env`, `model: jev-preview` in `jud`'s configuration file ([Configuration](../../reference/configuration.md)), then the `jud record` line above with `--refresh` into a scratch directory, and `jud eval` with `--replay` pointed at it. To regenerate the images, the generator is Pillow drawing from a fixed seed, which is why the labels are exact; the committed PNGs are the ones the recordings were made on, and a redrawn set changes every request fingerprint.
