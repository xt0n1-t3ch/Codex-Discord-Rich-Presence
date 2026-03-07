use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use tempfile::NamedTempFile;
use tracing_subscriber::{EnvFilter, fmt};

use crate::session::ReasoningEffort;

pub fn setup_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).without_time().try_init();
}

pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}K", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

pub fn format_cost(cost_usd: f64) -> String {
    if !cost_usd.is_finite() || cost_usd <= 0.0 {
        return "$0.00".to_string();
    }
    if cost_usd < 0.01 {
        format!("${cost_usd:.4}")
    } else if cost_usd < 1.0 {
        format!("${cost_usd:.3}")
    } else {
        format!("${cost_usd:.2}")
    }
}

pub fn format_delta_tokens(tokens: u64) -> String {
    format_tokens(tokens)
}

pub fn format_model_name(model_id: &str) -> String {
    if model_id.trim().is_empty() {
        return "unknown".to_string();
    }

    model_id
        .split('-')
        .filter(|part| !part.is_empty())
        .map(format_model_component)
        .collect::<Vec<_>>()
        .join("-")
}

pub fn format_model_display(
    model_id: &str,
    reasoning_effort: Option<ReasoningEffort>,
    fast_active: bool,
) -> String {
    let base = format_model_name(model_id);
    let effort_suffix = reasoning_effort
        .map(|value| format!(" ({})", value.label()))
        .unwrap_or_default();
    if fast_active {
        format!("⚡ {base}{effort_suffix}")
    } else {
        format!("{base}{effort_suffix}")
    }
}

fn format_model_component(component: &str) -> String {
    let lower = component.to_ascii_lowercase();
    match lower.as_str() {
        "gpt" => "GPT".to_string(),
        "codex" => "Codex".to_string(),
        "mini" => "Mini".to_string(),
        "max" => "Max".to_string(),
        "nano" => "Nano".to_string(),
        "turbo" => "Turbo".to_string(),
        "preview" => "Preview".to_string(),
        _ => {
            if lower
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '.' || ch == 'x')
            {
                return lower;
            }
            if lower.starts_with('o') && lower.chars().skip(1).all(|ch| ch.is_ascii_digit()) {
                return lower.to_ascii_uppercase();
            }
            let mut chars = lower.chars();
            let Some(first) = chars.next() else {
                return lower;
            };
            format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
        }
    }
}

pub fn format_token_triplet(delta: Option<u64>, last: Option<u64>, total: Option<u64>) -> String {
    let mut parts = Vec::new();
    if let Some(value) = delta {
        parts.push(format!("This update {}", format_delta_tokens(value)));
    }
    if let Some(value) = last {
        parts.push(format!("Last response {}", format_tokens(value)));
    }
    if let Some(value) = total {
        parts.push(format!("Session total {}", format_tokens(value)));
    }
    if parts.is_empty() {
        "Tokens: unavailable".to_string()
    } else {
        format!("Tokens: {}", parts.join(" | "))
    }
}

pub fn format_time_until(target: Option<DateTime<Utc>>) -> String {
    let Some(target) = target else {
        return "n/a".to_string();
    };

    let now = Utc::now();
    if target <= now {
        return "now".to_string();
    }

    let delta = (target - now).to_std().unwrap_or_default();
    human_duration(delta)
}

pub fn human_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

pub fn progress_bar(percent: f64, width: usize) -> String {
    let pct = percent.clamp(0.0, 100.0);
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("{}{}", "#".repeat(filled), "-".repeat(empty))
}

pub fn truncate(input: &str, max_len: usize) -> String {
    if input.len() <= max_len {
        return input.to_string();
    }
    if max_len <= 3 {
        return safe_prefix(input, max_len).to_string();
    }
    format!("{}...", safe_prefix(input, max_len - 3))
}

pub fn now_local() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn format_since(target: Option<DateTime<Utc>>) -> String {
    let Some(target) = target else {
        return "n/a".to_string();
    };
    let now = Utc::now();
    if target >= now {
        return "just now".to_string();
    }
    let delta = (now - target).to_std().unwrap_or_default();
    format!("{} ago", human_duration(delta))
}

pub fn write_text_atomic(path: &Path, contents: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;

    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(contents.as_bytes())?;
    temp.flush()?;
    temp.as_file_mut().sync_all()?;
    temp.persist(path).map(|_| ()).map_err(|err| err.error)
}

pub fn write_json_pretty_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    write_text_atomic(path, &json)
}

fn safe_prefix(input: &str, max_len: usize) -> &str {
    let mut end = max_len.min(input.len());
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_formatting() {
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn token_triplet_formatting() {
        assert_eq!(
            format_token_triplet(Some(1500), Some(2500), Some(60_000)),
            "Tokens: This update 1.5K | Last response 2.5K | Session total 60.0K"
        );
        assert_eq!(
            format_token_triplet(None, None, None),
            "Tokens: unavailable"
        );
    }

    #[test]
    fn cost_formatting() {
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(0.0009), "$0.0009");
        assert_eq!(format_cost(0.1284), "$0.128");
        assert_eq!(format_cost(12.3456), "$12.35");
    }

    #[test]
    fn model_name_formatting() {
        assert_eq!(format_model_name("gpt-5.3-codex"), "GPT-5.3-Codex");
        assert_eq!(
            format_model_name("gpt-5.1-codex-mini"),
            "GPT-5.1-Codex-Mini"
        );
        assert_eq!(format_model_name("o3"), "O3");
    }

    #[test]
    fn model_display_includes_fast_icon_and_effort() {
        assert_eq!(
            format_model_display("gpt-5.4", Some(ReasoningEffort::XHigh), true),
            "⚡ GPT-5.4 (Extra High)"
        );
    }

    #[test]
    fn truncate_is_utf8_safe() {
        assert_eq!(truncate("⚡ GPT-5.4", 6), "⚡...");
    }

    #[test]
    fn write_text_atomic_replaces_existing_file_contents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("config.json");

        write_text_atomic(&path, "first").expect("write first");
        write_text_atomic(&path, "second").expect("write second");

        assert_eq!(std::fs::read_to_string(path).expect("read"), "second");
    }
}
