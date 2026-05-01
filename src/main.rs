mod app;
mod tree;
mod ui;

use std::fs::File;
use std::io::{self, BufReader, IsTerminal, Read};
#[cfg(unix)]
extern crate libc;
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{App, Mode};

#[derive(Parser)]
#[command(name = "xmlv", about = "Interactive XML viewer", version)]
struct Cli {
    /// XML file to view (reads stdin if omitted)
    file: Option<std::path::PathBuf>,

    /// Show closing tags (hidden by default)
    #[arg(long)]
    all: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let xml_content = match cli.file {
        Some(path) => {
            let f = File::open(&path)
                .map_err(|e| format!("Cannot open '{}': {e}", path.display()))?;
            let mut s = String::new();
            BufReader::new(f).read_to_string(&mut s)?;
            s
        }
        None => {
            if io::stdin().is_terminal() {
                eprintln!("Usage: xmlv [file.xml]");
                eprintln!("       echo '<foo/>' | xmlv");
                std::process::exit(1);
            }
            let mut s = String::new();
            io::stdin().read_to_string(&mut s)?;
            // stdin was a pipe; reopen /dev/tty as fd 0 so crossterm can read keyboard events
            reopen_tty_as_stdin()
                .map_err(|e| format!("Cannot reopen /dev/tty for input: {e}"))?;
            s
        }
    };

    let nodes = tree::parse(xml_content.as_bytes(), cli.all).map_err(|e| e)?;

    if nodes.is_empty() {
        eprintln!("No XML nodes found.");
        std::process::exit(1);
    }

    let mut app = App::new(nodes);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result?;
    Ok(())
}

#[cfg(unix)]
fn reopen_tty_as_stdin() -> io::Result<()> {
    use std::ffi::CStr;
    use std::fs::OpenOptions;
    use std::os::unix::io::IntoRawFd;

    // On macOS, kqueue can't register a freshly-opened /dev/tty fd (returns EINVAL),
    // but it can register the actual pty path (e.g. /dev/ttys001) that the shell
    // gave to stderr/stdout. Resolve it via ttyname.
    let candidate_fds = [libc::STDERR_FILENO, libc::STDOUT_FILENO];
    let mut tty_path: Option<std::path::PathBuf> = None;
    for fd in candidate_fds {
        if unsafe { libc::isatty(fd) } != 1 {
            continue;
        }
        let ptr = unsafe { libc::ttyname(fd) };
        if ptr.is_null() {
            continue;
        }
        let cstr = unsafe { CStr::from_ptr(ptr) };
        if let Ok(s) = cstr.to_str() {
            tty_path = Some(s.into());
            break;
        }
    }
    let tty_path = tty_path.unwrap_or_else(|| std::path::PathBuf::from("/dev/tty"));

    let tty = OpenOptions::new().read(true).write(true).open(&tty_path)?;
    let tty_fd = tty.into_raw_fd();
    let ret = unsafe { libc::dup2(tty_fd, libc::STDIN_FILENO) };
    let dup_err = if ret == -1 { Some(io::Error::last_os_error()) } else { None };
    unsafe { libc::close(tty_fd) };
    if let Some(e) = dup_err {
        return Err(e);
    }
    Ok(())
}

#[cfg(not(unix))]
fn reopen_tty_as_stdin() -> io::Result<()> {
    Ok(()) // no-op on Windows; crossterm handles this differently there
}

fn run_loop(
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let ev = event::read()?;

        match app.mode {
            Mode::Normal => match ev {
                Event::Key(k) => match (k.code, k.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    (KeyCode::Char('j'), _) | (KeyCode::Down, _) => app.move_down(),
                    (KeyCode::Char('k'), _) | (KeyCode::Up, _) => app.move_up(),
                    (KeyCode::Char('h'), _) | (KeyCode::Left, _) => app.collapse_current(),
                    (KeyCode::Char('l'), _) | (KeyCode::Right, _) => app.expand_current(),
                    (KeyCode::Char(' '), _) | (KeyCode::Enter, _) => app.toggle_collapse(),
                    (KeyCode::Char('d'), KeyModifiers::CONTROL)
                    | (KeyCode::PageDown, _) => app.page_down(),
                    (KeyCode::Char('u'), KeyModifiers::CONTROL)
                    | (KeyCode::PageUp, _) => app.page_up(),
                    (KeyCode::Char('g'), _) | (KeyCode::Home, _) => app.go_top(),
                    (KeyCode::Char('G'), _) | (KeyCode::End, _) => app.go_bottom(),
                    (KeyCode::Char('c'), _) => app.collapse_all(),
                    (KeyCode::Char('e'), _) => app.expand_all(),
                    (KeyCode::Char('/'), _) => app.enter_search(),
                    (KeyCode::Char('n'), _) => app.next_match(),
                    (KeyCode::Char('N'), _) => app.prev_match(),
                    _ => {}
                },
                _ => {}
            },
            Mode::Search => match ev {
                Event::Key(k) => match k.code {
                    KeyCode::Esc => app.cancel_search(),
                    KeyCode::Enter => app.commit_search(),
                    KeyCode::Backspace => app.pop_search_char(),
                    KeyCode::Char(c) => app.push_search_char(c),
                    _ => {}
                },
                _ => {}
            },
        }
    }
}
