//! The README opens with `examples/quickstart.rs`, which CI compiles and runs.
//! This test keeps the README's copy identical to the example, so the code a
//! reader sees first is code that builds and runs.

#![allow(clippy::expect_used)]

#[test]
fn the_readme_opens_with_the_quickstart_example() {
    let example = include_str!("../examples/quickstart.rs");
    let readme = include_str!("../README.md");
    let begin = example
        .find("// README:BEGIN\n")
        .expect("examples/quickstart.rs marks the README's part with // README:BEGIN");
    let end = example
        .find("// README:END")
        .expect("examples/quickstart.rs marks the end of the README's part with // README:END");
    let code = example[begin + "// README:BEGIN\n".len()..end].trim_end();
    let block = format!("```rust\n{code}\n```");
    assert!(
        readme.contains(&block),
        "README.md must contain examples/quickstart.rs between its README markers, verbatim, as a rust block"
    );
    let first_block = readme.find("```rust").expect("README.md has a rust block");
    assert_eq!(
        readme.find(&block),
        Some(first_block),
        "the quickstart is the README's first code block"
    );
}

/// The README's second example is `examples/jud_quickstart.rs`, held to the
/// file the same way; it follows the quickstart.
#[test]
fn the_readme_continues_with_the_jud_quickstart_example() {
    let example = include_str!("../examples/jud_quickstart.rs");
    let readme = include_str!("../README.md");
    let begin = example
        .find("// README:BEGIN\n")
        .expect("examples/jud_quickstart.rs marks the README's part with // README:BEGIN");
    let end = example
        .find("// README:END")
        .expect("examples/jud_quickstart.rs marks the end of the README's part with // README:END");
    let code = example[begin + "// README:BEGIN\n".len()..end].trim_end();
    let block = format!("```rust\n{code}\n```");
    let position = readme.find(&block).expect(
        "README.md must contain examples/jud_quickstart.rs between its README markers, verbatim, as a rust block",
    );
    let first_block = readme.find("```rust").expect("README.md has a rust block");
    assert!(
        position > first_block,
        "the .jud example follows the quickstart"
    );
}

/// The install snippet names the crate's major.minor, which
/// `scripts/release-bump.sh` rewrites on a release; this catches a hand bump
/// or a snippet edit that left the two apart.
#[test]
fn the_install_snippet_names_the_current_minor_version() {
    let readme = include_str!("../README.md");
    let expected = format!(
        "judgment = \"{}.{}\"",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    assert!(
        readme.lines().any(|line| line == expected),
        "README.md's install snippet must read `{expected}`, the version Cargo.toml declares"
    );
}
