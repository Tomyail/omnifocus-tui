mod app;
mod backend;
mod config;
mod i18n;

use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
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
    let config = config::Config::load().map_err(io::Error::other)?;
    let lang = config.language;
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--help" | "-h") => {
            println!("{}", lang.help_text());
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
                "{}",
                lang.check_ok(snapshot.projects.len(), snapshot.tasks.len())
            );
            for project in &snapshot.projects {
                let count = snapshot
                    .tasks
                    .iter()
                    .filter(|t| t.project.as_deref() == Some(project.name.as_str()))
                    .count();
                println!("{}", lang.check_project(&project.name, count));
            }
            return Ok(());
        }
        Some("--capture") if args.len() == 1 => {}
        None => {}
        _ => return Err(lang.unknown_arg().into()),
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(lang.need_tty().into());
    }
    let mut terminal = ratatui::try_init()?;
    let result = (|| -> io::Result<()> {
        execute!(
            io::stdout(),
            EnableMouseCapture,
            EnableBracketedPaste,
            // 让支持 kitty 键盘协议的终端区分 Shift+Enter 与 Enter；不支持的终端会忽略
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
        let mut app =
            app::App::new(args.first().is_some_and(|arg| arg == "--capture"), config);
        while !app.quit {
            app.tick();
            terminal.draw(|frame| app.draw(frame))?;
            if event::poll(Duration::from_millis(80))? {
                app.event(event::read()?);
            }
        }
        Ok(())
    })();
    let _ = execute!(
        io::stdout(),
        DisableMouseCapture,
        DisableBracketedPaste,
        PopKeyboardEnhancementFlags
    );
    ratatui::restore();
    result?;
    Ok(())
}
