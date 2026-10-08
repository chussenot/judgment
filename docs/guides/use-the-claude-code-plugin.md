---
title: Use the Claude Code plugin
description: How to install the jud plugin from this repository's marketplace, write a rubric and its cases with /jud:rubric and /jud:cases, check them with /jud:check, and measure and tune them with /jud:record, /jud:eval and /jud:tune; what the two skills know that the reader and the numbers cannot say.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, skill, agent, claude-code, how-to]
---

# Use the Claude Code plugin

**You will** have an agent that writes, reviews and fixes `.jud` documents, checks every one with the crate's own reader, and measures and tunes a rubric's bars on its labelled cases. **Prerequisites:** [Claude Code](https://code.claude.com/docs/en/plugins); the `jud` command ([Install](../start/install.md)), which the plugin checks with.

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
| `/jud:record <rubric> <cases> [dir]` | Runs `jud record` after telling you, in a message of its own, how many requests it will spend (only those the directory does not already answer), to which backend and model. Stops with what to set when no key is set, and never asks for the key. |
| `/jud:eval <rubric> <cases> [dir]` | Grades the recordings with `jud eval --replay`, counts the labels per outcome, and puts every miss in one cause: a wrong label, an ambiguous question, a fact the model cannot see, overlapping outcomes, a coverage gap, the bar's job or the model. It proposes each fix with its cost and changes no file. Explains stale recordings rather than working around them. |
| `/jud:tune <rubric> <cases> <dir> [apply] [holdout]` | Runs `jud tune`, decides gate by gate whether to take the proposal, and shows the coverage a lower target would buy. It refuses a bar of 0 on a handful of cases and a bar that absorbs a wrong label. With `apply` it edits the rubric in place, keeping comments and the notes that still hold, and shows each gate before and after. With `holdout` it tunes on three quarters of the cases and reports the rest as the numbers to expect. |
| `/jud:rust <rubric> [out.rs] [embed]` | Writes the rubric as a small Rust module: an enum per Choice and Score, a `Decision` struct and one `decide(backend, model, state)` call, with the rubric read through `include_str!` so its requests are what `jud lower` sends and a retune needs no regeneration. It compiles the module, runs its test and clippy before handing it over, and replies with the `Cargo.toml` and `mod` lines and a usage example. |

The `jud` skill loads on its own whenever a task touches a `.jud` file, a rubric, typed questions with thresholds, labelled cases, or questions written in code that should become a document. The `jud-rust` skill loads when a task is about using a rubric from Rust ("call screening.jud from my service with real types"). The `jud-tune` skill loads when a task is about how well a rubric works: its accuracy, why a decision came out wrong, its thresholds or its recordings ("our rubric sends too many tickets to the wrong queue, fix the thresholds"). Neither needs the word "jud" in the request.

## What is in it

| Path | What it is |
|---|---|
| `skills/jud/SKILL.md` | The guide: the workflow, the rules the reader enforces that writers trip on, how to write questions and cases that hold up, how to turn questions written in code into a rubric, how to move a document from the earlier envelope forward, what to report. |
| `skills/jud/references/format.md` | [The .jud format](../reference/jud-format.md), generated into the plugin by `scripts/gen-plugin-format.sh` so the two cannot differ; CI checks it. |
| `skills/jud/scripts/jud.sh` | Runs the `jud` command: one on `PATH`, else one built from a judgment checkout. |
| `skills/jud-tune/SKILL.md` | The tuning guide: what each edit costs and which ones make recordings stale, how to count a run's calls, what every `jud eval` and `jud tune` number means and how many cases a bar needs, the miss triage table, when to accept, keep or re-run a proposed bar, how to apply it in place, how to hold cases out, and what a reply must contain. |
| `skills/jud-rust/SKILL.md` | The Rust guide: what the generated module is and why the rubric stays its source, the naming rules from `.jud` keys to Rust identifiers, the mapping per primitive and declaration, how to wire and verify it. |
| `skills/jud-rust/references/` | The two modules the skill copies its shape from, with their rubrics: copies of `examples/jud/triage.rs` and `routing.rs`, which `cargo test` compiles and runs (`examples/jud_typed.rs`); `scripts/gen-plugin-rust.sh --check` keeps the copies equal. |
| `commands/rubric.md`, `cases.md`, `check.md`, `record.md`, `eval.md`, `tune.md`, `rust.md` | The seven commands, each a page of instructions that follows its skill. |
| `skills/jud/evals/` | The six tasks the authoring skill was tested on. |
| `skills/jud-tune/evals/` | The battle test of the tuning commands: the scenarios, the support fixture with its planted wrong labels, a mock System One server and the runner (below). |

## How the skill works

1. Decide the kind. A decision needs a rubric; a rubric's thresholds need cases, so the two are usually written together. A recording is written by hand only as a test fixture.
2. Read the format reference before writing.
3. Write the document in the `jud/v1.3` envelope, starting from the nearest example under `examples/jud/`.
4. Check it with `scripts/jud.sh check`, rubric and cases together, and fix every refusal in the document rather than around it.
5. When the request depends on the state, look at what each case sends with `scripts/jud.sh lower`.
6. When bars are to be measured, hand over to the `jud-tune` skill: `jud record` runs only when asked, and only `jud tune` writes a `tuning` block's values.
7. Report the files, the `jud check` summary and, when the rubric will be pinned, its two fingerprints.

## How the tuning skill works

1. Know what an edit costs before making it. A bar, a note or a label is free, since recordings still answer. A case's state is one call. A question is one call per case: every recording of it goes stale, and `jud check` with the recordings shows which.
2. Before `jud record`, count the requests the directory does not already answer, read the backend and model from `jud config`, and say it to the user before the call.
3. Read `jud eval` from its JSON. Check whether confidence separates right answers from wrong ones, and what the gate does with them.
4. Put every miss in one cause and propose its fix, measured rather than guessed. A relabel is tried on a copy outside the project.
5. Take a proposed bar only when the cases support it and no wrong label set it. Otherwise keep the written bar with a reason, or show what a lower target buys.
6. Apply in place: values and notes of the gates that move, the `tuning` block as printed, every comment kept. Then show the before and after, from held-out cases when there are 40 or more.

## How the Rust skill works

A program that asks a rubric's questions from Rust has the crate's `Rubric::lower` and `Rubric::apply`, which give verdicts keyed by strings. The module the skill writes turns them into types the compiler checks, and adds no logic of its own:

- **The rubric stays the source.** `SOURCE` is `include_str!` of the `.jud` file, and the request is `Rubric::lower`. The module therefore sends what `jud lower` and `jud record` send, and recordings made with the command line replay against it.
- **Only questions are compiled in.** A Choice's options and a Score's levels become enums, a question asked only `when` the state has a path becomes an `Option`, and options supplied per request get an `Offered` struct. The bars, bands and fallbacks are read from the file at run time, so `jud tune` changes them with no regeneration.
- **Drift fails a test.** The module's own test compares its fingerprint and enums with the parsed rubric, so an edited question fails `cargo test` until the module is regenerated.
- **It is cheap to wire.** The module depends only on `judgment` (feature `jud`), `serde` and `serde_json`. It builds under `clippy::pedantic` with `unwrap_used` and `expect_used` denied. A program adds `mod triage;` and calls `triage::decide(&backend, "jev-latest", &state)`.

[`examples/jud_typed.rs`](../../examples/jud_typed.rs) is such a program: it routes the calibration cases through `examples/jud/triage.rs` from recordings, and tests `examples/jud/routing.rs` with `Fake`. Both modules are the skill's templates.

## Work on the plugin itself

Add the checkout as a local marketplace (`/plugin marketplace add ./` from the repository root) and reload with `/reload-plugins` after an edit; a plugin loaded from a local marketplace is read in place. The plugin is not published with the crate (`Cargo.toml` excludes `plugins/`); the `jud` binary is. `scripts/gen-plugin-format.sh` regenerates `references/format.md` after the specification changes.

## How it was tested

### The authoring skill

The skill was run against six tasks of the kind a user would give, each once with the skill and once without it, by independent agents in the same checkout; a script ran `jud check` on the outputs and checked every assertion in `evals/evals.json` against the parsed documents. The baseline agents could read the specification, the examples and the `jud` command; what they lacked was the guide.

1. A rubric and six labelled cases for triaging GitHub issues, from a one-paragraph brief.
2. A rubric over a conversation and four labelled transcripts, with `from_turn` labels.
3. A routing rubric whose options come with each request, with `when`, `part_when`, bands and a static fallback.
4. Two broken documents from "last quarter" (a merge key, ids with spaces, `yes` and `no` as Noul criteria, a one-option Choice, gates on the wrong primitive, a level past the scale, a path-shaped case id) to repair, with a report of what was wrong.
5. The two questions of `examples/quickstart.rs` turned into a rubric whose request is byte for byte the code's, checked by fingerprint, with a labelled case and a hand-written recording.
6. A brief with traps: a name with spaces, a threshold asked for in the instructions, a twelve-level scale, a one-option Choice and a request to invent a `tuning` block.

Every document written with the skill passed every assertion (49 of 49); without it, 48 of 49. The difference the skill makes is mostly in cost and method: in the first round, 14 % fewer tokens and 24 % less wall-clock on average, ten tool calls per task against fifteen, and no agent with the skill built a throwaway crate to validate its documents, which two baselines did. The test was run before the manifest envelope; the skill and its eval set were carried over to it afterwards, so a later change to the format or the guide can be run against the same tasks.

### The tuning commands

`/jud:record`, `/jud:eval` and `/jud:tune` were battle-tested as a user runs them: each scenario is one prompt through `claude -p --plugin-dir plugins/jud` in a fresh git workspace. The allow-list is what a user of the plugin would grant: the plugin's script, `jud`, file edits and a few read-only commands.

The backend is `skills/jud-tune/evals/mock_system_one.py`, a System One test double that knows the cases' labels. It answers right with a set accuracy, wrong at a middling confidence, deterministically, and can force an answer so that a scenario plants a wrong label or a confusion. Every run's base URL is the mock or a closed port, so no scenario can reach a real API.

The fixture under `skills/jud-tune/evals/files/support/` is a three-question rubric with 48 labelled tickets. Two labels are wrong on purpose and three tickets straddle billing and account. On it, `jud tune` proposes raising the queue bar to 0.80, which would cover 15 of 48 cases: the right answer is to refuse that bar and fix its causes.

The 14 scenarios cover:

- recording from scratch, resuming, over stale recordings, with no key, and over a conversation;
- grading, with and without `--min-accuracy`, and over recordings a question edit made stale;
- proposing, applying, holding out and a relabel what-if;
- a plain-language request ("fix the thresholds") that names no command.

Each run is held to two kinds of check:

- **Scripted checks** (`run.py`): which files changed, `jud check`, the questions fingerprint, the comments, the YAML values, the number of calls the mock saw, stray files and any `jud` command the permission rules denied.
- **Graded expectations**: an independent reader grades each scenario's expectations from the reply and the transcript, lists every claim the `jud` output contradicts, and names the page whose wording led the run astray.

The pages were rewritten between rounds from what the graders found. A round on fewer scenarios re-ran those the previous fixes touched:

| Round | Scenarios | Expectations met | Claims the output contradicts | What the round changed |
|---|---|---|---|---|
| 1 | 11 | 36 of 41 | not counted | `/jud:record` says the cost before the call; one `jud` command per call, nothing written into the project; a Noul gate decides rather than defers |
| 2 | 14 | 46 of 50 | 20 | `--min-accuracy` explained as a CI gate no bar can raise; what-ifs measured, per target; counts that add up to `labelled` |
| 3 | 14 | 50 of 50 | 10 | hold out by default at 40 cases; every question in every reply; the stale recordings named in the announcement |
| 4 | 6 | 18 of 20 | 11 | "fix the thresholds" no longer means "accept every proposal"; a what-if copy in a directory of its own |
| 5 | 6 | 19 of 21 | 13 | the cost line carried on the `record` call itself; a proposed edit built on a copy, checked and replayed before it is shown |
| 6 | 4 | 12 of 13 | 2 | a wrong label only ever raises a bar; a comment the tuning makes false is reworded, the others kept |
| final | 14 | 50 of 50 | 18 | a raise that defers only misses is accepted; a non-strict gate acts at its bar; fingerprints quoted exactly |
| confirm | 6 | 21 of 22 | 8 | the `tuning` block replaced whenever a proposal is accepted; a single what-if question answered, not the whole flow |

Every scripted check passed in the final round: 109 of 109, at $4.61 for the 14 runs on Sonnet 5.5, $0.15 to $0.76 each. The confirmation round re-ran the six scenarios its fixes touched, and the two its own fixes touched were run once more, all checks passing. Two weaknesses remain, both in the replies' prose rather than in what the commands do:

- **Contradicted claims.** Their number swings between rounds (2 to 20) with no trend after round 3. Most are arithmetic done by hand on label counts, which `jud eval --json` does not yet report per outcome.
- **The cost announcement.** A model writes it reliably as the `record` call's description, and less reliably as text before the call, so the check accepts either.

To run them again, with `claude` logged in and `jud` built:

```sh
JUD=target/debug/jud python3 plugins/jud/skills/jud-tune/evals/run.py --out /tmp/jud-tune-evals --jobs 6
```

`--only ID ...` runs some, and `--model` picks the model. Each scenario's `reply.md`, `transcript.jsonl` and `checks.json` land under `--out`. Grading the prose expectations is left to a reader.

## Next

- [Write a rubric](write-a-rubric.md) and [Label cases](label-cases.md), the guides the skill follows.
- [Tune thresholds](tune-thresholds.md), the loop behind the bars the skill never writes itself.
