use crate::i18n::Lang;
use crossterm::event::KeyCode;
use serde::Deserialize;
use std::{env, fs, path::PathBuf};

/// 普通模式下可配置的动作。搜索/收集输入框与完成确认内的 Esc、Enter、退格，以及 Ctrl+C，固定不变。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Down,
    Up,
    Sidebar,
    Tasks,
    TogglePane,
    ToggleSidebar,
    ToggleDetail,
    DetailDown,
    DetailUp,
    Refresh,
    Search,
    Capture,
    Create,
    Flag,
    Tag,
    FocusProject,
    Complete,
    Delete,
    Edit,
    Open,
    Copy,
    Quit,
}

#[derive(Clone, Debug)]
pub struct Keymaps {
    pub down: Vec<KeyCode>,
    pub up: Vec<KeyCode>,
    pub sidebar: Vec<KeyCode>,
    pub tasks: Vec<KeyCode>,
    pub toggle_pane: Vec<KeyCode>,
    pub toggle_sidebar: Vec<KeyCode>,
    pub toggle_detail: Vec<KeyCode>,
    pub detail_down: Vec<KeyCode>,
    pub detail_up: Vec<KeyCode>,
    pub refresh: Vec<KeyCode>,
    pub search: Vec<KeyCode>,
    pub capture: Vec<KeyCode>,
    pub create: Vec<KeyCode>,
    pub flag: Vec<KeyCode>,
    pub tag: Vec<KeyCode>,
    pub focus_project: Vec<KeyCode>,
    pub complete: Vec<KeyCode>,
    pub delete: Vec<KeyCode>,
    pub edit: Vec<KeyCode>,
    pub open: Vec<KeyCode>,
    pub copy: Vec<KeyCode>,
    pub quit: Vec<KeyCode>,
}

impl Default for Keymaps {
    fn default() -> Self {
        Self::from_toml("").expect("空配置必然解析为内置默认值")
    }
}

impl Keymaps {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| format!("TOML 格式错误：{e}"))?;
        keymaps_from_raw(raw.keys)
    }

    fn table(&self) -> [(Action, &[KeyCode]); 22] {
        [
            (Action::Down, &self.down),
            (Action::Up, &self.up),
            (Action::Sidebar, &self.sidebar),
            (Action::Tasks, &self.tasks),
            (Action::TogglePane, &self.toggle_pane),
            (Action::ToggleSidebar, &self.toggle_sidebar),
            (Action::ToggleDetail, &self.toggle_detail),
            (Action::DetailDown, &self.detail_down),
            (Action::DetailUp, &self.detail_up),
            (Action::Refresh, &self.refresh),
            (Action::Search, &self.search),
            (Action::Capture, &self.capture),
            (Action::Create, &self.create),
            (Action::Flag, &self.flag),
            (Action::Tag, &self.tag),
            (Action::FocusProject, &self.focus_project),
            (Action::Complete, &self.complete),
            (Action::Delete, &self.delete),
            (Action::Edit, &self.edit),
            (Action::Open, &self.open),
            (Action::Copy, &self.copy),
            (Action::Quit, &self.quit),
        ]
    }

    /// 按动作声明顺序返回第一个匹配；同一按键绑定多个动作时排在前面的优先。
    pub fn action(&self, code: KeyCode) -> Option<Action> {
        self.table()
            .iter()
            .find(|(_, keys)| keys.contains(&code))
            .map(|(action, _)| *action)
    }

    /// 动作的第一绑定，用于帮助栏展示。
    pub fn first(&self, action: Action) -> String {
        self.table()
            .iter()
            .find(|(a, _)| *a == action)
            .and_then(|(_, keys)| keys.first())
            .map(key_name)
            .unwrap_or_else(|| "—".into())
    }
}

/// 语言与按键配置。默认中文 + 内置默认键；未配置语言时按系统环境检测。
#[derive(Clone, Debug, Default)]
pub struct Config {
    pub language: Lang,
    pub keys: Keymaps,
}

