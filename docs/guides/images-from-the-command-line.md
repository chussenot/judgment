---
title: Get verdicts on images from the command line
description: How to get a verdict on a photo or a page with the jud command alone; a rubric that names the image, --asset to send it and --pixels to read a box in pixels, the verdicts a Noul and a Box print and why the Noul is read first, what is refused before any call, images a rubric always sends, labelled photos through jud check, record, eval and tune, and what a real run needs and costs; every transcript replayed from the committed recordings, so no key is needed to follow it.
status: current
last_reviewed: 2026-10-25
tags: [judgment, jud, cli, images, box, how-to]
---

# Get verdicts on images from the command line

**You will** take a rubric that asks about an image, send a photo or a page with it from the shell, read the verdicts it prints, see what is refused before anything is sent, and run the labelled loop (`jud check`, `record`, `eval`, `tune`) over a set of photos. **Prerequisites:** the `jud` command ([Install](../start/install.md)) and a checkout of the repository for the replayed steps, which need no key; [Visual judgments](../concepts/visual-judgments.md) for what an image in a request is. The Rust side of the same is [Ask a question about an image](ask-about-an-image.md) and [Locate something with a Box question](locate-with-a-box.md).

## 1. A rubric that names the image

A rubric asks about an image the way it asks about anything in the state: by naming it. The state carries a *reference*, any text, where the image belongs, and the questions use the same reference in backticks. The stamps rubric asks two questions about one document page, `page`: a Noul for whether there is a round ink stamp on it, and a Box for where.

<!-- file: examples/jud/stamps.jud -->
```yaml
# A rubric asked about one document page: whether it carries a round ink
# stamp, and where. Generated pages (examples/jud/images/page-*.png, drawn by
# scripts/gen-document-pages.py), so the repository stays generic and the
# set regenerates from its seed. The target is small, a tenth of the page's
# width at most, among a round logo, a signature and a barcode that are not
# it. A Box always answers with a box, found or not, so the Noul beside it
# says whether the box means anything (docs/concepts/visual-judgments.md).
apiVersion: jud/v1
kind: Rubric
metadata:
  name: stamps
  minor: 4
  description: >-
    Whether a document page carries a round ink stamp, and where it is: the
    Noul gate and the Box it guards, on generated invoices and delivery notes.
spec:
  questions:
    stamp_present:
      type: noul
      instructions: Is there a round ink stamp on the page `page`?
      criteria:
        true: a round stamp impression, two rings with a word and a date inside, in coloured ink over the printed page
        false: no stamp; a printed logo, a signature, a barcode or a table is not one
    stamp:
      type: box
      instructions: the round ink stamp on the page `page`

  # The same bars as the shapes rubric, so the two studies compare: the
  # Noul's threshold is the default until `jud tune` reads one off the cases;
  # the Box's bar is the confidence under which a caller pads the box or asks
  # again on a crop.
  policy:
    stamp_present:
      threshold: 0.5
      note: a starting point; tune it on the recorded cases
    stamp:
      confidence: 0.5
      note: below it, pad the box before cropping, or ask again on a larger crop
```

Three things to copy from it. The questions name the image by its reference and nothing else about it: no path, no size, no format. A Box has `instructions` and no `criteria`, and it is gated by `confidence` or `bands`, never by a threshold. And a Box has a Noul beside it, because a Box always answers with a box, found or not; the Noul says whether to use it (step 3). A Noul, a Choice or a Score about an image is answered by Jev 1.13 and later; a Box needs Jev 1.14 or later, which `jev-latest` points to as of 2026-10-25. `minor: 4` is the format minor that reads a Box and images ([The .jud format](../reference/jud-format.md)).

## 2. Send the image with `--asset`

`--asset REF=PATH` adds the image under the reference the state uses; the file is read, held to the API's limits, base64-encoded and sent with the header the API requires, and nothing else in the request changes (unless the configuration file sets `shared_images` for Ollama or Liquid: [Configure a backend](configure-a-backend.md)). Replayed from the committed recordings, so the step needs no key:

```sh
echo '{"page": "page-00"}' \
  | jud --replay examples/recordings/stamps --asset page-00=examples/jud/images/page-00.png examples/jud/stamps.jud
```

<!-- transcript: replay page-00 -->
```json
{
  "stamp_present": {
    "verdict": "yes",
    "probability": 0.99
  },
  "stamp": {
    "verdict": "located",
    "box": {
      "top": 0.83,
      "bottom": 0.88,
      "left": 0.41,
      "right": 0.49
    },
    "confidence": 0.9,
    "pixels": {
      "left": 246,
      "top": 647,
      "right": 294,
      "bottom": 686
    }
  }
}
```

