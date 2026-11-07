//! Session-wide token and cost totals for the dashboard, kept in memory.

use std::collections::HashMap;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::cost::{PricingSource, PricingStatus, format_presentable_cost};
use crate::session::CodexSessionSnapshot;

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
    /// Backward-compatible known subtotal. Use `known_cost_usd` and `pricing_status` for display.
    pub cost_usd: f64,
    pub known_cost_usd: Option<f64>,
    pub pricing_status: PricingStatus,
    pub complete_sessions: u32,
    pub incomplete_sessions: u32,
    pub unavailable_sessions: u32,
    pub pricing_sources: PricingSourceCounts,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cache_hit_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostBreakdown {
    pub input_cost_usd: f64,
    pub cache_write_cost_usd: f64,
    pub cached_input_cost_usd: f64,
    pub output_cost_usd: f64,
    pub cached_input_savings_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub model_id: String,
    pub cost_usd: f64,
    pub known_cost_usd: Option<f64>,
    pub pricing_status: PricingStatus,
    pub complete_sessions: u32,
    pub incomplete_sessions: u32,
    pub unavailable_sessions: u32,
    pub pricing_sources: PricingSourceCounts,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub cache_hit_ratio: f64,
    pub session_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PricingSourceCounts {
    pub catalog_exact: u32,
    pub catalog_alias: u32,
    pub user_override: u32,
    pub provider_reported: u32,
    pub unavailable: u32,
    pub legacy: u32,
}

impl PricingSourceCounts {
    fn record(&mut self, source: PricingSource) {
        let counter = match source {
            PricingSource::Exact => &mut self.catalog_exact,
            PricingSource::Alias => &mut self.catalog_alias,
            PricingSource::Override => &mut self.user_override,
            PricingSource::ProviderReported => &mut self.provider_reported,
            PricingSource::Unavailable => &mut self.unavailable,
            PricingSource::Partial | PricingSource::Fallback => &mut self.legacy,
        };
        *counter = counter.saturating_add(1);
    }
}

#[derive(Debug, Clone, Default)]
struct SessionRecord {
    model_id: String,
    known_cost_usd: Option<f64>,
    pricing_status: PricingStatus,
    pricing_source: PricingSource,
    breakdown_reconciled: bool,
    input_tokens: u64,
    cached_input_tokens: u64,
    output_tokens: u64,
    input_cost_usd: f64,
    cache_write_cost_usd: f64,
    cached_input_cost_usd: f64,
    output_cost_usd: f64,
    cached_input_savings_usd: f64,
}

pub struct MetricsTracker {
    daemon_started_at: DateTime<Utc>,
    started_instant: Instant,
    sessions: HashMap<String, SessionRecord>,
    cached_snapshot: Option<MetricsSnapshot>,
}

impl MetricsTracker {
    pub fn new() -> Self {
        Self {
            daemon_started_at: Utc::now(),
            started_instant: Instant::now(),
            sessions: HashMap::new(),
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
                known_cost_usd: session.known_cost_usd,
                pricing_status: session.pricing_status,
                pricing_source: session.pricing_source,
                breakdown_reconciled: session.cost_breakdown_reconciled,
                input_tokens: session.input_tokens_total,
                cached_input_tokens: session.cached_input_tokens_total,
                output_tokens: session.output_tokens_total,
                input_cost_usd: session.cost_breakdown.input_cost_usd,
                cache_write_cost_usd: session.cost_breakdown.cache_write_cost_usd,
                cached_input_cost_usd: session.cost_breakdown.cached_input_cost_usd,
                output_cost_usd: session.cost_breakdown.output_cost_usd,
                cached_input_savings_usd: session.cost_breakdown.cached_input_savings_usd,
            };
            self.sessions.insert(session.session_id.clone(), record);
        }
        self.cached_snapshot = Some(self.compute_snapshot(sessions.len()));
    }

    pub fn snapshot(&self) -> Option<&MetricsSnapshot> {
        self.cached_snapshot.as_ref()
    }

    fn compute_snapshot(&self, active_sessions: usize) -> MetricsSnapshot {
        let mut totals = TokenTotals::default();
        let mut cost_breakdown = CostBreakdown::default();
        let mut by_model_map: HashMap<String, ModelMetrics> = HashMap::new();

        for record in self.sessions.values() {
            let known_cost = valid_known_cost(record.known_cost_usd);
            let pricing_status = effective_pricing_status(known_cost, record.pricing_status);
            if let Some(cost) = known_cost {
                totals.cost_usd = finite_add(totals.cost_usd, cost).unwrap_or(totals.cost_usd);
            }
            record_status(
                pricing_status,
                &mut totals.complete_sessions,
                &mut totals.incomplete_sessions,
                &mut totals.unavailable_sessions,
            );
            totals.pricing_sources.record(record.pricing_source);
            totals.input_tokens = totals.input_tokens.saturating_add(record.input_tokens);
            totals.cached_input_tokens = totals
                .cached_input_tokens
                .saturating_add(record.cached_input_tokens.min(record.input_tokens));
            totals.output_tokens = totals.output_tokens.saturating_add(record.output_tokens);

            if record.breakdown_reconciled {
                cost_breakdown.input_cost_usd =
                    finite_add(cost_breakdown.input_cost_usd, record.input_cost_usd)
                        .unwrap_or(cost_breakdown.input_cost_usd);
                cost_breakdown.cache_write_cost_usd = finite_add(
                    cost_breakdown.cache_write_cost_usd,
                    record.cache_write_cost_usd,
                )
                .unwrap_or(cost_breakdown.cache_write_cost_usd);
                cost_breakdown.cached_input_cost_usd = finite_add(
                    cost_breakdown.cached_input_cost_usd,
                    record.cached_input_cost_usd,
                )
                .unwrap_or(cost_breakdown.cached_input_cost_usd);
                cost_breakdown.output_cost_usd =
                    finite_add(cost_breakdown.output_cost_usd, record.output_cost_usd)
                        .unwrap_or(cost_breakdown.output_cost_usd);
                cost_breakdown.cached_input_savings_usd = finite_add(
                    cost_breakdown.cached_input_savings_usd,
                    record.cached_input_savings_usd,
                )
                .unwrap_or(cost_breakdown.cached_input_savings_usd);
            }

            let entry = by_model_map
                .entry(record.model_id.clone())
                .or_insert_with(|| ModelMetrics {
                    model_id: record.model_id.clone(),
                    cost_usd: 0.0,
                    known_cost_usd: None,
                    pricing_status: PricingStatus::Unavailable,
                    complete_sessions: 0,
                    incomplete_sessions: 0,
                    unavailable_sessions: 0,
                    pricing_sources: PricingSourceCounts::default(),
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                    cache_hit_ratio: 0.0,
                    session_count: 0,
                });
            if let Some(cost) = known_cost {
                entry.cost_usd = finite_add(entry.cost_usd, cost).unwrap_or(entry.cost_usd);
            }
            record_status(
                pricing_status,
                &mut entry.complete_sessions,
                &mut entry.incomplete_sessions,
                &mut entry.unavailable_sessions,
            );
            entry.pricing_sources.record(record.pricing_source);
            entry.input_tokens = entry.input_tokens.saturating_add(record.input_tokens);
            entry.cached_input_tokens = entry
                .cached_input_tokens
                .saturating_add(record.cached_input_tokens.min(record.input_tokens));
            entry.output_tokens = entry.output_tokens.saturating_add(record.output_tokens);
            entry.session_count = entry.session_count.saturating_add(1);
        }

        totals.known_cost_usd = (totals
            .complete_sessions
            .saturating_add(totals.incomplete_sessions)
            > totals.unavailable_sessions)
            .then_some(totals.cost_usd);
        totals.pricing_status =
            aggregate_pricing_status(totals.known_cost_usd, totals.incomplete_sessions);
        totals.total_tokens = totals.input_tokens.saturating_add(totals.output_tokens);
        totals.cache_hit_ratio = cache_hit_ratio(totals.input_tokens, totals.cached_input_tokens);

        let mut by_model: Vec<ModelMetrics> = by_model_map.into_values().collect();
        for model in &mut by_model {
            model.known_cost_usd = (model
                .complete_sessions
                .saturating_add(model.incomplete_sessions)
                > model.unavailable_sessions)
                .then_some(model.cost_usd);
            model.pricing_status =
                aggregate_pricing_status(model.known_cost_usd, model.incomplete_sessions);
            model.cache_hit_ratio = cache_hit_ratio(model.input_tokens, model.cached_input_tokens);
        }
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

fn cache_hit_ratio(input_tokens: u64, cached_input_tokens: u64) -> f64 {
    if input_tokens == 0 {
        0.0
    } else {
        cached_input_tokens.min(input_tokens) as f64 / input_tokens as f64
    }
}

fn valid_known_cost(value: Option<f64>) -> Option<f64> {
    value.filter(|cost| cost.is_finite() && *cost >= 0.0)
}

fn finite_add(left: f64, right: f64) -> Option<f64> {
    if !left.is_finite() || !right.is_finite() || left < 0.0 || right < 0.0 {
        return None;
    }
    let total = left + right;
    total.is_finite().then_some(total)
}

fn effective_pricing_status(known_cost_usd: Option<f64>, status: PricingStatus) -> PricingStatus {
    if known_cost_usd.is_none() {
        PricingStatus::Unavailable
    } else {
        status
    }
}

fn record_status(
    status: PricingStatus,
    complete_sessions: &mut u32,
    incomplete_sessions: &mut u32,
    unavailable_sessions: &mut u32,
) {
    match status {
        PricingStatus::Exact => {
            *complete_sessions = complete_sessions.saturating_add(1);
        }
        PricingStatus::Partial => {
            *incomplete_sessions = incomplete_sessions.saturating_add(1);
        }
        PricingStatus::Unavailable => {
            *incomplete_sessions = incomplete_sessions.saturating_add(1);
            *unavailable_sessions = unavailable_sessions.saturating_add(1);
        }
    }
}

fn aggregate_pricing_status(
    known_cost_usd: Option<f64>,
    incomplete_sessions: u32,
) -> PricingStatus {
    if known_cost_usd.is_none() {
        PricingStatus::Unavailable
    } else if incomplete_sessions > 0 {
        PricingStatus::Partial
    } else {
        PricingStatus::Exact
    }
}

pub fn format_metrics_cost(totals: &TokenTotals) -> String {
    format_presentable_cost(totals.known_cost_usd, totals.pricing_status)
        .unwrap_or_else(|| "cost unavailable".to_string())
}

impl Default for MetricsTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{CostAttribution, PricingSource, PricingStatus, TokenCostBreakdown};
    use crate::model::SessionSpeed;
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
            originator: None,
            source: None,
            model: Some(model.to_string()),
            reasoning_effort: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: Some(input + output),
            last_turn_tokens: Some(0),
            session_delta_tokens: Some(0),
            input_tokens_total: input,
            cached_input_tokens_total: cached,
            cache_write_tokens_total: None,
            output_tokens_total: output,
            last_input_tokens: Some(0),
            last_cached_input_tokens: Some(0),
            last_output_tokens: Some(0),
            total_cost_usd: cost,
            known_cost_usd: Some(cost),
            cost_breakdown: TokenCostBreakdown {
                input_cost_usd: cost / 2.0,
                cache_write_cost_usd: 0.0,
                cached_input_cost_usd: cost / 4.0,
                output_cost_usd: cost / 4.0,
                cached_input_savings_usd: cost / 8.0,
            },
            pricing_source: PricingSource::Exact,
            pricing_status: PricingStatus::Exact,
            cost_attribution: CostAttribution::SingleModel,
            cost_breakdown_reconciled: true,
            speed: SessionSpeed::default(),
            context_window: None,
            limits: RateLimits::default(),
            rate_limit_envelopes: Vec::new(),
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
        assert!((snapshot.totals.cache_hit_ratio - (500.0 / 3_000.0)).abs() < 0.0001);
    }

    #[test]
    fn aggregate_is_partial_when_any_session_is_incomplete() {
        let mut tracker = MetricsTracker::new();
        let exact = make_session("exact", "gpt-5.5", 1_000, 100, 200, 0.01);
        let mut partial = make_session("partial", "gpt-5.6-sol", 1_000, 100, 200, 0.02);
        partial.pricing_status = PricingStatus::Partial;
        tracker.update(&[exact, partial]);

        let totals = &tracker.snapshot().expect("snapshot").totals;
        assert_eq!(totals.pricing_status, PricingStatus::Partial);
        assert_eq!(totals.known_cost_usd, Some(0.03));
        assert_eq!(totals.complete_sessions, 1);
        assert_eq!(totals.incomplete_sessions, 1);
        assert_eq!(totals.unavailable_sessions, 0);
    }

    #[test]
    fn aggregate_is_unavailable_when_no_session_has_a_known_total() {
        let mut tracker = MetricsTracker::new();
        let mut unavailable = make_session("unknown", "future", 1_000, 0, 200, 0.0);
        unavailable.known_cost_usd = None;
        unavailable.pricing_status = PricingStatus::Unavailable;
        unavailable.pricing_source = PricingSource::Unavailable;
        unavailable.cost_breakdown = TokenCostBreakdown::default();
        unavailable.cost_breakdown_reconciled = false;
        tracker.update(&[unavailable]);

        let totals = &tracker.snapshot().expect("snapshot").totals;
        assert_eq!(totals.pricing_status, PricingStatus::Unavailable);
        assert_eq!(totals.known_cost_usd, None);
        assert_eq!(totals.incomplete_sessions, 1);
        assert_eq!(totals.unavailable_sessions, 1);
        assert_eq!(format_metrics_cost(totals), "cost unavailable");
    }

    #[test]
    fn tracks_cache_hit_ratio_and_savings() {
        let mut tracker = MetricsTracker::new();
        let sessions = vec![
            make_session("s1", "gpt-5.5", 1_000, 400, 200, 0.10),
            make_session("s2", "gpt-5.5", 3_000, 600, 800, 0.30),
        ];

        tracker.update(&sessions);

        let snapshot = tracker.snapshot().expect("snapshot");
        assert_eq!(snapshot.totals.input_tokens, 4_000);
        assert_eq!(snapshot.totals.cached_input_tokens, 1_000);
        assert!((snapshot.totals.cache_hit_ratio - 0.25).abs() < 0.0001);
        assert!((snapshot.cost_breakdown.cached_input_savings_usd - 0.05).abs() < 0.0001);
        assert!((snapshot.by_model[0].cache_hit_ratio - 0.25).abs() < 0.0001);
    }
}
