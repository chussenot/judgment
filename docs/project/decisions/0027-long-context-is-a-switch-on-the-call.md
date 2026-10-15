---
title: 0027 Long context is a switch on the call, not a request field or a default
description: How the crate exposes TypeSafe's long context (both context limits raised to 262k tokens behind the TypeSafe-Danger-Zone-Long-Context header); as CallOptions::long_context for one call and ClientBuilder::long_context for a client, off by default, never read off the request's size and never a field of Request, so a recording's identity and a request without it stay as they were.
status: accepted
date: 2026-10-15
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-15
tags: [decisions, client, long-context, headers, jev]
---

# 0027 Long context is a switch on the call, not a request field or a default

## Context and problem statement

Jev fits a request into two context limits, 64k tokens for the state and every question together and 32k for the state and any one question; over either, the hosted API answers 400 `max_tokens_exceeded`, which the crate names `Error::is_request_too_large`. Jev 1.14's release raised both to 262k tokens, on Jev 1.13 as well, behind a header, `TypeSafe-Danger-Zone-Long-Context: true`, sent per request ([Models](https://docs.typesafe.ai/models#long-context)); the OpenAPI document lists the header as a parameter taking the one value `true`. The crate had no way to send it but `CallOptions::header`, by name, with the header's spelling the caller's problem and nothing on the span to say a long request went out. The question was where the switch belongs: on the request, on the call, on the client, or nowhere, with the crate deciding from the state's size.

## Decision drivers

- TypeSafe says a long-context request is slower and less accurate than one within the limits, so sending the header is a choice with a cost, not a transport detail.
- A request without long context must be the same bytes, the same fingerprint and the same hash it always was.
- The header changes the limits, not the request: two calls that differ only by it ask the same question of the same state, and a recording of one answers the other.
- A client used for one kind of work (a batch over long transcripts) should not have to pass an option on every call; a client used for both should be able to say so per call.
- The crate has no tokeniser, and the limits are the server's to count.

## Considered options

1. **A switch on the call and on the client**: `CallOptions::long_context(bool)` for one call, `ClientBuilder::long_context(bool)` for every call, off by default; the header reserved.
2. **A field of `Request`**, beside `state`, `model`, `questions` and `assets`.
3. **Always on**: send the header on every request.
4. **Decided by the crate** from the request's size, with a token estimate.
5. **Nothing**: leave `CallOptions::header` as the way.

## Decision outcome

Chosen option 1.

- `CallOptions::long_context(true)` sends the header on that call; `ClientBuilder::long_context(true)` on every call of the client, and a call's `false` turns it off for that call. The header is reserved like the client's own (`DANGER_ZONE_LONG_CONTEXT_HEADER`), so it cannot be sent by name with another value or spelling, and the setting is the `typesafe.evaluate` span's `long_context` field. The body is untouched: the contract test pins that the request with the option is byte for byte the request without it, plus the header.
- The setting stays on the client side of the `SystemOne` trait, as every call option does ([How judgment works](../../concepts/how-judgment-works.md)); a `Recorder` over a client built with it on sends it on every call, and the recordings are keyed as before.
- `Error::is_request_too_large`'s documentation names the switch as the other remedy beside sending less.

### Consequences

- Good: a request without long context is unchanged on the wire and in every recording; a caller who needs it turns it on where the long states are, and the span says when it was on.
- Good: the cost TypeSafe names is paid deliberately.
- Bad: a caller whose states vary in size either turns it on for all of them, paying the accuracy cost on the short ones, or measures the state itself; the crate gives no estimate.
- Bad: the `jud` command has no flag or setting for it yet; a long case is refused as too large until it does (a follow-up, tracked in beads).

### Confirmation

`tests/client.rs` sends the header for a call that asked, for every call of a client that asked, and not otherwise, and refuses the header as a per-call header and as a builder default; `tests/contract.rs` pins the header parameter and its one value in the OpenAPI document and the unchanged body; `tests/live.rs` sends a 40,000-token state, refused without the header as `max_tokens_exceeded`, and sees it answered with it (74,550 input tokens, 1.4 s, on 2026-10-15).

## Pros and cons of the options

### Option 1: a switch on the call and on the client

- Good, because it is where every other per-call choice lives, with a client default for the batch case.
- Good, because the request and the recordings do not change.
- Bad, because it is one more thing to know about when a request is refused; the error's documentation points to it.

### Option 2: a field of `Request`

- Good, because the request says everything about itself.
- Bad, because it would enter the request's serialisation, or need a serde skip and a special case in the fingerprint and the hash, for something that is not part of the question asked.
- Bad, because a `Request` is built per call by `system_one`, so a client-wide default would still be needed.

### Option 3: always on

- Good, because no caller is ever refused for size.
- Bad, because TypeSafe says the answers are less accurate and slower under it; every caller would pay for the few.

### Option 4: decided by the crate from the request's size

- Good, because the caller never thinks about it.
- Bad, because the crate has no tokeniser, a byte estimate is wrong for JSON and for images, and a wrong guess either pays the accuracy cost or still gets the 400.
- Bad, because the limit is the server's and changes with the model; a guess written today is a bug later.

### Option 5: nothing

- Good, because it already works through `CallOptions::header`.
- Bad, because the header's name and value are then the caller's to spell, nothing on the span records it, and the error's remedy cannot name it.

## More information

[Models](https://docs.typesafe.ai/models#long-context) is the page the header comes from; `src/client.rs` (`# Long context`) implements it; [Configuration](../../reference/configuration.md#the-crates-client) lists the setting; [decision 0024](0024-images-are-part-of-the-request.md) made the same choice the other way for the images header, which the client sets itself because the image on the request is the opt-in.
