---
name: judgment-test
description: Test Rust code that calls the judgment crate with no key and no network - the Fake backend that refuses an answer the question could not produce, Recorder and Replay keyed by the request's content, wiremock for code that drives the Client itself (retries, a 429 with a wait, a 2xx that does not decode), and the rule that no test calls a real API. Use whenever a task is about testing, mocking, recording or replaying judgment calls, a flaky or slow test that hits the TypeSafe API, or a decision function with untested branches, even when the user does not say "judgment".
---

# Testing code that calls judgment

A decision built on a model's answer has three things to test: that the
code reads every answer the questions allow (each option, the deferred
path, the error groups it handles), that the request it sends is the one
it means to send, and that what it does with a real model's answers still
holds after a change. The crate gives a backend for each, and none of them
needs a key or a network. `references/record-replay-and-test.md`
(generated from the crate's guide) is the long form; this skill is the
workflow.

## Which backend for which test

| Test | Backend | Why |
|---|---|---|
| Every branch of the code that reads an answer | `Fake` | Scripted answers, checked against the questions exactly as a server's are: a `Fake` cannot answer an option the question does not offer, so a test cannot pass on an answer production would refuse. |
| The request the code sends (questions, state, options, images) | `Fake::calls()` | Every call the fake received, to assert on. |
| What a real model answered, replayed in CI | `Recorder` once with a key, then `Replay` | A recording is keyed by the request's fingerprint (the state, the questions, each image's digest): a request nobody recorded fails, never guesses, so a changed prompt cannot pass on an old answer. |
| Code that drives the `Client` itself (its retry handling, a fallback on `RateLimited`, a timeout) | the real `Client` against wiremock | The status, the headers and the body a server sends; `Client::builder().base_url(server.uri())`. |

Never a real API in a test: a test that needs a key fails for everyone
without one, costs money on every run, and changes its answer when the
model moves. A live check is an `#[ignore]` test run by hand.

## Workflow

1. **Read the code under test** and list what it must hold: each variant
   of each Choice it matches on, the path below each confidence bar, each
   `Error` group it handles (and that it does not turn an unfit answer
   into a default), and what it puts in the state.
2. **Script the answers with `Fake`**, one test per branch, the answers
   named after the case they test:

   ```rust
   use judgment::{Fake, Questions, SystemOne};

   #[tokio::test]
   async fn a_low_confidence_billing_answer_goes_to_a_person() -> judgment::Result<()> {
       let backend = Fake::new()
           .choice("desk", [("billing", 0.55), ("technical", 0.45)], 0.55)?
           .noul("urgent", 0.2)?;
       let route = decide(&backend, &json!({"message": "invoice?"})).await?;
       assert_eq!(route, Route::Human);
       Ok(())
   }
   ```

   - `Fake::new().noul(id, p)`, `.choice(id, [(key, p), ...], confidence)`,
     `.score(...)`, `.r#box(...)`, `.with_answer(id, Answer)` for a raw
     one; `.model(...)` and `.usage(...)` for what the response reports.
     Each builder returns `Result`: a probability outside 0 to 1 is
     refused there.
   - The fake answers only the ids it was given: a question the code adds
     later fails its test with `MissingAnswer` until the test answers it.
   - `backend.calls()` after the call gives the state, model and question
     ids sent; assert on the state the code built, not only on the route.
3. **The error paths.** A test per group the code handles: build the
   failure with a backend that returns it (wiremock for the client's own
   statuses; a small `SystemOne` impl returning `Err(...)` for the rest),
   and assert the fallback. For an answer that does not fit (an option
   the question did not offer), build the malformed `Response` directly,
   as the crate's own docs advise, and assert that `response.get` or
   `response.verify` errs with `is_unfit()` and that the code does not
   turn it into an option.
4. **Record once, replay in CI** when the decision's quality matters, not
   only its wiring: wrap the client in `Recorder::new(client, dir)` in an
   `#[ignore]` test run by hand with a key, commit the directory, and
   read it with `Replay::open(dir)?` in the test that runs everywhere. A
   recording is what a model said: never edit one to make a test pass;
   record the changed request again. A `.jud` rubric's recordings are the
   `jud` plugin's (`/jud:record`).
5. **wiremock for the client**: `MockServer::start().await`, a `Mock` on
   `POST /v1/systemone` with the status and body under test (a 429 with
   `retry-after-ms`, a 503 then a 200, a 200 whose body is not a
   response), `Client::builder().api_key("test").base_url(server.uri())
   .build()?`, and assert on the result and on `server.received_requests()`
   (how many attempts the policy made). Use `RetryPolicy` with a short
   backoff in tests, or `tokio::time::pause()`.
6. **Run them**: `cargo test` in the user's crate when it has a lock file;
   otherwise in a throwaway crate outside the project that mounts the
   module with `#[path = "..."]`. Report the tests that ran and passed by
   name. A test you did not run is not handed over.

## Never

- A test that calls a real API, or reads `TYPESAFE_API_KEY` outside an
  `#[ignore]` test.
- A hand-built `Response` used to reach a branch as if it were a good
  answer: production verifies every response, so that branch is never
  reached. Build one only to test the refusal of a malformed answer.
- An edited, deleted or hand-written recording to make a replay pass.
- A sleep to wait out a retry: pause tokio's clock or shorten the policy.

## Reply

The tests written, by name, with what each holds; which branches and error
groups are covered and which are not; the recordings the user must make
with a key and commit (the `#[ignore]` test to run); and the `cargo test`
result, as run.
