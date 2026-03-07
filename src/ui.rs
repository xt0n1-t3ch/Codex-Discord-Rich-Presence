use std::fmt::Write as _;
use std::io::{Write, stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::execute;
use crossterm::style::{Color, Stylize};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use viuer::Config as ViuerConfig;

use crate::config::{OpenAiPlanMode, OpenAiPlanTier, PlanPreset, TerminalLogoMode, plan_presets};
use crate::metrics::MetricsSnapshot;
use crate::session::{CodexSessionSnapshot, RateLimits, UsageWindow};
use crate::util::{
    format_cost, format_model_display, format_model_name, format_time_until, format_token_triplet,
    format_tokens, human_duration, now_local, progress_bar, truncate,
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
    " Presence for CLI + Codex VS Code Ext + App  ",
    "       Live activity + account usage         ",
];

const COMPACT_BANNER: [&str; 2] = ["OPENAI x CODEX PRESENCE", "Live activity + account usage"];

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
struct BannerRenderOptions<'a> {
    layout: UiLayoutMode,
    logo_mode: &'a TerminalLogoMode,
    logo_path: Option<&'a str>,
    phase: u8,
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

    execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;

    if let Some(plan_picker) = data.plan_picker {
        render_plan_picker_screen(&mut out, width, height, plan_picker)?;
        render_footer(&mut out, width as usize, height, true)?;
        out.flush()?;
        return Ok(());
    }

    let budget = FrameBudget::new(width, height);
    let max_body_row = budget.body_bottom();
    let layout = select_layout_mode(width, height);
    let w = width as usize;
    let reserved_recent = reserved_recent_rows(layout, max_body_row);
    let top_body_limit = max_body_row.saturating_sub(reserved_recent);

    let mut row = 0u16;
    let banner_options = BannerRenderOptions {
        layout,
        logo_mode: &data.logo_mode,
        logo_path: data.logo_path,
        phase: data.banner_phase,
    };
    draw_banner(&mut out, &mut row, top_body_limit, w, banner_options)?;
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
    render_footer(&mut out, w, height, false)?;

    out.flush()?;
    Ok(())
}

