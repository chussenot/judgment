---
name: jud-rust
description: Turn a .jud rubric into a small Rust module a program wires in with `mod <name>;` and one call. The module keeps the rubric as its source, gives each Choice's options and each Score's levels an enum and every question a typed field, and has a test that fails when the rubric's questions drift from it. Use this whenever someone wants to use, call, embed or run a .jud rubric from Rust, generate Rust code or types from a rubric, or replace string matching on verdicts with enums, even when they do not say "jud-rust".
---

# A .jud rubric as a Rust module

A program that asks a rubric's questions from Rust needs three things: the
request the rubric lowers for a state, a backend that answers it, and the
policy's verdicts in a form the compiler can check. The crate does the first
two (`Rubric::lower`, any `SystemOne`) and gives the verdicts as a map of
strings (`Rubric::apply`). This skill writes the thin module that turns that
map into enums and a struct, so a renamed option is a compile error rather
than a branch that silently never runs.

Two modules are the templates. Copy their shape exactly; change only what the
rubric changes:

- `references/triage.rs`, from `references/triage.jud`: a Noul, a Choice with
  static options and a Score.
- `references/routing.rs`, from `references/routing.jud`: Choices whose
  options come with each request (`options_from: request`), questions asked
  only `when` the state has a path, bands, a strict threshold and
  `level_at_least`.

Both compile against the crate and run in its test suite
(`examples/jud_typed.rs`), so they are known to build. They are copies of
`examples/jud/` in the judgment repository, kept in step by
`scripts/gen-plugin-rust.sh`.

## What the module is, and why

- **The rubric stays the source.** `SOURCE` is `include_str!` of the `.jud`
  file, parsed once, and the request is `Rubric::lower`. So the module sends
  exactly what `jud lower` and `jud record` send, and recordings made with the
  CLI replay against it.
- **Bars are data, not code.** `include_str!` compiles the file's text in
  and the module parses it at run time, so no Rust code holds a bar. A
  retune (`jud tune`, `/jud:tune`) takes a rebuild and no regeneration;
  only a change to the questions needs a new module. For the same reason no
  doc comment quotes a bar, a band or a fallback: it would go stale on the
  first retune.
- **Drift fails a test, not production.** `QUESTIONS_FINGERPRINT` and the
  enums are checked against the parsed rubric by the module's own
  `#[cfg(test)]` test. Edit a question and `cargo test` says to regenerate.
- **Few dependencies.** The module needs `judgment` with the `jud` feature,
  `serde` and `serde_json`, nothing else (no `thiserror`, no `indexmap`). Its
  error type is written out by hand.
- **Lint-clean in a strict crate.** It builds under `clippy::pedantic` with
  `unwrap_used` and `expect_used` denied, and `#![allow(dead_code)]` keeps a
  program that uses half of it quiet.

When the program must not read the `.jud` file at build time (the file lives
outside the package, or the program ships without it), embed the YAML
instead: `pub const SOURCE: &str = r"...";` with the file's text, or
`r#"..."#` only when the text contains a `"`. Three things then change from
the template:

- The header says the YAML is embedded, so a retune needs a regeneration
  (the bars are in the copy), and that only `cargo test` reads the file.
- When the `.jud` file is reachable from the module, a second test holds
  the copy to it, so a retune that forgets the module fails a test:

  ```rust
  /// Fails when the rubric file changed and this copy did not: regenerate.
  #[test]
  fn the_embedded_copy_is_the_file() {
      assert_eq!(SOURCE, include_str!("../rubrics/triage.jud"), "{RUBRIC} changed: regenerate this module");
  }
  ```
- The doc on `the_module_matches_the_rubric` says it holds the enums to
  the embedded copy; the copy test is the one that sees the file.

## How to run commands

- One command per Bash call, written out in full: no `&&`, `;`, `|`, `cd`
  or `$(...)`. A chained command is denied where a plain one is allowed.
