#!/bin/sh
# omnifocus-tui uninstaller.
# Usage: curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/uninstall.sh | sh
# Pass --purge to also remove ~/.config/omnifocus-tui (config file).
# herdr's own config is never modified; a leftover keybinding is only reported.
set -eu

install_dir="${OMNIFOCUS_TUI_INSTALL_DIR:-$HOME/.local/bin}"
purge=0
for arg in "$@"; do
  case "$arg" in
    --purge) purge=1 ;;
    *) echo "Unknown option: $arg" >&2; exit 1 ;;
  esac
done

# 1) The binary
if [ -e "$install_dir/omnifocus-tui" ]; then
  rm -f "$install_dir/omnifocus-tui"
  echo "=> Removed $install_dir/omnifocus-tui"
else
  echo "=> No binary at $install_dir/omnifocus-tui (already uninstalled?)"
fi

# 2) The herdr plugin, if registered
herdr_config="$HOME/.config/herdr"
if command -v herdr >/dev/null 2>&1; then
  entry="$(herdr plugin list 2>/dev/null | grep '^- omnifocus-tui ' || true)"
  case "$entry" in
    *"[local:"*)
      path="${entry##*\[local:}"
      path="${path%\]}"
      herdr plugin unlink "$path"
      echo "=> Unlinked the herdr plugin ($path)"
      ;;
    *"[github:"*)
      herdr plugin uninstall omnifocus-tui
      echo "=> Uninstalled the herdr plugin"
      ;;
  esac
fi

# 3) Report — never modify — any leftover keybinding in herdr's config
conf="$herdr_config/config.toml"
if [ -f "$conf" ] && grep -q 'omnifocus-tui\.open' "$conf"; then
  echo "note: a keybinding referencing omnifocus-tui.open remains in $conf;"
  echo "      remove that [[keys.command]] block manually if you no longer want it."
fi

# 4) Config file, only with --purge
if [ "$purge" -eq 1 ]; then
  rm -rf "$HOME/.config/omnifocus-tui"
  echo "=> Removed ~/.config/omnifocus-tui"
elif [ -d "$HOME/.config/omnifocus-tui" ]; then
  echo "=> Config kept at ~/.config/omnifocus-tui (re-run with --purge to remove it)"
fi

echo "=> Done. A source checkout, if any, was left untouched."
