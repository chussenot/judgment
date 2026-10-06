---
title: 0020 How a v1 minor of the .jud format is spelled
description: Proposed and not applied; whether a minor version of the jud/v1 format stays in the apiVersion string (jud/v1.3, jud/v1.4) or moves to a field of its own under apiVersion jud/v1, now that the stability page promises that a minor only adds and that a reader accepts every v1 document for at least twelve months; each option priced in files rewritten, reader logic and what tools that key on apiVersion see.
status: proposed
date: 2026-10-06
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-06
tags: [decisions, jud, format, versioning, apiVersion, stability]
---

# 0020 How a v1 minor of the `.jud` format is spelled

**Status: proposed.** Nothing in the code, the schemas or the documents moves until this record is accepted; the reader reads `apiVersion: jud/v1.3` exactly, as [decision 0018](0018-jud-1-3-takes-the-manifest-envelope.md) decided.

## Context and problem statement

[Decision 0018](0018-jud-1-3-takes-the-manifest-envelope.md) gave the format a manifest's envelope and one rule for versions: the whole version is in the `apiVersion` string, a reader reads exactly one, and any change, additive or not, takes a new string. That was the right cut for 1.3, which broke on purpose. [Stability](../stability.md) now promises more than 0018 does: `jud/v1` is stable, a minor only adds, and a reader accepts every `v1` document for at least twelve months after a later minor is published. The first additive change (a field under `spec`, a value a gate may take) will have to be spelled somehow, and the spelling decides what a reader must do to keep that promise and what every tool that keys on `apiVersion` sees.

Two spellings are on the table, and a third that keeps the question open. The question is which one makes the twelve-month promise cheapest to keep, for this reader and for another implementation, without giving back what 0018 bought: a document recognisable to the tools that read manifests, and a reader with no feature-to-version table.

## Decision drivers

