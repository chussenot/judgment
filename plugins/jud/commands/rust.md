---
description: Turn a .jud rubric into a small typed Rust module (enums for options and levels, one decide call), compiled and tested before it is handed over
argument-hint: "<rubric path> [output .rs path] [embed]"
---

Write the Rust module for this rubric:

$ARGUMENTS

Follow the jud-rust skill: read `${CLAUDE_PLUGIN_ROOT}/skills/jud-rust/SKILL.md`
and the templates it names (`references/triage.rs`, and `references/routing.rs`
for `options_from: request` or `when`; step 4 says which parts come from
which) before writing anything.

Run every `jud` command through `${CLAUDE_PLUGIN_ROOT}/skills/jud/scripts/jud.sh`,
written out in full, one command per Bash call, with nothing chained to it.

1. `jud.sh check <rubric>` must say `0 refused`; copy the questions
   fingerprint from it. A refusal stops here: report it and offer
   `/jud:check`.
2. Choose the output path. If one is given, use it. Otherwise, in a Rust
   crate (a `Cargo.toml` above the rubric or in the working directory), use
   `src/<module>.rs` of that crate; else write `<module>.rs` beside the
   rubric. The `include_str!` path is relative to the `.rs` file. With
   `embed` in the arguments, embed the YAML instead of `include_str!`, as
   the skill's embedded form says (a retune then needs a regeneration, and
   a test holds the copy to the file).
3. Name, map and write the module exactly as the skill says. An existing
   module at the path is regenerated or left alone by the skill's step 5
   rule (its `include_str!` target decides); a regeneration writes the
   module afresh, not by patching.
4. Verify as the skill says: in a throwaway crate outside the project,
   written with the Write tool, that mounts the module with `#[path = ...]`.
   Run, one per Bash call, `cargo test --manifest-path <dir>/Cargo.toml`
   and `cargo clippy --manifest-path <dir>/Cargo.toml --all-targets -- -D warnings`
   (cargo's flags before `--`), and fix the module until both pass. Change
   no other file in the project unless asked: the missing `Cargo.toml`
   lines go in the reply, and the `mod` line only inside the replacement
   `main` the skill's step 8 shows.
5. Reply as the skill's step 8 says, every bullet: among them what the
   caller owes, the lines of a sync `main` that change, and, when an
   existing module was replaced, what the regeneration changes.
