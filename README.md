# OmniFocus TUI

[![CI](https://github.com/Tomyail/omnifocus-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/Tomyail/omnifocus-tui/actions/workflows/ci.yml)

[中文文档](README.zh-CN.md)

A terminal UI for OmniFocus on macOS, built with Rust + Ratatui. It reads and writes your live OmniFocus database directly through [omnifocus-cli](https://github.com/stephendolan/omnifocus-cli) — no sync, no local copy, and nothing else on your system is modified.

![omnifocus-tui](assets/screenshot.png)

## Quick start

**Prerequisites:** a Mac with [OmniFocus](https://www.omnigroup.com/omnifocus/) installed.

**1.** Install the `of` CLI (needs [Bun](https://bun.sh)):

```sh
curl -fsSL https://bun.sh/install | bash   # skip if Bun is already installed
bun install -g @stephendolan/omnifocus-cli
```

**2.** Install omnifocus-tui — one line, no Rust toolchain required:

```sh
curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/install.sh | sh
```

**3.** Run it from any terminal:

```sh
omnifocus-tui
```

Upgrading later is one command: `omnifocus-tui update`. The app also checks for a new release in the background at startup (at most once a day) and installs it for the next launch; set `auto_update = false` in the config file to turn this off.

### First launch

macOS asks once for permission to automate OmniFocus — click **OK**, then press `r` to refresh. That's the whole setup.

A 30-second tour:

| Key | Does |
| --- | --- |
| `j` / `k` | Move selection |
| `a` | Capture a thought to Inbox |
| `Enter` | Open task detail |
| `q` | Quit |

The footer always shows the keys available right now, so you can't get lost. Mouse clicks and the scroll wheel work too.

## Keys (defaults)

| Key | Action |
| --- | --- |
| ↑↓ / j k | Move selection |
| Tab / h / l | Switch between the projects pane and the tasks pane |
| Enter | Toggle task detail |
| PgUp / PgDn | Scroll detail |
| / | Search the current view |
| a / + | Capture to Inbox |
| n | New task in the current project (Inbox otherwise) |
| e | Edit name, tags and note — Tab switches fields, Enter saves, Esc cancels |
| f | Toggle flag |
| t | Jump straight to the tags field |
| d | Complete task (asks for confirmation) |
| D | Delete task (asks for confirmation; restorable in OmniFocus) |
| F | Focus the task's project — sidebar shows only that project, F or Esc cancels |
| y | Copy the task's title, ID, link and note |
| o | Open the task in OmniFocus |
| r | Refresh |
| p | Show or hide the sidebar |
| q / Esc | Clear search, cancel focus, close detail, or quit |

Every key can be remapped in the config file — see [Key bindings](docs/keybindings.md).

Worth knowing:

- Starts in Inbox, followed by flagged and project views in the sidebar.
- The list auto-refreshes every 30 seconds; operations run in the background.
- Capture, create, flag, tag and edits write to OmniFocus immediately; completing and deleting ask for confirmation first.
- After capturing, the app switches to Inbox and selects the new task.

## Configuration

Optional config file at `~/.config/omnifocus-tui/config.toml`:

```toml
language = "en"   # UI language: zh (default) or en

[keys]
capture = ["a", "+"]   # remap any action
```

See [Key bindings](docs/keybindings.md) for the full action list, key syntax and validation rules.

## herdr (optional)

If you use [herdr](https://herdr.dev), install it as a plugin and get a full-screen popup:

```sh
herdr plugin install Tomyail/omnifocus-tui
```

Keybinding setup and other actions: [herdr integration](docs/herdr.md).

## Uninstall

```sh
curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/uninstall.sh | sh -s -- --purge
```

Drop `--purge` to keep the config file at `~/.config/omnifocus-tui`.

## Development

```sh
cargo run                                          # run from source
cargo test && cargo clippy --all-targets -- -D warnings
```

Useful flags: `--check` (read-only connection test), `--capture` (quick capture that exits after saving), `--keys` (print the key events your terminal reports); `omnifocus-tui update` downloads the latest release and replaces the binary in place. The app looks for `of` on PATH, then `~/.bun/bin/of`; override with `OMNIFOCUS_CLI=/path/to/of`.

## Documentation

- [Key bindings & remapping](docs/keybindings.md) — every action, key syntax, precedence rules
- [herdr integration](docs/herdr.md) — popup, split and tab actions, local development
