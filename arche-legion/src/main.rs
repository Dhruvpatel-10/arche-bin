use std::fs;
use std::io::{self, stdout, Write as IoWrite};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{ExecutableCommand, cursor};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Clear, Padding, Paragraph, Row, Table};
use ratatui::{Frame, Terminal};

// ─── Theme ───
// Loaded at runtime from ~/.config/legion/colors.toml (rendered by theme.sh).
// Fallback values match ember theme for when the config doesn't exist yet.

#[derive(Clone, Copy)]
struct Theme {
    base: Color,
    crust: Color,
    surface: Color,
    surface1: Color,
    surface2: Color,
    overlay0: Color,
    overlay1: Color,
    text: Color,
    subtext: Color,
    dim: Color,
    accent: Color,
    accent_alt: Color,
    success: Color,
    warn: Color,
    critical: Color,
    border: Color,
}

impl Theme {
    /// Ember fallback — used when colors.toml doesn't exist yet
    fn fallback() -> Self {
        Self {
            base: Color::Rgb(19, 21, 28),
            crust: Color::Rgb(10, 11, 16),
            surface: Color::Rgb(29, 32, 41),
            surface1: Color::Rgb(40, 44, 56),
            surface2: Color::Rgb(53, 58, 72),
            overlay0: Color::Rgb(82, 88, 102),
            overlay1: Color::Rgb(101, 107, 121),
            text: Color::Rgb(205, 200, 188),
            subtext: Color::Rgb(168, 162, 153),
            dim: Color::Rgb(129, 124, 114),
            accent: Color::Rgb(201, 148, 62),
            accent_alt: Color::Rgb(106, 159, 181),
            success: Color::Rgb(122, 184, 127),
            warn: Color::Rgb(212, 168, 67),
            critical: Color::Rgb(196, 92, 92),
            border: Color::Rgb(40, 44, 56),
        }
    }

    fn load() -> Self {
        let config_path = format!(
            "{}/.config/legion/colors.toml",
            std::env::var("HOME").unwrap_or_else(|_| "/home".into())
        );

        let fallback = Self::fallback();

        let content = match fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => return fallback,
        };

        let table: toml::Table = match content.parse() {
            Ok(t) => t,
            Err(_) => return fallback,
        };

        let colors = match table.get("colors").and_then(|c| c.as_table()) {
            Some(c) => c,
            None => return fallback,
        };

        let get = |key: &str, default: Color| -> Color {
            colors
                .get(key)
                .and_then(|v| v.as_str())
                .and_then(parse_hex_color)
                .unwrap_or(default)
        };

        Self {
            base: get("base", fallback.base),
            crust: get("crust", fallback.crust),
            surface: get("surface", fallback.surface),
            surface1: get("surface1", fallback.surface1),
            surface2: get("surface2", fallback.surface2),
            overlay0: get("overlay0", fallback.overlay0),
            overlay1: get("overlay1", fallback.overlay1),
            text: get("text", fallback.text),
            subtext: get("subtext", fallback.subtext),
            dim: get("dim", fallback.dim),
            accent: get("accent", fallback.accent),
            accent_alt: get("accent_alt", fallback.accent_alt),
            success: get("success", fallback.success),
            warn: get("warn", fallback.warn),
            critical: get("critical", fallback.critical),
            border: get("border", fallback.border),
        }
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

use std::sync::OnceLock;
static THEME: OnceLock<Theme> = OnceLock::new();
fn th() -> &'static Theme {
    THEME.get_or_init(Theme::load)
}

// ─── Sysfs paths ───

const IDEAPAD: &str = "/sys/bus/platform/drivers/ideapad_acpi/VPC2004:00";
const PROFILE_PATH: &str = "/sys/firmware/acpi/platform_profile";
const BAT: &str = "/sys/class/power_supply/BAT0";

// ─── Data model ───

struct Toggle {
    label: &'static str,
    sysfs: &'static str,
    on_label: &'static str,
    off_label: &'static str,
    description: &'static str,
    icon_on: &'static str,
    icon_off: &'static str,
}

