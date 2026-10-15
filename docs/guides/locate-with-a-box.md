---
title: Locate something with a Box question
description: How to ask where something is in an image with a Box question and read the box in fractions or pixels, why the Box is gated by a Noul beside it and a confidence bar, how to label cases with boxes and grade them by intersection over union with jud eval, and how to pad and crop from the answer, from the shell and in Rust.
status: current
last_reviewed: 2026-10-15
tags: [judgment, box, images, iou, how-to, jud, rust]
---

# Locate something with a Box question

**You will** ask a model where something is in an image, read the answer as a box, decide whether to trust it, and grade a set of such answers against labelled boxes. **Prerequisites:** [Ask a question about an image](ask-about-an-image.md); a checkout of the repository for the replayed steps, which need no key. A Box needs Jev 1.14: `jev-preview` on the hosted API until `jev-latest` moves.

## Ask where, and whether

A Box always returns a box, found or not ([Visual judgments](../concepts/visual-judgments.md)), so a rubric asks two questions: a Noul for whether the thing is there and a Box for where. The shapes example does exactly that over synthetic images:

<!-- file: examples/jud/shapes.jud -->
```yaml
# A rubric asked about one image: whether it holds a filled circle, and
# where. Synthetic images (examples/jud/images/, shapes on flat backgrounds),
# so the repository stays generic and the set regenerates byte for byte.
# A Box always answers with a box, found or not, so the Noul beside it says
# whether the box means anything (docs/concepts/visual-judgments.md).
apiVersion: jud/v1
kind: Rubric
metadata:
  name: shapes
  minor: 4
  description: >-
    Whether an image holds a filled circle, and where it is: the Noul gate and
    the Box it guards, on synthetic shapes.
spec:
  questions:
    circle_present:
      type: noul
      instructions: Is there a filled circle in `image`?
      criteria:
        true: one round, filled shape, whatever its colour or size
        false: only squares, rectangles or an empty background
    circle:
      type: box
      instructions: the filled circle in `image`

  # Where an answer becomes an action. The Noul's threshold is the default
  # until `jud tune` reads one off the cases; the Box's bar is the
  # confidence under which a caller pads the box or asks again on a crop.
  policy:
    circle_present:
      threshold: 0.5
      note: a starting point; tune it on the recorded cases
    circle:
      confidence: 0.5
      note: below it, pad the box before cropping, or ask again on a larger crop
```

The image is referenced as `image` in the state and declared by the case or on the command line. Replayed from the committed recordings, with the image's size read from the file so the box is also in pixels:

```sh
echo '{"image": "shape-01"}' \
  | jud --replay examples/recordings/shapes --asset shape-01=examples/jud/images/shape-01.png examples/jud/shapes.jud
```

<!-- transcript: replay shape-01 -->
```json
{
  "circle_present": {
    "verdict": "yes",
    "probability": 0.99
  },
  "circle": {
    "verdict": "located",
    "box": {
      "top": 0.36,
      "bottom": 0.86,
      "left": 0.2,
      "right": 0.67
    },
    "confidence": 0.92,
    "pixels": {
      "left": 64,
      "top": 115,
      "right": 214,
      "bottom": 275
    }
  }
}
```

`located` is the gate letting the box through: the edges as the wire gave them, the confidence, and `pixels` for an image 320 by 320 (`--pixels WxH` names another size, the original a scaled image came from, say). On an image with no circle the Noul says no and the Box is deferred below its bar, with the box it would have given as text:

```sh
echo '{"image": "shape-03"}' \
  | jud --replay examples/recordings/shapes --asset shape-03=examples/jud/images/shape-03.png examples/jud/shapes.jud
```

<!-- transcript: replay shape-03 -->
```json
{
  "circle_present": {
    "verdict": "no",
    "probability": 0.04
  },
  "circle": {
    "verdict": "deferred",
    "fallback": null,
    "nearest": "0.03,0.54,0.02,0.54",
    "confidence": 0.03,
    "bar": 0.5
  }
}
```

## In Rust

