#!/usr/bin/env sh
# The pre-bump hook of `cog bump` (cog.toml): write the new version where the
# repository states it, so the bump commit carries everything a release needs.
#
#   scripts/release-bump.sh NEW_VERSION [PREVIOUS_VERSION]
#
# - Cargo.toml: the [package] version.
# - Cargo.lock: the crate's own entry, through cargo, never by hand.
# - README.md and the pages tests/docs_examples.rs holds to it: every
#   `judgment = ` dependency line names MAJOR.MINOR; then docs/llms-full.txt,
#   which copies those pages, is generated again.
# - CHANGELOG.md: the "## [Unreleased]" section becomes "## [NEW] - today" and
#   a fresh, empty Unreleased section is opened above it. An Unreleased section
#   with nothing in it fails the bump: a release must say what it contains, and
#   cocogitto has no way to know whether the hand-written changelog was kept up.
#
# Run by cocogitto, which then commits the result; safe to run by hand on a
# branch to see what a bump would change (`git diff`), then `git checkout .`.
set -eu

version=${1:?usage: $0 NEW_VERSION [PREVIOUS_VERSION]}
previous=${2:-}
case "$version" in
  [0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "release-bump: '$version' is not a version" >&2; exit 2 ;;
esac

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"

die() { echo "release-bump: $*" >&2; exit 1; }

# --- Cargo.toml: the first `version = "…"` line, which is [package]'s.
current=$(awk -F'"' '/^version = "/ { print $2; exit }' Cargo.toml)
[ -n "$current" ] || die "no version line in Cargo.toml"
if [ -n "$previous" ] && [ "$current" != "$previous" ]; then
  die "Cargo.toml says $current but the latest tag is v$previous; the two must agree before a bump"
fi
awk -v v="$version" '!done && /^version = "/ { sub(/"[^"]*"/, "\"" v "\""); done = 1 } { print }' Cargo.toml > Cargo.toml.tmp
mv Cargo.toml.tmp Cargo.toml

# --- Cargo.lock: let cargo rewrite the crate's own entry.
cargo update --workspace --quiet

# --- The dependency lines, major.minor only (a caret requirement, so a patch
# release changes nothing a reader must copy): `judgment = "X.Y"` and
# `judgment = { version = "X.Y", ... }`, in every page that shows one. The
# list is the one tests/docs_examples.rs checks; keep the two together.
grep -q '^judgment = "[0-9]*\.[0-9]*"$' README.md || die 'README.md has no judgment = "X.Y" install line'
for page in README.md docs/start/install.md docs/reference/crate.md docs/start/first-decision-rust.md; do
  [ -f "$page" ] || die "$page is gone; update this list and tests/docs_examples.rs together"
  sed -E '/^judgment = /s/"[0-9]+\.[0-9]+"/"'"${version%.*}"'"/' "$page" > "$page.tmp"
  mv "$page.tmp" "$page"
done

# --- CHANGELOG.md
grep -q '^## \[Unreleased\]' CHANGELOG.md || die "CHANGELOG.md has no '## [Unreleased]' section"
if grep -q "^## \[$version\]" CHANGELOG.md; then
  die "CHANGELOG.md already has a section for $version"
fi
# The Unreleased section must hold at least one entry before the next release
# heading (or the end of the file).
entries=$(awk '
  /^## \[Unreleased\]/ { inside = 1; next }
  inside && /^## / { exit }
  inside && /^- / { n++ }
  END { print n + 0 }
' CHANGELOG.md)
[ "$entries" -gt 0 ] || die "the Unreleased section of CHANGELOG.md is empty; write what $version contains before bumping"

today=$(date -u +%Y-%m-%d)
awk -v v="$version" -v d="$today" '
  /^## \[Unreleased\]/ && !done {
    print "## [Unreleased]"
    print ""
    print "## [" v "] - " d
    done = 1
    next
  }
  { print }
' CHANGELOG.md > CHANGELOG.md.tmp
mv CHANGELOG.md.tmp CHANGELOG.md

# --- docs/llms-full.txt copies the pages above; a later hook checks it.
scripts/gen-llms-txt.sh >/dev/null

echo "release-bump: $current -> $version in Cargo.toml, Cargo.lock, the dependency lines, CHANGELOG.md and llms-full.txt ($today)"
