---
name: judgment
description: Ask typed questions of a System One model (Jev, Laya, Ollama, OpenRouter, Liquid) from Rust with the judgment crate and act on the answers with its guarantees intact - Questions and typed Handles, options!, Noul/Choice/Score/Box answers, Response::verify, the Error groups by what fixes them, Client and RetryPolicy with the official SDKs' retries, CallOptions, and choosing a backend. Use whenever Rust code calls, adds, reviews or debugs the judgment crate (use judgment::, Client::from_env, Questions::new, response.get), or replaces an LLM prompt-and-parse step or a keyword heuristic in Rust with a typed question, even when the user does not say "judgment".
---

# The judgment crate, from Rust

`judgment` asks a System One model narrow, typed questions about a JSON
state and returns calibrated answers the compiler checks: a Noul (a
probability of yes), a Choice (one of the options you offered, with a
probability per option), a Score (a level on your scale) or a Box (where
something is in an image). It holds every response against the questions
it was sent, so a decision never rests on an answer the question could not
produce. This skill is how to use it well; the pages under `references/`
(generated from the crate's documentation) are the detail, and the
rustdoc on docs.rs is the reference for every type.

What to ask and how to shape the state for any System One client is the
`typesafe-ai` skill's, when it is installed; this skill is this crate's
types, guarantees and failure modes. A decision that should be reviewable
and tunable outside the code belongs in a `.jud` rubric (the `jud`
plugin's `/jud:rubric` and `/jud:rust`): say so when the decision has
thresholds someone will want to move without a release.

## Workflow

1. **Read the code first**: the crate's line in `Cargo.toml` (the minor
   matters: 0.x may break between minors; features `http` (default),
   `jud`, `otel`, `pacer`, `multipart`, `zstd`), the call site, and what
   the program already knows. Read `references/crate.md` for the feature
   and module map before naming a type.
2. **Split the decision.** What the program already knows or can compute
   (a plan tier, an age, a status code, a field a tool wrote) stays in
   code and goes into the state. The model answers only what needs
   judgment, one atomic question each: "which team, and is it urgent?" is
   a Choice and a Noul, never one question.
3. **Write the questions** with `Questions::new()`, one handle per
   question, in the order the model should read them:

   ```rust
   use judgment::{Questions, options};

   options! {
       enum Desk {
           Billing = "billing" => "Invoices, charges, refunds",
           Technical = "technical" => "Bugs, outages, integrations",
           NoneOfThese = "none_of_these" => "Anything else, or not a request",
       }
   }

   let mut questions = Questions::new();
   let desk = questions.choice::<Desk>("desk", "Which desk should take `message`?")?;
   let urgent = questions.noul("urgent", "Does `message` convey urgency?", None)?;
   ```

   - A Choice gets a way out (`none_of_these`) when the state might fit
     none of its options: without one the model must pick a wrong one.
   - Name the state's keys in the instructions (`message`), and keep every
     threshold out of them: a bar in the instructions ("answer yes only if
     sure") cannot be tuned.
   - Options the request supplies (`dynamic_choice`) keep the order you
     give: it is the order the model reads.
   - The builder refuses a Choice outside 2 to 255 options, a Score
     outside 2 to 10 levels, an empty or duplicate id, before anything is
     sent.
4. **Pick the backend.** `judgment::Client::from_env()?` reads
   `TYPESAFE_API_KEY` and `TYPESAFE_BASE_URL`; `Client::builder()` sets
   the key, base URL, model, timeout, `RetryPolicy`, observer and pacer.
   `references/configure-a-backend.md` says how to point it at the hosted
   API, OpenRouter, Liquid, Ollama, laya-serve or Clef, and what differs
   between them (whose `confidence` means what, question limits, which
   models take a Box). Pin a model version (`jev-1.14.0`) when bars were
   tuned on it; `jev-latest` moves. Tests use `Fake`, `Recorder` and
   `Replay`, never a real API (the `judgment-test` skill).
5. **Call and read through the handles.** `backend.answer(&state, model,
   &questions).await?` returns a verified `Response`; `response.get(&handle)?`
   gives the typed answer:

   ```rust
   use judgment::SystemOne; // `answer` is the trait's: Client, Fake, Replay and Paced all implement it

   let response = client.answer(&state, "jev-1.14.0", &questions).await?;
   let desk = response.get(&desk)?;          // Choice<Desk>
   let urgent = response.get(&urgent)?;      // Noul
   let route = match desk.chosen {
       _ if !desk.confidence.at_least(0.7) => Route::Human,
       Desk::Billing => Route::Billing,
       Desk::Technical => Route::Technical,
       Desk::NoneOfThese => Route::Human,
   };
   let page = urgent.is_yes(0.6);
   ```

   - A Noul is `yes: Probability`; `is_yes(t)` compares it. A Choice is
     `chosen`, `probabilities` per option and `confidence`. A Score is
     `value` (the weighted score), `levels`, `probabilities` and
     `confidence`; `nearest_level()` and `nearest_label()` give the level
     the value is nearest, which is the one to act on. A `BoundingBox` is
     `edges` in fractions of the image and an optional `confidence`;
     `edges.to_pixels(w, h)` makes pixels. A Box always answers with a
     box, found or not: ask a Noul beside it whether there is anything to
     find, and read that first.
   - Every bar is a named value in the caller (`confidence.at_least(0.7)`,
     `is_yes(0.6)`), with a comment saying where it came from. A guessed
     bar says so.
   - A `match` on the enum lists every variant: an option added to the
     enum then fails to compile where it is not handled.
6. **Handle errors by what fixes them** (`references/crate.md`, Errors):
   configuration (`MissingApiKey`, `Url`) and refused requests
   (`InvalidRequest`, `InvalidQuestion`, `InvalidImage`) are bugs to fix,
   not to retry; `PaymentRequired` needs credit; `RateLimited`,
   `Overloaded` and `Http` already went through the retry policy and are
   the caller's fallback (a default route, a queue, a person);
   `Transport` is the network; an answer that does not fit the questions
   (`err.is_unfit()`: `MissingAnswer`, `AnswerTypeMismatch`,
   `UnknownOption`, `InvalidAnswer`) was billed and is never retried, and
   is never turned into a default option. Log `err.request_id()` with
   every server error. The crate's `Error` is `#[non_exhaustive]`: a
   `match` ends with a `_` arm that takes the safe fallback.
7. **Verify** what you wrote: `cargo check`, `cargo clippy --all-targets
   -- -D warnings` and a test with `Fake` (the `judgment-test` skill)
   before handing it over. A module you did not compile is not done.

## Rules the crate enforces, and why

- **An off-list option is an error, never a default.** `Response::verify`
  holds a response against the questions sent: an answer for every
  question, of its primitive, a Choice naming only offered options, a
  Score whose legend matches the levels. Code that catches
  `UnknownOption` and maps it to a default option hides a server that
  answers questions it was not asked.
- **Retries are the SDKs'.** `RetryPolicy::default()`: 2 retries, 500 ms
  doubled to 5 s with jitter taken off, 408, 429 and 5xx retried, the
  server's `retry-after` honoured up to 30 s. A loop around the call
  retries twice as often as the server asked; raise the policy instead.
  `RetryPolicy::conservative()` for a call that must not be billed twice.
- **No metrics of its own.** The crate writes `tracing` spans and reports
  tokens, cost, failed attempts and each call through an `Observer`
  (the `judgment-observe` skill); counting tokens by hand duplicates it.
- **Images** are `Image::read(path)?` (a PNG or JPEG, its size read and the
  API's limits checked before any call) in an `Assets` under the
  reference the state carries, sent with `answer_with_assets`. They cost
  input tokens by their pixels: send the smallest that keeps the target
  legible.
- **Long context** is a switch (`ClientBuilder::long_context(true)` or
  per call): 262k tokens, slower and less accurate by TypeSafe's account.
  Turn it on when long states are the subject, never because one is.
- **A stream of calls** is bounded with a `Pacer` (the
  `judgment-observe` skill): spawning a task per event outruns the server
  and every 429 is then a failure.

## Reply

The code written or changed, file by file; the features the `Cargo.toml`
line needs; every bar with where it came from (a guess says so); what the
caller must put in the state; which errors the code handles and how; the
checks run and their results; and, when the decision has bars someone will
tune, the offer to move it to a `.jud` rubric (`/jud:rubric`). Never claim
a call works against a real API from a `Fake` run.
