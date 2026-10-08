#!/usr/bin/env sh
# Run the `jud` command (src/bin/jud/): `jud.sh SUBCOMMAND ...` passes its
# arguments on, so `check`, `lower`, `record`, `eval` and `tune` all run
# through it. Uses a `jud` on PATH (cargo install judgment --features cli),
# else builds it from a judgment checkout: the one JUDGMENT_DIR names, the
# project, the one this plugin sits in, or the current directory.
set -eu
if command -v jud >/dev/null 2>&1; then
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
