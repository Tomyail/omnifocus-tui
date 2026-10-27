use crate::backend::{Backend, Project, Snapshot, Task};
use crate::config::{Action, Config, Keymaps};
use crate::i18n::Lang;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    sync::mpsc::{self, Receiver},
    thread,
    time::Instant,
};

const ACCENT: Color = Color::Rgb(180, 160, 240);
const MUTED: Color = Color::Rgb(145, 151, 173);
const BG: Color = Color::Rgb(27, 29, 40);
const SELECTED: Color = Color::Rgb(57, 57, 80);

#[derive(Clone, Debug, PartialEq)]
enum View {
    Inbox,
    Flagged,
    Project(String),
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Search,
    Capture,
    Edit,
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EditField {
    Name,
    Note,
    Tags,
}

struct EditState {
    /// None 表示新建任务；Some 为编辑既有任务的 id。
    id: Option<String>,
    name: String,
    note: String,
    /// 标签字段：空格分隔的完整标签列表。
    tags: String,
    /// 打开表单时的原有标签，用于判断是否需要写回。
    original_tags: Vec<String>,
    field: EditField,
    /// 标签联想中选中的候选下标（按候选数取模）。
    tag_index: usize,
    /// 新建任务的目标项目名；None 表示收件箱。编辑时忽略。
    project: Option<String>,
}

impl EditState {
    fn active_mut(&mut self) -> &mut String {
        match self.field {
            EditField::Name => &mut self.name,
            EditField::Note => &mut self.note,
            EditField::Tags => &mut self.tags,
        }
    }
}

/// CLI 的 --tag 至少要一个值，无法表达「清空全部标签」。
fn tries_to_clear_all_tags(edit: &EditState) -> bool {
    edit.tags.split_whitespace().next().is_none() && !edit.original_tags.is_empty()
}

enum Operation {
    Refresh,
    Capture(String),
    Create {
        name: String,
        note: String,
        project: Option<String>,
        tags: Vec<String>,
    },
    Complete(String),
    Drop(String),
    Update {
        id: String,
        name: String,
        note: String,
        /// Some 表示写回整组标签（替换语义）；None 表示不修改。
        tags: Option<Vec<String>>,
    },
    Flag(String, bool),
}

#[derive(Clone, Copy, PartialEq)]
enum ConfirmAction {
    Complete,
    Delete,
}

struct Outcome {
    snapshot: Result<Snapshot, String>,
    message: Option<String>,
    written: bool,
    /// 收集到收件箱的任务：刷新后切到收件箱并选中。
    captured_id: Option<String>,
    /// 新建的任务：保持当前视图，刷新后选中（不可见时回退）。
    created_id: Option<String>,
}

pub struct App {
    backend: Backend,
    data: Snapshot,
    lang: Lang,
    keys: Keymaps,
    views: Vec<(View, String)>,
    view: View,
    focused_project: Option<String>,
    rows: Vec<usize>,
    sidebar: ListState,
    tasks: ListState,
    sidebar_focus: bool,
    show_sidebar: bool,
    show_detail: bool,
    detail_scroll: u16,
    mode: Mode,
    input: String,
    query: String,
    edit: Option<EditState>,
    confirm: Option<(Task, ConfirmAction)>,
    status: String,
    error: bool,
    job: Option<Receiver<Outcome>>,
    /// 后台拉取的完整标签库，用于标签联想；为空时退回任务快照中出现过的标签。
    known_tags: Vec<String>,
    tag_job: Option<Receiver<Vec<String>>>,
    update_job: Option<Receiver<Option<String>>>,
    update_notice: Option<String>,
    refreshed: Instant,
    capture_only: bool,
    pub quit: bool,
    sidebar_area: Rect,
    tasks_area: Rect,
}

impl App {
    pub fn new(capture_only: bool, config: Config) -> Self {
        let mut app = Self {
            backend: Backend::new(),
            data: Snapshot::default(),
            lang: config.language,
            keys: config.keys,
            views: vec![],
            view: View::Inbox,
            focused_project: None,
            rows: vec![],
            sidebar: ListState::default().with_selected(Some(0)),
            tasks: ListState::default(),
            sidebar_focus: true,
            show_sidebar: true,
            show_detail: false,
            detail_scroll: 0,
            mode: if capture_only {
                Mode::Capture
            } else {
                Mode::Normal
            },
            input: String::new(),
            query: String::new(),
            edit: None,
            confirm: None,
            status: String::new(),
            error: false,
            job: None,
            known_tags: vec![],
            tag_job: None,
            update_job: None,
            update_notice: None,
            refreshed: Instant::now(),
            capture_only,
            quit: false,
            sidebar_area: Rect::default(),
            tasks_area: Rect::default(),
        };
        app.rebuild(None);
        if !capture_only {
            app.start(Operation::Refresh);
            if config.auto_update {
                let (sender, receiver) = mpsc::channel();
                let lang = app.lang;
                std::thread::spawn(move || {
                    let _ = sender.send(crate::update::auto(lang));
                });
                app.update_job = Some(receiver);
            }
        }
        app
    }

    fn selected(&self) -> Option<&Task> {
        self.tasks
            .selected()
            .and_then(|i| self.rows.get(i))
            .and_then(|i| self.data.tasks.get(*i))
    }

    fn rebuild(&mut self, selected_id: Option<String>) {
        if let Some(id) = &self.focused_project
            && !self.data.projects.iter().any(|p| &p.id == id)
        {
            self.focused_project = None;
        }
        let focused = self.focused_project.clone();
        let query = self.query.to_lowercase();
        let count_in = |view: &View| -> usize {
            let project_name = view_project_name(&self.data.projects, view);
            self.data
                .tasks
                .iter()
                .filter(|t| task_in_view(t, view, project_name) && task_matches_query(t, &query))
                .count()
        };
        let mut views = vec![
            (View::Inbox, self.lang.inbox().into()),
            (View::Flagged, self.lang.flagged_view().into()),
        ];
        views.extend(
            self.data
                .projects
                .iter()
                .filter(|p| focused.as_deref().is_none_or(|id| p.id == id))
                .map(|p| {
                    let label = if p.status == "on hold" {
                        format!("{} · {}", p.name, self.lang.on_hold())
                    } else {
                        p.name.clone()
                    };
                    (View::Project(p.id.clone()), label)
                }),
        );
        // 搜索时在侧栏每个视图旁标注匹配数，避免逐个点进去查看
        if !query.is_empty() {
            views = views
                .into_iter()
                .map(|(view, label)| {
                    let count = count_in(&view);
                    (view, format!("{label} · {count}"))
                })
                .collect();
        }
        self.views = views;
        if !self.views.iter().any(|(v, _)| v == &self.view) {
            self.view = View::Inbox;
        }
        self.sidebar
            .select(self.views.iter().position(|(v, _)| v == &self.view));
        let project = view_project_name(&self.data.projects, &self.view);
        self.rows = self
            .data
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| task_in_view(t, &self.view, project) && task_matches_query(t, &query))
            .map(|(i, _)| i)
            .collect();
        let previous = self.tasks.selected().unwrap_or(0);
        let selection = selected_id
            .and_then(|id| self.rows.iter().position(|i| self.data.tasks[*i].id == id))
            .unwrap_or(previous.min(self.rows.len().saturating_sub(1)));
        self.tasks.select(if self.rows.is_empty() {
            None
        } else {
            Some(selection)
        });
    }

