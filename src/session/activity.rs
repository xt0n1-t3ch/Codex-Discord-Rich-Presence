use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config::PricingConfig;
use crate::cost;
use crate::telemetry::limits::{
    RateLimitEnvelope, RateLimits, limits_present as telemetry_limits_present,
    parse_rate_limit_envelope, select_session_envelope_global_first,
};

use super::is_working_activity_kind;
use super::parser::{
    build_context_window_snapshot, compute_session_delta, last_cached_input_tokens_from_info,
    last_input_tokens_from_info, last_output_tokens_from_info, last_tokens_from_info, max_datetime,
    model_context_window_from_info, parse_utc_timestamp, str_at,
    total_cached_input_tokens_from_info, total_input_tokens_from_info,
    total_output_tokens_from_info, total_tokens_from_info, turn_context_reasoning_effort,
};
use super::{
    CodexSessionSnapshot, GitBranchCache, ReasoningEffort, SessionActivityKind,
    SessionActivitySnapshot,
};

#[derive(Debug, Default)]
pub(super) struct SessionAccumulator {
    session_id: Option<String>,
    cwd: Option<PathBuf>,
    started_at: Option<DateTime<Utc>>,
    originator: Option<String>,
    source: Option<String>,
    model: Option<String>,
    reasoning_effort: Option<ReasoningEffort>,
    approval_policy: Option<String>,
    sandbox_policy: Option<String>,
    pub(super) session_total_tokens: Option<u64>,
    previous_session_total_tokens: Option<u64>,
    last_turn_tokens: Option<u64>,
    input_tokens_total: u64,
    cached_input_tokens_total: u64,
    output_tokens_total: u64,
    last_input_tokens: Option<u64>,
    last_cached_input_tokens: Option<u64>,
    last_output_tokens: Option<u64>,
    model_context_window: Option<u64>,
    limits: RateLimits,
    rate_limit_envelopes: HashMap<String, RateLimitEnvelope>,
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

    fn note_commentary(&mut self, observed_at: Option<DateTime<Utc>>) {
        self.observe_effective_signal(observed_at);
        let should_promote = self.snapshot.as_ref().is_none_or(|snapshot| {
            matches!(
                snapshot.kind,
                SessionActivityKind::Idle | SessionActivityKind::WaitingInput
            )
        });
        if should_promote {
            self.mark_activity(SessionActivityKind::Thinking, None, observed_at);
            return;
        }

        if let Some(snapshot) = self.snapshot.as_mut() {
            snapshot.observed_at = max_datetime(snapshot.observed_at, observed_at);
            snapshot.last_active_at = max_datetime(snapshot.last_active_at, observed_at);
            snapshot.last_effective_signal_at = max_datetime(
                snapshot.last_effective_signal_at,
                self.last_effective_signal_at,
            );
            snapshot.pending_calls = self.pending_calls.len();
            if snapshot.pending_calls == 0 && is_working_activity_kind(&snapshot.kind) {
                snapshot.idle_candidate_at = snapshot.last_active_at;
            }
        }
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
        }

        Some(snapshot)
    }
}

