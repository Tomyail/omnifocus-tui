use crate::backend::Task;

/// 界面语言。默认中文；未配置时按系统 LANG 环境检测（en* 为英文）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    Zh,
    En,
}

impl Lang {
    pub fn from_config(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "zh" | "chinese" | "中文" => Ok(Lang::Zh),
            "en" | "english" => Ok(Lang::En),
            _ => Err(format!(
                "无法识别的语言「{value}」，可选值：zh、en (unknown language \"{value}\"; expected zh or en)"
            )),
        }
    }

    /// 依据 LC_ALL / LC_MESSAGES / LANG 的值（形如 en_US.UTF-8）猜测语言。
    pub fn detect(env_value: Option<&str>) -> Self {
        if env_value.is_some_and(|value| value.to_ascii_lowercase().starts_with("en")) {
            Lang::En
        } else {
            Lang::Zh
        }
    }

    // —— 侧栏与视图 ——
    pub fn all_tasks(self) -> &'static str {
        match self {
            Lang::Zh => "全部任务",
            Lang::En => "All Tasks",
        }
    }
    pub fn inbox(self) -> &'static str {
        match self {
            Lang::Zh => "收件箱",
            Lang::En => "Inbox",
        }
    }
    pub fn flagged_view(self) -> &'static str {
        match self {
            Lang::Zh => "已标记",
            Lang::En => "Flagged",
        }
    }
    pub fn on_hold(self) -> &'static str {
        match self {
            Lang::Zh => "暂停",
            Lang::En => "on hold",
        }
    }
    pub fn projects_views_title(self) -> &'static str {
        match self {
            Lang::Zh => " 项目 / 视图 ",
            Lang::En => " Projects / Views ",
        }
    }
    pub fn tasks_title(self) -> &'static str {
        match self {
            Lang::Zh => "任务",
            Lang::En => "Tasks",
        }
    }

    // —— 任务行与详情 ——
    pub fn task_row_meta(self, task: &Task) -> String {
        let mut meta = task.project.clone().unwrap_or_else(|| self.inbox().into());
        if !task.tags.is_empty() {
            meta.push_str(&format!(" · #{}", task.tags.join(" #")));
        }
        if let Some(due) = &task.due {
            meta.push_str(&format!(" · {} {}", self.due_label(), date(due)));
        }
        meta
    }
    pub fn task_detail(self, task: &Task) -> String {
        format!(
            "{}\n\n{}  {}\n{}  {}\n{}  {}\n{}  {}\n{}  {}\n{}  {}\nID    {}\n\n{}",
            task.name,
            self.project_label(),
            task.project.as_deref().unwrap_or(self.inbox()),
            self.tags_label(),
            task.tags.join(" · "),
            self.due_label(),
            task.due.as_deref().map(date).unwrap_or_else(|| "—".into()),
            self.defer_label(),
            task.defer.as_deref().map(date).unwrap_or_else(|| "—".into()),
            self.planned_label(),
            task.planned.as_deref().map(date).unwrap_or_else(|| "—".into()),
            self.estimate_label(),
            task.estimated_minutes.map(|m| self.minutes(m)).unwrap_or_else(|| "—".into()),
            task.id,
            task.note.as_deref().unwrap_or(self.no_note()),
        )
    }
    pub fn project_label(self) -> &'static str {
        match self {
            Lang::Zh => "项目",
            Lang::En => "Project",
        }
    }
    pub fn tags_label(self) -> &'static str {
        match self {
            Lang::Zh => "标签",
            Lang::En => "Tags",
        }
    }
    pub fn due_label(self) -> &'static str {
        match self {
            Lang::Zh => "截止",
            Lang::En => "Due",
        }
    }
    pub fn defer_label(self) -> &'static str {
        match self {
            Lang::Zh => "推迟",
            Lang::En => "Defer",
        }
    }
    pub fn planned_label(self) -> &'static str {
        match self {
            Lang::Zh => "计划",
            Lang::En => "Planned",
        }
    }
    pub fn estimate_label(self) -> &'static str {
        match self {
            Lang::Zh => "预估",
            Lang::En => "Est.",
        }
    }
    pub fn minutes(self, value: u32) -> String {
        match self {
            Lang::Zh => format!("{value} 分钟"),
            Lang::En => format!("{value} min"),
        }
    }
    pub fn no_note(self) -> &'static str {
        match self {
            Lang::Zh => "暂无备注",
            Lang::En => "No note",
        }
    }
    pub fn select_task(self) -> &'static str {
        match self {
            Lang::Zh => "选择一个任务",
            Lang::En => "Select a task",
        }
    }
    pub fn detail_title(self) -> &'static str {
        match self {
            Lang::Zh => " 详情 · PgUp/PgDn 滚动 ",
            Lang::En => " Details · PgUp/PgDn Scroll ",
        }
    }
    pub fn no_matches(self) -> &'static str {
        match self {
            Lang::Zh => "没有匹配的任务",
            Lang::En => "No matching tasks",
        }
    }
    pub fn loading(self) -> &'static str {
        match self {
            Lang::Zh => "正在读取…",
            Lang::En => "Loading…",
        }
    }
    pub fn too_small(self) -> &'static str {
        match self {
            Lang::Zh => "窗口太小，请扩大到至少 35×10",
            Lang::En => "Window too small; resize to at least 35×10",
        }
    }
    pub fn task_count(self, count: usize) -> String {
        match self {
            Lang::Zh => format!("{count} 个任务"),
            Lang::En => format!("{count} tasks"),
        }
    }
    pub fn syncing(self) -> &'static str {
        match self {
            Lang::Zh => "· 同步中",
            Lang::En => "· syncing",
        }
    }
    pub fn search_prefix(self) -> &'static str {
        match self {
            Lang::Zh => "搜索：",
            Lang::En => "Search: ",
        }
    }

    // —— 状态消息 ——
    pub fn reading(self) -> &'static str {
        match self {
            Lang::Zh => "正在读取 OmniFocus…",
            Lang::En => "Reading OmniFocus…",
        }
    }
    pub fn writing(self) -> &'static str {
        match self {
            Lang::Zh => "正在写入 OmniFocus…",
            Lang::En => "Writing to OmniFocus…",
        }
    }
    pub fn synced(self, projects: usize, tasks: usize) -> String {
        match self {
            Lang::Zh => format!("已同步 · {projects} 个项目 · {tasks} 个任务"),
            Lang::En => format!("Synced · {projects} projects · {tasks} tasks"),
        }
    }
    pub fn captured(self) -> &'static str {
        match self {
            Lang::Zh => "已收集到收件箱",
            Lang::En => "Captured to Inbox",
        }
    }
    pub fn completed(self) -> &'static str {
        match self {
            Lang::Zh => "任务已完成",
            Lang::En => "Task completed",
        }
    }
    pub fn deleted(self) -> &'static str {
        match self {
            Lang::Zh => "任务已删除",
            Lang::En => "Task dropped",
        }
    }
    pub fn updated(self) -> &'static str {
        match self {
            Lang::Zh => "任务已更新",
            Lang::En => "Task updated",
        }
    }
    pub fn created(self) -> &'static str {
        match self {
            Lang::Zh => "任务已创建",
            Lang::En => "Task created",
        }
    }
    pub fn flag_set(self, flagged: bool) -> &'static str {
        match (self, flagged) {
            (Lang::Zh, true) => "已添加旗标",
            (Lang::Zh, false) => "已移除旗标",
            (Lang::En, true) => "Flagged",
            (Lang::En, false) => "Unflagged",
        }
    }
    pub fn copied(self) -> &'static str {
        match self {
            Lang::Zh => "已复制任务与 ID，可粘贴给 agent",
            Lang::En => "Copied task and ID for your agent",
        }
    }
    pub fn no_task_for_focus(self) -> String {
        match self {
            Lang::Zh => "请先选中一个任务，再按 F 聚焦它的项目".into(),
            Lang::En => "Select a task first, then press F to focus its project".into(),
        }
    }
    pub fn project_not_found(self, name: &str) -> String {
        match self {
            Lang::Zh => format!("未找到项目「{name}」，按 r 刷新后重试"),
            Lang::En => format!("Project \"{name}\" not found; press r to refresh"),
        }
    }
    pub fn focused(self, name: &str) -> String {
        match self {
            Lang::Zh => {
                format!("已聚焦「{name}」· 侧栏只显示该项目，再按 F 或 Esc 取消")
            }
            Lang::En => format!(
                "Focused \"{name}\" · sidebar shows only this project; F or Esc to cancel"
            ),
        }
    }
    pub fn focus_cleared(self) -> &'static str {
        match self {
            Lang::Zh => "已取消聚焦，侧栏显示全部项目",
            Lang::En => "Focus cleared; showing all projects",
        }
    }
    pub fn inbox_no_project(self) -> &'static str {
        match self {
            Lang::Zh => "收件箱任务没有所属项目",
            Lang::En => "Inbox task has no project",
        }
    }
    pub fn name_empty(self) -> &'static str {
        match self {
            Lang::Zh => "任务名称不能为空",
            Lang::En => "Task name cannot be empty",
        }
    }
    pub fn cannot_clear_tags(self) -> &'static str {
        match self {
            Lang::Zh => "暂不支持清空全部标签（CLI 限制），请至少保留一个",
            Lang::En => "Cannot clear all tags (CLI limit); keep at least one",
        }
    }

    // —— 表单与对话框 ——
    pub fn field_name(self) -> &'static str {
        match self {
            Lang::Zh => "名称",
            Lang::En => "Name",
        }
    }
    pub fn field_note(self) -> &'static str {
        match self {
            Lang::Zh => "备注",
            Lang::En => "Note",
        }
    }
    pub fn field_tags(self) -> &'static str {
        match self {
            Lang::Zh => "标签",
            Lang::En => "Tags",
        }
    }
    pub fn keys_fields_hint(self) -> &'static str {
        match self {
            Lang::Zh => "Tab 切换字段 · Enter 保存 · Esc 取消",
            Lang::En => "Tab Fields · Enter Save · Esc Cancel",
        }
    }
    pub fn keys_suggest_hint(self) -> &'static str {
        match self {
            Lang::Zh => "↑↓ 选择 · Tab 补全 · Enter 保存 · Esc 取消",
            Lang::En => "↑↓ Pick · Tab Complete · Enter Save · Esc Cancel",
        }
    }
    pub fn suggest_label(self) -> &'static str {
        match self {
            Lang::Zh => "联想",
            Lang::En => "Suggest",
        }
    }
    pub fn capture_title(self) -> &'static str {
        match self {
            Lang::Zh => " 快速收集 → OmniFocus 收件箱 ",
            Lang::En => " Quick Capture → OmniFocus Inbox ",
        }
    }
    pub fn capture_hint(self) -> &'static str {
        match self {
            Lang::Zh => "Enter 保存 · Esc 取消",
            Lang::En => "Enter Save · Esc Cancel",
        }
    }
    pub fn search_title(self) -> &'static str {
        match self {
            Lang::Zh => " 搜索当前视图 ",
            Lang::En => " Search Current View ",
        }
    }
    pub fn search_hint(self) -> &'static str {
        match self {
            Lang::Zh => "标题、项目、标签、备注 · Enter 应用 · Esc 清除",
            Lang::En => "Title, project, tags, note · Enter Apply · Esc Clear",
        }
    }
    pub fn edit_title(self) -> &'static str {
        match self {
            Lang::Zh => " 编辑任务 ",
            Lang::En => " Edit Task ",
        }
    }
    pub fn new_task_title(self, target: &str) -> String {
        match self {
            Lang::Zh => format!(" 新建任务 · {target} "),
            Lang::En => format!(" New Task · {target} "),
        }
    }
    pub fn complete_title(self) -> &'static str {
        match self {
            Lang::Zh => " 完成任务 ",
            Lang::En => " Complete Task ",
        }
    }
    pub fn complete_body(self, name: &str) -> String {
        match self {
            Lang::Zh => format!(
                "{name}\n\n完成父任务会影响子任务；重复任务将按原规则生成下次。\ny / Enter 完成 · 其他键取消"
            ),
            Lang::En => format!(
                "{name}\n\nCompleting a parent affects its children; repeats follow OmniFocus rules.\ny / Enter Complete · any other key cancels"
            ),
        }
    }
    pub fn delete_title(self) -> &'static str {
        match self {
            Lang::Zh => " 删除任务 ",
            Lang::En => " Delete Task ",
        }
    }
    pub fn delete_body(self, name: &str) -> String {
        match self {
            Lang::Zh => format!(
                "{name}\n\n删除会把任务标记为已放弃并从所有视图消失，可在 OmniFocus 中恢复。\ny / Enter 删除 · 其他键取消"
            ),
            Lang::En => format!(
                "{name}\n\nDeleting drops the task from every view; it can be restored in OmniFocus.\ny / Enter Delete · any other key cancels"
            ),
        }
    }

    // —— 帮助栏 ——
    pub fn h_panes(self) -> &'static str {
        match self {
            Lang::Zh => "切换",
            Lang::En => "Panes",
        }
    }
    pub fn h_move(self) -> &'static str {
        match self {
            Lang::Zh => "移动",
            Lang::En => "Move",
        }
    }
    pub fn h_detail(self) -> &'static str {
        match self {
            Lang::Zh => "详情",
            Lang::En => "Detail",
        }
    }
    pub fn h_search(self) -> &'static str {
        match self {
            Lang::Zh => "搜索",
            Lang::En => "Search",
        }
    }
    pub fn h_capture(self) -> &'static str {
        match self {
            Lang::Zh => "收集",
            Lang::En => "Capture",
        }
    }
    pub fn h_new(self) -> &'static str {
        match self {
            Lang::Zh => "新建",
            Lang::En => "New",
        }
    }
    pub fn h_edit(self) -> &'static str {
        match self {
            Lang::Zh => "编辑",
            Lang::En => "Edit",
        }
    }
    pub fn h_tags(self) -> &'static str {
        match self {
            Lang::Zh => "标签",
            Lang::En => "Tags",
        }
    }
    pub fn h_complete(self) -> &'static str {
        match self {
            Lang::Zh => "完成",
            Lang::En => "Done",
        }
    }
    pub fn h_delete(self) -> &'static str {
        match self {
            Lang::Zh => "删除",
            Lang::En => "Drop",
        }
    }
    pub fn h_flag(self) -> &'static str {
        match self {
            Lang::Zh => "旗标",
            Lang::En => "Flag",
        }
    }
    pub fn h_focus(self) -> &'static str {
        match self {
            Lang::Zh => "聚焦",
            Lang::En => "Focus",
        }
    }
    pub fn h_copy(self) -> &'static str {
        match self {
            Lang::Zh => "复制",
            Lang::En => "Copy",
        }
    }
    pub fn h_open(self) -> &'static str {
        match self {
            Lang::Zh => "打开",
            Lang::En => "Open",
        }
    }
    pub fn h_refresh(self) -> &'static str {
        match self {
            Lang::Zh => "刷新",
            Lang::En => "Refresh",
        }
    }
    pub fn h_sidebar(self) -> &'static str {
        match self {
            Lang::Zh => "项目栏",
            Lang::En => "Sidebar",
        }
    }
    pub fn h_quit(self) -> &'static str {
        match self {
            Lang::Zh => "退出",
            Lang::En => "Quit",
        }
    }

    // —— 命令行输出 ——
    pub fn help_text(self) -> String {
        match self {
            Lang::Zh => "omnifocus-tui — 独立的 OmniFocus 终端界面\n\n用法：omnifocus-tui [--capture | --check | --version]\n\n  --capture  快速收集到 Inbox，保存后退出\n  --check    只读验证 of 连接并显示项目与任务数量\n\n需要 macOS、OmniFocus、of CLI。可用 OMNIFOCUS_CLI 指定 of 路径。\n任意终端直接运行；可选 Herdr 集成见 integrations/herdr/。".into(),
            Lang::En => "omnifocus-tui — a standalone terminal UI for OmniFocus\n\nUsage: omnifocus-tui [--capture | --check | --version]\n\n  --capture  quick capture to Inbox, exits after saving\n  --check    read-only check of the of connection with counts\n\nRequires macOS, OmniFocus and the of CLI. OMNIFOCUS_CLI overrides the of path.\nRuns in any terminal; optional Herdr integration lives in integrations/herdr/.".into(),
        }
    }
    pub fn unknown_arg(self) -> &'static str {
        match self {
            Lang::Zh => "未知参数，请运行 omnifocus-tui --help",
            Lang::En => "Unknown argument; run omnifocus-tui --help",
        }
    }
    pub fn need_tty(self) -> &'static str {
        match self {
            Lang::Zh => "请在交互式终端中运行；只读检查可使用 --check",
            Lang::En => "Run inside an interactive terminal; use --check for a read-only check",
        }
    }
    pub fn check_ok(self, projects: usize, tasks: usize) -> String {
        match self {
            Lang::Zh => format!(
                "OmniFocus 连接成功：{projects} 个项目，{tasks} 个任务（已排除项目根任务）"
            ),
            Lang::En => format!(
                "OmniFocus connected: {projects} projects, {tasks} tasks (project roots excluded)"
            ),
        }
    }
    pub fn check_project(self, name: &str, count: usize) -> String {
        match self {
            Lang::Zh => format!("  {name} · {count} 个任务"),
            Lang::En => format!("  {name} · {count} tasks"),
        }
    }
}

fn date(value: &str) -> String {
    // 与 app::date 相同：保留接口返回的 UTC 时间戳。
    value
        .replace('T', " ")
        .replace(".000Z", " UTC")
        .replace('Z', " UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_language_values_and_detects_env() {
        assert_eq!(Lang::from_config("zh").unwrap(), Lang::Zh);
        assert_eq!(Lang::from_config("EN").unwrap(), Lang::En);
        assert!(Lang::from_config("fr").is_err());
        assert_eq!(Lang::detect(Some("en_US.UTF-8")), Lang::En);
        assert_eq!(Lang::detect(Some("zh_CN.UTF-8")), Lang::Zh);
        assert_eq!(Lang::detect(None), Lang::Zh);
    }
}
