#!/bin/sh
# Carry the beads between a machine that cannot push `refs/dolt/data` (a cloud
# container) and a laptop, through the committed export `.beads/issues.jsonl`.
#
#   scripts/bd-sync.sh export   container: write every issue to the file
#   scripts/bd-sync.sh import   laptop:    upsert the file into the local database
#   scripts/bd-sync.sh status   either:    issue counts, database against file
#
# The file is a passive export: no history, no Dolt branches. Import is an
# upsert, so it creates and updates issues and never deletes one; an issue
# deleted on one side stays on the other. The file is the only thing committed.
set -eu

file=".beads/issues.jsonl"
mode="${1:-status}"

count_file() { [ -f "$file" ] && wc -l <"$file" | tr -d ' ' || echo 0; }
count_db() { bd export --all 2>/dev/null | wc -l | tr -d ' '; }

case "$mode" in
export)
  bd export --all -o "$file"
  echo "exported $(count_file) issues to $file"
  if git diff --quiet -- "$file" 2>/dev/null; then
    echo "no change to commit"
  else
    git diff --stat -- "$file"
    echo "next: git add $file && git commit -m 'docs(beads): refresh the export' && git push"
  fi
  ;;
import)
  [ -f "$file" ] || { echo "no $file: git pull first" >&2; exit 2; }
  before="$(count_db)"
  bd import "$file"
  echo "database: $before issues before, $(count_db) after; file: $(count_file)"
  ;;
status)
  echo "database: $(count_db) issues; $file: $(count_file)"
  ;;
*)
  echo "usage: $0 export|import|status" >&2
  exit 2
  ;;
esac