- cargo's own flags (`--manifest-path`, `--target-dir`, `--all-targets`)
  go before `--`; only what clippy reads (`-D warnings`) goes after it.
- Create and change files only with the Write and Edit tools, never with
  `mkdir`, `cp`, `sed`, `cat >` or `echo >`. Write creates the directories.
- The user's `main.rs`, `lib.rs` and `Cargo.toml` are theirs: change them
  only when asked. The lines they need go in the reply.

## Workflow

1. **Check the rubric.**
   `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh check <rubric>` must say
   `0 refused`. Copy the `questions` fingerprint from its output; it goes in
   `QUESTIONS_FINGERPRINT`, exactly as printed.
2. **Read the rubric.** List every question in order: its id, its
   primitive, a Choice's static option keys with their descriptions, whether
   it has `options_from: request`, a Score's levels, its `when`, and its gate
   (threshold, `strict`, confidence, bands, fallback, `level_at_least`). The
   gate shapes nothing in the module: it is read from the rubric at run
   time and quoted nowhere.
3. **Decide the names** with the rules below, before writing any code.
4. **Write the module**, taking each part from the template that shows
   it. The templates are per feature, not per file: a static Choice (its
   `Copy` enum, `ALL`, fallible `from_key` and its `matches!` assertion)
   and `or_fallback` over `T: Copy` come from `triage.rs`; `Offered`,
   `Supplied(String)` variants, `or_fallback` over `T: Clone` and the
   `Option` field of a `when` question come from `routing.rs`. Start from
   `routing.rs` when some Choice has `options_from: request`, else from
   `triage.rs`. Keep, verbatim, everything that does not depend on the
   rubric:
   - `rubric()`;
   - `YesNo`, `Gated` and `or_fallback`;
   - `yes_no` and `gated`;
   - `Error` and its impls;
   - the shape of `questions`, `read` and `decide`.

   Write one enum per Choice and per Score, one field per question in
   `Decision` (in rubric order), and an `Offered` struct only when some
   Choice has `options_from: request`. Leave out what the rubric does not
   use: `Gated`, `or_fallback` and `gated` when it has no Choice or Score,
   `YesNo` and `yes_no` when it has no Noul.

   Name the rubric by its path from the crate root (from the project root
   with no crate), as `rubrics/triage.jud`, in the header's first two
   sentences and in `RUBRIC`, which the errors and the test messages use.
   The header says what a retune and a question change each take (as the
   templates do), and carries a complete usage example with the module's
   real names: the `mod` line, a `main` that builds the backend and the
   state, calls `decide`, and matches on every variant of one enum (with
   no Choice or Score, branches on a Noul's `yes`). When a Score's gate
   has `level_at_least`, the example reads `reached: Some(true)` rather
   than comparing with a level, so the bar stays in the rubric. Every name the example
   binds is used, so it compiles under `-D warnings` when pasted. With no
   crate, its `mod` line is the mounting line of step 5.

   On a regeneration, write the module afresh from the template with Write
   rather than patching the old one, then compare the two and report any
   hand edit the new one drops.
5. **Place it.** If a module is already at the path, regenerate it in
   place when its `include_str!` target (or, embedded, its `RUBRIC`)
   resolves to this rubric, and say in the reply if its header named a
   stale path. Stop and ask when it reads another rubric or has no
   "Generated by the jud plugin" header. Put the module where the program's modules live
   (`src/<module>.rs`), or beside the rubric. The `include_str!` path is
   relative to the `.rs` file: `include_str!("../rubrics/triage.jud")` from
   `src/triage.rs` for a rubric in `rubrics/`. If the rubric is outside the
   crate's package directory, say that `cargo package` will not include it,
   and offer the embedded form. With no crate, the module goes beside the
   rubric; say how a crate made later mounts it: `#[path =
   "../handoff.rs"] mod handoff;` from `src/main.rs` of a crate whose
   `Cargo.toml` sits beside the module, or move it to `src/` and change the
   `include_str!` path to the rubric's path from there.
6. **Wire it.** Read the program's `src/main.rs` or `src/lib.rs` first:
   whether it already has the `mod` line, and whether its `main` is
   `async` and returns a `Result`. The program needs, in `Cargo.toml`:

   ```toml
   judgment = { version = "0.11", features = ["jud"] }
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   tokio = { version = "1", features = ["macros", "rt"] }  # or the program's runtime
   ```

   The default `http` feature gives `judgment::Client`. A program that only
   replays or uses `Fake` can turn it off; when the program's `judgment`
   line turns default features off, say that the example's `Client` needs
   `http`. Add `mod <module>;` to `main.rs` or `lib.rs` only when asked;
   otherwise the line appears in the reply only inside the `main` of step
   8, never as a separate instruction, so it is not added twice.
7. **Verify** in a throwaway crate that mounts the module where you wrote
   it. A module that no `mod` line names is not compiled by `cargo check` in
   the user's crate, and the user's `main.rs` is not yours to edit. So:
   - Pick a directory of its own outside the project: the session's
     scratchpad or temporary directory if the system names one, else
     `/tmp/jud-rust-check-<module>/`.
   - With the Write tool (no `mkdir`, `cp`, `sed` or `cd`), write
     `<dir>/Cargo.toml`. It holds `[package] name = "jud-rust-check"`,
     `edition = "2021"` (the user's edition when there is a crate), and the
     dependencies of step 6. When the user's crate already names `judgment`,
     copy that line as it is and add `"jud"` to its features. Copy the
     user's `[lints]` tables as they are; with none, add `[lints.clippy]`
     with `all = { level = "warn", priority = -1 }`,
     `pedantic = { level = "warn", priority = -1 }`, `unwrap_used = "warn"`
     and `expect_used = "warn"`. End it with an empty `[workspace]` table,
     so a workspace above the directory does not claim it.
   - Write `<dir>/src/lib.rs` as one line,
     `#[path = "<absolute path of the module>"] mod <module>;`.
   - Write `<dir>/examples/usage.rs` as the header's usage example, its
     `mod` line replaced by that same `#[path]` line, so the `main` the
     reply hands over is compiled and linted too.
   - Run these two, one per Bash call, exactly in this shape. Add
     `--target-dir <the user's crate>/target` only when that directory
     already exists (it saves a rebuild); never create one in the project.
     - `cargo test --manifest-path <dir>/Cargo.toml`, which must run and
       pass `<module>::tests::the_module_matches_the_rubric`;
     - `cargo clippy --manifest-path <dir>/Cargo.toml --all-targets -- -D warnings`.
   - When the user's crate already has the `mod` line (in `main.rs`, or
     `pub mod` in `lib.rs`) and a `Cargo.lock`, also run
     `cargo test --manifest-path <the user's Cargo.toml> --target-dir <dir>/target`
     (its own `target` when `<the user's crate>/target` already exists).
     Without a `Cargo.lock` do not: cargo would write one into the project.
     Say instead that `cargo test` in their crate runs the drift test, and
     that its first run creates the lock file.

   Fix every error in the module. Never add an `allow` to get past a lint,
   other than the module's own `dead_code`. Leave the throwaway crate out of
   the project.
8. **Reply** with:
   - the file written and the `include_str!` path (or that the YAML is
     embedded, and that a retune then needs regeneration);
   - every name you had to change, and why;
   - the `Cargo.toml` lines still missing (or that none are); the `mod`
     line is not one of them, since the `main` below carries it;
   - the whole `main` from the module's header, with its `mod` line,
     runtime attribute, `async` and `Result` return, said to replace the
     `fn main` in `src/main.rs` and to keep the rest of the file (the `mod`
     line goes at the top, and is not given twice). When step 6 found the
     module already mounted from the crate's `src/lib.rs` (`pub mod
     <module>;`), a binary or an example reaches it through the library:
     give the `main` with `use <crate>::<module>;` (the package name with
     `-` turned to `_`) in place of the `mod` line, and say so;
   - when step 6 found `main` sync or returning `()`, a list of what
     changes in it, every item: the `#[tokio::main(flavor =
     "current_thread")]` attribute, `async fn`, the return type
     `Result<(), Box<dyn std::error::Error>>`, `Ok(())` at the end, and
     that the body's statements move into the new `main`;
   - what the caller owes:
     - with `Offered`, which options it must supply on every request, how
       many at least, and that no supplied key may be one of the
       question's own keys (name them), which `lower` refuses;
     - which fields are `Option`, and the state path that makes each one
       `Some`: present and not null, `""`, `[]` or `{}`;
     - the state fields the instructions refer to, and that nothing checks
       them: a state without them is sent as is;
     - for a conversation, that `decide` takes the turns so far, once per
       turn;
     - that every `decide` is one request to the backend, as slow as the
       model (seconds on a CPU): a caller that feeds it a stream or a batch
       bounds how many are in flight and what waits, and samples rather
       than queue without end when states arrive faster than answers;
   - the checks that passed, the throwaway crate's absolute path, and that
     once the `mod` line is in, `cargo test` in their crate runs the
     module's drift test (with no crate: once a crate mounts it as step 5
     says);
   - embedded, that a retune is followed by `/jud:rust <rubric> embed`
     again, which regenerates the module in place.

   On a regeneration, also say:
   - which questions changed;
   - what now fails to compile: a `match` that lists every variant with no
     wildcard, on an enum that gained or lost one;
   - what changes with no compile error: a `_` arm, a `matches!` or an
     `if let` that now takes a new variant; answers that move to a new
     option or level because the model now sees it, or away from a removed
     one; an edited instruction or description;
   - that recordings made before the change no longer replay, and the bars
     want re-tuning (`/jud:record`, then `/jud:tune`). When an option or
     level was added, the labelled cases need examples of it first
     (`/jud:cases`), and until then the rubric's `tuning` block and its
     gate notes describe the old questions.

## Names

Rust identifiers are stricter than `.jud` keys. Apply these rules in order,
and list in the reply every name that differs from the plain
`snake_case` or `PascalCase` conversion (a keyword, a reserved word, a
leading digit, a clash, a collision, a level with no letters), each with its
reason; plain case conversion needs no row.

- **Module name:** the rubric file's stem in `snake_case` (`triage.jud` gives
  `triage`, `support-triage.jud` gives `support_triage`), unless the user
  names one.
- **Field names** (in `Decision` and `Offered`): the question id in
  `snake_case`. `-` and `.` become `_` (`support-level.v2` gives
  `support_level_v2`). An id that is a strict or reserved keyword of the
  crate's edition (`type`, `match`, `move`, `ref`, `use`, `fn`, `impl`,
  `loop`, `for`, `if`, `in`, `let`, `mod`, `struct`, `true`, `async`,
  `await`, `dyn`, `gen` in edition 2024, and the rest) becomes a raw
  identifier (`r#type`). `self`, `super`, `crate` and `Self` cannot be raw:
  they get a `q_` prefix (`q_self`), as does an id starting with a digit.
