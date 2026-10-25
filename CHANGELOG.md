# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Conventional Commits](https://www.conventionalcommits.org).
## [v0.1.3](https://github.com/Tomyail/omnifocus-tui/releases/tag/v0.1.3) — 2026-10-25
### Bug fixes

- Start in Inbox and remove the all-tasks view ([`0e73a20`](https://github.com/Tomyail/omnifocus-tui/commit/0e73a20e0ade8a38d78dc0628d98e9e985b6f96b))
## [v0.1.2](https://github.com/Tomyail/omnifocus-tui/releases/tag/v0.1.2) — 2026-10-25
### Features

- Update command to self-update to the latest release ([`2ceb4de`](https://github.com/Tomyail/omnifocus-tui/commit/2ceb4defc46fdf1e3b5065725f2f2661726ab6b9))
## [v0.1.1](https://github.com/Tomyail/omnifocus-tui/releases/tag/v0.1.1) — 2026-10-25
### Bug fixes

- Update rust crate toml to v1 ([`c577e2b`](https://github.com/Tomyail/omnifocus-tui/commit/c577e2b36fd013197cda747efb9759a884799d71))
### Documentation

- One-line install script and beginner quick start ([`5b9b86c`](https://github.com/Tomyail/omnifocus-tui/commit/5b9b86c03130c554bfc019d7a72a99c777f4730a))
- Herdr install step in quick start ([`3611675`](https://github.com/Tomyail/omnifocus-tui/commit/3611675b5c4b6aa68c38bb89da9af457310908b5))
- Streamline README for first-time users, move customization to docs ([`4b94cdc`](https://github.com/Tomyail/omnifocus-tui/commit/4b94cdc44c77dfc4ef8f4fa0a7c0d79d4b3e6f73))
- Streamline README for first-time users, move customization to docs ([`c8a8079`](https://github.com/Tomyail/omnifocus-tui/commit/c8a8079405b67938ded22ffdae4a8b52d17257ab))
- Add screenshot to READMEs ([`ba447c0`](https://github.com/Tomyail/omnifocus-tui/commit/ba447c0486626c921b0b44fc99b30c05eb29b3ac))
### Miscellaneous

- Install.sh messages in English ([`2cb6c16`](https://github.com/Tomyail/omnifocus-tui/commit/2cb6c160346e234b6f6c84d94789f97e05dec694))
- One-line uninstaller ([`621c849`](https://github.com/Tomyail/omnifocus-tui/commit/621c8492690d94c012233c7f2d1fd68fd863ec9e))
### Refactoring

- Herdr plugin manifest at repo root, tsk-style ([`f0598ca`](https://github.com/Tomyail/omnifocus-tui/commit/f0598ca26a3eb00f641aa4072507599b491394bc))
- Stop touching herdr's config from our scripts ([`2d37b6b`](https://github.com/Tomyail/omnifocus-tui/commit/2d37b6b1e19e4e0b46fd2904c84d2de62eddc006))
## [v0.1.0](https://github.com/Tomyail/omnifocus-tui/releases/tag/v0.1.0) — 2026-10-25
### Bug fixes

- Auto-create unknown tags before writing them to a task ([`3d543ce`](https://github.com/Tomyail/omnifocus-tui/commit/3d543ce7d4d8da315199d8ea9fac50fe265c1dec))
- Use report-all-keys kitty flags so Shift+Enter is delivered ([`ab76648`](https://github.com/Tomyail/omnifocus-tui/commit/ab76648994d090c11371524df78f853d7103e01f))
- Accept Option+Enter as note newline too ([`293e14a`](https://github.com/Tomyail/omnifocus-tui/commit/293e14aaa96da26c9f320e9f09680c91273296d9))
### Documentation

- English README as default, Chinese moved to README.zh-CN.md ([`5dea99a`](https://github.com/Tomyail/omnifocus-tui/commit/5dea99a1d592fb0fcaae78399c4201cca08aa388))
### Features

- Add standalone OmniFocus terminal UI ([`60633f5`](https://github.com/Tomyail/omnifocus-tui/commit/60633f5396d789630b749996a9470af14e9cf90c))
- Project focus, configurable keymap, task delete/edit/create ([`71a4192`](https://github.com/Tomyail/omnifocus-tui/commit/71a41923cb8d5904504bb065173a07e619552c08))
- Add tags to tasks via t dialog ([`543a685`](https://github.com/Tomyail/omnifocus-tui/commit/543a68589ff6382b24aad43bfcc0923122fb2cc9))
- Show per-view match counts in sidebar while searching ([`9a83774`](https://github.com/Tomyail/omnifocus-tui/commit/9a83774b63f552f4ac76b5e8283826cd419a05c7))
- Tag autocomplete in the task form ([`4fb3cc1`](https://github.com/Tomyail/omnifocus-tui/commit/4fb3cc11325543e5eec9370dd154c0b96b8a753b))
- Bilingual UI (zh default, en via config or LANG) ([`ab1d245`](https://github.com/Tomyail/omnifocus-tui/commit/ab1d24563a74a4307b4edd7da73f31c762d41fbc))
- Note field last in the form, Shift+Enter inserts newlines ([`3dfbbf3`](https://github.com/Tomyail/omnifocus-tui/commit/3dfbbf3f1fc867aec945752200da0d5998a731c4))
- Herdr integration as a popup pane on prefix+o ([`896e97e`](https://github.com/Tomyail/omnifocus-tui/commit/896e97e9f37fa6b3080c31fcb93608ac83c44d8d))
### Miscellaneous

- Prepare for GitHub publish ([`e83c39c`](https://github.com/Tomyail/omnifocus-tui/commit/e83c39ca901e1bf51e293c3ede498656a6a124d6))
- Release tooling — changelog, binary releases, no-toolchain fallback ([`53cce76`](https://github.com/Tomyail/omnifocus-tui/commit/53cce76b6f5061edb40afd99a190f2bebce79df6))
### Refactoring

- Fold tag editing into the shared task form ([`8ff31d7`](https://github.com/Tomyail/omnifocus-tui/commit/8ff31d77052893e7522c7a3477a52f9af799d800))