    fn start(&mut self, operation: Operation) {
        if self.job.is_some() {
            return;
        }
        let backend = self.backend.clone();
        let lang = self.lang;
        self.status = match &operation {
            Operation::Refresh => lang.reading(),
            _ => lang.writing(),
        }
        .into();
        self.error = false;
        let capture_only = self.capture_only;
        let (sender, receiver) = mpsc::channel();
        self.job = Some(receiver);
        thread::spawn(move || {
            // (提示消息, 刷新后要选中的任务 id, 是否切到收件箱视图)
            let result = match operation {
                Operation::Refresh => Ok((None, None, false)),
                Operation::Capture(name) => backend.capture(&name).map(|task| {
                    (
                        Some(lang.captured().into()),
                        task.get("id").and_then(|id| id.as_str()).map(str::to_owned),
                        true,
                    )
                }),
                Operation::Create {
                    name,
                    note,
                    project,
                    tags,
                } => backend
                    .create(&name, &note, project.as_deref(), &tags)
                    .map(|task| {
                        (
                            Some(lang.created().into()),
                            task.get("id").and_then(|id| id.as_str()).map(str::to_owned),
                            false,
                        )
                    }),
                Operation::Complete(id) => backend
                    .complete(&id)
                    .map(|_| (Some(lang.completed().into()), None, false)),
                Operation::Drop(id) => backend
                    .drop(&id)
                    .map(|_| (Some(lang.deleted().into()), None, false)),
                Operation::Update {
                    id,
                    name,
                    note,
                    tags,
                } => backend
                    .update(&id, &name, &note, tags.as_deref())
                    .map(|_| (Some(lang.updated().into()), None, false)),
                Operation::Flag(id, flagged) => backend
                    .flag(&id, flagged)
                    .map(|_| (Some(lang.flag_set(flagged).into()), None, false)),
            };
            let outcome = match result {
                Ok((message, select_id, to_inbox)) => {
                    let written = message.is_some();
                    let (captured_id, created_id) = if to_inbox {
                        (select_id, None)
                    } else {
                        (None, select_id)
                    };
                    Outcome {
                        snapshot: if capture_only {
                            Ok(Snapshot::default())
                        } else {
                            backend.snapshot()
                        },
                        message,
                        written,
                        captured_id,
                        created_id,
                    }
                }
                Err(error) => Outcome {
                    snapshot: Err(error),
                    message: None,
                    written: false,
                    captured_id: None,
                    created_id: None,
                },
            };
            let _ = sender.send(outcome);
        });
    }

    pub fn tick(&mut self) {
        if let Some(result) = self.update_job.as_ref().and_then(|job| job.try_recv().ok()) {
            self.update_job = None;
            self.update_notice = result.map(|version| self.lang.update_notice(&version));
        }
        if let Some(names) = self.tag_job.as_ref().and_then(|job| job.try_recv().ok()) {
            self.tag_job = None;
            self.known_tags = names;
        }
        if let Some(outcome) = self.job.as_ref().and_then(|job| job.try_recv().ok()) {
            self.job = None;
            self.refreshed = Instant::now();
            self.error = outcome.snapshot.is_err();
            match outcome.snapshot {
                Ok(snapshot) => {
                    let selected = if let Some(id) = outcome.captured_id {
                        self.view = View::Inbox;
                        self.query.clear();
                        self.sidebar_focus = false;
                        self.detail_scroll = 0;
                        Some(id)
                    } else if let Some(id) = outcome.created_id {
                        // 新建任务：留在当前视图并选中它；被搜索过滤掉时由 rebuild 回退
                        Some(id)
                    } else {
                        self.selected().map(|t| t.id.clone())
                    };
                    self.data = snapshot;
                    self.rebuild(selected);
                    self.status = outcome.message.unwrap_or_else(|| {
                        self.lang
                            .synced(self.data.projects.len(), self.data.tasks.len())
                    });
                }
                Err(error) => {
                    self.status = if outcome.written {
                        format!("{}；刷新失败：{error}", outcome.message.unwrap_or_default())
                    } else {
                        error
                    };
                }
            }
            if outcome.written {
                self.input.clear();
                self.mode = Mode::Normal;
                if self.capture_only {
                    self.quit = true;
                }
            }
        }
        if !self.capture_only
            && self.job.is_none()
            && self.mode == Mode::Normal
            && self.refreshed.elapsed().as_secs() >= 30
        {
            self.start(Operation::Refresh);
        }
    }

    fn move_selection(&mut self, delta: isize) {
        self.detail_scroll = 0;
        if self.sidebar_focus && self.show_sidebar {
            let next = self
                .sidebar
                .selected()
                .unwrap_or(0)
                .saturating_add_signed(delta)
                .min(self.views.len().saturating_sub(1));
            self.sidebar.select(Some(next));
            if let Some((view, _)) = self.views.get(next) {
                self.view = view.clone();
            }
            self.tasks.select(None);
            self.rebuild(None);
        } else if !self.rows.is_empty() {
            let next = self
                .tasks
                .selected()
                .unwrap_or(0)
                .saturating_add_signed(delta)
                .min(self.rows.len() - 1);
            self.tasks.select(Some(next));
        }
    }

    /// 打开编辑表单时后台刷新一次完整标签库，用于联想；失败时静默退回任务快照标签。
    fn start_tag_fetch(&mut self) {
        if self.tag_job.is_some() || self.capture_only {
            return;
        }
        let backend = self.backend.clone();
        let (sender, receiver) = mpsc::channel();
        self.tag_job = Some(receiver);
        thread::spawn(move || {
            let _ = sender.send(backend.tags().unwrap_or_default());
        });
    }

    fn tag_suggestions(&self) -> Vec<String> {
        self.edit
            .as_ref()
            .map(|edit| tag_suggestions_for(&self.known_tags, &self.data.tasks, &edit.tags))
            .unwrap_or_default()
    }

    fn open_editor(&mut self, field: EditField) {
        self.start_tag_fetch();
        if let Some(task) = self.selected() {
            self.edit = Some(EditState {
                id: Some(task.id.clone()),
                name: task.name.clone(),
                note: task.note.clone().unwrap_or_default(),
                tags: task.tags.join(" "),
                original_tags: task.tags.clone(),
                field,
                tag_index: 0,
                project: None,
            });
            self.mode = Mode::Edit;
            self.error = false;
        }
    }

    fn focus_selected_task_project(&mut self) {
        let Some((id, project)) = self
            .selected()
            .map(|task| (task.id.clone(), task.project.clone()))
        else {
            self.status = self.lang.no_task_for_focus();
            self.error = false;
            return;
        };
        // of 按 名称 归属任务与视图过滤，这里保持同一口径。
        let view = match project.as_deref() {
            Some(name) => match self.data.projects.iter().find(|p| p.name == name) {
                Some(project) => View::Project(project.id.clone()),
                None => {
                    self.status = self.lang.project_not_found(name);
                    self.error = true;
                    return;
                }
            },
            None => View::Inbox,
        };
        self.error = false;
        match &view {
            View::Project(project_id) => {
                self.focused_project = if self.focused_project.as_deref() == Some(project_id) {
                    self.status = self.lang.focus_cleared().into();
                    None
                } else {
                    self.status = self
                        .lang
                        .focused(project.clone().unwrap_or_default().as_str());
                    Some(project_id.clone())
                };
            }
            _ => {
                self.focused_project = None;
                self.status = self.lang.inbox_no_project().into();
            }
        }
        self.view = view;
        self.rebuild(Some(id));
        self.sidebar_focus = false;
        self.detail_scroll = 0;
    }

