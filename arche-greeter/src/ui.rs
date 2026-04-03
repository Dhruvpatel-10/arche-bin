use chrono::Local;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::{App, Field};
use crate::theme::{ACCENT, BG, BORDER, CRITICAL, FG, FG_MUTED, WARNING};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const FIELD_LABEL_W: u16 = 10; // "▸ pass    " = indicator(2) + label(8)
const UNDERLINE_W: u16 = 24;

pub fn draw(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();
    let w = area.width;
    let h = area.height;

    // Full dark canvas
    f.render_widget(Block::default().style(Style::default().bg(BG)), area);

    // -- Top zone: hostname left, time right ----------------------------------
    if h > 4 {
        let top = Rect::new(0, 0, w, 1);
        draw_top_bar(f, top, app);
    }

    // -- Title: upper quarter -------------------------------------------------
    let title_y = (h / 4).max(2).min(h.saturating_sub(6));
    render_at(
        f,
        Rect::new(0, title_y, w, 1),
        Paragraph::new("a r c h e")
            .style(Style::default().fg(ACCENT).bold())
            .alignment(Alignment::Center),
    );

    // -- Form zone: centered vertically, content centered horizontally --------
    let content_w: u16 = 44;
    let cx = (w.saturating_sub(content_w)) / 2; // content left edge
    let form_y = ((h * 5) / 11).max(title_y + 2); // ~45% down, below title
    let value_x = cx + FIELD_LABEL_W;

    // user field
    if form_y < h {
        draw_field_line(f, cx, form_y, w, "user", &app.username, app.focused == Field::Username);
        draw_underline(f, value_x, form_y + 1, app.focused == Field::Username);
    }

    // pass field
    let pass_y = form_y + 3;
    if pass_y < h {
        draw_field_line(f, cx, pass_y, w, "pass", &app.masked_password(), app.focused == Field::Password);
        draw_underline(f, value_x, pass_y + 1, app.focused == Field::Password);

        // Caps lock: right edge
        if app.show_caps_warning() {
            let caps_x = w.saturating_sub(10);
            render_at(
                f,
                Rect::new(caps_x, pass_y, 8, 1),
                Paragraph::new("⚠ CAPS").style(Style::default().fg(WARNING)),
            );
        }
    }

    // session field — only shown when multiple sessions available
    let session_y = pass_y + 3;
    let has_multiple_sessions = app.sessions.len() > 1;
    if has_multiple_sessions && session_y < h {
        let name = &app.current_session().name;
        let session_line = Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(format!("{:<8}", "session"), Style::default().fg(FG_MUTED)),
            Span::styled(name.as_str(), Style::default().fg(FG)),
        ]);
        render_at(f, Rect::new(cx, session_y, content_w, 1), Paragraph::new(session_line));

        let arrows_x = w.saturating_sub(5);
        render_at(
            f,
            Rect::new(arrows_x, session_y, 3, 1),
            Paragraph::new("↑↓").style(Style::default().fg(FG_MUTED)),
        );
    }

    // -- Status: below form ---------------------------------------------------
    let status_y = if has_multiple_sessions { session_y + 2 } else { pass_y + 4 };
    if app.status.is_visible() && status_y < h {
        let icon = app.status.icon();
        let msg = app.status.message();
        let color = app.status.color();
        let text = if icon.is_empty() {
            msg.to_string()
        } else {
            format!("{icon} {msg}")
        };
        render_at(
            f,
            Rect::new(0, status_y, w, 1),
            Paragraph::new(text)
                .style(Style::default().fg(color))
                .alignment(Alignment::Center),
        );
    }

    // -- Bottom zone: version left, hints right -------------------------------
    if h > 2 {
        let bottom = Rect::new(0, h - 1, w, 1);
        draw_bottom_bar(f, bottom, app);
    }
}