const TOGGLES: &[Toggle] = &[
    Toggle {
        label: "Conservation",
        sysfs: "conservation_mode",
        on_label: "ON  cap ~80%",
        off_label: "OFF",
        description: "Limit battery charge to ~80% for longevity",
        icon_on: "󱈏 ",
        icon_off: "󰁹 ",
    },
    Toggle {
        label: "Fan Mode",
        sysfs: "fan_mode",
        on_label: "FULL SPEED",
        off_label: "AUTO",
        description: "Override fan control to maximum speed",
        icon_on: "󰈐 ",
        icon_off: "󰈐 ",
    },
    Toggle {
        label: "Camera",
        sysfs: "camera_power",
        on_label: "ON",
        off_label: "KILLED",
        description: "Hardware camera kill switch",
        icon_on: "󰄀 ",
        icon_off: "󰄀 ",
    },
    Toggle {
        label: "USB Charging",
        sysfs: "usb_charging",
        on_label: "ON",
        off_label: "OFF",
        description: "Charge USB devices while laptop is off",
        icon_on: " ",
        icon_off: " ",
    },
    Toggle {
        label: "Fn Lock",
        sysfs: "fn_lock",
        on_label: "LOCKED",
        off_label: "UNLOCKED",
        description: "Lock function keys as F1\u{2013}F12 primary",
        icon_on: "󰌐 ",
        icon_off: "󰌐 ",
    },
];

const PROFILES: &[&str] = &["low-power", "balanced", "performance", "max-power"];

// ─── Auth state ───

#[derive(Clone, PartialEq)]
enum AuthState {
    Unknown,
    Authenticated,
    Prompting,
    Failed(String),
}

#[derive(Clone)]
enum PendingAction {
    Toggle(usize),
    Profile(usize),
}

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Status,
    Controls,
}