- **Enum names:** the question id in `PascalCase`, by the variant rule
  below (`duplicate_of` gives `DuplicateOf`, `support-level.v2` gives
  `SupportLevelV2`, `2fa` gives `Q2fa`, to match its field `q_2fa`). If that clashes with a name the module defines (`Decision`,
  `Error`, `Gated`, `YesNo`, `Offered`, `Rubric`), append the primitive:
  `ErrorChoice`, `DecisionLevel`.
- **Variant names:** the option key, or the level's text, in `PascalCase`.
  Split on anything that is not an ASCII letter or digit, capitalise the
  first letter of each part, keep the rest as written, and join. So
  `none_of_these`, `none-of-these` and `none of these` all give
  `NoneOfThese`, `very high!` gives `VeryHigh`, and `a-b` gives `AB`.
  - A leading digit gets an `N` prefix (`3d` gives `N3d`).
  - `Self` is reserved, and `None`, `Some`, `Ok` and `Err` would read as
    the prelude's, so these take an `Option` suffix: `self` gives
    `SelfOption`, `none` gives `NoneOption`.
  - A Score level whose text gives nothing usable (`!!!`) becomes `Level`
    and its index (`Level2` for the third level).
  - If two keys give the same variant, number the later ones in rubric
    order (`a-b` gives `AB`, then `a_b` gives `AB2`). Keep counting until
    the name is unused: `a-b`, `a_b`, `a-b-2` give `AB`, `AB2`, `AB3`.