impl SessionAccumulator {
    pub(super) fn apply_event(&mut self, parsed: &Value) {
        let typ = str_at(parsed, &["type"]);
        let payload = parsed.get("payload").unwrap_or(&Value::Null);
        let event_timestamp = str_at(parsed, &["timestamp"])
            .or_else(|| str_at(payload, &["timestamp"]))
            .and_then(parse_utc_timestamp);
        self.activity_tracker.observe_timestamp(event_timestamp);

        match typ.as_deref() {
            Some("session_meta") => {
                if let Some(incoming_session_id) = str_at(payload, &["id"]) {
                    let changed_session = self
                        .session_id
                        .as_deref()
                        .is_some_and(|current| current != incoming_session_id.as_str());
                    if changed_session {
                        self.reset_for_new_session(incoming_session_id.clone());
                    } else if self.session_id.is_none() {
                        self.session_id = Some(incoming_session_id);
                    }
                }
                let session_started = str_at(payload, &["timestamp"]).and_then(parse_utc_timestamp);
                self.started_at = max_datetime(self.started_at, session_started);
                if self.cwd.is_none() {
                    self.cwd = str_at(payload, &["cwd"]).map(PathBuf::from);
                }
                if self.originator.is_none() {
                    self.originator = str_at(payload, &["originator"]);
                }
                if self.source.is_none() {
                    self.source = str_at(payload, &["source"]);
                }
            }
            Some("turn_context") => {
                if self.cwd.is_none() {
                    self.cwd = str_at(payload, &["cwd"]).map(PathBuf::from);
                }
                if self.model.is_none() {
                    self.model = str_at(payload, &["model"]);
                }
                if let Some(reasoning_effort) = turn_context_reasoning_effort(payload) {
                    self.reasoning_effort = Some(reasoning_effort);
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
                    self.previous_session_total_tokens = self.session_total_tokens;

                    if let Some(total_input_tokens) = total_input_tokens_from_info(payload) {
                        self.input_tokens_total = total_input_tokens;
                    }
                    if let Some(total_cached_input_tokens) =
                        total_cached_input_tokens_from_info(payload)
                    {
                        self.cached_input_tokens_total = total_cached_input_tokens;
                    }
                    if let Some(total_output_tokens) = total_output_tokens_from_info(payload) {
                        self.output_tokens_total = total_output_tokens;
                    }

                    if let Some(last_input_tokens) = last_input_tokens_from_info(payload) {
                        self.last_input_tokens = Some(last_input_tokens);
                    }
                    if let Some(last_cached_input_tokens) =
                        last_cached_input_tokens_from_info(payload)
                    {
                        self.last_cached_input_tokens = Some(last_cached_input_tokens);
                    }
                    if let Some(last_output_tokens) = last_output_tokens_from_info(payload) {
                        self.last_output_tokens = Some(last_output_tokens);
                    }

                    if let Some(total_tokens) = total_tokens_from_info(payload) {
                        self.session_total_tokens = Some(total_tokens);
                    } else if self.input_tokens_total > 0 || self.output_tokens_total > 0 {
                        self.session_total_tokens =
                            Some(self.input_tokens_total + self.output_tokens_total);
                    }

                    if let Some(last_tokens) = last_tokens_from_info(payload) {
                        self.last_turn_tokens = Some(last_tokens);
                    } else if self.last_input_tokens.is_some() || self.last_output_tokens.is_some()
                    {
                        let last_input = self.last_input_tokens.unwrap_or(0);
                        let last_output = self.last_output_tokens.unwrap_or(0);
                        self.last_turn_tokens = Some(last_input + last_output);
                    }
                    if let Some(context_window) = model_context_window_from_info(payload) {
                        self.model_context_window = Some(context_window);
                    }

                    if let Some(parsed_limit) =
                        parse_rate_limit_envelope(payload.get("rate_limits"), event_timestamp)
                    {
                        let key = parsed_limit
                            .limit_id
                            .clone()
                            .unwrap_or_else(|| format!("scope:{}", parsed_limit.scope.as_slug()));
                        self.rate_limit_envelopes.insert(key, parsed_limit);

                        let envelopes: Vec<RateLimitEnvelope> =
                            self.rate_limit_envelopes.values().cloned().collect();
                        if let Some(selected) = select_session_envelope_global_first(&envelopes) {
                            self.limits = selected.limits;
                        }
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
                    self.activity_tracker.note_commentary(event_timestamp);
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
                Some("web_search_call") | Some("web_search_result") => {
                    self.activity_tracker.mark_activity(
                        SessionActivityKind::RunningCommand,
                        web_search_target(payload),
                        event_timestamp,
                    );
                }
                Some("message") => {
                    if str_at(payload, &["role"]).as_deref() == Some("assistant") {
                        if str_at(payload, &["phase"]).as_deref() == Some("commentary") {
                            self.activity_tracker.note_commentary(event_timestamp);
                        } else {
                            self.activity_tracker.mark_activity(
                                SessionActivityKind::WaitingInput,
                                None,
                                event_timestamp,
                            );
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn reset_for_new_session(&mut self, session_id: String) {
        *self = SessionAccumulator::default();
        self.session_id = Some(session_id);
    }

    pub(super) fn build_snapshot(
        &self,
        jsonl_path: &Path,
        last_activity: SystemTime,
        git_cache: &mut GitBranchCache,
        pricing_config: &PricingConfig,
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
            && self.input_tokens_total == 0
            && self.cached_input_tokens_total == 0
            && self.output_tokens_total == 0
            && self.rate_limit_envelopes.is_empty()
            && !telemetry_limits_present(&self.limits)
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
        let cost = cost::compute_total_cost(
            self.model.as_deref().unwrap_or(""),
            self.input_tokens_total,
            self.cached_input_tokens_total,
            self.output_tokens_total,
            pricing_config,
        );
        let mut rate_limit_envelopes: Vec<RateLimitEnvelope> =
            self.rate_limit_envelopes.values().cloned().collect();
        rate_limit_envelopes.sort_by_key(|item| {
            Reverse(
                item.observed_at
                    .map(|ts| ts.timestamp_millis())
                    .unwrap_or(i64::MIN),
            )
        });
        let selected_limits = select_session_envelope_global_first(&rate_limit_envelopes)
            .map(|selected| selected.limits)
            .unwrap_or_else(|| self.limits.clone());
        let context_window = build_context_window_snapshot(
            self.model.as_deref(),
            self.model_context_window,
            self.last_turn_tokens,
            self.session_total_tokens,
        );

        Some(CodexSessionSnapshot {
            session_id: self.session_id.clone().unwrap_or(fallback_id),
            cwd,
            project_name,
            git_branch,
            originator: self.originator.clone(),
            source: self.source.clone(),
            model: self.model.clone(),
            reasoning_effort: self.reasoning_effort,
            approval_policy: self.approval_policy.clone(),
            sandbox_policy: self.sandbox_policy.clone(),
            session_total_tokens: self.session_total_tokens,
            last_turn_tokens: self.last_turn_tokens,
            session_delta_tokens,
            input_tokens_total: self.input_tokens_total,
            cached_input_tokens_total: self.cached_input_tokens_total,
            output_tokens_total: self.output_tokens_total,
            last_input_tokens: self.last_input_tokens,
            last_cached_input_tokens: self.last_cached_input_tokens,
            last_output_tokens: self.last_output_tokens,
            total_cost_usd: cost.total_cost_usd,
            cost_breakdown: cost.breakdown,
            pricing_source: cost.source,
            context_window,
            limits: selected_limits,
            rate_limit_envelopes,
            activity,
            started_at: self.started_at,
            last_token_event_at: self.last_token_event_at,
            last_activity,
            source_file: jsonl_path.to_path_buf(),
        })
    }
}

pub(super) fn looks_like_desktop_surface(value: &str) -> bool {
    value.to_ascii_lowercase().contains("desktop")
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
        target: Some(summarize_command_for_presence(&command, 72)),
    }
}

fn classify_function_call(name: &str, arguments: &str) -> PendingActivity {
    match name {
        "shell_command" | "exec_command" => classify_shell_command(arguments),
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

fn web_search_target(payload: &Value) -> Option<String> {
    if let Some(query) = str_at(payload, &["action", "query"]) {
        return Some(truncate_activity_target(format!("web search: {query}"), 72));
    }

    if let Some(query) = payload
        .get("action")
        .and_then(|value| value.get("queries"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
    {
        return Some(truncate_activity_target(format!("web search: {query}"), 72));
    }

    Some("web search".to_string())
}

fn shell_command_text(arguments: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(arguments) {
        if let Some(command) = value.get("command").and_then(Value::as_str) {
            return command.to_string();
        }
        if let Some(command) = value.get("cmd").and_then(Value::as_str) {
            return command.to_string();
        }
    }
    arguments.to_string()
}

fn summarize_command_for_presence(command: &str, max_len: usize) -> String {
    let tokens: Vec<String> = command
        .split_whitespace()
        .map(clean_shell_token)
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return truncate_activity_target(command.trim().to_string(), max_len);
    }

    let first = tokens[0].clone();
    let second = tokens.get(1).cloned();
    let summary = match (first.as_str(), second.as_deref()) {
        ("rg", Some("--files")) => "rg --files".to_string(),
        ("cargo", Some(sub)) => format!("cargo {sub}"),
        ("sed", Some("-n")) => "sed -n".to_string(),
        ("git", Some(sub)) => format!("git {sub}"),
        ("cmd", Some("/c")) => "cmd /c".to_string(),
        ("powershell", Some(sub)) => format!("powershell {sub}"),
        ("pwsh", Some(sub)) => format!("pwsh {sub}"),
        (_, Some(sub)) if !sub.starts_with('-') && sub.len() <= 18 => {
            format!("{first} {sub}")
        }
        _ => first,
    };

    truncate_activity_target(summary, max_len)
}

fn clean_shell_token(token: &str) -> String {
    token
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`')
        .to_string()
}

fn extract_view_image_target(arguments: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(arguments).ok()?;
    str_at(&value, &["path"])
        .or_else(|| str_at(&value, &["image_path"]))
        .map(|target| sanitize_file_target(&target, 72))
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
        return Some(sanitize_file_target(&cleaned, 72));
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
                return Some(sanitize_file_target(&value, 72));
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
        return positional
            .first()
            .map(|target| sanitize_file_target(target, 72));
    }

    positional
        .get(1)
        .map(|target| sanitize_file_target(target, 72))
}

fn extract_patch_target(input: &str) -> Option<String> {
    for line in input.lines() {
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            return Some(sanitize_file_target(path.trim(), 72));
        }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            return Some(sanitize_file_target(path.trim(), 72));
        }
        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            return Some(sanitize_file_target(path.trim(), 72));
        }
        if let Some(path) = line.strip_prefix("*** Move to: ") {
            return Some(sanitize_file_target(path.trim(), 72));
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

fn sanitize_file_target(raw: &str, max_len: usize) -> String {
    let cleaned = raw
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_matches('`');
    if cleaned.is_empty() {
        return String::new();
    }

    let path = Path::new(cleaned);
    if let Some(file_name) = path.file_name().and_then(|item| item.to_str())
        && !file_name.trim().is_empty()
    {
        return truncate_activity_target(file_name.trim().to_string(), max_len);
    }

    truncate_activity_target(cleaned.to_string(), max_len)
}
