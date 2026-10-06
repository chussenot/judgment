#!/usr/bin/env sh
# Run the `jud` command (src/bin/jud.rs): `jud.sh check FILE...` or
# `jud.sh lower RUBRIC ...`. Uses a `jud` on PATH (cargo install judgment
# --features jud), else builds it from the judgment checkout this skill sits
# in, or the one JUDGMENT_DIR names.
set -eu
if command -v jud >/dev/null 2>&1; then
  exec jud "$@"
fi
here=$(cd "$(dirname "$0")" && pwd)
for root in "${JUDGMENT_DIR:-}" "$here/../../../.." "$PWD"; do
  [ -n "$root" ] || continue
  if [ -f "$root/Cargo.toml" ] && grep -q '^name = "judgment"$' "$root/Cargo.toml"; then
    exec cargo run -q --manifest-path "$root/Cargo.toml" --features jud --bin jud -- "$@"
  fi
done
echo "jud.sh: no jud on PATH and no judgment checkout found; run \`cargo install judgment --features jud\` or set JUDGMENT_DIR" >&2
exit 2