impl Config {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| format!("TOML 格式错误：{e}"))?;
        let language = match raw.language {
            Some(value) => Lang::from_config(&value).map_err(|e| format!("language：{e}"))?,
            None => Lang::default(),
        };
        Ok(Self {
            language,
            keys: keymaps_from_raw(raw.keys)?,
        })
    }

    pub fn load() -> Result<Self, String> {
        let Some(path) = config_path() else {
            return Ok(Self::detect_from_env());
        };
        // 配置文件不存在时按系统环境选择语言；存在但内容非法时显式报错，避免静默忽略用户配置。
        let Ok(text) = fs::read_to_string(&path) else {
            return Ok(Self::detect_from_env());
        };
        Self::from_toml(&text).map_err(|error| format!("{}：{error}", path.display()))
    }

    fn detect_from_env() -> Self {
        let binding = env::var_os("LC_ALL")
            .or_else(|| env::var_os("LC_MESSAGES"))
            .or_else(|| env::var_os("LANG"));
        let env_value = binding.as_deref().and_then(|value| value.to_str());
        Self {
            language: Lang::detect(env_value),
            keys: Keymaps::default(),
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    language: Option<String>,
    keys: RawKeys,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawKeys {
    down: Option<Vec<String>>,
    up: Option<Vec<String>>,
    sidebar: Option<Vec<String>>,
    tasks: Option<Vec<String>>,
    toggle_pane: Option<Vec<String>>,
    toggle_sidebar: Option<Vec<String>>,
    toggle_detail: Option<Vec<String>>,
    detail_down: Option<Vec<String>>,
    detail_up: Option<Vec<String>>,
    refresh: Option<Vec<String>>,
    search: Option<Vec<String>>,
    capture: Option<Vec<String>>,
    create: Option<Vec<String>>,
    flag: Option<Vec<String>>,
    tag: Option<Vec<String>>,
    focus_project: Option<Vec<String>>,
    complete: Option<Vec<String>>,
    delete: Option<Vec<String>>,
    edit: Option<Vec<String>>,
    open: Option<Vec<String>>,
    copy: Option<Vec<String>>,
    quit: Option<Vec<String>>,
}

fn keymaps_from_raw(raw: RawKeys) -> Result<Keymaps, String> {
    // 配置里出现的字段整组替换默认键，未出现的保持默认；空列表表示解除绑定。
    let resolve = |field: &str, raw: Option<Vec<String>>, default: &[&str]| {
        raw.map_or_else(
            || Ok(defaults(default)),
            |specs| {
                specs
                    .iter()
                    .map(|s| parse_key(s))
                    .collect::<Result<Vec<_>, _>>()
            },
        )
        .map_err(|e| format!("keys.{field}：{e}"))
    };
    Ok(Keymaps {
        down: resolve("down", raw.down, &["j", "down"])?,
        up: resolve("up", raw.up, &["k", "up"])?,
        sidebar: resolve("sidebar", raw.sidebar, &["h", "left"])?,
        tasks: resolve("tasks", raw.tasks, &["l", "right"])?,
        toggle_pane: resolve("toggle_pane", raw.toggle_pane, &["tab", "backtab"])?,
        toggle_sidebar: resolve("toggle_sidebar", raw.toggle_sidebar, &["p"])?,
        toggle_detail: resolve("toggle_detail", raw.toggle_detail, &["enter"])?,
        detail_down: resolve("detail_down", raw.detail_down, &["pagedown"])?,
        detail_up: resolve("detail_up", raw.detail_up, &["pageup"])?,
        refresh: resolve("refresh", raw.refresh, &["r"])?,
        search: resolve("search", raw.search, &["/"])?,
        capture: resolve("capture", raw.capture, &["a", "+"])?,
        create: resolve("create", raw.create, &["n"])?,
        flag: resolve("flag", raw.flag, &["f"])?,
        tag: resolve("tag", raw.tag, &["t"])?,
        focus_project: resolve("focus_project", raw.focus_project, &["F"])?,
        complete: resolve("complete", raw.complete, &["d"])?,
        delete: resolve("delete", raw.delete, &["D"])?,
        edit: resolve("edit", raw.edit, &["e"])?,
        open: resolve("open", raw.open, &["o"])?,
        copy: resolve("copy", raw.copy, &["y"])?,
        quit: resolve("quit", raw.quit, &["q", "esc"])?,
    })
}

fn config_path() -> Option<PathBuf> {
    env::var_os("OMNIFOCUS_TUI_CONFIG")
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("HOME").map(|home| {
                PathBuf::from(home)
                    .join(".config")
                    .join("omnifocus-tui")
                    .join("config.toml")
            })
        })
}

