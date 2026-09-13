mod app;
mod brew;
mod model;
mod view;

use std::{
    io::{self, Write},
    sync::{Arc, atomic::AtomicBool, mpsc},
    time::Duration,
};

use app::App;
use crossterm::{
    cursor::{SetCursorStyle, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    style::ResetColor,
    terminal::{
        EnableLineWrap, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use signal_hook::{SigId, consts::SIGINT, flag};

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture) {
            restore_terminal();
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = write_terminal_restore(&mut io::stdout());
}

fn write_terminal_restore(writer: &mut impl Write) -> io::Result<()> {
    execute!(
        writer,
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show,
        SetCursorStyle::DefaultUserShape,
        EnableLineWrap,
        ResetColor
    )
}

fn install_sigint_handler() -> io::Result<(SigId, Arc<AtomicBool>)> {
    let seen = Arc::new(AtomicBool::new(false));
    let handler = flag::register(SIGINT, Arc::clone(&seen))?;
    Ok((handler, seen))
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
    if !brew_is_available() {
        return Err("Homebrew is not available in PATH".into());
    }

    let (_sigint_handler, _sigint_seen) = install_sigint_handler()?;

    let (tx, rx) = mpsc::channel();
    let mut app = App::new(tx.clone());
    if let Some(catalog) = brew::cached_catalog() {
        app.use_cached_catalog(catalog);
    }
    brew::load_catalog(tx, false);

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

fn brew_is_available() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|path| path.join("brew").is_file()))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::{install_sigint_handler, write_terminal_restore};

    #[test]
    fn terminal_restore_resets_screen_cursor_and_line_wrapping() {
        let mut output = Vec::new();
        write_terminal_restore(&mut output).expect("terminal restore commands should render");

        let output = String::from_utf8(output).expect("terminal commands should be UTF-8");
        assert!(output.contains("\x1b[?1049l"));
        assert!(output.contains("\x1b[?25h"));
        assert!(output.contains("\x1b[0 q"));
        assert!(output.contains("\x1b[?7h"));
        assert!(output.contains("\x1b[0m"));
    }

    #[test]
    fn sigint_is_captured_instead_of_terminating() {
        let (handler, seen) = install_sigint_handler().expect("SIGINT handler should install");
        signal_hook::low_level::raise(signal_hook::consts::SIGINT)
            .expect("SIGINT should be delivered");

        for _ in 0..100 {
            if seen.load(Ordering::Relaxed) {
                break;
            }
            std::thread::yield_now();
        }
        signal_hook::low_level::unregister(handler);
        assert!(seen.load(Ordering::Relaxed));
    }
}
