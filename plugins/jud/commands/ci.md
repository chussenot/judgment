---
description: Write or update the CI job that holds a rubric's labels (jud check over every document, a smoke replay, jud eval --replay with --min-accuracy and --fail-if-moved, the four exit statuses mapped), for GitHub Actions or plain sh, after checking the recordings are committed and the bars pass today
argument-hint: "[rubric cases recordings dir ...] [github | sh] [--min-accuracy [QUESTION=]X ...] [--policy-accuracy X]"
---

Write the CI job that holds these rubrics to their labels:

$ARGUMENTS

Follow the jud-tune skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`
("How to run them") and `${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/references/ci.md`
(what each status means, what `--min-accuracy` holds and does not,
`--fail-if-moved`, the container form). Run every `jud` command through
`${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`, one per Bash call.
Read with the Read tool, write with the Write tool.

1. **Find the documents.** The rubrics, cases and recordings given, else
   every `.jud` under the project (Glob, skipping build output): a cases
   document names its rubric in `spec.rubric`, and a recordings directory
   is one whose files read `kind: Recording`. List the triples you found
   and ask nothing; a rubric with cases and no recordings gets a `check`
   step only, and the reply says so.
2. **Check them together** (`jud.sh check <every document>`, recordings
   included: `0 refused`, each cases document bound, each recording
   `fingerprint matches`). A refusal or a stale recording stops here:
   report it, offer `/jud:check` or `/jud:record`, and write no job for a
   state the job would fail on.
3. **Grade each rubric today**: `jud.sh eval <rubric> <cases> --replay
   <dir> --json` with the `--min-accuracy` bars given (none given: hold
   nothing yet, and propose a bar per question from its `accuracy` and
   `accuracy_interval95`, the interval's low end rounded down to 0.05, as
   the floor the recordings meet). A bar the recordings do not meet is
   not written: say the accuracy, the bar and the shortfall, and what
   would raise it (`eval.md`'s arithmetic). Say when the rubric's
   `tuning` names a model and the recordings came from another (`moved`):
   then `--fail-if-moved` would fail today, so leave it out and say why.
4. **Check the recordings are committed**: `git ls-files <dir>` as
   written. A job that replays uncommitted recordings fails on status 1;
   an empty listing stops here with `git add <dir>` as the step to take.
5. **Write the job.** GitHub Actions unless `sh` is asked: a new workflow
   file `.github/workflows/jud-<rubric name>.yml` (one file for every
   rubric found when there are several), or the steps the user names a
   file for. Ask before writing into an existing workflow file. The job:
   - installs `jud` pinned to the version on this machine (`jud.sh
     --version`), with `jdx/mise-action` and `mise use -g
     github:chussenot/judgment@X.Y.Z`, or the container image
     `ghcr.io/chussenot/jud:X.Y.Z` when the user asks for it;
   - `jud check` over every document;
   - `jud eval RUBRIC CASES --replay DIR --min-accuracy ...` per rubric,
     `--fail-if-moved` when the rubric's `tuning` names a model and the
     recordings came from it;
   - the four statuses mapped to `::error::` lines as `references/ci.md`
     shows, so a log says whether a label fell short (3), a case has no
     recording (1) or the job is wrong (2);
   - with `--policy-accuracy X`, the `jq -e` step over `--json` that
     holds every gate's `accuracy_when_acted` at or above X;
   - never a `jud record`: the job never records, since a job that
     rewrites its own recordings cannot fail.
   `sh` is the same as a script with `set -eu` and the status read into a
   variable.
6. **Reply** with the file written (or the steps, when none was), the
   bars it holds per question with today's accuracy beside each, what
   each status means in the log, which rubrics got a `check` step only
   and why, and that a new case is recorded on a machine with a key and
   committed with its recording (`/jud:record`).

Never write a job whose bars fail on today's recordings without saying
so; never lower a bar the user gave to make it pass; never add a step
that records or that writes into the repository.
