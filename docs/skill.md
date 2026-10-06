---
title: The jud plugin
description: A Claude Code plugin, a skill and three commands, that writes, reviews and fixes .jud documents and checks them with the crate's own reader through the jud command; what it does, how to install it from this repository's marketplace, the commands, and how it was tested.
status: current
last_reviewed: 2026-10-06
tags: [judgment, jud, skill, agent, claude-code, tooling]
---

# The jud plugin

An agent writing a `.jud` file has two ways to get it wrong: the shape, which the reader refuses with the field named, and the substance, which no reader can see: a compound question, a Choice with no way out, a threshold the instructions already moved into the model, a `tuning` block with no run behind it. The plugin under [`plugins/jud/`](../plugins/jud/) exists for both. It gives an agent the field reference for version 1.3 in one page, the rules writers trip on, and a command that reads a document exactly as the crate does, so a document is checked before it is handed over rather than when it is first loaded.

It ships as a [Claude Code plugin](https://code.claude.com/docs/en/plugins), `jud`, from a marketplace this repository serves (`.claude-plugin/marketplace.json`): a skill that loads whenever a task touches a `.jud` file, and three commands, `/jud:rubric`, `/jud:cases` and `/jud:check`. In this repository the plugin is enabled for every session through `.claude/settings.json`; anywhere else it is two commands away.

## Installing it

```sh
claude plugin marketplace add chussenot/judgment   # once: this repository is the marketplace
claude plugin install jud@judgment                 # the plugin; --scope project enables it for everyone in a repository
mise use -g github:chussenot/judgment@latest       # the `jud` command the plugin checks with (or cargo install judgment --features jud)
```

Or interactively: `/plugin marketplace add chussenot/judgment`, then `/plugin install jud@judgment`. The plugin has no dependency beyond the `jud` command: with none on `PATH`, its script builds one from a judgment checkout named by `JUDGMENT_DIR`, from the project when the project is this crate, or from the checkout the plugin sits in. To work on the plugin itself, add the checkout as a local marketplace (`/plugin marketplace add ./` from the repository root) and reload with `/reload-plugins` after an edit; a plugin loaded from a local marketplace is read in place, not copied. The plugin is not published with the crate (`Cargo.toml` excludes `plugins/`); the `jud` binary is.

## The commands

| Command | What it does |
|---|---|
| `/jud:rubric <brief>` | Writes a Rubric from a brief, or revises one at a path: narrow questions with criteria, a policy with a `note` per gate and no invented `tuning`, every deviation from the brief explained, checked with the reader before it is handed over. |
| `/jud:cases <rubric> [brief]` | Writes the Cases a rubric is tuned on: real states covering every outcome, labels only for what each case is sure about, a `note` on the hard ones, bound to the rubric and checked, with a count of cases per outcome so the gaps are visible. |
| `/jud:check [files]` | Runs the reader over the files given, or every `.jud` under the project, cases bound to their rubric and recordings verified, and explains every refusal; fixes only when asked, and in the document rather than around it. |

## What is in it

Under `plugins/jud/`:

| Path | What it is |
|---|---|
| `.claude-plugin/plugin.json` | The manifest: name, version, author, where the documentation is. |
| `commands/rubric.md`, `cases.md`, `check.md` | The three commands above, each a page of instructions that follows the skill. |
| `skills/jud/SKILL.md` | The guide: the workflow, the rules the reader enforces that writers trip on, how to write questions that hold up, how to review an existing document, what to report. |
| `skills/jud/references/format.md` | Version 1.3 in one page: the envelope (`apiVersion`, `kind`, `metadata`, `spec`) and names, the YAML rules, every field of the three kinds, the gate fields per primitive, the label shapes, the fingerprints, and a checklist of what the reader refuses. Condensed from [the specification](jud.md), which wins on any difference. |
| `skills/jud/scripts/jud.sh` | Runs the `jud` command: a `jud` on `PATH` if there is one, else the one built from the judgment checkout the skill sits in, or the one `JUDGMENT_DIR` names. |
| `skills/jud/evals/` | The six tasks the skill was tested on and the broken documents, in the old envelope, one of them moves forward and repairs. |

The skill triggers on a task that touches a `.jud` file, a rubric, typed questions with thresholds, labelled cases, or questions written in code that should become a document. It does not need the word "jud" in the request.

## The `jud` command

The checker is `src/bin/jud.rs`, a binary of this crate behind the `jud` feature. It is the crate's reader as a command, so what it accepts, the crate accepts, and what it refuses, it refuses with the same message.

```sh
cargo run -q --features jud --bin jud -- check examples/jud/*.jud examples/recordings/jud_calibration/*.jud
mise run jud:check                      # the same, over the examples
mise run jud:check -- path/to/a.jud     # over the files given
mise use -g github:chussenot/judgment@latest   # a `jud` on PATH, from the release tarball (docs/cli.md)
```

`jud check FILE...` reads every file. A cases document is bound to the rubric it names among the files, or to the only rubric given, so each label is checked against the request its case lowers to: a label for a question whose `when` is not present in that state, an option the case was not offered, a level past the last one, are all refused here and nowhere later. A recording whose `metadata.name` names a case of a bound document, or one turn of it as `<case>-turn-<n>`, is verified against that request as a live response would be, and its fingerprint compared with the one the request has. The output is one line per document with its name, the `apiVersion` it declares and its fingerprints, `error` lines naming the file and the field, and a summary; the exit status is 1 when any document was refused.

```text
rubric    examples/jud/triage.jud: id inbox-triage, jud/v1.3, 3 questions, 3 gates (tuned)
          questions sha256:fda347c2cba6deaa8ad784360c46a8289adbc0284b373c3d9162ce11f3e7ac8a
          policy    sha256:1585094f59711426a98044a00ded2ec747010b5369e941f33a2732a50262e99f
          tuned on  sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
cases     examples/jud/triage-cases.jud: id inbox-triage-cases, jud/v1.3, 7 cases, bound to examples/jud/triage.jud
          cases     sha256:d752d8066b2067b299512b2d3153ca0aeaed0284589dc5624039aed1abd895b6
recording examples/recordings/jud_calibration/receipt.jud: case receipt, jud/v1.3, model jev-1.13.0, verified, fingerprint matches
3 documents, 0 refused
```

`jud lower RUBRIC` prints the request a rubric lowers to, as the questions map the wire carries, for one state (`--state JSON` or `--state-file PATH`, with `--options JSON` for a Choice whose options come with the request) or for every case of a cases document (`--cases FILE`). It is how a `when`, a `part_when` or an `options_from` is seen rather than guessed: a question missing from a case's request cannot be labelled for that case, and a part missing from the instructions was left out on purpose.

```sh
cargo run -q --features jud --bin jud -- lower examples/jud/routing.jud --cases examples/jud/routing-cases.jud
cargo run -q --features jud --bin jud -- lower examples/jud/routing.jud \
  --state '{"message": {"text": "hi"}, "customer": {"open_tickets": [{"id": "T-1"}]}}' \
  --options '{"desk": {"billing": "Invoices"}, "duplicate_of": {"T-1": "Locked out"}}'
```

## How the skill works

1. Decide the kind. A decision needs a rubric; a rubric's thresholds need cases, so the two are usually written together. A recording is written by hand only as a test fixture.
2. Read `references/format.md` before writing.
3. Write the document in the 1.3 envelope (`apiVersion: jud/v1.3`, `kind`, `metadata.name`, `spec`), starting from the nearest example under `examples/jud/`.
4. Check it with `scripts/jud.sh check`, rubric and cases together, and fix every refusal in the document rather than around it.
5. When the request depends on the state, look at what each case sends with `scripts/jud.sh lower`.
6. Report the files, the `jud check` summary and, when the rubric will be pinned, its two fingerprints.

The guide spends most of its words on what the checker cannot see, because that is where an agent without it goes wrong: one question per thing, criteria that define the outcomes rather than restate the question, a `none_of_these` option, questions asked only when the state carries what they are about, facts the application already has kept in code, and a policy that is honest about being a guess until a tuning run writes its `tuning` block. A section on turning questions written in code into a rubric says how the builder calls map to the document and how parity is proved with `jud lower`, and one on moving a document forward says how the earlier envelope (a `jud:` key with the fields at the top level) is rewritten into this one, because the reader refuses it by name rather than reading it.

## How it was tested

The skill and its eval set were carried over to the manifest envelope after the test below, which was run before it. The skill was run against six tasks of the kind a user would actually give, each once with the skill and once without it, by independent agents in the same checkout, and the outputs were graded by a script that runs `jud check` on them and checks every assertion in `evals/evals.json` against the parsed documents. The baseline agents could read the specification, the examples and the `jud` command; what they lacked was the guide.

1. A rubric and six labelled cases for triaging GitHub issues, from a one-paragraph brief.
2. A rubric over a conversation and four labelled transcripts, with `from_turn` labels.
3. A routing rubric whose options come with each request, with `when`, `part_when`, bands and a static fallback, and cases that supply the options.
4. Two broken documents from "last quarter" (a merge key, ids with spaces, `yes` and `no` as Noul criteria and label, a one-option Choice, gates on the wrong primitive, a level past the scale, a path-shaped case id) to be repaired into valid documents with a report of what was wrong. The fixtures stay in the envelope of the time, so the task now also moves them forward.
5. The two questions of `examples/quickstart.rs` turned into a rubric whose request is byte for byte the code's (checked by fingerprint), with a labelled case and a hand-written recording that `jud check` verifies with a matching fingerprint.
6. A brief with traps: a name with spaces, a threshold asked for in the instructions, a twelve-level scale, a one-option Choice and a request to invent a `tuning` block, to be met with a valid document and an explanation of each deviation.

Every document written with the skill passed every assertion (49 of 49); without it, 48 of 49, the miss being a document declared under an older version than the one wanted. The difference the skill makes is mostly in cost and method: in the first round, 14% fewer tokens and 24% less wall-clock on average, ten tool calls per task against fifteen, and no agent with the skill built a throwaway crate to validate its documents, which two baselines did. The second round changed the guide in two places: Score levels are written as plain strings, because an object level's text is its JSON and a `level_at_least` or a label would have to repeat it; and a section on turning questions written in code into a rubric, so parity is proved with `jud lower` rather than a probe crate. The eval set stays in the skill, its prompts and assertions now in the 1.3 format, so a later change to the format or the guide can be run against the same tasks.