struct App {
    tab: Tab,
    selected: usize,
    profile_idx: usize,
    last_refresh: Instant,
    auth: AuthState,
    password: String,
    pending_action: Option<PendingAction>,
    status_msg: Option<(String, Color, Instant)>,
    toggle_states: Vec<bool>,
    bat_capacity: u32,
    bat_status: String,
    bat_energy_wh: f64,
    bat_full_wh: f64,
    bat_design_wh: f64,
    bat_power_w: f64,
    bat_cycles: u32,
    bat_health: f64,
    cpu_governor: String,
    cpu_epp: String,
    profile: String,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            tab: Tab::Status,
            selected: 0,
            profile_idx: 0,
            last_refresh: Instant::now() - Duration::from_secs(10),
            auth: AuthState::Unknown,
            password: String::new(),
            pending_action: None,
            status_msg: None,
            toggle_states: vec![false; TOGGLES.len()],
            bat_capacity: 0,
            bat_status: String::new(),
            bat_energy_wh: 0.0,
            bat_full_wh: 0.0,
            bat_design_wh: 0.0,
            bat_power_w: 0.0,
            bat_cycles: 0,
            bat_health: 0.0,
            cpu_governor: String::new(),
            cpu_epp: String::new(),
            profile: String::new(),
        };
        app.refresh();
        app.check_sudo_cached();
        app
    }

    fn check_sudo_cached(&mut self) {
        let ok = Command::new("sudo")
            .args(["-n", "true"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success());
        self.auth = if ok {
            AuthState::Authenticated
        } else {
            AuthState::Unknown
        };
    }

    fn try_authenticate(&mut self) -> bool {
        let mut child = match Command::new("sudo")
            .args(["-S", "-v"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => {
                self.auth = AuthState::Failed("Failed to run sudo".into());
                return false;
            }
        };

        if let Some(ref mut stdin) = child.stdin {
            let _ = writeln!(stdin, "{}", self.password);
        }

        let ok = child.wait().is_ok_and(|s| s.success());
        self.password.clear();

        if ok {
            self.auth = AuthState::Authenticated;
            self.set_status("Authenticated", th().success);
            true
        } else {
            self.auth = AuthState::Failed("Wrong password".into());
            false
        }
    }

    fn set_status(&mut self, msg: &str, color: Color) {
        self.status_msg = Some((msg.to_string(), color, Instant::now()));
    }

    fn request_write(&mut self, action: PendingAction) {
        if self.auth == AuthState::Authenticated {
            self.execute_action(action);
        } else {
            self.pending_action = Some(action);
            self.auth = AuthState::Prompting;
            self.password.clear();
        }
    }

    fn execute_action(&mut self, action: PendingAction) {
        match action {
            PendingAction::Toggle(idx) => {
                let t = &TOGGLES[idx];
                let path = format!("{IDEAPAD}/{}", t.sysfs);
                let new_val = if self.toggle_states[idx] { "0" } else { "1" };
                if write_sysfs(&path, new_val) {
                    self.toggle_states[idx] = !self.toggle_states[idx];
                    let state = if self.toggle_states[idx] {
                        t.on_label
                    } else {
                        t.off_label
                    };
                    self.set_status(&format!("{}: {}", t.label, state), th().success);
                } else {
                    self.set_status(&format!("Failed to set {}", t.label), th().critical);
                }
            }
            PendingAction::Profile(idx) => {
                let target = PROFILES[idx];
                if write_sysfs(PROFILE_PATH, target) {
                    self.profile_idx = idx;
                    self.profile = target.to_string();
                    self.set_status(&format!("Profile: {target}"), th().success);
                } else {
                    self.set_status("Failed to set profile", th().critical);
                }
            }
        }
    }

    fn on_auth_submit(&mut self) {
        if self.try_authenticate() {
            if let Some(action) = self.pending_action.take() {
                self.execute_action(action);
            }
        }
    }

    fn on_auth_cancel(&mut self) {
        self.auth = AuthState::Unknown;
        self.password.clear();
        self.pending_action = None;
    }

    fn is_prompting(&self) -> bool {
        matches!(self.auth, AuthState::Prompting | AuthState::Failed(_))
    }

    fn refresh(&mut self) {
        self.last_refresh = Instant::now();

        for (i, t) in TOGGLES.iter().enumerate() {
            self.toggle_states[i] = read_sysfs_u32(&format!("{IDEAPAD}/{}", t.sysfs)) == 1;
        }

        self.profile = read_sysfs(PROFILE_PATH);
        self.profile_idx = PROFILES
            .iter()
            .position(|p| *p == self.profile)
            .unwrap_or(2);

        self.bat_capacity = read_sysfs_u32(&format!("{BAT}/capacity"));
        self.bat_status = read_sysfs(&format!("{BAT}/status"));
        let energy = read_sysfs_u64(&format!("{BAT}/energy_now"));
        let full = read_sysfs_u64(&format!("{BAT}/energy_full"));
        let design = read_sysfs_u64(&format!("{BAT}/energy_full_design"));
        let power = read_sysfs_u64(&format!("{BAT}/power_now"));
        self.bat_energy_wh = energy as f64 / 1_000_000.0;
        self.bat_full_wh = full as f64 / 1_000_000.0;
        self.bat_design_wh = design as f64 / 1_000_000.0;
        self.bat_power_w = power as f64 / 1_000_000.0;
        self.bat_cycles = read_sysfs_u32(&format!("{BAT}/cycle_count"));
        self.bat_health = if design > 0 {
            (full as f64 / design as f64) * 100.0
        } else {
            0.0
        };

        self.cpu_governor =
            read_sysfs("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor");
        self.cpu_epp =
            read_sysfs("/sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference");
    }

    fn max_items(&self) -> usize {
        match self.tab {
            Tab::Status => 0,
            Tab::Controls => TOGGLES.len() + 1,
        }
    }

    fn toggle_selected(&mut self) {
        if self.tab != Tab::Controls || self.selected >= TOGGLES.len() {
            return;
        }
        self.request_write(PendingAction::Toggle(self.selected));
    }

    fn cycle_profile(&mut self, forward: bool) {
        if self.tab != Tab::Controls || self.selected != TOGGLES.len() {
            return;
        }
        let new_idx = if forward {
            (self.profile_idx + 1) % PROFILES.len()
        } else {
            self.profile_idx
                .checked_sub(1)
                .unwrap_or(PROFILES.len() - 1)
        };
        self.request_write(PendingAction::Profile(new_idx));
    }
}

// ─── Sysfs helpers ───

fn read_sysfs(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn read_sysfs_u32(path: &str) -> u32 {
    read_sysfs(path).parse().unwrap_or(0)
}

fn read_sysfs_u64(path: &str) -> u64 {
    read_sysfs(path).parse().unwrap_or(0)
}

fn write_sysfs(path: &str, value: &str) -> bool {
    Command::new("sudo")
        .args(["-n", "tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .and_then(|mut child| {
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(value.as_bytes())?;
            }
            child.wait()
        })
        .is_ok_and(|s| s.success())
}

// ─── Custom gauge rendering ───

/// Renders a smooth battery bar using Unicode block elements.
/// Uses eighth-blocks (▏▎▍▌▋▊▉█) for sub-cell precision.
fn render_battery_bar(width: u16, ratio: f64, fg: Color, bg: Color) -> Line<'static> {
    let blocks = [' ', '\u{258F}', '\u{258E}', '\u{258D}', '\u{258C}', '\u{258B}', '\u{258A}', '\u{2589}', '\u{2588}'];
    let total_eighths = (width as f64 * ratio * 8.0).round() as usize;
    let full_cells = total_eighths / 8;
    let remainder = total_eighths % 8;

    let mut spans = Vec::new();

    // Full cells
    if full_cells > 0 {
        spans.push(Span::styled(
            "\u{2588}".repeat(full_cells),
            Style::new().fg(fg).bg(bg),
        ));
    }

    // Partial cell
    if remainder > 0 && full_cells < width as usize {
        spans.push(Span::styled(
            blocks[remainder].to_string(),
            Style::new().fg(fg).bg(bg),
        ));
    }

    // Empty cells
    let filled = full_cells + if remainder > 0 { 1 } else { 0 };
    if filled < width as usize {
        spans.push(Span::styled(
            " ".repeat(width as usize - filled),
            Style::new().bg(bg),
        ));
    }

    Line::from(spans)
}

