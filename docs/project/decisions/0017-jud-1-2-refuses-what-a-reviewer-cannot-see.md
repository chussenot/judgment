---
title: 0017 The .jud format 1.2 refuses what a reviewer cannot see, and names the policy
description: Version 1.2 of the .jud format adds no field; it makes every id a name that is never a path, refuses YAML merge keys and tags the core schema does not define, keeps a binary scalar as its text, keeps the document out of error messages, and gives a rubric's policy a fingerprint of its own, because a threat model found that a reviewed rubric could still read differently from how it looked and that the questions' fingerprint could not show a moved threshold.
status: superseded by 0018
date: 2026-10-06
decision-makers: [platform engineering]
consulted: []
informed: []
last_reviewed: 2026-10-06
tags: [decisions, jud, format, versioning, security, fingerprint]
---

Superseded by [0018](0018-jud-1-3-takes-the-manifest-envelope.md) on 2026-10-06: the envelope is a manifest's (`apiVersion`, `kind`, `metadata`, `spec`), a reader reads one apiVersion and the versioning rule below is withdrawn; the rules this record added to the format stand, without a version mark. The record is kept as written.

# 0017 The .jud format 1.2 refuses what a reviewer cannot see, and names the policy

## Context and problem statement

A `.jud` rubric is reviewed by people and read by a program, and the format's value rests on both reading the same thing: the thresholds in the file are the decision, and a pinned fingerprint is how an application notices that the file moved. A threat model of the crate ([the STRIDE threat model](https://github.com/chussenot/judgment/pull/13), T-005, T-006, T-012, T-013 and H-013) found five places where that did not hold:

- The reader accepted YAML merge keys, tags the core schema does not define, and `!!binary` scalars decoded from base64, so a question's instructions could be text the reviewer saw only as a blob or as a reference to another mapping, while [the specification](../../reference/jud-format.md) promised the YAML 1.2 core schema.
- `Rubric::fingerprint` covers the questions and deliberately not the policy, so a gate moved from `level_at_least: 2` to `3` left every pinned fingerprint, log line and evaluation report unchanged.
- A case id only had to be non-empty, so an id of `../../.config/app/settings` named a file outside the recordings directory for any tool that names a recording after its case, as the crate's `recording_path` does.
- A syntax error carried the lines around it, so a malformed cases file printed a customer's state into whatever log recorded the error.
- A replay followed symlinks in its directory and looked a request up by a 64-bit FNV hash before the SHA-256 fingerprint.

[Decision 0016](0016-jud-takes-minor-versions.md) says a minor version only adds and a change of meaning takes a major version. None of the five is a meaning any conforming document had: a merge key, a foreign tag and a path-shaped id were accepted by a lenient reader, not defined by the format. How should the format say so, and under which version?

## Decision drivers

- A reviewer must see what the model will be asked; the file must not read differently from how it looks
- An application that pins a rubric must notice a moved threshold as surely as a changed question
- An id from a document must never reach the file system as a path
- An error message must be safe to log, since a cases document's state can be someone's data
- Every existing document under `examples/jud/`, and every document a 1.1 reader accepted that used none of the above, must read the same and keep its fingerprints

## Considered options

1. Version 1.2: no new field; the rules stated and applied to every version the reader reads; a policy fingerprint added to the specification
2. Version 2: the same rules as a change of meaning, refused by 1.x readers
3. Fix the reader only, leave the specification and the schemas as they were
4. Sign documents, so a tampered policy or recording fails to verify

## Decision outcome

Chosen option: 1. `jud: 1.2` adds no field and no document needs to declare it; a writer still declares the lowest version that reads a document, so every document the crate wrote before is written the same. The 1.2 rules are:

