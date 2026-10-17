use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config::PricingConfig;
use crate::model;

use super::activity::SessionAccumulator;
use super::{
    CachedSessionEntry, CodexSessionSnapshot, ContextWindowSnapshot, ReasoningEffort,
    SessionParseCache,
};

#[cfg(test)]
pub(super) fn parse_session_file(
    jsonl_path: &Path,
    last_activity: SystemTime,
    pricing_config: &PricingConfig,
) -> Result<Option<CodexSessionSnapshot>> {
    let file = File::open(jsonl_path)
        .with_context(|| format!("failed to open session file {}", jsonl_path.display()))?;
    let mut reader = BufReader::new(file);
    let mut accumulator = SessionAccumulator::default();
    let mut partial_line_buffer = LineBuffer::default();
    parse_new_lines(&mut reader, &mut accumulator, &mut partial_line_buffer)?;
    Ok(accumulator.build_snapshot(jsonl_path, last_activity, pricing_config))
}

pub(super) fn parse_session_file_cached(
    jsonl_path: &Path,
    metadata: &std::fs::Metadata,
    last_activity: SystemTime,
    parse_cache: &mut SessionParseCache,
    pricing_config: &PricingConfig,
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
    parse_new_lines(
        &mut reader,
        &mut cached.accumulator,
        &mut cached.partial_line_buffer,
    )?;
    cached.cursor = reader.stream_position().unwrap_or(file_len);
    cached.file_len = file_len;
    cached.modified = modified;

    let snapshot = cached
        .accumulator
        .build_snapshot(jsonl_path, last_activity, pricing_config);
    cached.snapshot = snapshot.clone();
    Ok(snapshot)
}

pub(super) const MAX_SESSION_LINE_BYTES: usize = 16 * 1024 * 1024;
const RETAINED_LINE_CAPACITY: usize = 64 * 1024;

#[derive(Debug, Default)]
pub(super) struct LineBuffer {
    bytes: Vec<u8>,
    discarding: bool,
}

impl LineBuffer {
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.bytes = Vec::new();
        self.discarding = false;
    }

    fn finish_line(&mut self) {
        if self.bytes.capacity() > RETAINED_LINE_CAPACITY {
            self.bytes = Vec::new();
        } else {
            self.bytes.clear();
        }
        self.discarding = false;
    }

    fn append(&mut self, chunk: &[u8]) {
        if self.discarding {
            return;
        }
        if self.bytes.len() + chunk.len() > MAX_SESSION_LINE_BYTES {
            self.bytes = Vec::new();
            self.discarding = true;
            return;
        }
        self.bytes.extend_from_slice(chunk);
    }
}

pub(super) fn parse_new_lines(
    reader: &mut BufReader<File>,
    accumulator: &mut SessionAccumulator,
    line: &mut LineBuffer,
) -> Result<()> {
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            break;
        }
        match available.iter().position(|byte| *byte == b'\n') {
            Some(newline) => {
                line.append(&available[..newline]);
                reader.consume(newline + 1);
                if !line.discarding {
                    apply_line(&line.bytes, accumulator);
                }
                line.finish_line();
            }
            None => {
                let consumed = available.len();
                line.append(available);
                reader.consume(consumed);
            }
        }
    }

    if !line.discarding && !line.bytes.is_empty() && apply_line(&line.bytes, accumulator) {
        line.finish_line();
    }
    Ok(())
}

fn apply_line(bytes: &[u8], accumulator: &mut SessionAccumulator) -> bool {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return true;
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(parsed) => {
            accumulator.apply_event(&parsed);
            true
        }
        Err(_) => false,
    }
}

pub(super) fn compute_session_delta(
    latest_total: Option<u64>,
    previous_total: Option<u64>,
    fallback_last_turn: Option<u64>,
) -> Option<u64> {
    match (latest_total, previous_total) {
        (Some(latest), Some(previous)) => Some(latest.saturating_sub(previous)),
        _ => fallback_last_turn,
    }
}

