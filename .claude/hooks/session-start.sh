#!/bin/bash
# SessionStart hook for a Claude Code cloud session: make a fresh container the
# contributor's machine that `mise install` and `mise run setup` describe, so
# `mise run check`, the git hooks and `bd` work from the first command.
# Runs only in the cloud (CLAUDE_CODE_REMOTE=true); a laptop keeps its own setup.
# Idempotent: every step is skipped or a no-op when already done, and the
# container's state is snapshotted after the hook, so later sessions pay
# nothing for what is already installed or built.
#
# What it does, and why each step exists (docs/project/contributing.md, "Cloud sessions"):
#   1. mise from its GitHub release (mise.run is not always reachable).
#   2. MISE_ENV=cloud, so mise.cloud.toml replaces the backends the sandbox cannot
#      use with pinned release URLs; the tools go on PATH as plain binaries.
#   3. The toolchain rust-toolchain.toml names, through rustup, as CI does.
#   4. `mise install`: mbx, prek, bd, cocogitto, lychee.
#   5. The beads database: `bd bootstrap` from the committed export, then the
#      import the post-merge hook runs (scripts/bd-sync.sh; the export is how
#      the beads travel between machines).
#   6. `mise run setup`: the git hooks (prek block, then beads) and `cargo fetch`.
#   7. A warm build of every test target through mbx, so the first
#      `cargo test` in the session is incremental.
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

MISE_VERSION="2026.9.12"
root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$root"

log() { printf '[session-start] %s\n' "$*" >&2; }

export MISE_ENV=cloud
export MISE_YES=1           # never prompt (trust, install)
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

# Every later shell of the session sees the same environment.
if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
  {
    echo 'export MISE_ENV=cloud'
    echo 'export MISE_YES=1'
    echo 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'
  } >>"$CLAUDE_ENV_FILE"
fi

# 1. mise, pinned, from the release tarball.
if ! command -v mise >/dev/null 2>&1; then
  log "installing mise $MISE_VERSION"
  mkdir -p "$HOME/.local/bin"
  tmp=$(mktemp -d)
  curl -fsSL --retry 3 \
    "https://github.com/jdx/mise/releases/download/v${MISE_VERSION}/mise-v${MISE_VERSION}-linux-x64.tar.gz" \
    | tar -xz -C "$tmp"
  install -m 0755 "$tmp/mise/bin/mise" "$HOME/.local/bin/mise"
  rm -rf "$tmp"
fi
mise trust mise.toml >/dev/null 2>&1 || true
mise trust mise.cloud.toml >/dev/null 2>&1 || true
log "mise $(mise --version 2>/dev/null | head -1)"

# 3. The pinned toolchain, as CI installs it; rustup reads rust-toolchain.toml.
if ! command -v rustup >/dev/null 2>&1; then
  log "rustup is missing from the image; install it before this hook can continue" ; exit 1
fi
rustup toolchain install >/dev/null 2>&1 || rustup toolchain install
rustup component add rustfmt clippy >/dev/null 2>&1 || true
log "$(cargo --version)"

# 4. The tools (mise.toml, overridden by mise.cloud.toml). cocogitto and lychee
#    build from crates.io the first time, then live in the snapshot.
log "mise install"
mise install

# The tools as plain binaries on PATH, so git hooks and scripts find them
# without mise being activated in the shell that runs them.
for tool in bd prek cog lychee mbx; do
  if path=$(mise which "$tool" 2>/dev/null) && [ -n "$path" ]; then
    ln -sfn "$path" "$HOME/.local/bin/$tool"
  fi
done
log "bd $(bd --version 2>/dev/null | head -1), $(prek --version 2>/dev/null), $(cog --version 2>/dev/null), $(lychee --version 2>/dev/null)"

# 5. The beads database. A checkout carries .beads/metadata.json, config.yaml
#    and the export, but no database. `bd bootstrap` creates one from the export
#    (never `bd init`: it rewrites CLAUDE.md and .claude/settings.json and
#    commits them) and reports when one exists; the import after it is what
#    the post-merge hook runs, for a snapshotted container whose database is
#    behind the export (newer rows win, nothing is deleted).
chmod 700 .beads
git config beads.role >/dev/null 2>&1 || git config beads.role maintainer
bd bootstrap --non-interactive 2>&1 | grep -v '^$' | sed 's/^/[bd bootstrap] /' >&2
scripts/bd-sync.sh import 2>&1 | sed 's/^/[bd import] /' >&2

# 6. The git hooks (prek block, then beads) and a fetched registry.
mise run setup

# 7. A warm build through mbx (mise.toml wraps cargo), so the first test run is
#    incremental. Every feature, every target, nothing executed.
if [ "${JUDGMENT_SESSION_SKIP_BUILD:-}" != "1" ]; then
  log "warming the build (cargo test --all-features --no-run)"
  mise exec -- cargo test --all-features --no-run --quiet
  mise exec -- cargo check --no-default-features --all-targets --quiet
fi

log "done"
