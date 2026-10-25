#!/bin/sh
# omnifocus-tui one-line installer: downloads the latest prebuilt binary
# for the current architecture from GitHub Releases.
# Usage: curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/install.sh | sh
set -eu

repo="Tomyail/omnifocus-tui"
install_dir="${OMNIFOCUS_TUI_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *)
    echo "Unsupported system: $(uname -s) $(uname -m) (macOS with OmniFocus is required)" >&2
    exit 1
    ;;
esac

echo "=> Fetching the latest version…"
version="$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" |
  sed -n 's/.*"tag_name": *"\(v[^"]*\)".*/\1/p' | head -1)"
[ -n "$version" ] || { echo "Could not resolve the latest version; check your network" >&2; exit 1; }
ver="${version#v}"
fname="omnifocus-tui-${ver}-${target}.tar.gz"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "=> Downloading $fname"
curl -fL "https://github.com/$repo/releases/download/$version/$fname" -o "$tmp/$fname"

echo "=> Verifying SHA256"
curl -fsL "https://github.com/$repo/releases/download/$version/checksums.txt" -o "$tmp/checksums.txt"
expected="$(grep " $fname\$" "$tmp/checksums.txt" | awk '{print $1}')"
actual="$(shasum -a 256 "$tmp/$fname" | awk '{print $1}')"
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  echo "Checksum mismatch (expected=${expected}, actual=${actual})" >&2
  exit 1
fi

tar -xzf "$tmp/$fname" -C "$tmp"
mkdir -p "$install_dir"
mv "$tmp/omnifocus-tui" "$install_dir/omnifocus-tui"
chmod +x "$install_dir/omnifocus-tui"
echo "=> Installed $install_dir/omnifocus-tui (v${ver})"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "WARN: $install_dir is not on your PATH; run the line below and reopen the terminal:"
    echo "    echo 'export PATH=\"$install_dir:\$PATH\"' >> ~/.zshrc"
    ;;
esac

if ! command -v of >/dev/null 2>&1; then
  echo "WARN: the of CLI (runtime dependency) was not found. Install it with:"
  echo "    bun install -g @stephendolan/omnifocus-cli"
  echo "    (install Bun first: curl -fsSL https://bun.sh/install | bash)"
fi

echo "=> Done! Run omnifocus-tui in any terminal to get started."
