#!/usr/bin/env sh
# PostToolUse (Edit|Write): read a .jud document the agent just wrote with
# the crate's reader, so a refusal reaches it on the next turn whether or
# not it remembered to run `jud check`. The document is checked alone (a
# cases document unbound, which `jud check` allows); the skill's check of
# the rubric and its cases together still applies. Silent when the document
# reads; exit 2 with the `error` lines on stderr when it does not, which
# Claude Code hands to the agent. Fails open when no `jud` can be found:
# the script's own message, and nothing blocked.
set -eu
input=$(cat)
if command -v jq >/dev/null 2>&1; then
  file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')
else
  file=$(printf '%s' "$input" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)
fi
case "$file" in
  *.jud) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0
here=$(cd "$(dirname "$0")" && pwd)
out=$("$here/../skills/jud/scripts/jud.sh" check "$file" 2>&1) || true
case "$out" in
  *" 0 refused"*) exit 0 ;;
  *"jud.sh: no jud on PATH"*) exit 0 ;;
esac
printf 'jud check %s refused the document; fix each error in the document, not around it:\n%s\n' "$file" "$(printf '%s\n' "$out" | grep -E '^(error|[0-9]+ documents)' || printf '%s' "$out")" >&2
exit 2
