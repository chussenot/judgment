---
description: Write the tests a Rust call site of the judgment crate needs (Fake for every branch, the request it sends, the error groups, Recorder and Replay for real answers, wiremock for the client), and run them
argument-hint: "<file or function that calls judgment> [record]"
---

Write and run the tests this code needs:

$ARGUMENTS

Follow the judgment-test skill: read
`${CLAUDE_PLUGIN_ROOT}/skills/judgment-test/SKILL.md` and
`${CLAUDE_PLUGIN_ROOT}/skills/judgment-test/references/record-replay-and-test.md`
first. One command per Bash call; files with the Write and Edit tools.
Never a test that calls a real API.

1. **Read the code** and list what it must hold, as the skill's step 1
   says: each branch it takes on an answer, each bar, each error group it
   handles, the state it builds. Say which are tested today (run `cargo
   test` once first when the crate has a lock file) and which are not.
2. **Write the missing tests** with `Fake`, one per branch, named after
   what they hold; the request through `Fake::calls()`; the error groups
   as the skill's step 3 says, an unfit answer built directly. A test that
   passes on an answer the question could not produce is a finding about
   the code: say so and test the refusal instead.
3. **With `record`**: an `#[ignore]` test that wraps the client in
   `Recorder::new(client, dir)` and a test that reads `Replay::open(dir)`;
   say the command the user runs with a key, and that the directory is
   committed. Never run the `#[ignore]` test here.
4. **Run** `cargo test` (the user's crate with a lock file; else a
   throwaway crate outside the project, as `/judgment:integrate` step 5
   builds it) and `cargo clippy --all-targets -- -D warnings`. Fix the
   tests until both pass; a failure that is the code's is reported, not
   hidden.
5. **Reply** as the skill's "Reply" says.
