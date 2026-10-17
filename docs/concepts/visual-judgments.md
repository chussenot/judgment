---
title: Visual judgments
description: What changes when a request carries an image; the reference that stands for it in the state, the questions a model answers about it (a Noul, a Choice, a Score, and the Box that answers where), why a Box always returns a box and is read beside a Noul, what its confidence means, what an image costs in tokens, and the limits the crate checks before anything is sent.
status: current
last_reviewed: 2026-10-17
tags: [judgment, images, box, concepts, visual]
---

# Visual judgments

A System One request is a state and typed questions about it ([System One](system-one.md)). Since Jev 1.13 the state can include an image, and since Jev 1.14 a question can ask where something is in one. Nothing else changes: the questions are as narrow, the answers as calibrated, the thresholds as much the application's. This page is what is different.

## An image is part of the state

An image travels under `assets`, a map of *reference* to image, beside `state` and `questions`. The reference is text, and it stands for the image wherever it appears as or inside a string in the state or a question's instructions: `{"photo": "img-3f2a…"}` in the state, or "the signature on `page`" in a Box's instructions. The model sees the image where the reference is. An asset nothing refers to is refused by the API, so the crate refuses it first, by name.

The crate treats an image as part of the request's identity ([decision 0024](../project/decisions/0024-images-are-part-of-the-request.md)): the request fingerprint and hash fold each image in by the SHA-256 of its bytes, so a recording is found by the image it answered and two requests that differ only by their image are two recordings. A request without an image is the request it always was, on the wire and in every recording. The `.jud` format names an image by a path relative to the document, or inline for a small one ([Assets](../reference/jud-format.md#assets), [decision 0025](../project/decisions/0025-a-document-names-an-image-by-path.md)).

An image costs input tokens in proportion to its pixels: about one token per 120 pixels at 320 by 320, one per 190 at the largest size the API takes, 1568 by 1568, so the same picture costs fifteen times more at the top of the range than at the bottom, for the same answer ([the calibration study](../project/verification/box-calibration-study.md)). Send the smallest image that keeps the target legible; the latency hardly moves with the size, the bill does.

## Four questions about an image

| Primitive | Asked about an image | Answers with |
|---|---|---|
| Noul | Is the thing there, is the page a form, is this moderation-safe | the probability of yes |
| Choice | Which of these layouts, which document type, which state is the UI in | an option, a distribution, a confidence |
| Score | How blurred, how severe, how far from the reference | a position on levels, a confidence |
| Box | Where is the thing | four edges as fractions of the image, and a confidence |

The first three are the text primitives asked about a picture; everything on [System One](system-one.md) holds for them. The Box is the one new shape: `{top, bottom, left, right}` in fractions of the image's height and width from its top-left corner, so a box written for a scaled copy holds for the original, and a `PixelBox` is one multiplication away when the size is known.

## A Box always returns a box

A Box does not say whether the thing is there. Asked for the filled circle in an image with no circle, Jev 1.14 returns a box all the same, with a confidence near zero ([the calibration study](../project/verification/box-calibration-study.md): 0.02 to 0.04 on ten empty images); asked for a round stamp on a page that has none but has a round logo, it returns a box on the logo at a confidence of 0.4 to 0.6, over the gate's bar on three pages of ten ([the second study](../project/verification/box-calibration-study-pages.md)), so the confidence says nothing about presence once something resembles the target. That is the primitive's contract, not a defect: a Box answers *where*, and *whether* is a Noul's question. The crate's documentation and the `.jud` format therefore pair every Box with a Noul beside it, and the application reads the two together: the Noul says whether to use the box, the Box's confidence says how far to trust it, and a gate on each turns the pair into an action (locate, pad and crop; ask again on a larger crop; send to a person). A Box's `confidence` may be absent on the wire; the crate reads it as `None`, and a gate reads an absent confidence as 0.

The confidence is informative in both directions on the study's images: near zero when there was nothing, lower on the boxes that were off, and under-confident in the middle. It is not a probability that the box is right; it is measured the same way a Choice's is, against hits under a bar, and `jud eval` reports its calibration error.

## How a box is graded

Two boxes are compared by intersection over union, the overlap divided by the area they cover together, 0 to 1. A labelled case carries the box and, when 0.5 is not the bar the task needs, `iou_at_least`; an answer at or over the bar is right, and the overlap is kept as a number, averaged into a report where a Noul or a Choice has a Brier score ([decision 0026](../project/decisions/0026-a-box-is-graded-by-intersection-over-union.md)). The bar is harsh on small targets, where a few pixels of offset halve the overlap; a label for a small target lowers it and says why.

## What is checked before anything is sent

An image is a PNG or a JPEG, not animated, each side from 256 to 1568 pixels, 78,400 to 2,458,624 pixels in all, at most 16 MiB, and sent as it is, so upright: an EXIF orientation tag is not read, and a photo a camera stored sideways is rotated before it is given to the crate; a request's images weigh at most 32 MiB together; a reference is 1 to 1024 bytes and all of a request's at most 65,536. The crate checks every one when the image is built or added, as `Error::InvalidImage`, `Error::InvalidAsset` or `Error::RequestTooLarge`, so the refusal names the limit and costs no upload; the hosted API refuses the same with a 400 ([Jev 1.14 on the hosted API](../project/verification/hosted-typesafe-jev-1-14.md)). The header the API keeps the feature behind is the client's to send, and it sends it on every request that carries an image or asks a Box.

## Next

- [Get verdicts on images from the command line](../guides/images-from-the-command-line.md), [Ask a question about an image](../guides/ask-about-an-image.md), [Locate something with a Box question](../guides/locate-with-a-box.md).
- [Other shapes](../guides/patterns.md#other-shapes): screenshots, document pages, moderation, visual regression, cropping.
- [A Box calibration study on synthetic shapes](../project/verification/box-calibration-study.md): the numbers behind this page.
- [A Box calibration study on document pages](../project/verification/box-calibration-study-pages.md): the same questions on a small target among distractors, at two sizes.