pub fn frame_signature(data: &RenderData<'_>) -> String {
    let mut signature = String::with_capacity(768);
    let _ = write!(
        signature,
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|",
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
        data.plan_picker
            .map(|value| value.current_index)
            .unwrap_or(usize::MAX)
    );
    let _ = write!(
        signature,
        "plan:{}|fast:{}|ls:{}|lu:{}|",
        data.plan_status_label,
        data.fast_mode_label,
        data.limits_source_label,
        data.limits_updated_label
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
        lines.push(kv_line(
            "Quota Policy",
            "Prioritize account-wide quota (/codex)",
        ));
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
        format_model_display(
            active.model.as_deref().unwrap_or("unknown"),
            active.reasoning_effort,
            data.fast_active,
        ),
        data.plan_display_label
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
    if !write_line(
        out,
        row,
        max_body_row,
        width,
        &kv_line("Fast Mode", data.fast_mode_label),
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
    if matches!(layout, UiLayoutMode::Minimal) {
        let account_line = format!("{} | Fast {}", data.plan_status_label, data.fast_mode_label);
        if !write_line(
            out,
            row,
            max_body_row,
            width,
            &kv_line("Account", &account_line),
        )? {
            return Ok(());
        }
    } else {
        if !write_line(
            out,
            row,
            max_body_row,
            width,
            &kv_line("Account Type", data.plan_status_label),
        )? {
            return Ok(());
        }
        if !write_line(
            out,
            row,
            max_body_row,
            width,
            &kv_line("Quota Source", data.limits_source_label),
        )? {
            return Ok(());
        }
        if !write_line(
            out,
            row,
            max_body_row,
            width,
            &kv_line("Quota Sync", data.limits_updated_label),
        )? {
            return Ok(());
        }
    }
    if let Some(warning) = data.spark_plan_warning
        && !write_line(
            out,
            row,
            max_body_row,
            width,
            &kv_line("Model Gate", warning),
        )?
    {
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
        let summary = render_compact_limits_row(limits, width);
        let _ = write_line_unchecked(out, row, max_body_row, &summary);
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
    options: BannerRenderOptions<'_>,
) -> Result<()> {
    if *row >= max_body_row {
        return Ok(());
    }

    let available_rows = max_body_row.saturating_sub(*row);
    let effective_logo_path = resolve_effective_logo_path(options.logo_path);
    let mut allow_image = matches!(options.logo_mode, TerminalLogoMode::Image)
        && !matches!(options.layout, UiLayoutMode::Minimal)
        && effective_logo_path.is_some();

    loop {
        match select_banner_variant(width, available_rows, options.layout, allow_image) {
            BannerVariant::Image => {
                if let Some(used_rows) = try_draw_logo_image(
                    out,
                    *row,
                    max_body_row,
                    width,
                    options.logo_mode,
                    effective_logo_path.as_deref(),
                )? {
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
                        &center_line("Live activity + account usage", width),
                    )?;
                    return Ok(());
                }
                allow_image = false;
            }
            BannerVariant::AsciiDual => {
                draw_dual_ascii_banner(out, row, max_body_row, width, options.phase)?;
                return Ok(());
            }
            BannerVariant::AsciiCodex => {
                draw_codex_ascii_banner(out, row, max_body_row, width, options.phase)?;
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
    phase: u8,
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
        let right = style_codex_banner_line(right, idx, phase);
        let line = format!(
            "{left_pad}{left:<left_width$}{spacer}{right}",
            left_width = left_width
        );
        if !write_line_unchecked(out, row, max_body_row, &line)? {
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
    phase: u8,
) -> Result<()> {
    let codex_width = banner_ascii_width(&CODEX_ASCII);
    let left_pad = " ".repeat(width.saturating_sub(codex_width) / 2);
    for (idx, text) in CODEX_ASCII.into_iter().enumerate() {
        let line = format!("{left_pad}{}", style_codex_banner_line(text, idx, phase));
        if !write_line_unchecked(out, row, max_body_row, &line)? {
            break;
        }
    }
    Ok(())
}

fn style_codex_banner_line(text: &str, line_index: usize, phase: u8) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    // Monochrome OpenAI-like palette: white logo strokes over black background.
    let pulse_line = usize::from(phase) % 6;
    let styled = if line_index < 6 {
        if line_index == pulse_line {
            text.with(Color::White).bold()
        } else {
            text.with(Color::Grey)
        }
    } else if line_index == 6 {
        text.with(Color::White)
    } else {
        text.with(Color::Grey)
    };
    styled.to_string()
}

fn select_banner_variant(
    width: usize,
    available_rows: u16,
    _layout: UiLayoutMode,
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
    if width >= dual_min_width && available_rows >= dual_rows {
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
    logo_path: Option<&Path>,
) -> Result<Option<u16>> {
    if matches!(logo_mode, TerminalLogoMode::Ascii) {
        return Ok(None);
    }

    let Some(path) = logo_path else {
        return Ok(None);
    };
    if !path.exists() {
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

    // Let viuer choose best available renderer (Sixel/Kitty/iTerm/Block).
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
    if viuer::print_from_file(path, &conf).is_ok() {
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

fn render_footer(
    out: &mut impl Write,
    width: usize,
    height: u16,
    plan_picker_open: bool,
) -> Result<()> {
    if height == 0 || width == 0 {
        return Ok(());
    }

    let row = height - 1;
    let (left, right) = footer_parts(width, plan_picker_open);
    execute!(out, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
    write!(out, "{left}")?;

    if !right.is_empty() {
        let right_col = width.saturating_sub(right.len()) as u16;
        execute!(out, MoveTo(right_col, row))?;
        write!(out, "{}", right.with(Color::Grey))?;
    }
    Ok(())
}

fn footer_parts(width: usize, plan_picker_open: bool) -> (String, String) {
    if width == 0 {
        return (String::new(), String::new());
    }

    let left_text = if plan_picker_open {
        "Plan selector: arrows or 1-7 move | Enter apply | P or Esc close"
    } else {
        "Press P to change plan | q or Ctrl+C to quit."
    };
    let left = truncate(left_text, width);
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

fn render_compact_limits_row(limits: &RateLimits, width: usize) -> String {
    let bar_width = if width >= 90 { 8 } else { 6 };
    let primary = compact_limit_text(limits.primary.as_ref(), bar_width);
    let secondary = compact_limit_text(limits.secondary.as_ref(), bar_width);
    format!("Usage: 5h {primary} | 7d {secondary}")
}

fn limit_percent_text(window: Option<&UsageWindow>) -> String {
    window
        .map(|item| format!("{:.0}%", item.remaining_percent.clamp(0.0, 100.0)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn compact_limit_text(window: Option<&UsageWindow>, bar_width: usize) -> String {
    let Some(window) = window else {
        return "n/a".to_string();
    };
    let color = limit_color(window.remaining_percent);
    let pct_plain = limit_percent_text(Some(window));
    let pct = pct_plain.with(color).bold();
    let bar = progress_bar(window.remaining_percent, bar_width).with(color);
    format!("{pct} {bar}")
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

fn resolve_effective_logo_path(raw_path: Option<&str>) -> Option<PathBuf> {
    let value = raw_path?;
    let resolved = resolve_logo_path(value);
    resolved.exists().then_some(resolved)
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

fn render_plan_picker_screen(
    out: &mut impl Write,
    width: u16,
    height: u16,
    plan_picker: PlanPickerView,
) -> Result<()> {
    let presets = plan_presets();
    if presets.is_empty() || width < 24 || height < 10 {
        return Ok(());
    }

    let width = width as usize;
    let body_bottom = height.saturating_sub(FOOTER_ROWS);
    render_plan_picker_backdrop(out, width, body_bottom)?;

    let current_label = presets
        .get(plan_picker.current_index)
        .map(|preset| preset.label)
        .unwrap_or("Auto Detect");
    let selected_label = presets
        .get(plan_picker.selected_index)
        .map(|preset| preset.label)
        .unwrap_or("Auto Detect");

    let mut lines: Vec<String> = Vec::with_capacity(presets.len() + 10);
    lines.push("Account Plan Selector".to_string());
    lines.push("Choose the OpenAI plan shown in Discord and the TUI.".to_string());
    lines.push("".to_string());
    lines.push(format!("Current setting : {current_label}"));
    lines.push(format!("Selected option : {selected_label}"));
    lines.push("".to_string());
    for (idx, preset) in presets.iter().copied().enumerate() {
        let active_suffix = if idx == plan_picker.current_index {
            " [active]"
        } else {
            ""
        };
        lines.push(format!(
            "[{}] {:<10} {}{}",
            idx + 1,
            preset.label,
            plan_preset_detail(preset),
            active_suffix
        ));
    }
    lines.push("".to_string());
    lines.push("Enter applies immediately and saves to config.".to_string());
    lines.push("Press P or Esc to close without changing the plan.".to_string());

    let inner_width = lines
        .iter()
        .map(|line| line.len())
        .max()
        .unwrap_or(24)
        .min(width.saturating_sub(6))
        .max(24);
    let box_width = inner_width + 4;
    let box_height = (lines.len() + 2) as u16;
    let start_col = width.saturating_sub(box_width) / 2;
    let show_codex_header = width >= banner_ascii_width(&CODEX_ASCII) + 2
        && body_bottom >= box_height + CODEX_ASCII.len() as u16 + 3;
    let start_row = if show_codex_header {
        let group_height = CODEX_ASCII.len() as u16 + 1 + box_height;
        let group_top = body_bottom.saturating_sub(group_height) / 2;
        let mut header_row = group_top;
        draw_codex_ascii_banner(out, &mut header_row, body_bottom, width, 0)?;
        header_row.saturating_add(1)
    } else {
        body_bottom.saturating_sub(box_height) / 2
    };

    let top = format!("+{}+", "-".repeat(inner_width + 2));
    let bottom = top.clone();
    execute!(out, MoveTo(start_col as u16, start_row))?;
    write!(out, "{}", top.as_str().with(Color::Grey))?;

    for (idx, line) in lines.iter().enumerate() {
        let row = start_row + idx as u16 + 1;
        let is_option = idx >= 6 && idx < 6 + presets.len();
        let mut content = line.clone();
        if is_option {
            let option_index = idx - 6;
            let prefix = if option_index == plan_picker.selected_index {
                ">"
            } else {
                " "
            };
            content = format!("{prefix} {line}");
        }
        let padded = format!(
            " {:<width$} ",
            truncate(&content, inner_width),
            width = inner_width
        );
        execute!(out, MoveTo(start_col as u16, row))?;
        write!(out, "{}", "|".with(Color::Grey))?;
        if idx == 0 {
            write!(out, "{}", padded.as_str().with(Color::White).bold())?;
        } else if idx == 1 || idx == 3 {
            write!(out, "{}", padded.as_str().with(Color::Grey))?;
        } else if idx == 4 {
            write!(out, "{}", padded.as_str().with(Color::White).bold())?;
        } else if idx == 5 {
            write!(out, "{}", padded.as_str().with(Color::DarkGrey))?;
        } else if is_option && idx - 6 == plan_picker.selected_index {
            write!(out, "{}", padded.as_str().black().on_white().bold())?;
        } else if is_option && idx - 6 == plan_picker.current_index {
            write!(out, "{}", padded.as_str().with(Color::Green))?;
        } else if is_option {
            write!(out, "{}", padded.as_str().with(Color::Grey))?;
        } else if idx + 2 >= lines.len() {
            write!(out, "{}", padded.as_str().with(Color::DarkGrey))?;
        } else {
            write!(out, "{padded}")?;
        }
        write!(out, "{}", "|".with(Color::Grey))?;
    }

    let bottom_row = start_row + box_height - 1;
    execute!(out, MoveTo(start_col as u16, bottom_row))?;
    write!(out, "{}", bottom.as_str().with(Color::Grey))?;
    Ok(())
}

fn plan_preset_detail(preset: PlanPreset) -> &'static str {
    match (preset.mode, preset.tier) {
        (OpenAiPlanMode::Auto, None) => "Use detected account telemetry",
        (OpenAiPlanMode::Manual, Some(OpenAiPlanTier::Free)) => "$0 personal plan",
        (OpenAiPlanMode::Manual, Some(OpenAiPlanTier::Go)) => "$8 personal plan",
        (OpenAiPlanMode::Manual, Some(OpenAiPlanTier::Plus)) => "$20 personal plan",
        (OpenAiPlanMode::Manual, Some(OpenAiPlanTier::Pro)) => "$200 power-user plan",
        (OpenAiPlanMode::Manual, Some(OpenAiPlanTier::Business)) => "Team workspace plan",
        (OpenAiPlanMode::Manual, Some(OpenAiPlanTier::Enterprise)) => "Enterprise workspace plan",
        _ => "Manual override",
    }
}

fn render_plan_picker_backdrop(
    out: &mut impl Write,
    width: usize,
    max_body_row: u16,
) -> Result<()> {
    if width < 16 || max_body_row == 0 {
        return Ok(());
    }

    const GRID_COL_SPAN: usize = 10;
    const GRID_ROW_SPAN: u16 = 4;

    for row in 0..max_body_row {
        let horizontal = row % GRID_ROW_SPAN == 0;
        let mut line = String::with_capacity(width);
        for col in 0..width {
            let vertical = col % GRID_COL_SPAN == 0;
            let ch = match (horizontal, vertical) {
                (true, true) => '+',
                (true, false) => '.',
                (false, true) => ':',
                (false, false) => ' ',
            };
            line.push(ch);
        }

        execute!(out, MoveTo(0, row))?;
        write!(out, "{}", line.as_str().with(Color::DarkGrey))?;
    }

    Ok(())
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
        let (left, right) = footer_parts(84, false);
        assert!(left.len() + 1 + right.len() <= 84);

        let (left_small, right_small) = footer_parts(20, false);
        assert_eq!(right_small, "");
        assert_eq!(left_small, "Press P to change...");
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
            BannerVariant::CompactText
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
}