- The wire key or level text stays exactly as written, in `key()` or
  `label()`. Never "clean" it there: the response names it byte for byte.
- A doc comment on each variant starts with the wire key or the level
  (``/// `billing`: Invoices, charges, refunds.``, ``/// `calm` (level 0)``),
  then the option's description. The doc comment on each field gives the
  question's instructions, shortened to the first sentence when long.

## Mapping

| In the rubric | In the module |
|---|---|
| a Noul | `YesNo { yes, probability }` field. `yes` is the gate's decision at its threshold (and `strict`), not `probability >= 0.5`. |
| a Choice, static options | `#[derive(Copy, Eq, Hash)] enum` with a variant per key in rubric order, `ALL`, `key()`, and a `from_key` that returns `Error::Unexpected` for a key it does not know; field `Gated<Enum>`. |
| a Choice, `options_from: request` | an enum with a variant per static key and `Supplied(String)` (not `Copy`). `from_key` returns `Self`: any key the response names was offered (`Response::verify`). There is a `Vec<(String, String)>` field for it in `Offered`, an entry for it in the `(id, options)` array `Offered::supplied` loops over, and an `offered` parameter on `questions` and `decide`. |
| a Score | an enum with a variant per level, lowest first, deriving `PartialOrd, Ord` so `>=` compares levels, plus `ALL`, `label()` and `from_level(&str)` (an index or a level's text, as a fallback may name it), tested by `levels_read_by_index_and_by_text`; field `Gated<Enum>`. |
| `when: <path>` | the field is `Option<...>`, read with `verdicts.get(id).map(...).transpose()?`. Its doc comment says it is `None` when the path is absent, null or empty. |
| `part_when` | nothing in the types: the part is sent or not by `lower`. |
| a gate's bars, bands, fallback, `level_at_least` | nothing, not even a doc comment. They are read from the rubric at run time, and `Gated::Act { band, reached, .. }` and `Gated::Defer { fallback, .. }` carry what they decided. |
| a question with no gate | the default gate: a Noul at 0.5, a Choice or Score that never defers. Same field types. |
| a conversation (the state is an array of turns) | nothing different. The program calls `decide` with the turns so far, once per turn. Say so in the module's header. |

The test checks, for the rubric's questions:

- the fingerprint;
- each static Choice's keys in order;
- each Score's levels in order;
- for a Choice with `options_from: request`, its static keys.

Write one assertion per enum, as the templates do.

## Never

- Copy a threshold, a confidence bar or a fallback into Rust as a constant
  that decides anything. The bars belong to the rubric; the module reads
  them.
- Build the request by hand with `Questions::new()...` instead of
  `rubric.lower`. The bytes would differ from `jud lower`, and the
  recordings would stop replaying.
- Build `Supplied` through a `serde_json` object. Its keys sort, and the
  option order is what the model sees. Collect the pairs into the map
  directly, as `Offered::supplied` does.
- Match on `Verdict` strings in the program. That is what the module
  replaces.
- Hand over a module you did not compile and whose test you did not run.
