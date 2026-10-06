---
title: 0018 The .jud format 1.3 takes the manifest envelope and reads one apiVersion
description: Version 1.3 of the .jud format moves its envelope to a Kubernetes manifest's (apiVersion, kind, metadata, spec), so that people and tools that read manifests and Backstage catalog files read these, and its reader reads exactly one apiVersion, withdrawing the rule of 0016 that a minor version adds and a reader reads every earlier one; this record maps the earlier keys onto the new envelope and is the one page that names them.
status: accepted
date: 2026-10-06
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-06
tags: [decisions, jud, format, versioning, envelope, manifest]
---

# 0018 The .jud format 1.3 takes the manifest envelope and reads one apiVersion

## Context and problem statement

[Decision 0014](0014-a-file-format-for-rubrics-cases-and-recordings.md) gave the `.jud` format an envelope of its own: a `jud` key holding a version number, a lowercase `kind`, and the document's identity (`id`, `version`, `description`) and body as sibling keys at the top level. [Decision 0016](0016-jud-takes-minor-versions.md) added minor versions with a compatibility rule (a minor version only adds, a reader reads every earlier one, a writer declares the lowest that reads a document) and reserved top-level `x-` keys for what a tool keeps beside the format. [Decision 0017](0017-jud-1-2-refuses-what-a-reviewer-cannot-see.md) had to bend that rule once already, applying stricter reading to every version.

Three things were wrong with the envelope by then:

- A `.jud` file lives in a repository beside Kubernetes manifests and Backstage catalog files, and nothing in it is recognisable to the tools that index those: a catalog, an editor's schema association, a policy engine and a person skimming a directory all key on `apiVersion` and `kind`, and the format gave them a `jud: 1.2` float instead. YAML readers compare floats loosely (`1.10` is `1.1`), so the specification had to say which spellings were a version and which were not.
- The compatibility rule cost more than it gave. The reader carried a table of which feature arrived in which version and named the first one in a fixed order; the writer recomputed the lowest version over the in-memory value; a recording was always the oldest version by construction. All of it served a reader that read three envelopes to deliver one format, and the second record already had to say where the rule did not hold.
- `x-` keys were a prefix convention with no stated shape. A tool's data could be any YAML, at the top level only, and a reader that round-tripped it carried an open map of arbitrary values.

The maintainer decided that the next version is 1.3 and that it breaks: the number continues the sequence as a label, not as a claim of compatibility. How should the envelope look, what does one reader read, and where do the earlier keys go?

## Decision drivers

- A `.jud` document should be recognisable to people and tools that read manifests, with the identity of the document where they expect it
- A reader reads one thing: no feature-to-version table, no lowest-version computation, no tolerance that half-reads a document of another version
- The field names inside the body are the wire's and the format's; a rename there would change what a question looks like for no reason
- No fingerprint moves: every committed `tuning.cases`, every recording's `fingerprint` and every pinned rubric stays valid
- What a tool keeps beside the format has a stated shape and a stated place

## Considered options

1. A manifest envelope (`apiVersion`, `kind`, `metadata`, `spec`); one `apiVersion`, `jud/v1.3`, read exactly; `metadata.labels` and `metadata.annotations` in place of `x-` keys
2. Keep the format's own envelope and bump to `jud: 2`, adding a `metadata` block, with 0016's compatibility rule kept for the 2.x line
3. The manifest envelope, with a reader that also reads the earlier envelopes and converts them on the way in
4. Leave the envelope as it was and add `apiVersion` and `kind` beside `jud` as aliases for tools that key on them

## Decision outcome

Chosen option: 1. A document is `apiVersion: jud/v1.3`, `kind: Rubric`, `Cases` or `Recording` (PascalCase, as a manifest spells a kind), `metadata` and `spec`, and nothing else at the top level.

