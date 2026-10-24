use crate::backend::{Backend, Snapshot, Task};
use crate::config::{Action, Keymaps};
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
    All,
    Inbox,
    Flagged,
    Project(String),
}

#[derive(PartialEq)]
enum Mode {
    Normal,
    Search,
    Capture,
    Tag,
    Edit,
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum EditField {
    Name,
    Note,
}

struct EditState {
    /// None 表示新建任务；Some 为编辑既有任务的 id。
    id: Option<String>,
    name: String,
    note: String,
    field: EditField,
    /// 新建任务的目标项目名；None 表示收件箱。编辑时忽略。
    project: Option<String>,
}

impl EditState {
    fn active_mut(&mut self) -> &mut String {
        match self.field {
            EditField::Name => &mut self.name,
            EditField::Note => &mut self.note,
        }
    }
}

enum Operation {
    Refresh,
    Capture(String),
    Create {
        name: String,
        note: String,
        project: Option<String>,
    },
    Complete(String),
    Drop(String),
    Update { id: String, name: String, note: String },
    Flag(String, bool),
    Tag { id: String, tags: Vec<String> },
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
    refreshed: Instant,
    capture_only: bool,
    pub quit: bool,
    sidebar_area: Rect,
    tasks_area: Rect,
}

impl App {
    pub fn new(capture_only: bool, keys: Keymaps) -> Self {
        let mut app = Self {
            backend: Backend::new(),
            data: Snapshot::default(),
            keys,
            views: vec![],
            view: View::All,
            focused_project: None,
            rows: vec![],
            sidebar: ListState::default().with_selected(Some(0)),
            tasks: ListState::default(),
            sidebar_focus: false,
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
            refreshed: Instant::now(),
            capture_only,
            quit: false,
            sidebar_area: Rect::default(),
            tasks_area: Rect::default(),
        };
        app.rebuild(None);
        if !capture_only {
            app.start(Operation::Refresh);
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
        self.views = vec![
            (View::All, "全部任务".into()),
            (View::Inbox, "收件箱".into()),
            (View::Flagged, "已标记".into()),
        ];
        self.views.extend(
            self.data
                .projects
                .iter()
                .filter(|p| focused.as_deref().is_none_or(|id| p.id == id))
                .map(|p| {
                    let label = if p.status == "on hold" {
                        format!("{} · 暂停", p.name)
                    } else {
                        p.name.clone()
                    };
                    (View::Project(p.id.clone()), label)
                }),
        );
        if !self.views.iter().any(|(v, _)| v == &self.view) {
            self.view = View::All;
        }
        self.sidebar
            .select(self.views.iter().position(|(v, _)| v == &self.view));
        let project = match &self.view {
            View::Project(id) => self
                .data
                .projects
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.name.as_str()),
            _ => None,
        };
        let query = self.query.to_lowercase();
        self.rows = self
            .data
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let matches = match &self.view {
                    View::All => true,
                    View::Inbox => t.project.is_none(),
                    View::Flagged => t.flagged,
                    View::Project(_) => t.project.as_deref() == project,
                };
                matches
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {}",
                            t.name,
                            t.project.as_deref().unwrap_or(""),
                            t.tags.join(" "),
                            t.note.as_deref().unwrap_or("")
                        )
                        .to_lowercase()
                        .contains(&query))
            })
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
        self.status = match &operation {
            Operation::Refresh => "正在读取 OmniFocus…",
            _ => "正在写入 OmniFocus…",
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
                        Some("已收集到收件箱".into()),
                        task.get("id").and_then(|id| id.as_str()).map(str::to_owned),
                        true,
                    )
                }),
                Operation::Create { name, note, project } => {
                    backend.create(&name, &note, project.as_deref()).map(|task| {
                        (
                            Some("任务已创建".into()),
                            task.get("id").and_then(|id| id.as_str()).map(str::to_owned),
                            false,
                        )
                    })
                }
                Operation::Complete(id) => backend
                    .complete(&id)
                    .map(|_| (Some("任务已完成".into()), None, false)),
                Operation::Drop(id) => backend
                    .drop(&id)
                    .map(|_| (Some("任务已删除".into()), None, false)),
                Operation::Update { id, name, note } => backend
                    .update(&id, &name, &note)
                    .map(|_| (Some("任务已更新".into()), None, false)),
                Operation::Flag(id, flagged) => backend.flag(&id, flagged).map(|_| {
                    (
                        Some(
                            if flagged {
                                "已添加旗标"
                            } else {
                                "已移除旗标"
                            }
                            .into(),
                        ),
                        None,
                        false,
                    )
                }),
                Operation::Tag { id, tags } => backend
                    .tag(&id, &tags)
                    .map(|_| (Some("标签已更新".into()), None, false)),
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
                        format!(
                            "已同步 · {} 个项目 · {} 个任务",
                            self.data.projects.len(),
                            self.data.tasks.len()
                        )
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

    fn focus_selected_task_project(&mut self) {
        let Some((id, project)) = self
            .selected()
            .map(|task| (task.id.clone(), task.project.clone()))
        else {
            self.status = "请先选中一个任务，再按 F 聚焦它的项目".into();
            self.error = false;
            return;
        };
        // of 按 名称 归属任务与视图过滤，这里保持同一口径。
        let view = match project.as_deref() {
            Some(name) => match self.data.projects.iter().find(|p| p.name == name) {
                Some(project) => View::Project(project.id.clone()),
                None => {
                    self.status = format!("未找到项目「{name}」，按 r 刷新后重试");
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
                    self.status = "已取消聚焦，侧栏显示全部项目".into();
                    None
                } else {
                    self.status = format!(
                        "已聚焦「{}」· 侧栏只显示该项目，再按 F 或 Esc 取消",
                        project.clone().unwrap_or_default()
                    );
                    Some(project_id.clone())
                };
            }
            _ => {
                self.focused_project = None;
                self.status = "收件箱任务没有所属项目".into();
            }
        }
        self.view = view;
        self.rebuild(Some(id));
        self.sidebar_focus = false;
        self.detail_scroll = 0;
    }

    pub fn event(&mut self, event: Event) {
        if let Event::Paste(text) = event {
            if matches!(self.mode, Mode::Search | Mode::Capture | Mode::Tag | Mode::Edit) {
                let text: String = text.chars().filter(|c| !c.is_control()).collect();
                if matches!(self.mode, Mode::Edit) {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.active_mut().push_str(&text);
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
        if matches!(
            self.mode,
            Mode::Capture | Mode::Search | Mode::Tag | Mode::Edit
        ) {
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
                KeyCode::Enter if self.mode == Mode::Tag => {
                    let added: Vec<String> = self
                        .input
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect();
                    if let (Some(task), false) = (self.selected(), added.is_empty()) {
                        self.start(Operation::Tag {
                            id: task.id.clone(),
                            tags: merged_tags(&task.tags, &added),
                        });
                    }
                }
                KeyCode::Enter if self.mode == Mode::Edit => match self.edit.take() {
                    Some(edit) if !edit.name.trim().is_empty() => {
                        let name = edit.name.trim().to_owned();
                        let note = edit.note;
                        match edit.id {
                            Some(id) => self.start(Operation::Update { id, name, note }),
                            None => {
                                self.start(Operation::Create {
                                    name,
                                    note,
                                    project: edit.project,
                                });
                            }
                        }
                    }
                    Some(edit) => {
                        // 名称被清空：回到编辑态并提示，不写入
                        self.edit = Some(edit);
                        self.status = "任务名称不能为空".into();
                        self.error = true;
                    }
                    None => {}
                },
                KeyCode::Tab if self.mode == Mode::Edit => {
                    if let Some(edit) = self.edit.as_mut() {
                        edit.field = match edit.field {
                            EditField::Name => EditField::Note,
                            EditField::Note => EditField::Name,
                        };
                    }
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    self.sidebar_focus = false;
                }
                KeyCode::Backspace if self.job.is_none() => {
                    if matches!(self.mode, Mode::Edit) {
                        if let Some(edit) = self.edit.as_mut() {
                            edit.active_mut().pop();
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
                    self.status = "已取消聚焦，侧栏显示全部项目".into();
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
                if self.selected().is_some() {
                    self.mode = Mode::Tag;
                    self.input.clear();
                    self.error = false;
                }
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
                if let Some(task) = self.selected() {
                    self.edit = Some(EditState {
                        id: Some(task.id.clone()),
                        name: task.name.clone(),
                        note: task.note.clone().unwrap_or_default(),
                        field: EditField::Name,
                        project: None,
                    });
                    self.mode = Mode::Edit;
                    self.error = false;
                }
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
                self.edit = Some(EditState {
                    id: None,
                    name: String::new(),
                    note: String::new(),
                    field: EditField::Name,
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
                    let result = copy(&reference);
                    self.status = result
                        .err()
                        .unwrap_or_else(|| "已复制任务与 ID，可粘贴给 agent".into());
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
            frame.render_widget(Paragraph::new("窗口太小，请扩大到至少 35×10"), area);
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
                        "{} 个任务  {}",
                        self.rows.len(),
                        if self.job.is_some() {
                            "· 同步中"
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
                    .block(panel(" 项目 / 视图 ", self.sidebar_focus))
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
            .unwrap_or("任务");
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .map(|i| {
                let task = &self.data.tasks[*i];
                let mut metadata = task.project.clone().unwrap_or_else(|| "收件箱".into());
                if !task.tags.is_empty() {
                    metadata.push_str(&format!(" · #{}", task.tags.join(" #")));
                }
                if let Some(due) = &task.due {
                    metadata.push_str(&format!(" · 截止 {}", date(due)));
                }
                ListItem::new(Text::from(vec![
                    Line::from(format!(
                        "{} {}",
                        if task.flagged { "⚑" } else { "○" },
                        task.name
                    )),
                    Line::styled(format!("  {metadata}"), Style::default().fg(MUTED)),
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
                    "正在读取…"
                } else {
                    "没有匹配的任务"
                })
                .style(Style::default().fg(MUTED)),
                task_area.inner(ratatui::layout::Margin::new(2, 2)),
            );
        }
        if let Some(detail) = detail_area {
            let text = self.selected().map(|t| {
                format!("{}\n\n项目  {}\n标签  {}\n截止  {}\n推迟  {}\n计划  {}\n预估  {}\nID    {}\n\n{}", t.name, t.project.as_deref().unwrap_or("收件箱"), t.tags.join(" · "),
                    t.due.as_deref().map(date).unwrap_or_else(|| "—".into()), t.defer.as_deref().map(date).unwrap_or_else(|| "—".into()),
                    t.planned.as_deref().map(date).unwrap_or_else(|| "—".into()), t.estimated_minutes.map(|m| format!("{m} 分钟")).unwrap_or_else(|| "—".into()), t.id, t.note.as_deref().unwrap_or("暂无备注"))
            }).unwrap_or_else(|| "选择一个任务".into());
            frame.render_widget(
                Paragraph::new(text)
                    .block(panel(" 详情 · PgUp/PgDn 滚动 ", false))
                    .wrap(Wrap { trim: false })
                    .scroll((self.detail_scroll, 0)),
                detail,
            );
        }
        let status_text = if self.query.is_empty() {
            self.status.clone()
        } else {
            format!("搜索：{} · {}", self.query, self.status)
        };
        frame.render_widget(
            Paragraph::new(status_text)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(if self.error { Color::LightRed } else { MUTED })),
            status,
        );
        let keys = &self.keys;
        let footer_text = format!(
            " {} 切换  {}/{} 移动  {} 详情  {} 搜索  {} 收集  {} 新建  {} 编辑  {} 标签\n {} 完成  {} 删除  {} 旗标  {} 聚焦  {} 复制  {} 打开  {} 刷新  {} 项目栏  {} 退出",
            keys.first(Action::TogglePane),
            keys.first(Action::Down),
            keys.first(Action::Up),
            keys.first(Action::ToggleDetail),
            keys.first(Action::Search),
            keys.first(Action::Capture),
            keys.first(Action::Create),
            keys.first(Action::Edit),
            keys.first(Action::Tag),
            keys.first(Action::Complete),
            keys.first(Action::Delete),
            keys.first(Action::Flag),
            keys.first(Action::FocusProject),
            keys.first(Action::Copy),
            keys.first(Action::Open),
            keys.first(Action::Refresh),
            keys.first(Action::ToggleSidebar),
            keys.first(Action::Quit),
        );
        frame.render_widget(
            Paragraph::new(footer_text).style(Style::default().fg(MUTED)),
            footer,
        );
        if matches!(
            self.mode,
            Mode::Capture | Mode::Search | Mode::Tag | Mode::Confirm | Mode::Edit
        ) {
            let [_, center, _] = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(8),
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
                    " 快速收集 → OmniFocus 收件箱 ".into(),
                    format!(
                        "{}▏\n\nEnter 保存 · Esc 取消\n{}",
                        self.input,
                        if self.error || self.job.is_some() {
                            &self.status
                        } else {
                            ""
                        }
                    ),
                ),
                Mode::Search => (
                    " 搜索当前视图 ".into(),
                    format!(
                        "{}▏\n\n标题、项目、标签、备注 · Enter 应用 · Esc 清除",
                        self.input
                    ),
                ),
                Mode::Tag => (
                    " 添加标签 ".into(),
                    format!(
                        "{}▏\n\n{} · Enter 保存 · Esc 取消\n已有  {}",
                        self.input,
                        self.selected().map(|t| t.name.as_str()).unwrap_or(""),
                        self.selected()
                            .map(|t| t.tags.join(" "))
                            .unwrap_or_default(),
                    ),
                ),
                Mode::Edit => {
                    let (name, note, field, is_new, project) = self
                        .edit
                        .as_ref()
                        .map(|edit| {
                            (
                                edit.name.as_str(),
                                edit.note.as_str(),
                                edit.field,
                                edit.id.is_none(),
                                edit.project.as_deref(),
                            )
                        })
                        .unwrap_or(("", "", EditField::Name, false, None));
                    let (name_mark, note_mark) = if field == EditField::Name {
                        ("▏", "")
                    } else {
                        ("", "▏")
                    };
                    let title = if is_new {
                        format!(" 新建任务 · {} ", project.unwrap_or("收件箱"))
                    } else {
                        " 编辑任务 ".into()
                    };
                    (
                        title,
                        format!(
                            "名称  {name}{name_mark}\n备注  {note}{note_mark}\n\nTab 切换字段 · Enter 保存 · Esc 取消{}",
                            if self.error || self.job.is_some() {
                                format!("\n{}", self.status)
                            } else {
                                String::new()
                            }
                        ),
                    )
                }
                _ => match self.confirm.as_ref().map(|(task, action)| (task.name.as_str(), action))
                {
                    Some((name, ConfirmAction::Delete)) => (
                        " 删除任务 ".into(),
                        format!(
                            "{name}\n\n删除会把任务标记为已放弃并从所有视图消失，可在 OmniFocus 中恢复。\ny / Enter 删除 · 其他键取消"
                        ),
                    ),
                    _ => (
                        " 完成任务 ".into(),
                        format!(
                            "{}\n\n完成父任务会影响子任务；重复任务将按原规则生成下次。\ny / Enter 完成 · 其他键取消",
                            self.confirm.as_ref().map(|(t, _)| t.name.as_str()).unwrap_or("")
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

/// 合并已有标签与新标签：保持原有顺序，去重，新标签按输入顺序追加。
fn merged_tags(existing: &[String], added: &[String]) -> Vec<String> {
    let mut tags = existing.to_vec();
    tags.extend(
        added
            .iter()
            .filter(|tag| !existing.contains(tag))
            .cloned(),
    );
    tags
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
fn date(value: &str) -> String {
    // Preserve the source timestamp and its UTC marker instead of guessing the user's timezone.
    value
        .replace('T', " ")
        .replace(".000Z", " UTC")
        .replace('Z', " UTC")
}
fn copy(text: &str) -> Result<(), String> {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("无法打开剪贴板")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("复制失败".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use serde_json::json;

    fn app_with(keys: Keymaps) -> App {
        let mut app = App::new(true, keys);
        app.capture_only = false;
        app.mode = Mode::Normal;
        app.data = Snapshot::from_json(
            json!([{"id":"p","name":"工作"}, {"id":"p2","name":"生活"}]),
            json!([
                {"id":"one","name":"中文任务","project":"工作","note":"详细说明","tags":["下一步"]},
                {"id":"two","name":"Inbox idea","flagged":true},
                {"id":"three","name":"生活任务","project":"生活"}
            ]),
        )
        .unwrap();
        app.rebuild(None);
        app
    }

    fn app() -> App {
        app_with(Keymaps::default())
    }

    #[test]
    fn custom_keymap_overrides_defaults() {
        let keys = Keymaps::from_toml("[keys]\nfocus_project = [\"g\"]").unwrap();
        let mut app = app_with(keys);
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
        app.query = "下一步".into();
        app.rebuild(None);
        assert_eq!(app.selected().unwrap().id, "one");
        app.query.clear();
        app.view = View::Inbox;
        app.rebuild(None);
        assert_eq!(app.selected().unwrap().id, "two");
        app.view = View::All;
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
            app.mode = Mode::Tag;
            app.input = "errand".into();
            terminal.draw(|frame| app.draw(frame)).unwrap();
            app.input.clear();
            app.mode = Mode::Edit;
            app.edit = Some(EditState {
                id: Some("one".into()),
                name: "中文任务".into(),
                note: "详细说明".into(),
                field: EditField::Note,
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
        // 按 F：切到项目视图，侧栏项目只剩被聚焦的一个
        app.event(press('F'));
        assert_eq!(app.view, View::Project("p".into()));
        assert_eq!(app.focused_project.as_deref(), Some("p"));
        assert_eq!(project_count(&app), 1);
        assert_eq!(app.selected().unwrap().id, "one");
        assert!(app.job.is_none());

        // 收件箱任务：切到收件箱视图并取消聚焦
        app.sidebar_focus = true;
        app.view = View::All;
        app.rebuild(None);
        app.tasks.select(Some(1));
        app.event(press('F'));
        assert_eq!(app.view, View::Inbox);
        assert!(app.focused_project.is_none());
        assert_eq!(project_count(&app), 2);
        assert!(!app.sidebar_focus);
        assert_eq!(app.selected().unwrap().id, "two");

        // 聚焦另一个项目：目标随之切换
        app.view = View::All;
        app.rebuild(None);
        app.tasks.select(Some(2));
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
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let key = |code: KeyCode| {
            Event::Key(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE))
        };

        // 侧栏不触发
        app.sidebar_focus = true;
        app.event(press('e'));
        assert!(app.edit.is_none());
        app.sidebar_focus = false;

        // 打开编辑：预填当前名称与备注
        app.event(press('e'));
        assert!(app.mode == Mode::Edit);
        assert_eq!(
            app.edit.as_ref().map(|e| e.name.as_str()),
            Some("中文任务")
        );
        assert_eq!(
            app.edit.as_ref().map(|e| e.note.as_str()),
            Some("详细说明")
        );

        // Tab 切换字段后分别编辑
        app.event(press('2'));
        app.event(key(KeyCode::Tab));
        app.event(press('！'));
        let edit = app.edit.as_ref().unwrap();
        assert_eq!(edit.name, "中文任务2");
        assert_eq!(edit.note, "详细说明！");
        assert_eq!(edit.field, EditField::Note);

        // 清空名称后 Enter 不写入、不关弹窗
        app.event(key(KeyCode::Tab));
        while app
            .edit
            .as_ref()
            .is_some_and(|edit| !edit.name.is_empty())
        {
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
        // 输入名称与备注（不按 Enter，避免真实写入）
        app.event(press('买'));
        app.event(press('菜'));
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
    fn tag_dialog_merges_without_duplicates() {
        let existing = vec!["下一步".to_string()];
        assert_eq!(
            merged_tags(&existing, &["errand".into(), "下一步".into(), "家".into()]),
            vec!["下一步".to_string(), "errand".to_string(), "家".to_string()]
        );
        assert!(merged_tags(&[], &[]).is_empty());

        let mut app = app();
        let press = |c: char| {
            Event::Key(crossterm::event::KeyEvent::new(
                KeyCode::Char(c),
                KeyModifiers::NONE,
            ))
        };
        let key = |code: KeyCode| {
            Event::Key(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE))
        };

        // 侧栏不触发
        app.sidebar_focus = true;
        app.event(press('t'));
        assert!(app.mode == Mode::Normal);
        app.sidebar_focus = false;

        // 打开、输入、空输入 Enter 不写入，Esc 取消
        app.event(press('t'));
        assert!(app.mode == Mode::Tag);
        app.event(press('x'));
        assert_eq!(app.input, "x");
        app.event(key(KeyCode::Backspace));
        app.event(key(KeyCode::Enter)); // 空输入不写入
        assert!(app.mode == Mode::Tag);
        assert!(app.job.is_none());
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
