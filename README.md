# judgment

[![crates.io](https://img.shields.io/crates/v/judgment.svg)](https://crates.io/crates/judgment)
[![docs.rs](https://img.shields.io/docsrs/judgment)](https://docs.rs/judgment)

A decision written as a file, answered by a calibrated model, verified before
it is read, and testable with no key: typed judgments from TypeSafe System One
models and every server that speaks the same wire, as a Rust crate and a
command line.

A System One model does not generate text. It evaluates a JSON state against
typed questions and returns calibrated probabilities: a yes or no, one option
out of a set with the distribution and a confidence, a position on ordered
levels. The threshold that turns a number into an action is yours, written in
a file or in code, tested against labelled cases and tuned from recorded
answers. This crate makes four things properties of the types rather than
habits of the caller: an answer can only be read as what was asked, a
probability is a probability, a transient failure does not fail the decision,
and the decision is testable without a key.

Not affiliated with TypeSafe AI.

## Install

```sh
mise use -g github:chussenot/judgment@latest     # Linux x86-64 and arm64, Apple silicon
brew install chussenot/tap/jud                   # the same tarballs, through Homebrew
cargo binstall judgment                          # or cargo install judgment --features cli
docker run -i --rm -v "$PWD:/work" ghcr.io/chussenot/jud triage.jud < event.json
```

```toml
[dependencies]
judgment = "0.11"
```

[Install](docs/start/install.md) has every route, with checksums and
attestations.

## The command line

A rubric is the decision as a file: the questions a model is asked about a
JSON state, and the policy that turns its answers into verdicts. Pipe the
state in, the verdicts come out, one per question:

```sh
$ cat event.json
{"message": "This is the third time I'm writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel."}
$ cat event.json | jud triage.jud
{
  "actionable": {
    "verdict": "yes",
    "probability": 0.97
  },
  "desk": {
    "verdict": "option",
    "key": "billing",
    "confidence": 0.87
  },
  "tone": {
    "verdict": "level",
    "index": 2,
    "label": "angry",
    "value": 1.86,
    "confidence": 0.84
  }
}
```

`triage.jud` is [`examples/jud/triage.jud`](examples/jud/triage.jud); the
transcript is what the binary prints for that state, replayed from a
recorded answer with no key and no network (`tests/jud_cli.rs` holds it to
that). The example recordings are scripted answers labelled `jev-1.13.0`,
not a model's. With `TYPESAFE_API_KEY` set the model answers;
`TYPESAFE_BASE_URL` points the same command at any other server that speaks
the wire. [Your first decision from the command line](docs/start/first-decision-cli.md)
walks through it.

The loop around a rubric runs from the shell too: `jud record` keeps a
model's answers to the rubric's labelled cases as recordings, `jud eval`
grades them against the labels (and exits 3 below an accuracy bar you set),
and `jud tune` proposes each gate's bar from them. The last two replay the
recordings (`--replay DIR`) with no key. [Record, replay and test](docs/guides/record-replay-and-test.md#record-a-rubrics-cases-from-the-shell)
and [Tune thresholds](docs/guides/tune-thresholds.md) walk through it.

## The Rust API

The same decision in code: typed questions, a handle per question that fixes
its answer's type, and a backend that checks every answer against the
question before it is read. This runs as is, with no key
([`examples/quickstart.rs`](examples/quickstart.rs)):

```rust
use judgment::{Fake, Questions, SystemOne, options};

options! {
    enum Department {
        Billing = "billing" => "Payments, invoicing, refunds",
        Technical = "technical" => "Bugs, outages, integrations",
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> judgment::Result<()> {
    let mut questions = Questions::new();
    let dept =
        questions.choice::<Department>("department", "Which team should handle `message`?")?;
    let urgent = questions.noul("is_urgent", "Does `message` convey urgency?", None)?;

    // Scripted answers, checked against the questions like a server's would be.
    // `Client::from_env()?` asks the real model instead.
    let backend = Fake::new()
        .choice("department", [("billing", 0.92), ("technical", 0.08)], 0.85)?
        .noul("is_urgent", 0.81)?;

    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let response = backend.answer(&state, "jev-latest", &questions).await?;

    let dept = response.get(&dept)?; // Choice<Department>
    let urgent = response.get(&urgent)?; // Noul
    let page_billing =
        dept.chosen == Department::Billing && dept.confidence.at_least(0.7) && urgent.is_yes(0.6);
    assert!(page_billing);
    println!("page billing on-call: {page_billing}");
    Ok(())
}
```

To ask the real model, build a `Client` (`Client::from_env()?` reads
`TYPESAFE_API_KEY`) and pass it where the fake is: both implement
`SystemOne`. [Your first decision in Rust](docs/start/first-decision-rust.md)
continues with the same decision from a `.jud` file and against the hosted
API.

## Documentation

[docs/index.md](docs/index.md) picks a path by what you want to do; the
rustdoc on [docs.rs](https://docs.rs/judgment) is the reference for every
type.

- **Start**: [Install](docs/start/install.md), [first decision from the command line](docs/start/first-decision-cli.md), [first decision in Rust](docs/start/first-decision-rust.md).
- **Guides**: [Configure a backend](docs/guides/configure-a-backend.md) (TypeSafe, Ollama, Laya, Clef, any server), [Write a rubric](docs/guides/write-a-rubric.md), [Label cases](docs/guides/label-cases.md), [Record, replay and test](docs/guides/record-replay-and-test.md), [Tune thresholds](docs/guides/tune-thresholds.md), [Run in CI](docs/guides/run-in-ci.md), [Run in a container](docs/guides/run-in-a-container.md), [Use the Claude Code plugin](docs/guides/use-the-claude-code-plugin.md), [Patterns](docs/guides/patterns.md).
- **Reference**: [The jud command line](docs/reference/cli.md), [Configuration](docs/reference/configuration.md), [The .jud format](docs/reference/jud-format.md), [The crate](docs/reference/crate.md), [The container image](docs/reference/container-image.md), [Stability](docs/reference/stability.md), [Glossary](docs/reference/glossary.md).
- **Concepts**: [System One](docs/concepts/system-one.md), [Rubrics, cases and recordings](docs/concepts/rubrics-cases-recordings.md), [How judgment works](docs/concepts/how-judgment-works.md).
- **Project**: [Contributing](docs/project/contributing.md), [Internals](docs/project/internals.md), [Releasing](docs/project/releasing.md), [verification](docs/project/verification/method.md) against real servers, [research](docs/project/research/client-comparison.md) including the comparison with the other Rust clients, [decisions](docs/project/decisions/README.md), [CHANGELOG](CHANGELOG.md), [llms.txt](docs/llms.txt).

## Status

On [crates.io](https://crates.io/crates/judgment) since 0.3.0; 0.x means a
minor release may break, so pin the minor. [Stability](docs/reference/stability.md)
says what holds across versions, the `.jud` format first. Live behaviour has
been verified against the hosted API, Laya's server and Ollama; the vendored
OpenAPI document and the wiremock tests are the contract in this repository.
Licensed MIT.
