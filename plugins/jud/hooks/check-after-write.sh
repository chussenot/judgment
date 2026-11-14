#!/usr/bin/env sh
# PostToolUse (Edit|Write): read a .jud document the agent just wrote with
# the crate's reader, so a refusal reaches it on the next turn whether or
# not it remembered to run `jud check`. The document is checked alone (a
# cases document unbound, which `jud check` allows); the skill's check of
# the rubric and its cases together still applies. Silent when the document
# reads; exit 2 with the `error` lines on stderr when it does not, which
# Claude Code hands to the agent.
#
# Two rules keep the hook from saying what `jud` did not (tests/plugin_hooks.rs
# runs this script against stand-ins for both):
# - It never builds. A hook has seconds, and a build that fails or waits on
#   a lock is not a verdict on the document. With no `jud` on PATH it says
#   nothing (JUD_NO_BUILD stops jud.sh before its `cargo run`).
# - Only `jud check`'s own count of refused documents is a refusal. Any
#   other output (a `jud` that crashed, a wrapper's message) blocks nothing.
set -eu
input=$(cat)
if command -v jq >/dev/null 2>&1; then
  file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')
else
  # Without jq: the JSON string with its escapes, then the three a path can
  # carry undone (\" \\ \/).
  file=$(printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p' | head -n1 | sed -E 's/\\(["\\/])/\1/g')
fi
case "$file" in
  *.jud) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0
here=$(cd "$(dirname "$0")" && pwd)
out=$(JUD_NO_BUILD=1 "$here/../skills/jud/scripts/jud.sh" check "$file" 2>&1) || true
printf '%s\n' "$out" | grep -qE '^[0-9]+ documents, [1-9][0-9]* refused' || exit 0
printf 'jud check %s refused the document; fix each error in the document, not around it:\n%s\n' "$file" "$(printf '%s\n' "$out" | grep -E '^(error|[0-9]+ documents)')" >&2
exit 2
