---
title: Use long context
description: How to send a state over Jev's default context limits (64k tokens per request, 32k per question) by asking for long context, 262k for both, from the shell with jud's --long-context flag or its long_context setting, on one call with CallOptions::long_context or on every call of a client with ClientBuilder::long_context; what it costs in speed and accuracy, how to tell a refusal that it would fix, and why the crate never turns it on by itself.
status: current
last_reviewed: 2026-10-15
tags: [judgment, long-context, client, how-to, rust]
---

# Use long context

**You will** send a state that the default limits refuse, and know when not to. **Prerequisites:** [Your first decision in Rust](../start/first-decision-rust.md) for the Rust sections, [Your first decision from the shell](../start/first-decision-cli.md) for the `jud` one.

## The limits, and the switch

Jev reads the state once and answers every question against it, inside two limits: 64k tokens for the state and all the questions together, 32k for the state and any one question ([Models](https://docs.typesafe.ai/models#context-length)). Over either, the hosted API answers 400 `max_tokens_exceeded`, which the crate reports as `Error::InvalidRequest` with `is_request_too_large()` true: the one refusal fixed by sending less, or by this page.

Long context raises both limits to 262k tokens, on Jev 1.13 and 1.14, behind a header the client sends when asked and never otherwise ([decision 0027](../project/decisions/0027-long-context-is-a-switch-on-the-call.md)). TypeSafe says a request under it is slower and less accurate than one within the limits, which is why it is a switch and not a default, and why the crate does not guess from the state's size: the caller who built a long state knows it did.

## From the shell

`jud RUBRIC`, `jud record` and `jud eval` take `--long-context`, which turns it on for every call of that run; `long_context: true` in `~/.config/jud/config.yaml` turns it on for every run, and the flag wins over the file ([Configuration](../reference/configuration.md#the-jud-command)). A replay ignores both, since it asks nothing.

```sh
jud record transcripts.jud transcripts-cases.jud --out recordings/transcripts --long-context
```

`jud record --dry-run` prints `long context: on (from command line)`, or `(from config file)`, where a run's target is checked, and the line before a run's first call says the same, so a run that would pay the cost on every request can be stopped first; off, the dry run says `long context: off` and the announcement says nothing. The recordings are keyed as without it: the header changes the limits, not the request, so a recording made under long context answers a replay that never mentions it. `jud tune` reads recordings only and takes no flag.

## For one call

```rust no_run
use judgment::{CallOptions, Client, Questions, Request};

async fn run(transcript: &serde_json::Value) -> judgment::Result<()> {
    let mut questions = Questions::new();
    let decided = questions.noul(
        "decided",
        "Does `transcript` end with a decision the participants agreed on?",
        None,
    )?;
    let client = Client::from_env()?;
    let request = Request::new(transcript, "jev-latest", &questions);
    let response = client
        .evaluate_with(&request, &CallOptions::new().long_context(true))
        .await?;
    println!("decided: {:.2}", response.get(&decided)?.yes);
    Ok(())
}
```

The body is the same bytes as without the option; only the header is added, and the `typesafe.evaluate` span records `long_context = true`. A recording of the call is keyed as any other, since the header changes the limits and not the request.

## For every call of a client

A client that only ever reads long transcripts or contracts turns it on once:

```rust no_run
use judgment::{CallOptions, Client, Questions, Request};

async fn run(contract: &serde_json::Value) -> judgment::Result<()> {
    let client = Client::builder().long_context(true).build()?;
    let mut questions = Questions::new();
    let termination = questions.noul(
        "termination",
        "Does `contract` let either party end the agreement early?",
        None,
    )?;
    let response = client.system_one(contract, &questions).await?;
    println!("early termination: {:.2}", response.get(&termination)?.yes);
    // A short request from the same client, within the limits, without it.
    let short = Request::new(&"one line", "jev-latest", &questions);
    let _ = client
        .evaluate_with(&short, &CallOptions::new().long_context(false))
        .await?;
    Ok(())
}
```

A `Recorder` over such a client sends the header on every call too.

## Only when refused

When the states vary and nothing says which are long, ask within the limits first and resend under the header only on the refusal; a refused body is not billed, so the cost of the first try is a round trip:

```rust no_run
use judgment::{CallOptions, Client, Questions, Request, Response};

async fn ask(client: &Client, state: &serde_json::Value, questions: &Questions) -> judgment::Result<Response> {
    let request = Request::new(state, "jev-latest", questions);
    match client.evaluate(&request).await {
        Err(e) if e.is_request_too_large() => {
            client
                .evaluate_with(&request, &CallOptions::new().long_context(true))
                .await
        }
        other => other,
    }
}
```

`is_request_too_large` is also true of a 413 and of an image over its limits, which the header does not fix; a request that is refused again under it is one to send less of.

## Next

- [Configure a backend](configure-a-backend.md): the client's other settings.
- [Jev 1.14 on the hosted API](../project/verification/hosted-typesafe-jev-1-14.md): the 74,550-token state the header let through.
