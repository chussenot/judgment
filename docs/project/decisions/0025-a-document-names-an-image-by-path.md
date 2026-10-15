---
title: 0025 A document names an image by a path relative to itself
description: How a .jud rubric or case declares the image a Box question or a state refers to; under spec.assets (and a case's assets) as a path relative to the document, with the bytes inline allowed for a small image, the reference the text the state or the instructions carry, and the file loaded when the request is built, rather than a URL, a separate manifest or the bytes always inline.
status: accepted
date: 2026-10-15
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-15
tags: [decisions, jud, format, images, assets, box]
---

# 0025 A document names an image by a path relative to itself

## Context and problem statement

[Decision 0024](0024-images-are-part-of-the-request.md) put images on the request: an `Assets` map of reference to image, each reference the text the state or a question carries where the image belongs, each image in the request's fingerprint by the SHA-256 of its bytes. The `.jud` format had no way to say which image a case is about or which image a rubric's questions always look at, so a Box question could be written in a rubric but never recorded, replayed, evaluated or tuned from the command line, and a cases document for a visual judgment could label nothing. The format needed a declaration, and the declaration decides what a document can be moved with, what a reviewer sees, and what a fingerprint is over.

## Decision drivers

- A document and its images are reviewed and versioned together; a reviewer must be able to open the image a label is about.
- The fingerprint of a cases document is over its `spec.cases`, and the request fingerprint over the bytes: a document moved to another directory, or with its images re-encoded, must keep the request fingerprints its recordings were keyed by, or say clearly that it did not.
- A reference is what the API resolves; the document should carry the same text, once, where the image belongs, so the request the document builds is the request a caller would write by hand.
- A document must still be readable without its images (`jud check` on a laptop without the image store), and fail by name when an image is missing at the time it is needed.
- Nothing that lets a document read a file outside its own directory, since documents are shared and checked by tools that run on behalf of others.

## Considered options

1. **A path relative to the document, with the bytes inline allowed.** `assets: {ref: {type: image, path: images/x.png}}` beside the document, or `{type: image, format: png, data_base64: …}` for a small image; a reference appears in the state or the instructions as text.
2. **The bytes always inline.** Every image as `data_base64` in the document; nothing to resolve.
3. **A URL.** `{type: image, url: https://…}`, fetched when the request is built.
4. **A separate asset manifest** beside the documents, mapping references to files, named from the rubric or the cases document.

## Decision outcome

Chosen option 1.

- A Rubric's `spec.assets` holds the images every request sends; a case's `assets` holds the image that case is about. The two merge into one request, and a reference in both must name the same bytes. The map is keyed by the reference, exactly as the wire's `assets` is, and the state or the instructions carry the reference where the image belongs; a reader refuses a declared reference that appears in neither, as the API refuses an unreferenced asset with a 400, and refuses a reference over 1024 bytes or references over 65,536 bytes together, the API's own limits.
- `path` is relative to the directory the document was read from; an absolute path or one with a `..` component is refused when the document is read, before any file is touched. `data_base64` with a required `format` is the inline form. Exactly one of the two.
- The files are loaded when a request is built (`Rubric::load_assets`, `Case::load_assets`, by the `jud` command for every subcommand that builds one), not when the document is read, so a document checks without its images and a missing or malformed file fails by the entry's name at the point it is needed. The image's own limits are checked by `Image::read` and `Image::from_base64` as for any request.
- The request fingerprint is over the bytes' digest under the reference, never over the path or the base64: moving the images with the document, or inlining one, changes no fingerprint; replacing the bytes does, and a replay then says so by finding no recording. A cases document's fingerprint is over `spec.cases`, which holds the asset declarations as written (the path or the base64), so the labels' identity changes when the declaration does, which a reviewer wants.
- `jud RUBRIC` takes an image from the command line too, `--asset REF=PATH`, repeatable, beside the rubric's own; `--pixels WxH` names the size a located box is printed in pixels of, the one image's size when there is exactly one.

### Consequences

- Good: a reviewer opens the image beside the document; a repository moves as a whole; a small image that should never drift from its document can be inlined.
- Good: recordings key by what the model saw, so a re-encoded or edited image invalidates its recordings and nothing else.
- Bad: a cases document for hundreds of images carries hundreds of declarations, one line each. A generator writes them; the format does not fold them.
- Bad: a path is relative to the document and nothing else, so two documents in different directories cannot share one declaration; they share the files.
- A document at minor 4 is refused by a minor 3 reader by number ([decision 0020](0020-how-a-v1-minor-is-spelled.md)), and a minor 4 reader reads every minor 3 document unchanged.

### Confirmation

`tests/jud.rs` reads `examples/jud/shapes-cases.jud` (forty cases, each declaring its image by path), loads every image into a request under the reference the state carries, refuses the documents the schemas refuse (neither source, both, a path that leaves the directory, a format the API does not take, bytes without a format), and checks the request fingerprint of a request with an image against the known vector on the format page. `tests/jud_cli.rs` sends `--asset` with the images header and prints the box in pixels; `tests/jud_eval.rs` evaluates two cases by their images from hand-written recordings.

## Pros and cons of the options

### Option 1: a path relative to the document, inline allowed

- Good, because the document and the image are reviewed and moved together, and the reference is the one text the wire uses.
- Good, because a document reads without its images, and fails by name when one is needed and missing.
- Neutral, because the relative path is a rule to state (no absolute, no `..`); the reader states it once and the schema's pattern says the same.
- Bad, because a shared image is a shared file, not a shared declaration.

### Option 2: the bytes always inline

- Good, because nothing resolves: the document is the request.
- Bad, because a 300 KiB image is 400 KiB of base64 that no reviewer reads and every diff carries; forty of them is a document no editor opens comfortably.
- Bad, because the image's provenance (the file it was, the tool that made it) is lost in the encoding.

### Option 3: a URL

- Good, because images can live where they are served from.
- Bad, because a document then reads the network, and a check on a laptop or in CI needs credentials and a connection for what should be a file; the request fingerprint would depend on what a server returned today.
- Bad, because it is a fetch on behalf of whoever wrote the document, from wherever they chose.

### Option 4: a separate asset manifest

- Good, because many documents could share one map, and large sets have one place to list.
- Bad, because a case's label and its image are then in two files that must be kept together, and a reviewer reads both.
- Bad, because the format gains a fourth kind for what is one field on the two kinds that have states and questions.

## More information

[The .jud format](../../reference/jud-format.md) (Assets) is the specification; `src/jud/mod.rs` (`AssetSource`, `load_assets`, `check_assets`) and `src/bin/jud/batch.rs` implement it; `schemas/jud/common.schema.json` holds the `asset` definition; [decision 0024](0024-images-are-part-of-the-request.md) is where the request side of this was decided; [decision 0026](0026-a-box-is-graded-by-intersection-over-union.md) says how a Box label grades.
