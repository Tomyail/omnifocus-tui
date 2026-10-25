# herdr integration

[Back to README](../README.md) · [中文 README](../README.zh-CN.md)

omnifocus-tui runs in any interactive terminal and does not require [herdr](https://herdr.dev). If you do use herdr, this page covers installing it as a plugin, the available actions, and local development.

## Install as a plugin

The repo root carries a `herdr-plugin.toml`, so herdr can install it directly — a prebuilt release binary is fetched automatically (no Rust toolchain), with `cargo build` as the fallback:

```sh
herdr plugin install Tomyail/omnifocus-tui
```

## Actions

Three actions come with the plugin:

| Action | What it does |
| --- | --- |
| `omnifocus-tui.open` | Open the TUI in a full-screen popup |
| `omnifocus-tui.open-split` | Open in a split pane |
| `omnifocus-tui.open-tab` | Open in a new tab |

herdr does not add keybindings for you. Add one for the popup by appending to `~/.config/herdr/config.toml`:

```toml
[[keys.command]]
key = "prefix+o"
type = "plugin_action"
command = "omnifocus-tui.open"
```

Press `q` inside the TUI to close the pane. This project never edits herdr's config.

## Local development

To develop against a checkout instead of the published plugin:

```sh
integrations/herdr/install.sh
```

The script builds the repo and links it with `herdr plugin link`; add the keybinding shown above yourself.
