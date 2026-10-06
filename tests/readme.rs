//! The README opens with `examples/quickstart.rs`, which CI compiles and runs.
//! This test keeps the README's copy identical to the example, so the code a
//! reader sees first is code that builds and runs.

#![allow(clippy::expect_used)]

/// The README's part of an example, as the rust block the README must hold.
fn readme_block(example: &str, name: &str) -> String {
    let begin = example.find("// README:BEGIN\n").unwrap_or_else(|| {
        panic!("examples/{name}.rs marks the README's part with // README:BEGIN")
    });
    let end = example.find("// README:END").unwrap_or_else(|| {
        panic!("examples/{name}.rs marks the end of the README's part with // README:END")
    });
    let code = example[begin + "// README:BEGIN\n".len()..end].trim_end();
    format!("```rust\n{code}\n```")
}

/// Where the README holds `example`'s block, verbatim.
fn position_of(readme: &str, example: &str, name: &str) -> usize {
    let block = readme_block(example, name);
    readme.find(&block).unwrap_or_else(|| {
        panic!("README.md must contain examples/{name}.rs between its README markers, verbatim, as a rust block")
    })
}

/// The README opens with the quickstart, then the same decision from a `.jud`
/// rubric, then a rubric from a file asked of the hosted API; each is held to
/// its example file, in that order.
#[test]
fn the_readme_holds_the_three_examples_in_order() {
    let readme = include_str!("../README.md");
    let quickstart = position_of(
        readme,
        include_str!("../examples/quickstart.rs"),
        "quickstart",
    );
    let jud = position_of(
        readme,
        include_str!("../examples/jud_quickstart.rs"),
        "jud_quickstart",
    );
    let live = position_of(readme, include_str!("../examples/jud_live.rs"), "jud_live");
    let first_block = readme.find("```rust").expect("README.md has a rust block");
    assert_eq!(
        quickstart, first_block,
        "the quickstart is the README's first code block"
    );
    assert!(
        quickstart < jud && jud < live,
        "the examples follow in order"
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
