use std::fmt::Write as _;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline, Wrap};

use crate::config::{PlanPreset, TerminalLogoMode, plan_presets};
use crate::metrics::MetricsSnapshot;
use crate::session::{CodexSessionSnapshot, RateLimits, UsageWindow};
use crate::util::{
    format_cost, format_model_display, format_time_until, format_token_triplet, format_tokens,
    human_duration, truncate,
};

const FOOTER_ROWS: u16 = 1;
const FULL_RECENT_RESERVED_ROWS: u16 = 5;
const COMPACT_RECENT_RESERVED_ROWS: u16 = 3;
const MINIMAL_RECENT_RESERVED_ROWS: u16 = 1;

const CODEX_ASCII: [&str; 6] = [
    "  ______          __",
    " / ____/___  ____/ /__  _  __",
    "/ /   / __ \\/ __  / _ \\| |/_/",
    "/ /___/ /_/ / /_/ /  __/>  <",
    "\\____/\\____/\\__,_/\\___/_/|_|",
    "local-first Discord Rich Presence",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLayoutMode {
    Full,
    Compact,
    Minimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BannerVariant {
    Image,
    AsciiDual,
    CompactText,
    MinimalText,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameBudget {
    pub width: u16,
    pub height: u16,
    pub footer_rows: u16,
}

impl FrameBudget {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            footer_rows: FOOTER_ROWS.min(height),
        }
    }

    fn body_bottom(self) -> u16 {
        self.height.saturating_sub(self.footer_rows)
    }
}

pub struct RenderData<'a> {
    pub running_for: Duration,
    pub mode_label: &'a str,
    pub discord_status: &'a str,
    pub client_id_configured: bool,
    pub poll_interval_secs: u64,
    pub stale_secs: u64,
    pub show_activity: bool,
    pub show_activity_target: bool,
    pub plan_display_label: &'a str,
    pub plan_status_label: &'a str,
    pub fast_mode_label: &'a str,
    pub fast_active: bool,
    pub limits_source_label: &'a str,
    pub limits_updated_label: &'a str,
    pub spark_plan_warning: Option<&'a str>,
    pub logo_mode: TerminalLogoMode,
    pub logo_path: Option<&'a str>,
    pub banner_phase: u8,
    pub active: Option<&'a CodexSessionSnapshot>,
    pub effective_limits: Option<&'a RateLimits>,
    pub metrics: Option<&'a MetricsSnapshot>,
    pub sessions: &'a [CodexSessionSnapshot],
    pub plan_picker: Option<PlanPickerView>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanPickerView {
    pub selected_index: usize,
    pub current_index: usize,
}

type UiTerminal = ratatui::DefaultTerminal;

static TERMINAL: OnceLock<Mutex<Option<UiTerminal>>> = OnceLock::new();

pub fn enter_terminal() -> Result<()> {
    let terminal = ratatui::init();
    *terminal_cell().lock().expect("terminal lock") = Some(terminal);
    Ok(())
}

pub fn leave_terminal() -> Result<()> {
    *terminal_cell().lock().expect("terminal lock") = None;
    ratatui::restore();
    Ok(())
}

pub fn draw(data: &RenderData<'_>) -> Result<()> {
    let mut guard = terminal_cell().lock().expect("terminal lock");
    if guard.is_none() {
        *guard = Some(ratatui::init());
    }
    if let Some(terminal) = guard.as_mut() {
        terminal.draw(|frame| render_frame(frame, data))?;
    }
    Ok(())
}

fn terminal_cell() -> &'static Mutex<Option<UiTerminal>> {
    TERMINAL.get_or_init(|| Mutex::new(None))
}

