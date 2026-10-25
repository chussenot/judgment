---
title: 0029 Request compression is a feature and a switch, off by default
description: How the crate sends a JSON request body compressed with zstd, as the official SDKs do by default since Python 0.7.5 and JS 0.7.0; behind the zstd cargo feature, as ClientBuilder::zstd for a client and CallOptions::zstd for one call, off by default, from 1 KiB, never for a multipart body, with the fingerprint, the recordings and the retries unchanged, rather than on by default as in the SDKs, always, or not at all.
status: accepted
date: 2026-10-25
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-25
tags: [decisions, client, compression, zstd, features]
---

# 0029 Request compression is a feature and a switch, off by default

## Context and problem statement

The official Python SDK 0.7.5 and JavaScript SDK 0.7.0 compress every request body of 1 KiB or more with zstd unless told not to (`compression=False`, `compression: false`); the Python one only when the runtime has zstd (Python 3.14, or the `zstd` extra), the JavaScript one on Node.js 22.15 or later. The Python release says that non-official APIs may not support it yet. The hosted API documents the header in its [request rules](https://docs.typesafe.ai/api/errors#compression): `Content-Encoding: zstd` or `gzip`, one layer, a zstd window of at most 8 MiB, the 32 MiB limit on the body both compressed and decompressed, `415` for an encoding it does not take. The OpenAPI document says nothing about it. The crate is run against servers that copy the wire ([compatible servers](../research/compatible-servers-and-models.md)), none of which was probed with a compressed body. The question was whether the crate should compress, and if so, when.

## Decision drivers

- A body a server cannot read is a refused call, or worse, a 400 that reads like the caller's mistake; the crate's default must work against every server it is run against.
- The request, its fingerprint and the recordings must not change: a compressed body is the same request, and a `Replay` must answer it.
- A build that does not compress should not compile or link a compressor; libzstd is a C build.
- The saving is bytes on the wire for a large state, not calls or tokens.

## Considered options

1. **A feature and a switch**: cargo feature `zstd`; `ClientBuilder::zstd(true)` for every call of a client, `CallOptions::zstd` for one; off by default; from 1 KiB, the SDKs' threshold; never a multipart body.
2. **The SDKs' default**: compression on whenever the feature is compiled in, off per client for another server.
3. **Always**, without a feature.
4. **gzip as well**, or instead, through a choice of encoding.
5. **Nothing**.

## Decision outcome

Chosen option 1.

- The switch lives where long context and multipart do ([decision 0027](0027-long-context-is-a-switch-on-the-call.md), [decision 0028](0028-multipart-is-a-feature-and-a-switch.md)): on the call, with a client-wide default, never on the `Request`, so the request's serialisation, fingerprint and hash are untouched.
- The JSON body is built as before, held to the 32 MiB bound as before, then compressed once with zstd at level 3 (a window of at most 2 MiB, inside the hosted API's 8 MiB) when it weighs 1 KiB or more, and sent under `Content-Encoding: zstd` with `Content-Type: application/json` on every attempt. A smaller body goes as it always did, and so does a multipart body, whose images are already compressed.
- `Content-Encoding` is a reserved header with or without the feature: a caller's value would label a body the client did not encode.
- The span's `zstd` field says whether the body went compressed; the observer's report is unchanged.

### Consequences

- Good: a caller who sends large states to the hosted API turns it on in one place, and everyone else's build, wire and recordings are as they were.
- Good: a feature-less build carries no C compressor.
- Bad: the crate departs from the SDKs' default, so a reader comparing traffic sees uncompressed bodies from the crate unless it was asked.
- Bad: a third body form to keep in step with the hosted API's rules, verified live only on the hosted API.
- Bad: the `jud` command has no setting for it (a follow-up if a run measures the cost).

### Confirmation

`tests/client.rs` (under the feature) decompresses what the server received and holds it to the plain JSON byte for byte, at 1,024 bytes compressed and 1,023 not; checks that the switch is off by default and turns on and off per call and per client, that a retried attempt carries the same compressed body, that a recording made through a compressing client replays, and (with `multipart`) that a multipart body is never compressed; without the feature it checks that `Content-Encoding` is refused as a header. `tests/live.rs` sends the same request plain and compressed to the hosted API: `jev-1.14.0` answered both with 566 input tokens on 2026-10-25 ([the hosted API](../verification/hosted-typesafe.md#compressed-bodies-2026-10-25)).

## Pros and cons of the options

### Option 1: a feature and a switch

- Good, because the default works against every server the crate is run against.
- Good, because the request, the recordings and the default build do not change.
- Bad, because the caller has to know to ask.

### Option 2: the SDKs' default

- Good, because the crate's traffic would match the SDKs'.
- Bad, because every client built with the feature would send bodies the compatible servers were never shown to read, and the remedy (turning it off) would be found from a refused call.

### Option 3: always

- Good, because one body path.
- Bad, because every build compiles libzstd, and every server that copies the wire would have to decode it.

### Option 4: gzip as well

- Good, because gzip is the more widely decoded encoding, and the hosted API takes it.
- Bad, because neither SDK sends it, so nothing would have been held against it, and a choice of encoding is an option nobody has asked for.

### Option 5: nothing

- Good, because no new body path and no new dependency.
- Bad, because the saving is real for a large state, and both SDKs now send it.

## More information

The SDK releases: [Python 0.7.5](https://github.com/typesafe-ai/typesafe-sdk-python/releases/tag/v0.7.5) (`MIN_COMPRESSED_BODY_LENGTH = 1024` in its `_core/compression.py`) and [JavaScript 0.7.0](https://github.com/typesafe-ai/typesafe-sdk-js/releases/tag/v0.7.0) ("request bodies of 1 KiB or more"); the hosted API's [request rules](https://docs.typesafe.ai/api/errors#compression). `src/client.rs` (`# Compression`) implements it; [Configuration](../../reference/configuration.md) and [The crate](../../reference/crate.md#features) list the switch and the feature; beads issue `judgment-mwx` tracked the work.
