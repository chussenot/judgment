---
title: A Box calibration study on document pages
description: Forty generated invoices and delivery notes, thirty with one round ink stamp a tenth of the page wide among a round logo, a signature and a barcode, ten without, asked a Noul and a Box by Jev 1.14.0 through jud record on 2026-10-15 at 600 by 780 pixels and again at twice that; set against the synthetic-shapes baseline for the Box's accuracy at the 0.5 intersection-over-union bar, what the misses look like on a small target, what the confidence says about the box and about presence, what the gate acts on, what a padded crop would contain, what resolution buys for its tokens, and what jud tune makes of it; the first run's recordings are committed, so its numbers replay with no key.
status: current
last_reviewed: 2026-10-15
tags: [judgment, verification, box, images, calibration, iou, jev, resolution]
---

# A Box calibration study on document pages

[The first study](box-calibration-study.md) asked Jev 1.14 for a filled circle on a flat background: a target a sixth to a half of the image wide, with nothing else round in it. It ended with the questions a harder set should answer: how the overlap falls on a small target, whether the confidence still tracks the overlap, what the gate's accuracy becomes when it acts, and whether the under-confidence it found between 0.3 and 0.5 holds. This is that run, on something closer to a document a service would crop: a generated invoice with a round ink stamp a tenth of the page wide, over printed text, among three things that are round, inked or black and are not a stamp. The pages are drawn, not photographed, for the same reason the shapes were: the generator's coordinates are the labels, the set regenerates from one seed, and nothing in it belongs to anyone.

## What was run

| | |
|---|---|
| Pages | `examples/jud/images/page-00.png` to `page-39.png`: forty 600 by 780 pixel pages drawn by `scripts/gen-document-pages.py` from one seed, each an invoice, delivery note, purchase order or receipt with a round two-letter logo in the header, two address blocks, a table of five to nine lines, a totals block, a signature scribble and a barcode. Thirty carry one stamp: two rings with a word (`RECEIVED`, `PAID`, …) and a date inside, 48 to 72 pixels across (median 60, 8 to 12 % of the width, under 1 % of the area), rotated up to 28°, in red, blue, green or purple ink at 78 % opacity, anywhere under the header, text and table included; ten carry none. Saved with a 64-colour palette: 12 to 15 KB each. |
| Rubric | `examples/jud/stamps.jud`: `stamp_present`, a Noul whose `false` criterion names the logo, the signature, the barcode and the table as not a stamp, and `stamp`, a Box asking for the round ink stamp on `page`; the policy is the shapes rubric's, a threshold of 0.5 on the Noul and a confidence of 0.5 on the Box, so the two studies compare |
| Cases | `examples/jud/stamps-cases.jud`, written by the generator: forty cases, each naming its page by a path relative to the document and labelled `true` with the stamp's bounding box (the outer ring's, which a rotation keeps), or `false` with no box |
| Server, model | `https://api.typesafe.ai`, `jev-preview`, answered by `jev-1.14.0` |
| Run | `jud record examples/jud/stamps.jud examples/jud/stamps-cases.jud --out examples/recordings/stamps` on 2026-10-15, with `model: jev-preview` in the configuration file; forty requests in six seconds, 114 to 678 ms each, median 124 ms, 1,596 input tokens and 37 output tokens a request |
| A second size | the same forty pages drawn at scale 2 (1200 by 1560 pixels, every coordinate doubled, so the labels in fractions are the same) and recorded the same way into a scratch directory, not committed: 161 to 644 ms, median 179 ms, 5,180 input tokens a request |
| Report | `jud eval … --replay examples/recordings/stamps`, replayed in `tests/jud.rs`, which pins the counts below; the per-case tables come from the recordings' boxes against the cases' labels |

## The Noul: forty of forty at both sizes

| | 600 by 780 | 1200 by 1560 |
|---|---|---|
| Labelled, right | 40, 40 (accuracy 1.00, 95 % interval 0.91 to 1.00) | 40, 40 |
| Brier score, calibration error | 0.007, 0.040 | 0.001, 0.020 |
| Probability of yes, with a stamp | 0.75 to 0.99 (one page at 0.75, the next at 0.89) | 0.98 to 0.99 |
| Probability of yes, without | 0.05 to 0.08 | 0.04 to 0.06 |
| Gate at 0.5 | acts on 40, defers 0, right on 40 | the same |

A stamp over text among a logo, a signature and a barcode is a harder question than a circle on a flat background, and the Noul answers it as well: not one of the ten pages without a stamp got more than 0.08, with the round logo in every header. `jud tune` proposes a threshold of 0.10 on these recordings, because every bar from 0.10 to 0.75 is forty of forty and ties go to the lower one; that is the tie rule speaking, not evidence, and the bar stays at 0.5.