pub(super) fn total_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "total_token_usage", "total_tokens"])
}

pub(super) fn last_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "last_token_usage", "total_tokens"])
}

pub(super) fn total_input_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "total_token_usage", "input_tokens"])
}

pub(super) fn total_cached_input_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(
        payload,
        &["info", "total_token_usage", "cached_input_tokens"],
    )
}

pub(super) fn total_output_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "total_token_usage", "output_tokens"])
}

pub(super) fn model_context_window_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "model_context_window"])
}

pub(super) fn turn_context_reasoning_effort(payload: &Value) -> Option<ReasoningEffort> {
    if let Some(raw) = str_at(payload, &["effort"]) {
        return ReasoningEffort::parse(Some(raw.as_str()));
    }
    let nested = str_at(
        payload,
        &["collaboration_mode", "settings", "reasoning_effort"],
    );
    ReasoningEffort::parse(nested.as_deref())
}

pub(super) fn last_input_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "last_token_usage", "input_tokens"])
}

pub(super) fn last_cached_input_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(
        payload,
        &["info", "last_token_usage", "cached_input_tokens"],
    )
}

pub(super) fn last_output_tokens_from_info(payload: &Value) -> Option<u64> {
    uint_at(payload, &["info", "last_token_usage", "output_tokens"])
}

pub(super) fn build_context_window_snapshot(
    model_id: Option<&str>,
    event_window_tokens: Option<u64>,
    last_turn_tokens: Option<u64>,
    session_total_tokens: Option<u64>,
) -> Option<ContextWindowSnapshot> {
    #[cfg(not(test))]
    let resolved = model::resolve_context_window(model_id.unwrap_or(""), event_window_tokens)?;
    #[cfg(test)]
    let resolved = model::resolve_context_window_from_cache_path(
        model_id.unwrap_or(""),
        event_window_tokens,
        std::path::Path::new("__codex_presence_no_models_cache__.json"),
    )?;
    let window_tokens = resolved.effective_tokens;
    if window_tokens == 0 {
        return None;
    }
    // Context usage must track active-turn usage first; session totals are cumulative and can
    // greatly exceed context windows in long sessions.
    let used_tokens = if let Some(last_turn_tokens) = last_turn_tokens {
        last_turn_tokens
    } else {
        session_total_tokens.filter(|tokens| *tokens <= window_tokens)?
    }
    .min(window_tokens);

    let remaining_tokens = window_tokens.saturating_sub(used_tokens);
    let remaining_percent =
        ((remaining_tokens as f64 / window_tokens as f64) * 100.0).clamp(0.0, 100.0);
    Some(ContextWindowSnapshot {
        raw_window_tokens: resolved.raw_tokens,
        window_tokens,
        effective_percent: resolved.effective_percent,
        used_tokens,
        remaining_tokens,
        remaining_percent,
        source: resolved.source,
        raw_source: resolved.raw_source,
    })
}

pub(super) fn max_datetime(
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

pub(super) fn parse_utc_timestamp(text: String) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&text)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

const GIT_BRANCH_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) fn fetch_git_branch(project_path: &Path) -> Option<String> {
    let branch = git_rev_parse(project_path, "--abbrev-ref")?;
    if branch == "HEAD" {
        return git_rev_parse(project_path, "--short");
    }
    Some(branch)
}

fn git_rev_parse(project_path: &Path, mode: &str) -> Option<String> {
    let mut command = crate::util::silent_command("git");
    command
        .arg("-C")
        .arg(project_path)
        .arg("rev-parse")
        .arg(mode)
        .arg("HEAD");
    let output = crate::util::output_with_timeout(&mut command, GIT_BRANCH_TIMEOUT)?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub(super) fn str_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor.as_str().map(|s| s.to_string())
}

pub(super) fn uint_at(value: &Value, path: &[&str]) -> Option<u64> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor
        .as_u64()
        .or_else(|| cursor.as_i64().and_then(|n| (n >= 0).then_some(n as u64)))
}
