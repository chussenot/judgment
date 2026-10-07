---
title: Run in CI
description: How to check every .jud document in a repository, replay a smoke case with no key, and evaluate with a key in a CI job, with the binary installed or as the container image, and how to read the exit status in a script.
status: current
last_reviewed: 2026-10-07
tags: [judgment, jud, ci, github-actions, replay, how-to]
---

# Run in CI

**You will** have a job that refuses a broken rubric, replays a known case without a key, and evaluates with a key when one is provided. **Prerequisites:** rubrics and cases ([Write a rubric](write-a-rubric.md), [Label cases](label-cases.md)); recordings for the replay ([Record, replay and test](record-replay-and-test.md)).

## Get the binary into the job

Any route from [Install](../start/install.md) works in a job; the two that need no toolchain:

```yaml
# GitHub Actions, with mise
- uses: jdx/mise-action@v2
- run: mise use -g github:chussenot/judgment@0.10.4
```

or the container image, which needs no install step at all ([Run in a container](run-in-a-container.md)). Pin a version either way.

## Check every document

`jud check` reads the files as the crate does, binds cases to the rubric they name among the files, verifies recordings against the request they answer, and exits 2 when any document is refused:

```sh
jud check rubrics/*.jud
```

Give it the rubric and its cases together, so each label is checked against the request its case lowers to. With the image:

```sh
docker run --rm -v "$PWD:/work:ro" ghcr.io/chussenot/jud:0.10.4 check rubrics/*.jud
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

## Evaluate with a key

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
COPY --from=ghcr.io/chussenot/jud:0.10.4 /usr/local/bin/jud /usr/local/bin/jud
```

`jud` is statically linked, so it runs on any Linux base, musl or glibc, and the bytes are still the release's.

## Read the exit status

| Status | Meaning | In a script |
|---|---|---|
| 0 | Verdicts on stdout | read them with `jq` |
| 1 | The backend was asked and the call failed, or its answer did not fit the rubric, or no recording for the state | retry later, or a missing recording to add |
| 2 | Wrong before any call: the file, the input, a missing key, the configuration | fix the job |

Every error goes to stderr, prefixed `jud:`; stdout carries verdicts and nothing else, so a pipeline never reads an error as a result ([The jud command line](../reference/cli.md#exit-status)).

## Next

- [Tune thresholds](tune-thresholds.md), which the same recordings feed.
