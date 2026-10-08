---
title: Run in CI
description: How to check every .jud document in a repository, replay a smoke case with no key, fail a job when a model's recorded answers no longer meet the labels with jud eval and its exit status 3, and run a rubric with a key in a CI job, with the binary installed or as the container image, and how to read the exit status in a script.
status: current
last_reviewed: 2026-10-08
tags: [judgment, jud, ci, github-actions, replay, eval, how-to]
---

# Run in CI

**You will** have a job that refuses a broken rubric, replays a known case without a key, fails when a model's recorded answers no longer meet the labels, and runs a rubric with a key when one is provided. **Prerequisites:** rubrics and cases ([Write a rubric](write-a-rubric.md), [Label cases](label-cases.md)); recordings of the cases, committed to the repository ([Record, replay and test](record-replay-and-test.md)).

## Get the binary into the job

Any route from [Install](../start/install.md) works in a job; the two that need no toolchain:

```yaml
# GitHub Actions, with mise
- uses: jdx/mise-action@v2
- run: mise use -g github:chussenot/judgment@X.Y.Z
```

or the container image, which needs no install step at all ([Run in a container](run-in-a-container.md)). Pin a version either way. `X.Y.Z` stands for a release that has `jud eval`, `jud record` and `jud tune`, which a job that holds the labels needs. 0.10.4 has `check` and `--replay` but not those three, which arrived after it: pin a release that lists them in [the changelog](../../CHANGELOG.md).

## Check every document

`jud check` reads the files as the crate does, binds cases to the rubric they name among the files, verifies recordings against the request they answer, and exits 2 when any document is refused:

```sh
jud check rubrics/*.jud
```

Give it the rubric and its cases together, so each label is checked against the request its case lowers to. With the image:

```sh
docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/jud:X.Y.Z check rubrics/*.jud
```

## Replay a smoke case with no key

A directory of recordings is a backend: a state that was recorded is answered exactly as it was, and a state nobody recorded fails with status 1 rather than guessing. That is what makes it a test.

```yaml
- name: Check the rubrics and replay the smoke case
  run: |
    jud check rubrics/*.jud
    jq -c . tests/smoke-event.json \
      | JUD_REPLAY=tests/recordings jud rubrics/triage.jud \
      | jq -e '.desk.key == "billing"'
```

`jq -e` exits non-zero when the verdict is not what the test expects, so the job fails for the right reason.

## Hold the labels with `jud eval`

A change can break the labels in two ways. A label or a recording can change so that the model's accuracy falls below the bar, and a case or a question can change so that a request has no recording. `jud eval` over committed recordings catches both, with no key, no network, and no file written:

```yaml
- name: Hold the labels
  run: jud eval rubrics/triage.jud cases/triage.jud --replay recordings/triage --min-accuracy 0.9
```

`--min-accuracy 0.9` holds every labelled question. `--min-accuracy desk=0.95` holds one, and the flag repeats. The bar holds the model's accuracy per question, as [Tune thresholds](tune-thresholds.md#2-hold-the-labels-with-a-gate) explains. The job fails for two different reasons, with two statuses:

- **Status 3** means a question's accuracy is below its bar. The report is on stdout, and the message on stderr names the questions.
- **Status 1** means a case has no recording, because a case was added, its state changed or a question was edited, and nobody recorded the new request. The message names the cases:

```text
jud: no recording answers 1 case: password-reset; record them first with `jud record`
```

A step fails on any status other than 0. To tell the two apart in the job's log, read the status:

```yaml
- name: Hold the labels
  run: |
    status=0
    jud eval rubrics/triage.jud cases/triage.jud --replay recordings/triage --min-accuracy 0.9 || status=$?
    case "$status" in
      0) ;;
      3) echo "::error::a question fell below its accuracy bar" ;;
      1) echo "::error::a case has no recording: run jud record and commit the recordings" ;;
      *) echo "::error::jud was invoked wrongly" ;;
    esac
    exit "$status"
```

`--min-accuracy` does not hold the policy's own accuracy, the `accuracy when acted` of a gate. To hold that, read the JSON report. A gate that acted on no labelled answer has no accuracy and fails the check:

```yaml
- run: |
    jud eval rubrics/triage.jud cases/triage.jud --replay recordings/triage --json \
      | jq -e '[.questions[] | select(.gate != null) | .gate.accuracy_when_acted >= 0.95] | all'
```

When a case is added, record it on your machine, with a key, and commit the file. `jud record` keeps what is already in the directory and asks only for the new case:

```sh
jud record rubrics/triage.jud cases/triage.jud --out recordings/triage
git add recordings/triage
```

The job never records. Recording needs a key and a directory it can write, and a job that rewrites its own recordings cannot fail.

With the image, `eval` reads through a read-only mount, and `docker run` exits with the container's status, so 1, 2 and 3 reach the job unchanged:

```sh
docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/jud:X.Y.Z \
  eval rubrics/triage.jud cases/triage.jud --replay recordings/triage --min-accuracy 0.9
```

`record` is the command that needs a writable mount and a key; `tune --out` needs the writable mount and no key. The image runs as uid 65532, so the directory must be writable by that user, or run the container as yourself:

```sh
docker run --rm -v "$PWD:/work" --user "$(id -u):$(id -g)" -e TYPESAFE_API_KEY ghcr.io/chussenot/jud:X.Y.Z \
  record rubrics/triage.jud cases/triage.jud --out recordings/triage
```

## Run a rubric with a key

A key is a secret passed through the job's masked variables, never written into a file in the job:

```yaml
- run: cat event.json | jud rubrics/triage.jud
  env:
    TYPESAFE_API_KEY: ${{ secrets.TYPESAFE_API_KEY }}
```

With the image, `-e TYPESAFE_API_KEY` copies the variable in.

## A job that runs inside an image

A CI system that runs a job *inside* an image (GitLab's Docker executor, a Kubernetes job, a Tekton step) runs the job's script through a shell in that image, and `ghcr.io/chussenot/jud` has none, so it fails before the first line of the script. Build a job image of your own that takes the binary from this one:

```dockerfile
FROM alpine:3.21
COPY --from=ghcr.io/chussenot/jud:X.Y.Z /usr/local/bin/jud /usr/local/bin/jud
```

`jud` is statically linked, so it runs on any Linux base, musl or glibc, and the bytes are still the release's.

## Read the exit status

[The jud command line](../reference/cli.md#exit-status) defines each status. In a job they read:

| Status | What it asks of the job |
|---|---|
| 0 | Nothing: read the verdicts, or `jud eval`'s report, with `jq` |
| 1 | A call or a recording failed: retry later, or record the case (again) with `jud record` |
| 2 | Wrong before any call: fix the job |
| 3 | `jud eval` only: the labels are no longer met, so read the report |

Every failure the command finds goes to stderr as `jud: MESSAGE`; a usage error that clap catches reads `error:` instead, also on stderr with status 2. Stdout carries the result, so a pipeline does not read a failure as a result, with one exception: `jud check` reports its refusals as `error` lines in its result, on stdout. `jud eval` prints its report on stdout before it exits 3.

## Next

- [Tune thresholds](tune-thresholds.md), which the same recordings feed.
- [Record, replay and test](record-replay-and-test.md): record a case you added.
