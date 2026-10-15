---
title: 0024 Images are part of the request, not a call option
description: Where the images a Jev 1.13 and later request carries live in the crate's API; on Request and the SystemOne trait, keyed into every recording's fingerprint by the bytes' SHA-256 and referenced by a name derived from them, with the danger-zone header set by the client, rather than as a CallOptions field, a random UUID per call or a header the caller sets.
status: accepted
date: 2026-10-15
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-15
tags: [decisions, images, assets, request, fingerprint, client]
---

# 0024 Images are part of the request, not a call option

## Context and problem statement

Jev 1.13 and 1.14 take images: a top-level `assets` map of reference to `{type: image, format, data_base64}`, each reference standing for its image wherever it appears as or inside a string in the `state`, a question's `instructions` or its `criteria`, under the header `TypeSafe-Danger-Zone-Images: true` ([Images](https://docs.typesafe.ai/images)). Until now a caller could only send one through `CallOptions::extra("assets", …)` and `CallOptions::header(…)`, by hand, with the base64 and the limits its own problem and no recording able to tell two images apart: the request fingerprint and hash were over the state and the questions alone, so two calls that differed only by their image collided in a `Replay`.

Three things had to be decided: where the images live in the API, how a request names them, and who sends the header.

## Decision drivers

- An image changes the answer, so it is part of a request's identity: a recording must be found by it, and two requests with different images must never share one.
- A request without images must be the same bytes on the wire, the same fingerprint and the same hash it always was, so every committed recording stays valid.
- The documented limits (format, sides, pixels, file size, the request's total, the reference lengths) should be refused before the upload, with the limit named, not after it as a 413.
- A caller that adds an image has already opted in; a second switch for the same decision is a 400 waiting to happen.
- A backend written outside the crate against `SystemOne` must keep compiling.

## Considered options

1. **Images on `Request` and the `SystemOne` trait**, in the fingerprint by the bytes' digest, referenced by a name derived from the bytes, the header set by the client.
2. **Images as a `CallOptions` field**, beside the headers and extra fields.
3. **A random UUID reference per call**, as the Images page recommends for hand-written requests.
4. **The caller sets the header**, as the SDKs have it.

## Decision outcome

Chosen option 1, for all three questions.

- `Request` gains a fourth field, `assets: &Assets`, left off the wire when empty; `Request::new` builds one without images and `with_assets` adds them. `SystemOne` gains `answer_with_assets`, with a default that answers through `answer` when there are no images and refuses them otherwise, so an implementation written elsewhere keeps compiling and never drops an image on the floor; `Fake`, `Recorder`, `Replay` and `Client` implement it. `Fake::calls` records each image's reference and digest.
- The request fingerprint and hash are over `{assets: {reference: "sha256:…"}, questions, state}`, the `assets` key present only with images, each named by the SHA-256 of its bytes rather than by its base64 (`canonical::request_fingerprint_with`, `eval::request_hash_with`). Re-encoding changes nothing; a different image is a different request; a request without images fingerprints as before.
- `Image::reference` is `img-` and 32 hex digits of that digest: unique enough never to occur in text by accident, and the same every time the same image is sent, so a recording of one call answers the next. A caller may choose its own reference (`Assets::insert`); the fingerprint still names the image by its bytes.
- The client sends `TypeSafe-Danger-Zone-Images: true` on every request that has an image or asks a Box question, and on no other; the header is reserved like the client's own, and `assets` is a reserved extra field.
- `Image::from_bytes` reads the format and the size from the bytes and refuses what the page refuses (`Error::InvalidImage`); `Assets::insert` refuses a reference the page refuses and a total over 32 MiB (`Error::InvalidAsset`, `Error::RequestTooLarge`); the client refuses a reference that appears nowhere and a body over 32 MiB before sending. What needs the server (the expanded size, the re-encoded size) stays a 413, which `Error::is_request_too_large` names with the rest.
- The `Observer` is told the images' count and bytes once per call (`on_assets`), and the `typesafe.evaluate` span carries the count as `images`.

### Consequences

- Good, because a recording is keyed by what the model saw, so a replay cannot answer one image with another's answer, and nothing recorded before images moves.
- Good, because an image is refused with its limit named before any bytes leave the process, and the header cannot be forgotten.
- Good, because the trait grows without breaking an implementation elsewhere.
- Bad, because every `Request { .. }` literal gains a field and `Fake::Call` gains one; both are listed as breaking under the crate's 0.x rule, and `Request::new` is the spelling that does not break next time.
- Bad, because a derived reference is not a fresh UUID: the same image sent twice in one request is one asset under one reference, which is what the page asks for, but a caller who wanted two copies under two names writes them with `Assets::insert`.
- Bad, because the hosted API's image limits are checked for every server; a compatible server with other limits gets the hosted API's refusals.

### Confirmation

`tests/client.rs`: the assets map and the header on a request with an image, neither on one without, an unreferenced image refused before the wire, the server's 413 and 400 mapped and not retried, the header and the field reserved, the observer told the bytes. `tests/backend.rs`: a `Fake` takes and remembers images and refuses an unreferenced one, a `Recorder` keys by the image and a `Replay` finds it again (and not with other bytes), the fingerprint names the image by its digest and is unchanged without one, a backend without images refuses them. `tests/contract.rs`: a request with an image conforms to the vendored document with the request components closed, and the document names the header. `src/asset.rs`: the limits and the base64, against fixture images under `tests/fixtures/images/`.

## Pros and cons of the options

### 1. On `Request` and the trait

- Good, because images are request content and the trait is where request content crosses into every backend; the fingerprint follows.
- Bad, because the struct and the trait both change.

### 2. A `CallOptions` field

- Good, because `Request` and the trait stay as they are.
- Bad, because options stop at the trait by design: a `Replay` cannot see them, so an image could not key a recording, and the one rule the options have (nothing in them changes the answer or the hash) would be broken by the first option that did.

### 3. A random UUID per call

- Good, because it is what the page recommends for a hand-written request, and can never match text by accident.
- Bad, because the reference is in the state, so every call would fingerprint differently and no recording would ever replay; a digest-derived name is as safe from accidental matches and stable.

### 4. The caller sets the header

- Good, because the crate sends nothing the caller did not ask for, as the SDKs do.
- Bad, because the caller already asked by adding the image; a second switch for one decision is a 400 after the upload when forgotten, and a header the client would otherwise reserve.

## More information

[Images](https://docs.typesafe.ai/images) is the page the limits and the header come from; `src/asset.rs` holds the types and the limits; `src/client.rs` sets the header and makes the pre-wire checks; `src/eval/canonical.rs` and `src/eval/mod.rs` fold the images into the fingerprint and the hash; [The .jud format](../../reference/jud-format.md) says how a document names an image ([decision 0025](0025-a-document-names-an-image-by-path.md)).
