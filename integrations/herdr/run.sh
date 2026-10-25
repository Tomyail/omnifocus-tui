#!/usr/bin/env bash
# herdr 插件入口：向上探测包含 Cargo.toml 的目录作为仓库根。
# 本地 herdr plugin link 与 herdr plugin install 的 managed checkout 目录层级不同，
# 因此不能写死相对层级。按需构建并启动 release 版 omnifocus-tui；
# panic 输出记录到 target/omnifocus-tui-pane.log，失败时停留 5 秒避免报错一闪而过。
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
if [ -z "$root" ] && [ -f "$here/Cargo.toml" ]; then
  root="$here"
fi

fail() {
  echo "$1" >&2
  echo ""
  echo "omnifocus-tui 未能启动；详细输出见 target/omnifocus-tui-pane.log"
  sleep 5
  exit 1
}

[ -n "$root" ] || fail "未找到 omnifocus-tui 仓库根（不含 Cargo.toml）：$here 附近目录布局异常"

log="$root/target/omnifocus-tui-pane.log"
bin="$root/target/release/omnifocus-tui"

if [ ! -x "$bin" ]; then
  (cd "$root" && cargo build --release) || fail "cargo build --release 失败（仓库：$root）"
fi
[ -x "$bin" ] || fail "未找到二进制：$bin"

status=0
"$bin" 2>"$log" || status=$?
if [ "$status" -ne 0 ]; then
  echo ""
  echo "omnifocus-tui 退出码 $status；详细输出见 $log"
  sleep 5
fi
exit "$status"
