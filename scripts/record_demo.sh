#!/usr/bin/env sh
# Record the command-line demo as an asciinema cast (v2 format), with no key
# and no network: the answers come from the recordings under
# examples/recordings/jud_calibration through `jud --replay`, the crate's
# offline backend, so the cast is the same on every run and can be regenerated
# on any change.
#
#   scripts/record_demo.sh [OUT]       default OUT: docs/demo.cast
#   asciinema play docs/demo.cast      or upload it, or embed it with the player
#
# JUD names the binary to record (default: the one this checkout builds). The
# cast is written by this script, not by `asciinema rec`, so no terminal and
# no asciinema install are needed: a v2 cast is a JSON header and one JSON
# event per line, which awk writes from the transcript the commands produce.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
out=${1:-"$root/docs/demo.cast"}
case "$out" in /*) ;; *) out="$PWD/$out" ;; esac

if [ -z "${JUD:-}" ]; then
  (cd "$root" && cargo build -q --features cli --bin jud)
  JUD="$root/target/debug/jud"
fi

# A clean room: the example rubric, its cases and its recordings under short
# names, no key, no configuration file, so what is typed is what runs.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM
cp "$root/examples/jud/triage.jud" "$root/examples/jud/triage-cases.jud" "$tmp/"
mkdir "$tmp/recordings" "$tmp/xdg"
cp "$root"/examples/recordings/jud_calibration/*.jud "$tmp/recordings/"
printf '%s\n' '{"message": "This is the third time I'"'"'m writing. I was charged twice last month and nobody has refunded me. Fix it today or I cancel."}' > "$tmp/event.json"
cd "$tmp"
unset TYPESAFE_API_KEY TYPESAFE_BASE_URL
export XDG_CONFIG_HOME="$tmp/xdg" JUD_REPLAY=recordings PATH="$(dirname -- "$JUD"):$PATH"

# The transcript: `#` a comment shown in the prompt, `$` a command typed and
# run, with its stdout and stderr captured after it.
transcript="$tmp/transcript"
say() { printf '#\t%s\n' "$1" >> "$transcript"; }
run() {
  printf '$\t%s\n' "$1" >> "$transcript"
  sh -c "$1" 2>&1 | sed 's/^/>\t/' >> "$transcript" || true
}
: > "$transcript"
say "jud: a decision written as a file, answered by a calibrated model"
run "jud --version"
say "the state is JSON on stdin; the rubric holds the questions and the policy"
run "cat event.json"
run "cat event.json | jud triage.jud"
say "the documents are checked the way the library reads them"
run "jud check triage.jud triage-cases.jud"
say "docs/cli.md: install, configuration, every subcommand"

# The cast: header, then [time, "o", text] events. Keystrokes are typed at
# 40 ms, a line of output takes 20 ms, a comment holds for a second.
mkdir -p "$(dirname -- "$out")"
awk -v width=100 -v height=30 '
  # A JSON string: the backslash first, then the quote and every control
  # character a transcript or an escape sequence can hold.
  function esc(s) {
    gsub(/\\/, "\\\\", s); gsub(/"/, "\\\"", s)
    gsub(/\t/, "\\t", s); gsub(/\r/, "\\r", s); gsub(/\n/, "\\n", s)
    gsub(/\033/, "\\u001b", s)
    return s
  }
  function emit(dt, text) { t += dt; printf "[%.3f, \"o\", \"%s\"]\n", t, esc(text) }
  BEGIN {
    printf "{\"version\": 2, \"width\": %d, \"height\": %d, \"env\": {\"SHELL\": \"/bin/sh\", \"TERM\": \"xterm-256color\"}, \"title\": \"jud: a decision as a file\"}\n", width, height
    t = 0
  }
  {
    kind = substr($0, 1, 1); text = substr($0, 3)
    if (kind == "#") { emit(0.6, "\033[2m# " text "\033[0m\r\n") }
    else if (kind == "$") {
      emit(0.4, "\033[1;32m$\033[0m ")
      n = length(text)
      for (i = 1; i <= n; i++) emit(0.04, substr(text, i, 1))
      emit(0.3, "\r\n")
    }
    else { emit(0.02, text "\r\n") }
  }
  END { emit(1.0, "") }
' "$transcript" > "$out"
echo "wrote $out ($(wc -l < "$out") events)"