// ─── UI rendering ───

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(Block::new().style(Style::new().bg(th().base)), area);

    let [header, body, status_area, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(area);

    draw_header(frame, header, app);

    match app.tab {
        Tab::Status => draw_status(frame, body, app),
        Tab::Controls => draw_controls(frame, body, app),
    }

    draw_status_bar(frame, status_area, app);
    draw_footer(frame, footer, app);

    if app.is_prompting() {
        draw_auth_modal(frame, area, app);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    // Fill header background
    frame.render_widget(
        Block::new().style(Style::new().bg(th().surface)),
        area,
    );

    let [left, center, right] = Layout::horizontal([
        Constraint::Length(30),
        Constraint::Min(0),
        Constraint::Length(20),
    ])
    .areas(area);

    // Device identity — left aligned
    let title = Paragraph::new(Line::from(vec![
        Span::styled("  󰍹 ", Style::new().fg(th().accent)),
        Span::styled("LEGION", Style::new().fg(th().text).bold()),
        Span::styled(" PRO 5", Style::new().fg(th().subtext)),
        Span::styled("  16ARX8", Style::new().fg(th().dim)),
    ]))
    .block(Block::new().padding(Padding::vertical(1)));
    frame.render_widget(title, left);

    // Tab navigation — centered
    let tab_idx = match app.tab {
        Tab::Status => 0,
        Tab::Controls => 1,
    };
    let tab_names = ["  Status", "  Controls"];
    let tab_spans: Vec<Span> = tab_names
        .iter()
        .enumerate()
        .flat_map(|(i, name)| {
            let is_active = i == tab_idx;
            let mut spans = vec![];
            if i > 0 {
                spans.push(Span::styled("  ", Style::new().fg(th().border)));
            }
            if is_active {
                spans.push(Span::styled(
                    format!(" {name} "),
                    Style::new().fg(th().accent).bold(),
                ));
                // Underline indicator via bottom border effect
            } else {
                spans.push(Span::styled(
                    format!(" {name} "),
                    Style::new().fg(th().dim),
                ));
            }
            spans
        })
        .collect();
    let tabs = Paragraph::new(Line::from(tab_spans))
        .alignment(Alignment::Center)
        .block(Block::new().padding(Padding::vertical(1)));
    frame.render_widget(tabs, center);

    // Auth badge — right aligned
    let (auth_icon, auth_label, auth_color) = match &app.auth {
        AuthState::Authenticated => (" ", "unlocked", th().success),
        AuthState::Unknown => (" ", "locked", th().dim),
        AuthState::Prompting => ("󰌾 ", "auth...", th().accent),
        AuthState::Failed(_) => (" ", "failed", th().critical),
    };
    let auth = Paragraph::new(Line::from(vec![
        Span::styled(auth_icon, Style::new().fg(auth_color)),
        Span::styled(auth_label, Style::new().fg(auth_color)),
        Span::raw("  "),
    ]))
    .alignment(Alignment::Right)
    .block(Block::new().padding(Padding::vertical(1)));
    frame.render_widget(auth, right);

    // Bottom accent line
    let accent_line = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    let mut spans = vec![Span::styled(
        "\u{2500}".repeat(area.width as usize),
        Style::new().fg(th().border),
    )];
    // Active tab indicator: a small amber segment
    let tab_center = area.width / 2;
    let indicator_start = if tab_idx == 0 {
        tab_center.saturating_sub(12)
    } else {
        tab_center.saturating_add(2)
    };
    let _ = indicator_start; // We'll use a simpler approach
    spans.clear();
    let w = area.width as usize;
    spans.push(Span::styled("\u{2500}".repeat(w), Style::new().fg(th().border)));
    frame.render_widget(Paragraph::new(Line::from(spans)), accent_line);
}

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let content = area.inner(Margin::new(3, 1));

    let [bat_section, perf_section, switch_section] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Min(0),
    ])
    .areas(content);

    draw_battery_section(frame, bat_section, app);
    draw_performance_section(frame, perf_section, app);
    draw_switches_section(frame, switch_section, app);
}