- The stability promise must be mechanical: a reader given a `v1` document of an earlier minor reads it, and a reader given a later minor than it knows says so by name, never half-reads it.
- Tools that key on `apiVersion` (an editor's schema association, a catalog, a policy engine, `kind`-and-`apiVersion` selectors) should see one line, `jud/v1`, for one stable format, as they see `v1` for a Kubernetes Deployment across years of additive change.
- No fingerprint moves, whatever is chosen: every fingerprint is over a value inside `spec`, and a version field, wherever it sits, stays outside them.
- The envelope stays four fields; a manifest has no fifth top-level key, and the reader refuses one by name.
- A minor's cost should fall on the reader that adds it, once, and not on every document ever written.

## Considered options

1. **Keep the minor in the string: `apiVersion: jud/v1.3`, then `jud/v1.4`.** A reader keeps the list of `v1` minors it reads and refuses any other string by name. A writer writes the minor it implements.
2. **`apiVersion: jud/v1` with a `minor` field of its own** (`metadata.minor: 3`, an integer; the name is the question's, the place is this record's). A reader accepts `jud/v1` and any `minor` at or below the one it implements, refusing a higher one by number. A writer writes the lowest minor whose fields the document uses, or simply the one it implements.
3. **`apiVersion: jud/v1` and no minor anywhere.** A document is a `v1` document; what it uses is what it uses. A reader of an earlier revision refuses an unknown field by its path, as the strict reader already does; the specification is dated, not numbered.

## Proposed outcome

Option 2, `apiVersion: jud/v1` with `metadata.minor`, when the first additive change is ready, and not before: the rewrite it costs is paid once and should carry a change worth it. Until then 1.3 stays as it is and the stability promise holds trivially, with one minor published.

Why 2 over 1: option 1 keeps the minor where a manifest reader looks for the major, so each minor is a new `apiVersion` to every tool outside this repository: a new schema association, a new selector, a new entry in a catalog's list of kinds it knows, for a change that by the stability promise cannot break them. The reader's list of accepted strings is 0018's feature-to-version table in another form. Option 2 gives those tools one line for the life of `v1`, and the reader one integer comparison.

Why 2 over 3: option 3 keeps the promise by accident. A 1.3 reader given a 1.5 document fails on the first field it does not know, with that field's path, which is the right refusal but the wrong message: nothing in the document says the reader is out of date rather than the document wrong, and nothing lets a writer state the revision a document needs. The twelve-month clock on the stability page needs a number to run against. The cost of option 2 over 3 is one integer per document and the rule for choosing it.

### Priced

Costs are counted on this repository at 0.9.0 and on the day the first minor ships. "Rewrite" means every document under `examples/jud/`, `examples/recordings/`, `tests/`, `plugins/jud/skills/jud/evals/` and the inline documents in `src/jud/*.rs`, `tests/jud.rs`, `README.md` and `docs/`: about 20 files of documents, 13 recordings and some 60 inline documents, done once by a script, with no fingerprint moving.

| Cost | 1. `jud/v1.3`, `jud/v1.4` | 2. `jud/v1` + `metadata.minor` | 3. `jud/v1`, no minor |
|---|---|---|---|
| Now (before any minor) | none | none, if deferred to the first minor; the rewrite otherwise | same as 2 |
| The first minor, documents | none: 1.3 documents stay `jud/v1.3` and the reader reads both | the rewrite, once: `apiVersion: jud/v1` and `minor: 3` on every existing document | the rewrite, once: `apiVersion: jud/v1` |
| The first minor, reader | a list of accepted strings and a per-minor rule for which fields each admits; the 0016 table again | one integer, one comparison, one error ("needs minor 4, this reader reads 3"); fields admitted by the schema, not by minor | no version logic; an unknown field is refused by path, which it is already |
| Each later minor | a new string, a new schema set, and every tool keying on `apiVersion` updated | a new integer in the schema's enum and the reader's constant; tools see no change | a dated schema revision; tools see no change |
| Tools outside the repository | a new `apiVersion` per minor to associate, select and list | one for the life of `v1` | one for the life of `v1` |
| Another implementation keeping the twelve months | must read every string in the list and know what each admits | must read every `minor` up to its own; one number to compare | must read every field it has ever known; cannot tell a stale reader from a wrong document |
| What a document can state | the minor it was written for, in the string | the minor it needs, in a field | nothing |
| `jud check` and the plugin | print and teach a per-minor string | print `jud/v1` and the minor | print `jud/v1` |
| Schemas | one set per minor (`schemas/jud/1.3/`, `1.4/`) | one set, `minor` an enum that grows | one set, revised |

The one cost option 2 has that 1 does not is the rewrite, and it is the cost 0018 already paid once for a better envelope: a script over the documents, no fingerprint touched, every recording still replaying. The one cost option 1 avoids is that rewrite; what it buys is a reader and a tool landscape that treat every minor as a new format, which is what the stability page says a minor is not.

### Consequences

If accepted:

- The envelope stays four fields. `metadata.minor` is an integer, required on every kind from the first minor on, outside every fingerprint like the rest of `metadata`.
- `jud::API_VERSION` becomes `"jud/v1"`, with a `MINOR` constant beside it; the reader refuses an `apiVersion` other than `jud/v1` by name and a `minor` above its own by number; `jud check` prints both.
- The stability page's twelve-month clock starts at the release that first reads minor 4, and the `CHANGELOG` names it.
- [Decision 0018](0018-jud-1-3-takes-the-manifest-envelope.md)'s rule that a reader reads exactly one `apiVersion` stands for the string; its rule that any change takes a new string is narrowed to a change of meaning, which the stability page already states. A status note on 0018 records that.

### Confirmation

When applied: `tests/jud.rs` reads a document of every published minor and refuses one with a `minor` above the reader's, naming the number; the known fingerprint vector on the specification page does not move; `examples/recordings/` replay unchanged; `jud check` over `examples/jud/` prints `jud/v1` and the minor for every document.

## Pros and cons of the options

### 1. Keep the minor in the string

- Good, because nothing changes until something changes; the first minor costs no rewrite.
- Good, because a document names its exact version where everything else in a manifest names a version.
- Bad, because every minor is a new `apiVersion` to tools that cannot tell it is additive, and the stability promise has to be explained to each of them.
- Bad, because the reader carries a list of strings and what each admits, the table 0018 removed.

### 2. `jud/v1` with a `minor` field

- Good, because `apiVersion` says what is stable and `minor` says what was added, each in the place a reader of either kind looks.
- Good, because keeping the twelve-month promise is one integer comparison, for this reader and for any other.
- Bad, because of the one-time rewrite, and because `minor` is a field a manifest reader has not seen before; it sits in `metadata`, where a manifest keeps what the format does not define.
- Bad, because a writer must choose the minor to write: the rule has to be stated (the one it implements, unless told to write the lowest that reads the document).

### 3. `jud/v1` and no minor

- Good, because it is the least machinery: the strict reader already refuses what it does not know.
- Good, because tools see one `apiVersion` for the life of `v1`.
- Bad, because a document cannot say what it needs and a reader cannot say it is out of date rather than the document wrong.
- Bad, because the twelve-month promise has no number to run against: "every `v1` document" becomes "every document using fields published before a date", which nobody can check from the document.

## More information

- [Stability](../stability.md): the promises this record has to make cheap to keep.
- [Decision 0018](0018-jud-1-3-takes-the-manifest-envelope.md): the envelope and the one-`apiVersion` rule this record narrows.
- [Decision 0016](0016-jud-takes-minor-versions.md), superseded: the earlier minor-version rule and what it cost, the table option 1 would bring back.
- [The .jud format](../jud.md), "Extending the format": what a minor may add.
- `src/jud/mod.rs` (`API_VERSION`, `Envelope::check`), `schemas/jud/`, `tests/jud.rs`: where the accepted option lands.
