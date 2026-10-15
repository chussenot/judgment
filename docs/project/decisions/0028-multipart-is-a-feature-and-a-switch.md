---
title: 0028 Multipart upload is a feature and a switch on the call, off by default
description: How the crate sends a request's images as multipart/form-data parts (the transport TypeSafe recommends for images, a JSON request part and one part per image) beside base64 in the JSON body; behind the multipart cargo feature, as CallOptions::multipart for one call and ClientBuilder::multipart for a client, off by default, with the request's fingerprint, the recordings, the checks and the header unchanged, rather than always, by size, or by a second request type.
status: accepted
date: 2026-10-15
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-15
tags: [decisions, client, images, multipart, features]
---

# 0028 Multipart upload is a feature and a switch on the call, off by default

## Context and problem statement

A request with images carries them as base64 in the JSON body, each asset `{type: image, format, data_base64}` ([decision 0024](0024-images-are-part-of-the-request.md)). The OpenAPI document also takes `multipart/form-data`: a first part named `request` holding the JSON body, in which an asset is `{type: image, part}` naming the part that holds its bytes, then one part per image with its media type; TypeSafe recommends it for images. Base64 costs a third more bytes on the wire and nothing else: on the hosted API the same image sent both ways costs the same input tokens and is answered within hundredths the same (1,167 tokens, a box within 0.01 of the base64 answer, on 2026-10-15). The question was whether the crate should send multipart at all, and if so, when.

## Decision drivers

- A request without images, and the recordings, must not change: a multipart body is the same request, and a `Replay` must answer it.
- The crate's one body path had been verified live on every probe; a second path is a second thing to hold to the contract.
- `reqwest`'s multipart encoding pulls in `mime_guess` and its generated table, a compile cost a client that sends no image should not pay.
- The saving is bytes, not calls or tokens; a caller who measures it should be able to turn it on without touching the rest.
- A part's name goes in a `Content-Disposition` header; a reference is any text up to 1024 bytes.

## Considered options

1. **A feature and a switch**: cargo feature `multipart`; `CallOptions::multipart(true)` for one call, `ClientBuilder::multipart(true)` for every call of a client; off by default; parts named `part-N` in reference order, with the asset's `part` mapping the reference to it.
2. **Always multipart** for a request with images, the JSON body kept for one without.
3. **By size**: multipart over some number of bytes, JSON under it.
4. **A second request type** or a second method (`evaluate_multipart`), the caller choosing per call site.
5. **Nothing**: base64 only, the design note on the bead kept for when a caller asks.

## Decision outcome

Chosen option 1.

- The switch lives where long context does ([decision 0027](0027-long-context-is-a-switch-on-the-call.md)): on the call, with a client-wide default, and never on the `Request`, so the request's serialisation, its fingerprint and its hash (over the bytes' digests, decision 0024) are untouched; the `typesafe.evaluate` span records `multipart`.
- The body is the document's: the `request` part first, under `application/json`, the request's fields in the JSON body's order with each asset as `{type: image, part}`; then the images in reference order as `part-0`, `part-1`, …, each under `image/png` or `image/jpeg` with a file name of its digest, every part with its length so the request carries a `content-length`. The part name is not the reference: a reference may hold text a header cannot carry, and the asset's `part` field is the document's own way of mapping one to the other.
- Everything before and after the wire is shared with the JSON path: the reference check, the 32 MiB bound (the JSON part and the images' bytes together), the images header, the observer's report, the decode and the verification. The retry loop rebuilds the form per attempt from the image's shared bytes, so an attempt costs no copy.
- The feature is off by default and implies `http`; without it the switch does not exist and the span's field is `false`.

### Consequences

- Good: a caller who measures the base64 cost turns it on in one place; everyone else's build, wire and recordings are as they were.
- Good: the contract test reads the form back and holds the `request` part to the request schema and every image part to an asset's `part`.
- Bad: a second body path to keep in step with the document, and a feature flag to know about.
- Bad: the `jud` command has no setting for it; its images go as base64 (a follow-up if a run measures the cost).

### Confirmation

`tests/client.rs` (under the feature) reads the multipart body back part by part, holds the `request` part to the JSON body's fields and order and each image part to its bytes and media type, sends the images header, checks that the switch is off by default and turns on and off per call and per client, that a request without images is the JSON body whatever was asked, and that a retried attempt carries the whole form again; `tests/contract.rs` pins the document's multipart schema (the `request` part required and JSON, the extra properties `image/*`, an asset's `part`) and holds the `request` part to `SystemOneRequest` and every image part to exactly one asset; `tests/live.rs` sends the shapes image and its Box question as parts to the hosted API, answered by `jev-1.14.0` in 0.7 s at an overlap of 0.98 with the label ([Jev 1.14 on the hosted API](../verification/hosted-typesafe-jev-1-14.md)).

## Pros and cons of the options

### Option 1: a feature and a switch

- Good, because the request, the recordings and the default build do not change.
- Good, because the saving is measured by the caller who wants it, not assumed for everyone.
- Bad, because it is one more switch; its documentation says what it buys and what it does not.

### Option 2: always multipart

- Good, because the recommended transport is the only one, and there is one body path with images.
- Bad, because every client that sends an image compiles `mime_guess`, and every recording's request is sent in a form the earlier live runs never verified.
- Bad, because a server that speaks the JSON wire and not the form (the compatible servers the crate is run against were never probed with one) would refuse every image.

### Option 3: by size

- Good, because small images stay on the verified path and large ones save the most.
- Bad, because the threshold is a number nobody can justify, and the same request takes two forms depending on it, which is two paths to test and a surprise in a trace.

### Option 4: a second request type or method

- Good, because the choice is explicit at the call site.
- Bad, because a `Request` is one thing with one fingerprint whatever its transport, and a second method on the trait would put a transport choice in front of every backend, `Fake` and `Replay` included, which have no transport.

### Option 5: nothing

- Good, because base64 carries every image the limits allow, and the hosted API accepted it on every probe.
- Bad, because the saving is real for a caller who sends many images, and the document names the form as the recommended one.

## More information

The OpenAPI document's `multipart/form-data` request body and its `Asset.part` field (`tests/fixtures/typesafe-openapi.json`); `src/client.rs` (`# Multipart`) implements it; [Ask a question about an image](../../guides/ask-about-an-image.md#send-the-images-as-parts) is the procedure; [The crate](../../reference/crate.md#features) lists the feature; the design note that preceded this record is on beads issue `judgment-7k5.4`.
