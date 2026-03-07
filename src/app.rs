use std::env;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use std::{io, io::IsTerminal};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tracing::debug;

use crate::config::{
    self, OpenAiPlanDisplayConfig, PresenceConfig, PresenceSurface, RuntimeSettings,
    apply_plan_preset, plan_preset_index, plan_presets,
};
use crate::discord::DiscordPresence;
use crate::metrics::MetricsTracker;
use crate::process_guard::{self, RunningState};
use crate::session::{
    CodexSessionSnapshot, EffectiveLimitSelection, GitBranchCache, RateLimits, SessionParseCache,
    collect_active_sessions_multi, collect_active_sessions_multi_with_diagnostics,
    latest_limits_source, preferred_active_session,
};
use crate::telemetry::plan::{PlanDetector, ResolvedPlan, is_model_allowed_for_plan};
use crate::telemetry::service_tier::{ResolvedServiceTier, resolve_service_tier};
use crate::ui::{self, RenderData};
use crate::util::{
    format_cost, format_model_display, format_since, format_time_until, format_token_triplet,
};

const RELAUNCH_GUARD_ENV: &str = "CODEX_PRESENCE_TERMINAL_RELAUNCHED";

#[derive(Debug, Clone)]
pub enum AppMode {
    SmartForeground,
    CodexChild { args: Vec<String> },
}

#[derive(Debug, Default)]
struct RuntimeSnapshot {
    sessions: Vec<CodexSessionSnapshot>,
    limits_source: Option<EffectiveLimitSelection>,
    resolved_plan: ResolvedPlan,
    resolved_service_tier: ResolvedServiceTier,
}

impl RuntimeSnapshot {
    fn from_sessions(
        sessions: Vec<CodexSessionSnapshot>,
        plan_detector: &mut PlanDetector,
        plan_config: &OpenAiPlanDisplayConfig,
    ) -> Self {
        let limits_source = latest_limits_source(&sessions);
        let resolved_plan = plan_detector.resolve_from_sessions(&sessions, plan_config);
        let resolved_service_tier = resolve_service_tier();

        Self {
            sessions,
            limits_source,
            resolved_plan,
            resolved_service_tier,
        }
    }

    fn active_session(&self) -> Option<&CodexSessionSnapshot> {
        preferred_active_session(&self.sessions)
    }

    fn effective_limits(&self) -> Option<&RateLimits> {
        self.limits_source.as_ref().map(|source| &source.limits)
    }
}

pub fn run(config: PresenceConfig, mode: AppMode, runtime: RuntimeSettings) -> Result<()> {
    match mode {
        AppMode::SmartForeground => run_foreground_tui(config, runtime),
        AppMode::CodexChild { args } => run_codex_wrapper(config, runtime, args),
    }
}

pub fn print_status(config: &PresenceConfig) -> Result<()> {
    let runtime = config::runtime_settings();
    let session_roots = config::sessions_paths();
    let mut cache = GitBranchCache::new(Duration::from_secs(30));
    let mut parse_cache = SessionParseCache::default();
    let (sessions, diagnostics) = collect_active_sessions_multi_with_diagnostics(
        &session_roots,
        runtime.stale_threshold,
        runtime.active_sticky_window,
        &mut cache,
        &mut parse_cache,
        &config.pricing,
    )?;
    let running = process_guard::inspect_running_instance()?;
    let (is_running, running_pid) = match running {
        RunningState::NotRunning => (false, None),
        RunningState::Running { pid } => (true, pid),
    };

    println!("codex-discord-presence status");
    println!("running: {is_running}");
    if let Some(pid) = running_pid {
        println!("pid: {pid}");
    }
    println!("config: {}", config::config_path().display());
    print_session_roots("sessions_dirs", &session_roots);
    let default_client_id = config.effective_client_id_for_surface(PresenceSurface::Default);
    let desktop_client_id = config.effective_client_id_for_surface(PresenceSurface::Desktop);
    println!(
        "discord_client_id_default: {}",
        if default_client_id.is_some() {
            "configured"
        } else {
            "missing"
        }
    );
    println!(
        "discord_client_id_desktop: {}",
        if desktop_client_id.is_some() {
            "configured"
        } else {
            "missing"
        }
    );
    println!("active_sessions: {}", sessions.len());
    println!("session_files_seen: {}", diagnostics.session_files_seen);
    println!("discarded_stale: {}", diagnostics.dropped_stale);
    println!(
        "discarded_outside_sticky: {}",
        diagnostics.dropped_outside_sticky
    );
    let mut plan_detector = PlanDetector::new();
    let snapshot =
        RuntimeSnapshot::from_sessions(sessions, &mut plan_detector, &config.openai_plan);
    if let Some(active) = snapshot.active_session() {
        if let Some(source) = &snapshot.limits_source {
            println!("limits_source_session: {}", source.source_session_id);
            println!("limits_source: {}", source.source_label());
            println!("limits_updated: {}", format_since(source.observed_at));
        }
        print_active_summary(
            active,
            snapshot.effective_limits(),
            snapshot.limits_source.as_ref(),
            &snapshot.resolved_plan,
            &snapshot.resolved_service_tier,
            config,
        );
    }
    Ok(())
}

