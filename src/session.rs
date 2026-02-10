use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageWindow {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_minutes: u64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RateLimits {
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexSessionSnapshot {
    pub session_id: String,
    pub cwd: PathBuf,
    pub project_name: String,
    pub git_branch: Option<String>,
    pub model: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_policy: Option<String>,
    pub session_total_tokens: Option<u64>,
    pub last_turn_tokens: Option<u64>,
    pub session_delta_tokens: Option<u64>,
    pub limits: RateLimits,
    pub activity: Option<SessionActivitySnapshot>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_token_event_at: Option<DateTime<Utc>>,
    pub last_activity: SystemTime,
    pub source_file: PathBuf,
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

#[derive(Debug)]
struct CachedSessionEntry {
    cursor: u64,
    file_len: u64,
    modified: SystemTime,
    accumulator: SessionAccumulator,
    snapshot: Option<CodexSessionSnapshot>,
}

impl CachedSessionEntry {
    fn new(modified: SystemTime) -> Self {
        Self {
            cursor: 0,
            file_len: 0,
            modified,
            accumulator: SessionAccumulator::default(),
            snapshot: None,
        }
    }

    fn reset(&mut self, modified: SystemTime) {
        self.cursor = 0;
        self.file_len = 0;
        self.modified = modified;
        self.accumulator = SessionAccumulator::default();
        self.snapshot = None;
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
) -> Result<Vec<CodexSessionSnapshot>> {
    if !sessions_root.exists() {
        parse_cache.entries.clear();
        return Ok(Vec::new());
    }

    let now = SystemTime::now();
    let stale_cutoff = now
        .checked_sub(stale_threshold)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let sticky_cutoff = now
        .checked_sub(active_sticky_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut sessions = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();

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

        let metadata = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Some(mut snapshot) =
            parse_session_file_cached(path, &metadata, modified, git_cache, parse_cache)?
        {
            let recency = session_recency(&snapshot, modified);
            snapshot.last_activity = recency;
            if should_include_session(&snapshot, recency, stale_cutoff, sticky_cutoff) {
                sessions.push(snapshot);
            }
        }
    }

    parse_cache
        .entries
        .retain(|path, _| seen_paths.contains(path));
    sessions.sort_by_key(|session| Reverse(session_rank_key(session)));
    Ok(sessions)
}

pub fn latest_limits_source(sessions: &[CodexSessionSnapshot]) -> Option<&CodexSessionSnapshot> {
    sessions
        .iter()
        .filter(|session| limits_present(&session.limits))
        .max_by_key(|session| {
            let observed = session
                .last_token_event_at
                .map(|ts| ts.timestamp())
                .unwrap_or(i64::MIN);
            let activity = session
                .last_activity
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            (observed, activity)
        })
}

pub fn preferred_active_session(
    sessions: &[CodexSessionSnapshot],
) -> Option<&CodexSessionSnapshot> {
    sessions
        .iter()
        .max_by_key(|session| session_rank_key(session))
}

pub fn limits_present(limits: &RateLimits) -> bool {
    limits.primary.is_some() || limits.secondary.is_some()
}

fn should_include_session(
    snapshot: &CodexSessionSnapshot,
    recency: SystemTime,
    stale_cutoff: SystemTime,
    sticky_cutoff: SystemTime,
) -> bool {
    if recency >= stale_cutoff {
        return true;
    }
    if recency < sticky_cutoff {
        return false;
    }
    snapshot
        .activity
        .as_ref()
        .is_some_and(session_activity_is_sticky_active)
}

fn session_activity_is_sticky_active(activity: &SessionActivitySnapshot) -> bool {
    !matches!(activity.kind, SessionActivityKind::Idle)
}

fn session_rank_key(snapshot: &CodexSessionSnapshot) -> (usize, u8, SystemTime) {
    let (pending_calls, non_idle) = snapshot
        .activity
        .as_ref()
        .map_or((0usize, 0u8), |activity| {
            let non_idle = if matches!(activity.kind, SessionActivityKind::Idle) {
                0
            } else {
                1
            };
            (activity.pending_calls, non_idle)
        });
    (pending_calls, non_idle, snapshot.last_activity)
}

fn session_recency(snapshot: &CodexSessionSnapshot, file_modified: SystemTime) -> SystemTime {
    let mut newest = file_modified;

    if let Some(ts) = snapshot
        .last_token_event_at
        .and_then(datetime_to_system_time)
        && ts > newest
    {
        newest = ts;
    }

    if let Some(activity) = &snapshot.activity {
        for candidate in [activity.last_active_at, activity.observed_at] {
            if let Some(ts) = candidate.and_then(datetime_to_system_time)
                && ts > newest
            {
                newest = ts;
            }
        }
    }

    newest
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

#[derive(Debug, Default)]
struct SessionAccumulator {
    session_id: Option<String>,
    cwd: Option<PathBuf>,
    started_at: Option<DateTime<Utc>>,
    model: Option<String>,
    approval_policy: Option<String>,
    sandbox_policy: Option<String>,
    session_total_tokens: Option<u64>,
    previous_session_total_tokens: Option<u64>,
    last_turn_tokens: Option<u64>,
    limits: RateLimits,
    last_token_event_at: Option<DateTime<Utc>>,
    activity_tracker: ActivityTracker,
}

#[derive(Debug, Clone)]
struct PendingActivity {
    kind: SessionActivityKind,
    target: Option<String>,
}

const IDLE_DEBOUNCE_SECS: i64 = 45;

#[derive(Debug, Default)]
struct ActivityTracker {
    snapshot: Option<SessionActivitySnapshot>,
    pending_calls: HashMap<String, PendingActivity>,
    last_event_at: Option<DateTime<Utc>>,
    last_effective_signal_at: Option<DateTime<Utc>>,
}

impl ActivityTracker {
    fn observe_timestamp(&mut self, observed_at: Option<DateTime<Utc>>) {
        if let Some(ts) = observed_at {
            self.last_event_at = max_datetime(self.last_event_at, Some(ts));
        }
    }

    fn observe_effective_signal(&mut self, observed_at: Option<DateTime<Utc>>) {
        self.observe_timestamp(observed_at);
        self.last_effective_signal_at = max_datetime(self.last_effective_signal_at, observed_at);
        if let Some(snapshot) = self.snapshot.as_mut() {
            snapshot.last_effective_signal_at =
                max_datetime(snapshot.last_effective_signal_at, observed_at);
        }
    }

    fn mark_activity(
        &mut self,
        kind: SessionActivityKind,
        target: Option<String>,
        observed_at: Option<DateTime<Utc>>,
    ) {
        self.observe_effective_signal(observed_at);
        let previous_active = self.snapshot.as_ref().and_then(|item| item.last_active_at);
        let last_active_at = max_datetime(previous_active, observed_at);
        let idle_candidate_at = if self.pending_calls.is_empty()
            && !matches!(
                kind,
                SessionActivityKind::Idle | SessionActivityKind::WaitingInput
            ) {
            last_active_at
        } else {
            None
        };

        self.snapshot = Some(SessionActivitySnapshot {
            kind,
            target,
            observed_at,
            last_active_at,
            last_effective_signal_at: self.last_effective_signal_at,
            idle_candidate_at,
            pending_calls: self.pending_calls.len(),
        });
    }

    fn start_call(
        &mut self,
        call_id: Option<String>,
        pending: PendingActivity,
        observed_at: Option<DateTime<Utc>>,
    ) {
        if let Some(call_id) = call_id {
            self.pending_calls.insert(call_id, pending.clone());
        }
        self.mark_activity(pending.kind, pending.target, observed_at);
    }

    fn complete_call(&mut self, call_id: Option<String>, observed_at: Option<DateTime<Utc>>) {
        self.observe_effective_signal(observed_at);
        if let Some(call_id) = call_id {
            self.pending_calls.remove(&call_id);
        }

        if let Some(snapshot) = self.snapshot.as_mut() {
            snapshot.pending_calls = self.pending_calls.len();
            if snapshot.pending_calls == 0
                && !matches!(
                    snapshot.kind,
                    SessionActivityKind::Idle | SessionActivityKind::WaitingInput
                )
            {
                snapshot.idle_candidate_at = snapshot.last_active_at.or(observed_at);
            }
        }
    }

    fn finalize(&self, now: DateTime<Utc>) -> Option<SessionActivitySnapshot> {
        let mut snapshot = self.snapshot.clone()?;
        snapshot.pending_calls = self.pending_calls.len();

        if snapshot.last_active_at.is_none() {
            snapshot.last_active_at = snapshot
                .observed_at
                .or(snapshot.last_effective_signal_at)
                .or(self.last_effective_signal_at)
                .or(self.last_event_at);
        }

        if snapshot.pending_calls > 0 {
            snapshot.idle_candidate_at = None;
            return Some(snapshot);
        }

        if matches!(
            snapshot.kind,
            SessionActivityKind::Idle | SessionActivityKind::WaitingInput
        ) {
            if matches!(snapshot.kind, SessionActivityKind::Idle) {
                snapshot.target = None;
            }
            return Some(snapshot);
        }

        let idle_candidate = snapshot
            .idle_candidate_at
            .or(snapshot.last_active_at)
            .or(snapshot.observed_at)
            .or(self.last_event_at);
        let effective_signal = snapshot
            .last_effective_signal_at
            .or(self.last_effective_signal_at)
            .or(snapshot.last_active_at)
            .or(snapshot.observed_at)
            .or(self.last_event_at);
        let idle_reference = max_datetime(idle_candidate, effective_signal);
        snapshot.idle_candidate_at = idle_reference;
        snapshot.last_effective_signal_at = effective_signal;

        if let Some(idle_reference) = idle_reference
            && now.signed_duration_since(idle_reference).num_seconds() >= IDLE_DEBOUNCE_SECS
        {
            snapshot.kind = SessionActivityKind::Idle;
            snapshot.target = None;
            snapshot.observed_at = Some(now);
        }

        Some(snapshot)
    }
}

impl SessionAccumulator {
    fn apply_event(&mut self, parsed: &Value) {
        let typ = str_at(parsed, &["type"]);
        let event_timestamp = str_at(parsed, &["timestamp"]).and_then(parse_utc_timestamp);
        let payload = parsed.get("payload").unwrap_or(&Value::Null);
        self.activity_tracker.observe_timestamp(event_timestamp);

        match typ.as_deref() {
            Some("session_meta") => {
                self.session_id = self.session_id.take().or_else(|| str_at(payload, &["id"]));
                if self.started_at.is_none() {
                    self.started_at = str_at(payload, &["timestamp"]).and_then(parse_utc_timestamp);
                }
                if self.cwd.is_none() {
                    self.cwd = str_at(payload, &["cwd"]).map(PathBuf::from);
                }
            }
            Some("turn_context") => {
                if self.cwd.is_none() {
                    self.cwd = str_at(payload, &["cwd"]).map(PathBuf::from);
                }
                if self.model.is_none() {
                    self.model = str_at(payload, &["model"]);
                }
                if self.approval_policy.is_none() {
                    self.approval_policy = str_at(payload, &["approval_policy"]);
                }
                if self.sandbox_policy.is_none() {
                    self.sandbox_policy = str_at(payload, &["sandbox_policy", "type"])
                        .or_else(|| str_at(payload, &["sandbox_policy"]));
                }
            }
            Some("event_msg") => match str_at(payload, &["type"]).as_deref() {
                Some("token_count") => {
                    if let Some(total_tokens) = total_tokens_from_info(payload) {
                        self.previous_session_total_tokens = self.session_total_tokens;
                        self.session_total_tokens = Some(total_tokens);
                    }
                    if let Some(last_tokens) = last_tokens_from_info(payload) {
                        self.last_turn_tokens = Some(last_tokens);
                    }

                    let parsed_limits = parse_rate_limits(payload.get("rate_limits"));
                    if limits_present(&parsed_limits) {
                        self.limits = parsed_limits;
                    }

                    if event_timestamp.is_some() {
                        self.last_token_event_at = event_timestamp;
                    }
                }
                Some("agent_reasoning") => {
                    self.activity_tracker.mark_activity(
                        SessionActivityKind::Thinking,
                        None,
                        event_timestamp,
                    );
                }
                Some("agent_message") => {
                    self.activity_tracker.mark_activity(
                        SessionActivityKind::WaitingInput,
                        None,
                        event_timestamp,
                    );
                }
                _ => {}
            },
            Some("response_item") => match str_at(payload, &["type"]).as_deref() {
                Some("reasoning") => {
                    self.activity_tracker.mark_activity(
                        SessionActivityKind::Thinking,
                        None,
                        event_timestamp,
                    );
                }
                Some("function_call") => {
                    let name = str_at(payload, &["name"]).unwrap_or_default();
                    let arguments = str_at(payload, &["arguments"]).unwrap_or_default();
                    let classified = classify_function_call(&name, &arguments);
                    self.activity_tracker.start_call(
                        str_at(payload, &["call_id"]),
                        classified,
                        event_timestamp,
                    );
                }
                Some("custom_tool_call") => {
                    let name = str_at(payload, &["name"]).unwrap_or_default();
                    let input = str_at(payload, &["input"]).unwrap_or_default();
                    let classified = classify_custom_tool_call(&name, &input);
                    self.activity_tracker.start_call(
                        str_at(payload, &["call_id"]),
                        classified,
                        event_timestamp,
                    );
                }
                Some("function_call_output") | Some("custom_tool_call_output") => {
                    self.activity_tracker
                        .complete_call(str_at(payload, &["call_id"]), event_timestamp);
                }
                Some("message") => {
                    if str_at(payload, &["role"]).as_deref() == Some("assistant") {
                        self.activity_tracker.mark_activity(
                            SessionActivityKind::WaitingInput,
                            None,
                            event_timestamp,
                        );
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn build_snapshot(
        &self,
        jsonl_path: &Path,
        last_activity: SystemTime,
        git_cache: &mut GitBranchCache,
    ) -> Option<CodexSessionSnapshot> {
        let activity = self.activity_tracker.finalize(Utc::now());
        let session_delta_tokens = compute_session_delta(
            self.session_total_tokens,
            self.previous_session_total_tokens,
            self.last_turn_tokens,
        );

        if self.session_id.is_none()
            && self.cwd.is_none()
            && self.model.is_none()
            && self.session_total_tokens.is_none()
            && self.last_turn_tokens.is_none()
            && session_delta_tokens.is_none()
            && !limits_present(&self.limits)
            && activity.is_none()
        {
            return None;
        }

        let cwd = self.cwd.clone().unwrap_or_else(|| PathBuf::from("."));
        let project_name = cwd
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "unknown-project".to_string());
        let git_branch = git_cache.get(&cwd);
        let fallback_id = jsonl_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown-session")
            .to_string();

        Some(CodexSessionSnapshot {
            session_id: self.session_id.clone().unwrap_or(fallback_id),
            cwd,
            project_name,
            git_branch,
            model: self.model.clone(),
            approval_policy: self.approval_policy.clone(),
            sandbox_policy: self.sandbox_policy.clone(),
            session_total_tokens: self.session_total_tokens,
            last_turn_tokens: self.last_turn_tokens,
            session_delta_tokens,
            limits: self.limits.clone(),
            activity,
            started_at: self.started_at,
            last_token_event_at: self.last_token_event_at,
            last_activity,
            source_file: jsonl_path.to_path_buf(),
        })
    }
}

fn classify_shell_command(arguments: &str) -> PendingActivity {
    let command = shell_command_text(arguments);
    if command.trim().is_empty() {
        return PendingActivity {
            kind: SessionActivityKind::RunningCommand,
            target: None,
        };
    }

    if let Some(path) = extract_read_target(&command) {
        return PendingActivity {
            kind: SessionActivityKind::ReadingFile,
            target: Some(path),
        };
    }

    PendingActivity {
        kind: SessionActivityKind::RunningCommand,
        target: Some(truncate_activity_target(command, 72)),
    }
}

fn classify_function_call(name: &str, arguments: &str) -> PendingActivity {
    match name {
        "shell_command" => classify_shell_command(arguments),
        "view_image" => PendingActivity {
            kind: SessionActivityKind::ReadingFile,
            target: extract_view_image_target(arguments),
        },
        "request_user_input" => PendingActivity {
            kind: SessionActivityKind::WaitingInput,
            target: None,
        },
        _ => PendingActivity {
            kind: SessionActivityKind::RunningCommand,
            target: None,
        },
    }
}

fn classify_custom_tool_call(name: &str, input: &str) -> PendingActivity {
    match name {
        "apply_patch" => PendingActivity {
            kind: SessionActivityKind::EditingFile,
            target: extract_patch_target(input),
        },
        _ => PendingActivity {
            kind: SessionActivityKind::RunningCommand,
            target: None,
        },
    }
}

fn shell_command_text(arguments: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(arguments)
        && let Some(command) = value.get("command").and_then(Value::as_str)
    {
        return command.to_string();
    }
    arguments.to_string()
}

fn extract_view_image_target(arguments: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(arguments).ok()?;
    str_at(&value, &["path"]).or_else(|| str_at(&value, &["image_path"]))
}

fn extract_read_target(command: &str) -> Option<String> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    // Heuristic-only parser for common read-only shell command patterns.
    let prefixes = [
        "Get-Content ",
        "cat ",
        "type ",
        "rg ",
        "rg --files ",
        "Select-String ",
        "Get-ChildItem ",
    ];
    if !prefixes.iter().any(|prefix| command.starts_with(prefix)) {
        return None;
    }

    if command.starts_with("Get-Content ") {
        return positional_argument_after(command, "Get-Content");
    }

    if command.starts_with("cat ") {
        return positional_argument_after(command, "cat");
    }

    if command.starts_with("type ") {
        return positional_argument_after(command, "type");
    }

    if command.starts_with("rg ") {
        return extract_rg_target(command);
    }

    if command.starts_with("Select-String ") {
        if let Some(path_target) = named_argument(command, "-Path") {
            return Some(path_target);
        }
        return positional_argument_after(command, "Select-String");
    }

    if command.starts_with("Get-ChildItem ") {
        if let Some(path_target) = named_argument(command, "-Path") {
            return Some(path_target);
        }
        return positional_argument_after(command, "Get-ChildItem");
    }

    None
}

fn positional_argument_after(command: &str, prefix: &str) -> Option<String> {
    let rest = command.strip_prefix(prefix)?.trim();
    for token in rest.split_whitespace() {
        let cleaned = token
            .trim_matches('"')
            .trim_matches('\'')
            .trim_matches('`')
            .to_string();
        if cleaned.is_empty() || cleaned.starts_with('-') {
            continue;
        }
        return Some(cleaned);
    }
    None
}

fn named_argument(command: &str, flag: &str) -> Option<String> {
    let tokens: Vec<String> = command
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches('`')
                .to_string()
        })
        .collect();
    let mut idx = 0usize;
    while idx + 1 < tokens.len() {
        if tokens[idx].eq_ignore_ascii_case(flag) {
            let value = tokens[idx + 1].clone();
            if !value.starts_with('-') && !value.is_empty() {
                return Some(value);
            }
        }
        idx += 1;
    }
    None
}

fn extract_rg_target(command: &str) -> Option<String> {
    let tokens: Vec<String> = command
        .split_whitespace()
        .map(|token| {
            token
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches('`')
                .to_string()
        })
        .collect();

    let mut positional = Vec::new();
    let mut skip_next = false;
    for token in tokens.into_iter().skip(1) {
        if skip_next {
            skip_next = false;
            continue;
        }
        if token.is_empty() {
            continue;
        }
        if token.starts_with("--") {
            if token == "--glob"
                || token == "--iglob"
                || token == "--type"
                || token == "--type-not"
                || token == "--max-filesize"
                || token == "--sort"
                || token == "--engine"
                || token == "--replace"
                || token == "--file"
            {
                skip_next = true;
            }
            continue;
        }
        if token.starts_with('-') {
            if token == "-g"
                || token == "-t"
                || token == "-T"
                || token == "-m"
                || token == "-A"
                || token == "-B"
                || token == "-C"
                || token == "-j"
                || token == "-M"
                || token == "-S"
                || token == "-e"
                || token == "-f"
                || token == "-r"
            {
                skip_next = true;
            }
            continue;
        }
        positional.push(token);
    }

    if positional.is_empty() {
        return None;
    }

    if command.contains("--files") {
        return positional.first().cloned();
    }

    // rg [pattern] [path]
    positional.get(1).cloned()
}

fn extract_patch_target(input: &str) -> Option<String> {
    for line in input.lines() {
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            return Some(path.trim().to_string());
        }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            return Some(path.trim().to_string());
        }
        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            return Some(path.trim().to_string());
        }
        if let Some(path) = line.strip_prefix("*** Move to: ") {
            return Some(path.trim().to_string());
        }
    }
    None
}

fn truncate_activity_target(input: String, max_len: usize) -> String {
    if input.len() <= max_len {
        return input;
    }
    if max_len <= 3 {
        return input[..max_len].to_string();
    }
    format!("{}...", &input[..max_len - 3])
}

#[cfg(test)]
fn parse_session_file(
    jsonl_path: &Path,
    last_activity: SystemTime,
    git_cache: &mut GitBranchCache,
) -> Result<Option<CodexSessionSnapshot>> {
    let file = File::open(jsonl_path)
        .with_context(|| format!("failed to open session file {}", jsonl_path.display()))?;
    let mut reader = BufReader::new(file);
    let mut accumulator = SessionAccumulator::default();
    parse_new_lines(&mut reader, &mut accumulator)?;
    Ok(accumulator.build_snapshot(jsonl_path, last_activity, git_cache))
}

fn parse_session_file_cached(
    jsonl_path: &Path,
    metadata: &std::fs::Metadata,
    last_activity: SystemTime,
    git_cache: &mut GitBranchCache,
    parse_cache: &mut SessionParseCache,
) -> Result<Option<CodexSessionSnapshot>> {
    let modified = metadata.modified().unwrap_or(last_activity);
    let file_len = metadata.len();
    let key = jsonl_path.to_path_buf();
    let cached = parse_cache
        .entries
        .entry(key)
        .or_insert_with(|| CachedSessionEntry::new(modified));

    let should_reset = cached.cursor > file_len || modified < cached.modified;
    if should_reset {
        cached.reset(modified);
    }

    if cached.file_len == file_len
        && cached.modified == modified
        && let Some(snapshot) = cached.snapshot.clone()
    {
        return Ok(Some(snapshot));
    }

    let mut file = File::open(jsonl_path)
        .with_context(|| format!("failed to open session file {}", jsonl_path.display()))?;
    file.seek(SeekFrom::Start(cached.cursor))
        .with_context(|| format!("failed to seek session file {}", jsonl_path.display()))?;
    let mut reader = BufReader::new(file);
    parse_new_lines(&mut reader, &mut cached.accumulator)?;
    cached.cursor = reader.stream_position().unwrap_or(file_len);
    cached.file_len = file_len;
    cached.modified = modified;

    let snapshot = cached
        .accumulator
        .build_snapshot(jsonl_path, last_activity, git_cache);
    cached.snapshot = snapshot.clone();
    Ok(snapshot)
}

fn parse_new_lines(
    reader: &mut BufReader<File>,
    accumulator: &mut SessionAccumulator,
) -> Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed = match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => value,
            Err(_) => continue,
        };
        accumulator.apply_event(&parsed);
    }
    Ok(())
}

