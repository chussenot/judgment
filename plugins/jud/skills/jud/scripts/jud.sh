#!/usr/bin/env sh
# Run the `jud` command (src/bin/jud/): `jud.sh SUBCOMMAND ...` passes its
# arguments on, so `check`, `lower`, `record`, `eval` and `tune` all run
# through it. Uses a `jud` on PATH (cargo install judgment --features cli),
# else builds it from a judgment checkout: the one JUDGMENT_DIR names, the
# project, the one this plugin sits in, or the current directory.
#
# The plugin's pages describe the flags and the JSON keys of `jud` at
# JUD_MIN or later. An older binary on PATH still runs, after one line on
# stderr saying so, so a page's flag that it refuses is explained and not a
# mystery. POSIX sh only.
set -eu
# ponytail: the pages describe --fail-if-moved, ece_probability and
# --injection-perturb, unreleased past 0.16.1; raise JUD_MIN to the release
# that ships them when it is cut (a dev build still reports 0.16.1).
JUD_MIN=0.16.1

older_than_min() {
  # $1: a version as `jud --version` prints it (`jud X.Y.Z`). True when it
  # is below JUD_MIN; false when it is at or above it, or cannot be read.
  v=${1#jud }
  v=${v%%[!0-9.]*}
  [ -n "$v" ] || return 1
  set -- "$v" "$JUD_MIN"
  a1=${1%%.*}; r=${1#*.}; a2=${r%%.*}; a3=${r#*.}
  b1=${2%%.*}; r=${2#*.}; b2=${r%%.*}; b3=${r#*.}
  for n in "$a1" "$a2" "$a3"; do case "$n" in ''|*[!0-9]*) return 1;; esac; done
  [ "$a1" -lt "$b1" ] && return 0
  [ "$a1" -gt "$b1" ] && return 1
  [ "$a2" -lt "$b2" ] && return 0
  [ "$a2" -gt "$b2" ] && return 1
  [ "$a3" -lt "$b3" ]
}

if command -v jud >/dev/null 2>&1; then
  if older_than_min "$(jud --version 2>/dev/null || true)"; then
    echo "jud.sh: $(jud --version) is older than the plugin's pages (they describe $JUD_MIN or later; mise use -g github:chussenot/judgment@latest, or cargo install judgment --features cli); a flag or a report key a page names may be missing" >&2
  fi
  exec jud "$@"
fi
here=$(cd "$(dirname "$0")" && pwd)
for root in "${JUDGMENT_DIR:-}" "${CLAUDE_PROJECT_DIR:-}" "$here/../../../../.." "$PWD"; do
  [ -n "$root" ] || continue
  if [ -f "$root/Cargo.toml" ] && grep -q '^name = "judgment"$' "$root/Cargo.toml"; then
    exec cargo run -q --manifest-path "$root/Cargo.toml" --features cli --bin jud -- "$@"
  fi
done
echo "jud.sh: no jud on PATH and no judgment checkout found; run \`cargo install judgment --features cli\` or set JUDGMENT_DIR" >&2
exit 2
