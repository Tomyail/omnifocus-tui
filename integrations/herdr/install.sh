#!/usr/bin/env bash
# Local-development install: build the release binary and link the repo root
# as a herdr plugin. herdr's own config is never modified — add keybindings
# yourself. End users should prefer `herdr plugin install Tomyail/omnifocus-tui`.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

(cd "$root" && cargo build --release)
herdr plugin link "$root" --enabled

echo "done: plugin linked. Optional keybinding for ~/.config/herdr/config.toml:"
echo '  [[keys.command]]'
echo '  key = "prefix+o"'
echo '  type = "plugin_action"'
echo '  command = "omnifocus-tui.open"'
