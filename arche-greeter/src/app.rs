use std::fs;

use ratatui::style::Color;

use crate::ipc::{self, AuthOutcome, GreetdConnection};
use crate::session::{self, Session};
use crate::theme::{CRITICAL, FG_MUTED, SUCCESS};

const LAST_USER_PATH: &str = "/tmp/arche-greeter-last-user";

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Field {
    Username,
    Password,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Status {
    Idle,
    Authenticating,
    SessionStarted,
    EmptyUsername,
    AuthFailed(String),
    SessionError(String),
    ConnectionError(String),
}

impl Status {
    pub fn message(&self) -> &str {
        match self {
            Status::Idle => "",
            Status::Authenticating => "Authenticating…",
            Status::SessionStarted => "Logging in…",
            Status::EmptyUsername => "Enter a username",
            Status::AuthFailed(msg) => msg,
            Status::SessionError(msg) => msg,
            Status::ConnectionError(msg) => msg,
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Status::Idle => "",
            Status::Authenticating => "◌",
            Status::SessionStarted => "✓",
            Status::EmptyUsername | Status::AuthFailed(_) | Status::SessionError(_) | Status::ConnectionError(_) => "✗",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Status::Idle | Status::Authenticating => FG_MUTED,
            Status::SessionStarted => SUCCESS,
            Status::EmptyUsername | Status::AuthFailed(_) | Status::SessionError(_) | Status::ConnectionError(_) => CRITICAL,
        }
    }

    pub fn is_visible(&self) -> bool {
        !matches!(self, Status::Idle)
    }
}

pub struct App {
    pub username: String,
    pub password: String,
    pub focused: Field,
    pub status: Status,
    pub running: bool,
    pub sessions: Vec<Session>,
    pub selected_session: usize,
    pub caps_lock: bool,
    pub hostname: String,
}

impl App {
    pub fn new() -> Self {
        let username = fs::read_to_string(LAST_USER_PATH)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(detect_sole_user)
            .unwrap_or_default();

        let sessions = session::detect();
        let selected_session = session::load_last_index(&sessions);
        let hostname = fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let focused = if username.is_empty() {
            Field::Username
        } else {
            Field::Password
        };

        Self {
            username,
            password: String::new(),
            focused,
            status: Status::Idle,
            running: true,
            sessions,
            selected_session,
            caps_lock: false,
            hostname,
        }
    }

    /// Test-friendly constructor with minimal state.
    #[cfg(test)]
    pub fn with_username(username: String) -> Self {
        let focused = if username.is_empty() {
            Field::Username
        } else {
            Field::Password
        };
        Self {
            username,
            password: String::new(),
            focused,
            status: Status::Idle,
            running: true,
            sessions: vec![Session {
                name: "Hyprland".into(),
                cmd: vec!["Hyprland".into()],
            }],
            selected_session: 0,
            caps_lock: false,
            hostname: String::new(),
        }
    }

    pub fn toggle_field(&mut self) {
        self.focused = match self.focused {
            Field::Username => Field::Password,
            Field::Password => Field::Username,
        };
    }

    pub fn handle_char(&mut self, c: char) {
        self.active_field_mut().push(c);
    }

    pub fn handle_backspace(&mut self) {
        self.active_field_mut().pop();
    }

    fn active_field_mut(&mut self) -> &mut String {
        match self.focused {
            Field::Username => &mut self.username,
            Field::Password => &mut self.password,
        }
    }

    pub fn next_session(&mut self) {
        if !self.sessions.is_empty() {
            self.selected_session = (self.selected_session + 1) % self.sessions.len();
        }
    }

    pub fn prev_session(&mut self) {
        if !self.sessions.is_empty() {
            self.selected_session = self
                .selected_session
                .checked_sub(1)
                .unwrap_or(self.sessions.len() - 1);
        }
    }

    pub fn current_session(&self) -> &Session {
        &self.sessions[self.selected_session]
    }

    /// Validate and attempt login via the provided connection.
    pub fn submit(&mut self, conn: &mut dyn GreetdConnection) {
        if self.username.is_empty() {
            self.status = Status::EmptyUsername;
            return;
        }

        self.status = Status::Authenticating;
        let _ = fs::write(LAST_USER_PATH, &self.username);
        session::save_last(self.current_session());

        let cmd = self.current_session().cmd.clone();
        match ipc::authenticate(conn, &self.username, &self.password, &cmd) {
            Ok(outcome) => self.apply_auth_outcome(outcome),
            Err(_) => {
                self.status = Status::ConnectionError("Login service unavailable".into())
            }
        }
    }

    pub fn apply_auth_outcome(&mut self, outcome: AuthOutcome) {
        match outcome {
            AuthOutcome::SessionStarted => {
                self.status = Status::SessionStarted;
                self.running = false;
            }
            AuthOutcome::AuthFailed(_) => {
                self.status = Status::AuthFailed("Authentication failed".into());
                self.password.clear();
                self.focused = Field::Password;
            }
            AuthOutcome::SessionError(_) => {
                self.status = Status::SessionError("Session failed to start".into());
            }
        }
    }

    pub fn masked_password(&self) -> String {
        "\u{00b7}".repeat(self.password.chars().count())
    }

    /// Refresh caps lock state from sysfs.
    pub fn refresh_caps_lock(&mut self) {
        self.caps_lock = read_caps_lock();
    }

    pub fn show_caps_warning(&self) -> bool {
        self.caps_lock && self.focused == Field::Password && !self.password.is_empty()
    }
}

fn read_caps_lock() -> bool {
    let Ok(entries) = fs::read_dir("/sys/class/leds") else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.contains("capslock") {
            if let Ok(brightness) = fs::read_to_string(entry.path().join("brightness")) {
                return brightness.trim() == "1";
            }
        }
    }
    false
}

