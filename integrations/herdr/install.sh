#!/usr/bin/env bash
# 一键接入 herdr：构建 release、链接本地插件，并添加 prefix+o 键位。
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

(cd "$root" && cargo build --release)
herdr plugin link "$here" --enabled

config="${HERDR_CONFIG:-$HOME/.config/herdr/config.toml}"
if [ -f "$config" ] && ! grep -q 'omnifocus-tui.open' "$config"; then
  printf '\n[[keys.command]]\nkey = "prefix+o"\ntype = "plugin_action"\ncommand = "omnifocus-tui.open"\ndescription = "open omnifocus"\n' >>"$config"
  echo "已添加 prefix+o 键位"
fi
echo "安装完成：herdr 里按 prefix+o 弹出 OmniFocus"
