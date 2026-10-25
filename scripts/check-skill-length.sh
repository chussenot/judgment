#!/usr/bin/env sh
# Refuse a plugin skill's SKILL.md over 500 lines. The plugin ships to users,
# and a SKILL.md loads whole into their context whenever the skill triggers;
# Anthropic's skill-authoring guidance keeps it under 500 lines and moves the
# detail into files under references/ that load when a step needs them.
# Usage: scripts/check-skill-length.sh plugins/jud/skills/*/SKILL.md
set -eu

max=500
fail=0
for f in "$@"; do
  n=$(wc -l < "$f")
  if [ "$n" -gt "$max" ]; then
    echo "$f: $n lines, over $max; move a section into references/ and say in SKILL.md when to read it"
    fail=1
  fi
done
exit "$fail"
