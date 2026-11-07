#!/usr/bin/env sh
# PreToolUse (Edit|Write): a recording is what a model said, and the tuning
# skill's rule is that it is never edited to make `eval` pass. Deny an Edit
# or Write whose target is an existing .jud file that reads `kind:
# Recording`. A new file is allowed: a hand-written fixture is the one
# legitimate recording an agent writes (the `jud` skill names it).
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
if grep -qE '^kind:[[:space:]]*Recording[[:space:]]*$' "$file"; then
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s is a Recording: what a model answered once. The jud-tune skill never edits, deletes or hand-writes one to make eval pass; a request the rubric or the cases changed is recorded again with jud record (announce the cost first), and a stale file is deleted by the user."}}\n' "$file"
fi
exit 0
