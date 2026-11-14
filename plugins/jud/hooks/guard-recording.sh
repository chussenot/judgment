#!/usr/bin/env sh
# PreToolUse (Edit|Write): a recording is what a model said, and the tuning
# skill's rule is that it is never edited to make `eval` pass. Deny an Edit
# or Write whose target is an existing .jud file that reads `kind:
# Recording`. A new file is allowed: a hand-written fixture is the one
# legitimate recording an agent writes (the `jud` skill names it).
#
# The deny is JSON the host must be able to read: a quote or a backslash in
# the path is escaped and a control character dropped, because a decision
# the host cannot parse is no decision and the edit would go through
# (tests/plugin_hooks.rs runs this script on such paths).
set -eu
input=$(cat)
if command -v jq >/dev/null 2>&1; then
  file=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')
else
  # Without jq: the JSON string with its escapes, then the three a path can
  # carry undone (\" \\ \/). A path with any other escape in it (a control
  # character, \uXXXX) is not decoded, is not found below, and is let through.
  file=$(printf '%s' "$input" | sed -nE 's/.*"file_path"[[:space:]]*:[[:space:]]*"(([^"\\]|\\.)*)".*/\1/p' | head -n1 | sed -E 's/\\(["\\/])/\1/g')
fi
case "$file" in
  *.jud) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0
if grep -qE '^kind:[[:space:]]*Recording[[:space:]]*$' "$file"; then
  shown=$(printf '%s' "$file" | tr -d '\000-\037' | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"%s is a Recording: what a model answered once. The jud-tune skill never edits, deletes or hand-writes one to make eval pass; a request the rubric or the cases changed is recorded again with jud record (announce the cost first), and a stale file is deleted by the user."}}\n' "$shown"
fi
exit 0
