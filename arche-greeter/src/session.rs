use std::fs;
use std::path::Path;

const SESSION_DIRS: &[&str] = &["/usr/share/wayland-sessions", "/usr/share/xsessions"];
const LAST_SESSION_PATH: &str = "/tmp/arche-greeter-last-session";

#[derive(Debug, Clone)]
pub struct Session {
    pub name: String,
    pub cmd: Vec<String>,
}

/// Detect available sessions from .desktop files, sorted by name.
pub fn detect() -> Vec<Session> {
    let mut sessions = Vec::new();
    for dir in SESSION_DIRS {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "desktop") {
                if let Some(session) = parse_desktop_file(&path) {
                    sessions.push(session);
                }
            }
        }
    }
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    sessions.dedup_by(|a, b| a.name == b.name);
    if sessions.is_empty() {
        sessions.push(Session {
            name: "Shell".into(),
            cmd: vec![std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into())],
        });
    }
    sessions
}

fn parse_desktop_file(path: &Path) -> Option<Session> {
    let content = fs::read_to_string(path).ok()?;
    parse_desktop_content(&content)
}

/// Parse Name= and Exec= from .desktop file content.
fn parse_desktop_content(content: &str) -> Option<Session> {
    let mut name = None;
    let mut exec = None;
    for line in content.lines() {
        if let Some(n) = line.strip_prefix("Name=") {
            name = Some(n.trim().to_string());
        } else if let Some(e) = line.strip_prefix("Exec=") {
            exec = Some(e.trim().to_string());
        }
    }
    let cmd: Vec<String> = exec?
        .split_whitespace()
        .filter(|s| !s.starts_with('%'))
        .map(String::from)
        .collect();
    if cmd.is_empty() {
        return None;
    }
    Some(Session {
        name: name.unwrap_or_else(|| cmd[0].clone()),
        cmd,
    })
}

/// Find the index of the last used session by name.
pub fn load_last_index(sessions: &[Session]) -> usize {
    fs::read_to_string(LAST_SESSION_PATH)
        .ok()
        .and_then(|saved| {
            let saved = saved.trim();
            sessions.iter().position(|s| s.name == saved)
        })
        .unwrap_or(0)
}

/// Save the session name for next boot.
pub fn save_last(session: &Session) {
    let _ = fs::write(LAST_SESSION_PATH, &session.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_desktop() {
        let content = "\
[Desktop Entry]
Name=Hyprland
Comment=An independent tiling Wayland compositor
Exec=Hyprland
Type=Application
";
        let session = parse_desktop_content(content).unwrap();
        assert_eq!(session.name, "Hyprland");
        assert_eq!(session.cmd, vec!["Hyprland"]);
    }

    #[test]
    fn parse_exec_with_args() {
        let content = "\
[Desktop Entry]
Name=Sway
Exec=sway --unsupported-gpu
";
        let session = parse_desktop_content(content).unwrap();
        assert_eq!(session.name, "Sway");
        assert_eq!(session.cmd, vec!["sway", "--unsupported-gpu"]);
    }

    #[test]
    fn parse_strips_percent_codes() {
        let content = "\
[Desktop Entry]
Name=Firefox
Exec=firefox %u
";
        let session = parse_desktop_content(content).unwrap();
        assert_eq!(session.cmd, vec!["firefox"]);
    }

    #[test]
    fn parse_missing_name_uses_exec() {
        let content = "\
[Desktop Entry]
Exec=startplasma-wayland
";
        let session = parse_desktop_content(content).unwrap();
        assert_eq!(session.name, "startplasma-wayland");
    }

    #[test]
    fn parse_missing_exec_returns_none() {
        let content = "\
[Desktop Entry]
Name=Hyprland
";
        assert!(parse_desktop_content(content).is_none());
    }

    #[test]
    fn parse_empty_exec_returns_none() {
        let content = "\
[Desktop Entry]
Name=Empty
Exec=
";
        assert!(parse_desktop_content(content).is_none());
    }

    #[test]
    fn load_last_index_default_zero() {
        let sessions = vec![
            Session { name: "A".into(), cmd: vec!["a".into()] },
            Session { name: "B".into(), cmd: vec!["b".into()] },
        ];
        // File doesn't exist in test env, should default to 0
        assert_eq!(load_last_index(&sessions), 0);
    }
}
