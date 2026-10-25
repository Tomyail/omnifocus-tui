#!/bin/sh
# herdr build step: place a prebuilt release binary at target/release when one
# matches this checkout's version and platform; otherwise build with cargo.
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
[ -f "$root/Cargo.toml" ] || { echo "fetch-binary: no Cargo.toml at $root" >&2; exit 1; }

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *) echo "fetch-binary: unsupported platform $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

ver="$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$root/Cargo.toml" | head -1)"
out="$root/target/release/omnifocus-tui"
mkdir -p "$root/target/release"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
url="https://github.com/Tomyail/omnifocus-tui/releases/download/v${ver}/omnifocus-tui-${ver}-${target}.tar.gz"
if curl -fsL "$url" -o "$tmp/pkg.tar.gz"; then
  tar -xzf "$tmp/pkg.tar.gz" -C "$tmp"
  mv "$tmp/omnifocus-tui" "$out"
  chmod +x "$out"
  echo "fetch-binary: placed release binary at $out (v${ver})"
else
  echo "fetch-binary: no matching release binary, building with cargo"
  (cd "$root" && cargo build --release)
fi
