---
description: Grade a .jud rubric's recorded answers against its labelled cases (jud eval), explain accuracy, calibration and the gates, and triage every miss into label, question, bar, coverage or model
argument-hint: "<rubric> <cases> [recordings dir] [--min-accuracy X] [live]"
---

Grade this rubric on its cases and say what to change:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
first, in particular "Reading `jud eval`", "Triage every miss", "Coverage"
and "What the reply contains".

Run every `jud` command through the plugin's script,
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, written out in full, one
command per Bash call: no shell variable, no `cd`, nothing chained before or
after it, no redirection into a file. The tool result shows the output and
a non-zero exit status. Read files with the Read tool. This command changes
no file.

1. **Resolve the arguments.** The rubric and the cases document are
   required; if either is missing, say so and stop. The recordings directory
   is the one given, else `recordings/<rubric metadata.name>/` beside the
   cases document when it exists.
2. **No recordings.** Do not grade live unless the arguments say `live`: a
   live `eval` spends one call per case and keeps nothing. Say that, and
   offer `/jud:record`.
3. **Grade.** Run `jud.sh eval <rubric> <cases> --replay <dir> --json`
   (with any `--min-accuracy` given). Use the JSON for every number you
   quote; run it once more without `--json` only if you want the text
   layout.
   - Exit 3: a `--min-accuracy` bar was not met. That is a finding, not a
     failed run. Report each question from `min_accuracy` in the JSON
     (accuracy against its bar, met or not). Say that this status is how CI
     holds the rubric against committed recordings: the same command in a
     CI job fails the build. Say that the floor holds the model's accuracy
     over every labelled case, which no bar or fallback moves, and what
     would raise it (a fixed label, a sharper question, another model), with
     the arithmetic.
   - Compare the rubric's `spec.tuning.cases` with `cases.fingerprint`:
     equal means the bars were tuned on these very cases, so the gate
     numbers are optimistic. It says nothing about the recordings.
   - `models` with more than one entry: the directory mixes models, which
     `tune` refuses. Say so.
   - `eval` does not report recordings that no case asks for. List the
     directory with Glob, and name the files whose stem is no case's name
     (a conversation's are `<case>-turn-<n>`), as your own count.
4. **Stale recordings.** Exit 1 with `no recording answers N cases: ...;
   record them first` means the rubric or the cases changed since recording.
   - Run `jud.sh check <rubric> <cases> <dir>/*.jud`. Every recording with
     `fingerprint differs` is stale. All of them: a question changed. Some:
     those cases' state or options changed.
   - Find which question: `git diff -- <rubric>` exactly as written, from
     the working directory (no `-C`, no `cd`), when the project is a git
     repository; `git log -p -1 -- <rubric>` if the change is committed. Without history, say that the recordings cannot tell
     (they keep answers, not the questions), and ask what was edited. If the
     recorded option keys and Score legends still match the rubric, the
     change is in instructions or criteria.
   - Reply with this and stop. Give the stale count, the change, and the
     cost (one call per stale request). Offer `/jud:record <rubric> <cases>
     <dir>`, which asks each stale case again and replaces its file, or a
     new directory to keep the old answers for comparison. Say it is not
     broken, only out of date. Give no accuracy or bar advice until there
     are recordings that answer. Do not edit recordings or revert the
     rubric.
5. **Coverage and signals.** Each question's `outcomes` gives the labels
   per outcome, every outcome listed; quote those counts. Name the cases
   behind every count of 3 or fewer from the cases document. Read
   `majority` beside the accuracy, and put each of the question's
   `signals` (`no_better_than_majority`, `collapsed`, `never_answered`,
   `defers_most`, `defers_nearly_all`)
   first in its line, as the skill says. A `jud` whose report has no
   `outcomes` predates them: then count the labels from the cases document,
   and the answers from `misses` plus the right ones.
6. **Triage every miss.** Read the case's state and note and the question's
   criteria, then put the miss in one row of the skill's triage table. A
   Score miss is printed as level indices; map them to level names first.
7. **Reply** as "What the reply contains" says, for `eval`:
   - the `--min-accuracy` verdict, when one was asked;
   - one line per question;
   - whether the count supports a bar;
   - the triage table;
   - coverage gaps;
   - ranked next steps with their cost.

   Leave bars and notes to `/jud:tune`. When a bar looks wrong, say so and
   point there. Proposed label and question changes are for the user to
   accept; apply none.
