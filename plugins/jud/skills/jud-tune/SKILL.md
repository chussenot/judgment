---
name: jud-tune
description: Measure and tune a .jud rubric against its labelled cases. Record a model's answers once, read what jud eval says about accuracy, calibration and misses, and decide whether each miss is a wrong label, a weak question, a bar or the model. Then apply what jud tune proposes to the rubric in place and show the before and after. Use this whenever a task is about how well a rubric's questions or thresholds work, why a decision came out wrong, recordings, calibration, accuracy, thresholds that "feel off", or tuning or evaluating .jud files, even when the user does not say "jud", "eval" or "tune".
---

# Tuning a .jud rubric

A rubric's questions are asked of a model; its policy turns the answers into
actions at bars (`threshold`, `confidence`, `level_at_least`). The bars are
only as good as the cases they were measured on. Three `jud` commands make
the measurement; this guide is about reading what they print and deciding
what to change, which they do not do.

The commands run through the plugin's script, so they work wherever `jud` is
on PATH or a judgment checkout is at hand:

```sh
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh record RUBRIC CASES --out DIR     # spends one call per request; keeps the answers
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh eval RUBRIC CASES --replay DIR    # grades the kept answers; no key, no network
${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh tune RUBRIC CASES --replay DIR    # proposes each gate's bar; never calls anything
```

How to run them, because a user's permission rules match the command as
written:

- Write the script's path out in full every time. Do not set a shell
  variable or `cd` first. Below, `jud.sh` is short for that path.
- Run one `jud` command per Bash call, with nothing chained before or after
  it (`;`, `&&`, `|`, `echo $?`) and no redirection into a file. The tool
  result shows stdout, stderr and a non-zero exit status.
- The same holds for every other command: run `git diff -- support.jud`
  as written, from the working directory, with no `-C`, no `cd`, no
  absolute path.
- Read files with the Read tool and edit them with the Edit tool. Make a
  copy with Read and Write, not `cp` or `mkdir`: the Write tool creates the
  directory.
- Write nothing into the user's project that the task did not ask for: no
  copy of the rubric, no saved output. A what-if copy (a cases document with
  one label changed, to measure what the change would do) or a split you
  were not asked to keep goes in a directory of its own outside the project:
  the session's scratchpad or temporary directory if the system names one,
  else `/tmp/jud-<rubric name>-<what>/` (for example
  `/tmp/jud-support-triage-relabel-refund-request/`). Do not run a command
  to find `$TMPDIR`. Build the copy with Read then Write only, no `sed`,
  `cp`, `mkdir` or redirection. If the file already exists, Read it in full
  before overwriting it, and never run `jud` on a copy this conversation
  did not write. The reply says it was a copy, gives its absolute path as
  written, and says the project's files were not changed. An edited copy of
  the rubric, made to measure a proposal before it is applied, is such a
  copy too.

The format and how to write a rubric or cases are the `jud` skill's
(`${CLAUDE_PLUGIN_ROOT}/skills/jud/SKILL.md`); read it before changing a
question or a label.

Every reply after `eval` or `tune` ends with the items in "What the reply
contains" (the last section): one line per question, the shortfall in cases,
the triage table, coverage counts, and for `tune` the decision table, the
warnings as printed, and the before and after. Check them off before you
send it.

## What costs what, and what invalidates what

This decides the order of everything else, so know it first.

| You change | Recordings still answer? | Cost of the next measurement |
|---|---|---|
| A gate in `policy` (a bar, a fallback, a note), or the `tuning` block | yes: the policy is never sent | none: `eval` and `tune` replay |
| A label in a case's `expect`, or a case's `note` | yes: labels are never sent | none |
| A case's `state` or `options`, or a new case | no, for that case | one call per new request |
| Anything under `questions` (instructions, criteria, options, levels, `when`) | no, for every case that asks it | one call per case |
| An image's bytes (resized, re-encoded, replaced), under a case's or the rubric's `assets` | no, for every case that sends it: a recording is keyed by each image's digest as well as the state | one call per such case; a `path` that moves while the bytes stay costs nothing |
| A label's `iou_at_least`, or the box in a label | yes: labels are never sent | none |
| The model (`model` in `jud config`) | the old answers still replay, but they are the old model's | a full record into a new directory (`record --out NEW`), then `eval --replay NEW --against OLD` to see where it answers differently before labelling; `--refresh` writes over the old answers |

A state over the model's context (`max_tokens_exceeded`, 400, from the
hosted API at 32k tokens for the state and one question, 64k for the
whole request) fails that case's call. `--long-context` on `record` or
`eval`, or `long_context: true` in `jud`'s configuration file, raises both
limits to 262k for every call of the run, at a cost TypeSafe names (slower,
less accurate); the recordings are keyed as without it. Say which it is
when you propose it: turn it on when the long states are the rubric's own
subject, shorten the state when they are not, or leave the case out and
say so. Never turn it on for a whole run because one case is long.

