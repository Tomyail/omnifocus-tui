# OmniFocus TUI

独立的 OmniFocus 终端界面，使用 Rust + Ratatui，通过 [omnifocus-cli](https://github.com/stephendolan/omnifocus-cli) 直接读写 OmniFocus。可在任何交互式终端运行，不依赖 herdr。

## 运行

需要 macOS、OmniFocus、Rust，以及 Bun 安装的 `of`：

```sh
bun install -g @stephendolan/omnifocus-cli
cargo run
```

```sh
cargo run -- --check    # 只读检查连接
cargo run -- --capture  # 快速收集到 Inbox，保存后退出
cargo build --release
./target/release/omnifocus-tui
```

程序会查找 PATH 中的 `of`，然后查找 `~/.bun/bin/of`。也可设置 `OMNIFOCUS_CLI=/absolute/path/to/of`。首次使用时，macOS 可能要求允许自动化访问 OmniFocus。

## 操作

| 按键 | 操作 |
| --- | --- |
| Tab / h / l | 切换项目栏与任务栏 |
| ↑↓ / j k | 移动选择 |
| Enter | 展开或收起任务详情 |
| PgUp / PgDn | 滚动详情 |
| / | 搜索当前视图的标题、项目、标签和备注 |
| a / + | 收集到 Inbox |
| d | 完成选中任务，确认后写入 |
| f | 切换旗标 |
| y | 复制任务标题、ID、原生链接和备注，方便交给 agent |
| o | 在 OmniFocus 中打开任务 |
| r | 刷新 |
| p | 显示或隐藏项目栏 |
| q / Esc | 清除搜索、关闭详情或退出 |

支持鼠标选择和滚轮。每 30 秒自动刷新；操作在后台执行。收集成功后自动切换到 Inbox、清除搜索并选中新任务。收集和旗标直接写入 OmniFocus；完成任务需要确认，父任务与重复任务遵循 OmniFocus 自身的规则。正在执行操作时，请等待结果再退出。

## 当前范围

支持全部任务、Inbox、旗标和项目视图。使用 `of task list` 的默认活动任务范围；列表排除已完成、已放弃和项目根任务。`of` 当前按项目名称归属任务，同名项目的任务无法精确区分；日期暂显示接口返回的 UTC 时间。未实现任务层级展示、编辑日期、移动任务与重复规则编辑。

没有本地任务数据库，也没有 herdr 配置修改。pi 等 agent 可以继续通过现有的 OmniFocus MCP 操作同一份数据库。

## 验证

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

真实连接可通过 `--check` 检查。开发时已使用真实 PTY 验证启动、读取数据、搜索、详情、退出和终端恢复；写入操作未在真实用户任务上做自动化测试。
