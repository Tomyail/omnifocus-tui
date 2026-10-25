#!/bin/sh
# omnifocus-tui uninstaller.
# Usage: curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/uninstall.sh | sh
# Pass --purge to also remove ~/.config/omnifocus-tui (config file).
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

# 3) The prefix+o keybinding herdr integration added (config backed up first)
conf="$herdr_config/config.toml"
if [ -f "$conf" ] && grep -q 'omnifocus-tui\.open' "$conf"; then
  cp "$conf" "$conf.omnifocus-backup"
  awk '
    function flushbuf() { if (inblock == 1 && keep == 1) printf "%s", buf; inblock = 0 }
    /^\[\[/ {
      flushbuf()
      if ($0 ~ /^\[\[keys\.command\]\]/) { inblock = 1; keep = 1; buf = $0 "\n" }
      else print
      next
    }
    /^\[/ { flushbuf(); print; next }
    inblock == 1 {
      buf = buf $0 "\n"
      if ($0 ~ /omnifocus-tui\.open/) keep = 0
      next
    }
    { print }
    END { flushbuf() }
  ' "$conf" > "$conf.tmp" && mv "$conf.tmp" "$conf"
  echo "=> Removed the prefix+o keybinding (backup: $conf.omnifocus-backup)"
fi

# 4) Config file, only with --purge
if [ "$purge" -eq 1 ]; then
  rm -rf "$HOME/.config/omnifocus-tui"
  echo "=> Removed ~/.config/omnifocus-tui"
elif [ -d "$HOME/.config/omnifocus-tui" ]; then
  echo "=> Config kept at ~/.config/omnifocus-tui (re-run with --purge to remove it)"
fi

echo "=> Done. A source checkout, if any, was left untouched."
