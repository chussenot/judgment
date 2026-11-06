#!/usr/bin/env sh
# List what the jud binary offers that no page of the jud plugin names: the
# subcommands and flags in the help blocks of docs/reference/cli.md (which
# are the binary's own) and the keys of jud eval's JSON report on that
# page, grepped across plugins/jud/ outside evals/. Informational: always
# exit 0, since some are left out on purpose (the allow-list below says
# which and why). Run by `mise run docs:check`, so the next field watch
# reads the list instead of grepping by hand. POSIX sh only.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
ref=docs/reference/cli.md
plugin=plugins/jud

# Left out on purpose, one per line: NAME reason
allow='
completion a shell concern, not an agent one
--pixels named through the argument hint of /jud:run only
-h clap own
-V clap own
--help clap own
--version clap own
--out named on every page as --out DIR or PATH
help clap own
'

mentioned() {
  # $1: a word; true when a plugin page outside evals/ names it
  grep -rqF -- "$1" "$plugin" --include='*.md' --exclude-dir=evals 2>/dev/null
}
mentioned_word() {
  # $1: a word; true when a plugin page outside evals/ names it as a whole word
  grep -rqwF -- "$1" "$plugin" --include='*.md' --exclude-dir=evals 2>/dev/null
}
allowed() {
  printf '%s\n' "$allow" | awk -v k="$1" '$1 == k { found = 1 } END { exit !found }'
}

missing=""
# Subcommands: the `Commands:` list of the top-level help block.
for sub in $(awk '/^Commands:/{f=1;next} /^$/{f=0} f{print $1}' "$ref" | sort -u); do
  mentioned "jud $sub" || mentioned "\`$sub\`" || allowed "$sub" || missing="$missing
  subcommand $sub"
done
# Flags: every `--word` at the start of an option line in a help block.
for flag in $(grep -oE '^ +(-[A-Za-z], )?--[a-z][a-z-]*' "$ref" | grep -oE -- '--[a-z][a-z-]*' | sort -u); do
  mentioned "$flag" || allowed "$flag" || missing="$missing
  flag $flag"
done
# JSON keys of the eval report: the first column of its table.
for key in $(grep -oE '^\| `[a-z_]+(\[\]\.[a-z_]+)*` \|' "$ref" | sed 's/^| `//; s/` |$//' | sort -u); do
  leaf=${key##*.}
  mentioned "\`$key\`" || mentioned_word "$leaf" || allowed "$key" || missing="$missing
  key $key"
done

if [ -n "$missing" ]; then
  echo "jud offers, and no page of $plugin names (scripts/check-plugin-coverage.sh; informational):$missing"
else
  echo "every subcommand, flag and eval key of $ref is named in $plugin or allowed"
fi
exit 0
