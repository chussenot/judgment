#!/bin/sh
# post-merge, post-checkout: name the tools mise.toml pins that this machine
# has not installed. A pull that brings a new cargo tool otherwise surfaces
# as the next `mise exec` or `mise run` stopping to compile it, for minutes,
# in the middle of whatever it was asked to do. Read-only and offline; says
# nothing when every tool is there.
command -v mise >/dev/null 2>&1 || exit 0
missing=$(MISE_OFFLINE=1 mise ls --missing --no-header 2>/dev/null | awk '{print $1 "@" $2}' | tr '\n' ' ')
[ -n "$missing" ] || exit 0
printf 'mise: this checkout pins tools this machine lacks: %s\n' "$missing"
printf "  run 'mise install' now (a cargo tool compiles for minutes); otherwise the next 'mise exec' or 'mise run' installs them first.\n"
exit 0
