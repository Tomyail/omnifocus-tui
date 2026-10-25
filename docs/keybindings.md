# Key bindings

[Back to README](../README.md) · [中文 README](../README.zh-CN.md)

Every key in omnifocus-tui can be remapped through an optional config file. This page is the full reference; the README covers only the defaults.

## Config file

Location: `~/.config/omnifocus-tui/config.toml` (override the path with the `OMNIFOCUS_TUI_CONFIG` environment variable).

Rules:

- A missing file means built-in defaults.
- A file with invalid TOML or invalid keys aborts startup with an explicit error.
- A field you set replaces the whole group; an empty list unbinds the action.

```toml
language = "en"   # UI language: zh (default) or en; falls back to the system LANG when unset

[keys]
down = ["j", "down"]
up = ["k", "up"]
focus_project = ["F"]
flag = ["f"]
```

## Actions

| Action | Default | Description |
| --- | --- | --- |
| down / up | j、↑ / k、↓ | Move selection |
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

## Key syntax

- Single characters are case-sensitive: `f` and `F` are two different keys.
- Named keys: `up` `down` `left` `right` `enter` `esc` `tab` `backtab` `pageup` `pagedown` `space`.

## Precedence and fixed keys

- The footer renders the first binding of each action from the live config.
- Esc, Enter and Backspace inside the search/capture inputs and the completion dialog are fixed, as is Ctrl+C for quitting.
- When one key is bound to several actions, the action listed earlier in the table above wins.

## Newlines in the note field

Newlines accept both Shift+Enter and Option+Enter. Some macOS input methods redeliver Shift+Enter as Option+Enter; they are equivalent. Distinguishing them at the protocol level relies on the kitty keyboard protocol, supported by kitty, WezTerm, Ghostty, iTerm2, among others. In terminals without it, Option+Enter still inserts a newline.
