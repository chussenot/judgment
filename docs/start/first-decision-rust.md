---
title: Your first decision in Rust
description: From an empty crate to a decision made on a model's answers, in Rust, with no account; first against the Fake backend, then the same decision from a .jud rubric, then against the hosted API with a key.
status: current
last_reviewed: 2026-10-07
tags: [judgment, rust, tutorial, getting-started, fake, jud]
---

# Your first decision in Rust

**You will** write a program that asks typed questions about a JSON state, reads the answers through handles that fix their types, and makes a decision on them; first with scripted answers and no account, then from a rubric file, then against the hosted API. **Prerequisites:** Rust 1.91 or later; no key until the last step.

## 1. Create the crate

```sh
cargo new first-decision && cd first-decision
```

```toml
[dependencies]
judgment = { version = "0.10", features = ["jud"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt"] }
```

`jud` is for step 3; the first program needs the crate alone.

## 2. Ask two questions and decide

Replace `src/main.rs` with this. It is [`examples/quickstart.rs`](../../examples/quickstart.rs) in the repository.

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

```sh
cargo run
```

```text
page billing on-call: true
```

What happened: `options!` wrote an enum and its wire keys and descriptions together, so the request's criteria and the parser come from one definition. Adding each question returned a handle that fixes its answer's type: `dept` can only be read as a `Choice<Department>`, `urgent` only as a `Noul`. The `Fake` checked its scripted answers against the questions the way a server's response is checked, so a scripted option the question did not offer would have failed the call. The decision is three comparisons on typed values, and the threshold on the confidence cannot be applied to the probability by mistake ([How judgment works](../concepts/how-judgment-works.md)).

Try it: change `"billing"` in the scripted answer to `"sales"`, an option the question does not offer, and run again. The call fails naming the question and the option, which is what a real model's off-list answer would do.

## 3. The same decision from a rubric

The questions and the thresholds can live in a file instead of in code, so a reviewer sees both without reading Rust. This is [`examples/jud_quickstart.rs`](../../examples/jud_quickstart.rs).

```rust
use judgment::jud::{Rubric, Supplied, Verdict};
use judgment::{Fake, SystemOne};

// The same questions as above, with the thresholds beside them, in a file
// another tool can read (docs/reference/jud-format.md). The policy is never
// sent to the model.
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

The rubric is checked as the builder checks questions, lowered to the same request as step 2, and its policy turns the answers into verdicts. Nothing in the code names a question or a threshold. [Rubrics, cases and recordings](../concepts/rubrics-cases-recordings.md) says why the policy is a separate half of the file; [Write a rubric](../guides/write-a-rubric.md) is the how.

## 4. Ask the real model

This step needs a key from [TypeSafe](https://docs.typesafe.ai) in `TYPESAFE_API_KEY` and spends one model call; [Configure a backend](../guides/configure-a-backend.md) says how to use a local open-weight model instead. The program reads a rubric from a file, [`examples/jud/triage.jud`](../../examples/jud/triage.jud) in the repository, and passes a `Client` where the `Fake` was. This is [`examples/jud_live.rs`](../../examples/jud_live.rs).

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
TYPESAFE_API_KEY=... cargo run --example jud_live --features jud   # in the repository
```

```text
actionable: {"verdict":"yes","probability":0.98}
desk: {"verdict":"option","key":"billing","confidence":0.7}
tone: {"verdict":"level","index":1,"label":"annoyed","value":1.02,"confidence":0.9}
answered by jev-1.13.0
```

The output is what `jev-1.13.0` answered on 2026-10-04; a later model answers with other numbers, which is why the thresholds in the file say which model they were tuned on. The model's answers are verified against the questions the file lowered to before the policy reads them; a transient failure is retried the way the official SDKs retry it, and the request id the server sent is on every response and every error.

## Next

- [Record, replay and test](../guides/record-replay-and-test.md): record that call once and replay it in your tests.
- [The crate](../reference/crate.md): the features, the modules and the rustdoc.
- [Patterns](../guides/patterns.md): the four shapes a call takes inside a real program.
