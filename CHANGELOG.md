# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Conventional Commits](https://www.conventionalcommits.org).
## v0.1.0 — 2026-10-25
### Bug fixes

- Auto-create unknown tags before writing them to a task (`3d543ce`)
- Use report-all-keys kitty flags so Shift+Enter is delivered (`ab76648`)
- Accept Option+Enter as note newline too (`293e14a`)
### Documentation

- English README as default, Chinese moved to README.zh-CN.md (`5dea99a`)
### Features

- Add standalone OmniFocus terminal UI (`60633f5`)
- Project focus, configurable keymap, task delete/edit/create (`71a4192`)
- Add tags to tasks via t dialog (`543a685`)
- Show per-view match counts in sidebar while searching (`9a83774`)
- Tag autocomplete in the task form (`4fb3cc1`)
- Bilingual UI (zh default, en via config or LANG) (`ab1d245`)
- Note field last in the form, Shift+Enter inserts newlines (`3dfbbf3`)
- Herdr integration as a popup pane on prefix+o (`896e97e`)
### Miscellaneous

- Prepare for GitHub publish (`e83c39c`)
- Release tooling — changelog, binary releases, no-toolchain fallback (`53cce76`)
### Refactoring

- Fold tag editing into the shared task form (`8ff31d7`)