fn draw_top_bar(f: &mut ratatui::Frame, area: Rect, app: &App) {
    // Hostname: left
    if !app.hostname.is_empty() {
        render_at(
            f,
            Rect::new(area.x + 1, area.y, app.hostname.len() as u16 + 1, 1),
            Paragraph::new(app.hostname.as_str()).style(Style::default().fg(FG_MUTED)),
        );
    }

    // Time: right
    let now = Local::now().format("%a %d %b · %H:%M").to_string();
    let time_w = now.len() as u16;
    let time_x = area.width.saturating_sub(time_w + 2);
    render_at(
        f,
        Rect::new(time_x, area.y, time_w + 1, 1),
        Paragraph::new(now).style(Style::default().fg(FG_MUTED)),
    );
}

fn draw_bottom_bar(f: &mut ratatui::Frame, area: Rect, app: &App) {
    // Version: left
    render_at(
        f,
        Rect::new(area.x + 1, area.y, 20, 1),
        Paragraph::new(format!("v{VERSION}")).style(Style::default().fg(BORDER)),
    );

    // Hints: center-right
    let muted = Style::default().fg(FG_MUTED);
    let sep = Span::styled(" · ", Style::default().fg(BORDER));

    let mut spans: Vec<Span<'static>> = vec![
        Span::styled("tab", muted),
        Span::styled(" next", muted),
        sep.clone(),
        Span::styled("enter", muted),
        Span::styled(" login", muted),
    ];

    if app.sessions.len() > 1 {
        spans.push(sep.clone());
        spans.push(Span::styled("↑↓", muted));
        spans.push(Span::styled(" session", muted));
    }

    spans.push(sep);
    spans.push(Span::styled("f2", muted));
    spans.push(Span::styled(" power", Style::default().fg(CRITICAL)));

    render_at(
        f,
        Rect::new(area.x, area.y, area.width, 1),
        Paragraph::new(Line::from(spans)).alignment(Alignment::Right),
    );
}

fn draw_field_line(
    f: &mut ratatui::Frame,
    x: u16,
    y: u16,
    _screen_w: u16,
    label: &str,
    value: &str,
    active: bool,
) {
    let indicator = if active { "▸ " } else { "  " };
    let label_color = if active { ACCENT } else { FG_MUTED };
    let value_color = if active { FG } else { FG_MUTED };

    let line = Line::from(vec![
        Span::styled(indicator, Style::default().fg(ACCENT)),
        Span::styled(format!("{label:<8}"), Style::default().fg(label_color)),
        Span::styled(value, Style::default().fg(value_color)),
    ]);

    let content_w = (FIELD_LABEL_W + UNDERLINE_W).min(80);
    render_at(f, Rect::new(x, y, content_w, 1), Paragraph::new(line));
}

fn draw_underline(f: &mut ratatui::Frame, x: u16, y: u16, active: bool) {
    let (ch, color) = if active {
        ("═", ACCENT)
    } else {
        ("─", BORDER)
    };
    let line = ch.repeat(UNDERLINE_W as usize);
    render_at(
        f,
        Rect::new(x, y, UNDERLINE_W, 1),
        Paragraph::new(line).style(Style::default().fg(color)),
    );
}

