use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::config::PricingConfig;
use crate::cost::{PricingSource, TokenCostBreakdown};
pub use crate::telemetry::limits::{
    EffectiveLimitSelection, RateLimitEnvelope, RateLimitScope, RateLimits, UsageWindow,
};
use crate::telemetry::limits::{
    SessionLimitCandidate, limits_present as telemetry_limits_present,
    select_effective_limits_global_first,
};

mod activity;
mod parser;

use activity::{SessionAccumulator, looks_like_desktop_surface};
use parser::{fetch_git_branch, parse_session_file_cached};
#[cfg(test)]
use parser::{parse_new_lines, parse_session_file, parse_utc_timestamp};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContextWindowSource {
    #[default]
    Event,
    Catalog,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextWindowSnapshot {
    pub window_tokens: u64,
    pub used_tokens: u64,
    pub remaining_tokens: u64,
    pub remaining_percent: f64,
    pub source: ContextWindowSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionActivityKind {
    #[default]
    Idle,
    Thinking,
    ReadingFile,
    EditingFile,
    RunningCommand,
    WaitingInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionActivitySnapshot {
    pub kind: SessionActivityKind,
    pub target: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub last_effective_signal_at: Option<DateTime<Utc>>,
    pub idle_candidate_at: Option<DateTime<Utc>>,
    pub pending_calls: usize,
}

impl SessionActivitySnapshot {
    pub fn action_text(&self) -> &'static str {
        match self.kind {
            SessionActivityKind::Thinking => "Thinking",
            SessionActivityKind::ReadingFile => "Reading",
            SessionActivityKind::EditingFile => "Editing",
            SessionActivityKind::RunningCommand => "Running command",
            SessionActivityKind::WaitingInput => "Waiting for input",
            SessionActivityKind::Idle => "Idle",
        }
    }

    pub fn to_text(&self, show_target: bool) -> String {
        if show_target
            && let Some(target) = &self.target
            && !target.trim().is_empty()
        {
            return format!("{} {}", self.action_text(), target);
        }
        self.action_text().to_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl ReasoningEffort {
    pub fn parse(raw: Option<&str>) -> Option<Self> {
        let normalized = raw
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .filter(|value| !value.is_empty())?;
        match normalized.as_str() {
            "minimal" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" | "extra_high" | "extra-high" => Some(Self::XHigh),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "Extra High",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSessionSnapshot {
    pub session_id: String,
    pub cwd: PathBuf,
    pub project_name: String,
    pub git_branch: Option<String>,
    pub originator: Option<String>,
    pub source: Option<String>,
    pub model: Option<String>,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub approval_policy: Option<String>,
    pub sandbox_policy: Option<String>,
    pub session_total_tokens: Option<u64>,
    pub last_turn_tokens: Option<u64>,
    pub session_delta_tokens: Option<u64>,
    pub input_tokens_total: u64,
    pub cached_input_tokens_total: u64,
    pub output_tokens_total: u64,
    pub last_input_tokens: Option<u64>,
    pub last_cached_input_tokens: Option<u64>,
    pub last_output_tokens: Option<u64>,
    pub total_cost_usd: f64,
    pub cost_breakdown: TokenCostBreakdown,
    pub pricing_source: PricingSource,
    pub context_window: Option<ContextWindowSnapshot>,
    pub limits: RateLimits,
    pub rate_limit_envelopes: Vec<RateLimitEnvelope>,
    pub activity: Option<SessionActivitySnapshot>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_token_event_at: Option<DateTime<Utc>>,
    pub last_activity: SystemTime,
    pub source_file: PathBuf,
}

impl CodexSessionSnapshot {
    pub fn is_desktop_surface(&self) -> bool {
        self.originator
            .as_deref()
            .is_some_and(looks_like_desktop_surface)
            || self
                .source
                .as_deref()
                .is_some_and(looks_like_desktop_surface)
    }
}

#[derive(Debug, Default)]
pub struct GitBranchCache {
    ttl: Duration,
    entries: HashMap<PathBuf, CachedBranch>,
}

#[derive(Debug, Clone)]
struct CachedBranch {
    value: Option<String>,
    expires_at: Instant,
}

#[derive(Debug, Default)]
pub struct SessionParseCache {
    entries: HashMap<PathBuf, CachedSessionEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionCollectionDiagnostics {
    pub session_files_seen: usize,
    pub dropped_stale: usize,
    pub dropped_outside_sticky: usize,
}

#[derive(Debug)]
struct CachedSessionEntry {
    cursor: u64,
    file_len: u64,
    modified: SystemTime,
    accumulator: SessionAccumulator,
    snapshot: Option<CodexSessionSnapshot>,
    partial_line_buffer: String,
}

impl CachedSessionEntry {
    fn new(modified: SystemTime) -> Self {
        Self {
            cursor: 0,
            file_len: 0,
            modified,
            accumulator: SessionAccumulator::default(),
            snapshot: None,
            partial_line_buffer: String::new(),
        }
    }

    fn reset(&mut self, modified: SystemTime) {
        self.cursor = 0;
        self.file_len = 0;
        self.modified = modified;
        self.accumulator = SessionAccumulator::default();
        self.snapshot = None;
        self.partial_line_buffer.clear();
    }
}

impl GitBranchCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: HashMap::new(),
        }
    }

    pub fn get(&mut self, project_path: &Path) -> Option<String> {
        if project_path.as_os_str().is_empty() || !project_path.exists() {
            return None;
        }

        let key = project_path.to_path_buf();
        if let Some(cached) = self.entries.get(&key)
            && Instant::now() < cached.expires_at
        {
            return cached.value.clone();
        }

        let value = fetch_git_branch(project_path);
        self.entries.insert(
            key,
            CachedBranch {
                value: value.clone(),
                expires_at: Instant::now() + self.ttl,
            },
        );
        value
    }
}

pub fn collect_active_sessions(
    sessions_root: &Path,
    stale_threshold: Duration,
    active_sticky_window: Duration,
    git_cache: &mut GitBranchCache,
    parse_cache: &mut SessionParseCache,
    pricing_config: &PricingConfig,
) -> Result<Vec<CodexSessionSnapshot>> {
    collect_active_sessions_multi(
        &[sessions_root.to_path_buf()],
        stale_threshold,
        active_sticky_window,
        git_cache,
        parse_cache,
        pricing_config,
    )
}

pub fn collect_active_sessions_multi(
    sessions_roots: &[PathBuf],
    stale_threshold: Duration,
    active_sticky_window: Duration,
    git_cache: &mut GitBranchCache,
    parse_cache: &mut SessionParseCache,
    pricing_config: &PricingConfig,
) -> Result<Vec<CodexSessionSnapshot>> {
    let (sessions, _diagnostics) = collect_active_sessions_multi_with_diagnostics(
        sessions_roots,
        stale_threshold,
        active_sticky_window,
        git_cache,
        parse_cache,
        pricing_config,
    )?;
    Ok(sessions)
}

pub fn collect_active_sessions_multi_with_diagnostics(
    sessions_roots: &[PathBuf],
    stale_threshold: Duration,
    active_sticky_window: Duration,
    git_cache: &mut GitBranchCache,
    parse_cache: &mut SessionParseCache,
    pricing_config: &PricingConfig,
) -> Result<(Vec<CodexSessionSnapshot>, SessionCollectionDiagnostics)> {
    let now = SystemTime::now();
    let stale_cutoff = now
        .checked_sub(stale_threshold)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let sticky_cutoff = now
        .checked_sub(active_sticky_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut sessions = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let mut diagnostics = SessionCollectionDiagnostics::default();

    for sessions_root in sessions_roots {
        if !sessions_root.exists() {
            continue;
        }

        for entry in WalkDir::new(sessions_root)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            seen_paths.insert(path.to_path_buf());
            diagnostics.session_files_seen = diagnostics.session_files_seen.saturating_add(1);

            let metadata = match entry.metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            let modified = match metadata.modified() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if let Some(mut snapshot) = parse_session_file_cached(
                path,
                &metadata,
                modified,
                git_cache,
                parse_cache,
                pricing_config,
            )? {
                let recency = session_recency(&snapshot, modified);
                snapshot.last_activity = recency;
                match session_inclusion_decision(&snapshot, recency, stale_cutoff, sticky_cutoff) {
                    SessionInclusionDecision::Include => sessions.push(snapshot),
                    SessionInclusionDecision::DropStale => {
                        diagnostics.dropped_stale = diagnostics.dropped_stale.saturating_add(1);
                    }
                    SessionInclusionDecision::DropOutsideSticky => {
                        diagnostics.dropped_outside_sticky =
                            diagnostics.dropped_outside_sticky.saturating_add(1);
                    }
                }
            }
        }
    }

    parse_cache
        .entries
        .retain(|path, _| seen_paths.contains(path));
    sessions = dedupe_sessions_by_id(sessions);
    sessions.sort_by_key(|session| Reverse(session_rank_key(session)));
    Ok((sessions, diagnostics))
}

fn dedupe_sessions_by_id(sessions: Vec<CodexSessionSnapshot>) -> Vec<CodexSessionSnapshot> {
    let mut deduped: Vec<CodexSessionSnapshot> = Vec::new();
    let mut index_by_id: HashMap<String, usize> = HashMap::new();

    for session in sessions {
        let session_id = session.session_id.clone();
        if let Some(existing_index) = index_by_id.get(&session_id).copied() {
            if session_rank_key(&session) > session_rank_key(&deduped[existing_index]) {
                deduped[existing_index] = session;
            }
            continue;
        }

        let next_index = deduped.len();
        index_by_id.insert(session_id, next_index);
        deduped.push(session);
    }

    deduped
}

pub fn latest_limits_source(sessions: &[CodexSessionSnapshot]) -> Option<EffectiveLimitSelection> {
    let mut candidates: Vec<SessionLimitCandidate> = Vec::new();
    for session in sessions {
        if session.rate_limit_envelopes.is_empty() {
            if telemetry_limits_present(&session.limits) {
                candidates.push(SessionLimitCandidate {
                    session_id: session.session_id.clone(),
                    session_last_activity: session.last_activity,
                    envelope: RateLimitEnvelope {
                        limit_id: None,
                        limit_name: None,
                        plan_type: None,
                        observed_at: session.last_token_event_at,
                        scope: RateLimitScope::Other,
                        limits: session.limits.clone(),
                    },
                });
            }
            continue;
        }

        for envelope in &session.rate_limit_envelopes {
            if telemetry_limits_present(&envelope.limits) {
                candidates.push(SessionLimitCandidate {
                    session_id: session.session_id.clone(),
                    session_last_activity: session.last_activity,
                    envelope: envelope.clone(),
                });
            }
        }
    }

    select_effective_limits_global_first(&candidates)
}

pub fn preferred_active_session(
    sessions: &[CodexSessionSnapshot],
) -> Option<&CodexSessionSnapshot> {
    sessions
        .iter()
        .max_by_key(|session| session_rank_key(session))
}

pub fn limits_present(limits: &RateLimits) -> bool {
    telemetry_limits_present(limits)
}

#[cfg(test)]
fn should_include_session(
    snapshot: &CodexSessionSnapshot,
    recency: SystemTime,
    stale_cutoff: SystemTime,
    sticky_cutoff: SystemTime,
) -> bool {
    matches!(
        session_inclusion_decision(snapshot, recency, stale_cutoff, sticky_cutoff),
        SessionInclusionDecision::Include
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionInclusionDecision {
    Include,
    DropStale,
    DropOutsideSticky,
}

fn session_inclusion_decision(
    snapshot: &CodexSessionSnapshot,
    recency: SystemTime,
    stale_cutoff: SystemTime,
    sticky_cutoff: SystemTime,
) -> SessionInclusionDecision {
    if recency >= stale_cutoff {
        return SessionInclusionDecision::Include;
    }
    if recency < sticky_cutoff {
        return SessionInclusionDecision::DropOutsideSticky;
    }
    if snapshot
        .activity
        .as_ref()
        .is_some_and(session_activity_is_sticky_active)
    {
        SessionInclusionDecision::Include
    } else {
        SessionInclusionDecision::DropStale
    }
}

fn session_activity_is_sticky_active(activity: &SessionActivitySnapshot) -> bool {
    if activity.pending_calls > 0 {
        return true;
    }
    matches!(activity.kind, SessionActivityKind::WaitingInput)
        || is_working_activity_kind(&activity.kind)
}

fn session_rank_key(snapshot: &CodexSessionSnapshot) -> (SystemTime, usize, u8, String) {
    let (pending_calls, activity_priority) =
        snapshot
            .activity
            .as_ref()
            .map_or((0usize, 0u8), |activity| {
                (
                    activity.pending_calls,
                    session_activity_priority(&activity.kind),
                )
            });
    (
        snapshot.last_activity,
        pending_calls,
        activity_priority,
        snapshot.session_id.clone(),
    )
}

fn session_activity_priority(kind: &SessionActivityKind) -> u8 {
    match kind {
        SessionActivityKind::Thinking
        | SessionActivityKind::ReadingFile
        | SessionActivityKind::EditingFile
        | SessionActivityKind::RunningCommand => 3,
        SessionActivityKind::WaitingInput => 2,
        SessionActivityKind::Idle => 1,
    }
}

fn is_working_activity_kind(kind: &SessionActivityKind) -> bool {
    matches!(
        kind,
        SessionActivityKind::Thinking
            | SessionActivityKind::ReadingFile
            | SessionActivityKind::EditingFile
            | SessionActivityKind::RunningCommand
    )
}

fn session_recency(snapshot: &CodexSessionSnapshot, file_modified: SystemTime) -> SystemTime {
    let mut newest = SystemTime::UNIX_EPOCH;

    if let Some(activity) = &snapshot.activity {
        for candidate in [
            activity.last_effective_signal_at,
            activity.last_active_at,
            activity.observed_at,
        ] {
            if let Some(ts) = candidate.and_then(datetime_to_system_time)
                && ts > newest
            {
                newest = ts;
            }
        }
    }

    if let Some(ts) = snapshot
        .last_token_event_at
        .and_then(datetime_to_system_time)
        && ts > newest
    {
        newest = ts;
    }

    if newest > SystemTime::UNIX_EPOCH {
        newest
    } else {
        file_modified
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::BufReader;

    use crate::config::PricingConfig;
    use crate::cost::{PricingSource, TokenCostBreakdown};
    use chrono::Duration as ChronoDuration;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn parse_one(content: &str) -> CodexSessionSnapshot {
        let tmp = TempDir::new().expect("temp dir");
        let file_path = tmp.path().join("session.jsonl");
        std::fs::write(&file_path, content).expect("write jsonl");
        let modified = SystemTime::now();
        let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
        parse_session_file(
            &file_path,
            modified,
            &mut git_cache,
            &PricingConfig::default(),
        )
        .expect("parse")
        .expect("snapshot")
    }

    fn policy_snapshot(activity_kind: Option<SessionActivityKind>) -> CodexSessionSnapshot {
        CodexSessionSnapshot {
            session_id: "policy".to_string(),
            cwd: PathBuf::from("."),
            project_name: "policy-project".to_string(),
            git_branch: None,
            originator: None,
            source: None,
            model: None,
            reasoning_effort: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: None,
            last_turn_tokens: None,
            session_delta_tokens: None,
            input_tokens_total: 0,
            cached_input_tokens_total: 0,
            output_tokens_total: 0,
            last_input_tokens: None,
            last_cached_input_tokens: None,
            last_output_tokens: None,
            total_cost_usd: 0.0,
            cost_breakdown: TokenCostBreakdown::default(),
            pricing_source: PricingSource::Fallback,
            context_window: None,
            limits: RateLimits::default(),
            rate_limit_envelopes: Vec::new(),
            activity: activity_kind.map(|kind| SessionActivitySnapshot {
                kind,
                target: None,
                observed_at: Some(Utc::now()),
                last_active_at: Some(Utc::now()),
                last_effective_signal_at: Some(Utc::now()),
                idle_candidate_at: None,
                pending_calls: 0,
            }),
            started_at: None,
            last_token_event_at: None,
            last_activity: SystemTime::now(),
            source_file: PathBuf::from("policy.jsonl"),
        }
    }

    #[test]
    fn parses_tokens_delta_and_remaining_limits() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-09T16:33:13Z","type":"session_meta","payload":{"id":"abc-123","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:34:13Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1500},"last_token_usage":{"total_tokens":300}},"rate_limits":{"primary":{"used_percent":36.0,"window_minutes":300,"resets_at":1770671532},"secondary":{"used_percent":82.0,"window_minutes":10080,"resets_at":1771091103}}}}
{"timestamp":"2026-02-09T16:35:13Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":1900},"last_token_usage":{"total_tokens":420}},"rate_limits":{"primary":{"used_percent":40.0,"window_minutes":300,"resets_at":1770671532},"secondary":{"used_percent":84.0,"window_minutes":10080,"resets_at":1771091103}}}}"#,
        );

        assert_eq!(snapshot.session_total_tokens, Some(1900));
        assert_eq!(snapshot.last_turn_tokens, Some(420));
        assert_eq!(snapshot.session_delta_tokens, Some(400));
        assert!(snapshot.last_token_event_at.is_some());
        assert_eq!(
            snapshot
                .limits
                .primary
                .as_ref()
                .expect("primary")
                .remaining_percent,
            60.0
        );
        assert_eq!(
            snapshot
                .limits
                .secondary
                .as_ref()
                .expect("secondary")
                .remaining_percent,
            16.0
        );
    }

    #[test]
    fn fallback_delta_uses_last_turn_when_no_previous_total() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-09T16:33:13Z","type":"session_meta","payload":{"id":"delta-fallback","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:34:13Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":1280}},"rate_limits":{"primary":{"used_percent":14.0,"window_minutes":300,"resets_at":1770671532}}}}"#,
        );
        assert_eq!(snapshot.session_total_tokens, None);
        assert_eq!(snapshot.last_turn_tokens, Some(1280));
        assert_eq!(snapshot.session_delta_tokens, Some(1280));
    }

    #[test]
    fn session_meta_updates_started_at_when_newer() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"meta-refresh","timestamp":"2026-02-09T16:30:00Z","cwd":"C:\\repo\\app"}}
{"type":"session_meta","payload":{"id":"meta-refresh","timestamp":"2026-02-09T16:35:00Z","cwd":"C:\\repo\\app"}}"#,
        );
        let expected = parse_utc_timestamp("2026-02-09T16:35:00Z".to_string());
        assert_eq!(snapshot.started_at, expected);
    }

    #[test]
    fn session_meta_parses_desktop_originator() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"desktop","cwd":"C:\\repo\\app","originator":"Codex Desktop","source":"vscode"}}"#,
        );
        assert_eq!(snapshot.originator.as_deref(), Some("Codex Desktop"));
        assert_eq!(snapshot.source.as_deref(), Some("vscode"));
        assert!(snapshot.is_desktop_surface());
    }

    #[test]
    fn session_meta_ignores_non_string_source_values() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"subagent","cwd":"C:\\repo\\app","originator":"codex_vscode","source":{"subagent":{"thread_spawn":{"depth":1}}}}}"#,
        );
        assert_eq!(snapshot.originator.as_deref(), Some("codex_vscode"));
        assert_eq!(snapshot.source, None);
        assert!(!snapshot.is_desktop_surface());
    }

    #[test]
    fn session_id_change_resets_accumulator() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"session-a","timestamp":"2026-02-09T16:30:00Z","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:31:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":100},"last_token_usage":{"total_tokens":40}}}}
{"type":"session_meta","payload":{"id":"session-b","timestamp":"2026-02-09T16:40:00Z","cwd":"C:\\repo\\other"}}
{"timestamp":"2026-02-09T16:41:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":20},"last_token_usage":{"total_tokens":20}}}}"#,
        );
        assert_eq!(snapshot.session_id, "session-b");
        assert_eq!(snapshot.session_total_tokens, Some(20));
        assert_eq!(snapshot.last_turn_tokens, Some(20));
        assert_eq!(snapshot.session_delta_tokens, Some(20));
        let expected = parse_utc_timestamp("2026-02-09T16:40:00Z".to_string());
        assert_eq!(snapshot.started_at, expected);
    }

    #[test]
    fn parse_clamps_invalid_percent_values() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"clamp","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:34:13Z","type":"event_msg","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":133.0,"window_minutes":300,"resets_at":1770671532},"secondary":{"used_percent":-12.0,"window_minutes":10080,"resets_at":1771091103}}}}"#,
        );
        let primary = snapshot.limits.primary.expect("primary");
        let secondary = snapshot.limits.secondary.expect("secondary");
        assert_eq!(primary.used_percent, 100.0);
        assert_eq!(primary.remaining_percent, 0.0);
        assert_eq!(secondary.used_percent, 0.0);
        assert_eq!(secondary.remaining_percent, 100.0);
    }

    #[test]
    fn parses_running_activity_from_exec_command_cmd() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"exec-cmd","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-23T03:40:45Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"rg --files --hidden -g '*.rs'\"}","call_id":"call_exec"}}"#,
        );

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::RunningCommand);
        assert_eq!(activity.target.as_deref(), Some("rg --files"));
    }

    #[test]
    fn shell_command_accepts_cmd_argument_key() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"shell-cmd","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-23T03:40:45Z","type":"response_item","payload":{"type":"function_call","name":"shell_command","arguments":"{\"cmd\":\"cargo test --all\"}","call_id":"call_shell"}}"#,
        );

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::RunningCommand);
        assert_eq!(activity.target.as_deref(), Some("cargo test"));
    }

    #[test]
    fn parses_reasoning_effort_from_turn_context_root_field() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T03:40:38Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.4","effort":"xhigh"}}"#,
        );
        assert_eq!(snapshot.reasoning_effort, Some(ReasoningEffort::XHigh));
    }

    #[test]
    fn falls_back_to_nested_turn_context_reasoning_effort() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T03:40:38Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.4","collaboration_mode":{"mode":"default","settings":{"reasoning_effort":"high"}}}}"#,
        );
        assert_eq!(snapshot.reasoning_effort, Some(ReasoningEffort::High));
    }

    #[test]
    fn parses_context_window_from_token_event_info() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T03:40:38Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.3-codex"}}
{"timestamp":"2026-02-23T03:40:45Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":15224,"cached_input_tokens":6528,"output_tokens":450,"total_tokens":15674},"last_token_usage":{"input_tokens":15224,"cached_input_tokens":6528,"output_tokens":450,"total_tokens":15674},"model_context_window":258400}}}"#,
        );

        let context = snapshot.context_window.expect("context window");
        assert_eq!(context.window_tokens, 258_400);
        assert_eq!(context.used_tokens, 15_674);
        assert_eq!(context.remaining_tokens, 242_726);
        assert!((context.remaining_percent - 93.93).abs() < 0.05);
        assert_eq!(context.source, ContextWindowSource::Event);
    }

    #[test]
    fn falls_back_to_catalog_context_window_when_event_window_is_missing() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T03:40:38Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.2"}}
{"timestamp":"2026-02-23T03:40:45Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":120000},"last_token_usage":{"total_tokens":50000}}}}"#,
        );

        let context = snapshot.context_window.expect("context window");
        assert_eq!(context.window_tokens, 400_000);
        assert_eq!(context.used_tokens, 50_000);
        assert_eq!(context.remaining_tokens, 350_000);
        assert!((context.remaining_percent - 87.5).abs() < 0.01);
        assert_eq!(context.source, ContextWindowSource::Catalog);
    }

    #[test]
    fn falls_back_to_last_turn_tokens_when_session_total_is_missing_for_context_window() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T03:40:38Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.3-codex"}}
{"timestamp":"2026-02-23T03:40:45Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":33000},"model_context_window":258400}}}"#,
        );

        let context = snapshot.context_window.expect("context window");
        assert_eq!(context.window_tokens, 258_400);
        assert_eq!(context.used_tokens, 33_000);
        assert_eq!(context.remaining_tokens, 225_400);
        assert!((context.remaining_percent - 87.23).abs() < 0.05);
        assert_eq!(context.source, ContextWindowSource::Event);
    }

    #[test]
    fn context_window_usage_is_incremental_since_context_compaction() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T04:09:07Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.3-codex"}}
{"timestamp":"2026-02-23T04:09:08Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":11900000},"last_token_usage":{"total_tokens":22800},"model_context_window":258400}}}
{"timestamp":"2026-02-23T04:09:09Z","type":"event_msg","payload":{"type":"context_compacted"}}
{"timestamp":"2026-02-23T04:10:08Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":11934000},"last_token_usage":{"total_tokens":34000},"model_context_window":258400}}}"#,
        );

        let context = snapshot.context_window.expect("context window");
        assert_eq!(context.window_tokens, 258_400);
        assert_eq!(context.used_tokens, 34_000);
        assert_eq!(context.remaining_tokens, 224_400);
        assert!((context.remaining_percent - 86.84).abs() < 0.05);
        assert_eq!(context.source, ContextWindowSource::Event);
    }

    #[test]
    fn context_window_ignores_large_session_total_when_last_turn_is_missing() {
        let snapshot = parse_one(
            r#"{"timestamp":"2026-02-23T04:09:07Z","type":"turn_context","payload":{"cwd":"C:\\repo\\app","model":"gpt-5.3-codex"}}
{"timestamp":"2026-02-23T04:09:08Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":9300000},"model_context_window":258400}}}"#,
        );
        assert!(snapshot.context_window.is_none());
    }

    #[test]
    fn parses_thinking_activity_from_reasoning_event() {
        let ts = Utc::now().to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"thinking","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"agent_reasoning","text":"Inspecting files"}}}}"#
        );
        let snapshot = parse_one(&json);

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::Thinking);
        assert_eq!(activity.to_text(true), "Thinking");
    }

    #[test]
    fn parses_reading_activity_from_shell_command() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"read","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:41:13Z","type":"response_item","payload":{"type":"function_call","name":"shell_command","arguments":"{\"command\":\"Get-Content src/ui.rs\"}","call_id":"call_read"}}"#,
        );

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::ReadingFile);
        assert_eq!(activity.target.as_deref(), Some("ui.rs"));
        assert_eq!(activity.to_text(true), "Reading ui.rs");
    }

    #[test]
    fn parses_editing_activity_from_apply_patch() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"edit","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:42:13Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"call_patch","input":"*** Begin Patch\n*** Update File: src/session.rs\n@@\n*** End Patch\n"}}"#,
        );

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::EditingFile);
        assert_eq!(activity.target.as_deref(), Some("session.rs"));
        assert_eq!(activity.to_text(true), "Editing session.rs");
    }

    #[test]
    fn commentary_keeps_existing_working_activity() {
        let call_ts = Utc::now().to_rfc3339();
        let commentary_ts = (Utc::now() + ChronoDuration::seconds(1)).to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"commentary-keep","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{call_ts}","type":"response_item","payload":{{"type":"function_call","name":"shell_command","arguments":"{{\"command\":\"Get-Content src/ui.rs\"}}","call_id":"call_1"}}}}
{{"timestamp":"{call_ts}","type":"response_item","payload":{{"type":"function_call_output","call_id":"call_1"}}}}
{{"timestamp":"{commentary_ts}","type":"response_item","payload":{{"type":"message","role":"assistant","phase":"commentary","content":[{{"type":"output_text","text":"working..."}}]}}}}"#
        );
        let snapshot = parse_one(&json);
        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::ReadingFile);
        assert_eq!(activity.target.as_deref(), Some("ui.rs"));
    }

    #[test]
    fn commentary_reactivates_waiting_to_thinking() {
        let waiting_ts = Utc::now().to_rfc3339();
        let commentary_ts = (Utc::now() + ChronoDuration::seconds(1)).to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"commentary-reactivate","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{waiting_ts}","type":"response_item","payload":{{"type":"message","role":"assistant","phase":"final_answer","content":[{{"type":"output_text","text":"done"}}]}}}}
{{"timestamp":"{commentary_ts}","type":"response_item","payload":{{"type":"message","role":"assistant","phase":"commentary","content":[{{"type":"output_text","text":"still working"}}]}}}}"#
        );
        let snapshot = parse_one(&json);
        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::Thinking);
    }

    #[test]
    fn final_answer_message_marks_waiting_input() {
        let ts = Utc::now().to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"final-answer","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"message","role":"assistant","phase":"final_answer","content":[{{"type":"output_text","text":"done"}}]}}}}"#
        );
        let snapshot = parse_one(&json);
        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::WaitingInput);
    }

    #[test]
    fn web_search_call_counts_as_running_activity() {
        let ts = Utc::now().to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"search","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"web_search_call","status":"completed","action":{{"type":"search","query":"rust serde flatten examples"}}}}}}"#
        );
        let snapshot = parse_one(&json);
        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::RunningCommand);
        assert!(
            activity
                .target
                .as_deref()
                .is_some_and(|target| target.contains("web search"))
        );
    }

    #[test]
    fn does_not_mark_idle_immediately_after_tool_output() {
        let now = Utc::now() - ChronoDuration::seconds(10);
        let ts = now.to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"active","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"function_call","name":"shell_command","arguments":"{{\"command\":\"Get-Content src/ui.rs\"}}","call_id":"call_1"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"function_call_output","call_id":"call_1"}}}}"#
        );
        let snapshot = parse_one(&json);

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::ReadingFile);
        assert_eq!(activity.pending_calls, 0);
    }

    #[test]
    fn recent_tool_output_signal_prevents_idle_transition() {
        let old = Utc::now() - ChronoDuration::seconds(120);
        let recent = Utc::now() - ChronoDuration::seconds(5);
        let old_ts = old.to_rfc3339();
        let recent_ts = recent.to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"active","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{old_ts}","type":"response_item","payload":{{"type":"function_call","name":"shell_command","arguments":"{{\"command\":\"Get-Content src/ui.rs\"}}","call_id":"call_1"}}}}
{{"timestamp":"{recent_ts}","type":"response_item","payload":{{"type":"function_call_output","call_id":"call_1"}}}}"#
        );
        let snapshot = parse_one(&json);

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::ReadingFile);
        assert_eq!(activity.pending_calls, 0);
    }

    #[test]
    fn marks_idle_after_debounce_without_new_events() {
        let old = Utc::now() - ChronoDuration::seconds(120);
        let ts = old.to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"idle","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"function_call","name":"shell_command","arguments":"{{\"command\":\"Get-Content src/ui.rs\"}}","call_id":"call_1"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"function_call_output","call_id":"call_1"}}}}"#
        );
        let snapshot = parse_one(&json);

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::Idle);
        assert_eq!(activity.target, None);
    }

    #[test]
    fn latest_limits_source_prefers_most_recent_token_event() {
        let now = SystemTime::now();
        let older = CodexSessionSnapshot {
            session_id: "older".to_string(),
            cwd: PathBuf::from("."),
            project_name: "older".to_string(),
            git_branch: None,
            originator: None,
            source: None,
            model: None,
            reasoning_effort: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: None,
            last_turn_tokens: None,
            session_delta_tokens: None,
            input_tokens_total: 0,
            cached_input_tokens_total: 0,
            output_tokens_total: 0,
            last_input_tokens: None,
            last_cached_input_tokens: None,
            last_output_tokens: None,
            total_cost_usd: 0.0,
            cost_breakdown: TokenCostBreakdown::default(),
            pricing_source: PricingSource::Fallback,
            context_window: None,
            limits: RateLimits {
                primary: Some(UsageWindow {
                    used_percent: 50.0,
                    remaining_percent: 50.0,
                    window_minutes: 300,
                    resets_at: None,
                }),
                secondary: None,
            },
            rate_limit_envelopes: vec![RateLimitEnvelope {
                limit_id: Some("codex".to_string()),
                limit_name: None,
                plan_type: None,
                observed_at: Utc.timestamp_opt(1000, 0).single(),
                scope: RateLimitScope::GlobalCodex,
                limits: RateLimits {
                    primary: Some(UsageWindow {
                        used_percent: 50.0,
                        remaining_percent: 50.0,
                        window_minutes: 300,
                        resets_at: None,
                    }),
                    secondary: None,
                },
            }],
            activity: None,
            started_at: None,
            last_token_event_at: Utc.timestamp_opt(1000, 0).single(),
            last_activity: now,
            source_file: PathBuf::from("older.jsonl"),
        };
        let newer = CodexSessionSnapshot {
            session_id: "newer".to_string(),
            cwd: PathBuf::from("."),
            project_name: "newer".to_string(),
            git_branch: None,
            originator: None,
            source: None,
            model: None,
            reasoning_effort: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: None,
            last_turn_tokens: None,
            session_delta_tokens: None,
            input_tokens_total: 0,
            cached_input_tokens_total: 0,
            output_tokens_total: 0,
            last_input_tokens: None,
            last_cached_input_tokens: None,
            last_output_tokens: None,
            total_cost_usd: 0.0,
            cost_breakdown: TokenCostBreakdown::default(),
            pricing_source: PricingSource::Fallback,
            context_window: None,
            limits: RateLimits {
                primary: Some(UsageWindow {
                    used_percent: 20.0,
                    remaining_percent: 80.0,
                    window_minutes: 300,
                    resets_at: None,
                }),
                secondary: None,
            },
            rate_limit_envelopes: vec![RateLimitEnvelope {
                limit_id: Some("codex".to_string()),
                limit_name: None,
                plan_type: None,
                observed_at: Utc.timestamp_opt(2000, 0).single(),
                scope: RateLimitScope::GlobalCodex,
                limits: RateLimits {
                    primary: Some(UsageWindow {
                        used_percent: 20.0,
                        remaining_percent: 80.0,
                        window_minutes: 300,
                        resets_at: None,
                    }),
                    secondary: None,
                },
            }],
            activity: None,
            started_at: None,
            last_token_event_at: Utc.timestamp_opt(2000, 0).single(),
            last_activity: now,
            source_file: PathBuf::from("newer.jsonl"),
        };

        let sessions = vec![older, newer];
        let source = latest_limits_source(&sessions).expect("limits source");
        assert_eq!(source.source_session_id, "newer");
    }

    #[test]
    fn sticky_policy_keeps_working_session_within_window() {
        let now = SystemTime::now();
        let recency = now
            .checked_sub(Duration::from_secs(8 * 60))
            .expect("recency");
        let stale_cutoff = now.checked_sub(Duration::from_secs(90)).expect("stale");
        let sticky_cutoff = now
            .checked_sub(Duration::from_secs(60 * 60))
            .expect("sticky");
        let snapshot = policy_snapshot(Some(SessionActivityKind::Thinking));

        assert!(should_include_session(
            &snapshot,
            recency,
            stale_cutoff,
            sticky_cutoff
        ));
    }

    #[test]
    fn sticky_policy_excludes_idle_session_beyond_stale_cutoff() {
        let now = SystemTime::now();
        let recency = now
            .checked_sub(Duration::from_secs(8 * 60))
            .expect("recency");
        let stale_cutoff = now.checked_sub(Duration::from_secs(90)).expect("stale");
        let sticky_cutoff = now
            .checked_sub(Duration::from_secs(60 * 60))
            .expect("sticky");
        let snapshot = policy_snapshot(Some(SessionActivityKind::Idle));

        assert!(!should_include_session(
            &snapshot,
            recency,
            stale_cutoff,
            sticky_cutoff
        ));
    }

    #[test]
    fn sticky_policy_keeps_waiting_session_within_window() {
        let now = SystemTime::now();
        let recency = now
            .checked_sub(Duration::from_secs(8 * 60))
            .expect("recency");
        let stale_cutoff = now.checked_sub(Duration::from_secs(90)).expect("stale");
        let sticky_cutoff = now
            .checked_sub(Duration::from_secs(60 * 60))
            .expect("sticky");
        let snapshot = policy_snapshot(Some(SessionActivityKind::WaitingInput));

        assert!(should_include_session(
            &snapshot,
            recency,
            stale_cutoff,
            sticky_cutoff
        ));
    }

    #[test]
    fn sticky_policy_excludes_session_outside_sticky_window() {
        let now = SystemTime::now();
        let recency = now
            .checked_sub(Duration::from_secs(2 * 60 * 60))
            .expect("recency");
        let stale_cutoff = now.checked_sub(Duration::from_secs(90)).expect("stale");
        let sticky_cutoff = now
            .checked_sub(Duration::from_secs(60 * 60))
            .expect("sticky");
        let snapshot = policy_snapshot(Some(SessionActivityKind::Thinking));

        assert!(!should_include_session(
            &snapshot,
            recency,
            stale_cutoff,
            sticky_cutoff
        ));
    }

    #[test]
    fn strict_stale_cutoff_includes_recent_session_without_activity() {
        let now = SystemTime::now();
        let recency = now.checked_sub(Duration::from_secs(30)).expect("recency");
        let stale_cutoff = now.checked_sub(Duration::from_secs(90)).expect("stale");
        let sticky_cutoff = now
            .checked_sub(Duration::from_secs(60 * 60))
            .expect("sticky");
        let snapshot = policy_snapshot(None);

        assert!(should_include_session(
            &snapshot,
            recency,
            stale_cutoff,
            sticky_cutoff
        ));
    }

    #[test]
    fn session_recency_uses_newest_activity_signal() {
        let file_modified = SystemTime::now()
            .checked_sub(Duration::from_secs(2 * 60 * 60))
            .expect("file_modified");
        let observed_ts = Utc::now() - ChronoDuration::minutes(30);
        let active_ts = Utc::now() - ChronoDuration::minutes(20);
        let effective_ts = Utc::now() - ChronoDuration::minutes(10);
        let token_ts = Utc::now() - ChronoDuration::minutes(15);
        let mut snapshot = policy_snapshot(Some(SessionActivityKind::Thinking));
        snapshot.last_token_event_at = Some(token_ts);
        if let Some(activity) = snapshot.activity.as_mut() {
            activity.observed_at = Some(observed_ts);
            activity.last_active_at = Some(active_ts);
            activity.last_effective_signal_at = Some(effective_ts);
        }

        let recency = session_recency(&snapshot, file_modified);
        let expected = datetime_to_system_time(effective_ts).expect("expected");
        assert_eq!(recency, expected);
    }

    #[test]
    fn session_ranking_prioritizes_recency_before_pending_and_activity() {
        let now = SystemTime::now();

        let mut pending_old = policy_snapshot(Some(SessionActivityKind::RunningCommand));
        pending_old.session_id = "pending_old".to_string();
        pending_old.last_activity = now
            .checked_sub(Duration::from_secs(600))
            .expect("pending recency");
        if let Some(activity) = pending_old.activity.as_mut() {
            activity.pending_calls = 2;
        }

        let mut working_mid = policy_snapshot(Some(SessionActivityKind::Thinking));
        working_mid.session_id = "working_mid".to_string();
        working_mid.last_activity = now
            .checked_sub(Duration::from_secs(120))
            .expect("working recency");

        let mut waiting_recent = policy_snapshot(Some(SessionActivityKind::WaitingInput));
        waiting_recent.session_id = "waiting_recent".to_string();
        waiting_recent.last_activity = now
            .checked_sub(Duration::from_secs(20))
            .expect("waiting recency");

        let mut idle_newest = policy_snapshot(Some(SessionActivityKind::Idle));
        idle_newest.session_id = "idle_newest".to_string();
        idle_newest.last_activity = now;

        let mut sessions = [pending_old, working_mid, waiting_recent, idle_newest];
        sessions.sort_by_key(|session| Reverse(session_rank_key(session)));

        assert_eq!(sessions[0].session_id, "idle_newest");
        assert_eq!(sessions[1].session_id, "waiting_recent");
        assert_eq!(sessions[2].session_id, "working_mid");
        assert_eq!(sessions[3].session_id, "pending_old");
    }

    #[test]
    fn preferred_active_session_prefers_most_recent_signal() {
        let now = SystemTime::now();

        let mut older_pending = policy_snapshot(Some(SessionActivityKind::RunningCommand));
        older_pending.session_id = "older_pending".to_string();
        older_pending.last_activity = now.checked_sub(Duration::from_secs(120)).expect("older");
        if let Some(activity) = older_pending.activity.as_mut() {
            activity.pending_calls = 4;
        }

        let mut newest_waiting = policy_snapshot(Some(SessionActivityKind::WaitingInput));
        newest_waiting.session_id = "newest_waiting".to_string();
        newest_waiting.last_activity = now;

        let sessions = vec![older_pending, newest_waiting];
        let active = preferred_active_session(&sessions).expect("active");
        assert_eq!(active.session_id, "newest_waiting");
    }

    #[test]
    fn ranking_tiebreaks_by_pending_then_activity_when_recency_equal() {
        let now = SystemTime::now();

        let mut pending = policy_snapshot(Some(SessionActivityKind::WaitingInput));
        pending.session_id = "pending".to_string();
        pending.last_activity = now;
        if let Some(activity) = pending.activity.as_mut() {
            activity.pending_calls = 2;
        }

        let mut working = policy_snapshot(Some(SessionActivityKind::Thinking));
        working.session_id = "working".to_string();
        working.last_activity = now;

        let mut waiting = policy_snapshot(Some(SessionActivityKind::WaitingInput));
        waiting.session_id = "waiting".to_string();
        waiting.last_activity = now;

        let mut idle = policy_snapshot(Some(SessionActivityKind::Idle));
        idle.session_id = "idle".to_string();
        idle.last_activity = now;

        let mut sessions = [idle, waiting, working, pending];
        sessions.sort_by_key(|session| Reverse(session_rank_key(session)));

        assert_eq!(sessions[0].session_id, "pending");
        assert_eq!(sessions[1].session_id, "working");
        assert_eq!(sessions[2].session_id, "waiting");
        assert_eq!(sessions[3].session_id, "idle");
    }

    #[test]
    fn idle_transition_does_not_refresh_recency_to_now() {
        let old = Utc::now() - ChronoDuration::seconds(120);
        let ts = old.to_rfc3339();
        let json = format!(
            r#"{{"type":"session_meta","payload":{{"id":"idle-recency","cwd":"C:\\repo\\app"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"function_call","name":"shell_command","arguments":"{{\"command\":\"Get-Content src/ui.rs\"}}","call_id":"call_1"}}}}
{{"timestamp":"{ts}","type":"response_item","payload":{{"type":"function_call_output","call_id":"call_1"}}}}"#
        );
        let snapshot = parse_one(&json);
        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::Idle);
        let observed = activity.observed_at.expect("observed_at");
        let drift = observed.signed_duration_since(old).num_milliseconds().abs();
        assert!(drift <= 1000, "observed_at drifted by {drift}ms");
    }

    #[test]
    fn parser_keeps_partial_json_until_completed() {
        let tmp = TempDir::new().expect("temp dir");
        let file_path = tmp.path().join("partial.jsonl");
        let full_event_line = r#"{"timestamp":"2026-02-09T16:35:13Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":210},"last_token_usage":{"total_tokens":80}}}}"#;
        let split_at = full_event_line.len().saturating_sub(8);
        let (event_head, _event_tail) = full_event_line.split_at(split_at);
        std::fs::write(
            &file_path,
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"partial\",\"cwd\":\"C:\\\\repo\\\\app\"}}}}\n{}",
                event_head
            ),
        )
        .expect("write");

        let file = File::open(&file_path).expect("open");
        let mut reader = BufReader::new(file);
        let mut accumulator = SessionAccumulator::default();
        let mut partial_line_buffer = String::new();
        parse_new_lines(&mut reader, &mut accumulator, &mut partial_line_buffer).expect("parse");

        assert!(
            !partial_line_buffer.is_empty(),
            "partial json line should be retained"
        );
        assert_eq!(accumulator.session_total_tokens, None);
    }

    #[test]
    fn cached_parser_does_not_drop_split_line() {
        let tmp = TempDir::new().expect("temp dir");
        let file_path = tmp.path().join("split.jsonl");
        let full_event_line = r#"{"timestamp":"2026-02-09T16:35:13Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":210},"last_token_usage":{"total_tokens":80}}}}"#;
        let split_at = full_event_line.len().saturating_sub(8);
        let (event_head, event_tail) = full_event_line.split_at(split_at);
        std::fs::write(
            &file_path,
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"split\",\"cwd\":\"C:\\\\repo\\\\app\"}}}}\n{}",
                event_head
            ),
        )
        .expect("write initial");

        let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
        let mut parse_cache = SessionParseCache::default();
        let meta1 = std::fs::metadata(&file_path).expect("meta1");
        let modified1 = meta1.modified().expect("modified1");

        let snapshot1 = parse_session_file_cached(
            &file_path,
            &meta1,
            modified1,
            &mut git_cache,
            &mut parse_cache,
            &PricingConfig::default(),
        )
        .expect("parse1")
        .expect("snapshot1");
        assert_eq!(snapshot1.session_total_tokens, None);
        assert!(
            !parse_cache
                .entries
                .get(&file_path)
                .expect("cache1")
                .partial_line_buffer
                .is_empty()
        );

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&file_path)
            .expect("append open");
        use std::io::Write as _;
        writeln!(file, "{}", event_tail).expect("append tail");
        file.flush().expect("flush tail");
        drop(file);

        let meta2 = std::fs::metadata(&file_path).expect("meta2");
        let modified2 = meta2.modified().expect("modified2");
        let snapshot2 = parse_session_file_cached(
            &file_path,
            &meta2,
            modified2,
            &mut git_cache,
            &mut parse_cache,
            &PricingConfig::default(),
        )
        .expect("parse2")
        .expect("snapshot2");
        let cache2 = parse_cache.entries.get(&file_path).expect("cache2");

        assert_eq!(snapshot2.session_total_tokens, Some(210));
        assert_eq!(snapshot2.last_turn_tokens, Some(80));
        assert!(cache2.partial_line_buffer.is_empty());
    }

    #[test]
    fn cached_parser_advances_cursor_with_appended_lines() {
        let tmp = TempDir::new().expect("temp dir");
        let file_path = tmp.path().join("session.jsonl");
        std::fs::write(
            &file_path,
            r#"{"type":"session_meta","payload":{"id":"cached","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:34:13Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":100},"last_token_usage":{"total_tokens":40}}}}"#,
        )
        .expect("write initial");

        let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
        let mut parse_cache = SessionParseCache::default();
        let meta1 = std::fs::metadata(&file_path).expect("metadata1");
        let modified1 = meta1.modified().expect("modified1");

        let snapshot1 = parse_session_file_cached(
            &file_path,
            &meta1,
            modified1,
            &mut git_cache,
            &mut parse_cache,
            &PricingConfig::default(),
        )
        .expect("parse1")
        .expect("snapshot1");
        let first_cursor = parse_cache
            .entries
            .get(&file_path)
            .expect("cache entry")
            .cursor;

        assert_eq!(snapshot1.session_total_tokens, Some(100));
        assert_eq!(snapshot1.last_turn_tokens, Some(40));

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&file_path)
            .expect("open append");
        use std::io::Write as _;
        writeln!(
            file,
            r#"{{"timestamp":"2026-02-09T16:35:13Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"total_tokens":160}},"last_token_usage":{{"total_tokens":60}}}}}}}}"#
        )
        .expect("append");

        let meta2 = std::fs::metadata(&file_path).expect("metadata2");
        let modified2 = meta2.modified().expect("modified2");
        let snapshot2 = parse_session_file_cached(
            &file_path,
            &meta2,
            modified2,
            &mut git_cache,
            &mut parse_cache,
            &PricingConfig::default(),
        )
        .expect("parse2")
        .expect("snapshot2");
        let second_cursor = parse_cache
            .entries
            .get(&file_path)
            .expect("cache entry")
            .cursor;

        assert!(second_cursor > first_cursor);
        assert_eq!(snapshot2.session_total_tokens, Some(160));
        assert_eq!(snapshot2.last_turn_tokens, Some(60));
        assert_eq!(snapshot2.session_delta_tokens, Some(60));
    }
}
