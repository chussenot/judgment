#!/usr/bin/env sh
# The three Claude Code plugins this repository works with, installed or
# updated through the `claude` command line: `jud` from this checkout (the
# marketplace is the checkout itself, as a directory source, so the plugin
# loads in place and an edit under plugins/jud/ is live at the next session
# start or /reload-plugins), `typesafe` from TypeSafe's own marketplace
# (github typesafe-ai/skills), whose `typesafe-ai` skill CLAUDE.md asks for
# before touching the wire, and `ponytail` (github DietrichGebert/ponytail),
# the less-code discipline CLAUDE.md asks for, pinned to the tag below so a
# new release never changes a session's behaviour unseen: moving the pin
# is an edit of PONYTAIL_REF here and of the same `ref` in
# .claude/settings.json, together.
#
#   scripts/claude-plugins.sh install   add the marketplaces and install the plugins, user scope; idempotent
#   scripts/claude-plugins.sh update    refresh the marketplaces and update the plugins (ponytail to its pin)
#
# `.claude/settings.json` declares the same plugins for the project, which a
# laptop's Claude Code installs on its own when the session is interactive;
# a cloud session's does not, so .claude/hooks/session-start.sh runs
# `install`, and `mise run plugins:install` and `mise run plugins:update`
# wrap both for a laptop. Every step is a no-op when already done, and the
# commands never prompt (-y answers for the one case that would). Without a
# `claude` on PATH the script says so and exits 0: nothing else depends on it.
#
# POSIX sh only.
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mode=${1:-install}
PONYTAIL_REF=v5.2.0
# `owner/repo` is cloned over HTTPS, never probed over SSH first: a container
# has no SSH key, and the proxy serves HTTPS.
export CLAUDE_CODE_PLUGIN_PREFER_HTTPS=1

log() { printf '[claude-plugins] %s\n' "$*" >&2; }

if ! command -v claude >/dev/null 2>&1; then
  log "no \`claude\` on PATH: the plugins are not installed (https://code.claude.com/docs/en/plugins)"
  exit 0
fi

# One line of the CLI's JSON per command, so a log reads what happened; the
# exit status is the command's own.
run() {
  out=$(claude plugin "$@" --json 2>&1) || {
    log "claude plugin $*: $out"
    return 1
  }
  printf '%s\n' "$out" | tail -n 1 | sed 's/^/[claude-plugins] /' >&2
}

# A marketplace already registered under the name, from any source (a
# laptop's Claude Code registers `judgment` from GitHub when it trusts the
# workspace, since .claude/settings.json names it so), is left as it is:
# the plugin installs from wherever the name points.
has_marketplace() {
  claude plugin marketplace list --json 2>/dev/null | grep -q "\"name\": \"$1\""
}

case "$mode" in
  install)
    has_marketplace judgment || run marketplace add "$root" --scope user
    has_marketplace typesafe-ai || run marketplace add typesafe-ai/skills --scope user
    has_marketplace ponytail || run marketplace add "DietrichGebert/ponytail#$PONYTAIL_REF" --scope user
    run install jud@judgment --scope user -y
    run install typesafe@typesafe-ai --scope user -y
    run install ponytail@ponytail --scope user -y
    ;;
  update)
    run marketplace update judgment
    run marketplace update typesafe-ai
    run marketplace update ponytail
    run update jud@judgment -y
    run update typesafe@typesafe-ai -y
    run update ponytail@ponytail -y
    log "a running session sees the update after /reload-plugins or at its next start"
    ;;
  *)
    echo "usage: scripts/claude-plugins.sh install|update" >&2
    exit 2
    ;;
esac
