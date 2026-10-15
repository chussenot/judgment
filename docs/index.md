---
title: judgment documentation
description: Typed, calibrated judgments from System One models as a Rust crate and a command line; where to start by what you want to do.
status: current
last_reviewed: 2026-10-15
tags: [judgment, index]
---

# judgment

A decision written as a file, answered by a calibrated model, verified before it is read, and testable with no key. `judgment` is a Rust crate and a command line, `jud`, for [TypeSafe](https://docs.typesafe.ai) System One models and every server that speaks the same wire. The questions, the thresholds, the labelled cases and the recorded answers live in `.jud` files that name each other by content.

Thirty seconds of the command line, recorded from the examples with no key: [`demo.cast`](demo.cast), played with `asciinema play docs/demo.cast`.

Not affiliated with TypeSafe AI.

## Pick your path

| You are | Start with | Then |
|---|---|---|
| Evaluating whether this fits | [System One](concepts/system-one.md), [How judgment works](concepts/how-judgment-works.md) | [Stability](reference/stability.md), [Compared with the other Rust clients](project/research/client-comparison.md) |
| Using `jud` from a shell, no Rust | [Install](start/install.md), [Your first decision from the command line](start/first-decision-cli.md) | [Configure a backend](guides/configure-a-backend.md), [Record, replay and test](guides/record-replay-and-test.md#record-a-rubrics-cases-from-the-shell), [Tune thresholds](guides/tune-thresholds.md), [Run in CI](guides/run-in-ci.md), [The jud command line](reference/cli.md) |
| Writing `.jud` documents | [Rubrics, cases and recordings](concepts/rubrics-cases-recordings.md), [Write a rubric](guides/write-a-rubric.md) | [Label cases](guides/label-cases.md), [Tune thresholds](guides/tune-thresholds.md), [The .jud format](reference/jud-format.md) |
| Using the crate from Rust | [Your first decision in Rust](start/first-decision-rust.md) | [Record, replay and test](guides/record-replay-and-test.md), [Patterns](guides/patterns.md), [The crate](reference/crate.md) and the [rustdoc](https://docs.rs/judgment) |
| Asking about images | [Visual judgments](concepts/visual-judgments.md) | [Ask a question about an image](guides/ask-about-an-image.md), [Locate something with a Box question](guides/locate-with-a-box.md), [A Box calibration study](project/verification/box-calibration-study.md) and [its second run on document pages](project/verification/box-calibration-study-pages.md) |
| Contributing | [Contributing](project/contributing.md) | [Internals](project/internals.md), [How the crate is checked](project/verification/method.md), [Decisions](project/decisions/README.md) |

## The documentation

- **Start**: [Install](start/install.md), [first decision from the command line](start/first-decision-cli.md), [first decision in Rust](start/first-decision-rust.md).
- **Guides**: [Configure a backend](guides/configure-a-backend.md), [Write a rubric](guides/write-a-rubric.md), [Label cases](guides/label-cases.md), [Record, replay and test](guides/record-replay-and-test.md), [Tune thresholds](guides/tune-thresholds.md), [Run in CI](guides/run-in-ci.md), [Run in a container](guides/run-in-a-container.md), [Use the Claude Code plugin](guides/use-the-claude-code-plugin.md), [Patterns](guides/patterns.md), [Ask a question about an image](guides/ask-about-an-image.md), [Locate something with a Box question](guides/locate-with-a-box.md), [Use long context](guides/use-long-context.md).
- **Reference**: [The jud command line](reference/cli.md), [Configuration](reference/configuration.md), [The .jud format](reference/jud-format.md), [The crate](reference/crate.md), [The container image](reference/container-image.md), [Stability](reference/stability.md), [Glossary](reference/glossary.md); the [rustdoc](https://docs.rs/judgment) for every type.
- **Concepts**: [System One](concepts/system-one.md), [Rubrics, cases and recordings](concepts/rubrics-cases-recordings.md), [How judgment works](concepts/how-judgment-works.md), [Visual judgments](concepts/visual-judgments.md).
- **Project**: [Contributing](project/contributing.md), [Internals](project/internals.md), [Releasing](project/releasing.md), [verification](project/verification/method.md) against real servers, [research](project/research/compatible-servers-and-models.md), [decisions](project/decisions/README.md), the [CHANGELOG](../CHANGELOG.md); [llms.txt](llms.txt) and [llms-full.txt](llms-full.txt) for an agent.

The crate began as the client layer of another application and moved here with its history; how any one application uses it belongs to that application's documentation. A page belongs here when it would still be true, and still be needed, if no particular application existed.