    pub fn event(&mut self, event: Event) {
        if let Event::Paste(text) = event {
            if matches!(self.mode, Mode::Search | Mode::Capture | Mode::Edit) {
                let multiline = self
                    .edit
                    .as_ref()
                    .is_some_and(|edit| edit.field == EditField::Note);
                let text: String = text
                    .chars()
                    // 备注字段保留换行；其余控件按单行处理
                    .filter(|c| !c.is_control() || (*c == '\n' && multiline))
                    .collect();
                if matches!(self.mode, Mode::Edit) {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.active_mut().push_str(&text);
                        edit.tag_index = 0;
                    }
                } else {
                    self.input.push_str(&text);
                }
                if self.mode == Mode::Search {
                    self.query = self.input.clone();
                    self.rebuild(None);
                }
            }
            return;
        }
        if let Event::Mouse(mouse) = event {
            if self.mode != Mode::Normal {
                return;
            }
            let pos = (mouse.column, mouse.row).into();
            match mouse.kind {
                MouseEventKind::ScrollDown => self.move_selection(1),
                MouseEventKind::ScrollUp => self.move_selection(-1),
                MouseEventKind::Down(MouseButton::Left)
                    if self.show_sidebar && self.sidebar_area.contains(pos) =>
                {
                    self.sidebar_focus = true;
                    let index = self.sidebar.offset()
                        + mouse.row.saturating_sub(self.sidebar_area.y + 1) as usize;
                    if index < self.views.len() {
                        self.view = self.views[index].0.clone();
                        self.tasks.select(None);
                        self.rebuild(None);
                    }
                }
                MouseEventKind::Down(MouseButton::Left) if self.tasks_area.contains(pos) => {
                    self.sidebar_focus = false;
                    let index = self.tasks.offset()
                        + mouse.row.saturating_sub(self.tasks_area.y + 1) as usize / 2;
                    if index < self.rows.len() {
                        self.tasks.select(Some(index));
                        self.detail_scroll = 0;
                    }
                }
                _ => {}
            }
            return;
        }
        let Event::Key(key) = event else {
            return;
        };
        if key.kind == KeyEventKind::Release {
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.job.is_none() {
                self.quit = true;
            }
            return;
        }
        if matches!(self.mode, Mode::Capture | Mode::Search | Mode::Edit) {
            match key.code {
                KeyCode::Esc if self.job.is_none() => {
                    if self.capture_only {
                        self.quit = true;
                    }
                    self.mode = Mode::Normal;
                    self.input.clear();
                    self.query.clear();
                    self.edit = None;
                    self.rebuild(None);
                }
                KeyCode::Enter if self.mode == Mode::Capture => {
                    if !self.input.trim().is_empty() {
                        self.start(Operation::Capture(self.input.clone()));
                    }
                }
                // 备注（最后一个字段）支持 Shift+Enter 换行；macOS 部分输入法会把
                // Shift+Enter 改报为 Option+Enter（ESC \r），因此两种修饰键都视为换行。
                // 名称与标签保持单行：带修饰的 Enter 既不换行也不提交。
                KeyCode::Enter
                    if self.mode == Mode::Edit
                        && key
                            .modifiers
                            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT)
                        && self
                            .edit
                            .as_ref()
                            .is_some_and(|edit| edit.field == EditField::Note) =>
                {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.note.push('\n');
                        edit.tag_index = 0;
                    }
                }
                // Ctrl+J（即 \n）：未启用 kitty 协议的终端里唯一稳定的换行键
                KeyCode::Char('j')
                    if self.mode == Mode::Edit
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                        && self
                            .edit
                            .as_ref()
                            .is_some_and(|edit| edit.field == EditField::Note) =>
                {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.note.push('\n');
                        edit.tag_index = 0;
                    }
                }
                KeyCode::Enter
                    if self.mode == Mode::Edit
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT) =>
                {
                    match self.edit.take() {
                        Some(edit)
                            if !edit.name.trim().is_empty() && !tries_to_clear_all_tags(&edit) =>
                        {
                            let name = edit.name.trim().to_owned();
                            let note = edit.note;
                            let tags: Vec<String> =
                                edit.tags.split_whitespace().map(str::to_owned).collect();
                            match edit.id {
                                Some(id) => {
                                    let tags = (tags != edit.original_tags).then_some(tags);
                                    self.start(Operation::Update {
                                        id,
                                        name,
                                        note,
                                        tags,
                                    });
                                }
                                None => {
                                    self.start(Operation::Create {
                                        name,
                                        note,
                                        project: edit.project,
                                        tags,
                                    });
                                }
                            }
                        }
                        Some(edit) => {
                            // 名称被清空或试图清空全部标签：回到编辑态并提示，不写入
                            self.edit = Some(edit);
                            if self.edit.as_ref().is_some_and(tries_to_clear_all_tags) {
                                self.status = self.lang.cannot_clear_tags().into();
                            } else {
                                self.status = self.lang.name_empty().into();
                            }
                            self.error = true;
                        }
                        None => {}
                    }
                }
                KeyCode::Tab if self.mode == Mode::Edit => {
                    // 标签字段有联想时 Tab 先补全选中项，否则在字段间循环
                    let in_tags = self
                        .edit
                        .as_ref()
                        .is_some_and(|edit| edit.field == EditField::Tags);
                    let suggestions = if in_tags {
                        self.tag_suggestions()
                    } else {
                        vec![]
                    };
                    if let Some(edit) = self.edit.as_mut()
                        && !suggestions.is_empty()
                    {
                        let picked = suggestions[edit.tag_index % suggestions.len()].clone();
                        let prefix = edit
                            .tags
                            .rsplit_once(' ')
                            .map(|(head, _)| format!("{head} "))
                            .unwrap_or_default();
                        edit.tags = format!("{prefix}{picked} ");
                        edit.tag_index = 0;
                    } else if let Some(edit) = self.edit.as_mut() {
                        edit.field = match edit.field {
                            EditField::Name => EditField::Tags,
                            EditField::Tags => EditField::Note,
                            EditField::Note => EditField::Name,
                        };
                    }
                }
                KeyCode::Down | KeyCode::Up if self.mode == Mode::Edit => {
                    // ↑↓ 在标签联想候选中循环选择
                    let in_tags = self
                        .edit
                        .as_ref()
                        .is_some_and(|edit| edit.field == EditField::Tags);
                    let len = if in_tags {
                        self.tag_suggestions().len()
                    } else {
                        0
                    };
                    if len > 0
                        && let Some(edit) = self.edit.as_mut()
                    {
                        edit.tag_index = if key.code == KeyCode::Down {
                            (edit.tag_index + 1) % len
                        } else {
                            (edit.tag_index + len - 1) % len
                        };
                    }
                }
                KeyCode::Enter if !matches!(self.mode, Mode::Edit) => {
                    self.mode = Mode::Normal;
                    self.sidebar_focus = false;
                }
                KeyCode::Backspace if self.job.is_none() => {
                    if matches!(self.mode, Mode::Edit) {
                        if let Some(edit) = self.edit.as_mut() {
                            edit.active_mut().pop();
                            edit.tag_index = 0;
                        }
                    } else {
                        self.input.pop();
                    }
                }
                KeyCode::Char(c)
                    if self.job.is_none()
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    if matches!(self.mode, Mode::Edit) {
                        if let Some(edit) = self.edit.as_mut() {
                            edit.active_mut().push(c);
                            edit.tag_index = 0;
                        }
                    } else {
                        self.input.push(c);
                    }
                }
                _ => {}
            }
            if self.mode == Mode::Search {
                self.query = self.input.clone();
                self.rebuild(None);
            }
            return;
        }
        if self.mode == Mode::Confirm {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    if let Some((task, action)) = self.confirm.take() {
                        let operation = match action {
                            ConfirmAction::Complete => Operation::Complete(task.id),
                            ConfirmAction::Delete => Operation::Drop(task.id),
                        };
                        self.start(operation);
                    }
                    self.mode = Mode::Normal;
                }
                _ => {
                    self.confirm = None;
                    self.mode = Mode::Normal;
                }
            }
            return;
        }
        let Some(action) = self.keys.action(key.code) else {
            return;
        };
        match action {
            Action::Quit if self.job.is_none() => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.rebuild(None);
                } else if self.show_detail {
                    self.show_detail = false;
                } else if self.focused_project.is_some() {
                    self.focused_project = None;
                    self.status = self.lang.focus_cleared().into();
                    self.error = false;
                    self.rebuild(None);
                } else {
                    self.quit = true;
                }
            }
            Action::Down => self.move_selection(1),
            Action::Up => self.move_selection(-1),
            Action::Sidebar => {
                self.show_sidebar = true;
                self.sidebar_focus = true;
            }
            Action::Tasks => self.sidebar_focus = false,
            Action::TogglePane => {
                self.sidebar_focus = !self.sidebar_focus;
                self.show_sidebar = true;
            }
            Action::ToggleSidebar => {
                self.show_sidebar = !self.show_sidebar;
                self.sidebar_focus = self.show_sidebar;
            }
            Action::ToggleDetail => {
                self.show_detail = !self.show_detail;
                self.detail_scroll = 0;
            }
            Action::DetailDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            Action::DetailUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            Action::Refresh => self.start(Operation::Refresh),
            Action::Search => {
                self.mode = Mode::Search;
                self.input = self.query.clone();
            }
            Action::Capture if self.job.is_none() => {
                self.mode = Mode::Capture;
                self.input.clear();
            }
            Action::Flag if !self.sidebar_focus && self.job.is_none() => {
                if let Some(task) = self.selected() {
                    self.start(Operation::Flag(task.id.clone(), !task.flagged));
                }
            }
            Action::Tag if !self.sidebar_focus && self.job.is_none() => {
                // 复用编辑表单，直接定位到标签字段
                self.open_editor(EditField::Tags);
            }
            Action::FocusProject if self.job.is_none() => self.focus_selected_task_project(),
            Action::Complete if !self.sidebar_focus && self.job.is_none() => {
                self.confirm = self
                    .selected()
                    .cloned()
                    .map(|task| (task, ConfirmAction::Complete));
                if self.confirm.is_some() {
                    self.mode = Mode::Confirm;
                }
            }
            Action::Delete if !self.sidebar_focus && self.job.is_none() => {
                self.confirm = self
                    .selected()
                    .cloned()
                    .map(|task| (task, ConfirmAction::Delete));
                if self.confirm.is_some() {
                    self.mode = Mode::Confirm;
                }
            }
            Action::Edit if !self.sidebar_focus && self.job.is_none() => {
                self.open_editor(EditField::Name);
            }
            Action::Create if self.job.is_none() => {
                // 在当前项目视图下新建到该项目，否则进收件箱；侧栏聚焦时同样生效。
                let project = match &self.view {
                    View::Project(id) => self
                        .data
                        .projects
                        .iter()
                        .find(|p| &p.id == id)
                        .map(|p| p.name.clone()),
                    _ => None,
                };
                self.start_tag_fetch();
                self.edit = Some(EditState {
                    id: None,
                    name: String::new(),
                    note: String::new(),
                    tags: String::new(),
                    original_tags: vec![],
                    field: EditField::Name,
                    tag_index: 0,
                    project,
                });
                self.mode = Mode::Edit;
                self.error = false;
            }
            Action::Open if !self.sidebar_focus => {
                if let Some(task) = self.selected()
                    && let Err(error) = std::process::Command::new("open")
                        .arg(format!("omnifocus:///task/{}", task.id))
                        .spawn()
                {
                    self.status = error.to_string();
                    self.error = true;
                }
            }
            Action::Copy if !self.sidebar_focus => {
                if let Some(task) = self.selected() {
                    let reference = format!(
                        "{}\nOmniFocus ID: {}\nomnifocus:///task/{}\n{}",
                        task.name,
                        task.id,
                        task.id,
                        task.note.as_deref().unwrap_or("")
                    );
                    let result = copy(&reference, self.lang);
                    self.status = result.err().unwrap_or_else(|| self.lang.copied().into());
                }
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(
            Block::default().style(Style::default().bg(BG).fg(Color::Rgb(218, 221, 235))),
            area,
        );
        if area.width < 35 || area.height < 10 {
            frame.render_widget(Paragraph::new(self.lang.too_small()), area);
            return;
        }
        let [header, body, status, footer] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .areas(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " ◉ OmniFocus ",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        " {}  {}",
                        self.lang.task_count(self.rows.len()),
                        if self.job.is_some() {
                            self.lang.syncing()
                        } else {
                            ""
                        }
                    ),
                    Style::default().fg(MUTED),
                ),
            ])),
            header,
        );
        let mut content = body;
        self.sidebar_area = Rect::default();
        if self.show_sidebar {
            let sidebar_width = if area.width < 75 { 18 } else { 26 };
            let [side, rest] =
                Layout::horizontal([Constraint::Length(sidebar_width), Constraint::Min(15)])
                    .areas(body);
            self.sidebar_area = side;
            content = rest;
            let items: Vec<ListItem> = self
                .views
                .iter()
                .map(|(view, label)| {
                    let folder = match view {
                        View::Project(id) => self
                            .data
                            .projects
                            .iter()
                            .find(|p| &p.id == id)
                            .and_then(|p| p.folder.as_deref()),
                        _ => None,
                    };
                    ListItem::new(label.clone()).style(if folder.is_some() {
                        Style::default()
                    } else {
                        Style::default().fg(ACCENT)
                    })
                })
                .collect();
            frame.render_stateful_widget(
                List::new(items)
                    .block(panel(self.lang.projects_views_title(), self.sidebar_focus))
                    .highlight_style(highlight())
                    .highlight_symbol("› "),
                side,
                &mut self.sidebar,
            );
        }
        let mut task_area = content;
        let mut detail_area = None;
        if self.show_detail && content.width >= 85 {
            let [tasks, detail] =
                Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .areas(content);
            task_area = tasks;
            detail_area = Some(detail);
        } else if self.show_detail && content.height >= 16 {
            let [tasks, detail] =
                Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                    .areas(content);
            task_area = tasks;
            detail_area = Some(detail);
        }
        self.tasks_area = task_area;
        let title = self
            .views
            .iter()
            .find(|(v, _)| v == &self.view)
            .map(|(_, label)| label.as_str())
            .unwrap_or(self.lang.tasks_title());
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|i| {
                let task = &self.data.tasks[*i];
                ListItem::new(Text::from(vec![
                    Line::from(format!(
                        "{} {}",
                        if task.flagged { "⚑" } else { "○" },
                        task.name
                    )),
                    Line::styled(
                        format!("  {}", self.lang.task_row_meta(task)),
                        Style::default().fg(MUTED),
                    ),
                ]))
            })
            .collect();
        frame.render_stateful_widget(
            List::new(items)
                .block(panel(&format!(" {title} "), !self.sidebar_focus))
                .highlight_style(highlight())
                .highlight_symbol("▸ "),
            task_area,
            &mut self.tasks,
        );
        if self.rows.is_empty() {
            frame.render_widget(
                Paragraph::new(if self.job.is_some() {
                    self.lang.loading()
                } else {
                    self.lang.no_matches()
                })
                .style(Style::default().fg(MUTED)),
                task_area.inner(ratatui::layout::Margin::new(2, 2)),
            );
        }
        if let Some(detail) = detail_area {
            let text = self
                .selected()
                .map(|t| self.lang.task_detail(t))
                .unwrap_or_else(|| self.lang.select_task().into());
            frame.render_widget(
                Paragraph::new(text)
                    .block(panel(self.lang.detail_title(), false))
                    .wrap(Wrap { trim: false })
                    .scroll((self.detail_scroll, 0)),
                detail,
            );
        }
        let mut status_text = if self.query.is_empty() {
            self.status.clone()
        } else {
            format!(
                "{}{} · {}",
                self.lang.search_prefix(),
                self.query,
                self.status
            )
        };
        if let Some(notice) = &self.update_notice {
            status_text = format!("{status_text} · {notice}");
        }
        frame.render_widget(
            Paragraph::new(status_text)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(if self.error { Color::LightRed } else { MUTED })),
            status,
        );
        let keys = &self.keys;
        let lang = self.lang;
        let footer_text = format!(
            " {} {}  {}/{} {}  {} {}  {} {}  {} {}  {} {}  {} {}  {} {}\n {} {}  {} {}  {} {}  {} {}  {} {}  {} {}  {} {}  {} {}  {} {}",
            keys.first(Action::TogglePane),
            lang.h_panes(),
            keys.first(Action::Down),
            keys.first(Action::Up),
            lang.h_move(),
            keys.first(Action::ToggleDetail),
            lang.h_detail(),
            keys.first(Action::Search),
            lang.h_search(),
            keys.first(Action::Capture),
            lang.h_capture(),
            keys.first(Action::Create),
            lang.h_new(),
            keys.first(Action::Edit),
            lang.h_edit(),
            keys.first(Action::Tag),
            lang.h_tags(),
            keys.first(Action::Complete),
            lang.h_complete(),
            keys.first(Action::Delete),
            lang.h_delete(),
            keys.first(Action::Flag),
            lang.h_flag(),
            keys.first(Action::FocusProject),
            lang.h_focus(),
            keys.first(Action::Copy),
            lang.h_copy(),
            keys.first(Action::Open),
            lang.h_open(),
            keys.first(Action::Refresh),
            lang.h_refresh(),
            keys.first(Action::ToggleSidebar),
            lang.h_sidebar(),
            keys.first(Action::Quit),
            lang.h_quit(),
        );
        frame.render_widget(
            Paragraph::new(footer_text).style(Style::default().fg(MUTED)),
            footer,
        );
        if matches!(
            self.mode,
            Mode::Capture | Mode::Search | Mode::Confirm | Mode::Edit
        ) {
            // 联想行只在编辑表单的标签字段有候选时出现，多占一行高度；备注的额外行同理
            let note_extra = if self.mode == Mode::Edit {
                self.edit
                    .as_ref()
                    .map(|edit| edit.note.split('\n').count().saturating_sub(1))
                    .unwrap_or(0)
                    .min(8)
            } else {
                0
            };
            let tag_suggestions = if self.mode == Mode::Edit
                && self
                    .edit
                    .as_ref()
                    .is_some_and(|edit| edit.field == EditField::Tags)
            {
                let suggestions = self.tag_suggestions();
                (!suggestions.is_empty()).then(|| {
                    let index = self
                        .edit
                        .as_ref()
                        .map_or(0, |edit| edit.tag_index % suggestions.len());
                    (suggestions, index)
                })
            } else {
                None
            };
            let [_, center, _] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(8 + u16::from(tag_suggestions.is_some()) + note_extra as u16),
                Constraint::Fill(1),
            ])
            .areas(area);
            let [_, dialog, _] = Layout::horizontal([
                Constraint::Fill(1),
                Constraint::Length(area.width.saturating_sub(4).min(75)),
                Constraint::Fill(1),
            ])
            .areas(center);
            frame.render_widget(Clear, dialog);
            let (title, text) = match self.mode {
                Mode::Capture => (
                    self.lang.capture_title().into(),
                    format!(
                        "{}▏\n\n{}\n{}",
                        self.input,
                        self.lang.capture_hint(),
                        if self.error || self.job.is_some() {
                            &self.status
                        } else {
                            ""
                        }
                    ),
                ),
                Mode::Search => (
                    self.lang.search_title().into(),
                    format!("{}▏\n\n{}", self.input, self.lang.search_hint()),
                ),
                Mode::Edit => {
                    let (name, note, tags, field, is_new, project) = self
                        .edit
                        .as_ref()
                        .map(|edit| {
                            (
                                edit.name.as_str(),
                                edit.note.as_str(),
                                edit.tags.as_str(),
                                edit.field,
                                edit.id.is_none(),
                                edit.project.as_deref(),
                            )
                        })
                        .unwrap_or(("", "", "", EditField::Name, false, None));
                    let (name_mark, note_mark, tags_mark) = match field {
                        EditField::Name => ("▏", "", ""),
                        EditField::Note => ("", "▏", ""),
                        EditField::Tags => ("", "", "▏"),
                    };
                    let title = if is_new {
                        self.lang
                            .new_task_title(project.unwrap_or_else(|| self.lang.inbox()))
                    } else {
                        self.lang.edit_title().into()
                    };
                    let name_label = self.lang.field_name();
                    let note_label = self.lang.field_note();
                    let tags_label = self.lang.field_tags();
                    let suggestion_line = tag_suggestions.as_ref().map(|(list, index)| {
                        let items: Vec<String> = list
                            .iter()
                            .enumerate()
                            .map(|(i, tag)| {
                                if i == *index {
                                    format!("▸{tag}")
                                } else {
                                    tag.clone()
                                }
                            })
                            .collect();
                        format!("{}  {}\n", self.lang.suggest_label(), items.join("  "))
                    });
                    let keys_hint = if tag_suggestions.is_some() {
                        self.lang.keys_suggest_hint()
                    } else {
                        self.lang.keys_fields_hint()
                    };
                    (
                        title,
                        format!(
                            "{name_label}  {name}{name_mark}\n{tags_label}  {tags}{tags_mark}\n{note_label}  {note}{note_mark}\n\n{}{keys_hint}{}",
                            suggestion_line.unwrap_or_default(),
                            if self.error || self.job.is_some() {
                                format!("\n{}", self.status)
                            } else {
                                String::new()
                            }
                        ),
                    )
                }
                _ => match self
                    .confirm
                    .as_ref()
                    .map(|(task, action)| (task.name.as_str(), action))
                {
                    Some((name, ConfirmAction::Delete)) => {
                        (self.lang.delete_title().into(), self.lang.delete_body(name))
                    }
                    _ => (
                        self.lang.complete_title().into(),
                        self.lang.complete_body(
                            self.confirm
                                .as_ref()
                                .map(|(task, _)| task.name.as_str())
                                .unwrap_or(""),
                        ),
                    ),
                },
            };
            frame.render_widget(
                Paragraph::new(text)
                    .block(panel(&title, true))
                    .style(Style::default().bg(BG))
                    .wrap(Wrap { trim: false }),
                dialog,
            );
        }
    }
}