- `metadata` holds `name`, required on every kind and a [name](../jud.md#names) as before; `version` on a Rubric; `description` on a Rubric and a Cases document; `labels` and `annotations`, maps of string to string on every kind. A kind refuses the optional fields it does not take. `labels` are for selecting and grouping, `annotations` for what a tool keeps that the format does not name, a shared YAML anchor included; the reader checks only that keys and values are strings, and neither is part of any fingerprint.
- `spec` holds the kind's fields with their names and meanings unchanged: a Rubric's `questions`, `policy` and `tuning`; a Cases document's `rubric` and `cases`; a Recording's `response`, `elapsed_ms`, `fingerprint`, `request_hash`, `rubric`, `server` and `recorded_at`. The names inside `spec` keep their snake_case because they are the wire's (`type`, `instructions`, `criteria`) and the format's own (`options_from`, `level_at_least`, `from_turn`); only the envelope moved, and a document's body reads as it did.
- The reader reads exactly `apiVersion: jud/v1.3`. A missing `apiVersion`, a number, or any other string is refused by name before anything else is read. The compatibility rule of 0016 is withdrawn: there is no reader that reads several versions, a writer writes the one `apiVersion` it implements, and a change of the format, additive or not, takes a new `apiVersion` (`jud/v1.4`) that a reader of this one refuses. A document is moved forward by rewriting its envelope.
- Every rule the earlier records added is a rule of the format, without a version mark: the declarations `when`, `part_when` and `options_from`, a case's `options`, a gate's `bands`, `level_at_least` and `strict`, the name grammar, the refusal of merge keys and tags the core schema does not define, the policy fingerprint.
- Fingerprints are unchanged. Each is over a value inside `spec` (`questions`, `policy`, `cases`, or `{questions, state}` for a request), so moving the envelope moved none, and the known vector in the specification stays.
- In the crate: `jud::API_VERSION` is `"jud/v1.3"` and replaces `VERSION` and `MINOR`; `Rubric { name, version, description, labels, annotations, questions, policy }` replaces `id` and `extensions`, built by `Rubric::new(name, questions)`; `Cases { name: String, description, labels, annotations, rubric, cases }` replaces `id: Option<String>` and `extensions`; `eval::Recording` is unchanged, its `case` read from and written to `metadata.name`. The `jud` command prints `apiVersion` where it printed `jud`.

The mapping from the earlier envelope to this one, recorded here so that the specification and the other pages need not name the earlier keys:

| Earlier | Now |
|---|---|
| `jud: 1`, `jud: 1.1`, `jud: 1.2` | `apiVersion: jud/v1.3` |
| `kind: rubric`, `cases`, `recording` | `kind: Rubric`, `Cases`, `Recording` |
| a rubric's or a cases document's `id` | `metadata.name` (required on a Cases document, where `id` was optional) |
| a rubric's `version` | `metadata.version` |
| a rubric's or a cases document's `description` | `metadata.description` |
| top-level `x-…` keys | `metadata.annotations`, one entry per key, the value as a string; a value that was not a string is serialised or belongs in `spec.tuning` |
| a recording's `case` | `metadata.name` |
| a cases document's `rubric` | `spec.rubric` |
| every other top-level field of a kind | the same key under `spec` |

### Consequences

- Good, because a `.jud` file is recognisable to anything that reads manifests, and a catalog or an editor associates a schema with it by `apiVersion` and `kind` as it does for every other manifest
- Good, because the reader and the writer each handle one envelope: no version table, no first-feature search, no lowest-version computation, and the specification describes one format with no version marks on its fields
- Good, because what a tool keeps beside the format has a shape (string to string) and a place (`metadata`), so a round trip carries strings rather than arbitrary YAML
- Good, because no fingerprint moves, so every committed evaluation run, every `tuning.cases` and every pinned rubric stays valid once the envelope is rewritten
- Bad, because every existing document must be rewritten; a reader of this version refuses the earlier envelopes outright, and nothing in the crate converts them (the table above is the conversion, and it is mechanical)
- Bad, because the crate's API breaks under its 0.x rule: `Rubric::id` and `Cases::id` are `name`, `Cases::name` is required, `extensions` is two string maps, and the version constants are one string
- Bad, because a tool's annotation value must be a string: structured data a tool kept under an `x-` key is serialised into a string or moved into `spec.tuning`
- Bad, because the next change of the format, however small, is a new `apiVersion` and a rewrite of every document; the format accepts that cost in exchange for a reader that never half-reads

### Confirmation

`tests/jud.rs`: every document under `examples/jud/` and every document the crate writes validates against the schemas under `schemas/jud/`, which state the envelope and `metadata` in `common.schema.json`; the example cases bind to their rubric and `tuning.cases` still names their fingerprint, which pins that no fingerprint moved; the schemas and the reader refuse the same documents, a missing or other `apiVersion`, the earlier envelope, a lowercase kind, a top-level `x-` key, a document without `metadata`, `metadata.version` on a Cases document, `metadata.description` on a Recording and a path-shaped `metadata.name` among them; a document reads and is written back with the same fingerprints. `tests/backend.rs`: a replay reads the recordings under `examples/recordings/jud_calibration/` by fingerprint. `examples/jud/` and `examples/recordings/jud_calibration/` carry the new envelope.

## Pros and cons of the options

### 1. The manifest envelope, one apiVersion

- Good, because the envelope is one that people and tools already read, and the identity of a document is in one block with a stated shape
- Good, because one reader reads one envelope, which is the simplest reader the format can have
- Bad, because every document is rewritten once, and every later change of the format is a rewrite too

### 2. `jud: 2` with a `metadata` block, keeping the compatibility rule

- Good, because the format's own envelope stays, and 0016's rule stays for the 2.x line
- Bad, because the reader keeps the version table and the writer the lowest-version computation, which are the cost this record removes; a `jud:` float is still not what a manifest reader keys on

### 3. The manifest envelope, with the earlier envelopes converted on the way in

- Good, because no document has to be rewritten by hand
- Bad, because the reader carries three envelopes forever, a document that reads is not what the file says, and a writer must choose which envelope to write back; the conversion is a table any script applies once, and the crate's reader is not the place for it

### 4. `apiVersion` and `kind` as aliases beside `jud`

- Good, because a manifest reader has something to key on without a rewrite
- Bad, because two keys say the same thing and can disagree, the identity fields stay scattered at the top level, and `x-` keys stay shapeless; it buys recognition without any of the simplification

## More information

[The .jud format](../jud.md) is the specification of `jud/v1.3`; `schemas/jud/common.schema.json` states the envelope, `metadata` and the name grammar; `src/jud/mod.rs` checks the envelope and reads `metadata`; `src/jud/rubric.rs` and `src/jud/cases.rs` read each kind's `spec`. This record supersedes [0016](0016-jud-takes-minor-versions.md) and [0017](0017-jud-1-2-refuses-what-a-reviewer-cannot-see.md): their versioning rules are withdrawn, and the rules they added to the format stand without a version mark.
