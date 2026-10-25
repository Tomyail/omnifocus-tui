#!/usr/bin/env bash
# Local-development install: build the release binary, link the repo root as
# a herdr plugin (herdr-plugin.toml lives there), and add the prefix+o
# keybinding. End users should prefer `herdr plugin install Tomyail/omnifocus-tui`.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

(cd "$root" && cargo build --release)
herdr plugin link "$root" --enabled

config="${HERDR_CONFIG:-$HOME/.config/herdr/config.toml}"
if [ -f "$config" ] && ! grep -q 'omnifocus-tui.open' "$config"; then
  printf '\n[[keys.command]]\nkey = "prefix+o"\ntype = "plugin_action"\ncommand = "omnifocus-tui.open"\ndescription = "open omnifocus"\n' >>"$config"
  echo "已添加 prefix+o 键位 / added prefix+o keybinding"
fi
echo "完成 / done: press prefix+o in herdr for the OmniFocus popup"