fn highlight() -> Style {
    Style::default().bg(SELECTED).fg(Color::White)
}

fn view_project_name<'a>(projects: &'a [Project], view: &View) -> Option<&'a str> {
    match view {
        View::Project(id) => projects
            .iter()
            .find(|p| &p.id == id)
            .map(|p| p.name.as_str()),
        _ => None,
    }
}

fn task_in_view(task: &Task, view: &View, project_name: Option<&str>) -> bool {
    match view {
        View::Inbox => task.project.is_none(),
        View::Flagged => task.flagged,
        View::Project(_) => task.project.as_deref() == project_name,
    }
}

fn task_matches_query(task: &Task, query: &str) -> bool {
    query.is_empty()
        || format!(
            "{} {} {} {}",
            task.name,
            task.project.as_deref().unwrap_or(""),
            task.tags.join(" "),
            task.note.as_deref().unwrap_or("")
        )
        .to_lowercase()
        .contains(query)
}

/// 标签字段的联想：匹配正在输入的词（最后一个空格后的片段，包含匹配、忽略大小写），
/// 优先用完整标签库，未取到时退回任务快照里出现过的标签；排除已输入的标签，最多 5 个。
fn tag_suggestions_for(known: &[String], tasks: &[Task], field: &str) -> Vec<String> {
    let Some(word) = field.rsplit(' ').next().map(str::trim) else {
        return vec![];
    };
    if word.is_empty() {
        return vec![];
    }
    let mut pool: Vec<String> = if known.is_empty() {
        tasks
            .iter()
            .flat_map(|task| task.tags.iter().cloned())
            .collect()
    } else {
        known.to_vec()
    };
    pool.sort();
    pool.dedup();
    let lower = word.to_lowercase();
    let entered: Vec<&str> = field.split_whitespace().collect();
    pool.into_iter()
        .filter(|tag| tag.to_lowercase().contains(&lower) && !entered.contains(&tag.as_str()))
        .take(5)
        .collect()
}
fn panel(title: &str, focused: bool) -> Block<'_> {
    Block::bordered()
        .title(title)
        .border_style(Style::default().fg(if focused {
            ACCENT
        } else {
            Color::Rgb(67, 71, 92)
        }))
}
fn copy(text: &str, lang: Lang) -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let clipboard_unavailable = || match lang {
        Lang::Zh => "无法打开剪贴板",
        Lang::En => "Cannot open clipboard",
    };
    let copy_failed = || match lang {
        Lang::Zh => "复制失败",
        Lang::En => "Copy failed",
    };
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or_else(clipboard_unavailable)?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(copy_failed().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    fn app_with(config: Config) -> App {
        let mut app = App::new(true, config);
        app.capture_only = false;
        app.mode = Mode::Normal;
        app.sidebar_focus = false;
        app.data = Snapshot::from_json(
            json!([{"id":"p","name":"工作"}, {"id":"p2","name":"生活"}]),
            json!([
                {"id":"one","name":"中文任务","project":"工作","note":"详细说明","tags":["下一步"]},
                {"id":"two","name":"Inbox idea","flagged":true},
                {"id":"three","name":"生活任务","project":"生活","tags":["shop","shopping"]}
            ]),
        )
        .unwrap();
        app.rebuild(None);
        app
    }

    fn app() -> App {
        app_with(Config::default())
    }

    #[test]
    fn starts_with_sidebar_focused() {
        assert!(App::new(false, Config::default()).sidebar_focus);
    }

    #[test]
    fn english_lang_switches_ui_labels() {
        let config = Config {
            language: Lang::En,
            ..Config::default()
        };
        let mut app = app_with(config);
        let label = |app: &App, view: &View| {
            app.views
                .iter()
                .find(|(v, _)| v == view)
                .map(|(_, label)| label.clone())
                .unwrap()
        };
        assert_eq!(app.view, View::Inbox);
        assert_eq!(app.sidebar.selected(), Some(0));
        assert_eq!(app.selected().unwrap().id, "two");
        assert_eq!(
            app.views
                .iter()
                .map(|(view, _)| view.clone())
                .collect::<Vec<_>>(),
            vec![
                View::Inbox,
                View::Flagged,
                View::Project("p".into()),
                View::Project("p2".into())
            ]
        );
        assert_eq!(label(&app, &View::Inbox), "Inbox");
        assert_eq!(label(&app, &View::Flagged), "Flagged");
        assert_eq!(app.data.tasks[0].name, "中文任务");
        let mut terminal = Terminal::new(TestBackend::new(110, 30)).unwrap();
        terminal.draw(|frame| app.draw(frame)).unwrap();
    }

    #[test]
    fn custom_keymap_overrides_defaults() {
        let keys = Keymaps::from_toml("[keys]\nfocus_project = [\"g\"]").unwrap();
        let mut app = app_with(Config {
            keys,
            ..Config::default()
        });
        app.view = View::Project("p".into());
        app.rebuild(None);
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        app.event(press('g'));
        assert_eq!(app.view, View::Project("p".into()));
        assert_eq!(app.focused_project.as_deref(), Some("p"));
        // 默认键 F 已被替换，按下不再触发聚焦（否则会切换取消）
        app.event(press('F'));
        assert_eq!(app.focused_project.as_deref(), Some("p"));
        assert_eq!(app.view, View::Project("p".into()));
    }

    #[test]
    fn filters_search_and_retains_selection_by_id() {
        let mut app = app();
        app.view = View::Project("p".into());
        app.query = "下一步".into();
        app.rebuild(None);
        assert_eq!(app.selected().unwrap().id, "one");
        app.query.clear();
        app.view = View::Inbox;
        app.rebuild(None);
        assert_eq!(app.selected().unwrap().id, "two");
        app.view = View::Flagged;
        app.rebuild(Some("two".into()));
        assert_eq!(app.selected().unwrap().id, "two");
        app.query = "no match".into();
        app.rebuild(None);
        app.move_selection(1);
        assert!(app.selected().is_none());
    }

    #[test]
    fn captured_inbox_task_is_visible_after_refresh_from_filtered_project() {
        for query in ["", "中文"] {
            let mut app = app();
            app.view = View::Project("p".into());
            app.query = query.into();
            app.rebuild(None);
            app.mode = Mode::Capture;
            app.input = "test".into();
            let mut snapshot = Snapshot::from_json(
                json!([{"id":"p","name":"工作"}]),
                json!([
                    {"id":"one","name":"中文任务","project":"工作"},
                    {"id":"new","name":"test","project":null}
                ]),
            )
            .unwrap();
            let (sender, receiver) = mpsc::channel();
            app.job = Some(receiver);
            sender
                .send(Outcome {
                    snapshot: Ok(std::mem::take(&mut snapshot)),
                    message: Some("已收集到收件箱".into()),
                    written: true,
                    captured_id: Some("new".into()),
                    created_id: None,
                })
                .unwrap();
            app.tick();
            assert!(
                app.rows.iter().any(|i| app.data.tasks[*i].id == "new"),
                "new Inbox task must be visible after capture, including from a filtered project"
            );
            assert_eq!(app.selected().unwrap().id, "new");
            assert!(!app.sidebar_focus);
        }
    }

    #[test]
    fn other_writes_preserve_project_and_search() {
        let mut app = app();
        app.view = View::Project("p".into());
        app.query = "中文".into();
        app.rebuild(None);
        let (sender, receiver) = mpsc::channel();
        app.job = Some(receiver);
        sender
            .send(Outcome {
                snapshot: Ok(std::mem::take(&mut app.data)),
                message: Some("已添加旗标".into()),
                written: true,
                captured_id: None,
                created_id: None,
            })
            .unwrap();
        app.tick();
        assert_eq!(app.view, View::Project("p".into()));
        assert_eq!(app.query, "中文");
        assert_eq!(app.selected().unwrap().id, "one");
    }

    #[test]
    fn renders_narrow_wide_empty_and_modal_without_panicking() {
        for (width, height) in [(35, 10), (70, 24), (150, 40), (10, 5)] {
            let mut app = app();
            app.show_detail = true;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.mode = Mode::Capture;
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.mode = Mode::Edit;
            app.edit = Some(EditState {
                id: Some("one".into()),
                name: "中文任务".into(),
                note: "详细说明".into(),
                tags: "下一步".into(),
                original_tags: vec!["下一步".into()],
                field: EditField::Note,
                tag_index: 0,
                project: None,
            });
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.edit = None;
            app.mode = Mode::Normal;
            app.query = "missing".into();
            app.rebuild(None);
            terminal.draw(|frame| app.draw(frame)).unwrap();
        }
    }

    #[test]
    fn capital_f_focuses_and_filters_sidebar_to_the_tasks_project() {
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let project_count = |app: &App| {
            app.views
                .iter()
                .filter(|(v, _)| matches!(v, View::Project(_)))
                .count()
        };
        let mut app = app();
        app.view = View::Project("p".into());
        app.rebuild(None);
        // 按 F：切到项目视图，侧栏项目只剩被聚焦的一个
        app.event(press('F'));
        assert_eq!(app.view, View::Project("p".into()));
        assert_eq!(app.focused_project.as_deref(), Some("p"));
        assert_eq!(project_count(&app), 1);
        assert_eq!(app.selected().unwrap().id, "one");
        assert!(app.job.is_none());

        // 收件箱任务：切到收件箱视图并取消聚焦
        app.sidebar_focus = true;
        app.view = View::Flagged;
        app.rebuild(None);
        app.event(press('F'));
        assert_eq!(app.view, View::Inbox);
        assert!(app.focused_project.is_none());
        assert_eq!(project_count(&app), 2);
        assert!(!app.sidebar_focus);
        assert_eq!(app.selected().unwrap().id, "two");

        // 聚焦另一个项目：目标随之切换
        app.view = View::Project("p2".into());
        app.rebuild(None);
        app.event(press('F'));
        assert_eq!(app.view, View::Project("p2".into()));
        assert_eq!(app.focused_project.as_deref(), Some("p2"));
        assert_eq!(project_count(&app), 1);

        // Esc 取消聚焦而不是退出
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(app.focused_project.is_none());
        assert_eq!(project_count(&app), 2);
        assert_eq!(app.view, View::Project("p2".into()));
        assert!(!app.quit);

        // 同一项目再按 F：切换取消
        app.tasks.select(Some(0));
        app.event(press('F'));
        assert_eq!(app.focused_project.as_deref(), Some("p2"));
        app.event(press('F'));
        assert!(app.focused_project.is_none());
        assert_eq!(project_count(&app), 2);

        // 小写 f 恢复纯旗标语义：在侧栏按不切视图、不触发写入
        app.sidebar_focus = true;
        app.query.clear();
        app.rebuild(None);
        app.event(press('f'));
        assert_eq!(app.view, View::Project("p2".into()));
        assert!(app.sidebar_focus);
        assert!(app.job.is_none());

        // 没有选中任务时只提示，不切视图
        app.query = "no match".into();
        app.rebuild(None);
        assert!(app.selected().is_none());
        app.event(press('F'));
        assert_eq!(app.view, View::Project("p2".into()));
    }

    #[test]
    fn editing_tasks_switches_fields_and_requires_a_name() {
        let mut app = app();
        app.view = View::Project("p".into());
        app.rebuild(None);
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let key =
            |code: KeyCode| Event::Key(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE));

        // 侧栏不触发
        app.sidebar_focus = true;
        app.event(press('e'));
        assert!(app.edit.is_none());
        app.sidebar_focus = false;

        // 打开编辑：预填当前名称、备注与标签
        app.event(press('e'));
        assert!(app.mode == Mode::Edit);
        assert_eq!(app.edit.as_ref().map(|e| e.name.as_str()), Some("中文任务"));
        assert_eq!(app.edit.as_ref().map(|e| e.note.as_str()), Some("详细说明"));
        assert_eq!(app.edit.as_ref().map(|e| e.tags.as_str()), Some("下一步"));

        // Tab 在三个字段间循环：名称 → 标签 → 备注 → 名称
        app.event(press('2'));
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().field, EditField::Tags);
        app.event(press('x'));
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().field, EditField::Note);
        app.event(press('！'));
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().field, EditField::Name);
        let edit = app.edit.as_ref().unwrap();
        assert_eq!(edit.name, "中文任务2");
        assert_eq!(edit.note, "详细说明！");
        assert_eq!(edit.tags, "下一步x");

        // 清空名称后 Enter 不写入、不关弹窗（当前已在名称字段）
        while app.edit.as_ref().is_some_and(|edit| !edit.name.is_empty()) {
            app.event(key(KeyCode::Backspace));
        }
        app.event(key(KeyCode::Enter));
        assert!(app.mode == Mode::Edit);
        assert!(app.job.is_none());

        // Esc 取消编辑
        app.event(key(KeyCode::Esc));
        assert!(app.mode == Mode::Normal);
        assert!(app.edit.is_none());
        assert!(app.job.is_none());
    }

    #[test]
    fn creating_tasks_targets_the_current_view() {
        let mut app = app();
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let esc = || {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Esc,
                KeyModifiers::NONE,
            ))
        };

        // 非项目视图：目标为收件箱
        app.event(press('n'));
        assert!(app.mode == Mode::Edit);
        let edit = app.edit.as_ref().unwrap();
        assert!(edit.id.is_none());
        assert!(edit.project.is_none());
        assert!(edit.name.is_empty());
        app.event(esc());
        assert!(app.edit.is_none());

        // 项目视图：目标为该项目，侧栏聚焦时同样生效
        app.view = View::Project("p".into());
        app.rebuild(None);
        app.sidebar_focus = true;
        app.event(press('n'));
        assert_eq!(
            app.edit.as_ref().and_then(|e| e.project.as_deref()),
            Some("工作")
        );
        // 输入名称与备注（不按 Enter，避免真实写入）；两次 Tab 才到备注
        app.event(press('买'));
        app.event(press('菜'));
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        )));
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::NONE,
        )));
        app.event(press('快'));
        let edit = app.edit.as_ref().unwrap();
        assert_eq!(edit.name, "买菜");
        assert_eq!(edit.note, "快");
        app.event(esc());
        assert!(app.mode == Mode::Normal);
        assert!(app.job.is_none());
    }

    #[test]
    fn search_shows_match_counts_next_to_sidebar_views() {
        let mut app = app();
        app.query = "中文".into(); // 只匹配「中文任务」（项目 工作）
        app.rebuild(None);
        let label = |app: &App, view: &View| {
            app.views
                .iter()
                .find(|(v, _)| v == view)
                .map(|(_, label)| label.clone())
                .unwrap()
        };
        assert_eq!(label(&app, &View::Inbox), "收件箱 · 0");
        assert_eq!(label(&app, &View::Flagged), "已标记 · 0");
        assert_eq!(label(&app, &View::Project("p".into())), "工作 · 1");
        assert_eq!(label(&app, &View::Project("p2".into())), "生活 · 0");
        assert!(app.rows.is_empty());

        // 无搜索词时不显示计数
        app.query.clear();
        app.rebuild(None);
        assert_eq!(label(&app, &View::Inbox), "收件箱");
        assert_eq!(label(&app, &View::Project("p".into())), "工作");
    }

    #[test]
    fn tag_key_reuses_edit_form_at_the_tags_field() {
        let mut app = app();
        app.view = View::Project("p".into());
        app.rebuild(None);
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let key =
            |code: KeyCode| Event::Key(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE));

        // 侧栏不触发
        app.sidebar_focus = true;
        app.event(press('t'));
        assert!(app.edit.is_none());
        app.sidebar_focus = false;

        // t 打开编辑表单并定位到标签字段，预填已有标签
        app.event(press('t'));
        assert!(app.mode == Mode::Edit);
        let edit = app.edit.as_ref().unwrap();
        assert_eq!(edit.field, EditField::Tags);
        assert_eq!(edit.id.as_deref(), Some("one"));
        assert_eq!(edit.tags, "下一步");
        assert_eq!(edit.original_tags, vec!["下一步".to_string()]);

        // 清空标签字段后 Enter 被拦截（CLI 无法表达清空全部标签）
        while app.edit.as_ref().is_some_and(|edit| !edit.tags.is_empty()) {
            app.event(key(KeyCode::Backspace));
        }
        app.event(key(KeyCode::Enter));
        assert!(app.mode == Mode::Edit);
        assert!(app.job.is_none());
        assert!(app.error);

        // 追加一个标签再 Esc 取消，不写入
        app.event(press('e'));
        app.event(press('r'));
        app.event(press('r'));
        app.event(press('a'));
        app.event(press('n'));
        app.event(press('d'));
        assert_eq!(app.edit.as_ref().unwrap().tags, "errand");
        app.event(key(KeyCode::Esc));
        assert!(app.mode == Mode::Normal);
        assert!(app.job.is_none());
    }

    #[test]
    fn tag_field_suggests_and_completes() {
        let fixture = app();
        let tasks = &fixture.data.tasks;
        // 联想匹配当前词：退回任务快照标签、忽略大小写、排除已输入的标签
        assert_eq!(
            tag_suggestions_for(&[], tasks, "下一步 sh"),
            vec!["shop".to_string(), "shopping".to_string()]
        );
        assert_eq!(
            tag_suggestions_for(&[], tasks, "SH"),
            vec!["shop".to_string(), "shopping".to_string()]
        );
        assert!(tag_suggestions_for(&[], tasks, "下一步 ").is_empty());
        assert!(tag_suggestions_for(&[], tasks, "下一步").is_empty());
        // 完整标签库优先
        assert_eq!(
            tag_suggestions_for(&["自定义".into()], tasks, "自"),
            vec!["自定义".to_string()]
        );
        drop(fixture);

        let mut app = app();
        app.view = View::Project("p".into());
        app.rebuild(None);
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let key =
            |code: KeyCode| Event::Key(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE));

        // 打开编辑表单，一次 Tab 即到标签字段（名称 → 标签 → 备注）
        app.event(press('e'));
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().field, EditField::Tags);
        app.event(press(' '));
        app.event(press('s'));
        app.event(press('h'));
        // ↓ 选中第二个候选 shopping，Tab 补全
        app.event(key(KeyCode::Down));
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().tags, "下一步 shopping ");

        // 无候选时 Tab 仍切换字段（标签 → 备注）
        app.edit.as_mut().unwrap().tags = "zz".into();
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().field, EditField::Note);

        // Esc 取消，不写入
        app.event(key(KeyCode::Esc));
        assert!(app.mode == Mode::Normal);
        assert!(app.job.is_none());
    }

    #[test]
    fn note_field_supports_shift_enter_newlines_and_multiline_paste() {
        let mut app = app();
        app.view = View::Project("p".into());
        app.rebuild(None);
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let key =
            |code: KeyCode| Event::Key(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE));
        let shift_enter = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::SHIFT,
        ));
        // macOS 部分输入法把 Shift+Enter 上报为 Option+Enter，同样视为换行
        let alt_enter = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::ALT,
        ));

        // 名称字段按 Shift+Enter / Option+Enter 不换行也不提交
        app.event(press('e'));
        app.event(shift_enter.clone());
        app.event(alt_enter.clone());
        assert!(app.mode == Mode::Edit);
        assert_eq!(app.edit.as_ref().unwrap().name, "中文任务");

        // 备注字段（两次 Tab 到达）支持 Shift+Enter 换行
        app.event(key(KeyCode::Tab));
        app.event(key(KeyCode::Tab));
        assert_eq!(app.edit.as_ref().unwrap().field, EditField::Note);
        app.event(shift_enter);
        app.event(press('第'));
        app.event(press('二'));
        app.event(press('行'));
        assert_eq!(app.edit.as_ref().unwrap().note, "详细说明\n第二行");

        // Option+Enter 同样换行
        app.event(alt_enter);
        app.event(press('三'));
        assert_eq!(app.edit.as_ref().unwrap().note, "详细说明\n第二行\n三");

        // 粘贴多行文本：备注保留换行
        app.event(Event::Paste("粘贴\n多行".into()));
        assert_eq!(
            app.edit.as_ref().unwrap().note,
            "详细说明\n第二行\n三粘贴\n多行"
        );

        // 多行备注可整体删除（退格逐字符）
        while app.edit.as_ref().is_some_and(|edit| !edit.note.is_empty()) {
            app.event(key(KeyCode::Backspace));
        }
        assert_eq!(app.edit.as_ref().unwrap().note, "");
        app.event(key(KeyCode::Esc));
        assert!(app.mode == Mode::Normal);
        assert!(app.job.is_none());
    }

    #[test]
    fn deleting_requires_confirmation_and_never_runs_from_sidebar() {
        let mut app = app();
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        app.sidebar_focus = true;
        app.event(press('D'));
        assert!(app.confirm.is_none());
        app.sidebar_focus = false;
        app.event(press('D'));
        assert!(app.mode == Mode::Confirm);
        assert!(matches!(app.confirm, Some((_, ConfirmAction::Delete))));
        assert!(app.job.is_none());
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(app.mode == Mode::Normal);
        assert!(app.job.is_none());
        assert!(app.confirm.is_none());
    }

    #[test]
    fn completing_requires_confirmation_and_sidebar_never_completes_tasks() {
        let mut app = app();
        app.sidebar_focus = true;
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::NONE,
        )));
        assert!(app.confirm.is_none());
        app.sidebar_focus = false;
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::NONE,
        )));
        assert!(app.mode == Mode::Confirm);
        assert!(app.job.is_none());
        app.event(Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(app.mode == Mode::Normal);
        assert!(app.job.is_none());
    }
}
