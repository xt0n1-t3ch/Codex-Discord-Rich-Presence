use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config::PricingConfig;
use crate::cost;

use super::activity::SessionAccumulator;
use super::{
    CachedSessionEntry, CodexSessionSnapshot, ContextWindowSnapshot, ContextWindowSource,
    GitBranchCache, ReasoningEffort, SessionParseCache,
};

#[cfg(test)]
pub(super) fn parse_session_file(
    jsonl_path: &Path,
    last_activity: SystemTime,
    git_cache: &mut GitBranchCache,
    pricing_config: &PricingConfig,
) -> Result<Option<CodexSessionSnapshot>> {
    let file = File::open(jsonl_path)
        .with_context(|| format!("failed to open session file {}", jsonl_path.display()))?;
    let mut reader = BufReader::new(file);
    let mut accumulator = SessionAccumulator::default();
    let mut partial_line_buffer = String::new();
    parse_new_lines(&mut reader, &mut accumulator, &mut partial_line_buffer)?;
    Ok(accumulator.build_snapshot(jsonl_path, last_activity, git_cache, pricing_config))
}

pub(super) fn parse_session_file_cached(
    jsonl_path: &Path,
    metadata: &std::fs::Metadata,
    last_activity: SystemTime,
    git_cache: &mut GitBranchCache,
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

    let snapshot =
        cached
            .accumulator
            .build_snapshot(jsonl_path, last_activity, git_cache, pricing_config);
    cached.snapshot = snapshot.clone();
    Ok(snapshot)
}

pub(super) fn parse_new_lines(
    reader: &mut BufReader<File>,
    accumulator: &mut SessionAccumulator,
    partial_line_buffer: &mut String,
) -> Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }

        let line_has_terminator = line.ends_with('\n');
        let combined = if partial_line_buffer.is_empty() {
            line.to_string()
        } else {
            let mut pending = std::mem::take(partial_line_buffer);
            pending.push_str(&line);
            pending
        };

        let trimmed = combined.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(parsed) => accumulator.apply_event(&parsed),
            Err(_) if !line_has_terminator => {
                partial_line_buffer.push_str(&combined);
                break;
            }
            Err(_) => continue,
        }
    }
    Ok(())
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
    let (window_tokens, source) = if let Some(window_tokens) = event_window_tokens {
        (window_tokens, ContextWindowSource::Event)
    } else {
        let fallback_window = cost::default_model_context_window(model_id.unwrap_or(""))?;
        (fallback_window, ContextWindowSource::Catalog)
    };
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
        window_tokens,
        used_tokens,
        remaining_tokens,
        remaining_percent,
        source,
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

pub(super) fn fetch_git_branch(project_path: &Path) -> Option<String> {
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