fn draw_battery_section(frame: &mut Frame, area: Rect, app: &App) {
    let [label_row, _gap, gauge_row, detail_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    // Section header with subtle decoration
    let header = Line::from(vec![
        Span::styled("󰁹 ", Style::new().fg(th().accent)),
        Span::styled("BATTERY", Style::new().fg(th().accent).bold()),
        Span::styled("  \u{2500}\u{2500}\u{2500}", Style::new().fg(th().border)),
    ]);
    frame.render_widget(Paragraph::new(header), label_row);

    // Battery gauge — smooth Unicode bar
    let bat_color = match app.bat_capacity {
        0..=10 => th().critical,
        11..=25 => th().accent,
        _ => th().success,
    };
    let gauge_content = area.inner(Margin::new(1, 0));
    let gauge_width = gauge_content.width.saturating_sub(20);

    // First line: the bar
    let bar_area = Rect::new(gauge_row.x + 1, gauge_row.y, gauge_width, 1);
    let bar = render_battery_bar(gauge_width, app.bat_capacity as f64 / 100.0, bat_color, th().surface1);
    frame.render_widget(Paragraph::new(bar), bar_area);

    // Percentage + status next to bar
    let pct_area = Rect::new(
        gauge_row.x + 1 + gauge_width + 1,
        gauge_row.y,
        18,
        1,
    );
    let status_text = match app.bat_status.as_str() {
        "Charging" => "  charging",
        "Discharging" => "  discharging",
        "Not charging" => "  idle",
        "Full" => "  full",
        _ => &app.bat_status,
    };
    let pct = Line::from(vec![
        Span::styled(
            format!("{}%", app.bat_capacity),
            Style::new().fg(bat_color).bold(),
        ),
        Span::styled(status_text, Style::new().fg(th().dim)),
    ]);
    frame.render_widget(Paragraph::new(pct), pct_area);

    // Detail metrics row
    let detail_area = Rect::new(detail_row.x + 1, detail_row.y, detail_row.width - 1, 1);
    let sep = Span::styled("  \u{00B7}  ", Style::new().fg(th().border));
    let detail = Line::from(vec![
        Span::styled(
            format!("{:.1}", app.bat_energy_wh),
            Style::new().fg(th().subtext),
        ),
        Span::styled(
            format!("/{:.1} Wh", app.bat_full_wh),
            Style::new().fg(th().dim),
        ),
        sep.clone(),
        Span::styled("Health ", Style::new().fg(th().dim)),
        Span::styled(
            format!("{:.1}%", app.bat_health),
            Style::new().fg(if app.bat_health > 90.0 { th().success } else if app.bat_health > 80.0 { th().subtext } else { th().critical }),
        ),
        sep.clone(),
        Span::styled(format!("{}", app.bat_cycles), Style::new().fg(th().subtext)),
        Span::styled(" cycles", Style::new().fg(th().dim)),
        sep,
        Span::styled(
            format!("{:.1}W", app.bat_power_w),
            Style::new().fg(if app.bat_power_w > 30.0 { th().warn } else { th().subtext }),
        ),
        Span::styled(" draw", Style::new().fg(th().dim)),
    ]);
    frame.render_widget(Paragraph::new(detail), detail_area);
}

fn draw_performance_section(frame: &mut Frame, area: Rect, app: &App) {
    let [label_row, _gap, content_row, track_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let header = Line::from(vec![
        Span::styled("󰓅 ", Style::new().fg(th().accent)),
        Span::styled("PERFORMANCE", Style::new().fg(th().accent).bold()),
        Span::styled("  \u{2500}\u{2500}\u{2500}", Style::new().fg(th().border)),
    ]);
    frame.render_widget(Paragraph::new(header), label_row);

    // Profile visual track
    let profile_color = match app.profile.as_str() {
        "low-power" => th().success,
        "balanced" => th().accent_alt,
        "performance" => th().accent,
        "max-power" => th().critical,
        _ => th().dim,
    };

    // Track: visual dots showing position
    let track_area = Rect::new(content_row.x + 1, content_row.y, content_row.width - 1, 1);
    let mut track_spans: Vec<Span> = Vec::new();
    let profile_labels = ["ECO", "BAL", "PERF", "MAX"];
    for (i, label) in profile_labels.iter().enumerate() {
        let is_active = i == app.profile_idx;
        if i > 0 {
            // Connector
            let connector_color = if i <= app.profile_idx { profile_color } else { th().border };
            track_spans.push(Span::styled(" \u{2500}\u{2500}\u{2500} ", Style::new().fg(connector_color)));
        }
        if is_active {
            track_spans.push(Span::styled(
                format!(" {label} "),
                Style::new().fg(th().base).bg(profile_color).bold(),
            ));
        } else {
            track_spans.push(Span::styled(
                format!(" {label} "),
                Style::new().fg(if i < app.profile_idx { profile_color } else { th().dim }),
            ));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(track_spans)), track_area);

    // CPU details
    let detail_area = Rect::new(track_row.x + 1, track_row.y, track_row.width - 1, 1);
    let sep = Span::styled("    ", Style::new());
    let detail = Line::from(vec![
        Span::styled("Governor ", Style::new().fg(th().dim)),
        Span::styled(&app.cpu_governor, Style::new().fg(th().subtext)),
        sep,
        Span::styled("EPP ", Style::new().fg(th().dim)),
        Span::styled(&app.cpu_epp, Style::new().fg(th().subtext)),
    ]);
    frame.render_widget(Paragraph::new(detail), detail_area);
}

fn draw_switches_section(frame: &mut Frame, area: Rect, app: &App) {
    let [label_row, _gap, table_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    let header = Line::from(vec![
        Span::styled("󰔡 ", Style::new().fg(th().accent)),
        Span::styled("SWITCHES", Style::new().fg(th().accent).bold()),
        Span::styled("  \u{2500}\u{2500}\u{2500}", Style::new().fg(th().border)),
    ]);
    frame.render_widget(Paragraph::new(header), label_row);

    let rows: Vec<Row> = TOGGLES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let on = app.toggle_states[i];
            let icon = if on { t.icon_on } else { t.icon_off };
            let (state_label, state_color) = if on {
                (t.on_label, th().success)
            } else {
                (t.off_label, th().dim)
            };

            // Toggle switch visual: ⏽ or similar
            let switch_visual = if on { "◉" } else { "○" };

            Row::new(vec![
                Cell::from(Span::styled(
                    format!("  {icon}"),
                    Style::new().fg(if on { state_color } else { th().border }),
                )),
                Cell::from(Span::styled(t.label, Style::new().fg(th().subtext))),
                Cell::from(Span::styled(
                    switch_visual,
                    Style::new().fg(state_color),
                )),
                Cell::from(Span::styled(
                    state_label,
                    Style::new().fg(state_color).bold(),
                )),
                Cell::from(Span::styled(t.description, Style::new().fg(th().dim))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(16),
            Constraint::Length(2),
            Constraint::Length(14),
            Constraint::Min(20),
        ],
    );
    let table_inner = Rect::new(table_area.x + 1, table_area.y, table_area.width.saturating_sub(1), table_area.height);
    frame.render_widget(table, table_inner);
}

fn draw_controls(frame: &mut Frame, area: Rect, app: &App) {
    let content = area.inner(Margin::new(3, 1));

    let [label_row, _gap, table_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(content);

    let header = Line::from(vec![
        Span::styled("󰒓 ", Style::new().fg(th().accent)),
        Span::styled("CONTROLS", Style::new().fg(th().accent).bold()),
        Span::styled("  \u{2500}\u{2500}\u{2500}", Style::new().fg(th().border)),
    ]);
    frame.render_widget(Paragraph::new(header), label_row);

    let mut rows: Vec<Row> = TOGGLES
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let on = app.toggle_states[i];
            let selected = app.selected == i;
            let icon = if on { t.icon_on } else { t.icon_off };
            let (state_label, state_color) = if on {
                (t.on_label, th().success)
            } else {
                (t.off_label, th().dim)
            };

            let switch_visual = if on { "◉" } else { "○" };
            let indicator_color = if selected { th().accent } else { th().base };

            let row_bg = if selected { th().surface } else { th().base };

            Row::new(vec![
                Cell::from(Span::styled(
                    if selected { " \u{25B8}" } else { "  " },
                    Style::new().fg(indicator_color),
                )),
                Cell::from(Span::styled(
                    format!("{icon}"),
                    Style::new().fg(if on { state_color } else { th().border }),
                )),
                Cell::from(Span::styled(
                    t.label,
                    Style::new().fg(if selected { th().text } else { th().subtext }),
                )),
                Cell::from(Span::styled(
                    switch_visual,
                    Style::new().fg(state_color),
                )),
                Cell::from(Span::styled(
                    state_label,
                    Style::new().fg(state_color).bold(),
                )),
                Cell::from(Span::styled(
                    if selected { "enter to toggle" } else { "" },
                    Style::new().fg(th().border),
                )),
            ])
            .style(Style::new().bg(row_bg))
            .height(2)
        })
        .collect();

    // Profile row
    let profile_selected = app.selected == TOGGLES.len();
    let profile_color = match PROFILES[app.profile_idx] {
        "low-power" => th().success,
        "balanced" => th().accent_alt,
        "performance" => th().accent,
        "max-power" => th().critical,
        _ => th().dim,
    };
    let row_bg = if profile_selected { th().surface } else { th().base };

    rows.push(
        Row::new(vec![
            Cell::from(Span::styled(
                if profile_selected { " \u{25B8}" } else { "  " },
                Style::new().fg(if profile_selected { th().accent } else { th().base }),
            )),
            Cell::from(Span::styled(
                "󰓅 ",
                Style::new().fg(profile_color),
            )),
            Cell::from(Span::styled(
                "Profile",
                Style::new().fg(if profile_selected { th().text } else { th().subtext }),
            )),
            Cell::from(Span::styled(
                "\u{25C0}",
                Style::new().fg(if profile_selected { th().dim } else { th().base }),
            )),
            Cell::from(Span::styled(
                PROFILES[app.profile_idx],
                Style::new().fg(profile_color).bold(),
            )),
            Cell::from(Span::styled(
                if profile_selected { "\u{25B6}  h/l to cycle" } else { "" },
                Style::new().fg(th().border),
            )),
        ])
        .style(Style::new().bg(row_bg))
        .height(2),
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(16),
            Constraint::Length(2),
            Constraint::Length(16),
            Constraint::Min(10),
        ],
    );

    let table_inner = Rect::new(table_area.x + 1, table_area.y, table_area.width.saturating_sub(1), table_area.height);
    frame.render_widget(table, table_inner);
}

fn draw_status_bar(frame: &mut Frame, area: Rect, app: &App) {
    if let Some((ref msg, color, when)) = app.status_msg {
        if when.elapsed() < Duration::from_secs(4) {
            let bar = Paragraph::new(Line::from(vec![
                Span::styled(" \u{2022} ", Style::new().fg(color)),
                Span::styled(msg, Style::new().fg(color)),
            ]))
            .alignment(Alignment::Center);
            frame.render_widget(bar, area);
        }
    }
}

fn draw_auth_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_width = 48;
    let modal_height = 11;

    let modal_area = centered_rect(modal_width, modal_height, area);

    // Dim background
    frame.render_widget(
        Block::new().style(Style::new().bg(th().crust)),
        area,
    );

    frame.render_widget(Clear, modal_area);

    let is_failed = matches!(app.auth, AuthState::Failed(_));
    let border_color = if is_failed { th().critical } else { th().accent };

    let modal_block = Block::new()
        .title(Line::from(vec![
            Span::styled(" 󰌾 ", Style::new().fg(border_color)),
            Span::styled("Authentication ", Style::new().fg(border_color).bold()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border_color))
        .style(Style::new().bg(th().surface2))
        .padding(Padding::new(2, 2, 1, 0));

    let inner = modal_block.inner(modal_area);
    frame.render_widget(modal_block, modal_area);

    let [msg_area, _gap, input_area, _gap2, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    // Message
    let msg = if is_failed {
        Line::from(vec![
            Span::styled(" ", Style::new().fg(th().critical)),
            Span::styled("Wrong password \u{2014} try again", Style::new().fg(th().critical)),
        ])
    } else {
        Line::from(Span::styled(
            "Enter password for sudo access:",
            Style::new().fg(th().subtext),
        ))
    };
    frame.render_widget(Paragraph::new(msg), msg_area);

    // Password input
    let dots: String = "\u{2022}".repeat(app.password.len());
    let cursor_char = "\u{2588}";
    let field_content = format!(" {dots}{cursor_char}");

    let input = Paragraph::new(Line::from(Span::styled(
        &field_content,
        Style::new().fg(th().accent),
    )))
    .block(
        Block::new()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(if is_failed { th().critical } else { th().surface2 }))
            .style(Style::new().bg(th().surface)),
    );
    frame.render_widget(input, input_area);

    // Hints
    let hints = Line::from(vec![
        Span::styled(" Enter ", Style::new().fg(th().surface).bg(th().overlay0)),
        Span::styled(" submit  ", Style::new().fg(th().overlay1)),
        Span::styled(" Esc ", Style::new().fg(th().surface).bg(th().overlay0)),
        Span::styled(" cancel", Style::new().fg(th().overlay1)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App) {
    if app.is_prompting() {
        return;
    }

    // Separator line
    let sep_area = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "\u{2500}".repeat(area.width as usize),
            Style::new().fg(th().border),
        ))),
        sep_area,
    );

    let keys_area = Rect::new(area.x, area.y + 1, area.width, 1);

    let keys = match app.tab {
        Tab::Status => vec![
            ("Tab", "controls"),
            ("r", "refresh"),
            ("q", "quit"),
        ],
        Tab::Controls => vec![
            ("Tab", "status"),
            ("j/k", "navigate"),
            ("Enter", "toggle"),
            ("h/l", "cycle"),
            ("q", "quit"),
        ],
    };

    let spans: Vec<Span> = keys
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(format!(" {key} "), Style::new().fg(th().surface).bg(th().overlay0).bold()),
                Span::styled(format!(" {desc}  "), Style::new().fg(th().overlay1)),
            ]
        })
        .collect();

    let footer = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(footer, keys_area);
}

