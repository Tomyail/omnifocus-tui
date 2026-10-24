mod app;
mod backend;
mod config;

use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
    execute,
};
use std::{
    io::{self, IsTerminal},
    time::Duration,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("omnifocus-tui: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            println!(
                "omnifocus-tui — 独立的 OmniFocus 终端界面\n\n用法：omnifocus-tui [--capture | --check | --version]\n\n  --capture  快速收集到 Inbox，保存后退出\n  --check    只读验证 of 连接并显示项目与任务数量\n\n需要 macOS、OmniFocus、of CLI。可用 OMNIFOCUS_CLI 指定 of 路径。\n任意终端直接运行；可选 Herdr 集成见 integrations/herdr/。"
            );
            return Ok(());
        }
        Some("--version" | "-V") => {
            println!("omnifocus-tui {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("--check") if args.len() == 1 => {
            let snapshot = backend::Backend::new()
                .snapshot()
                .map_err(io::Error::other)?;
            println!(
                "OmniFocus 连接成功：{} 个项目，{} 个任务（已排除项目根任务）",
                snapshot.projects.len(),
                snapshot.tasks.len()
            );
            for project in &snapshot.projects {
                let count = snapshot
                    .tasks
                    .iter()
                    .filter(|t| t.project.as_deref() == Some(project.name.as_str()))
                    .count();
                println!("  {} · {} 个任务", project.name, count);
            }
            return Ok(());
        }
        Some("--capture") if args.len() == 1 => {}
        None => {}
        _ => return Err("未知参数，请运行 omnifocus-tui --help".into()),
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("请在交互式终端中运行；只读检查可使用 --check".into());
    }
    let keys = config::Keymaps::load().map_err(io::Error::other)?;
    let mut terminal = ratatui::try_init()?;
    let result = (|| -> io::Result<()> {
        execute!(io::stdout(), EnableMouseCapture, EnableBracketedPaste)?;
        let mut app =
            app::App::new(args.first().is_some_and(|arg| arg == "--capture"), keys);
        while !app.quit {
            app.tick();
            terminal.draw(|frame| app.draw(frame))?;
            if event::poll(Duration::from_millis(80))? {
                app.event(event::read()?);
            }
        }
        Ok(())
    })();
    let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    result?;
    Ok(())
}