```rust no_run
use judgment::{Assets, BoundingBox, Client, Image, Questions, SystemOne};

async fn run() -> judgment::Result<()> {
    let page = Image::read("scans/page-7.png")?;
    let (width, height) = (page.width(), page.height());
    let mut assets = Assets::new();
    assets.insert("page", page)?;
    let state = serde_json::json!({ "page": "page" });

    let mut questions = Questions::new();
    let signed = questions.noul("signed", "Is there a handwritten signature on `page`?", None)?;
    let signature = questions.r#box("signature", "the handwritten signature on `page`")?;

    let client = Client::from_env()?;
    let response = client
        .answer_with_assets(&state, "jev-preview", &questions, &assets)
        .await?;
    let signed = response.get(&signed)?;
    let signature: BoundingBox = response.get(&signature)?;
    // Whether from the Noul, how far to trust the box from its confidence.
    if signed.is_yes(0.6) && signature.confidence_at_least(0.5) {
        // A little margin, then the page's own pixels, for a crop.
        let crop = signature.edges.padded(0.02).to_pixels(width, height);
        println!("crop x {}..{}, y {}..{}", crop.left, crop.right, crop.top, crop.bottom);
    }
    Ok(())
}
```

`BoundingBox` is the typed view: `edges` (a `BoxEdges` with `width`, `height`, `center`, `area`, `contains`, `iou`, `padded`), `confidence` (`None` when the model gave none) and `to_pixels`. `Response::verify` has already refused a box that is not a region, so the edges are inside the image and ordered. A `Fake` scripts one with `Fake::r#box`.

## Label and grade

A case labels a Box with the box the thing is in, as fractions of the image, and a cases document declares each case's image by a path relative to itself:

```yaml
    - id: shape-01
      state: {image: shape-01}
      assets:
        shape-01: {type: image, path: images/shape-01.png}
      expect:
        circle_present: true
        circle: {box: {top: 0.3688, bottom: 0.85, left: 0.1969, right: 0.6781}}
```

An answer is right when its intersection over union with the label reaches `iou_at_least`, 0.5 unless the label says otherwise ([decision 0026](../project/decisions/0026-a-box-is-graded-by-intersection-over-union.md)); the overlap is kept, and `jud eval` reports its mean where a Noul has a Brier score, with each miss's overlap:

```sh
jud eval examples/jud/shapes.jud examples/jud/shapes-cases.jud --replay examples/recordings/shapes
```

<!-- transcript: jud eval examples/jud/shapes.jud examples/jud/shapes-cases.jud --replay examples/recordings/shapes -->
```text
rubric shapes: questions sha256:ce690f12891229b8596a87345f78d4763fefa03a527b1e9d4aa3ca6dc5876081, policy sha256:3909432a57ea86dbd757b9b3b00f1481ef9592185daef2b544f343d2cf0d1169
cases shapes-cases: sha256:f4a368d49c556c625e4a6d74437947c6720bc5d9cc907e909cb8aa42ce649adf, 40 cases
40 requests, model jev-1.14.0

circle_present
  labelled 40, correct 40, accuracy 1.00 (95% interval 0.91 to 1.00)
  brier 0.001, calibration error 0.015, confidence when right 0.99, when wrong -
  outcomes, labelled/answered/right: yes 30/30/30, no 10/10/10
  majority yes, 30 of 40 (0.75)
  gate: acts on 40 of 40, defers 0, accuracy when acted 1.00

circle
  labelled 30, correct 28, accuracy 0.93 (95% interval 0.79 to 0.98)
  brier -, calibration error 0.212, confidence when right 0.75, when wrong 0.49
  mean IoU 0.80 (a box is right at or above its label's IoU bar)
  outcomes, labelled/answered/right: box 30/30/28
  majority box, 30 of 30 (1.00)
  gate: acts on 23 of 40, defers 17, accuracy when acted 0.96

model misses (2)
  circle          shape-00: a box at IoU 0.00 with the label, confidence 0.29
  circle          shape-10: a box at IoU 0.24 with the label, confidence 0.70
```

Twenty-eight of thirty at the bar, a mean overlap of 0.80, and a gate at 0.5 that acts on 23 answers and is right on 22 of them; the two misses say how far off they were. [A Box calibration study on synthetic shapes](../project/verification/box-calibration-study.md) reads these numbers, and the hosted API's behaviour on the edges, in full. `jud tune` sweeps the Box's confidence bar as it does a Choice's; the overlap bar is the label's and is not swept.

## Next

- [Visual judgments](../concepts/visual-judgments.md): why the Box is read beside a Noul.
- [Other shapes](patterns.md#other-shapes): element location for cropping, visual regression review.
- [Tune thresholds](tune-thresholds.md): the confidence bar from the recordings.
