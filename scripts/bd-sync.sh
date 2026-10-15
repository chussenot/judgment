#!/bin/sh
# Carry the beads between machines through the committed export,
# .beads/issues.jsonl. The hooks do this on their own; the script is what they
# run, and what to run by hand when they did not (docs/project/contributing.md,
# "Tracking work"):
#
#   scripts/bd-sync.sh export   write every issue to the file (bd also does this
#                               after each of its writes: .beads/config.yaml)
#   scripts/bd-sync.sh import   upsert the file into the local database: rows
#                               older than the database's are skipped, nothing
#                               is deleted (the post-merge and post-checkout
#                               hooks, and a cloud session's start)
#   scripts/bd-sync.sh status   issue counts, and whether the file is current
#
# The file is a passive export: no history, no Dolt branches, reviewable in a
# pull request. A machine without bd, or a checkout without a database, is a
# quiet no-op for import and status, so a git hook never fails on them.
set -eu

file=".beads/issues.jsonl"
mode="${1:-status}"

have_db() { command -v bd >/dev/null 2>&1 && [ -d .beads/embeddeddolt ]; }
count_file() { [ -f "$file" ] && wc -l <"$file" | tr -d ' ' || echo 0; }
count_db() { bd export --all 2>/dev/null | wc -l | tr -d ' '; }

case "$mode" in
export)
  command -v bd >/dev/null 2>&1 || { echo "bd not found (mise install)" >&2; exit 2; }
  bd export --all -o "$file"
  echo "exported $(count_file) issues to $file"
  if git diff --quiet -- "$file" 2>/dev/null; then
    echo "no change to commit"
  else
    git diff --stat -- "$file"
    echo "next: git add $file && git commit -m 'chore(beads): refresh the export' && git push"
  fi
  ;;
import)
  have_db || exit 0
  [ -f "$file" ] || exit 0
  before="$(count_db)"
  # BD_GIT_HOOK, which beads' own hook shims set, makes bd skip the auto-export
  # after this import: the file stays as git wrote it, so a checkout of an
  # older branch leaves a clean tree; the next bd write exports it again.
  BD_GIT_HOOK=1 bd import "$file" || true
  echo "database: $before issues before, $(count_db) after; file: $(count_file)"
  ;;
status)
  have_db || { echo "no bd or no database here; $file: $(count_file) issues"; exit 0; }
  echo "database: $(count_db) issues; $file: $(count_file)"
  if bd export --all 2>/dev/null | cmp -s - "$file"; then
    echo "the file is current"
  else
    echo "the file differs from the database: 'scripts/bd-sync.sh import' takes newer rows from it, 'export' rewrites it"
  fi
  ;;
*)
  echo "usage: $0 export|import|status" >&2
  exit 2
  ;;
esac