## The Box: twenty-four of thirty at the 0.5 bar, thirty of thirty at twice the pixels

| | 600 by 780 | 1200 by 1560 | Shapes, 320 by 320 |
|---|---|---|---|
| Labelled, right at 0.5 | 30, 24 (accuracy 0.80, 95 % interval 0.63 to 0.90) | 30, 30 (1.00, 0.89 to 1.00) | 30, 28 (0.93) |
| Mean intersection over union, median | 0.65, 0.64 | 0.70, 0.71 | 0.80 |
| Confidence when right, when wrong | 0.81, 0.81 | 0.86, none wrong | 0.75, 0.49 |
| Calibration error, over the confidence against hits | 0.096, over-confident | 0.137, under-confident | 0.21, under-confident |
| Target's width, as a share of the image's | 8 to 12 % | 8 to 12 % | 16 to 48 % |
| Input tokens a request | 1,596 | 5,180 | 625 |

Where the overlap fell, with the model's mean confidence in each band:

| Intersection over union | Pages at 600 by 780 | Mean confidence | Which | At 1200 by 1560 | Mean confidence |
|---|---|---|---|---|---|
| 0.0 up to 0.5 | 6 | 0.81 | 09, 23, 31, 33, 34, 37 | 0 | |
| 0.5 up to 0.7 | 12 | 0.75 | 00, 01, 03, 04, 10, 13, 19, 20, 21, 27, 35, 38 | 14 | 0.83 |
| 0.7 up to 0.8 | 10 | 0.87 | 07, 11, 14, 16, 17, 22, 25, 26, 28, 39 | 10 | 0.87 |
| 0.8 up to 0.9 | 2 | 0.86 | 06, 12 | 6 | 0.93 |
| 0.9 to 1.0 | 0 | | | 0 | |

The first thing the small target changes is the shape of the distribution. On the shapes it ran from 0.00 to 0.98, with two misses that were a box over nothing and a box too large on one side; here it runs from 0.45 to 0.85, every box on the stamp and none tight. The six misses are all at 0.45 to 0.49, a hair under the bar, and they look alike:

| Page | Overlap | Confidence | The stamp (label) | The box the model gave | Its centre against the stamp's |
|---|---|---|---|---|---|
| 09 | 0.46 | 0.85 | 48 by 48 px | 36 by 47 px | 9 px left, 10 px down |
| 23 | 0.46 | 0.86 | 72 by 72 | 90 by 62 | 2 px left, 20 px down |
| 31 | 0.49 | 0.94 | 62 by 62 | 90 by 39 | 12 px left, 2 px down |
| 33 | 0.45 | 0.78 | 64 by 64 | 90 by 47 | 13 px left, 15 px down |
| 34 | 0.45 | 0.87 | 70 by 70 | 84 by 62 | 3 px right, 21 px down |
| 37 | 0.49 | 0.59 | 66 by 66 | 78 by 70 | 3 px left, 19 px down |

Five of the six boxes are wider than the ring and shorter than it, as if the model boxed the word across the stamp's middle rather than the ring around it, and five sit 10 to 21 pixels low. On a 60-pixel target a 15-pixel shift alone takes a box of the right size from an overlap of 1.0 to 0.6. Over all thirty, the box covers 82 % of the stamp's area on average (54 % at worst) and 76 % of the box is stamp; a service that pads the box before cropping gets the whole stamp in 20 of 30 crops at a quarter of the box's size each side and in 29 of 30 at half, and at the larger size in 29 and 30 of 30. The 0.5 bar measures the box's precision, not whether the stamp was found: it was found on every page.

## Confidence against the overlap, and against nothing

| Confidence | Pages at 600 by 780 | Right at 0.5 | Mean overlap | At 1200 by 1560 | Right | Mean overlap |
|---|---|---|---|---|---|---|
| 0.0 up to 0.3 | 0 | | | 0 | | |
| 0.3 up to 0.5 | 1 | 1 | 0.64 | 0 | | |
| 0.5 up to 0.7 | 4 | 3 | 0.58 | 1 | 1 | 0.66 |
| 0.7 up to 0.9 | 15 | 11 | 0.63 | 17 | 17 | 0.65 |
| 0.9 to 1.0 | 10 | 9 | 0.69 | 12 | 12 | 0.78 |

The confidence does not tell a near miss from a hit here: 0.81 on average for both, with the worst-overlapping box (page 33) at 0.78 and the box that missed by the least margin in shape (page 31) at 0.94. The under-confidence the first study found between 0.3 and 0.5 cannot be tested on these pages: one answer fell in that band, none at the larger size. On a document the model is sure of its box whether or not the box is tight, and at the larger size slightly more sure of boxes that are tighter (0.78 mean overlap in the top band against 0.65 below it), which is as much relation as there is.

