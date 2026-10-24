use crate::backend::{Backend, Snapshot, Task};
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
    Confirm,
}

enum Operation {
    Refresh,
    Capture(String),
    Complete(String),
    Flag(String, bool),
}

struct Outcome {
    snapshot: Result<Snapshot, String>,
    message: Option<String>,
    written: bool,
    captured_id: Option<String>,
}

pub struct App {
    backend: Backend,
    data: Snapshot,
    views: Vec<(View, String)>,
    view: View,
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
    confirm: Option<Task>,
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
    pub fn new(capture_only: bool) -> Self {
        let mut app = Self {
            backend: Backend::new(),
            data: Snapshot::default(),
            views: vec![],
            view: View::All,
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
        self.views = vec![
            (View::All, "全部任务".into()),
            (View::Inbox, "收件箱".into()),
            (View::Flagged, "已标记".into()),
        ];
        self.views.extend(self.data.projects.iter().map(|p| {
            let label = if p.status == "on hold" {
                format!("{} · 暂停", p.name)
            } else {
                p.name.clone()
            };
            (View::Project(p.id.clone()), label)
        }));
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
            let result = match operation {
                Operation::Refresh => Ok((None, None)),
                Operation::Capture(name) => backend.capture(&name).map(|task| {
                    (
                        Some("已收集到收件箱".into()),
                        task.get("id").and_then(|id| id.as_str()).map(str::to_owned),
                    )
                }),
                Operation::Complete(id) => backend
                    .complete(&id)
                    .map(|_| (Some("任务已完成".into()), None)),
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
                    )
                }),
            };
            let outcome = match result {
                Ok((message, captured_id)) => {
                    let written = message.is_some();
                    Outcome {
                        snapshot: if capture_only {
                            Ok(Snapshot::default())
                        } else {
                            backend.snapshot()
                        },
                        message,
                        written,
                        captured_id,
                    }
                }
                Err(error) => Outcome {
                    snapshot: Err(error),
                    message: None,
                    written: false,
                    captured_id: None,
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

    pub fn event(&mut self, event: Event) {
        if let Event::Paste(text) = event {
            if matches!(self.mode, Mode::Search | Mode::Capture) {
                self.input.extend(text.chars().filter(|c| !c.is_control()));
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
        if matches!(self.mode, Mode::Capture | Mode::Search) {
            match key.code {
                KeyCode::Esc if self.job.is_none() => {
                    if self.capture_only {
                        self.quit = true;
                    }
                    self.mode = Mode::Normal;
                    self.input.clear();
                    self.query.clear();
                    self.rebuild(None);
                }
                KeyCode::Enter if self.mode == Mode::Capture => {
                    if !self.input.trim().is_empty() {
                        self.start(Operation::Capture(self.input.clone()));
                    }
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    self.sidebar_focus = false;
                }
                KeyCode::Backspace if self.job.is_none() => {
                    self.input.pop();
                }
                KeyCode::Char(c)
                    if self.job.is_none()
                        && !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.input.push(c);
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
                    if let Some(task) = self.confirm.take() {
                        self.start(Operation::Complete(task.id));
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
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc if self.job.is_none() => {
                if !self.query.is_empty() {
                    self.query.clear();
                    self.rebuild(None);
                } else if self.show_detail {
                    self.show_detail = false;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('h') | KeyCode::Left => {
                self.show_sidebar = true;
                self.sidebar_focus = true;
            }
            KeyCode::Char('l') | KeyCode::Right => self.sidebar_focus = false,
            KeyCode::Tab | KeyCode::BackTab => {
                self.sidebar_focus = !self.sidebar_focus;
                self.show_sidebar = true;
            }
            KeyCode::Char('p') => {
                self.show_sidebar = !self.show_sidebar;
                self.sidebar_focus = self.show_sidebar;
            }
            KeyCode::Enter => {
                self.show_detail = !self.show_detail;
                self.detail_scroll = 0;
            }
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            KeyCode::Char('r') => self.start(Operation::Refresh),
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.input = self.query.clone();
            }
            KeyCode::Char('a' | '+') if self.job.is_none() => {
                self.mode = Mode::Capture;
                self.input.clear();
            }
            KeyCode::Char('f') if !self.sidebar_focus && self.job.is_none() => {
                if let Some(task) = self.selected() {
                    self.start(Operation::Flag(task.id.clone(), !task.flagged));
                }
            }
            KeyCode::Char('d') if !self.sidebar_focus && self.job.is_none() => {
                self.confirm = self.selected().cloned();
                if self.confirm.is_some() {
                    self.mode = Mode::Confirm;
                }
            }
            KeyCode::Char('o') if !self.sidebar_focus => {
                if let Some(task) = self.selected()
                    && let Err(error) = std::process::Command::new("open")
                        .arg(format!("omnifocus:///task/{}", task.id))
                        .spawn()
                {
                    self.status = error.to_string();
                    self.error = true;
                }
            }
            KeyCode::Char('y') if !self.sidebar_focus => {
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
        frame.render_widget(Paragraph::new(" Tab 项目/任务  ↑↓/jk 移动  Enter 详情  / 搜索  a 收集  d 完成\n f 旗标  y 复制给 agent  o 打开原生  r 刷新  p 项目栏  q 退出").style(Style::default().fg(MUTED)), footer);
        if self.mode == Mode::Capture || self.mode == Mode::Search || self.mode == Mode::Confirm {
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
                    " 快速收集 → OmniFocus 收件箱 ",
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
                    " 搜索当前视图 ",
                    format!(
                        "{}▏\n\n标题、项目、标签、备注 · Enter 应用 · Esc 清除",
                        self.input
                    ),
                ),
                _ => (
                    " 完成任务 ",
                    format!(
                        "{}\n\n完成父任务会影响子任务；重复任务将按原规则生成下次。\ny / Enter 完成 · 其他键取消",
                        self.confirm.as_ref().map(|t| t.name.as_str()).unwrap_or("")
                    ),
                ),
            };
            frame.render_widget(
                Paragraph::new(text)
                    .block(panel(title, true))
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

    fn app() -> App {
        let mut app = App::new(true);
        app.capture_only = false;
        app.mode = Mode::Normal;
        app.data = Snapshot::from_json(
            json!([{"id":"p","name":"工作"}]),
            json!([
                {"id":"one","name":"中文任务","project":"工作","note":"详细说明","tags":["下一步"]},
                {"id":"two","name":"Inbox idea","flagged":true}
            ]),
        )
        .unwrap();
        app.rebuild(None);
        app
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
            app.mode = Mode::Normal;
            app.query = "missing".into();
            app.rebuild(None);
            terminal.draw(|frame| app.draw(frame)).unwrap();
        }
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
