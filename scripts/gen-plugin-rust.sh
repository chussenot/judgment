#!/usr/bin/env sh
# Copy the Rust modules the jud plugin's `jud-rust` skill takes as its
# templates, with the rubrics they are generated from, into the plugin. The
# modules live under examples/jud/, where `cargo test` compiles and runs them
# (examples/jud_typed.rs), so the templates the skill copies are code that
# builds against this crate, not prose about it.
#
#   scripts/gen-plugin-rust.sh            write plugins/jud/skills/jud-rust/references/
#   scripts/gen-plugin-rust.sh --check    exit 1 when a committed copy is stale
#
# POSIX sh only.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
out=plugins/jud/skills/jud-rust/references

mode=write
if [ "${1:-}" = "--check" ]; then
  mode=check
fi

stale=0
mkdir -p "$out"
for name in triage.jud triage.rs routing.jud routing.rs; do
  src=examples/jud/$name
  dst=$out/$name
  if cmp -s "$src" "$dst"; then
    continue
  fi
  case "$mode" in
    write)
      cp "$src" "$dst"
      echo "wrote $dst"
      ;;
    check)
      diff -u "$dst" "$src" >&2 || true
      echo "$dst differs from $src: run scripts/gen-plugin-rust.sh and commit the result" >&2
      stale=1
      ;;
  esac
done
if [ "$mode" = check ] && [ "$stale" = 0 ]; then
  echo "$out matches examples/jud/"
fi
exit "$stale"
