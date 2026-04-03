mod app;
mod error;
mod ipc;
mod session;
mod theme;
mod ui;

use std::io;
use std::process::Command;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use app::{App, Status};
use error::GreeterError;

fn main() -> Result<(), GreeterError> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    while app.running {
        app.refresh_caps_lock();
        terminal.draw(|f| ui::draw(f, &app))?;

        if event::poll(Duration::from_secs(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::F(2) => {
                        let _ = Command::new("loginctl").arg("poweroff").spawn();
                    }
                    KeyCode::Tab | KeyCode::BackTab => app.toggle_field(),
                    KeyCode::Up => app.prev_session(),
                    KeyCode::Down => app.next_session(),
                    KeyCode::Enter => match ipc::connect() {
                        Ok(mut conn) => app.submit(&mut conn),
                        Err(_) => {
                            app.status = Status::ConnectionError(
                                "Login service unavailable".into(),
                            )
                        }
                    },
                    KeyCode::Backspace => app.handle_backspace(),
                    KeyCode::Char(c) => app.handle_char(c),
                    _ => {}
                }
            }
        }
    }

    execute!(terminal.backend_mut(), LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;
    Ok(())
}