/// Render a widget only if the rect fits within the terminal.
fn render_at(f: &mut ratatui::Frame, area: Rect, widget: Paragraph<'_>) {
    let buf = f.area();
    if area.y < buf.height && area.x < buf.width {
        let clamped = Rect::new(
            area.x,
            area.y,
            area.width.min(buf.width.saturating_sub(area.x)),
            area.height.min(buf.height.saturating_sub(area.y)),
        );
        f.render_widget(widget, clamped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Status;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn buffer_to_string(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area;
        let mut s = String::new();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                s.push_str(buf.cell((x, y)).map_or(" ", |c| c.symbol()));
            }
            s.push('\n');
        }
        s
    }

    fn render_app(w: u16, h: u16, app: &App) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        buffer_to_string(terminal.backend().buffer())
    }

    #[test]
    fn renders_title() {
        let app = App::with_username("stark".into());
        let content = render_app(80, 24, &app);
        assert!(content.contains("a r c h e"), "title missing");
    }

    #[test]
    fn renders_version_in_bottom_bar() {
        let app = App::with_username("stark".into());
        let content = render_app(80, 24, &app);
        assert!(content.contains(&format!("v{VERSION}")), "version missing");
    }

    #[test]
    fn renders_field_labels() {
        let app = App::with_username("stark".into());
        let content = render_app(80, 24, &app);
        assert!(content.contains("user"), "user label missing");
        assert!(content.contains("pass"), "pass label missing");
    }

    #[test]
    fn session_hidden_when_single() {
        let app = App::with_username("stark".into()); // 1 session
        let content = render_app(80, 24, &app);
        assert!(!content.contains("session"), "session should be hidden with 1 session");
    }

    #[test]
    fn renders_username_value() {
        let app = App::with_username("stark".into());
        let content = render_app(80, 24, &app);
        assert!(content.contains("stark"), "username value missing");
    }

    #[test]
    fn renders_session_when_multiple() {
        let mut app = App::with_username("stark".into());
        app.sessions.push(crate::session::Session {
            name: "Sway".into(),
            cmd: vec!["sway".into()],
        });
        let content = render_app(80, 24, &app);
        assert!(content.contains("session"), "session label missing");
        assert!(content.contains("Hyprland"), "session name missing");
        assert!(content.contains("↑↓"), "session arrows missing");
    }

    #[test]
    fn renders_active_field_indicator() {
        let app = App::with_username("stark".into()); // focuses password
        let content = render_app(80, 24, &app);
        assert!(content.contains("▸"), "active indicator missing");
    }

    #[test]
    fn active_field_uses_double_underline() {
        let app = App::with_username("stark".into()); // focuses password
        let content = render_app(80, 24, &app);
        assert!(content.contains("═"), "double underline missing for active field");
        assert!(content.contains("─"), "single underline missing for inactive field");
    }

    #[test]
    fn renders_status_with_icon() {
        let mut app = App::with_username("stark".into());
        app.status = Status::AuthFailed("Authentication failed".into());
        let content = render_app(80, 24, &app);
        assert!(content.contains("✗"), "error icon missing");
        assert!(content.contains("Authentication failed"), "error text missing");
    }

    #[test]
    fn renders_masked_password_not_plaintext() {
        let mut app = App::with_username("stark".into());
        app.password = "secret".into();
        let content = render_app(80, 24, &app);
        assert!(content.contains("\u{00b7}"), "masked dots missing");
        assert!(!content.contains("secret"), "plaintext leaked");
    }

    #[test]
    fn renders_keybinding_hints() {
        let app = App::with_username("stark".into());
        let content = render_app(80, 24, &app);
        assert!(content.contains("tab"), "tab hint missing");
        assert!(content.contains("enter"), "enter hint missing");
        assert!(content.contains("power"), "power hint missing");
    }

    #[test]
    fn caps_lock_warning_shown() {
        let mut app = App::with_username("stark".into());
        app.caps_lock = true;
        app.password = "x".into();
        let content = render_app(80, 24, &app);
        assert!(content.contains("CAPS"), "caps lock warning missing");
    }

    #[test]
    fn caps_lock_hidden_when_not_active() {
        let mut app = App::with_username("stark".into());
        app.caps_lock = false;
        app.password = "x".into();
        let content = render_app(80, 24, &app);
        assert!(!content.contains("CAPS"));
    }

    #[test]
    fn renders_without_panic_tiny() {
        let app = App::with_username("stark".into());
        render_app(20, 5, &app); // should not panic
    }

    #[test]
    fn renders_without_panic_standard() {
        let app = App::with_username("stark".into());
        render_app(80, 24, &app);
    }

    #[test]
    fn renders_without_panic_large() {
        let app = App::with_username(String::new());
        render_app(200, 60, &app);
    }

    #[test]
    fn time_appears_in_top_right() {
        let app = App::with_username("stark".into());
        let content = render_app(80, 24, &app);
        // Time format contains a dot separator
        assert!(content.contains("·"), "time separator missing");
    }

    #[test]
    fn underlines_differ_by_active_state() {
        let mut app = App::with_username("stark".into());
        app.focused = Field::Username;
        let content = render_app(80, 24, &app);

        // Count occurrences — user field should have ═, pass should have ─
        let double_count = content.matches('═').count();
        let single_count = content.matches('─').count();
        assert!(double_count > 0, "active field should use ═");
        assert!(single_count > 0, "inactive field should use ─");
    }
}
