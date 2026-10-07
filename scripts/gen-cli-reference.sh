#!/usr/bin/env sh
# Refresh docs/reference/cli.md from the jud binary's own --help text, so the
# page cannot drift from the command tree clap parses.
#
#   scripts/gen-cli-reference.sh            rewrite the help blocks in place
#   scripts/gen-cli-reference.sh --check    exit 1 when a block is stale
#
# The page is hand-written prose with marked blocks. Each marker names the
# command whose help fills the fence that follows it:
#
#   <!-- help: jud check -->
#   ```text
#   ...replaced by `jud check --help`...
#   ```
#
# The binary is `$JUD` when set, else the one `cargo run --features cli`
# builds. POSIX sh and awk only, like the other scripts.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
page=docs/reference/cli.md

mode=write
if [ "${1:-}" = "--check" ]; then
  mode=check
fi

if [ -z "${JUD:-}" ]; then
  cargo build -q --features cli --bin jud
  JUD="$root/target/debug/jud"
fi

help_of() {
  # $1 is the marker's text, "jud" or "jud check": run the binary with the
  # subcommand words and --help.
  set -- $1
  shift
  "$JUD" "$@" --help
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# Pass 1: write every command's help to a file named by its marker.
grep -o '<!-- help: [^>]* -->' "$page" | sed 's/<!-- help: //; s/ -->//' | while IFS= read -r cmd; do
  file=$(printf '%s' "$cmd" | tr ' ' '_')
  help_of "$cmd" > "$tmp/$file.txt"
done

# Pass 2: rebuild the page, replacing each marked fence's body.
awk -v dir="$tmp" '
  /^<!-- help: .* -->$/ {
    cmd = $0; sub(/^<!-- help: /, "", cmd); sub(/ -->$/, "", cmd)
    file = cmd; gsub(/ /, "_", file)
    print; pending = dir "/" file ".txt"; next
  }
  pending != "" && /^```text$/ {
    print
    while ((getline line < pending) > 0) print line
    close(pending)
    pending = ""; skipping = 1; next
  }
  skipping && /^```$/ { skipping = 0; print; next }
  skipping { next }
  { print }
' "$page" > "$tmp/page.md"

case "$mode" in
  write)
    if cmp -s "$tmp/page.md" "$page"; then
      echo "$page is up to date"
    else
      cp "$tmp/page.md" "$page"
      echo "wrote $page"
    fi
    ;;
  check)
    if cmp -s "$tmp/page.md" "$page"; then
      echo "$page matches the binary's help"
    else
      diff -u "$page" "$tmp/page.md" >&2 || true
      echo "$page is stale: run scripts/gen-cli-reference.sh and commit the result" >&2
      exit 1
    fi
    ;;
esac
