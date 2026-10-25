#!/usr/bin/env bash
# herdr 插件入口：定位仓库根目录，按需构建并启动 release 版 omnifocus-tui。
# 插件位于 <仓库>/integrations/herdr，回仓库根是两级 ../..。
# panic 输出记录到 target/omnifocus-tui-pane.log；崩溃时停留 5 秒避免报错一闪而过。
set -uo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
log="$root/target/omnifocus-tui-pane.log"
bin="$root/target/release/omnifocus-tui"

if [ ! -x "$bin" ] && [ -f "$root/Cargo.toml" ]; then
  (cd "$root" && cargo build --release)
fi

status=0
if [ -x "$bin" ]; then
  "$bin" 2>"$log" || status=$?
else
  echo "未找到 omnifocus-tui：$bin" >&2
  echo "请在仓库 $root 里先运行 cargo build --release" >&2
  status=1
fi

if [ "$status" -ne 0 ]; then
  echo ""
  echo "omnifocus-tui 退出码 $status；详细输出见 $log"
  sleep 5
fi
exit "$status"
