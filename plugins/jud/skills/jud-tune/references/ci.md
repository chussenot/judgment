# Holding a rubric in CI

Part of the `jud-tune` skill (`${CLAUDE_PLUGIN_ROOT}/skills/jud-tune/SKILL.md`), which says when to read this page; its rules on running commands ("How to run them") apply here too. `/jud:ci` follows it; the guide `docs/guides/run-in-ci.md` in the judgment repository is the long form.

## What a job holds

A directory of recordings is a backend: a state that was recorded is
answered exactly as it was, with no key and no network, and a state
nobody recorded is status 1, never a guess. So a job can refuse a broken
document, replay a known case, and fail when the model's recorded
answers no longer meet the labels, all without a key:

```sh
jud check rubrics/*.jud cases/*.jud recordings/triage/*.jud
jud eval rubrics/triage.jud cases/triage.jud --replay recordings/triage --min-accuracy 0.9 --fail-if-moved
```

- **`--min-accuracy [QUESTION=]X`** holds the model's accuracy per
  question, over every labelled case, and never the gate's `accuracy
  when acted`: a deferred miss still counts, and no bar, fallback or
  note moves it. A bare `X` holds every labelled question; a question
  with no labelled case does not meet its bar. Only a fixed label, a
  sharper question (a re-recording) or another model raises it.
- **`--fail-if-moved`** holds the rubric to the model and server its
  `tuning` block names: status 3 when the recordings answered from
  another. A rubric retuned since its recordings were made, or recordings
  refreshed on a new model, rests its bars on other answers than the ones
  graded. Leave the flag out when the block names nothing.
- **The gate's own accuracy** is held through the JSON: `jud eval ...
  --json | jq -e '[.questions[] | select(.gate != null) |
  .gate.accuracy_when_acted >= 0.95] | all'`. A gate that acted on no
  labelled answer has no accuracy and fails it.

## The four statuses in a log

| Status | Meaning | The log line |
|---|---|---|
| 0 | every bar met | nothing |
| 1 | a case has no recording (a case added, a state or a question edited), or a recording no longer fits its questions | `::error::a case has no recording: run jud record and commit the recordings` |
| 2 | the job is wrong before any call: a document refused, a bar outside 0 to 1, a question name the rubric does not ask, a missing file | `::error::jud was invoked wrongly` |
| 3 | `eval` only: a question's accuracy is below its bar, or, with `--fail-if-moved`, the model or server moved | `::error::a question fell below its accuracy bar, or the model moved` |

Read the status into a variable so the log says which:

```yaml
- name: Hold the labels
  run: |
    status=0
    jud eval rubrics/triage.jud cases/triage.jud --replay recordings/triage --min-accuracy 0.9 --fail-if-moved || status=$?
    case "$status" in
      0) ;;
      3) echo "::error::a question fell below its accuracy bar, or the model moved" ;;
      1) echo "::error::a case has no recording: run jud record and commit the recordings" ;;
      *) echo "::error::jud was invoked wrongly" ;;
    esac
    exit "$status"
```

The report is on stdout before `eval` exits 3, so the log has the numbers.

## The binary in the job

Pin a release, since a bar is tuned against one binary's reading:

```yaml
- uses: jdx/mise-action@v2
- run: mise use -g github:chussenot/judgment@X.Y.Z
```

or the container image, which needs no install step:
`docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/jud:X.Y.Z eval ...`
exits with the container's status, so 1, 2 and 3 reach the job. A job
that runs *inside* the image needs a shell the image has none of: build a
job image from `alpine` that copies `/usr/local/bin/jud` out of it.

## What the job never does

- **Record.** Recording needs a key and a writable directory, and a job
  that rewrites its own recordings cannot fail. A new case is recorded
  on a machine with a key and committed with its recording (`jud record`
  keeps what is there and asks only for the new case).
- **Lower a bar to pass.** A bar the recordings do not meet today is a
  finding for `/jud:eval`, not a number to move.
- **Replay uncommitted recordings.** `git ls-files DIR` must list them;
  an empty listing is a status 1 waiting to happen.

## Writing the bar

With no bar given, propose one per question from today's report: the low
end of `accuracy_interval95` rounded down to 0.05 is the floor the
recordings meet with room for a relabel; the accuracy itself is a bar
that fails on the next honest miss. Say which was written and why.
