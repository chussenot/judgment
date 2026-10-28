//! Rules about the repository itself, held by CI: a bounded proof that
//! `mise run kani` does not run is a proof nobody checks.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::collections::BTreeSet;
use std::path::Path;

/// Every `.rs` file under `dir`, read.
fn sources(dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(std::fs::read_to_string(path).unwrap());
        }
    }
}

// ponytail: line-based string parsing, not a Rust or shell parser; it takes the first `fn` after
// each `#[kani::proof]` and each `--harness` word in the task. Move to `syn` if a proof is ever
// spelled another way (a macro, a `cfg_attr`).
#[test]
fn every_kani_proof_is_run_by_the_kani_task() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut proofs = BTreeSet::new();
    for text in &files {
        let mut lines = text.lines();
        while let Some(line) = lines.next() {
            if line.trim() == "#[kani::proof]" {
                let name = lines
                    .by_ref()
                    .find_map(|l| l.trim().strip_prefix("fn "))
                    .expect("a proof attribute with no function after it");
                proofs.insert(name.split('(').next().unwrap().to_owned());
            }
        }
    }

    let mise =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("mise.toml")).unwrap();
    let task = mise
        .split("[tasks.kani]")
        .nth(1)
        .expect("mise.toml has no kani task")
        .split("\n[")
        .next()
        .unwrap();
    let run: BTreeSet<String> = task
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|w| w[0] == "--harness")
        .map(|w| w[1].to_owned())
        .collect();

    assert!(!proofs.is_empty(), "no #[kani::proof] found under src/");
    assert_eq!(
        proofs, run,
        "the #[kani::proof] functions under src/ (left) and the `--harness` names of the \
         kani task in mise.toml (right) differ"
    );
}
