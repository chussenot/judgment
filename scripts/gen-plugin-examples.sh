#!/usr/bin/env sh
# Copy the example documents the jud plugin's skills start from into the
# plugin, so a user who installed it from the marketplace has them without a
# checkout. The sources stay under examples/jud/, where `cargo test` reads
# them; the copies are byte for byte the sources, images included, since a
# document names its images by a path relative to itself.
#
#   scripts/gen-plugin-examples.sh            write plugins/jud/skills/jud/references/examples/
#   scripts/gen-plugin-examples.sh --check    exit 1 when a committed copy is stale or missing
#
# POSIX sh only.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
src=examples/jud
out=plugins/jud/skills/jud/references/examples

# The documents the `jud` skill names in its workflow and the bias page
# needs, each with its cases; the shapes pair brings its forty images (160 KiB).
files="
triage.jud
triage-cases.jud
handoff.jud
handoff-cases.jud
routing.jud
routing-cases.jud
shapes.jud
shapes-cases.jud
pairwise-judge.jud
pairwise-judge-cases.jud
pairwise-judge-tone.yaml
pairwise-judge-injection.yaml
"
images=$(cd "$src" && ls images/shape-*.png)

mode=write
if [ "${1:-}" = "--check" ]; then
  mode=check
fi

stale=0
copy() {
  # $1: the path under $src and $out
  if [ "$mode" = check ]; then
    if ! cmp -s "$src/$1" "$out/$1"; then
      echo "$out/$1 is stale or missing: run scripts/gen-plugin-examples.sh" >&2
      stale=1
    fi
  else
    mkdir -p "$(dirname "$out/$1")"
    cp -f "$src/$1" "$out/$1"
  fi
}

for f in $files $images; do
  copy "$f"
done

if [ "$mode" = check ]; then
  # A copy with no source is stale too.
  for f in $(cd "$out" && find . -type f | sed 's|^\./||' | sort); do
    [ -f "$src/$f" ] || { echo "$out/$f has no source under $src" >&2; stale=1; }
  done
  exit "$stale"
fi
echo "wrote $(find "$out" -type f | wc -l | tr -d ' ') files under $out"