// ─── Main loop ───

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    stdout().execute(cursor::Hide)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();

    loop {
        if !app.is_prompting() && app.last_refresh.elapsed() > Duration::from_secs(3) {
            app.refresh();
        }

        if let Some((_, _, when)) = &app.status_msg {
            if when.elapsed() > Duration::from_secs(4) {
                app.status_msg = None;
            }
        }

        terminal.draw(|frame| draw(frame, &app))?;

        if event::poll(Duration::from_secs(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.is_prompting() {
                    match key.code {
                        KeyCode::Esc => app.on_auth_cancel(),
                        KeyCode::Enter => app.on_auth_submit(),
                        KeyCode::Backspace => {
                            app.password.pop();
                            if matches!(app.auth, AuthState::Failed(_)) {
                                app.auth = AuthState::Prompting;
                            }
                        }
                        KeyCode::Char(c) => {
                            if matches!(app.auth, AuthState::Failed(_)) {
                                app.auth = AuthState::Prompting;
                            }
                            app.password.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab | KeyCode::BackTab => {
                        app.tab = match app.tab {
                            Tab::Status => Tab::Controls,
                            Tab::Controls => Tab::Status,
                        };
                        app.selected = 0;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let max = app.max_items();
                        if max > 0 {
                            app.selected = (app.selected + 1) % max;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let max = app.max_items();
                        if max > 0 {
                            app.selected =
                                app.selected.checked_sub(1).unwrap_or(max - 1);
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        app.toggle_selected();
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        app.cycle_profile(true);
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        app.cycle_profile(false);
                    }
                    KeyCode::Char('r') => {
                        app.refresh();
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    stdout().execute(cursor::Show)?;
    Ok(())
}
