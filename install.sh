#!/bin/sh
# omnifocus-tui 一键安装：从 GitHub Releases 下载当前架构的最新预编译二进制。
# 用法：curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/install.sh | sh
set -eu

repo="Tomyail/omnifocus-tui"
install_dir="${OMNIFOCUS_TUI_INSTALL_DIR:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) target="aarch64-apple-darwin" ;;
  Darwin-x86_64) target="x86_64-apple-darwin" ;;
  *)
    echo "不支持的系统：$(uname -s) $(uname -m)（需要 macOS + OmniFocus）" >&2
    exit 1
    ;;
esac

echo "=> 正在获取最新版本…"
version="$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest" |
  sed -n 's/.*"tag_name": *"\(v[^"]*\)".*/\1/p' | head -1)"
[ -n "$version" ] || { echo "无法获取最新版本号，请检查网络" >&2; exit 1; }
ver="${version#v}"
fname="omnifocus-tui-${ver}-${target}.tar.gz"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "=> 下载 $fname"
curl -fL "https://github.com/$repo/releases/download/$version/$fname" -o "$tmp/$fname"

echo "=> 校验 SHA256"
curl -fsL "https://github.com/$repo/releases/download/$version/checksums.txt" -o "$tmp/checksums.txt"
expected="$(grep " $fname\$" "$tmp/checksums.txt" | awk '{print $1}')"
actual="$(shasum -a 256 "$tmp/$fname" | awk '{print $1}')"
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  echo "校验失败（expected=${expected} actual=${actual}）" >&2
  exit 1
fi

tar -xzf "$tmp/$fname" -C "$tmp"
mkdir -p "$install_dir"
mv "$tmp/omnifocus-tui" "$install_dir/omnifocus-tui"
chmod +x "$install_dir/omnifocus-tui"
echo "=> 已安装到 $install_dir/omnifocus-tui（v${ver}）"

case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    echo "⚠  $install_dir 不在 PATH 里，请执行并重开终端："
    echo "    echo 'export PATH=\"$install_dir:\$PATH\"' >> ~/.zshrc"
    ;;
esac

if ! command -v of >/dev/null 2>&1; then
  echo "⚠  未找到 of CLI（运行时依赖）。安装："
  echo "    bun install -g @stephendolan/omnifocus-cli"
  echo "    （bun 安装：curl -fsSL https://bun.sh/install | bash）"
fi

echo "=> 完成！在任意终端运行 omnifocus-tui 开始使用。"