fn compute_session_delta(
    latest_total: Option<u64>,
    previous_total: Option<u64>,
    fallback_last_turn: Option<u64>,
) -> Option<u64> {
    match (latest_total, previous_total) {
        (Some(latest), Some(previous)) => Some(latest.saturating_sub(previous)),
        _ => fallback_last_turn,
    }
}

fn total_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "total_token_usage", "total_tokens"])
}

fn last_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "last_token_usage", "total_tokens"])
}

fn parse_rate_limits(value: Option<&Value>) -> RateLimits {
    let Some(value) = value else {
        return RateLimits::default();
    };
    RateLimits {
        primary: parse_usage_window(value.get("primary")),
        secondary: parse_usage_window(value.get("secondary")),
    }
}

fn parse_usage_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?;
    let used_percent = clamp_percent(float_at(value, &["used_percent"]).unwrap_or(0.0));
    let remaining_percent = clamp_percent(100.0 - used_percent);

    Some(UsageWindow {
        used_percent,
        remaining_percent,
        window_minutes: uint_at(value, &["window_minutes"]).unwrap_or(0),
        resets_at: int_at(value, &["resets_at"])
            .and_then(|epoch| Utc.timestamp_opt(epoch, 0).single()),
    })
}

fn clamp_percent(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 100.0)
}

