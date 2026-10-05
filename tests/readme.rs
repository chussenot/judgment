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
