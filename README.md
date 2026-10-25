# OmniFocus TUI

[![CI](https://github.com/Tomyail/omnifocus-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/Tomyail/omnifocus-tui/actions/workflows/ci.yml)

[中文文档](README.zh-CN.md)

A standalone terminal UI for OmniFocus, built with Rust + Ratatui, reading and writing OmniFocus directly through [omnifocus-cli](https://github.com/stephendolan/omnifocus-cli). Runs in any interactive terminal, no herdr required.

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

On first launch, macOS asks for permission to automate OmniFocus — click **OK** and press `r` to refresh. Move with `j`/`k`, capture a thought with `a`, open the detail pane with `Enter`, quit with `q`; the footer always shows the active keys.

To uninstall (removes the binary and unregisters the herdr plugin; herdr's own config is never modified):

```sh
curl -fsSL https://raw.githubusercontent.com/Tomyail/omnifocus-tui/main/uninstall.sh | sh -s -- --purge
```

Drop `--purge` to keep the config file at `~/.config/omnifocus-tui`.

Building from source works too: `cargo run` (or `cargo build --release`). Useful flags: `--check` (read-only connection test), `--capture` (quick capture that exits after saving), `--keys` (print the key events your terminal reports). The app looks for `of` on PATH, then `~/.bun/bin/of`; override with `OMNIFOCUS_CLI=/path/to/of`.

### Troubleshooting

| Symptom | Fix |
| --- | --- |
| `omnifocus-tui: command not found` | `~/.local/bin` is not on your PATH — add `export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc` and reopen the terminal |
| "无法运行 of" or an empty task list | The `of` CLI is missing — `bun install -g @stephendolan/omnifocus-cli` |
| Still empty right after granting permission | Press `r` to refresh |
| Shift+Enter does nothing | The terminal lacks the kitty keyboard protocol — Option+Enter inserts a newline too |
| Window shows "窗口太小" | Resize the terminal to at least 35×10 |

The UI language follows `language` in the config file, then the system locale (`en*` gets English), defaulting to Chinese.

## Keys

Defaults below; every one can be remapped via the config file (see [Key configuration](#key-configuration)):

| Key | Action |
| --- | --- |
| Tab / h / l | Switch between the projects pane and the tasks pane |
| ↑↓ / j k | Move selection |
| Enter | Toggle task detail |
| PgUp / PgDn | Scroll detail |
| / | Search title, project, tags and note of the current view; while searching, each sidebar view shows its match count |
| a / + | Capture to Inbox |
| n | New task: into the current project view, or Inbox otherwise; Tab switches fields, Enter saves |
| e | Edit the selected task's name, tags and note: Tab switches fields (tags are space-separated with autocomplete — ↑↓ to pick, Tab to complete), the note supports Shift+Enter newlines and multi-line paste; Enter saves, Esc cancels |
| f | Toggle flag |
| t | Open the edit form directly on the tags field |
| D | Delete (drop) the selected task after confirmation; restorable in OmniFocus |
| F | Focus the selected task's project: the sidebar shows only that project; press F or Esc again to cancel |
| y | Copy the task title, ID, native link and note, ready to paste to an agent |
| o | Open the task in OmniFocus |
| r | Refresh |
| p | Show or hide the sidebar |
| q / Esc | Clear the search, cancel focus, close detail, or quit |

Mouse selection and the scroll wheel are supported. Auto-refresh every 30 seconds; operations run in the background. After capturing, the app switches to Inbox, clears the search and selects the new task; after creating a task it stays in the current view and selects it. Capture, create, flag, tag and name/note edits write to OmniFocus directly; completing and deleting require confirmation (delete means dropping, restorable in OmniFocus); parent tasks and repeats follow OmniFocus's own rules. Tags use whole-set replace semantics: individual tags can be added or removed, unknown names are created automatically before being linked, and the tags field offers suggestions (from the OmniFocus tag library, falling back to tags seen in the current task snapshot); clearing every tag at once is not supported because `of` rejects an empty tag list. Please wait for a running operation to finish before quitting.

## Key configuration

Optional config file `~/.config/omnifocus-tui/config.toml` (override the path with the `OMNIFOCUS_TUI_CONFIG` environment variable). A missing file means built-in defaults; a file with invalid TOML or keys aborts startup with an explicit error. Fields you set replace the whole group; an empty list unbinds the action:

```toml
language = "en"   # UI language: zh (default) or en; falls back to the system LANG when unset

[keys]
down = ["j", "down"]
up = ["k", "up"]
focus_project = ["F"]
flag = ["f"]
```

| Action | Default | Description |
| --- | --- | --- |
| down / up | j、↓ / k、↑ | Move selection |
| sidebar / tasks | h、← / l、→ | Focus the projects pane / the tasks pane |
| toggle_pane | Tab、Shift-Tab | Toggle focus between the panes |
| toggle_sidebar | p | Show or hide the sidebar |
| toggle_detail | Enter | Toggle task detail |
| detail_down / detail_up | PgDn / PgUp | Scroll detail |
| refresh | r | Refresh |
| search | / | Search the current view |
| capture | a、+ | Capture to Inbox |
| create | n | New task in the current project |
| flag | f | Toggle flag |
| tag | t | Open the edit form on the tags field |
| focus_project | F | Focus the selected task's project |
| complete | d | Complete a task (with confirmation) |
| delete | D | Delete a task, i.e. drop it (with confirmation) |
| edit | e | Edit name, tags and note |
| open | o | Open in OmniFocus |
| copy | y | Copy task info for an agent |
| quit | q、Esc | Clear search, cancel focus, close detail, or quit |

Key syntax: single characters are case-sensitive (`f` and `F` are different keys); named keys are `up` `down` `left` `right` `enter` `esc` `tab` `backtab` `pageup` `pagedown` `space`. The footer renders the first binding of each action from the live config. Esc, Enter and Backspace inside the search/capture inputs and the completion dialog are fixed, as is Ctrl+C for quitting; when one key is bound to several actions, the one earlier in the table above wins. Note newlines accept both Shift+Enter and Option+Enter (some macOS input methods redeliver Shift+Enter as Option+Enter; they are equivalent); distinguishing them at the protocol level relies on the kitty keyboard protocol (supported by kitty, WezTerm, Ghostty, iTerm2, among others).

## Herdr integration

The repo root carries a `herdr-plugin.toml`, so herdr can install it directly — a prebuilt release binary is fetched automatically (no Rust toolchain), with `cargo build` as the fallback:

```sh
herdr plugin install Tomyail/omnifocus-tui
```

Three actions come with it: `open` (full-screen popup), `open-split` and `open-tab`. Add a keybinding for the popup (herdr does not add keybindings for you):

```toml
[[keys.command]]
key = "prefix+o"
type = "plugin_action"
command = "omnifocus-tui.open"
```

For local development against a checkout instead:

```sh
integrations/herdr/install.sh
```

It builds and links the repo root with `herdr plugin link`; add the keybinding shown above yourself — this project never edits herdr's config. Press `q` inside the TUI to close the pane.

## Current scope

All tasks, Inbox, flagged and project views. Uses the default active-task scope of `of task list`; the list excludes completed, dropped and project-root tasks. `of` attributes tasks to projects by name, so tasks in same-named projects cannot be told apart; dates are shown in the UTC returned by the API. Task hierarchy, editing dates, moving tasks and repeat-rule editing are not implemented.

There is no local task database and no herdr configuration is modified. Agents such as pi can keep operating on the same database through the existing OmniFocus MCP.

## Verification

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

A real connection can be checked with `--check`. Startup, reading, search, detail, quitting and terminal restore were verified against a real PTY during development; write operations have not been automated against real user tasks.
