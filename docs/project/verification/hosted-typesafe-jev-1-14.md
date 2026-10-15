---
title: Jev 1.14 on the hosted API, images and the Box
description: The live run of 2026-10-15 against api.typesafe.ai with jev-preview (Jev 1.14.0) that verified the crate's image support and its Box primitive against the hosted wire, with the seventeenth live test and forty-odd raw probes; what the hosted API did on each point, where it departs from its own documentation, and what the crate does about each departure.
status: current
last_reviewed: 2026-10-15
tags: [judgment, typesafe, jev, images, box, compatibility, verification]
---

# Jev 1.14 on the hosted API, images and the Box

[The earlier record](hosted-typesafe.md) verified the three text primitives against the hosted API on `jev-1.13.0`. Jev 1.13 also takes images, and Jev 1.14 adds the Box, a question that locates something in one. The crate's support for both ([decision 0024](../decisions/0024-images-are-part-of-the-request.md), the `.jud` format's minor 4) was written against the published pages and the refreshed OpenAPI document, and then run against the server on 2026-10-15 with a real key. This page records that run.

## What was tested, and with what

| | |
|---|---|
| Server | `https://api.typesafe.ai` |
| Model requested | `jev-preview`, which resolved to `jev-1.14.0`; `jev-latest` was still `jev-1.13.0` |
| Crate | `judgment` at the commit this page was added in, with the `http` and `jud` features |
| Live test | `an_image_and_a_box_question_round_trip` in `tests/live.rs`, under the `typesafe` profile: a 320 by 320 PNG through `Client::answer_with_assets` with a Noul and a Box, then the same body raw with and without the images header, each body held to the OpenAPI document |
| Recording run | `jud record` over the forty shapes cases, committed under `examples/recordings/shapes/` ([the calibration study](box-calibration-study.md)) |
| Probes | about twenty raw requests past what the client lets through: the image at five sizes and as a JPEG, dimensions outside the limits, an unreferenced asset, a Box with no image, with `null` instructions, with criteria, a wrong `format`, a Box on `jev-latest`, an image on `jev-latest` |

## What the hosted API does

- **An image round-trips, and the Box answers inside it.** `jev-1.14.0` put the circle of `shape-01.png` at an overlap of 0.96 with the box the generator drew it at, confidence 0.82, with `circle_present` at 0.99. The body the client sends, `assets` under the state with the base64 and the format and the header `typesafe-danger-zone-images: true`, is the body the document describes, and the success body conforms to `SystemOneResponse` with the `box` answer kind.
- **The header is required, and its absence is a 400 that says so.** `api_usage_error`, "Images are experimental. Send the header TypeSafe-Danger-Zone-Images: true to use them." The client sets the header on every request that carries an image or asks a Box, so a caller never sees this.
- **Images work on Jev 1.13; the Box needs 1.14.** A Noul about an image was answered by `jev-1.13.0` under `jev-latest` (863 input tokens, the image counted); a Box under `jev-latest` is a 400 `api_usage_error` with the message "Invalid request." and nothing naming the question type. The crate's Box documentation names `jev-preview` until the alias moves.
- **The dimension limits are a 400, not a 413.** A side under 256 or over 1568, or more than 2,458,624 pixels, is `api_usage_error`, "Invalid image dimensions."; the [Images](https://docs.typesafe.ai/images) page files size limits under 413, which the probes could not reach: a PNG inside the pixel limits is a few megabytes, far under the 16 MiB file limit. The crate checks the dimensions and the 78,400 pixel minimum (which rules out 256 by 256) before anything is sent, as `Error::InvalidImage`, so the 400 is never paid for; the 413 mapping stays for the size limits, untested.
- **An unreferenced asset is refused**, 400 `api_usage_error`, "Invalid image request.", whether it is the only asset or one of two. The client refuses it first, by the reference's name (`Error::InvalidAsset`).
- **A Box without an image is answered**, with a box over nothing at confidence 0.03. The reference page says a Box locates something in an image; the server does not insist on one. The crate sends such a request as written (a rubric's image may come with the case, or `jud RUBRIC --asset`), and the gate catches the answer: this is the strongest reason the Box documentation pairs it with a Noul.
- **`null` instructions and `criteria` on a Box are accepted** and the criteria ignored; a PNG declared `format: jpg` is accepted and sniffed. The crate refuses all three before the wire, since none can mean what the caller meant: the instructions are all a Box is, a Box has no outcomes to describe, and a format that is not the bytes' is a mistake somewhere.
- **Input tokens follow the pixels**: 860 at 320 by 320, 2,400 at 640, 5,760 at 1024, 12,828 at 1568, for the same twenty output tokens and a latency between 190 and 300 ms. [The calibration study](box-calibration-study.md) tables it.

## What the crate changed for it

Nothing in the wire shape, which the OpenAPI refresh of the same day had already settled. Two things in words: the format page no longer says a reader must refuse a Box in a rubric without an image, and the Box's documentation names which alias answers it. The live test is the seventeenth in `tests/live.rs`; `mise run live:typesafe` runs it with the others, asking `jev-preview` for the Box part until `jev-latest` is 1.14.
