mod app;
mod brew;
mod model;
mod view;

use std::{io, sync::mpsc, time::Duration};

use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("brewy {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if std::env::args().any(|argument| argument == "--help" || argument == "-h") {
        println!(
            "brewy {}\n\nAn interactive Homebrew package manager.\n\nKeys: type to search, Tab select, Enter run, ? help, Ctrl-Q quit",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if which_brew().is_none() {
        return Err("Homebrew is not available in PATH".into());
    }

    let (tx, rx) = mpsc::channel();
    let mut app = App::new(tx.clone());
    brew::load_catalog(tx);

    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    while !app.should_quit {
        while let Ok(message) = rx.try_recv() {
            app.handle_worker_event(message);
        }
        app.request_selected_detail();
        terminal.draw(|frame| view::draw(frame, &app))?;
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == event::KeyEventKind::Press => app.handle_key(key),
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
    }
    Ok(())
}

fn which_brew() -> Option<()> {
    std::process::Command::new("brew")
        .arg("--prefix")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?
        .success()
        .then_some(())
}