pub fn doctor(config: &PresenceConfig) -> Result<u8> {
    let mut issues = 0u8;
    let session_roots = config::sessions_paths();
    let existing_roots: Vec<&PathBuf> = session_roots.iter().filter(|path| path.exists()).collect();

    println!("codex-discord-presence doctor");
    println!("config_path: {}", config::config_path().display());
    print_session_roots("sessions_paths", &session_roots);

    if existing_roots.is_empty() {
        issues += 1;
        println!("[WARN] No discovered Codex sessions directory is currently accessible.");
    } else {
        println!(
            "[OK] Discovered {} accessible sessions root(s).",
            existing_roots.len()
        );
    }

    let default_client_id = config.effective_client_id_for_surface(PresenceSurface::Default);
    let desktop_client_id = config.effective_client_id_for_surface(PresenceSurface::Desktop);
    if default_client_id.is_none() && desktop_client_id.is_none() {
        issues += 1;
        println!("[WARN] Discord client ids are not configured.");
    } else {
        println!(
            "[OK] Discord client ids: default={} desktop={}",
            if default_client_id.is_some() {
                "configured"
            } else {
                "missing"
            },
            if desktop_client_id.is_some() {
                "configured"
            } else {
                "missing"
            }
        );
    }

    if command_available("codex") {
        println!("[OK] codex command available.");
    } else if !existing_roots.is_empty() {
        println!(
            "[INFO] codex command not found in PATH (session-file monitoring can still work)."
        );
    } else {
        issues += 1;
        println!("[WARN] codex command not found in PATH.");
    }

    if command_available("git") {
        println!("[OK] git command available.");
    } else {
        issues += 1;
        println!("[WARN] git command not found in PATH.");
    }

    if issues == 0 {
        println!("Doctor: healthy");
        Ok(0)
    } else {
        println!("Doctor: {issues} issue(s) found");
        Ok(1)
    }
}

