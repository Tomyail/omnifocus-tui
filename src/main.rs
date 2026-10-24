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
        Some("--keys") if args.len() == 1 => {
            // 按键调试：打印终端实际上报的按键事件，用于验证 kitty 协议与自定义键位
            use crossterm::{
                event::{self, Event, KeyCode, KeyModifiers},
                execute,
                terminal::{disable_raw_mode, enable_raw_mode},
            };
            use std::io::Write as _;
            enable_raw_mode()?;
            let result = (|| -> io::Result<()> {
                let mut out = io::stdout();
                execute!(
                    &mut out,
                    PushKeyboardEnhancementFlags(
                        KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                            | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    )
                )?;
                writeln!(
                    out,
                    "key debug: try shift+enter / shift+a / esc; ctrl+c quits\r\nkeyboard flags pushed"
                )?;
                out.flush()?;
                loop {
                    if let Event::Key(key) = event::read()? {
                        writeln!(out, "{:?}  {:?}  {:?}\r", key.code, key.modifiers, key.kind)?;
                        out.flush()?;
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            break;
                        }
                    }
                }
                execute!(&mut out, PopKeyboardEnhancementFlags)
            })();
            disable_raw_mode()?;
            result?;
            return Ok(());
        }
        Some("--capture") if args.len() == 1 => {}
        None => {}
        _ => return Err(lang.unknown_arg().into()),
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(lang.need_tty().into());
    }    let mut terminal = ratatui::try_init()?;
    let result = (|| -> io::Result<()> {
        execute!(
            io::stdout(),
            EnableMouseCapture,
            EnableBracketedPaste,
            // kitty 键盘协议：REPORT_ALL_KEYS 让 Shift+Enter 等组合键以 CSI u 上报；
            // REPORT_ALTERNATE_KEYS 让 shift+字母/符号按布局还原成实际字符。
            // 不支持该协议的终端会忽略这两个序列，退回传统行为。
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
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
