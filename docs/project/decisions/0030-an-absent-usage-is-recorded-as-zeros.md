---
title: 0030 An absent usage is recorded as zeros, not kept absent
description: Why Response.usage stays a plain Usage rather than an Option when a server omits it; the decoder normalises an absent usage, or an absent count inside it, to zero before anything reads it, so a recording of zeros is exactly what a live caller observed, and the format page states the one normalisation its fidelity rule makes.
status: accepted
date: 2026-10-28
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-28
tags: [decisions, answers, usage, recordings, jud]
---

# 0030 An absent usage is recorded as zeros, not kept absent

## Context and problem statement

The OpenAPI document requires `usage` on every response, with both counts.
The crate is more tolerant on purpose: a `usage` a server leaves out, or a
count inside it, decodes as zero, so one missing field does not lose a
billed response (`Response::usage`, the tolerance table in
[Internals](../internals.md)). A recording then writes the response back
with those zeros.

The second review pass (judgment-y7a.52) read that against the format
page's fidelity rule — a recording's `response` keeps whatever the server
sent — and found the contradiction: a server that sent no `usage` is
recorded as having sent zeros. The review fixed the larger half in 0.16.0
(an answer's unknown fields are kept in that answer's `extra`) and left
the question: should `Response::usage` become `Option<Usage>`, so a
recording keeps the absence?

## Decision drivers

- The decoder normalises before anything reads it: a live caller of a
  response without `usage` sees zeros, through the field, the `Observer`
  and the `typesafe.evaluate` span. What a recording writes should be what
  a replay makes a caller see, and both should be what the live caller saw.
- Every consumer reads `response.usage.input_tokens` as a number to add to
  a sum or a counter. None has a different action for "the server said
  nothing" — there is no better number to account a billed call with.
- The OpenAPI document requires `usage`; only a non-conforming server
  omits it, and the crate's contract tests check recordings against that
  document.
- 0.16.0 already broke `Answer` construction (`extra`); a second
  consecutive break of the response type needs to buy something a
  consumer can act on.
- A recording is not a packet capture: the format already normalises (an
  empty `part_when` is left out, numbers print through the double), and
  its fidelity rule is about not dropping information, with any
  normalisation stated on the page.

## Considered options

1. **Keep `usage: Usage`, state the normalisation in the format page**:
   absent decodes as zero, is written back as zeros, and the page says so
   beside the fidelity rule.
2. **`Option<Usage>` on `Response`**: `None` when the server sent
   nothing, serialised only when present.
3. **Skip serialising a default usage**: keep the type, leave zeros out
   on write.
4. **A hidden absence marker**: keep the type, remember absence in a
   private field, write it back faithfully.

## Decision outcome

Chosen option 1.

- `Response::usage` stays a plain `Usage`. The field's documentation says
  an absent usage is written back as zeros and why; the format page's
  fidelity rule now carries the carve-out in the same sentence that
  states the rule ([The .jud format](../../reference/jud-format.md),
  Recordings).
- The absent-versus-zero distinction is dropped knowingly: it reaches no
  decision a consumer of this crate makes, and keeping it would cost
  every reader of the commonest field on the response.

### Consequences

- Good: live and replay agree by construction — a replayed response
  reports to the `Observer` exactly what the live response did.
- Good: every recording conforms to the OpenAPI document's response
  schema, whatever the server did.
- Bad: a recording cannot show that a server violated the document by
  omitting `usage`; whoever debugs an off-spec server needs the wire, not
  a recording.
- Bad: zeros from an off-spec server read as a free call in cost sums.
  The counts were never sent, so no shape of the field could report a
  truer number.

### Confirmation

The unit round-trip test pins a response without `usage` written back as
zeros and one with a sent usage kept as sent (`src/answer.rs`); the
contract test's tolerance rows pin that the schema refuses a response
without `usage` while the crate decodes it, and what each tolerated value
reads as (`tests/contract.rs`); `mise run docs:check` holds the page and
its derived copies together.

## Pros and cons of the options

### Option 1: keep the type, state the normalisation

- Good, because the behaviour was already shipped, documented on the
  field and pinned by tests in 0.16.0; the only defect left was the page
  contradicting it.
- Good, because `usage.input_tokens` stays one field read on the path
  every consumer walks.
- Bad, because the fidelity rule gains an exception a reader must know.

### Option 2: `Option<Usage>`

- Good, because a recording would keep the absence, faithful to the wire.
- Bad, because every read of the commonest response field becomes an
  unwrap-or-default — about fifty in this crate alone, every consumer
  besides — for a distinction none of them acts on.
- Bad, because it is a second consecutive breaking change to the response
  type, one release after `Answer` gained `extra`.
- Bad, because a recording without `usage` no longer conforms to the
  OpenAPI document the contract tests hold recordings to.

### Option 3: skip serialising a default usage

- Good, because no type changes.
- Bad, because it conflates a *sent* zero usage with an absent one: a
  `Fake`'s explicit zeros would vanish on write, which is the measured
  failure (both committed-recording contract tests went red when the
  review fleet tried it).

### Option 4: a hidden absence marker

- Good, because reads stay one field and the write-back is faithful.
- Bad, because `Response` is constructed by struct literal, so any new
  field is the same break as option 2 with a stranger shape, and the
  marker would have to survive `PartialEq`, `Clone` and every test that
  builds a `Response` by hand.

## More information

[The .jud format](../../reference/jud-format.md) carries the fidelity
rule and its one normalisation; [Internals](../internals.md) lists the
decoder's four tolerances; [decision 0014](0014-a-file-format-for-rubrics-cases-and-recordings.md)
made recordings part of the format. The review finding and the weighing
are in the tracker as judgment-y7a.52; the answers half of the same
finding shipped in 0.16.0 (every `Answer` variant keeps unknown fields in
`extra`). The `usage` keys the crate does not model are a separate,
still-open question (judgment-bsz.16.13).