fn render_frame(frame: &mut Frame<'_>, data: &RenderData<'_>) {
    let area = frame.area();
    if area.width == 0 || area.height == 0 {
        return;
    }

    let layout = select_layout_mode(area.width, area.height);
    if let Some(plan_picker) = data.plan_picker {
        render_plan_picker(frame, area, plan_picker);
        return;
    }

    let budget = FrameBudget::new(area.width, area.height);
    let footer_height = budget.footer_rows;
    let _recent_rows = reserved_recent_rows(layout, area.height);
    let _body_bottom = budget.body_bottom();
    let root =
        Layout::vertical([Constraint::Min(0), Constraint::Length(footer_height)]).split(area);

    let body = body_layout(layout, root[0]);
    render_header(frame, body[0], layout, data);

    match layout {
        UiLayoutMode::Full => {
            let columns =
                Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                    .split(body[1]);
            let left =
                Layout::vertical([Constraint::Length(9), Constraint::Min(8)]).split(columns[0]);
            let right =
                Layout::vertical([Constraint::Length(9), Constraint::Min(8)]).split(columns[1]);
            render_active(frame, left[0], data);
            render_usage(frame, left[1], data);
            render_metrics(frame, right[0], data);
            render_recent(frame, right[1], layout, data);
        }
        UiLayoutMode::Compact => {
            let rows = Layout::vertical([
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Min(5),
            ])
            .split(body[1]);
            render_active(frame, rows[0], data);
            render_usage(frame, rows[1], data);
            render_recent(frame, rows[2], layout, data);
        }
        UiLayoutMode::Minimal => {
            let rows = Layout::vertical([Constraint::Length(5), Constraint::Min(3)]).split(body[1]);
            render_active(frame, rows[0], data);
            render_recent(frame, rows[1], layout, data);
        }
    }

    render_footer(frame, root[1], false);
}

fn body_layout(layout: UiLayoutMode, area: Rect) -> std::rc::Rc<[Rect]> {
    let header_height = match layout {
        UiLayoutMode::Full => 8,
        UiLayoutMode::Compact => 5,
        UiLayoutMode::Minimal => 3,
    }
    .min(area.height);
    Layout::vertical([Constraint::Length(header_height), Constraint::Min(0)]).split(area)
}