fn defaults(specs: &[&str]) -> Vec<KeyCode> {
    specs
        .iter()
        .map(|s| parse_key(s).expect("内置默认键必须合法"))
        .collect()
}

pub fn parse_key(spec: &str) -> Result<KeyCode, String> {
    match spec {
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        "enter" | "return" => Ok(KeyCode::Enter),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "tab" => Ok(KeyCode::Tab),
        "backtab" | "shift-tab" => Ok(KeyCode::BackTab),
        "pageup" | "page_up" => Ok(KeyCode::PageUp),
        "pagedown" | "page_down" => Ok(KeyCode::PageDown),
        "space" => Ok(KeyCode::Char(' ')),
        _ if spec.chars().count() == 1 => Ok(KeyCode::Char(spec.chars().next().unwrap())),
        _ => Err(format!("无法识别的按键「{spec}」")),
    }
}

pub fn key_name(code: &KeyCode) -> String {
    match code {
        KeyCode::Up => "↑".into(),
        KeyCode::Down => "↓".into(),
        KeyCode::Left => "←".into(),
        KeyCode::Right => "→".into(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::BackTab => "Shift-Tab".into(),
        KeyCode::PageUp => "PgUp".into(),
        KeyCode::PageDown => "PgDn".into(),
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        _ => "?".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyCode::*;

    #[test]
    fn absent_fields_keep_defaults_and_present_ones_replace() {
        let maps =
            Keymaps::from_toml("[keys]\nfocus_project = [\"g\"]\nquit = [\"Q\", \"esc\"]").unwrap();
        assert_eq!(maps.focus_project, vec![Char('g')]);
        assert_eq!(maps.quit, vec![Char('Q'), Esc]);
        assert_eq!(maps.flag, vec![Char('f')]);
        assert_eq!(maps.delete, vec![Char('D')]);
        assert_eq!(maps.edit, vec![Char('e')]);
        assert_eq!(maps.create, vec![Char('n')]);
        assert_eq!(maps.tag, vec![Char('t')]);
        assert_eq!(maps.down, vec![Char('j'), Down]);
        assert_eq!(maps.capture, vec![Char('a'), Char('+')]);
    }

    #[test]
    fn keys_are_case_sensitive() {
        let maps = Keymaps::default();
        assert_eq!(maps.action(Char('f')), Some(Action::Flag));
        assert_eq!(maps.action(Char('F')), Some(Action::FocusProject));
        assert_eq!(maps.action(Esc), Some(Action::Quit));
    }

    #[test]
    fn unknown_key_and_bad_toml_report_field_level_errors() {
        let error = Keymaps::from_toml("[keys]\ndown = [\"nope\"]").unwrap_err();
        assert!(error.contains("keys.down"), "{error}");
        assert!(Keymaps::from_toml("[keys\ndown = 3").is_err());
        let error = Keymaps::from_toml("[keys]\nflga = [\"x\"]").unwrap_err();
        assert!(
            error.contains("flga") || error.contains("unknown"),
            "{error}"
        );
    }

    #[test]
    fn empty_list_unbinds_the_action() {
        let maps = Keymaps::from_toml("[keys]\ndown = []").unwrap();
        assert!(maps.down.is_empty());
        assert_eq!(maps.action(Char('j')), None);
        assert_eq!(maps.action(Down), None);
        assert_eq!(maps.action(Char('k')), Some(Action::Up));
    }

    #[test]
    fn language_option_parses_and_defaults_to_chinese() {
        let config = Config::from_toml("language = \"en\"\n[keys]\nflag = [\"x\"]").unwrap();
        assert_eq!(config.language, Lang::En);
        assert_eq!(config.keys.flag, vec![Char('x')]);
        assert_eq!(Config::from_toml("").unwrap().language, Lang::Zh);
        let error = Config::from_toml("language = \"fr\"").unwrap_err();
        assert!(error.contains("language"), "{error}");
    }
}
