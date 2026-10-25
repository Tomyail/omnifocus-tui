#!/usr/bin/env bash
# herdr 插件入口：向上探测包含 Cargo.toml 的目录作为仓库根。
# 本地 herdr plugin link 与 herdr plugin install 的 managed checkout 目录层级不同，
# 因此不能写死相对层级。二进制定位顺序：仓库构建产物 → cargo 构建 → PATH 中已安装
# 的 omnifocus-tui（如从 GitHub Releases 下载）；都没有时给出安装指引。
# panic 输出记录到日志，失败时停留 5 秒避免报错一闪而过。
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

root=""
probe="$here"
for _ in 1 2 3; do
  probe="$(dirname "$probe")"
  if [ -f "$probe/Cargo.toml" ]; then
    root="$probe"
    break
  fi
done

fail() {
  echo "$1" >&2
  echo ""
  echo "omnifocus-tui 未能启动；详细输出见 ${log:-<无>}"
  sleep 5
  exit 1
}

bin=""
if [ -n "$root" ] && [ -x "$root/target/release/omnifocus-tui" ]; then
  bin="$root/target/release/omnifocus-tui"
elif [ -n "$root" ] && command -v cargo >/dev/null 2>&1; then
  (cd "$root" && cargo build --release) || fail "cargo build --release 失败（仓库：$root）"
  [ -x "$root/target/release/omnifocus-tui" ] && bin="$root/target/release/omnifocus-tui"
fi
if [ -z "$bin" ]; then
  bin="$(command -v omnifocus-tui || true)"
fi
if [ -z "$bin" ]; then
  fail "未找到 omnifocus-tui。安装方式之一：从 https://github.com/Tomyail/omnifocus-tui/releases 下载二进制放入 PATH；或安装 Rust 工具链后重试（将自动构建）"
fi

if [ -n "$root" ]; then
  log="$root/target/omnifocus-tui-pane.log"
else
  log="${TMPDIR:-/tmp}/omnifocus-tui-pane.log"
fi

status=0
"$bin" 2>"$log" || status=$?
if [ "$status" -ne 0 ]; then
  echo ""
  echo "omnifocus-tui 退出码 $status；详细输出见 $log"
  sleep 5
fi
exit "$status"
