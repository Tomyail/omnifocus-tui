# OmniFocus TUI

[![CI](https://github.com/Tomyail/omnifocus-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/Tomyail/omnifocus-tui/actions/workflows/ci.yml)

[English](README.md)

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
cargo run -- --keys     # 按键调试：打印终端上报的按键事件
cargo build --release
./target/release/omnifocus-tui
```

程序会查找 PATH 中的 `of`，然后查找 `~/.bun/bin/of`。也可设置 `OMNIFOCUS_CLI=/absolute/path/to/of`。首次使用时，macOS 可能要求允许自动化访问 OmniFocus。

界面语言依次取配置文件里的 `language`、系统语言环境（`en*` 为英文），默认中文。

## 操作

以下为默认按键，全部可通过配置文件修改（见下方「按键配置」）：

| 按键 | 操作 |
| --- | --- |
| Tab / h / l | 切换项目栏与任务栏 |
| ↑↓ / j k | 移动选择 |
| Enter | 展开或收起任务详情 |
| PgUp / PgDn | 滚动详情 |
| / | 搜索当前视图的标题、项目、标签和备注；搜索时侧栏每个视图旁显示匹配数量 |
| a / + | 收集到 Inbox |
| n | 新建任务：项目视图下进入当前项目，否则进收件箱；Tab 切换字段，Enter 保存 |
| e | 编辑选中任务的名称、标签与备注：Tab 切换字段（标签空格分隔多个，输入时有联想：↑↓ 选择、Tab 补全），备注支持 Shift+Enter 换行与多行粘贴，Enter 保存，Esc 取消 |
| f | 切换旗标 |
| t | 打开编辑表单并直接定位到标签字段 |
| D | 删除选中任务（标记为已放弃），确认后写入，可在 OmniFocus 中恢复 |
| F | 聚焦选中任务所在的项目：侧栏只保留该项目，再按 F 或 Esc 取消 |
| y | 复制任务标题、ID、原生链接和备注，方便交给 agent |
| o | 在 OmniFocus 中打开任务 |
| r | 刷新 |
| p | 显示或隐藏项目栏 |
| q / Esc | 清除搜索、取消聚焦、关闭详情或退出 |

支持鼠标选择和滚轮。每 30 秒自动刷新；操作在后台执行。收集成功后自动切换到 Inbox、清除搜索并选中新任务；新建任务保留在当前视图并选中新任务。收集、新建、旗标、标签和编辑名称备注直接写入 OmniFocus；完成和删除任务需要确认（删除即标记为已放弃，可在 OmniFocus 中恢复），父任务与重复任务遵循 OmniFocus 自身的规则。标签为整组替换语义：可增删单个标签，标签字段输入时提供联想（候选来自 OmniFocus 标签库，未取到时用任务快照里出现过的标签），输入不存在的标签会先自动创建再关联，但 `of` 不接受空标签列表，暂不支持清空全部标签。正在执行操作时，请等待结果再退出。

## 按键配置

可选配置文件 `~/.config/omnifocus-tui/config.toml`（用 `OMNIFOCUS_TUI_CONFIG` 环境变量可指定其他路径）。文件不存在时使用内置默认值；文件存在但格式或按键非法时启动即报错退出。字段缺省保持默认，出现即整组替换，空列表表示解除绑定：

```toml
language = "zh"   # 界面语言：zh（默认）或 en；未配置时按系统 LANG 检测

[keys]
down = ["j", "down"]
up = ["k", "up"]
focus_project = ["F"]
flag = ["f"]
```

| 动作 | 默认键 | 说明 |
| --- | --- | --- |
| down / up | j、↓ / k、↑ | 移动选择 |
| sidebar / tasks | h、← / l、→ | 聚焦项目栏 / 任务栏 |
| toggle_pane | Tab、Shift-Tab | 在两栏间切换焦点 |
| toggle_sidebar | p | 显示或隐藏项目栏 |
| toggle_detail | Enter | 展开或收起任务详情 |
| detail_down / detail_up | PgDn / PgUp | 滚动详情 |
| refresh | r | 刷新 |
| search | / | 搜索当前视图 |
| capture | a、+ | 收集到 Inbox |
| create | n | 在当前项目新建任务 |
| flag | f | 切换旗标 |
| tag | t | 打开编辑表单并定位到标签字段 |
| focus_project | F | 聚焦选中任务所在的项目 |
| complete | d | 完成任务（需确认） |
| delete | D | 删除任务，标记为已放弃（需确认） |
| edit | e | 编辑任务名称、备注与标签 |
| open | o | 在 OmniFocus 中打开 |
| copy | y | 复制任务信息给 agent |
| quit | q、Esc | 清除搜索、取消聚焦、关闭详情或退出 |

按键写法：单个字符区分大小写（`f` 与 `F` 是两个键）；功能键写 `up` `down` `left` `right` `enter` `esc` `tab` `backtab` `pageup` `pagedown` `space`。底部帮助栏按当前配置显示各动作的第一个绑定。搜索、收集输入框和完成确认里的 Esc、Enter、退格固定不变；Ctrl+C 始终用于退出；同一按键绑定到多个动作时，上表中排在前面的动作优先。备注换行支持 Shift+Enter 与 Option+Enter（部分 macOS 输入法会把 Shift+Enter 改报为 Option+Enter，两者等效）；协议级区分依赖 kitty 键盘协议（kitty、WezTerm、Ghostty、iTerm2 等支持）。

## Herdr 集成

`integrations/herdr/` 是一个 herdr 插件，把 TUI 以独立弹窗（popup）接入 herdr。可直接从 GitHub 一键安装：

```sh
herdr plugin install Tomyail/omnifocus-tui/integrations/herdr
```

本地开发时则链接本地目录：

```sh
integrations/herdr/install.sh
```

`install.sh` 会构建 release、链接插件，并在 `~/.config/herdr/config.toml` 追加 `prefix+o` 键位；用 `plugin install` 安装的话需自行添加：

```toml
[[keys.command]]
key = "prefix+o"
type = "plugin_action"
command = "omnifocus-tui.open"
```

之后在 herdr 里按 `prefix+o` 全屏弹出 OmniFocus；TUI 内 `q` 退出即关闭弹窗。卸载：`herdr plugin uninstall omnifocus-tui`（本地链接用 `herdr plugin unlink integrations/herdr`）并删除上述键位。

## 当前范围

支持全部任务、Inbox、旗标和项目视图。使用 `of task list` 的默认活动任务范围；列表排除已完成、已放弃和项目根任务。`of` 当前按项目名称归属任务，同名项目的任务无法精确区分；日期暂显示接口返回的 UTC 时间。未实现任务层级展示、编辑日期、移动任务与重复规则编辑。

没有本地任务数据库，也没有 herdr 配置修改。pi 等 agent 可以继续通过现有的 OmniFocus MCP 操作同一份数据库。

## 验证

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

真实连接可通过 `--check` 检查。开发时已使用真实 PTY 验证启动、读取数据、搜索、详情、退出和终端恢复；写入操作未在真实用户任务上做自动化测试。
