# judgment

[![crates.io](https://img.shields.io/crates/v/judgment.svg)](https://crates.io/crates/judgment)

A decision written as a file, answered by a calibrated model, verified before
it is read, and testable with no key: typed judgments from TypeSafe System One
models and every server that speaks the same wire, as a Rust crate and a
command line.

Not affiliated with TypeSafe AI.

## The command line

```sh
mise use -g github:chussenot/judgment@latest   # Linux x86-64 and arm64, Apple silicon
brew install chussenot/tap/jud                 # the same tarballs, through Homebrew; or cargo install judgment --features cli
docker run -i --rm -v "$PWD:/work" ghcr.io/chussenot/jud triage.jud < event.json   # in a pipeline, no install
```

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

`triage.jud` is [`examples/jud/triage.jud`](examples/jud/triage.jud): three
questions about a support message and a policy whose gates were tuned on the
labelled cases beside it. The transcript above is what the binary prints
(`tests/jud_cli.rs` holds it to that), answered from a recording of
`jev-1.13.0` under [`examples/recordings/`](examples/recordings/jud_calibration)
with `JUD_REPLAY` set, so it ran with no key and no network;
`scripts/record_demo.sh` records the same session as an asciinema cast. With
`TYPESAFE_API_KEY` set the model answers; `TYPESAFE_BASE_URL`, or
`~/.config/jud/config.yaml`, points the same command at any other server that
speaks the wire, and `jud config` shows what a run would use. Errors go to
stderr with a non-zero status: 1 when the backend call failed, 2 when
something is wrong before any call (the file, the input, the configuration).
[The jud command line](docs/cli.md) has the install by hand with checksums,
shell completion, the configuration file and the other subcommands. An agent
writes and checks these files with [the jud plugin](docs/skill.md).

## The Rust API

```toml
[dependencies]
judgment = "0.10"
# What the first example uses besides the crate.
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt"] }
```

The same decision in code: typed questions, a handle per question that fixes
its answer's type, and a backend that checks every answer against the
question before it is read.

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

That example runs as is, with no key and no network: `cargo run --example
quickstart` ([`examples/quickstart.rs`](examples/quickstart.rs)). The `Fake`
backend checks its scripted answers against the questions the way a server's
response is checked, so the decision is tested on answers the real model could
have given. To ask the real model, build a `Client` (`Client::from_env()?`
reads `TYPESAFE_API_KEY`) and pass it where the fake is: both implement
`SystemOne`. The crate works with [TypeSafe](https://docs.typesafe.ai) System
One models (Jev) and with any server that speaks the same wire.

The same decision again, with the questions and the thresholds that read
their answers in one `.jud` document instead of in code, so another tool, or a
reviewer, can see both without reading Rust:

```rust
use judgment::jud::{Rubric, Supplied, Verdict};
use judgment::{Fake, SystemOne};

// The same questions as above, with the thresholds beside them, in a file
// another tool can read (docs/jud.md). The policy is never sent to the model.
const RUBRIC: &str = r"
apiVersion: jud/v1.3
kind: Rubric
metadata:
  name: inbox-triage
spec:
  questions:
    department:
      type: choice
      instructions: Which team should handle `message`?
      criteria:
        billing: Payments, invoicing, refunds
        technical: Bugs, outages, integrations
    is_urgent:
      type: noul
      instructions: Does `message` convey urgency?
  policy:
    department:
      confidence: 0.7
      fallback: technical
    is_urgent:
      threshold: 0.6
";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parsing checks the rubric as the builder checks questions; lowering
    // builds the request for this state.
    let rubric = Rubric::parse(RUBRIC)?;
    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let questions = rubric.lower(&state, &Supplied::default())?;

    let backend = Fake::new()
        .choice("department", [("billing", 0.92), ("technical", 0.08)], 0.85)?
        .noul("is_urgent", 0.81)?;
    let response = backend.answer(&state, "jev-latest", &questions).await?;

    // The policy reads the answers: an option at or above its confidence
    // bar, a yes at or above its threshold, or a deferral to the fallback.
    let verdicts = rubric.apply(&questions, &response)?;
    let page_billing = matches!(
        (&verdicts["department"], &verdicts["is_urgent"]),
        (Verdict::Option { key, .. }, Verdict::Yes { .. }) if key == "billing"
    );
    assert!(page_billing);
    println!("page billing on-call: {page_billing}");
    Ok(())
}
```

`cargo run --example jud_quickstart --features jud`
([`examples/jud_quickstart.rs`](examples/jud_quickstart.rs)). The rubric is
checked as the builder checks questions, lowered to the same request, and its
policy turns the answers into verdicts. The thresholds come from labelled cases
rather than guesses: [the .jud format](docs/jud.md) describes the rubric, the
cases and the recordings, and `examples/jud_calibration.rs` tunes a policy from
them. Every id in a document is a name and never a path, the reader refuses
what hides text from a reviewer (a merge key, a foreign tag), and a rubric's
policy has a fingerprint of its own beside its questions', so a moved threshold
is as visible as a changed question. The format is behind the `jud`
feature, off by default.

An agent writes these files with [the jud plugin](docs/skill.md): a skill and
three commands (`/jud:rubric`, `/jud:cases`, `/jud:check`) that check every
document with the crate's own reader. This repository is its marketplace:

```sh
claude plugin marketplace add chussenot/judgment
claude plugin install jud@judgment
mise use -g github:chussenot/judgment@latest   # the `jud` command the plugin checks with
```

The third example asks the real model. It reads a rubric from a file,
[`examples/jud/triage.jud`](examples/jud/triage.jud), three questions about a
support message and a policy whose gates were tuned on the labelled cases
beside it, and sends the request to the hosted TypeSafe API:

```rust
use std::time::Duration;

use judgment::jud::{Rubric, Supplied};
use judgment::{Client, SystemOne};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A rubric from a file: its questions, and the policy tuned on the
    // labelled cases beside it, which names them by fingerprint.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/jud/triage.jud");
    let rubric = Rubric::parse(&std::fs::read_to_string(path)?)?;

    // The hosted API with the key from TYPESAFE_API_KEY, where the fake was;
    // TYPESAFE_BASE_URL points it at any server that speaks the wire.
    let mut builder = Client::builder().timeout(Duration::from_secs(30));
    if let Ok(url) = std::env::var("TYPESAFE_BASE_URL") {
        builder = builder.base_url(url);
    }
    let client = builder.build()?;
    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let questions = rubric.lower(&state, &Supplied::default())?;
    let response = client.answer(&state, "jev-latest", &questions).await?;

    // The policy reads the model's answers into one verdict per question.
    for (id, verdict) in rubric.apply(&questions, &response)? {
        println!("{id}: {}", serde_json::to_string(&verdict)?);
    }
    println!("answered by {}", response.model);
    Ok(())
}
```

```sh
$ cargo run --example jud_live --features jud
actionable: {"verdict":"yes","probability":0.98}
desk: {"verdict":"option","key":"billing","confidence":0.7}
tone: {"verdict":"level","index":1,"label":"annoyed","value":1.02,"confidence":0.9}
answered by jev-1.13.0
```


`TYPESAFE_API_KEY=... cargo run --example jud_live --features jud`
([`examples/jud_live.rs`](examples/jud_live.rs)) spends one model call and
prints one verdict per question, each a yes or no, an option or a level with
its confidence, or a deferral to the gate's fallback; `TYPESAFE_BASE_URL`
sends the same request to any other server that speaks the wire. Nothing in the code names
a question or a threshold: both live in the file, and the model's answers are
verified against the questions the file lowered to before the policy reads
them. `examples/jud_calibration.rs` is where the policy's numbers come from.

The three examples above show the mechanics on one message. The ones under
[`examples/`](examples/) show the shapes a System One call takes inside a
real program, one per pattern TypeSafe documents, each in its own domain and
each replaying recorded answers so it runs with no key and no network
(`-- --live` asks the model, `-- --record` rewrites the recordings):

- [`fan_out.rs`](examples/fan_out.rs): speculative fan-out, every question a
  decision tree might need in one request, the branch taken reading only its
  handles.
- [`confidence_routing.rs`](examples/confidence_routing.rs): confidence-gated
  routing, the answer says what and the confidence says whether to act, each
  action with its own bar.
- [`composite_scoring.rs`](examples/composite_scoring.rs): composite scoring,
  atomic Scores combined with weights the code owns, re-weighted over
  recordings without a new call.
- [`intent_routing.rs`](examples/intent_routing.rs): intent routing, a cheap
  classifier in front of expensive handlers, with a second question gating the
  escalation.
- [`jud_calibration.rs`](examples/jud_calibration.rs): the `.jud` loop, a
  rubric and its labelled cases from files, answers replayed, thresholds swept
  and written back with their provenance.
- [`typed_decisions.rs`](examples/typed_decisions.rs): the typed-decisions
  benchmark replayed through the crate and scored, against any server that
  speaks the wire.

[Patterns](docs/patterns.md) says which type carries each shape and what the
recordings teach about the thresholds.

## Why

A generative model asked to classify something answers in prose, or in JSON it
was told to produce. A `confidence` field in that JSON is generated text, not a
measured probability, and a parse failure becomes a failure of the decision it
was meant to inform. A System One model does not generate text. It evaluates a
`state` (any JSON) against typed questions and returns calibrated answers: a
probability of yes, one option out of a defined set with the full distribution
and a confidence, or a position on ordered levels. Calibrated means the
probabilities can be measured against outcomes and held to account. That lets
code own the workflow: the model supplies a number, and the threshold that
turns the number into an action is written, tested and tuned in code.

The wire is simple. What is hard is trusting a decision made on a probability
that came over it: the answer must belong to the question that was asked, the
probability must be a probability, a transient failure must not fail the
decision, and the decision must be testable without a key. This crate exists
to make those four things properties of the types rather than habits of the
caller. It began as the client layer of an application that routes alerts on
such judgments, where a second project wanting the same layer had to depend on
the whole application; the layer became a crate, with every rule about what to
do with an answer left to the caller.

## What it guarantees

- **An answer can only be read as what was asked.** Adding a question returns
  a `Handle` that fixes its answer's type; reading through it yields a Rust
  enum, a probability or a score, never a misread one. Before anything is read,
  every backend holds the whole response against the questions it was sent: an
  answer for every question, of its primitive, a Choice naming only options it
  was offered, a Score whose legend is the levels sent. An option nobody
  offered is an error, never a guess.
- **A probability is a probability.** `Probability` and `Confidence` refuse a
  value outside `[0, 1]` when they are decoded, and cannot be confused with
  each other at a threshold.
- **A request that would be refused never costs a call.** The limits of the
  HTTP API reference (255 options, 2 to 10 levels, non-empty and unique ids)
  are checked before sending.
- **Transient failures are retried the way the official SDKs retry them**, and
  nothing that was billed is retried: two attempts more on 408, 429, 5xx and
  transport failures, the server's wait honoured, a response that does not fit
  reported but never re-sent. The request id TypeSafe's support asks for is on
  every response, every error and every span.
- **A decision is testable with no key and no network.** A `Fake` that refuses
  an answer its question could not produce, a `Recorder` and a `Replay` keyed
  by the request's content, and `eval` to grade recordings against labels with
  accuracy, Brier score and calibration error.
- **The parts the wire has no place for have a file.** The questions with the
  thresholds that read their answers, the labelled cases the thresholds were
  tuned on and the recorded answers are three kinds of one YAML format,
  [`.jud`](docs/jud.md), that name each other by content fingerprint, so a
  threshold says which cases and which model it rests on, and another tool
  can read all three (feature `jud`, off by default).

Point the client at another server with `TYPESAFE_BASE_URL`, or build without the
`http` feature to keep the questions, the answers, the fake and replay backends
and the metrics for a project with its own transport.

## Compared with the other Rust clients

About thirty Rust crates speak this wire. One table, the four properties where
they differ most, for the most downloaded crates on crates.io on 2026-10-04 and
`typesafe-client`, the one closest in architecture; each cell is read from the
published source of the version named. ✓ present, ◐ partial, ✗ absent.
[Compared with the other Rust clients](docs/research/client-comparison.md) has
the full grids for all ten crates (wire limits, retries, footprint and more),
a file and line for every cell, and what other crates have that judgment does
not.

| Crate | Typed handle per question | Response verified against the questions | Validated probability types | Testing without a key |
|---|---|---|---|---|
| judgment 0.10.0 | ✓ | ✓ every backend, legend included | ✓ | ✓ fake that refuses unfit answers, record and replay |
| kunobi-decision 0.3.0 | ✓ | ✗ | ✗ | ◐ fake, no replay |
| typesafe-sdk 0.2.0 | ✗ | ✗ | ✗ | ✗ |
| typesafe-ai-sdk 0.5.0 | ◐ derive | ✗ | ✗ | ◐ replay (SHA-256 cassettes), no fake |
| typesafeai-sdk-community 0.5.0 | ◐ derive | ✗ | ✗ | ◐ mock, answers unchecked; replay in order |
| typesafe-client 0.1.0 | ✓ | ◐ legend not compared | ✗ | ◐ fake that refuses unfit answers, no replay |

## Documentation

The crate's documentation lives under [`docs/`](docs/index.md); the rustdoc on
[docs.rs](https://docs.rs/judgment) is the reference for every type, error and
default.

| Page | What it answers |
|---|---|
| [How judgment works](docs/design.md) | How a handle ties a question to its answer, how a response is checked before it is read, how the retry loop decides |
| [What is in the crate](docs/tour.md) | What each module is for and what it promises |
| [How the crate is implemented](docs/implementation.md) | The rules the code applies and why: the decoder, the response check, the error bodies, the per-call options, the retry loop's edges, the recordings, the metrics and the .jud reader |
| [Patterns](docs/patterns.md) | TypeSafe's four patterns on the crate's types, one runnable example each, and what the recordings teach about thresholds |
| [Testing without the model](docs/testing.md) | The fake, the recordings and the metrics |
| [Open-weight models without an account](docs/open-weights.md) | The crate, its examples and the `jud` binary against tev1 or Clef-flash on Ollama, with no key, and what a local server does differently |
| [The .jud format](docs/jud.md) | The specification of the rubric, cases and recording documents, their fingerprints and reading rules, and the loop from labelled cases to a tuned policy |
| [What a rubric is](docs/jud/rubric.md), [what cases are](docs/jud/cases.md), [what a recording is](docs/jud/recording.md) | The reasoning behind each kind of `.jud` document: the decision, the labelled examples it is graded on, and what the model answered |
| [The jud command line](docs/cli.md) | `cat input.json \| jud rubric.jud`: the binary that evaluates JSON input against a Rubric and prints the verdicts, how to install it with mise, its configuration and exit status |
| [The jud container image](docs/container.md) | The binary as `ghcr.io/chussenot/jud`: what is inside, how files and a backend reach it, the host's Ollama through `--network host`, replay without a key, and CI |
| [The jud plugin](docs/skill.md) | A Claude Code plugin, a skill and three commands, that writes and checks `.jud` documents with the crate's own reader (`jud check`), and how to install it |
| [How the crate is checked](docs/verification/method.md) | The live tests, the benchmark replay and the contract test, and how to run them against the hosted API, Laya, Ollama or Clef on Workers AI |
| [Against the hosted TypeSafe API](docs/verification/hosted-typesafe.md), [Against Laya typed-decisions](docs/verification/laya-typed-decisions.md) | What real servers did with the live tests, and what the crate changed for it |
| [Compatible servers and models](docs/research/compatible-servers-and-models.md) | Which servers speak the wire and how the open models compare with Jev |
| [Compared with the other Rust clients](docs/research/client-comparison.md) | The full comparison grids, with a file and line for every cell |
| [Stability](docs/stability.md) | What stays the same across versions and for how long: `jud/v1` stable, a minor only adds, every `v1` document read for at least twelve months after a later minor, fingerprints fixed for a `spec` |
| [Decisions](docs/decisions/README.md) | Why the API is shaped as it is, and why releases are cut the way they are |
| [Releasing](docs/releasing.md) | How a version is cut and published |
| [llms.txt](docs/llms.txt) | The index for agents and models; [llms-full.txt](docs/llms-full.txt) is every page in one file |

## Status

On [crates.io](https://crates.io/crates/judgment) since 0.3.0; 0.x means a
minor release may break, so pin the minor; [Stability](docs/stability.md)
says what holds across versions, the `.jud` format first. What changed in each release,
breaking changes listed, is in [CHANGELOG.md](CHANGELOG.md). Live behaviour has
been verified against the hosted API and against Laya's server; the vendored
OpenAPI document and the wiremock tests are the contract in this repository.
Licensed MIT.

Not affiliated with TypeSafe AI.
