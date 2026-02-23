use std::fmt::Write as _;
use std::io::{Write, stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::execute;
use crossterm::style::{Color, Stylize};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use viuer::{Config as ViuerConfig, KittySupport};

use crate::config::TerminalLogoMode;
use crate::metrics::MetricsSnapshot;
use crate::session::{CodexSessionSnapshot, RateLimits, UsageWindow};
use crate::util::{
    format_cost, format_model_name, format_time_until, format_token_triplet, format_tokens,
    human_duration, now_local, progress_bar, truncate,
};

const FOOTER_ROWS: u16 = 1;
const FULL_RECENT_RESERVED_ROWS: u16 = 5;
const COMPACT_RECENT_RESERVED_ROWS: u16 = 3;
const MINIMAL_RECENT_RESERVED_ROWS: u16 = 1;

const OPENAI_ASCII: [&str; 8] = [
    "        .-========-.       ",
    "      .'  .----.    '.     ",
    "     /   .' __ '.     \\    ",
    "    ;   /  /  \\  \\     ;   ",
    "    ;   \\  \\__/  /     ;   ",
    "     \\   '.____.'     /    ",
    "      '.          _ .'     ",
    "        '-.____.-'         ",
];

const CODEX_ASCII: [&str; 8] = [
    "   _____   ____   _____   ______  __   __   ",
    "  / ____| / __ \\ |  __ \\ |  ____| \\ \\ / /   ",
    " | |     | |  | || |  | || |__     \\ V /    ",
    " | |     | |  | || |  | ||  __|     > <     ",
    " | |____ | |__| || |__| || |____   / . \\    ",
    "  \\_____| \\____/ |_____/ |______| /_/ \\_\\   ",
    "       Discord Presence for Codex CLI        ",
    "       Live activity + limits telemetry      ",
];

const COMPACT_BANNER: [&str; 2] = [
    "OPENAI x CODEX PRESENCE",
    "Live activity + limits telemetry",
];

const MINIMAL_BANNER: &str = "CODEX Presence";
const BANNER_TEXT_ROWS: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BannerVariant {
    Image,
    AsciiDual,
    AsciiCodex,
    CompactText,
    MinimalText,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiLayoutMode {
    Full,
    Compact,
    Minimal,
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
    pub openai_plan_label: &'a str,
    pub logo_mode: TerminalLogoMode,
    pub logo_path: Option<&'a str>,
    pub active: Option<&'a CodexSessionSnapshot>,
    pub effective_limits: Option<&'a RateLimits>,
    pub metrics: Option<&'a MetricsSnapshot>,
    pub sessions: &'a [CodexSessionSnapshot],
}

pub fn enter_terminal() -> Result<()> {
    let mut out = stdout();
    terminal::enable_raw_mode()?;
    execute!(out, EnterAlternateScreen, Hide)?;
    Ok(())
}

pub fn leave_terminal() -> Result<()> {
    let mut out = stdout();
    execute!(out, Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}

pub fn draw(data: &RenderData<'_>) -> Result<()> {
    let mut out = stdout();
    let (width, height) = terminal::size()?;
    if width == 0 || height == 0 {
        return Ok(());
    }

    let budget = FrameBudget::new(width, height);
    let max_body_row = budget.body_bottom();
    let layout = select_layout_mode(width, height);
    let w = width as usize;
    let reserved_recent = reserved_recent_rows(layout, max_body_row);
    let top_body_limit = max_body_row.saturating_sub(reserved_recent);

    execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;

    let mut row = 0u16;
    draw_banner(
        &mut out,
        &mut row,
        top_body_limit,
        w,
        layout,
        &data.logo_mode,
        data.logo_path,
    )?;
    write_section_gap(&mut out, &mut row, top_body_limit, w, layout)?;

    render_runtime_section(&mut out, &mut row, top_body_limit, w, layout, data)?;
    write_section_gap(&mut out, &mut row, top_body_limit, w, layout)?;

    render_active_section(&mut out, &mut row, top_body_limit, w, layout, data)?;
    write_section_gap(&mut out, &mut row, top_body_limit, w, layout)?;

    render_metrics_section(&mut out, &mut row, top_body_limit, w, layout, data)?;
    write_section_gap(&mut out, &mut row, top_body_limit, w, layout)?;

    if row < top_body_limit {
        row = top_body_limit;
    }
    render_recent_section(&mut out, &mut row, max_body_row, w, layout, data)?;
    render_footer(&mut out, w, height)?;

    out.flush()?;
    Ok(())
}

pub fn frame_signature(data: &RenderData<'_>) -> String {
    let mut signature = String::with_capacity(768);
    let _ = write!(
        signature,
        "{}|{}|{}|{}|{}|{}|{}|",
        data.mode_label,
        data.discord_status,
        data.client_id_configured,
        data.show_activity,
        data.show_activity_target,
        data.openai_plan_label,
        data.sessions.len()
    );

    if let Some(active) = data.active {
        let _ = write!(
            signature,
            "active:{}|{}|{}|{}|{}|",
            active.session_id,
            active.model.as_deref().unwrap_or(""),
            active.git_branch.as_deref().unwrap_or(""),
            active.session_total_tokens.unwrap_or(0),
            active.session_delta_tokens.unwrap_or(0),
        );
        if data.show_activity
            && let Some(activity) = &active.activity
        {
            let _ = write!(
                signature,
                "{}|{}|{}|",
                activity.action_text(),
                activity.target.as_deref().unwrap_or(""),
                activity.pending_calls
            );
        }
    } else {
        signature.push_str("active:none|");
    }

    if let Some(metrics) = data.metrics {
        let _ = write!(
            signature,
            "metrics:{:.6}|{}|{}|{}|",
            metrics.totals.cost_usd,
            metrics.totals.input_tokens,
            metrics.totals.cached_input_tokens,
            metrics.totals.output_tokens
        );
    }

    for session in data.sessions.iter().take(5) {
        let _ = write!(
            signature,
            "{}:{}:{}:{}|",
            session.session_id,
            session.session_delta_tokens.unwrap_or(0),
            session.last_turn_tokens.unwrap_or(0),
            session.session_total_tokens.unwrap_or(0)
        );
        if data.show_activity
            && let Some(activity) = &session.activity
        {
            let _ = write!(
                signature,
                "{}:{}:{}|",
                activity.action_text(),
                activity.target.as_deref().unwrap_or(""),
                activity.pending_calls
            );
        }
    }

    signature
}

fn render_runtime_section(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    layout: UiLayoutMode,
    data: &RenderData<'_>,
) -> Result<()> {
    if !write_line(out, row, max_body_row, width, &hr("Runtime", width))? {
        return Ok(());
    }

    let mut lines = vec![
        kv_line("Mode", data.mode_label),
        kv_line("Now", &now_local()),
        kv_line("Uptime", &human_duration(data.running_for)),
        kv_line("Discord", data.discord_status),
    ];

    if !matches!(layout, UiLayoutMode::Minimal) {
        let client_status = if data.client_id_configured {
            "configured"
        } else {
            "missing"
        };
        lines.push(kv_line("Client ID", client_status));
        lines.push(kv_line(
            "Polling",
            &format!(
                "{}s | Stale Threshold: {}s",
                data.poll_interval_secs, data.stale_secs
            ),
        ));
    }

    if matches!(layout, UiLayoutMode::Full) {
        lines.push(kv_line("Limits Mode", "remaining (Codex CLI parity)"));
    }

    for line in lines {
        if !write_line(out, row, max_body_row, width, &line)? {
            break;
        }
    }

    Ok(())
}

fn render_active_section(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    layout: UiLayoutMode,
    data: &RenderData<'_>,
) -> Result<()> {
    if !write_line(out, row, max_body_row, width, &hr("Active Session", width))? {
        return Ok(());
    }

    let Some(active) = data.active else {
        let _ = write_line(
            out,
            row,
            max_body_row,
            width,
            "No active Codex sessions detected.",
        );
        return Ok(());
    };

    if !write_line(
        out,
        row,
        max_body_row,
        width,
        &kv_line(
            "Project",
            &truncate(&active.project_name, width.saturating_sub(13)),
        ),
    )? {
        return Ok(());
    }

    let activity_text = if data.show_activity {
        active
            .activity
            .as_ref()
            .map(|item| item.to_text(data.show_activity_target))
            .unwrap_or_else(|| "Idle".to_string())
    } else {
        "hidden".to_string()
    };
    if !write_line(
        out,
        row,
        max_body_row,
        width,
        &kv_line("Activity", &activity_text),
    )? {
        return Ok(());
    }

    let model_line = format!(
        "{} | {}",
        format_model_name(active.model.as_deref().unwrap_or("unknown")),
        data.openai_plan_label
    );
    if !write_line(
        out,
        row,
        max_body_row,
        width,
        &kv_line("Model", &truncate(&model_line, width.saturating_sub(13))),
    )? {
        return Ok(());
    }

    let context_text = active
        .context_window
        .as_ref()
        .map(|context| {
            format!(
                "{}/{} used ({:.0}% left)",
                format_tokens(context.used_tokens),
                format_tokens(context.window_tokens),
                context.remaining_percent
            )
        })
        .unwrap_or_else(|| "n/a".to_string());
    if !write_line(
        out,
        row,
        max_body_row,
        width,
        &kv_line("Context", &context_text),
    )? {
        return Ok(());
    }

    if !matches!(layout, UiLayoutMode::Minimal) {
        let token_line = format_token_triplet(
            active.session_delta_tokens,
            active.last_turn_tokens,
            active.session_total_tokens,
        );
        if !write_line(out, row, max_body_row, width, &token_line)? {
            return Ok(());
        }
    }

    let cost_text = if active.total_cost_usd > 0.0 {
        format_cost(active.total_cost_usd)
    } else {
        "n/a".to_string()
    };
    if !write_line(out, row, max_body_row, width, &kv_line("Cost", &cost_text))? {
        return Ok(());
    }

    let limits = data.effective_limits.unwrap_or(&active.limits);
    if max_body_row.saturating_sub(*row) == 1 {
        let summary = render_compact_limits_row(limits);
        let _ = write_line(out, row, max_body_row, width, &summary);
        return Ok(());
    }

    if let Some(primary) = &limits.primary {
        let line = render_limit_row("5h", primary, width);
        if !write_line_unchecked(out, row, max_body_row, &line)? {
            return Ok(());
        }
    } else if !write_line(out, row, max_body_row, width, "5h remaining: n/a")? {
        return Ok(());
    }

    if let Some(secondary) = &limits.secondary {
        let line = render_limit_row("7d", secondary, width);
        if !write_line_unchecked(out, row, max_body_row, &line)? {
            return Ok(());
        }
    } else if !write_line(out, row, max_body_row, width, "7d remaining: n/a")? {
        return Ok(());
    }

    if !write_line(
        out,
        row,
        max_body_row,
        width,
        &kv_line("Branch", active.git_branch.as_deref().unwrap_or("n/a")),
    )? {
        return Ok(());
    }

    if matches!(layout, UiLayoutMode::Full) {
        let path_text = truncate(&active.cwd.display().to_string(), width.saturating_sub(13));
        let _ = write_line(out, row, max_body_row, width, &kv_line("Path", &path_text));
    }

    Ok(())
}

fn render_metrics_section(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    layout: UiLayoutMode,
    data: &RenderData<'_>,
) -> Result<()> {
    if !write_line(out, row, max_body_row, width, &hr("Metrics", width))? {
        return Ok(());
    }

    let Some(metrics) = data.metrics else {
        let _ = write_line(
            out,
            row,
            max_body_row,
            width,
            "Metrics: awaiting token events...",
        );
        return Ok(());
    };

    if metrics.totals.total_tokens == 0 && metrics.totals.cost_usd <= 0.0 {
        let _ = write_line(
            out,
            row,
            max_body_row,
            width,
            "Metrics: no token usage observed yet.",
        );
        return Ok(());
    }

    let summary = format!(
        "Total {} | Tokens {}",
        format_cost(metrics.totals.cost_usd),
        format_tokens(metrics.totals.total_tokens)
    );
    if !write_line(out, row, max_body_row, width, &summary)? {
        return Ok(());
    }

    let io_line = format!(
        "Input {} | Cached {} | Output {}",
        format_tokens(metrics.totals.input_tokens),
        format_tokens(metrics.totals.cached_input_tokens),
        format_tokens(metrics.totals.output_tokens)
    );
    if !write_line(out, row, max_body_row, width, &io_line)? {
        return Ok(());
    }

    if matches!(layout, UiLayoutMode::Minimal) {
        return Ok(());
    }

    let breakdown = format!(
        "Cost split I {} | C {} | O {}",
        format_cost(metrics.cost_breakdown.input_cost_usd),
        format_cost(metrics.cost_breakdown.cached_input_cost_usd),
        format_cost(metrics.cost_breakdown.output_cost_usd)
    );
    if !write_line(out, row, max_body_row, width, &breakdown)? {
        return Ok(());
    }

    if matches!(layout, UiLayoutMode::Full)
        && let Some(top_model) = metrics.by_model.first()
    {
        let top_line = format!(
            "Top model {} | {} | sessions {}",
            truncate(&format_model_name(&top_model.model_id), 24),
            format_cost(top_model.cost_usd),
            top_model.session_count
        );
        let _ = write_line(out, row, max_body_row, width, &top_line)?;
    }

    Ok(())
}

fn render_recent_section(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    layout: UiLayoutMode,
    data: &RenderData<'_>,
) -> Result<()> {
    if !write_line(out, row, max_body_row, width, &hr("Recent Sessions", width))? {
        return Ok(());
    }

    if data.sessions.is_empty() {
        let _ = write_line(
            out,
            row,
            max_body_row,
            width,
            "No active sessions within stale threshold.",
        );
        return Ok(());
    }

    let available_lines = max_body_row.saturating_sub(*row);
    if available_lines == 0 {
        return Ok(());
    }
    let per_session_lines = if matches!(layout, UiLayoutMode::Minimal) || available_lines < 2 {
        1u16
    } else {
        2u16
    };

    for (idx, session) in data.sessions.iter().enumerate() {
        if idx >= 8 {
            break;
        }
        if max_body_row.saturating_sub(*row) < per_session_lines {
            break;
        }

        let marker = if idx == 0 { ">" } else { "-" };
        let branch = session.git_branch.as_deref().unwrap_or("n/a");
        let model = format_model_name(session.model.as_deref().unwrap_or("unknown"));

        let header = format!(
            "{marker} {} | {} | {}",
            truncate(&session.project_name, 28),
            truncate(branch, 16),
            truncate(&model, 18)
        );
        if !write_line(out, row, max_body_row, width, &header)? {
            break;
        }

        if per_session_lines == 2 {
            let mut detail = format_token_triplet(
                session.session_delta_tokens,
                session.last_turn_tokens,
                session.session_total_tokens,
            );
            if data.show_activity {
                let activity = session
                    .activity
                    .as_ref()
                    .map(|item| item.to_text(data.show_activity_target))
                    .unwrap_or_else(|| "Idle".to_string());
                detail = format!("{} | {detail}", activity);
            }
            if !write_line(out, row, max_body_row, width, &format!("  {}", detail))? {
                break;
            }
        } else {
            let activity = session
                .activity
                .as_ref()
                .map(|item| item.to_text(data.show_activity_target))
                .unwrap_or_else(|| "Idle".to_string());
            let compact = format!(
                "{} | {} | Last {} | Total {}",
                truncate(&session.project_name, 22),
                truncate(&activity, 26),
                session
                    .last_turn_tokens
                    .map(crate::util::format_tokens)
                    .unwrap_or_else(|| "n/a".to_string()),
                session
                    .session_total_tokens
                    .map(crate::util::format_tokens)
                    .unwrap_or_else(|| "n/a".to_string())
            );
            if !write_line(out, row, max_body_row, width, &format!("  {}", compact))? {
                break;
            }
        }
    }

    Ok(())
}

fn draw_banner(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    layout: UiLayoutMode,
    logo_mode: &TerminalLogoMode,
    logo_path: Option<&str>,
) -> Result<()> {
    if *row >= max_body_row {
        return Ok(());
    }

    let available_rows = max_body_row.saturating_sub(*row);
    let mut allow_image = matches!(layout, UiLayoutMode::Full)
        && !matches!(logo_mode, TerminalLogoMode::Ascii)
        && logo_path.is_some();

    loop {
        match select_banner_variant(width, available_rows, layout, allow_image) {
            BannerVariant::Image => {
                if let Some(used_rows) =
                    try_draw_logo_image(out, *row, max_body_row, width, logo_mode, logo_path)?
                {
                    *row = row.saturating_add(used_rows);
                    let _ = write_line(
                        out,
                        row,
                        max_body_row,
                        width,
                        &center_line("CODEX DISCORD PRESENCE", width),
                    )?;
                    let _ = write_line(
                        out,
                        row,
                        max_body_row,
                        width,
                        &center_line("Live activity + limits telemetry", width),
                    )?;
                    return Ok(());
                }
                allow_image = false;
            }
            BannerVariant::AsciiDual => {
                draw_dual_ascii_banner(out, row, max_body_row, width)?;
                return Ok(());
            }
            BannerVariant::AsciiCodex => {
                draw_codex_ascii_banner(out, row, max_body_row, width)?;
                return Ok(());
            }
            BannerVariant::CompactText => {
                for text in COMPACT_BANNER {
                    if !write_line(out, row, max_body_row, width, &center_line(text, width))? {
                        break;
                    }
                }
                return Ok(());
            }
            BannerVariant::MinimalText => {
                let _ = write_line(
                    out,
                    row,
                    max_body_row,
                    width,
                    &center_line(MINIMAL_BANNER, width),
                )?;
                return Ok(());
            }
        }
    }
}

fn draw_dual_ascii_banner(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
) -> Result<()> {
    let left_width = banner_ascii_width(&OPENAI_ASCII);
    let right_width = banner_ascii_width(&CODEX_ASCII);
    let spacing = 4usize;
    let banner_width = left_width + spacing + right_width;
    let left_pad = " ".repeat(width.saturating_sub(banner_width) / 2);
    let spacer = " ".repeat(spacing);

    for idx in 0..OPENAI_ASCII.len().max(CODEX_ASCII.len()) {
        let left = OPENAI_ASCII.get(idx).copied().unwrap_or("");
        let right = CODEX_ASCII.get(idx).copied().unwrap_or("");
        let line = format!(
            "{left_pad}{left:<left_width$}{spacer}{right}",
            left_width = left_width
        );
        if !write_line(out, row, max_body_row, width, &line)? {
            break;
        }
    }

    Ok(())
}

fn draw_codex_ascii_banner(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
) -> Result<()> {
    let codex_width = banner_ascii_width(&CODEX_ASCII);
    let left_pad = " ".repeat(width.saturating_sub(codex_width) / 2);
    for text in CODEX_ASCII {
        let line = format!("{left_pad}{text}");
        if !write_line(out, row, max_body_row, width, &line)? {
            break;
        }
    }
    Ok(())
}

fn select_banner_variant(
    width: usize,
    available_rows: u16,
    layout: UiLayoutMode,
    allow_image: bool,
) -> BannerVariant {
    let left_width = banner_ascii_width(&OPENAI_ASCII);
    let right_width = banner_ascii_width(&CODEX_ASCII);
    let dual_width = left_width + 4 + right_width;
    let dual_min_width = dual_width + 4;
    let codex_min_width = right_width + 2;
    let dual_rows = OPENAI_ASCII.len().max(CODEX_ASCII.len()) as u16;
    let codex_rows = CODEX_ASCII.len() as u16;
    let compact_rows = COMPACT_BANNER.len() as u16;
    let image_rows = logo_image_rows(logo_image_width_cells(width)) + BANNER_TEXT_ROWS;

    if allow_image && available_rows >= image_rows {
        return BannerVariant::Image;
    }
    if matches!(layout, UiLayoutMode::Full)
        && width >= dual_min_width
        && available_rows >= dual_rows
    {
        return BannerVariant::AsciiDual;
    }
    if width >= codex_min_width && available_rows >= codex_rows {
        return BannerVariant::AsciiCodex;
    }
    if available_rows >= compact_rows {
        return BannerVariant::CompactText;
    }

    BannerVariant::MinimalText
}

fn banner_ascii_width(lines: &[&str]) -> usize {
    lines.iter().map(|line| line.len()).max().unwrap_or(0)
}

fn try_draw_logo_image(
    out: &mut impl Write,
    start_row: u16,
    max_body_row: u16,
    width: usize,
    logo_mode: &TerminalLogoMode,
    logo_path: Option<&str>,
) -> Result<Option<u16>> {
    if matches!(logo_mode, TerminalLogoMode::Ascii) {
        return Ok(None);
    }

    let Some(raw_path) = logo_path else {
        return Ok(None);
    };
    let path = resolve_logo_path(raw_path);
    if !path.exists() || !terminal_supports_logo_image() {
        return Ok(None);
    }

    let image_width_cells = logo_image_width_cells(width);
    let approx_rows = logo_image_rows(image_width_cells);
    if start_row
        .saturating_add(approx_rows)
        .saturating_add(BANNER_TEXT_ROWS)
        > max_body_row
    {
        return Ok(None);
    }

    let x_offset = width.saturating_sub(image_width_cells as usize) / 2;

    let conf = ViuerConfig {
        transparent: true,
        absolute_offset: false,
        x: x_offset as u16,
        y: start_row as i16,
        restore_cursor: false,
        width: Some(image_width_cells),
        ..Default::default()
    };

    out.flush()?;
    if viuer::print_from_file(&path, &conf).is_ok() {
        return Ok(Some(approx_rows));
    }

    Ok(None)
}

fn logo_image_width_cells(width: usize) -> u32 {
    if width >= 132 {
        44u32
    } else if width >= 112 {
        38u32
    } else {
        30u32
    }
}

fn logo_image_rows(image_width_cells: u32) -> u16 {
    if image_width_cells >= 44 {
        12u16
    } else if image_width_cells >= 38 {
        10u16
    } else {
        8u16
    }
}

fn render_footer(out: &mut impl Write, width: usize, height: u16) -> Result<()> {
    if height == 0 || width == 0 {
        return Ok(());
    }

    let row = height - 1;
    let (left, right) = footer_parts(width);
    execute!(out, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
    write!(out, "{left}")?;

    if !right.is_empty() {
        let right_col = width.saturating_sub(right.len()) as u16;
        execute!(out, MoveTo(right_col, row))?;
        write!(out, "{}", right.with(Color::Grey))?;
    }
    Ok(())
}

fn footer_parts(width: usize) -> (String, String) {
    if width == 0 {
        return (String::new(), String::new());
    }

    let left = truncate("Press q or Ctrl+C to quit.", width);
    if width <= left.len() + 1 {
        return (left, String::new());
    }

    let available_right = width - left.len() - 1;
    let right = truncate(&author_credit(width), available_right);
    (left, right)
}

fn write_section_gap(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    layout: UiLayoutMode,
) -> Result<()> {
    if matches!(layout, UiLayoutMode::Full) {
        let _ = write_line(out, row, max_body_row, width, "")?;
    }
    Ok(())
}

fn write_line(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    width: usize,
    text: &str,
) -> Result<bool> {
    if *row >= max_body_row {
        return Ok(false);
    }

    execute!(out, MoveTo(0, *row), Clear(ClearType::CurrentLine))?;
    write!(out, "{}", truncate(text, width))?;
    *row += 1;
    Ok(true)
}

fn write_line_unchecked(
    out: &mut impl Write,
    row: &mut u16,
    max_body_row: u16,
    text: &str,
) -> Result<bool> {
    if *row >= max_body_row {
        return Ok(false);
    }

    execute!(out, MoveTo(0, *row), Clear(ClearType::CurrentLine))?;
    write!(out, "{text}")?;
    *row += 1;
    Ok(true)
}

fn kv_line(label: &str, value: &str) -> String {
    format!("{label:<11}: {value}")
}

fn render_limit_row(label: &str, window: &UsageWindow, width: usize) -> String {
    let color = limit_color(window.remaining_percent);
    let pct_plain = format!("{:>3.0}%", window.remaining_percent);
    let pct = pct_plain.with(color).bold();
    let bar_width = limit_bar_width(width);
    let bar = progress_bar(window.remaining_percent, bar_width).with(color);
    let reset = format_time_until(window.resets_at);

    if width < 48 {
        return format!("{label} {pct}");
    }
    if width < 74 {
        return format!("{label} remaining [{pct}] {bar}");
    }
    format!("{label} remaining [{pct}] {bar} reset {reset}")
}

fn render_compact_limits_row(limits: &RateLimits) -> String {
    let primary = limit_percent_text(limits.primary.as_ref());
    let secondary = limit_percent_text(limits.secondary.as_ref());
    format!("Usage: 5h {primary} | 7d {secondary}")
}

fn limit_percent_text(window: Option<&UsageWindow>) -> String {
    window
        .map(|item| format!("{:.0}%", item.remaining_percent.clamp(0.0, 100.0)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn limit_bar_width(width: usize) -> usize {
    if width >= 140 {
        30
    } else if width >= 112 {
        24
    } else if width >= 92 {
        18
    } else if width >= 72 {
        14
    } else {
        10
    }
}

fn limit_color(percent: f64) -> Color {
    if percent >= 60.0 {
        Color::Green
    } else if percent >= 30.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn hr(title: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let core = format!(" {title} ");
    if core.len() >= width {
        return truncate(title, width);
    }

    let side = (width - core.len()) / 2;
    let right = width - core.len() - side;
    format!("{}{}{}", "-".repeat(side), core, "-".repeat(right))
}

fn select_layout_mode(width: u16, height: u16) -> UiLayoutMode {
    if width >= 112 && height >= 32 {
        UiLayoutMode::Full
    } else if width >= 80 && height >= 18 {
        UiLayoutMode::Compact
    } else {
        UiLayoutMode::Minimal
    }
}

fn reserved_recent_rows(layout: UiLayoutMode, max_body_row: u16) -> u16 {
    if max_body_row <= 12 {
        return 0;
    }

    let preferred = match layout {
        UiLayoutMode::Full if max_body_row >= 34 => FULL_RECENT_RESERVED_ROWS,
        UiLayoutMode::Full if max_body_row >= 28 => 3,
        UiLayoutMode::Full => 2,
        UiLayoutMode::Compact if max_body_row >= 22 => COMPACT_RECENT_RESERVED_ROWS,
        UiLayoutMode::Compact => 2,
        UiLayoutMode::Minimal => MINIMAL_RECENT_RESERVED_ROWS,
    };
    preferred.min(max_body_row)
}

fn terminal_supports_logo_image() -> bool {
    viuer::is_iterm_supported() || viuer::get_kitty_support() != KittySupport::None
}

fn resolve_logo_path(raw_path: &str) -> PathBuf {
    let path = raw_path.trim();
    if path == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }

    if let Some(stripped) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\"))
        && let Some(home) = dirs::home_dir()
    {
        return home.join(stripped);
    }

    Path::new(path).to_path_buf()
}

fn center_line(text: &str, width: usize) -> String {
    let clipped = truncate(text, width);
    let left_pad = width.saturating_sub(clipped.len()) / 2;
    format!("{}{}", " ".repeat(left_pad), clipped)
}

fn author_credit(width: usize) -> String {
    if width >= 92 {
        "XT0N1.T3CH | Discord @XT0N1.T3CH | ID 211189703641268224".to_string()
    } else if width >= 54 {
        "XT0N1.T3CH | @XT0N1.T3CH".to_string()
    } else {
        "XT0N1.T3CH".to_string()
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
        assert_eq!(limit_color(80.0), Color::Green);
        assert_eq!(limit_color(45.0), Color::Yellow);
        assert_eq!(limit_color(12.0), Color::Red);
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
        let (left, right) = footer_parts(84);
        assert!(left.len() + 1 + right.len() <= 84);

        let (left_small, right_small) = footer_parts(20);
        assert_eq!(right_small, "");
        assert_eq!(left_small, "Press q or Ctrl+C...");
    }

    #[test]
    fn reserved_recent_rows_by_layout() {
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 34), 5);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 28), 3);
        assert_eq!(reserved_recent_rows(UiLayoutMode::Full, 24), 2);
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
            BannerVariant::AsciiCodex
        );
        assert_eq!(
            select_banner_variant(100, 24, UiLayoutMode::Compact, false),
            BannerVariant::AsciiCodex
        );
        assert_eq!(
            select_banner_variant(120, 28, UiLayoutMode::Full, false),
            BannerVariant::AsciiDual
        );
    }

    #[test]
    fn banner_variant_falls_back_when_space_is_constrained() {
        assert_eq!(
            select_banner_variant(80, 7, UiLayoutMode::Compact, false),
            BannerVariant::CompactText
        );
        assert_eq!(
            select_banner_variant(60, 1, UiLayoutMode::Minimal, false),
            BannerVariant::MinimalText
        );
    }
}