What it says about presence is the finding of this run. On the ten pages with no stamp, the Box put its box on the round logo in the header every time, at a confidence of 0.36 to 0.54 at the smaller size and 0.49 to 0.59 at the larger: over the gate's bar on three pages of ten, then on nine. On the shapes it had given 0.02 to 0.04 on an empty image, and the first study allowed that a harder set might not; this one does not. Once something on the page resembles the target, the Box's confidence stops saying whether the target is there, and a rubric that read the Box alone would crop a logo. The Noul beside it, at 0.05 to 0.08 on the same pages, is what keeps that from happening, which is the format's rule ([Visual judgments](../../concepts/visual-judgments.md)): the Box answers where, never whether.

What the gate at 0.5 does with the forty answers, at 600 by 780: it acts on 32 (29 stamps, of which 23 at or over the bar, and 3 logos) and defers 8 (7 empty pages and one stamp, whose box was right); accuracy when acted 0.79 over the boxes, where the shapes gate was right on 22 of the 23 it acted on. At 1200 by 1560 it acts on 39, every stamp right and nine logos with them. `jud tune`, which sweeps the bar over the labelled boxes, finds no bar that reaches 95 % accuracy over three cases at the smaller size (accuracy sits at 0.79 to 0.81 from a bar of 0 to 0.80, with coverage falling) and leaves the confidence as written; at the larger size it proposes 0, since every labelled box is right with no bar, and says so. Neither sweep sees the logos: a page with no stamp has no box label, so the pages the bar should defer are not in the table the bar is read from. A proposal of 0 for a Box means the labelled boxes are good, not that the gate is needless.

## Resolution, for a small target

The first study sent one image at five sizes and found the box as good at 320 pixels as at 1568, for a target a third of the image wide. On a target a tenth of the page wide, twice the pixels did something:

| | 600 by 780 | 1200 by 1560 |
|---|---|---|
| Stamp, in pixels | 48 to 72 | 96 to 144 |
| Right at the 0.5 bar | 24 of 30 | 30 of 30 |
| Mean overlap | 0.65 | 0.70 |
| The six misses | 0.45 to 0.49 | 0.51 to 0.83 (page 09 from 0.46 to 0.83) |
| Pages whose overlap fell | | 11 of 30 (page 12 from 0.85 to 0.70, 17 from 0.73 to 0.58) |
| Best overlap | 0.85 | 0.88 |
| Input tokens, latency (median) | 1,596, 124 ms | 5,180, 179 ms |

More pixels raised the floor and left the ceiling: no box got tight, the near misses became hits, and a third of the pages lost some overlap. The price is 3.2 times the tokens for 55 ms, so the rule the first study gave stands with a number on it: send the smallest image that keeps the target legible, where legible for a Box means about a hundred pixels across. A page whose stamps are smaller than that is better cropped first, by the Noul's page and a coarse box, then asked again on the crop, than sent whole at the API's largest size.

## What to take from it, and what not

- On a small target the misses are near misses: every box was on the stamp, at an overlap of 0.45 to 0.85. A `iou_at_least` of 0.5 in a label says how tight the application needs the box, not whether the model can find the thing; label it at what the crop needs.
- The Box's confidence is not a presence signal once the page holds something round: 0.4 to 0.6 on a logo, 0.8 on a stamp. Gate on the Noul first, as the format asks, and read the Box's confidence as a measure of the box, which here it barely is.
- Pad before cropping: a quarter of the box each side contained the whole stamp in two crops of three, half of it in all but one. A tight box wants a second call on the crop.
- Pixels buy accuracy on a small target and nothing on a large one; about a hundred pixels across is where the near misses turned into hits here, at three times the tokens.
- `jud tune` sweeps labelled boxes; pages without the target are not in its table. Read a Box proposal as a statement about the boxes it saw.
- Thirty stamps give an interval of 0.63 to 0.90 on the accuracy, and the ten empty pages all carry the same logo in the same corner: a second distractor at another position is the next thing to add. The set is a baseline and a regression check (`tests/jud.rs` fails if a re-recording changes the counts), not a measurement of the model.

To repeat it against the live API: `TYPESAFE_API_KEY` in `.env`, `model: jev-preview` in `jud`'s configuration file ([Configuration](../../reference/configuration.md)), then the `jud record` line above with `--refresh` into a scratch directory and `jud eval` with `--replay` pointed at it. `python3 -I scripts/gen-document-pages.py --out DIR --scale 2` draws the larger set with its cases document; a page drawn with another Pillow or another font is other bytes, so the committed pages are the ones the recordings answer, and a redrawn set is a new run.