A recording is keyed by the request's fingerprint (the state and the
questions sent, and each image by its digest). When the questions change, `eval --replay` exits with
status 1 and `no recording answers N cases: ...; record them first`. That
message means the rubric and the recordings no longer match, not that
something is broken. `jud.sh check RUBRIC CASES DIR/*.jud` shows which:
every recording whose request changed says `fingerprint differs`. All of
them means a question changed; one means that case's state or options
did. Never edit a recording to make it fit, and never revert a question
silently to make the old recordings work: say which change made them stale
and what re-recording costs.

So fix labels and bars freely; batch question changes, then re-record once.

## Where the rest of the guide is

The detail for each step is a page of its own under `references/`. Read the
page before the step, every time, even when you think you remember it: the
checks it lists are what a reply is held to.

- **Before `jud record`**, or an `eval` without `--replay` (both spend
  calls): `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/record.md`, how to read the dry-run plan and say the cost
  before the call.
- **After `jud eval`**, before writing about its output:
  `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/eval.md` (every number it prints, the gate line, the warnings,
  coverage) and `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/triage.md` (the table every miss goes in, and how to
  measure what a fix would do).
- **Before `jud tune`, before applying a proposal, and before splitting
  cases:** `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/tune.md` (how each bar is chosen, accept or keep, editing
  the rubric in place, holding cases out). A `tune` reply also needs the
  two `eval` pages: it triages the misses and reports coverage.

## Iterating

One round is: triage the misses, fix the labels the user agrees are wrong
(free), batch the question changes (one re-recording), add cases where
coverage is thin (calls only for the new cases), then tune again. Stop when
the misses left are the model's own and the bars rest on enough cases.

## Never

- Write a `tuning` block, a fingerprint, a model or a time that `jud tune`
  did not print.
- Edit, delete or hand-write a recording to make `eval` pass. A recording is
  what a model said.
- Change a label to match the model without the user agreeing that the
  label was wrong.
- Spend calls without saying how many first, in a message before the call:
  `record`, `eval` without `--replay`, or `record --refresh`.
- Mix two models' recordings in one directory. `tune` refuses them (`the
  recordings come from more than one model`); record each model into its own
  directory and compare the two `eval` reports.
- Change a label's box or its `iou_at_least` to match the model's box. The
  label is where the thing is; the bar is what the task needs.
- Resize or re-encode a case's image to get a recording to answer. The
  bytes are the key: a changed image is a new request, one call.
- Present a bar read off a handful of cases as anything but a guess with a
  number on it.

## What the reply contains

A reply that ran `eval` or `tune` has items 2 to 6, always, for every
question in the rubric, even when the user asked about one gate: answer
their question first, then give the items. One triage row per line under
`model misses` (with more than 10, group the rows by cause, but name every
case), and counts for every question. Items 1, 6
(for `eval` alone) and 7 depend on what was done. In this order:

1. The cost, before anything was spent (`record`), and what was spent:
   calls, or time on a local server.
2. One line per question: accuracy with its interval beside the majority
   outcome's share, whether confidence separates right from wrong, what
   the gate does (acts, defers, accuracy when acted), and, first, a
   collapse or a gate that defers nearly everything when there is one.
3. Whether the labelled count supports a bar, and how many more cases each
   question needs, for which outcomes; for a Box, how many images hold the
   thing and how many do not.
4. The triage table, a row for every miss.
5. Coverage: the thin outcomes, with the cases behind them.
6. For `tune`: the decision table (gate, primitive, now, proposed,
   decision, reason) and the warnings `tune` printed, in plain words, apart
   from your own observations.
7. What changed: the before and after per gate, held-out first; or the
   exact edit as a unified diff when nothing was applied. If no file
   changed, say so.
8. The next steps, ranked, each with its cost: free (labels, bars),
   calls for new cases, or a full re-recording (questions).

Quote a fingerprint in full, as `jud` printed it, or as its first 12 hex
digits followed by `…`; never write an ending you did not copy. A sentence
that sums up counts holds for every count listed.

Before sending it, check every number against the output it came from: a
count adds up to its total, a width is `high - low` from the JSON, and a
statement of what a change would do names the run that measured it and the
target it ran at ("at 0.95: still 0.80"), and for every other target you
discussed, gives the measured bar or says "unmeasured at 0.90". Never write
that a change "won't move any bar" unless the copy ran at every target you
weighed. An announced number repeated in the reply is the number that was
announced.