fn render_header(frame: &mut Frame<'_>, area: Rect, layout: UiLayoutMode, data: &RenderData<'_>) {
    let mut lines = Vec::new();
    let variant = select_banner_variant(
        area.width,
        area.height,
        layout,
        matches!(
            data.logo_mode,
            TerminalLogoMode::Image | TerminalLogoMode::Auto
        ) && data.logo_path.is_some(),
    );
    match variant {
        BannerVariant::Image => {
            lines.push(Line::from(vec![
                Span::styled("▰ ", Style::default().fg(theme::PINK)),
                Span::styled("Codex", theme::title()),
                Span::styled(" real logo ready", Style::default().fg(theme::MUTED)),
            ]));
            if let Some(path) = data.logo_path {
                lines.push(Line::from(Span::styled(
                    truncate(path, area.width as usize),
                    theme::muted(),
                )));
            }
        }
        BannerVariant::AsciiDual => {
            for line in CODEX_ASCII
                .iter()
                .take(area.height.saturating_sub(2) as usize)
            {
                lines.push(Line::from(Span::styled(*line, theme::title())));
            }
        }
        BannerVariant::CompactText => {
            lines.push(Line::from(Span::styled("Codex Presence", theme::title())))
        }
        BannerVariant::MinimalText => lines.push(Line::from(Span::styled("Codex", theme::title()))),
    }
    let spinner = spinner(data.banner_phase);
    lines.push(Line::from(vec![
        Span::styled(format!("{spinner} "), Style::default().fg(theme::CYAN)),
        Span::styled(data.mode_label, Style::default().fg(theme::TEXT)),
        Span::styled(" · ", theme::muted()),
        Span::styled(data.discord_status, status_style(data.discord_status)),
        Span::styled(" · ", theme::muted()),
        Span::styled(format!("{} poll", data.poll_interval_secs), theme::muted()),
    ]));

    let block = panel("runtime", Some(theme::CYAN));
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn render_active(frame: &mut Frame<'_>, area: Rect, data: &RenderData<'_>) {
    let mut lines = Vec::new();
    if let Some(session) = data.active {
        lines.push(Line::from(vec![
            Span::styled(
                truncate(&session.project_name, 28),
                Style::default().fg(theme::TEXT).bold(),
            ),
            Span::styled("  ", theme::muted()),
            Span::styled(
                session.git_branch.as_deref().unwrap_or("no branch"),
                Style::default().fg(theme::GREEN),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("model ", theme::muted()),
            Span::styled(
                format_model_display(
                    session.model.as_deref().unwrap_or("unknown"),
                    session.reasoning_effort,
                    data.fast_active,
                ),
                Style::default().fg(theme::PINK),
            ),
            Span::styled(" · ", theme::muted()),
            Span::styled(
                format_cost(session.total_cost_usd),
                Style::default().fg(theme::YELLOW),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("tokens ", theme::muted()),
            Span::raw(format_token_triplet(
                session.session_delta_tokens,
                session.last_turn_tokens,
                session.session_total_tokens,
            )),
        ]));
        if let Some(context) = &session.context_window {
            lines.push(Line::from(vec![
                Span::styled("context ", theme::muted()),
                Span::raw(format_tokens(context.used_tokens)),
                Span::styled(" / ", theme::muted()),
                Span::raw(format_tokens(context.window_tokens)),
                Span::styled(
                    format!(" ({:.0}% free)", context.remaining_percent),
                    Style::default().fg(limit_color(context.remaining_percent)),
                ),
            ]));
        }
        if data.show_activity
            && let Some(activity) = &session.activity
        {
            lines.push(Line::from(vec![
                Span::styled("activity ", theme::muted()),
                Span::raw(activity.to_text(data.show_activity_target)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "No active Codex session",
            theme::muted(),
        )));
        lines.push(Line::from(Span::styled(
            "Idle keeps the last detected surface, so Codex App stays Codex App.",
            Style::default().fg(theme::TEXT),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(panel("active session", Some(theme::PINK)))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_usage(frame: &mut Frame<'_>, area: Rect, data: &RenderData<'_>) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(2),
    ])
    .split(area);
    let limits = data.effective_limits;
    render_usage_gauge(
        frame,
        rows[0],
        "primary",
        limits.and_then(|value| value.primary.as_ref()),
    );
    render_usage_gauge(
        frame,
        rows[1],
        "secondary",
        limits.and_then(|value| value.secondary.as_ref()),
    );
    let warning = data
        .spark_plan_warning
        .unwrap_or("OAuth Codex context is capped at 400K; API metadata is tracked separately.");
    let line = vec![
        Line::from(vec![
            Span::styled("plan ", theme::muted()),
            Span::styled(data.plan_display_label, Style::default().fg(theme::TEXT)),
            Span::styled(" · ", theme::muted()),
            Span::styled(data.fast_mode_label, fast_style(data.fast_active)),
        ]),
        Line::from(Span::styled(warning, Style::default().fg(theme::YELLOW))),
        Line::from(Span::styled(
            format!(
                "limits {} · {}",
                data.limits_source_label, data.limits_updated_label
            ),
            theme::muted(),
        )),
    ];
    frame.render_widget(
        Paragraph::new(line)
            .block(panel("quota + context", Some(theme::GREEN)))
            .wrap(Wrap { trim: true }),
        rows[2],
    );
}

fn render_usage_gauge(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    window: Option<&UsageWindow>,
) {
    let used = window
        .map(|value| value.used_percent)
        .unwrap_or(0.0)
        .clamp(0.0, 100.0);
    let title = window
        .and_then(|value| value.resets_at)
        .map(|reset| format!("{label} · resets {}", format_time_until(Some(reset))))
        .unwrap_or_else(|| label.to_string());
    let gauge = Gauge::default()
        .block(panel(&title, Some(limit_color(100.0 - used))))
        .gauge_style(Style::default().fg(limit_color(100.0 - used)))
        .ratio(used / 100.0)
        .label(format!("{used:.0}% used"));
    frame.render_widget(gauge, area);
}

fn render_metrics(frame: &mut Frame<'_>, area: Rect, data: &RenderData<'_>) {
    let Some(metrics) = data.metrics else {
        frame.render_widget(
            Paragraph::new("Metrics warm up after the first session scan.")
                .block(panel("usage snapshot", Some(theme::YELLOW))),
            area,
        );
        return;
    };
    let cache = metrics.totals.cache_hit_ratio * 100.0;
    let samples = sparkline_samples(metrics);
    let inner = Layout::vertical([Constraint::Length(5), Constraint::Min(2)]).split(area);
    let text = vec![
        Line::from(vec![
            Span::styled("cost ", theme::muted()),
            Span::styled(
                format_cost(metrics.totals.cost_usd),
                Style::default().fg(theme::YELLOW),
            ),
            Span::styled(" · cache ", theme::muted()),
            Span::styled(
                format!("{cache:.1}%"),
                Style::default().fg(limit_color(cache)),
            ),
        ]),
        Line::from(vec![
            Span::styled("saved ", theme::muted()),
            Span::styled(
                format_cost(metrics.cost_breakdown.cached_input_savings_usd),
                Style::default().fg(theme::GREEN),
            ),
            Span::styled(" · uptime ", theme::muted()),
            Span::raw(human_duration(Duration::from_secs(metrics.uptime_seconds))),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(text).block(panel("usage snapshot", Some(theme::YELLOW))),
        inner[0],
    );
    frame.render_widget(
        Sparkline::default()
            .block(panel("model spend sparkline", Some(theme::CYAN)))
            .style(Style::default().fg(theme::CYAN))
            .data(&samples),
        inner[1],
    );
}

fn render_recent(frame: &mut Frame<'_>, area: Rect, layout: UiLayoutMode, data: &RenderData<'_>) {
    let max_items = match layout {
        UiLayoutMode::Full => area.height.saturating_sub(2) as usize,
        UiLayoutMode::Compact => area.height.saturating_sub(2).min(4) as usize,
        UiLayoutMode::Minimal => area.height.saturating_sub(2).min(2) as usize,
    };
    let items: Vec<ListItem<'_>> = data
        .sessions
        .iter()
        .take(max_items)
        .map(|session| {
            let model = format_model_display(
                session.model.as_deref().unwrap_or("unknown"),
                session.reasoning_effort,
                data.fast_active,
            );
            let tokens = format_tokens(
                session
                    .session_total_tokens
                    .unwrap_or(session.input_tokens_total + session.output_tokens_total),
            );
            ListItem::new(Line::from(vec![
                Span::styled(
                    truncate(&session.project_name, 22),
                    Style::default().fg(theme::TEXT),
                ),
                Span::styled(" · ", theme::muted()),
                Span::styled(model, Style::default().fg(theme::PINK)),
                Span::styled(" · ", theme::muted()),
                Span::styled(tokens, Style::default().fg(theme::CYAN)),
            ]))
        })
        .collect();
    let list = if items.is_empty() {
        List::new(vec![ListItem::new("No recent sessions yet")])
    } else {
        List::new(items)
    };
    frame.render_widget(
        list.block(panel("recent sessions", Some(theme::CYAN))),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, plan_picker: bool) {
    let (left, right) = footer_parts(area.width as usize, plan_picker);
    let mut spans = vec![Span::styled(left, theme::muted())];
    if !right.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(right, Style::default().fg(theme::PINK)));
    }
    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_plan_picker(frame: &mut Frame<'_>, area: Rect, view: PlanPickerView) {
    let presets = plan_presets();
    let width = area.width.min(84);
    let height = area.height.min((presets.len() as u16 + 4).max(8));
    let panel_area = centered_rect(width, height, area);
    let items: Vec<ListItem<'_>> = presets
        .iter()
        .enumerate()
        .map(|(index, preset)| {
            let marker = if index == view.selected_index {
                "▶"
            } else {
                " "
            };
            let current = if index == view.current_index {
                " current"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(theme::PINK)),
                Span::raw(" "),
                Span::styled(plan_label(*preset), Style::default().fg(theme::TEXT)),
                Span::styled(current, theme::muted()),
            ]))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(panel("Plan selector", Some(theme::PINK))),
        panel_area,
    );
    let footer = Rect {
        x: panel_area.x,
        y: panel_area.y + panel_area.height.saturating_sub(1),
        width: panel_area.width,
        height: 1,
    };
    render_footer(frame, footer, true);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn panel(title: &str, accent: Option<Color>) -> Block<'_> {
    let style = Style::default().fg(accent.unwrap_or(theme::BORDER));
    Block::default()
        .borders(Borders::ALL)
        .border_style(style)
        .title(Span::styled(format!(" {title} "), style))
}

pub fn frame_signature(data: &RenderData<'_>) -> String {
    let mut signature = String::with_capacity(768);
    let _ = write!(
        signature,
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
        data.mode_label,
        data.discord_status,
        data.client_id_configured,
        data.show_activity,
        data.show_activity_target,
        data.plan_display_label,
        data.plan_status_label,
        data.fast_mode_label,
        data.fast_active,
        data.sessions.len(),
        data.banner_phase,
        data.plan_picker
            .map(|value| value.selected_index)
            .unwrap_or(usize::MAX),
    );
    if let Some(active) = data.active {
        let _ = write!(
            signature,
            "active:{}|{}|{}|{}|{}|{}|",
            active.session_id,
            active.model.as_deref().unwrap_or(""),
            active
                .reasoning_effort
                .map(|value| value.label())
                .unwrap_or(""),
            active.git_branch.as_deref().unwrap_or(""),
            active.session_total_tokens.unwrap_or(0),
            active.session_delta_tokens.unwrap_or(0),
        );
    } else {
        signature.push_str("active:none|");
    }
    if let Some(metrics) = data.metrics {
        let _ = write!(
            signature,
            "metrics:{:.6}|{}|{}|{}|{:.4}|",
            metrics.totals.cost_usd,
            metrics.totals.input_tokens,
            metrics.totals.cached_input_tokens,
            metrics.totals.output_tokens,
            metrics.totals.cache_hit_ratio,
        );
    }
    if let Some(limits) = data.effective_limits {
        write_window_signature(&mut signature, "primary", limits.primary.as_ref());
        write_window_signature(&mut signature, "secondary", limits.secondary.as_ref());
    }
    signature
}

fn write_window_signature(signature: &mut String, name: &str, window: Option<&UsageWindow>) {
    if let Some(window) = window {
        let _ = write!(
            signature,
            "{}:{:.2}:{:.2}:{}|",
            name, window.used_percent, window.remaining_percent, window.window_minutes
        );
    } else {
        let _ = write!(signature, "{name}:none|");
    }
}

fn select_layout_mode(width: u16, height: u16) -> UiLayoutMode {
    if width >= 118 && height >= 32 {
        UiLayoutMode::Full
    } else if width >= 72 && height >= 18 {
        UiLayoutMode::Compact
    } else {
        UiLayoutMode::Minimal
    }
}

fn reserved_recent_rows(layout: UiLayoutMode, max_body_row: u16) -> u16 {
    if max_body_row < 14 {
        return 0;
    }
    match layout {
        UiLayoutMode::Full => FULL_RECENT_RESERVED_ROWS.min(max_body_row / 8).max(2),
        UiLayoutMode::Compact => COMPACT_RECENT_RESERVED_ROWS.min(max_body_row / 8).max(2),
        UiLayoutMode::Minimal => MINIMAL_RECENT_RESERVED_ROWS,
    }
}

fn select_banner_variant(
    width: u16,
    available_rows: u16,
    layout: UiLayoutMode,
    image_enabled: bool,
) -> BannerVariant {
    if image_enabled && width >= 80 && available_rows >= 4 {
        return BannerVariant::Image;
    }
    match layout {
        UiLayoutMode::Full if width >= 96 && available_rows >= 7 => BannerVariant::AsciiDual,
        UiLayoutMode::Compact if width >= 72 && available_rows >= 5 => BannerVariant::AsciiDual,
        UiLayoutMode::Full | UiLayoutMode::Compact if available_rows >= 3 => {
            BannerVariant::CompactText
        }
        UiLayoutMode::Minimal if available_rows >= 2 => BannerVariant::MinimalText,
        _ => BannerVariant::MinimalText,
    }
}

fn limit_color(remaining_percent: f64) -> Color {
    if remaining_percent >= 60.0 {
        theme::GREEN
    } else if remaining_percent >= 25.0 {
        theme::YELLOW
    } else {
        theme::RED
    }
}

fn status_style(status: &str) -> Style {
    let normalized = status.to_ascii_lowercase();
    if normalized.contains("connected") {
        Style::default().fg(theme::GREEN)
    } else if normalized.contains("missing") {
        Style::default().fg(theme::YELLOW)
    } else {
        Style::default().fg(theme::MUTED)
    }
}

fn fast_style(active: bool) -> Style {
    if active {
        Style::default().fg(theme::PINK).bold()
    } else {
        Style::default().fg(theme::MUTED)
    }
}

fn spinner(phase: u8) -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    FRAMES[phase as usize % FRAMES.len()]
}

fn sparkline_samples(metrics: &MetricsSnapshot) -> Vec<u64> {
    let mut values: Vec<u64> = metrics
        .by_model
        .iter()
        .map(|model| (model.cost_usd * 10_000.0).round().max(1.0) as u64)
        .collect();
    if values.is_empty() {
        values.push(1);
    }
    values
}

fn footer_parts(width: usize, plan_picker: bool) -> (String, String) {
    let left = if plan_picker {
        "Plan selector: ↑/↓ choose · Enter apply · Esc close"
    } else {
        "Press P to change plan · Ctrl+C quit"
    };
    let right = author_credit(width);
    if width <= 32 {
        (truncate(left, width), String::new())
    } else {
        let left_budget = width.saturating_sub(right.len() + 1);
        (truncate(left, left_budget), right)
    }
}

fn author_credit(width: usize) -> String {
    if width >= 100 {
        "XT0N1.T3CH | @XT0N1.T3CH | ID 211189703641268224".to_string()
    } else if width >= 48 {
        "XT0N1.T3CH | @XT0N1.T3CH".to_string()
    } else {
        "XT0N1.T3CH".to_string()
    }
}

#[cfg(test)]
fn hr(title: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let label = format!(" {title} ");
    if label.len() >= width {
        return truncate(&label, width);
    }
    let right = width - label.len();
    format!("{label}{}", "-".repeat(right))
}

fn plan_label(preset: PlanPreset) -> &'static str {
    preset.label
}

mod theme {
    use ratatui::prelude::*;

    pub const TEXT: Color = Color::Rgb(237, 242, 247);
    pub const MUTED: Color = Color::Rgb(148, 163, 184);
    pub const BORDER: Color = Color::Rgb(71, 85, 105);
    pub const CYAN: Color = Color::Rgb(125, 211, 252);
    pub const PINK: Color = Color::Rgb(244, 114, 182);
    pub const GREEN: Color = Color::Rgb(134, 239, 172);
    pub const YELLOW: Color = Color::Rgb(253, 224, 71);
    pub const RED: Color = Color::Rgb(248, 113, 113);

    pub fn title() -> Style {
        Style::default().fg(CYAN).bold()
    }

    pub fn muted() -> Style {
        Style::default().fg(MUTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_rule_respects_requested_width() {
        let line = hr("Test", 24);
        assert_eq!(line.len(), 24);
    }

    #[test]
    fn limit_color_thresholds() {
        assert_eq!(limit_color(80.0), theme::GREEN);
        assert_eq!(limit_color(45.0), theme::YELLOW);
        assert_eq!(limit_color(12.0), theme::RED);
    }

    #[test]
    fn author_credit_is_responsive() {
        assert!(author_credit(120).contains("ID 211189703641268224"));
        assert_eq!(author_credit(60), "XT0N1.T3CH | @XT0N1.T3CH");
        assert_eq!(author_credit(20), "XT0N1.T3CH");
    }

    #[test]
    fn layout_mode_switches_by_terminal_size() {
        assert_eq!(select_layout_mode(120, 34), UiLayoutMode::Full);
        assert_eq!(select_layout_mode(110, 30), UiLayoutMode::Compact);
        assert_eq!(select_layout_mode(90, 22), UiLayoutMode::Compact);
        assert_eq!(select_layout_mode(60, 16), UiLayoutMode::Minimal);
    }

    #[test]
    fn frame_budget_reserves_footer() {
        let budget = FrameBudget::new(120, 30);
        assert_eq!(budget.body_bottom(), 29);
    }

    #[test]
    fn footer_parts_never_overlap() {
        let (left, right) = footer_parts(84, false);
        assert!(left.len() + 1 + right.len() <= 84);

        let (left_small, right_small) = footer_parts(20, false);
        assert_eq!(right_small, "");
        assert_eq!(left_small, "Press P to change...");
    }

    #[test]
    fn reserved_recent_rows_by_layout() {
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 34), 4);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 28), 3);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 24), 3);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Compact, 24), 3);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Compact, 20), 2);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Minimal, 20), 1);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 12), 0);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Compact, 10), 0);
    }

    #[test]
    fn banner_variant_targets_requested_window_sizes() {
        assert_eq!(
            select_banner_variant(80, 17, UiLayoutMode::Compact, false),
            BannerVariant::AsciiDual
        );
        assert_eq!(
            select_banner_variant(100, 24, UiLayoutMode::Compact, false),
            BannerVariant::AsciiDual
        );
        assert_eq!(
            select_banner_variant(120, 28, UiLayoutMode::Full, false),
            BannerVariant::AsciiDual
        );
    }

    #[test]
    fn banner_variant_allows_image_in_compact_when_enabled() {
        assert_eq!(
            select_banner_variant(100, 20, UiLayoutMode::Compact, true),
            BannerVariant::Image
        );
    }

    #[test]
    fn banner_variant_falls_back_when_space_is_constrained() {
        assert_eq!(
            select_banner_variant(80, 7, UiLayoutMode::Compact, false),
            BannerVariant::AsciiDual
        );
        assert_eq!(
            select_banner_variant(60, 1, UiLayoutMode::Minimal, false),
            BannerVariant::MinimalText
        );
    }

    #[test]
    fn footer_parts_change_when_plan_picker_is_open() {
        let (left, _right) = footer_parts(80, true);
        assert!(left.contains("Plan selector"));
    }

    #[test]
    fn spinner_animates_by_phase() {
        assert_ne!(spinner(0), spinner(1));
        assert_eq!(spinner(0), spinner(8));
    }
}
