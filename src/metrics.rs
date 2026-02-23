use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::config;
use crate::session::CodexSessionSnapshot;
use crate::util::{format_cost, format_tokens, human_duration};

const PERSIST_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub daemon_started_at: DateTime<Utc>,
    pub snapshot_at: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub totals: TokenTotals,
    pub cost_breakdown: CostBreakdown,
    pub by_model: Vec<ModelMetrics>,
    pub active_sessions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenTotals {
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostBreakdown {
    pub input_cost_usd: f64,
    pub cached_input_cost_usd: f64,
    pub output_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub model_id: String,
    pub cost_usd: f64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub session_count: u32,
}

#[derive(Debug, Clone, Default)]
struct SessionRecord {
    model_id: String,
    cost_usd: f64,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    input_cost_usd: f64,
    cached_input_cost_usd: f64,
    output_cost_usd: f64,
}

pub struct MetricsTracker {
    daemon_started_at: DateTime<Utc>,
    started_instant: Instant,
    sessions: HashMap<String, SessionRecord>,
    last_persist_at: Option<Instant>,
    cached_snapshot: Option<MetricsSnapshot>,
}

impl MetricsTracker {
    pub fn new() -> Self {
        Self {
            daemon_started_at: Utc::now(),
            started_instant: Instant::now(),
            sessions: HashMap::new(),
            last_persist_at: None,
            cached_snapshot: None,
        }
    }

    pub fn update(&mut self, sessions: &[CodexSessionSnapshot]) {
        for session in sessions {
            let record = SessionRecord {
                model_id: session
                    .model
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                cost_usd: session.total_cost_usd,
                input_tokens: session.input_tokens_total,
                cached_input_tokens: session.cached_input_tokens_total,
                output_tokens: session.output_tokens_total,
                input_cost_usd: session.cost_breakdown.input_cost_usd,
                cached_input_cost_usd: session.cost_breakdown.cached_input_cost_usd,
                output_cost_usd: session.cost_breakdown.output_cost_usd,
            };
            self.sessions.insert(session.session_id.clone(), record);
        }
        self.cached_snapshot = Some(self.compute_snapshot(sessions.len()));
    }

    pub fn snapshot(&self) -> Option<&MetricsSnapshot> {
        self.cached_snapshot.as_ref()
    }

    pub fn persist_if_due(&mut self) {
        if let Some(last) = self.last_persist_at
            && last.elapsed() < PERSIST_INTERVAL
        {
            return;
        }
        let Some(snapshot) = self.cached_snapshot.as_ref() else {
            return;
        };

        self.last_persist_at = Some(Instant::now());
        persist_json(snapshot);
        persist_markdown(snapshot);
    }

    fn compute_snapshot(&self, active_sessions: usize) -> MetricsSnapshot {
        let mut totals = TokenTotals::default();
        let mut cost_breakdown = CostBreakdown::default();
        let mut by_model_map: HashMap<String, ModelMetrics> = HashMap::new();

        for record in self.sessions.values() {
            totals.cost_usd += record.cost_usd;
            totals.input_tokens += record.input_tokens;
            totals.cached_input_tokens += record.cached_input_tokens;
            totals.output_tokens += record.output_tokens;

            cost_breakdown.input_cost_usd += record.input_cost_usd;
            cost_breakdown.cached_input_cost_usd += record.cached_input_cost_usd;
            cost_breakdown.output_cost_usd += record.output_cost_usd;

            let entry = by_model_map
                .entry(record.model_id.clone())
                .or_insert_with(|| ModelMetrics {
                    model_id: record.model_id.clone(),
                    cost_usd: 0.0,
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                    session_count: 0,
                });
            entry.cost_usd += record.cost_usd;
            entry.input_tokens += record.input_tokens;
            entry.cached_input_tokens += record.cached_input_tokens;
            entry.output_tokens += record.output_tokens;
            entry.session_count += 1;
        }

        totals.total_tokens = totals.input_tokens + totals.output_tokens;

        let mut by_model: Vec<ModelMetrics> = by_model_map.into_values().collect();
        by_model.sort_by(|a, b| {
            b.cost_usd
                .partial_cmp(&a.cost_usd)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        MetricsSnapshot {
            daemon_started_at: self.daemon_started_at,
            snapshot_at: Utc::now(),
            uptime_seconds: self.started_instant.elapsed().as_secs(),
            totals,
            cost_breakdown,
            by_model,
            active_sessions,
        }
    }
}

impl Default for MetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn persist_json(snapshot: &MetricsSnapshot) {
    let path = config::codex_home().join("discord-presence-metrics.json");
    let tmp = config::codex_home().join("discord-presence-metrics.json.tmp");
    match serde_json::to_string_pretty(snapshot) {
        Ok(data) => {
            if let Err(err) = std::fs::write(&tmp, data) {
                warn!(error = %err, "failed to write metrics JSON tmp");
                return;
            }
            if let Err(err) = std::fs::rename(&tmp, &path) {
                warn!(error = %err, "failed to move metrics JSON into place");
            }
        }
        Err(err) => warn!(error = %err, "failed to serialize metrics JSON"),
    }
}

fn persist_markdown(snapshot: &MetricsSnapshot) {
    let path = config::codex_home().join("discord-presence-metrics.md");
    let tmp = config::codex_home().join("discord-presence-metrics.md.tmp");
    let markdown = generate_markdown(snapshot);
    if let Err(err) = std::fs::write(&tmp, markdown) {
        warn!(error = %err, "failed to write metrics markdown tmp");
        return;
    }
    if let Err(err) = std::fs::rename(&tmp, &path) {
        warn!(error = %err, "failed to move metrics markdown into place");
    }
}

fn generate_markdown(snapshot: &MetricsSnapshot) -> String {
    let now_local = Local::now().format("%b %d, %Y %I:%M %p");
    let uptime = human_duration(Duration::from_secs(snapshot.uptime_seconds));

    let mut markdown = String::new();
    markdown.push_str("# Codex Metrics Report\n\n");
    markdown.push_str(&format!(
        "*Generated: {} | Uptime: {}*\n\n",
        now_local, uptime
    ));

    markdown.push_str("## Totals\n\n");
    markdown.push_str("| Metric | Value |\n");
    markdown.push_str("|--------|-------|\n");
    markdown.push_str(&format!(
        "| Total Cost | {} |\n",
        format_cost(snapshot.totals.cost_usd)
    ));
    markdown.push_str(&format!(
        "| Total Tokens | {} |\n",
        format_tokens(snapshot.totals.total_tokens)
    ));
    markdown.push_str(&format!(
        "| Input Tokens | {} |\n",
        format_tokens(snapshot.totals.input_tokens)
    ));
    markdown.push_str(&format!(
        "| Cached Input Tokens | {} |\n",
        format_tokens(snapshot.totals.cached_input_tokens)
    ));
    markdown.push_str(&format!(
        "| Output Tokens | {} |\n",
        format_tokens(snapshot.totals.output_tokens)
    ));
    markdown.push('\n');

    markdown.push_str("## Cost Breakdown\n\n");
    markdown.push_str("| Type | Cost |\n");
    markdown.push_str("|------|------|\n");
    markdown.push_str(&format!(
        "| Input | {} |\n",
        format_cost(snapshot.cost_breakdown.input_cost_usd)
    ));
    markdown.push_str(&format!(
        "| Cached Input | {} |\n",
        format_cost(snapshot.cost_breakdown.cached_input_cost_usd)
    ));
    markdown.push_str(&format!(
        "| Output | {} |\n",
        format_cost(snapshot.cost_breakdown.output_cost_usd)
    ));
    markdown.push('\n');

    if !snapshot.by_model.is_empty() {
        markdown.push_str("## By Model\n\n");
        markdown.push_str("| Model | Sessions | Cost | Tokens |\n");
        markdown.push_str("|-------|----------|------|--------|\n");
        for model in &snapshot.by_model {
            let total_tokens = model.input_tokens + model.output_tokens;
            markdown.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                model.model_id,
                model.session_count,
                format_cost(model.cost_usd),
                format_tokens(total_tokens)
            ));
        }
        markdown.push('\n');
    }

    markdown.push_str(&format!(
        "*Active sessions: {}*\n",
        snapshot.active_sessions
    ));
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{PricingSource, TokenCostBreakdown};
    use crate::session::{RateLimits, SessionActivitySnapshot};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn make_session(
        id: &str,
        model: &str,
        input: u64,
        cached: u64,
        output: u64,
        cost: f64,
    ) -> CodexSessionSnapshot {
        CodexSessionSnapshot {
            session_id: id.to_string(),
            cwd: PathBuf::from("/test"),
            project_name: "test".to_string(),
            git_branch: None,
            model: Some(model.to_string()),
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: Some(input + output),
            last_turn_tokens: Some(0),
            session_delta_tokens: Some(0),
            input_tokens_total: input,
            cached_input_tokens_total: cached,
            output_tokens_total: output,
            last_input_tokens: Some(0),
            last_cached_input_tokens: Some(0),
            last_output_tokens: Some(0),
            total_cost_usd: cost,
            cost_breakdown: TokenCostBreakdown {
                input_cost_usd: cost / 2.0,
                cached_input_cost_usd: cost / 4.0,
                output_cost_usd: cost / 4.0,
            },
            pricing_source: PricingSource::Exact,
            context_window: None,
            limits: RateLimits::default(),
            activity: Some(SessionActivitySnapshot::default()),
            started_at: None,
            last_token_event_at: None,
            last_activity: SystemTime::now(),
            source_file: PathBuf::from("/test.jsonl"),
        }
    }

    #[test]
    fn update_replaces_session_record_instead_of_double_counting() {
        let mut tracker = MetricsTracker::new();

        let first = vec![make_session("s1", "gpt-5.2-codex", 1_000, 500, 700, 0.05)];
        tracker.update(&first);
        let snapshot = tracker.snapshot().expect("snapshot");
        assert_eq!(snapshot.totals.input_tokens, 1_000);
        assert_eq!(snapshot.totals.cached_input_tokens, 500);
        assert_eq!(snapshot.totals.output_tokens, 700);

        let second = vec![make_session("s1", "gpt-5.2-codex", 2_000, 700, 1_500, 0.08)];
        tracker.update(&second);
        let snapshot = tracker.snapshot().expect("snapshot");
        assert_eq!(snapshot.totals.input_tokens, 2_000);
        assert_eq!(snapshot.totals.cached_input_tokens, 700);
        assert_eq!(snapshot.totals.output_tokens, 1_500);
        assert!((snapshot.totals.cost_usd - 0.08).abs() < 0.0001);
    }

    #[test]
    fn aggregates_multiple_models() {
        let mut tracker = MetricsTracker::new();
        let sessions = vec![
            make_session("s1", "gpt-5.2-codex", 1_000, 200, 500, 0.04),
            make_session("s2", "gpt-5.1-codex", 2_000, 300, 700, 0.05),
        ];
        tracker.update(&sessions);

        let snapshot = tracker.snapshot().expect("snapshot");
        assert_eq!(snapshot.active_sessions, 2);
        assert_eq!(snapshot.by_model.len(), 2);
        assert_eq!(snapshot.totals.input_tokens, 3_000);
        assert_eq!(snapshot.totals.cached_input_tokens, 500);
        assert_eq!(snapshot.totals.output_tokens, 1_200);
    }

    #[test]
    fn markdown_contains_expected_sections() {
        let snapshot = MetricsSnapshot {
            daemon_started_at: Utc::now(),
            snapshot_at: Utc::now(),
            uptime_seconds: 300,
            totals: TokenTotals {
                cost_usd: 1.23,
                input_tokens: 100_000,
                cached_input_tokens: 60_000,
                output_tokens: 40_000,
                total_tokens: 140_000,
            },
            cost_breakdown: CostBreakdown {
                input_cost_usd: 0.7,
                cached_input_cost_usd: 0.13,
                output_cost_usd: 0.4,
            },
            by_model: vec![ModelMetrics {
                model_id: "gpt-5.2-codex".to_string(),
                cost_usd: 1.23,
                input_tokens: 100_000,
                cached_input_tokens: 60_000,
                output_tokens: 40_000,
                session_count: 1,
            }],
            active_sessions: 1,
        };

        let markdown = generate_markdown(&snapshot);
        assert!(markdown.contains("# Codex Metrics Report"));
        assert!(markdown.contains("## Totals"));
        assert!(markdown.contains("## Cost Breakdown"));
        assert!(markdown.contains("## By Model"));
    }
}