One verdict per question, keyed by the question's id, in the rubric's order ([Verdicts](../reference/cli.md#verdicts)). The Noul's is `yes` with the probability it was read from. The Box's is `located`: the gate let the box through, with `box` as the wire gave it, four edges as fractions of the page's height and width from its top-left corner, `confidence`, and `pixels`, the same box in the pixels of the file, because the request had one image and its size was read from it. A crop is `pixels` plus a margin; the study behind this rubric found a quarter of the box each side enough for two stamps in three, half for all but one ([A Box calibration study on document pages](../project/verification/box-calibration-study-pages.md)).

`--pixels WxH` names another size, when the file sent was scaled down from the original the crop will be cut from. The fractions do not change; the pixels do:

```sh
echo '{"page": "page-00"}' \
  | jud --replay examples/recordings/stamps --asset page-00=examples/jud/images/page-00.png --pixels 1200x1560 examples/jud/stamps.jud
```

<!-- transcript: replay page-00 --pixels 1200x1560 -->
```json
{
  "stamp_present": {
    "verdict": "yes",
    "probability": 0.99
  },
  "stamp": {
    "verdict": "located",
    "box": {
      "top": 0.83,
      "bottom": 0.88,
      "left": 0.41,
      "right": 0.49
    },
    "confidence": 0.9,
    "pixels": {
      "left": 492,
      "top": 1295,
      "right": 588,
      "bottom": 1373
    }
  }
}
```

`--asset` repeats for a request with several images, each under its own reference (a rendered page and its reference rendering, say, for a Score of how different they are); with several images, or none, a box is printed as fractions only unless `--pixels` says whose pixels. The reference is any text, but it should be text that cannot occur in the state by accident: `page-00` or `img-3f2a`, not `page`, which also matches inside `pages`.

Without `--replay` the same command asks the configured backend ([step 7](#7-against-a-real-model)). The replay answers because the recording is keyed by the page's bytes as well as the state: the same file under the same reference is the same request, a re-encoded or resized copy is a request nobody recorded.

## 3. Read the Noul before the Box

A Box does not say whether the thing is there. On a page with no stamp the Noul says no, and the Box still answers with a box, here on the round logo in the header, at a confidence under the gate's bar, so it is `deferred`: the box it would have given as `nearest` (`top,bottom,left,right`), the confidence and the bar it fell under, and `fallback` always `null` for a Box, which has no option to fall back to.

```sh
echo '{"page": "page-02"}' \
  | jud --replay examples/recordings/stamps --asset page-02=examples/jud/images/page-02.png examples/jud/stamps.jud
```

<!-- transcript: replay page-02 -->
```json
{
  "stamp_present": {
    "verdict": "no",
    "probability": 0.06
  },
  "stamp": {
    "verdict": "deferred",
    "fallback": null,
    "nearest": "0.02,0.09,0.05,0.14",
    "confidence": 0.38,
    "bar": 0.5
  }
}
```

The bar does not always catch it. On another page with no stamp the Box's confidence in the logo crosses 0.5 and the verdict is `located`, with pixels, exactly as on a page with a stamp; only the Noul says there is nothing to crop:

```sh
echo '{"page": "page-08"}' \
  | jud --replay examples/recordings/stamps --asset page-08=examples/jud/images/page-08.png examples/jud/stamps.jud
```

<!-- transcript: replay page-08 -->
```json
{
  "stamp_present": {
    "verdict": "no",
    "probability": 0.07
  },
  "stamp": {
    "verdict": "located",
    "box": {
      "top": 0.02,
      "bottom": 0.09,
      "left": 0.04,
      "right": 0.15
    },
    "confidence": 0.53,
    "pixels": {
      "left": 24,
      "top": 16,
      "right": 90,
      "bottom": 70
    }
  }
}
```

So a script reads the two together, in this order: act on the box when the Noul's verdict is `yes` and the Box's is `located`; when the Noul is `yes` and the Box `deferred`, pad `nearest` before cropping or ask again on a larger crop; when the Noul is `no`, there is nothing to crop whatever the Box said. With `jq`:

```sh
jq -r 'if .stamp_present.verdict == "yes" and .stamp.verdict == "located"
       then .stamp.pixels | "\(.left) \(.top) \(.right) \(.bottom)"
       else "no crop" end'
```

Why the Box's confidence is not a presence signal, and how often it crossed the bar on nothing, is in [the study](../project/verification/box-calibration-study-pages.md#confidence-against-the-overlap-and-against-nothing).

## 4. What is refused before any call

Every refusal below is status 2, on stderr, before a backend is opened, so a wrong flag costs no call and no key is needed to see it.

| What was wrong | What `jud` says |
|---|---|
| `--asset page-00`, no `=` | `--asset takes REF=PATH, the reference the state uses and the image's file: `page-00`` |
| A file that is not there | `--asset page-00: cannot access image scans/nowhere.png: No such file or directory (os error 2)` |
| A file outside the limits (here 128 by 128) | `--asset p: invalid image: tests/fixtures/images/small-128.png: 128 by 128 pixels is 128 pixels wide; each side must be 256 to 1568` |
| A reference the state does not carry | `--asset scan is referenced nowhere in the state or the questions; write the reference where the image belongs, or pass the reference the state carries` |
| `--pixels 300`, not `WxH` | `--pixels takes the image's size as WIDTHxHEIGHT, in pixels: `300`` |

The limits are the hosted API's, checked when the file is read: a PNG or a JPEG, not animated, each side 256 to 1568 pixels, 78,400 to 2,458,624 pixels in all, at most 16 MiB a file and 32 MiB a request ([Visual judgments](../concepts/visual-judgments.md#what-is-checked-before-anything-is-sent) has the list). The last two rows are the two the API would answer with a 400 after the upload; `jud` answers them first. A reference typed one way in the state and another on the flag is the common one.

## 5. Images the rubric always sends

An image every request sends, a reference rendering or a template page to compare against, goes in the rubric rather than on the command line, under `spec.assets`, by a path relative to the rubric file. It is loaded from the rubric's directory on every run and merged with what `--asset` adds; a reference in both must name the same bytes.

```yaml
spec:
  assets:
    template: {type: image, path: images/template.png}
  questions:
    layout_changed:
      type: score
      instructions: How far does `page` depart from `template`?
      levels: [the same, a line moved, a block moved, another layout]
```

The reader holds a rubric's references to its questions when the file is read, so a declared image nothing asks about is refused by `jud check` before any run ([Assets](../reference/jud-format.md#assets)). A small image can be inline, as `format` and `data_base64`; a file is the better record of what was sent.

## 6. The loop over labelled photos

The loop that tunes a text rubric ([Tune thresholds](tune-thresholds.md)) is the same over photos, and the commands take no flag: a cases document declares each case's image beside its state, by a path relative to the cases file, and labels the Box with the box the thing is in, as fractions of the image.

```yaml
    - id: page-00
      state: {page: page-00}
      assets:
        page-00: {type: image, path: images/page-00.png}
      expect:
        stamp_present: true
        stamp: {box: {top: 0.8154, bottom: 0.8769, left: 0.4117, right: 0.4917}}
```

A case without the thing labels the Noul `false` and leaves the Box unlabelled: a Box with nothing to find has no right box. `jud check` reads both documents, binds the cases to the rubric and holds every label to its question, without opening an image:

```sh
jud check examples/jud/stamps.jud examples/jud/stamps-cases.jud
```

<!-- transcript: jud check examples/jud/stamps.jud examples/jud/stamps-cases.jud -->
```text
rubric    examples/jud/stamps.jud: name stamps, jud/v1 minor 4, 2 questions, 2 gates (untuned)
          questions sha256:48a1abd6bc3d940f2eccd273aef4bb1819a4b89b5ad5003963eb87a4bd8fb6ff
          policy    sha256:028e64e4bd908c960e58b252ce0017e85f4634c7c3ad6eda8cddb4882df16955
cases     examples/jud/stamps-cases.jud: name stamps-cases, jud/v1 minor 4, 40 cases, bound to examples/jud/stamps.jud
          cases     sha256:471b7ae2709272937c5926304df1c7bcf8f366f68bd38cd1ee7a5434d49babfe
2 documents, 0 refused
```

`jud record` asks a backend once per case and writes a recording per answer; `--dry-run` first loads every image, so a file that is missing or outside the limits is refused by its case's name before anything is spent, and says what the run would ask. Over the committed directory there is nothing left to ask:

```sh
jud record examples/jud/stamps.jud examples/jud/stamps-cases.jud --out examples/recordings/stamps --dry-run
```

<!-- transcript: jud record examples/jud/stamps.jud examples/jud/stamps-cases.jud --out examples/recordings/stamps --dry-run -->
```text
40 requests: 40 already recorded in examples/recordings/stamps, 0 to ask (0 replacing a stale recording)
backend https://api.typesafe.ai, model jev-latest (from default)
long context: off
API key: missing; the run would stop before the first call (a local server that ignores it takes any word)
time: nothing to ask
```

A run of your own goes to a fresh directory and needs a key, and a Box needs Jev 1.14 or later ([step 7](#7-against-a-real-model)); the recordings are keyed by each image's digest as well as the state, so a photo that is re-encoded or resized is asked again, and `--refresh` asks every case again. Then `jud eval` grades the recordings against the labels, a Box by intersection over union at 0.5 unless the label says `iou_at_least` ([decision 0026](../project/decisions/0026-a-box-is-graded-by-intersection-over-union.md)), with the mean overlap where a Noul has a Brier score and each miss's overlap:

```sh
jud eval examples/jud/stamps.jud examples/jud/stamps-cases.jud --replay examples/recordings/stamps
```

<!-- transcript: jud eval examples/jud/stamps.jud examples/jud/stamps-cases.jud --replay examples/recordings/stamps -->
```text
rubric stamps: questions sha256:48a1abd6bc3d940f2eccd273aef4bb1819a4b89b5ad5003963eb87a4bd8fb6ff, policy sha256:028e64e4bd908c960e58b252ce0017e85f4634c7c3ad6eda8cddb4882df16955
cases stamps-cases: sha256:471b7ae2709272937c5926304df1c7bcf8f366f68bd38cd1ee7a5434d49babfe, 40 cases
40 requests, model jev-1.14.0

stamp_present
  labelled 40, correct 40, accuracy 1.00 (95% interval 0.91 to 1.00)
  brier 0.007, calibration error 0.040, confidence when right 0.96, when wrong -
  outcomes, labelled/answered/right: yes 30/30/30, no 10/10/10
  majority yes, 30 of 40 (0.75)
  gate: acts on 40 of 40, defers 0, accuracy when acted 1.00

stamp
  labelled 30, correct 24, accuracy 0.80 (95% interval 0.63 to 0.90)
  brier -, calibration error 0.096, confidence when right 0.81, when wrong 0.81
  mean IoU 0.65 (a box is right at or above its label's IoU bar)
  outcomes, labelled/answered/right: box 30/30/24
  majority box, 30 of 30 (1.00)
  gate: acts on 32 of 40, defers 8, accuracy when acted 0.79

model misses (6)
  stamp          page-09: a box at IoU 0.46 with the label, confidence 0.85
  stamp          page-23: a box at IoU 0.46 with the label, confidence 0.86
  stamp          page-31: a box at IoU 0.49 with the label, confidence 0.94
  stamp          page-33: a box at IoU 0.45 with the label, confidence 0.78
  stamp          page-34: a box at IoU 0.45 with the label, confidence 0.87
  stamp          page-37: a box at IoU 0.49 with the label, confidence 0.59
```

Read the six misses before the accuracy: every one is at an overlap of 0.45 to 0.49, a box on the stamp and a little wide, not a box somewhere else, which is what a small target does to the 0.5 bar. A label for a target that small can say `iou_at_least: 0.4` and why, and a crop that pads the box does not care. `jud tune … --replay DIR` sweeps the Noul's threshold and the Box's confidence bar over the labelled cases; the overlap bar is the label's and is not swept, and pages with nothing to find carry no Box label, so the sweep cannot see what the bar does on them ([the study](../project/verification/box-calibration-study-pages.md) says what it did here).

## 7. Against a real model

Drop `--replay` and the same command asks the configured backend. For the hosted API that is a key. A Box needs Jev 1.14 or later, which `jev-latest`, the default, points to as of 2026-10-25; a model pinned to `jev-1.13.0` refuses it. Pin the version once its bars are tuned, so that the next release does not move them ([Models](https://docs.typesafe.ai/models)):

```sh
export TYPESAFE_API_KEY=...
printf 'model: jev-1.14.0\n' > ~/.config/jud/config.yaml    # the release the recordings name; jev-latest moves with the next one
jud config                                                  # "model": "jev-1.14.0", "model_from": "config_file", "api_key": "environment"
echo '{"page": "page-00"}' | jud --asset page-00=examples/jud/images/page-00.png examples/jud/stamps.jud
```

A Noul, a Choice or a Score about an image needs no change of model. `jud record` and `jud eval` name the model and the server before their first call, so a run aimed at the wrong one can be stopped ([What `jud record` prints](../reference/cli.md#what-it-prints)). What a run costs is tokens by pixels: a 600 by 780 page is about 1,600 input tokens, a 1200 by 1560 one 5,200, for the same thirty-seven output tokens and a latency under 200 ms either way. Send the smallest image that keeps the target legible, about a hundred pixels across for something a Box is to find; the two calibration studies measured both ([shapes](../project/verification/box-calibration-study.md), [pages](../project/verification/box-calibration-study-pages.md)). The command sends every image as base64 in the JSON body (in the `assets` map, or the shared array `shared_images` asks for); the crate's `multipart` feature is for a Rust caller who has measured the bytes ([Ask a question about an image](ask-about-an-image.md#send-the-images-as-parts)).

## Next

- [Locate something with a Box question](locate-with-a-box.md): the same rubric from Rust, and how a box is padded, cropped and graded.
- [Label cases](label-cases.md) and [Tune thresholds](tune-thresholds.md): the loop the photos join.
- [The jud command line](../reference/cli.md): every flag and exit status; [The .jud format](../reference/jud-format.md#assets): how a document declares an image.
- [Other shapes](patterns.md#other-shapes): screenshots, document pages, moderation, visual regression.
