# judgment

Typed, calibrated judgments from [TypeSafe](https://docs.typesafe.ai) System One
models (Jev) and from any server that speaks the same wire, as a Rust crate.

```toml
[dependencies]
judgment = "0.3"
```

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

```rust
use judgment::{Client, Questions, options};

options! {
    enum Department {
        Billing = "billing" => "Payments, invoicing, refunds",
        Technical = "technical" => "Bugs, outages, integrations",
    }
}

async fn run() -> judgment::Result<()> {
    let mut questions = Questions::new();
    let dept = questions.choice::<Department>("department", "Which team should handle `message`?")?;
    let urgent = questions.noul("is_urgent", "Does `message` convey urgency?", None)?;

    let client = Client::from_env()?;
    let state = serde_json::json!({ "message": "My payouts have been failing for 3 days." });
    let response = client.system_one(&state, &questions).await?;

    let dept = response.get(&dept)?;     // Choice<Department>
    let urgent = response.get(&urgent)?; // Noul
    if dept.chosen == Department::Billing && dept.confidence.at_least(0.7) && urgent.is_yes(0.6) {
        // page billing on-call
    }
    Ok(())
}
```

Point it at another server with `TYPESAFE_BASE_URL`, or build without the
`http` feature to keep the questions, the answers, the fake and replay backends
and the metrics for a project with its own transport.

## Compared with the other Rust clients

About thirty Rust crates speak this wire, most of them a few weeks old. The
grids below set judgment against the nine most downloaded on crates.io on
2026-10-04 and `typesafe-client`, the one closest in design, each read from
its published source and measured on one machine. The full comparison, with a
file and line for every cell and the columns judgment loses, is in
[Compared with the other Rust clients](docs/research/client-comparison.md).
✓ present, ◐ partial, ✗ absent.

**The wire and the questions**

| Crate | Structured Score levels | Enum and runtime option sets | Limits checked before sending | Per-call overrides | Builds without the HTTP client |
|---|---|---|---|---|---|
| **judgment 0.3.0** | ✓ | ✓ | ✓ | ✓ | ✓ |
| kunobi-decision 0.3.0 | ✓ | ✓ | ◐ duplicate id replaces | ✓ | ✗ |
| typesafe-sdk 0.2.0 | ◐ | ◐ strings | ✗ | ✓ | ✗ |
| typesafeai-sdk 0.4.1 | ✓ | ◐ random order | ◐ no caps | ✓ | ✗ |
| typesafe-sdk-* 0.6.2 | ✓ | ◐ strings | ◐ | ✓ | ◐ |
| jev-client 0.2.0 | ✓ | ◐ strings | ◐ opt-in | ✗ | ✗ |
| jev 0.1.2 | ✗ | ◐ alphabetical | ✗ | ✗ | ✗ |
| typesafe-ai-sdk 0.5.0 | ✓ | ✓ | ◐ no caps | ✓ | ✗ |
| typesafeai-sdk-community 0.5.0 | ✓ | ◐ alphabetical | ◐ no caps | ✓ | ✗ |
| typesafe-client 0.1.0 | ✓ | ✓ | ✓ | ◐ | ✓ |

**Reading answers safely**

| Crate | Typed handle | Validated probability types | Response verified against the questions | Unknown answer kind kept | Request id on errors |
|---|---|---|---|---|---|
| **judgment** | ✓ | ✓ | ✓ every backend, legend included | ✓ | ✓ |
| kunobi-decision | ✓ | ✗ | ✗ | ✓ | ✓ |
| typesafe-sdk | ✗ | ✗ | ✗ | ◐ dropped | ✓ |
| typesafeai-sdk | ✗ | ✗ | ✗ | ◐ payload lost | ✓ |
| typesafe-sdk-* | ✗ | ✗ | ✗ | ✗ fails | ✓ |
| jev-client | ✗ | ✗ | ✗ | ✓ | ✓ |
| jev | ✗ | ✗ | ✗ | ◐ | ✗ |
| typesafe-ai-sdk | ◐ derive | ✗ | ✗ | ◐ | ✓ |
| typesafeai-sdk-community | ◐ derive | ✗ | ✗ | ◐ | ✓ |
| typesafe-client | ✓ | ✗ | ◐ legend not compared | ✗ fails | ✓ |

**Resilience**

| Crate | SDK retry defaults | Server's wait, capped | Conservative preset | Body cap | Redirects not followed | Key never in `Debug` | Concurrency control |
|---|---|---|---|---|---|---|---|
| **judgment** | ✓ | ✓ 30 s | ✓ | ✓ 8 MiB | ✓ | ✓ | ✗ |
| kunobi-decision | ✓ | ✓ 60 s | ✗ | ✗ | ✓ | ✓ | ✓ |
| typesafe-sdk | ✓ | ◐ uncapped | ✗ | ✗ | ✗ | ✗ | ✗ |
| typesafeai-sdk | ◐ | ◐ no date | ✗ | ✓ 1 MiB | ✓ | ✓ | ✗ |
| typesafe-sdk-* | ✓ | ✓ 60 s | ✗ | ✗ | ✗ | ✓ | ✗ |
| jev-client | ✓ | ✓ 60 s | ✗ | ✓ 32 MiB | ✓ | ✓ | ✓ |
| jev | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| typesafe-ai-sdk | ✓ | ◐ uncapped | ✗ | ✗ | ✗ | ✓ | ✗ |
| typesafeai-sdk-community | ✓ | ◐ uncapped | ✗ | ✗ | ✗ | ◐ | ✓ |
| typesafe-client | ✓ | ✓ 60 s | ✗ | ✗ | ✗ | ✓ | ✗ |

**Testing, evaluation and footprint**

| Crate | Fake that refuses an unfit answer | Record and replay | Contract test against the OpenAPI document | Evaluation metrics | Verified against real servers | Transitive deps / clean build |
|---|---|---|---|---|---|---|
| **judgment** | ✓ | ✓ content hash | ✓ | ✓ with intervals | ✓ hosted API, Laya | 100 / 49 s |
| kunobi-decision | ✓ | ✗ | ✗ | ✗ | ✗ | 97 / 44 s |
| typesafe-sdk | ✗ | ✗ | ✗ | ✗ | ✗ | 104 / 25 s |
| typesafeai-sdk | ✗ | ✗ | ✗ | ✗ | ✗ | 91 / 23 s |
| typesafe-sdk-* | ◐ mock, unchecked | ✗ | ✗ | ✗ | ✗ | 110 / needs rustc 1.98 |
| jev-client | ◐ trait only | ✗ | ◐ own schema | ✗ | ✗ | 107 / 27 s |
| jev | ✗ | ✗ | ✗ | ✗ | ◐ one document | 91 / 40 s |
| typesafe-ai-sdk | ✗ | ✓ SHA-256 cassettes | ✗ | ✗ | ✗ | 113 / 43 s |
| typesafeai-sdk-community | ◐ mock, unchecked | ◐ in order | ✗ | ✓ no intervals | ◐ | 110 / 47 s |
| typesafe-client | ✓ | ✗ | ✓ | ✗ | ◐ hosted API | 99 / 45 s |

Where judgment is behind, and what it will take from the others next: a
pacer that pauses every in-flight call on one 429, provider presets as data,
`ranked()` and `margin()` helpers, and arithmetic checks in verification. The
comparison page lists them with the crate each idea comes from.

## Documentation

The crate's documentation lives under [`docs/`](docs/index.md); the rustdoc on
[docs.rs](https://docs.rs/judgment) is the reference for every type, error and
default.

| Page | What it answers |
|---|---|
| [How judgment works](docs/design.md) | How a handle ties a question to its answer, how a response is checked before it is read, how the retry loop decides |
| [What is in the crate](docs/tour.md) | What each module is for and what it promises |
| [Patterns](docs/patterns.md) | TypeSafe's four patterns on the crate's types, one runnable example each, and what the recordings teach about thresholds |
| [Testing without the model](docs/testing.md) | The fake, the recordings and the metrics |
| [How the crate is checked](docs/verification/method.md) | The live tests, the benchmark replay and the contract test, and how to run them against the hosted API, Laya or Ollama |
| [Against the hosted TypeSafe API](docs/verification/hosted-typesafe.md), [Against Laya typed-decisions](docs/verification/laya-typed-decisions.md) | What real servers did with the live tests, and what the crate changed for it |
| [Compatible servers and models](docs/research/compatible-servers-and-models.md) | Which servers speak the wire and how the open models compare with Jev |
| [Compared with the other Rust clients](docs/research/client-comparison.md) | The grids above in full, with evidence |
| [Decisions](docs/decisions/README.md) | Why the API is shaped as it is, and why releases are cut the way they are |
| [Releasing](docs/releasing.md) | How a version is cut and published |
| [llms.txt](docs/llms.txt) | The index for agents and models; [llms-full.txt](docs/llms-full.txt) is every page in one file |

## Status

On [crates.io](https://crates.io/crates/judgment) since 0.3.0; 0.x means a
minor release may break, so pin the minor. What changed in each release,
breaking changes listed, is in [CHANGELOG.md](CHANGELOG.md). Live behaviour has
been verified against the hosted API and against Laya's server; the vendored
OpenAPI document and the wiremock tests are the contract in this repository.
Licensed MIT.