fn max_datetime(
    left: Option<DateTime<Utc>>,
    right: Option<DateTime<Utc>>,
) -> Option<DateTime<Utc>> {
    match (left, right) {
        (Some(a), Some(b)) => Some(if a >= b { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn parse_utc_timestamp(text: String) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&text)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

fn fetch_git_branch(project_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if branch == "HEAD" {
        let output = Command::new("git")
            .arg("-C")
            .arg(project_path)
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let short = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return (!short.is_empty()).then_some(short);
    }
    (!branch.is_empty()).then_some(branch)
}

fn str_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(|s| s.to_string())
}

fn uint_at(value: &Value, path: &[&str]) -> Option<u64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor
        .as_u64()
        .or_else(|| cursor.as_i64().and_then(|n| (n >= 0).then_some(n as u64)))
}

fn int_at(value: &Value, path: &[&str]) -> Option<i64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor
        .as_i64()
        .or_else(|| cursor.as_u64().map(|n| n as i64))
}

fn float_at(value: &Value, path: &[&str]) -> Option<f64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor
        .as_f64()
        .or_else(|| cursor.as_u64().map(|n| n as f64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use tempfile::TempDir;

    fn parse_one(content: &str) -> CodexSessionSnapshot {
        let tmp = TempDir::new().expect("temp dir");
        let file_path = tmp.path().join("session.jsonl");
        std::fs::write(&file_path, content).expect("write jsonl");
        let modified = SystemTime::now();
        let mut git_cache = GitBranchCache::new(Duration::from_secs(30));
        parse_session_file(&file_path, modified, &mut git_cache)
            .expect("parse")
            .expect("snapshot")
    }

    fn policy_snapshot(activity_kind: Option<SessionActivityKind>) -> CodexSessionSnapshot {
        CodexSessionSnapshot {
            session_id: "policy".to_string(),
            cwd: PathBuf::from("."),
            project_name: "policy-project".to_string(),
            git_branch: None,
            model: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: None,
            last_turn_tokens: None,
            session_delta_tokens: None,
            limits: RateLimits::default(),
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
        assert_eq!(activity.target.as_deref(), Some("src/ui.rs"));
        assert_eq!(activity.to_text(true), "Reading src/ui.rs");
    }

    #[test]
    fn parses_editing_activity_from_apply_patch() {
        let snapshot = parse_one(
            r#"{"type":"session_meta","payload":{"id":"edit","cwd":"C:\\repo\\app"}}
{"timestamp":"2026-02-09T16:42:13Z","type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"call_patch","input":"*** Begin Patch\n*** Update File: src/session.rs\n@@\n*** End Patch\n"}}"#,
        );

        let activity = snapshot.activity.expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::EditingFile);
        assert_eq!(activity.target.as_deref(), Some("src/session.rs"));
        assert_eq!(activity.to_text(true), "Editing src/session.rs");
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
            model: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: None,
            last_turn_tokens: None,
            session_delta_tokens: None,
            limits: RateLimits {
                primary: Some(UsageWindow {
                    used_percent: 50.0,
                    remaining_percent: 50.0,
                    window_minutes: 300,
                    resets_at: None,
                }),
                secondary: None,
            },
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
            model: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: None,
            last_turn_tokens: None,
            session_delta_tokens: None,
            limits: RateLimits {
                primary: Some(UsageWindow {
                    used_percent: 20.0,
                    remaining_percent: 80.0,
                    window_minutes: 300,
                    resets_at: None,
                }),
                secondary: None,
            },
            activity: None,
            started_at: None,
            last_token_event_at: Utc.timestamp_opt(2000, 0).single(),
            last_activity: now,
            source_file: PathBuf::from("newer.jsonl"),
        };

        let sessions = vec![older, newer];
        let source = latest_limits_source(&sessions).expect("limits source");
        assert_eq!(source.session_id, "newer");
    }

    #[test]
    fn sticky_policy_keeps_non_idle_session_within_window() {
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
    fn sticky_policy_excludes_session_outside_sticky_window() {
        let now = SystemTime::now();
        let recency = now
            .checked_sub(Duration::from_secs(2 * 60 * 60))
            .expect("recency");
        let stale_cutoff = now.checked_sub(Duration::from_secs(90)).expect("stale");
        let sticky_cutoff = now
            .checked_sub(Duration::from_secs(60 * 60))
            .expect("sticky");
        let snapshot = policy_snapshot(Some(SessionActivityKind::WaitingInput));

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
        let activity_ts = Utc::now() - ChronoDuration::minutes(10);
        let token_ts = Utc::now() - ChronoDuration::minutes(20);
        let mut snapshot = policy_snapshot(Some(SessionActivityKind::Thinking));
        snapshot.last_token_event_at = Some(token_ts);
        if let Some(activity) = snapshot.activity.as_mut() {
            activity.observed_at = Some(activity_ts);
            activity.last_active_at = Some(activity_ts);
        }

        let recency = session_recency(&snapshot, file_modified);
        let expected = datetime_to_system_time(activity_ts).expect("expected");
        assert_eq!(recency, expected);
    }

    #[test]
    fn session_ranking_prioritizes_pending_then_non_idle_then_recency() {
        let now = SystemTime::now();

        let mut pending = policy_snapshot(Some(SessionActivityKind::RunningCommand));
        pending.session_id = "pending".to_string();
        pending.last_activity = now
            .checked_sub(Duration::from_secs(600))
            .expect("pending recency");
        if let Some(activity) = pending.activity.as_mut() {
            activity.pending_calls = 2;
        }

        let mut non_idle = policy_snapshot(Some(SessionActivityKind::Thinking));
        non_idle.session_id = "non_idle".to_string();
        non_idle.last_activity = now
            .checked_sub(Duration::from_secs(120))
            .expect("non_idle recency");

        let mut idle_recent = policy_snapshot(Some(SessionActivityKind::Idle));
        idle_recent.session_id = "idle_recent".to_string();
        idle_recent.last_activity = now;

        let mut sessions = [idle_recent, non_idle, pending];
        sessions.sort_by_key(|session| Reverse(session_rank_key(session)));

        assert_eq!(sessions[0].session_id, "pending");
        assert_eq!(sessions[1].session_id, "non_idle");
        assert_eq!(sessions[2].session_id, "idle_recent");
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