- **Names.** A rubric's and a cases document's `id`, a case's `id` and a recording's `case` are names: ASCII letters, digits, `.`, `_` and `-`, starting with a letter or a digit. A reader refuses any other id naming the field, the schemas carry the pattern, and the crate's `write_recording` and `read_recording` refuse an id that is not a name before touching the file system.
- **Nothing hidden.** A merge key and a tag the core schema does not define are refused with their line and column; a `!!binary` scalar is its text, never decoded. The reader sets serde-saphyr's `merge_keys = Error`, `reject_unsupported_tags` and `ignore_binary_tag_for_string`.
- **Nothing quoted.** A syntax error names a line and a column and no part of the document (`with_snippet = false`).
- **A policy fingerprint.** The fingerprint of a rubric's `policy` map alone, the gates as written, as `Rubric::policy_fingerprint`; `tuning` is part of neither fingerprint. The specification says plainly that the questions' fingerprint is not an integrity check of the thresholds.
- **In the crate's replay**, which the format does not govern: a directory under replay reads only its regular files, never a symlink or a subdirectory, and a request is looked up by its SHA-256 fingerprint before the crate's own FNV hash.

The rules apply to every version the reader reads, 1 and 1.1 included. That is a departure from "a 1.1 reader reads every 1 document as a 1 reader does", taken on purpose and narrowly: the documents it refuses are ones no reader should have accepted, and refusing a whole document is the failure the format chooses over half-reading one.

### Consequences

- Good, because the file and the request are the same text again: what a reviewer sees is what the model is asked
- Good, because an application pins two fingerprints and a moved bar is visible, without a signing scheme or a key to manage
- Good, because a case id is safe to use as a file name in any tool, and an error from a cases file is safe to log
- Good, because every document under `examples/jud/` reads unchanged with the same fingerprints, and the crate writes exactly what it wrote
- Bad, because a document that used a merge key or an anchor-and-merge to share a question, or a path-shaped id, is now refused and must be rewritten; the shared-anchor need is served by `x-` keys with plain aliases, which 1.1 added for that
- Bad, because a 1.2 reader is stricter than a 1 reader on 1 documents, which decision 0016 did not foresee; this record is where that exception is written down
- Bad, because the policy fingerprint is a second value to pin and to print; a combined fingerprint over questions and policy was left out, since two values say which one moved

### Confirmation

`tests/jud.rs`: the schemas and the reader refuse the same path-shaped ids on all four fields and `jud: 1.3`; a tag and a merge key are refused with a line, a `!!binary` scalar reads as its text; a syntax error over a state holding a secret does not quote it; a `jud: 1.2` document reads and is written back as `jud: 1` or `1.1`; the policy fingerprint moves with a threshold, not with the id, the questions or `tuning`, and an empty policy is the SHA-256 of `{}`. `tests/backend.rs`: a replay skips a symlink and a directory, prefers the fingerprint to the hash, and `write_recording` refuses a path-shaped id. `examples/jud/` is unchanged and still validates.

## Pros and cons of the options

### 1. Version 1.2, rules applied to every version

- Good, because nothing a conforming document could say changes, so no document, fingerprint or writer moves
- Good, because the version number records where the rules were stated, for a second implementation to check itself against
- Bad, because it bends the letter of decision 0016 and needs this record to say so

### 2. Version 2

- Good, because the stricter reading would be announced by the number
- Bad, because every 1 and 1.1 document would be refused by version until rewritten as `jud: 2`, for rules they already obey; a major version for a change that touches no field punishes every document to catch a few malformed ones

### 3. Fix the reader only

- Good, because it is the smallest change
- Bad, because the specification would still promise the core schema while saying nothing about tags and merge keys, and another implementation would keep accepting what this one refuses; the format exists so that two tools read a file the same way

### 4. Sign documents

- Good, because a tampered policy or recording would fail closed whoever edited it
- Bad, because it needs keys, a signing step and a trust decision in every consumer, for a format whose files live in reviewed repositories; a fingerprint an application pins gives the detection without the ceremony, and signing can still be added above the format later

## More information

[The .jud format](../../reference/jud-format.md) states the 1.2 rules (Versions, Names, Fingerprints, Reading rules); `schemas/jud/common.schema.json` carries the `name` pattern and the `1.2` version; `src/jud/mod.rs` sets the reader's options and checks names; `src/eval/mod.rs` refuses a non-name in the recording helpers; `src/backend.rs` is the replay. The threat model that motivated the record is [pull request 13](https://github.com/chussenot/judgment/pull/13).
