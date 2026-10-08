---
title: Use the Claude Code plugin
description: How to install the jud plugin from this repository's marketplace, write a rubric and its cases from a brief with the /jud:rubric and /jud:cases commands, and check documents with /jud:check; what the skill knows that the reader cannot see.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, skill, agent, claude-code, how-to]
---

# Use the Claude Code plugin

**You will** have an agent that writes, reviews and fixes `.jud` documents and checks every one with the crate's own reader. **Prerequisites:** [Claude Code](https://code.claude.com/docs/en/plugins); the `jud` command ([Install](../start/install.md)), which the plugin checks with.

An agent writing a `.jud` file has two ways to get it wrong: the shape, which the reader refuses with the field named, and the substance, which no reader can see: a compound question, a Choice with no way out, a threshold the instructions already moved into the model, a `tuning` block with no run behind it. The plugin under [`plugins/jud/`](../../plugins/jud/) covers both: the format's specification shipped inside it, the rules writers trip on, and a command that reads a document exactly as the crate does.

## Install

```sh
claude plugin marketplace add chussenot/judgment   # once: this repository is the marketplace
claude plugin install jud@judgment                 # --scope project enables it for everyone in a repository
```

Or interactively: `/plugin marketplace add chussenot/judgment`, then `/plugin install jud@judgment`. The plugin has no dependency beyond the `jud` command: with none on `PATH`, its script builds one from a judgment checkout named by `JUDGMENT_DIR`, from the project when the project is this crate, or from the checkout the plugin sits in.

## The commands

| Command | What it does |
|---|---|
| `/jud:rubric <brief>` | Writes a Rubric from a brief, or revises one at a path: narrow questions with criteria, a policy with a `note` per gate and no invented `tuning`, every deviation from the brief explained, checked with the reader before it is handed over. |
| `/jud:cases <rubric> [brief]` | Writes the Cases a rubric is tuned on: real states covering every outcome, labels only for what each case is sure about, a `note` on the hard ones, bound to the rubric and checked, with a count of cases per outcome so the gaps are visible. |
| `/jud:check [files]` | Runs the reader over the files given, or every `.jud` under the project, cases bound to their rubric and recordings verified, and explains every refusal; fixes only when asked, and in the document rather than around it. |

The skill also loads on its own whenever a task touches a `.jud` file, a rubric, typed questions with thresholds, labelled cases, or questions written in code that should become a document. It does not need the word "jud" in the request.

## What is in it

| Path | What it is |
|---|---|
| `skills/jud/SKILL.md` | The guide: the workflow, the rules the reader enforces that writers trip on, how to write questions and cases that hold up, how to turn questions written in code into a rubric, how to move a document from the earlier envelope forward, what to report. |
| `skills/jud/references/format.md` | [The .jud format](../reference/jud-format.md), generated into the plugin by `scripts/gen-plugin-format.sh` so the two cannot differ; CI checks it. |
| `skills/jud/scripts/jud.sh` | Runs the `jud` command: one on `PATH`, else one built from a judgment checkout. |
| `commands/rubric.md`, `cases.md`, `check.md` | The three commands, each a page of instructions that follows the skill. |
| `skills/jud/evals/` | The six tasks the skill was tested on. |

## How the skill works

1. Decide the kind. A decision needs a rubric; a rubric's thresholds need cases, so the two are usually written together. A recording is written by hand only as a test fixture.
2. Read the format reference before writing.
3. Write the document in the `jud/v1.3` envelope, starting from the nearest example under `examples/jud/`.
4. Check it with `scripts/jud.sh check`, rubric and cases together, and fix every refusal in the document rather than around it.
5. When the request depends on the state, look at what each case sends with `scripts/jud.sh lower`.
6. When bars are to be measured, name `jud record`, `jud eval` and `jud tune`, and run them only when asked. Never write a `tuning` block: tell the user to paste what `jud tune` prints under `spec:`.
7. Report the files, the `jud check` summary and, when the rubric will be pinned, its two fingerprints.

## Work on the plugin itself

Add the checkout as a local marketplace (`/plugin marketplace add ./` from the repository root) and reload with `/reload-plugins` after an edit; a plugin loaded from a local marketplace is read in place. The plugin is not published with the crate (`Cargo.toml` excludes `plugins/`); the `jud` binary is. `scripts/gen-plugin-format.sh` regenerates `references/format.md` after the specification changes.

## How it was tested

The skill was run against six tasks of the kind a user would give, each once with the skill and once without it, by independent agents in the same checkout; a script ran `jud check` on the outputs and checked every assertion in `evals/evals.json` against the parsed documents. The baseline agents could read the specification, the examples and the `jud` command; what they lacked was the guide.

1. A rubric and six labelled cases for triaging GitHub issues, from a one-paragraph brief.
2. A rubric over a conversation and four labelled transcripts, with `from_turn` labels.
3. A routing rubric whose options come with each request, with `when`, `part_when`, bands and a static fallback.
4. Two broken documents from "last quarter" (a merge key, ids with spaces, `yes` and `no` as Noul criteria, a one-option Choice, gates on the wrong primitive, a level past the scale, a path-shaped case id) to repair, with a report of what was wrong.
5. The two questions of `examples/quickstart.rs` turned into a rubric whose request is byte for byte the code's, checked by fingerprint, with a labelled case and a hand-written recording.
6. A brief with traps: a name with spaces, a threshold asked for in the instructions, a twelve-level scale, a one-option Choice and a request to invent a `tuning` block.

Every document written with the skill passed every assertion (49 of 49); without it, 48 of 49. The difference the skill makes is mostly in cost and method: in the first round, 14 % fewer tokens and 24 % less wall-clock on average, ten tool calls per task against fifteen, and no agent with the skill built a throwaway crate to validate its documents, which two baselines did. The test was run before the manifest envelope; the skill and its eval set were carried over to it afterwards, so a later change to the format or the guide can be run against the same tasks.

## Next

- [Write a rubric](write-a-rubric.md) and [Label cases](label-cases.md), the guides the skill follows.
- [Tune thresholds](tune-thresholds.md), the loop behind the bars the skill never writes itself.