/// If exactly one human user exists (UID 1000–65533, real login shell), return it.
fn detect_sole_user() -> Option<String> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    sole_human_user(&passwd)
}

/// Parse passwd content and return the username if exactly one human user exists.
fn sole_human_user(passwd: &str) -> Option<String> {
    let users: Vec<&str> = passwd
        .lines()
        .filter_map(|line| {
            let mut fields = line.splitn(7, ':');
            let name = fields.next()?;
            let _pass = fields.next()?;
            let uid: u32 = fields.next()?.parse().ok()?;
            let _gid = fields.next()?;
            let _gecos = fields.next()?;
            let _home = fields.next()?;
            let shell = fields.next()?;
            if uid >= 1000
                && uid < 65534
                && !shell.ends_with("nologin")
                && !shell.ends_with("false")
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    if users.len() == 1 {
        Some(users[0].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::GreeterError;
    use greetd_ipc::{Request, Response};
    use std::collections::VecDeque;

    // -- Test helpers ---------------------------------------------------------

    struct MockConnection {
        responses: VecDeque<Response>,
    }

    impl MockConnection {
        fn new(responses: Vec<Response>) -> Self {
            Self { responses: VecDeque::from(responses) }
        }
    }

    impl GreetdConnection for MockConnection {
        fn send(&mut self, _req: Request) -> Result<(), GreeterError> {
            Ok(())
        }
        fn recv(&mut self) -> Result<Response, GreeterError> {
            self.responses
                .pop_front()
                .ok_or_else(|| GreeterError::Ipc("no more responses".into()))
        }
    }

    struct FailingConnection;

    impl GreetdConnection for FailingConnection {
        fn send(&mut self, _: Request) -> Result<(), GreeterError> {
            Err(GreeterError::Ipc("connection refused".into()))
        }
        fn recv(&mut self) -> Result<Response, GreeterError> {
            Err(GreeterError::Ipc("not connected".into()))
        }
    }

    // -- Construction ---------------------------------------------------------

    #[test]
    fn empty_username_focuses_username_field() {
        let app = App::with_username(String::new());
        assert_eq!(app.focused, Field::Username);
        assert!(app.username.is_empty());
        assert!(app.password.is_empty());
        assert!(app.running);
        assert_eq!(app.status, Status::Idle);
    }

    #[test]
    fn saved_username_focuses_password_field() {
        let app = App::with_username("stark".into());
        assert_eq!(app.focused, Field::Password);
        assert_eq!(app.username, "stark");
    }

    #[test]
    fn default_has_one_session() {
        let app = App::with_username(String::new());
        assert!(!app.sessions.is_empty());
        assert_eq!(app.selected_session, 0);
    }

    // -- Field toggling -------------------------------------------------------

    #[test]
    fn toggle_cycles_between_fields() {
        let mut app = App::with_username(String::new());
        assert_eq!(app.focused, Field::Username);
        app.toggle_field();
        assert_eq!(app.focused, Field::Password);
        app.toggle_field();
        assert_eq!(app.focused, Field::Username);
    }

    // -- Character input ------------------------------------------------------

    #[test]
    fn char_input_targets_focused_field() {
        let mut app = App::with_username(String::new());
        app.handle_char('a');
        app.handle_char('b');
        assert_eq!(app.username, "ab");
        assert!(app.password.is_empty());

        app.toggle_field();
        app.handle_char('x');
        assert_eq!(app.password, "x");
        assert_eq!(app.username, "ab");
    }

    #[test]
    fn unicode_input_handled() {
        let mut app = App::with_username(String::new());
        app.handle_char('ä');
        app.handle_char('日');
        assert_eq!(app.username, "ä日");
    }

    // -- Backspace ------------------------------------------------------------

    #[test]
    fn backspace_removes_last_char() {
        let mut app = App::with_username(String::new());
        app.handle_char('a');
        app.handle_char('b');
        app.handle_backspace();
        assert_eq!(app.username, "a");
    }

    #[test]
    fn backspace_on_empty_field_is_noop() {
        let mut app = App::with_username(String::new());
        app.handle_backspace();
        assert!(app.username.is_empty());
    }

    // -- Session cycling ------------------------------------------------------

    #[test]
    fn next_session_wraps_around() {
        let mut app = App::with_username(String::new());
        app.sessions = vec![
            Session { name: "A".into(), cmd: vec!["a".into()] },
            Session { name: "B".into(), cmd: vec!["b".into()] },
            Session { name: "C".into(), cmd: vec!["c".into()] },
        ];
        app.selected_session = 0;

        app.next_session();
        assert_eq!(app.selected_session, 1);
        app.next_session();
        assert_eq!(app.selected_session, 2);
        app.next_session();
        assert_eq!(app.selected_session, 0); // wrap
    }

    #[test]
    fn prev_session_wraps_around() {
        let mut app = App::with_username(String::new());
        app.sessions = vec![
            Session { name: "A".into(), cmd: vec!["a".into()] },
            Session { name: "B".into(), cmd: vec!["b".into()] },
        ];
        app.selected_session = 0;

        app.prev_session();
        assert_eq!(app.selected_session, 1); // wrap to end
        app.prev_session();
        assert_eq!(app.selected_session, 0);
    }

    #[test]
    fn current_session_returns_selected() {
        let mut app = App::with_username(String::new());
        app.sessions = vec![
            Session { name: "Hyprland".into(), cmd: vec!["Hyprland".into()] },
            Session { name: "Sway".into(), cmd: vec!["sway".into()] },
        ];
        app.selected_session = 1;
        assert_eq!(app.current_session().name, "Sway");
    }

    // -- Status ---------------------------------------------------------------

    #[test]
    fn status_idle_is_invisible() {
        assert!(!Status::Idle.is_visible());
    }

    #[test]
    fn status_variants_are_visible() {
        assert!(Status::Authenticating.is_visible());
        assert!(Status::SessionStarted.is_visible());
        assert!(Status::EmptyUsername.is_visible());
        assert!(Status::AuthFailed("x".into()).is_visible());
    }

    #[test]
    fn status_icons_are_correct() {
        assert_eq!(Status::Idle.icon(), "");
        assert_eq!(Status::Authenticating.icon(), "◌");
        assert_eq!(Status::SessionStarted.icon(), "✓");
        assert_eq!(Status::AuthFailed("x".into()).icon(), "✗");
    }

    #[test]
    fn status_colors_are_correct() {
        assert_eq!(Status::Idle.color(), FG_MUTED);
        assert_eq!(Status::Authenticating.color(), FG_MUTED);
        assert_eq!(Status::SessionStarted.color(), SUCCESS);
        assert_eq!(Status::EmptyUsername.color(), CRITICAL);
        assert_eq!(Status::AuthFailed("x".into()).color(), CRITICAL);
    }

    // -- Caps lock warning ----------------------------------------------------

    #[test]
    fn caps_warning_only_when_typing_password() {
        let mut app = App::with_username("stark".into());
        app.caps_lock = true;
        app.password = "x".into();
        assert!(app.show_caps_warning());

        app.focused = Field::Username;
        assert!(!app.show_caps_warning());
    }

    #[test]
    fn caps_warning_hidden_when_password_empty() {
        let mut app = App::with_username("stark".into());
        app.caps_lock = true;
        assert!(!app.show_caps_warning()); // password is empty
    }

    // -- Password masking -----------------------------------------------------

    #[test]
    fn masked_password_uses_middle_dot() {
        let mut app = App::with_username(String::new());
        app.toggle_field();
        app.handle_char('a');
        app.handle_char('b');
        assert_eq!(app.masked_password(), "\u{00b7}\u{00b7}");
    }

    #[test]
    fn masked_password_counts_chars_not_bytes() {
        let mut app = App::with_username(String::new());
        app.toggle_field();
        app.handle_char('é');
        app.handle_char('日');
        assert_eq!(app.masked_password().chars().count(), 2);
    }

    // -- Auth outcomes --------------------------------------------------------

    #[test]
    fn session_started_stops_app() {
        let mut app = App::with_username("stark".into());
        app.apply_auth_outcome(AuthOutcome::SessionStarted);
        assert!(!app.running);
        assert_eq!(app.status, Status::SessionStarted);
    }

    #[test]
    fn auth_failed_clears_password_and_refocuses() {
        let mut app = App::with_username("stark".into());
        app.password = "wrong".into();
        app.focused = Field::Username;
        app.apply_auth_outcome(AuthOutcome::AuthFailed("bad password".into()));

        assert!(app.running);
        assert!(app.password.is_empty());
        assert_eq!(app.focused, Field::Password);
        assert!(matches!(app.status, Status::AuthFailed(_)));
    }

    #[test]
    fn session_error_keeps_password() {
        let mut app = App::with_username("stark".into());
        app.password = "pass".into();
        app.apply_auth_outcome(AuthOutcome::SessionError("failed".into()));
        assert_eq!(app.password, "pass");
        assert!(app.running);
    }

    // -- Submit ---------------------------------------------------------------

    #[test]
    fn submit_empty_username() {
        let mut app = App::with_username(String::new());
        let mut conn = MockConnection::new(vec![]);
        app.submit(&mut conn);
        assert_eq!(app.status, Status::EmptyUsername);
    }

    #[test]
    fn submit_success() {
        use greetd_ipc::{AuthMessageType, Response};
        let mut app = App::with_username("stark".into());
        app.toggle_field();
        for c in "pass".chars() { app.handle_char(c); }
        app.toggle_field(); // back to username so focused != password

        let mut conn = MockConnection::new(vec![
            Response::AuthMessage { auth_message_type: AuthMessageType::Secret, auth_message: "Password:".into() },
            Response::Success,
            Response::Success,
        ]);
        app.submit(&mut conn);
        assert!(!app.running);
    }

    #[test]
    fn submit_connection_error() {
        let mut app = App::with_username("stark".into());
        app.submit(&mut FailingConnection);
        assert!(matches!(app.status, Status::ConnectionError(_)));
    }

    // -- User detection -------------------------------------------------------

    #[test]
    fn sole_user_detected() {
        let passwd = "root:x:0:0::/root:/bin/bash\nstark:x:1000:1000::/home/stark:/bin/bash\n";
        assert_eq!(sole_human_user(passwd), Some("stark".into()));
    }

    #[test]
    fn sole_user_none_when_multiple() {
        let passwd = "alice:x:1000:1000::/home/alice:/bin/bash\nbob:x:1001:1001::/home/bob:/bin/zsh\n";
        assert_eq!(sole_human_user(passwd), None);
    }

    #[test]
    fn sole_user_none_when_no_humans() {
        let passwd = "root:x:0:0::/root:/bin/bash\nnobody:x:65534:65534::/:/usr/bin/nologin\n";
        assert_eq!(sole_human_user(passwd), None);
    }

    // -- Integration ----------------------------------------------------------

    #[test]
    fn full_input_flow() {
        let mut app = App::with_username(String::new());
        for c in "stark".chars() { app.handle_char(c); }
        app.toggle_field();
        for c in "hunter2".chars() { app.handle_char(c); }
        assert_eq!(app.username, "stark");
        assert_eq!(app.password, "hunter2");
        assert_eq!(app.masked_password(), "\u{00b7}".repeat(7));
    }

    #[test]
    fn auth_failure_then_retry() {
        let mut app = App::with_username("stark".into());
        for c in "wrong".chars() { app.handle_char(c); }
        app.apply_auth_outcome(AuthOutcome::AuthFailed("bad".into()));
        assert!(app.password.is_empty());
        assert_eq!(app.focused, Field::Password);

        for c in "correct".chars() { app.handle_char(c); }
        assert_eq!(app.password, "correct");
        app.apply_auth_outcome(AuthOutcome::SessionStarted);
        assert!(!app.running);
    }
}
