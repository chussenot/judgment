#!/usr/bin/env sh
# PostToolUse (Bash): the Edit and Write hooks format a Rust file and regenerate
# docs/llms.txt after an edit, but most edits in a long session go through Bash
# (a python heredoc, sed -i, cat >), which those hooks never see; one session
# ran `cargo fmt` and the llms generator by hand two hundred times for it.
# This hook asks git what the command left modified and does the same two
# things: rustfmt on every modified Rust file, the llms generator when a page,
# the README, llms-intro.txt or the mkdocs nav changed. Unmerged files are
# left alone (rustfmt would choke on conflict markers), and so is everything
# outside a git checkout. Cost when nothing changed: one `git status`.
set -eu
root=${CLAUDE_PROJECT_DIR:-.}
cd "$root" 2>/dev/null || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

status=$(git status --porcelain --untracked-files=all -- '*.rs' docs README.md mkdocs.yml 2>/dev/null || true)
[ -n "$status" ] || exit 0

# Paths of the modified, added or untracked files, without the two-letter
# status and without the unmerged (UU, AA) and deleted ones.
paths=$(printf '%s\n' "$status" | grep -v '^UU\|^AA\|^DD\|^.D\|^D' | sed 's/^...//; s/^.* -> //')

rs=$(printf '%s\n' "$paths" | grep '\.rs$' || true)
if [ -n "$rs" ] && command -v rustfmt >/dev/null 2>&1; then
  printf '%s\n' "$rs" | while IFS= read -r f; do
    [ -f "$f" ] && rustfmt --edition 2024 "$f" >/dev/null 2>&1 || true
  done
fi

if printf '%s\n' "$paths" | grep -Eq '^(docs/.*\.md|README\.md|docs/llms-intro\.txt|mkdocs\.yml)$'; then
  sh scripts/gen-llms-txt.sh >/dev/null 2>&1 || true
fi
exit 0