fn run_foreground_tui(mut config: PresenceConfig, runtime: RuntimeSettings) -> Result<()> {
    let stop = install_stop_signal()?;
    if !io::stdout().is_terminal() {
        if maybe_relaunch_in_terminal()? {
            return Ok(());
        }
        return run_headless_foreground(config, runtime, stop);
    }

    let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
    let mut parse_cache = SessionParseCache::default();
    let mut discord = DiscordPresence::new(config.effective_client_id());
    let mut metrics_tracker = MetricsTracker::new();
    let mut plan_detector = PlanDetector::new();
    let sessions_roots = config::sessions_paths();
    let started = Instant::now();
    let mut last_tick = Instant::now() - runtime.poll_interval;
    let mut snapshot = RuntimeSnapshot::default();
    let mut last_render_signature = String::new();
    let mut last_render_at = Instant::now() - Duration::from_secs(31);
    let mut force_redraw = true;
    let mut plan_picker_open = false;
    let mut plan_picker_selected = plan_preset_index(&config.openai_plan);

    ui::enter_terminal()?;

    let mut run = || -> Result<()> {
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            if last_tick.elapsed() >= runtime.poll_interval {
                snapshot = collect_runtime_snapshot(
                    &sessions_roots,
                    &runtime,
                    &config,
                    &mut git_cache,
                    &mut parse_cache,
                    &mut metrics_tracker,
                    &mut plan_detector,
                )?;
                publish_runtime_snapshot(&mut discord, &snapshot, &config);

                let active = snapshot.active_session();
                let plan_display_label =
                    snapshot.resolved_plan.label(config.openai_plan.show_price);
                let plan_status_label = snapshot.resolved_plan.status_label();
                let fast_mode_label = snapshot.resolved_service_tier.fast_mode_label();
                let limits_source_label = snapshot
                    .limits_source
                    .as_ref()
                    .map(|selection| selection.source_label())
                    .unwrap_or_else(|| "Awaiting account quota telemetry".to_string());
                let limits_updated_label = snapshot
                    .limits_source
                    .as_ref()
                    .map(|selection| format_since(selection.observed_at))
                    .unwrap_or_else(|| "not yet synced".to_string());
                let spark_plan_warning = active
                    .and_then(|session| session.model.as_deref())
                    .and_then(|model| {
                        (!is_model_allowed_for_plan(model, snapshot.resolved_plan.tier))
                            .then_some("Spark is Pro-only; received non-Pro telemetry (anomaly)")
                    });
                let render = RenderData {
                    running_for: started.elapsed(),
                    mode_label: "Smart Foreground",
                    discord_status: discord.status(),
                    client_id_configured: config
                        .effective_client_id_for_surface(PresenceSurface::Default)
                        .is_some()
                        || config
                            .effective_client_id_for_surface(PresenceSurface::Desktop)
                            .is_some(),
                    poll_interval_secs: runtime.poll_interval.as_secs(),
                    stale_secs: runtime.stale_threshold.as_secs(),
                    show_activity: config.privacy.show_activity,
                    show_activity_target: config.privacy.show_activity_target,
                    plan_display_label: plan_display_label.as_str(),
                    plan_status_label: plan_status_label.as_str(),
                    fast_mode_label,
                    fast_active: snapshot.resolved_service_tier.is_fast(),
                    limits_source_label: limits_source_label.as_str(),
                    limits_updated_label: limits_updated_label.as_str(),
                    spark_plan_warning,
                    logo_mode: config.display.terminal_logo_mode.clone(),
                    logo_path: config.display.terminal_logo_path.as_deref(),
                    banner_phase: ((started.elapsed().as_millis() / 450) % 8) as u8,
                    active,
                    effective_limits: snapshot.effective_limits(),
                    metrics: metrics_tracker.snapshot(),
                    sessions: &snapshot.sessions,
                    plan_picker: plan_picker_open.then_some(ui::PlanPickerView {
                        selected_index: plan_picker_selected,
                        current_index: plan_preset_index(&config.openai_plan),
                    }),
                };
                let signature = ui::frame_signature(&render);
                let should_draw = force_redraw
                    || signature != last_render_signature
                    || last_render_at.elapsed() >= Duration::from_secs(30);
                if should_draw {
                    ui::draw(&render)?;
                    last_render_signature = signature;
                    last_render_at = Instant::now();
                    force_redraw = false;
                }
                last_tick = Instant::now();
            }

            let wait_budget = if last_tick.elapsed() >= runtime.poll_interval {
                Duration::from_millis(10)
            } else {
                runtime
                    .poll_interval
                    .saturating_sub(last_tick.elapsed())
                    .min(Duration::from_millis(250))
            };

            if event::poll(wait_budget)? {
                match event::read()? {
                    Event::Key(key) => {
                        if matches!(key.kind, KeyEventKind::Release) {
                            continue;
                        }

                        if plan_picker_open {
                            if is_plan_picker_toggle_key(&key)
                                || (key.code == KeyCode::Esc && key.modifiers.is_empty())
                            {
                                plan_picker_open = false;
                                request_redraw(
                                    &mut force_redraw,
                                    &mut last_tick,
                                    runtime.poll_interval,
                                );
                            } else {
                                match key.code {
                                    KeyCode::Up | KeyCode::Left => {
                                        let preset_count = plan_presets().len();
                                        if preset_count > 0 {
                                            plan_picker_selected =
                                                (plan_picker_selected + preset_count - 1)
                                                    % preset_count;
                                            request_redraw(
                                                &mut force_redraw,
                                                &mut last_tick,
                                                runtime.poll_interval,
                                            );
                                        }
                                    }
                                    KeyCode::Down | KeyCode::Right | KeyCode::Tab => {
                                        let preset_count = plan_presets().len();
                                        if preset_count > 0 {
                                            plan_picker_selected =
                                                (plan_picker_selected + 1) % preset_count;
                                            request_redraw(
                                                &mut force_redraw,
                                                &mut last_tick,
                                                runtime.poll_interval,
                                            );
                                        }
                                    }
                                    KeyCode::Char(digit @ '1'..='7') => {
                                        let target = (digit as u8 - b'1') as usize;
                                        if target < plan_presets().len() {
                                            plan_picker_selected = target;
                                            request_redraw(
                                                &mut force_redraw,
                                                &mut last_tick,
                                                runtime.poll_interval,
                                            );
                                        }
                                    }
                                    KeyCode::Enter => {
                                        apply_plan_preset(
                                            &mut config.openai_plan,
                                            plan_picker_selected,
                                        );
                                        config.save()?;
                                        plan_picker_open = false;
                                        request_redraw(
                                            &mut force_redraw,
                                            &mut last_tick,
                                            runtime.poll_interval,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        } else if is_plan_picker_toggle_key(&key) {
                            plan_picker_selected = plan_preset_index(&config.openai_plan);
                            plan_picker_open = true;
                            request_redraw(
                                &mut force_redraw,
                                &mut last_tick,
                                runtime.poll_interval,
                            );
                        } else if key.code == KeyCode::Char('q')
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            break;
                        }
                    }
                    Event::Resize(_, _) => {
                        request_redraw(&mut force_redraw, &mut last_tick, runtime.poll_interval);
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    };

    let run_result = run();
    discord.shutdown();
    let _ = ui::leave_terminal();
    run_result
}

fn run_headless_foreground(
    config: PresenceConfig,
    runtime: RuntimeSettings,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
    let mut parse_cache = SessionParseCache::default();
    let mut discord = DiscordPresence::new(config.effective_client_id());
    let mut metrics_tracker = MetricsTracker::new();
    let mut plan_detector = PlanDetector::new();
    let sessions_roots = config::sessions_paths();
    println!("No interactive terminal detected; running in headless foreground mode.");
    println!("Press Ctrl+C to stop.");

    while !stop.load(Ordering::Relaxed) {
        let snapshot = collect_runtime_snapshot(
            &sessions_roots,
            &runtime,
            &config,
            &mut git_cache,
            &mut parse_cache,
            &mut metrics_tracker,
            &mut plan_detector,
        )?;
        publish_runtime_snapshot(&mut discord, &snapshot, &config);
        thread::sleep(runtime.poll_interval);
    }

    discord.shutdown();
    Ok(())
}

fn maybe_relaunch_in_terminal() -> Result<bool> {
    if env::var_os(RELAUNCH_GUARD_ENV).is_some() {
        return Ok(false);
    }

    let exe = env::current_exe().context("failed to resolve current executable path")?;
    let args: Vec<String> = env::args().skip(1).collect();

    #[cfg(windows)]
    {
        return relaunch_windows(&exe.display().to_string(), &args);
    }

    #[cfg(target_os = "macos")]
    {
        return relaunch_macos(&exe.display().to_string(), &args);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        return Ok(relaunch_linux_like(&exe.display().to_string(), &args));
    }

    #[allow(unreachable_code)]
    Ok(false)
}

#[cfg(windows)]
fn relaunch_windows(exe: &str, args: &[String]) -> Result<bool> {
    let escaped_exe = escape_powershell_single_quoted(exe);
    let escaped_args = args
        .iter()
        .map(|arg| format!("'{}'", escape_powershell_single_quoted(arg)))
        .collect::<Vec<_>>()
        .join(", ");
    let argument_list = if escaped_args.is_empty() {
        "@()".to_string()
    } else {
        format!("@({escaped_args})")
    };

    let command = format!(
        "$env:{RELAUNCH_GUARD_ENV}='1'; Start-Process -FilePath '{escaped_exe}' -ArgumentList {argument_list}"
    );
    let status = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(status.map(|s| s.success()).unwrap_or(false))
}

#[cfg(target_os = "macos")]
fn relaunch_macos(exe: &str, args: &[String]) -> Result<bool> {
    let command = build_unix_shell_command(exe, args);
    let mut apple_script_command = String::new();
    for ch in command.chars() {
        match ch {
            '\\' => apple_script_command.push_str("\\\\"),
            '"' => apple_script_command.push_str("\\\""),
            _ => apple_script_command.push(ch),
        }
    }

    let status = Command::new("osascript")
        .arg("-e")
        .arg(format!(
            "tell application \"Terminal\" to do script \"{apple_script_command}\""
        ))
        .arg("-e")
        .arg("tell application \"Terminal\" to activate")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(status.map(|s| s.success()).unwrap_or(false))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn relaunch_linux_like(exe: &str, args: &[String]) -> bool {
    let command = build_unix_shell_command(exe, args);
    let terminal_candidates: [(&str, &[&str]); 7] = [
        ("x-terminal-emulator", &["--", "bash", "-lc"]),
        ("gnome-terminal", &["--", "bash", "-lc"]),
        ("konsole", &["-e", "bash", "-lc"]),
        ("xfce4-terminal", &["--command", "bash -lc"]),
        ("alacritty", &["-e", "bash", "-lc"]),
        ("kitty", &["-e", "bash", "-lc"]),
        ("wezterm", &["start", "--", "bash", "-lc"]),
    ];

    for (terminal, prefix) in terminal_candidates {
        let spawned = if terminal == "xfce4-terminal" {
            Command::new(terminal)
                .arg(prefix[0])
                .arg(format!("bash -lc {}", shell_escape_single(&command)))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        } else {
            let mut cmd = Command::new(terminal);
            for part in prefix {
                cmd.arg(part);
            }
            cmd.arg(&command)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
        };

        if spawned.is_ok() {
            return true;
        }
    }

    false
}

#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
fn build_unix_shell_command(exe: &str, args: &[String]) -> String {
    use std::fmt::Write as _;

    let mut command = String::new();
    let _ = write!(
        command,
        "{RELAUNCH_GUARD_ENV}=1 {}",
        shell_escape_single(exe)
    );
    for arg in args {
        let _ = write!(command, " {}", shell_escape_single(arg));
    }
    command
}

#[cfg(any(target_os = "macos", all(unix, not(target_os = "macos"))))]
fn shell_escape_single(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn escape_powershell_single_quoted(input: &str) -> String {
    input.replace('\'', "''")
}

fn run_codex_wrapper(
    config: PresenceConfig,
    runtime: RuntimeSettings,
    args: Vec<String>,
) -> Result<()> {
    let stop = install_stop_signal()?;
    let mut child = spawn_codex_child(args)?;
    let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
    let mut parse_cache = SessionParseCache::default();
    let mut discord = DiscordPresence::new(config.effective_client_id());
    let mut metrics_tracker = MetricsTracker::new();
    let mut plan_detector = PlanDetector::new();
    let sessions_roots = config::sessions_paths();

    println!("codex child started; Discord presence tracking is active.");

    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            break;
        }

        let snapshot = collect_runtime_snapshot(
            &sessions_roots,
            &runtime,
            &config,
            &mut git_cache,
            &mut parse_cache,
            &mut metrics_tracker,
            &mut plan_detector,
        )?;
        publish_runtime_snapshot(&mut discord, &snapshot, &config);

        if let Some(status) = child
            .try_wait()
            .context("failed to query codex child status")?
        {
            println!("codex exited with status: {status}");
            break;
        }

        thread::sleep(runtime.poll_interval);
    }

    discord.shutdown();
    Ok(())
}

fn spawn_codex_child(args: Vec<String>) -> Result<Child> {
    let mut command = Command::new("codex");
    command
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    command
        .spawn()
        .context("failed to spawn `codex` child process")
}

fn collect_runtime_snapshot(
    sessions_roots: &[PathBuf],
    runtime: &RuntimeSettings,
    config: &PresenceConfig,
    git_cache: &mut GitBranchCache,
    parse_cache: &mut SessionParseCache,
    metrics_tracker: &mut MetricsTracker,
    plan_detector: &mut PlanDetector,
) -> Result<RuntimeSnapshot> {
    let sessions = collect_active_sessions_multi(
        sessions_roots,
        runtime.stale_threshold,
        runtime.active_sticky_window,
        git_cache,
        parse_cache,
        &config.pricing,
    )?;
    metrics_tracker.update(&sessions);
    metrics_tracker.persist_if_due();
    Ok(RuntimeSnapshot::from_sessions(
        sessions,
        plan_detector,
        &config.openai_plan,
    ))
}

fn publish_runtime_snapshot(
    discord: &mut DiscordPresence,
    snapshot: &RuntimeSnapshot,
    config: &PresenceConfig,
) {
    if let Err(err) = discord.update(
        snapshot.active_session(),
        snapshot.effective_limits(),
        &snapshot.resolved_plan,
        &snapshot.resolved_service_tier,
        config,
    ) {
        debug!(error = %err, "discord presence update failed");
    }
}

fn print_active_summary(
    active: &CodexSessionSnapshot,
    effective_limits: Option<&RateLimits>,
    limits_source: Option<&EffectiveLimitSelection>,
    resolved_plan: &ResolvedPlan,
    resolved_service_tier: &ResolvedServiceTier,
    config: &PresenceConfig,
) {
    let plan_display_label = resolved_plan.label(config.openai_plan.show_price);
    println!("active_session:");
    println!("  session_id: {}", active.session_id);
    println!("  project: {}", active.project_name);
    println!("  path: {}", active.cwd.display());
    if let Some(started_at) = active.started_at.as_ref() {
        let started_at_iso = started_at.to_rfc3339();
        let started_at_since = format_since(Some(started_at.to_owned()));
        println!("  started_at: {started_at_iso} ({started_at_since})");
    } else {
        println!("  started_at: n/a");
    }
    let last_activity_dt: DateTime<Utc> = DateTime::<Utc>::from(active.last_activity);
    let last_activity_iso = last_activity_dt.to_rfc3339();
    let last_activity_since = format_since(Some(last_activity_dt));
    println!("  last_activity: {last_activity_iso} ({last_activity_since})");
    println!("  recency_source: {}", recency_source_label(active));
    println!(
        "  model: {} | {}",
        format_model_display(
            active.model.as_deref().unwrap_or("unknown"),
            active.reasoning_effort,
            resolved_service_tier.is_fast(),
        ),
        plan_display_label
    );
    println!("  plan: {}", resolved_plan.status_label());
    println!("  fast_mode: {}", resolved_service_tier.fast_mode_label());
    if let Some(raw_tier) = resolved_service_tier.raw_tier.as_deref() {
        println!("  service_tier: {raw_tier}");
    }
    if let Some(reasoning_effort) = active.reasoning_effort {
        println!("  reasoning_effort: {}", reasoning_effort.label());
    } else {
        println!("  reasoning_effort: n/a");
    }
    println!(
        "  branch: {}",
        active.git_branch.as_deref().unwrap_or("n/a")
    );
    if config.privacy.show_activity
        && let Some(activity) = &active.activity
    {
        println!(
            "  activity: {}",
            activity.to_text(config.privacy.show_activity_target)
        );
    }
    if active.total_cost_usd > 0.0 {
        println!("  cost: {}", format_cost(active.total_cost_usd));
    }
    println!(
        "  tokens io: in {} | cached {} | out {}",
        crate::util::format_tokens(active.input_tokens_total),
        crate::util::format_tokens(active.cached_input_tokens_total),
        crate::util::format_tokens(active.output_tokens_total),
    );
    println!(
        "  {}",
        format_token_triplet(
            active.session_delta_tokens,
            active.last_turn_tokens,
            active.session_total_tokens
        )
    );
    if let Some(context) = &active.context_window {
        println!(
            "  context: {}/{} used ({:.0}% left)",
            crate::util::format_tokens(context.used_tokens),
            crate::util::format_tokens(context.window_tokens),
            context.remaining_percent
        );
    } else {
        println!("  context: n/a");
    }
    if let Some(source) = limits_source {
        println!("  limits source: {}", source.source_label());
        println!("  limits updated: {}", format_since(source.observed_at));
    }

    let limits = effective_limits.unwrap_or(&active.limits);
    if let Some(primary) = &limits.primary {
        println!(
            "  5h remaining: {:.0}% (reset {})",
            primary.remaining_percent,
            format_time_until(primary.resets_at)
        );
    }
    if let Some(secondary) = &limits.secondary {
        println!(
            "  7d remaining: {:.0}% (reset {})",
            secondary.remaining_percent,
            format_time_until(secondary.resets_at)
        );
    }
    if let Some(model) = active.model.as_deref()
        && !is_model_allowed_for_plan(model, resolved_plan.tier)
    {
        println!("  model gate: Spark is Pro-only (telemetry anomaly)");
    }
}

fn command_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn print_session_roots(label: &str, paths: &[PathBuf]) {
    println!("{label}:");
    for path in paths {
        println!("  - {}", path.display());
    }
}

fn recency_source_label(active: &CodexSessionSnapshot) -> &'static str {
    let last_activity = active.last_activity;
    if let Some(activity) = &active.activity {
        if activity
            .last_effective_signal_at
            .and_then(datetime_to_system_time)
            == Some(last_activity)
        {
            return "activity.last_effective_signal_at";
        }
        if activity.last_active_at.and_then(datetime_to_system_time) == Some(last_activity) {
            return "activity.last_active_at";
        }
        if activity.observed_at.and_then(datetime_to_system_time) == Some(last_activity) {
            return "activity.observed_at";
        }
    }
    if active.last_token_event_at.and_then(datetime_to_system_time) == Some(last_activity) {
        return "last_token_event_at";
    }
    "file_modified_or_fallback"
}

fn datetime_to_system_time(ts: DateTime<Utc>) -> Option<SystemTime> {
    if ts.timestamp() < 0 {
        return None;
    }
    let secs = ts.timestamp() as u64;
    let nanos = ts.timestamp_subsec_nanos() as u64;
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(secs))?
        .checked_add(Duration::from_nanos(nanos))
}

fn install_stop_signal() -> Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    ctrlc::set_handler(move || {
        flag.store(true, Ordering::Relaxed);
    })
    .context("failed to install Ctrl+C handler")?;
    Ok(stop)
}

fn is_plan_picker_toggle_key(key: &KeyEvent) -> bool {
    if !matches!(key.kind, KeyEventKind::Press) {
        return false;
    }

    matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
        && !key.modifiers.contains(KeyModifiers::CONTROL)
        && !key.modifiers.contains(KeyModifiers::ALT)
        && !key.modifiers.contains(KeyModifiers::SUPER)
}

fn request_redraw(force_redraw: &mut bool, last_tick: &mut Instant, poll_interval: Duration) {
    *force_redraw = true;
    *last_tick = Instant::now() - poll_interval;
}
