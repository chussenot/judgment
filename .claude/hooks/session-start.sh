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
#   4. `mise install`: mbx, prek, bd, cocogitto, lychee, the cargo gates
#      (semver-checks, deny, hack), insta and mutants, all release binaries.
#   5. The beads database: `bd bootstrap` from the committed export, then the
#      import the post-merge hook runs (scripts/bd-sync.sh; the export is how
#      the beads travel between machines).
#   6. `mise run setup`: the git hooks (prek block, then beads) and `cargo fetch`.
#   7. The three Claude Code plugins (scripts/claude-plugins.sh): `jud` from
#      this checkout, `typesafe` from typesafe-ai/skills and `ponytail` from
#      DietrichGebert/ponytail at its pinned tag. A cloud session
#      never installs the plugins .claude/settings.json declares (that needs
#      the trust dialog a laptop shows), so the hook does, at user scope;
#      they load at the next session start, which the snapshot makes every
#      later session of the container, and the hook asks for a skills
#      reload so the first session may see them too.
#   8. beads-mcp, the MCP server for beads (a separate Python package, not part
#      of the bd binary), through `uv tool install` at the version bd is
#      pinned to, when JUDGMENT_SESSION_BEADS_MCP=1. Installed, not
#      registered: `claude mcp add` is a choice of scope, left to whoever
#      wants the tools.
#   9. A ceiling on the mbx cache (gc.max_total_size) and a collection, so
#      the session disk, a fixed allowance, is never filled by the cache.
#  10. A warm build of every test target through mbx, so the first
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

# 4. The tools (mise.toml, overridden by mise.cloud.toml), every one a pinned
#    release binary: nothing is compiled, so a fresh container pays seconds.
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

# 7. The Claude Code plugins, through the `claude` command the image carries;
#    idempotent, and a missing `claude` is logged, not fatal.
scripts/claude-plugins.sh install

# 8. beads-mcp, at bd's own version (`bd --version`, so the pin in mise.toml
#    and mise.cloud.toml is the only one), only on request: the hook does not
#    register it, so an install nobody asked for is half a minute for nothing.
#    Idempotent: uv leaves a tool that is already at that version alone. A
#    missing uv or a failed install is logged, not fatal: the MCP server is a
#    convenience, `bd` is the tool.
if [ "${JUDGMENT_SESSION_BEADS_MCP:-}" != "1" ]; then
  log "beads-mcp skipped (JUDGMENT_SESSION_BEADS_MCP=1 installs it)"
elif command -v uv >/dev/null 2>&1; then
  bd_version=$(bd --version 2>/dev/null | sed -n 's/^bd version \([0-9][0-9.]*\).*/\1/p' | head -1 || true)
  spec="beads-mcp${bd_version:+==$bd_version}"
  if uv tool install "$spec" >/dev/null 2>&1; then
    log "$(uv tool list 2>/dev/null | grep '^beads-mcp' | head -1)"
  else
    log "could not install $spec with uv; the beads MCP server is not available this session"
  fi
else
  log "uv is not on PATH; skipping beads-mcp"
fi

# 9. The mbx cache budget. The session disk is a fixed allowance (about 40 GB
#    here), and an unbounded cache filled it once, a push failing mid-test.
#    mbx keeps a free-space floor by default; this adds a ceiling on what it
#    may hold in total, and collects down to it now, so a snapshotted
#    container never starts full. A budget the user set is kept.
mbx_conf="${XDG_CONFIG_HOME:-$HOME/.config}/mbx/config.toml"
mkdir -p "$(dirname "$mbx_conf")"
if ! grep -q 'max_total_size' "$mbx_conf" 2>/dev/null; then
  if grep -q '^\[gc\]' "$mbx_conf" 2>/dev/null; then
    sed -i 's/^\[gc\]/[gc]\nmax_total_size = "16GiB"/' "$mbx_conf"
  else
    printf '[gc]\nmin_free_size = "2GiB"\nmax_total_size = "16GiB"\n' >>"$mbx_conf"
  fi
fi
mise exec -- mbx gc >/dev/null 2>&1 || true
log "mbx: $(mise exec -- mbx stats 2>/dev/null | grep -E '^(shared cache|managed targets)' | tr -s ' ' | paste -sd ';' -)"

# 10. A warm build through mbx (mise.toml wraps cargo), so the first test run is
#    incremental. Every feature, every target, nothing executed.
if [ "${JUDGMENT_SESSION_SKIP_BUILD:-}" != "1" ]; then
  log "warming the build (cargo test --all-features --no-run)"
  mise exec -- cargo test --all-features --no-run --quiet
  mise exec -- cargo check --no-default-features --all-targets --quiet
fi

# 11. The network. The environment's egress policy decides which hosts answer,
#     and a session used to learn that a host is blocked mid-task, from a
#     failed fetch. Each host below is one the work depends on; the result
#     goes to the session as context, not only to the log.
probe() {
  blocked=""
  for h in "$@"; do
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 6 "https://$h/" 2>/dev/null || true)
    case "$code" in 000|"") blocked="$blocked $h" ;; esac
  done
  printf '%s' "$blocked"
}
hosts="api.typesafe.ai docs.typesafe.ai index.crates.io static.crates.io static.rust-lang.org docs.rs github.com release-assets.githubusercontent.com"
# PyPI serves only beads-mcp, which step 8 installs on request.
pypi=""
if [ "${JUDGMENT_SESSION_BEADS_MCP:-}" = "1" ]; then hosts="$hosts pypi.org"; pypi=", PyPI"; fi
blocked=$(probe $hosts)
if [ -n "$blocked" ]; then
  network="blocked by the egress policy:$blocked. The contract work (tests/openapi_drift.rs, the live TypeSafe docs CLAUDE.md names) needs api.typesafe.ai and docs.typesafe.ai; the environment's allowed domains are the fix (docs/project/contributing.md, Cloud sessions)."
else
  network="every host the work depends on answers (TypeSafe API and docs, crates.io, rust-lang, docs.rs, GitHub releases$pypi)."
fi
log "network: $network"
log "done"

# The plugins installed above load at the next session start; this asks the
# session that is starting to re-scan skills and commands as well
# (https://code.claude.com/docs/en/hooks, SessionStart `reloadSkills`), and
# hands it the network probe as context.
context="[session-start] network: $network Background commands write their output to the scratchpad through tail: the task output filesystem is small and a full one loses the output."
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","reloadSkills":true,"additionalContext":%s}}\n' "$(printf '%s' "$context" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"
