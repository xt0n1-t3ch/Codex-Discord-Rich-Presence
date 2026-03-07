use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config;
use crate::config::{OpenAiPlanDisplayConfig, OpenAiPlanMode, OpenAiPlanTier};
use crate::session::CodexSessionSnapshot;
use crate::telemetry::limits::RateLimitScope;
use crate::util::write_json_pretty_atomic;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DetectedPlanTier {
    Free,
    Go,
    Plus,
    Business,
    Enterprise,
    Pro,
    #[default]
    Unknown,
}

impl DetectedPlanTier {
    pub fn title(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Go => "Go",
            Self::Plus => "Plus",
            Self::Business => "Business",
            Self::Enterprise => "Enterprise",
            Self::Pro => "Pro",
            Self::Unknown => "Unknown",
        }
    }

    pub fn monthly_price_usd(self) -> Option<u32> {
        match self {
            Self::Free => Some(0),
            Self::Go => Some(8),
            Self::Plus => Some(20),
            Self::Pro => Some(200),
            Self::Business | Self::Enterprise | Self::Unknown => None,
        }
    }

    pub fn label(self, show_price: bool) -> String {
        if show_price && let Some(monthly) = self.monthly_price_usd() {
            return format!("{} (${monthly}/month)", self.title());
        }
        self.title().to_string()
    }
}

impl From<OpenAiPlanTier> for DetectedPlanTier {
    fn from(value: OpenAiPlanTier) -> Self {
        match value {
            OpenAiPlanTier::Free => Self::Free,
            OpenAiPlanTier::Go => Self::Go,
            OpenAiPlanTier::Plus => Self::Plus,
            OpenAiPlanTier::Business => Self::Business,
            OpenAiPlanTier::Enterprise => Self::Enterprise,
            OpenAiPlanTier::Pro => Self::Pro,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DetectedPlanSource {
    Manual,
    Telemetry,
    Memory,
    Cache,
    #[default]
    Unknown,
}

impl DetectedPlanSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Telemetry => "telemetry",
            Self::Memory => "memory",
            Self::Cache => "cache",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedPlan {
    pub tier: DetectedPlanTier,
    pub source: DetectedPlanSource,
    pub observed_at: Option<DateTime<Utc>>,
    pub raw_plan_type: Option<String>,
}

impl Default for ResolvedPlan {
    fn default() -> Self {
        Self {
            tier: DetectedPlanTier::Unknown,
            source: DetectedPlanSource::Unknown,
            observed_at: None,
            raw_plan_type: None,
        }
    }
}

impl ResolvedPlan {
    pub fn label(&self, show_price: bool) -> String {
        self.tier.label(show_price)
    }

    pub fn status_label(&self) -> String {
        match self.source {
            DetectedPlanSource::Manual => format!("{} (manual)", self.tier.title()),
            DetectedPlanSource::Telemetry => format!("{} (auto-detected)", self.tier.title()),
            DetectedPlanSource::Memory => format!("{} (remembered)", self.tier.title()),
            DetectedPlanSource::Cache => format!("{} (cached)", self.tier.title()),
            DetectedPlanSource::Unknown => self.tier.title().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlanCacheFile {
    tier: DetectedPlanTier,
    source: DetectedPlanSource,
    observed_at: Option<DateTime<Utc>>,
    raw_plan_type: Option<String>,
}

#[derive(Debug)]
pub struct PlanDetector {
    last_telemetry: Option<ResolvedPlan>,
    cached: Option<ResolvedPlan>,
}

impl Default for PlanDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanDetector {
    pub fn new() -> Self {
        let cached = load_plan_cache();
        Self {
            last_telemetry: None,
            cached,
        }
    }

    pub fn resolve_from_sessions(
        &mut self,
        sessions: &[CodexSessionSnapshot],
        plan_config: &OpenAiPlanDisplayConfig,
    ) -> ResolvedPlan {
        let auto_resolved = self.resolve_auto_from_sessions(sessions);

        if matches!(plan_config.mode, OpenAiPlanMode::Manual) {
            let raw_plan_type = plan_config.tier.title().to_ascii_lowercase();
            return ResolvedPlan {
                tier: plan_config.tier.into(),
                source: DetectedPlanSource::Manual,
                observed_at: None,
                raw_plan_type: Some(raw_plan_type),
            };
        }

        auto_resolved
    }

    fn resolve_auto_from_sessions(&mut self, sessions: &[CodexSessionSnapshot]) -> ResolvedPlan {
        if let Some(signal) = select_plan_signal(sessions) {
            let resolved = ResolvedPlan {
                tier: parse_plan_type(signal.raw_plan_type.as_deref()),
                source: DetectedPlanSource::Telemetry,
                observed_at: signal.observed_at,
                raw_plan_type: signal.raw_plan_type,
            };
            self.last_telemetry = Some(resolved.clone());
            let _ = save_plan_cache(&resolved);
            return resolved;
        }

        if let Some(previous) = &self.last_telemetry {
            let mut memory = previous.clone();
            memory.source = DetectedPlanSource::Memory;
            return memory;
        }
        if let Some(cached) = &self.cached {
            let mut resolved = cached.clone();
            resolved.source = DetectedPlanSource::Cache;
            return resolved;
        }

        ResolvedPlan::default()
    }
}

pub fn parse_plan_type(raw: Option<&str>) -> DetectedPlanTier {
    let normalized = raw
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match normalized.as_str() {
        "free" => DetectedPlanTier::Free,
        "go" => DetectedPlanTier::Go,
        "plus" => DetectedPlanTier::Plus,
        "business" => DetectedPlanTier::Business,
        "enterprise" => DetectedPlanTier::Enterprise,
        "pro" => DetectedPlanTier::Pro,
        _ => DetectedPlanTier::Unknown,
    }
}

pub fn is_spark_model(model_id: &str) -> bool {
    let normalized = model_id.trim().to_ascii_lowercase();
    normalized == "gpt-5.3-codex-spark"
        || normalized == "gpt-5.3-codex-spark-latest"
        || normalized.contains("codex-spark")
}

pub fn is_model_allowed_for_plan(model_id: &str, tier: DetectedPlanTier) -> bool {
    if !is_spark_model(model_id) {
        return true;
    }
    tier == DetectedPlanTier::Pro
}

fn plan_cache_path() -> PathBuf {
    config::codex_home().join("discord-presence-plan-cache.json")
}

fn load_plan_cache() -> Option<ResolvedPlan> {
    load_plan_cache_from_path(&plan_cache_path())
}

fn load_plan_cache_from_path(path: &Path) -> Option<ResolvedPlan> {
    let raw = fs::read_to_string(path).ok()?;
    let parsed: PlanCacheFile = serde_json::from_str(&raw).ok()?;
    Some(ResolvedPlan {
        tier: parsed.tier,
        source: parsed.source,
        observed_at: parsed.observed_at,
        raw_plan_type: parsed.raw_plan_type,
    })
}

fn save_plan_cache(plan: &ResolvedPlan) -> std::io::Result<()> {
    save_plan_cache_to_path(plan, &plan_cache_path())
}

fn save_plan_cache_to_path(plan: &ResolvedPlan, path: &Path) -> std::io::Result<()> {
    let payload = PlanCacheFile {
        tier: plan.tier,
        source: plan.source,
        observed_at: plan.observed_at,
        raw_plan_type: plan.raw_plan_type.clone(),
    };
    write_json_pretty_atomic(path, &payload)
}

#[derive(Debug)]
struct PlanSignal {
    raw_plan_type: Option<String>,
    observed_at: Option<DateTime<Utc>>,
    scope_priority: u8,
    session_last_activity: i64,
}

fn select_plan_signal(sessions: &[CodexSessionSnapshot]) -> Option<PlanSignal> {
    let mut global_candidates: Vec<PlanSignal> = Vec::new();
    let mut fallback_candidates: Vec<PlanSignal> = Vec::new();

    for session in sessions {
        let session_last_activity = session
            .last_activity
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_secs()).ok())
            .unwrap_or(i64::MIN);
        for envelope in &session.rate_limit_envelopes {
            let Some(raw_plan_type) = envelope
                .plan_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
            else {
                continue;
            };

            let signal = PlanSignal {
                raw_plan_type: Some(raw_plan_type),
                observed_at: envelope.observed_at,
                scope_priority: envelope.scope.preference(),
                session_last_activity,
            };
            if envelope.scope == RateLimitScope::GlobalCodex {
                global_candidates.push(signal);
            } else {
                fallback_candidates.push(signal);
            }
        }
    }

    let pool = if !global_candidates.is_empty() {
        global_candidates
    } else {
        fallback_candidates
    };

    pool.into_iter().max_by_key(|item| {
        (
            item.observed_at
                .map(|value| value.timestamp_millis())
                .unwrap_or(i64::MIN),
            item.scope_priority,
            item.session_last_activity,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{PricingSource, TokenCostBreakdown};
    use crate::session::{CodexSessionSnapshot, ContextWindowSnapshot, ContextWindowSource};
    use crate::telemetry::limits::{RateLimitEnvelope, RateLimitScope, RateLimits, UsageWindow};
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn sample_session(plan_type: Option<&str>, scope: RateLimitScope) -> CodexSessionSnapshot {
        CodexSessionSnapshot {
            session_id: "s1".to_string(),
            cwd: PathBuf::from("."),
            project_name: "p1".to_string(),
            git_branch: None,
            originator: None,
            source: None,
            model: Some("gpt-5.3-codex".to_string()),
            reasoning_effort: None,
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: Some(1),
            last_turn_tokens: Some(1),
            session_delta_tokens: Some(1),
            input_tokens_total: 1,
            cached_input_tokens_total: 0,
            output_tokens_total: 0,
            last_input_tokens: Some(1),
            last_cached_input_tokens: Some(0),
            last_output_tokens: Some(0),
            total_cost_usd: 0.0,
            cost_breakdown: TokenCostBreakdown::default(),
            pricing_source: PricingSource::Exact,
            context_window: Some(ContextWindowSnapshot {
                window_tokens: 100,
                used_tokens: 1,
                remaining_tokens: 99,
                remaining_percent: 99.0,
                source: ContextWindowSource::Event,
            }),
            limits: RateLimits::default(),
            rate_limit_envelopes: vec![RateLimitEnvelope {
                limit_id: Some(
                    match scope {
                        RateLimitScope::GlobalCodex => "codex",
                        RateLimitScope::ModelScoped => "codex_bengalfox",
                        RateLimitScope::Other => "other_limit",
                    }
                    .to_string(),
                ),
                limit_name: None,
                plan_type: plan_type.map(ToString::to_string),
                observed_at: Some(Utc::now()),
                scope,
                limits: RateLimits {
                    primary: Some(UsageWindow {
                        used_percent: 1.0,
                        remaining_percent: 99.0,
                        window_minutes: 300,
                        resets_at: None,
                    }),
                    secondary: None,
                },
            }],
            activity: None,
            started_at: None,
            last_token_event_at: None,
            last_activity: SystemTime::now(),
            source_file: PathBuf::from("s.jsonl"),
        }
    }

    #[test]
    fn parse_plan_type_maps_supported_tiers() {
        assert_eq!(parse_plan_type(Some("free")), DetectedPlanTier::Free);
        assert_eq!(parse_plan_type(Some("go")), DetectedPlanTier::Go);
        assert_eq!(parse_plan_type(Some("plus")), DetectedPlanTier::Plus);
        assert_eq!(
            parse_plan_type(Some("business")),
            DetectedPlanTier::Business
        );
        assert_eq!(
            parse_plan_type(Some("enterprise")),
            DetectedPlanTier::Enterprise
        );
        assert_eq!(parse_plan_type(Some("pro")), DetectedPlanTier::Pro);
        assert_eq!(
            parse_plan_type(Some("unexpected")),
            DetectedPlanTier::Unknown
        );
    }

    #[test]
    fn detector_prefers_global_signal() {
        let mut detector = PlanDetector::new();
        let sessions = vec![
            sample_session(Some("plus"), RateLimitScope::ModelScoped),
            sample_session(Some("pro"), RateLimitScope::GlobalCodex),
        ];
        let resolved =
            detector.resolve_from_sessions(&sessions, &OpenAiPlanDisplayConfig::default());
        assert_eq!(resolved.tier, DetectedPlanTier::Pro);
        assert_eq!(resolved.source, DetectedPlanSource::Telemetry);
    }

    #[test]
    fn detector_respects_manual_override() {
        let mut detector = PlanDetector::new();
        let sessions = vec![sample_session(Some("free"), RateLimitScope::GlobalCodex)];
        let resolved = detector.resolve_from_sessions(
            &sessions,
            &OpenAiPlanDisplayConfig {
                mode: OpenAiPlanMode::Manual,
                tier: OpenAiPlanTier::Plus,
                show_price: false,
            },
        );
        assert_eq!(resolved.tier, DetectedPlanTier::Plus);
        assert_eq!(resolved.source, DetectedPlanSource::Manual);
        assert_eq!(resolved.status_label(), "Plus (manual)");
    }

    #[test]
    fn spark_is_pro_only() {
        assert!(is_model_allowed_for_plan(
            "gpt-5.3-codex-spark",
            DetectedPlanTier::Pro
        ));
        assert!(!is_model_allowed_for_plan(
            "gpt-5.3-codex-spark",
            DetectedPlanTier::Plus
        ));
        assert!(is_model_allowed_for_plan(
            "gpt-5.3-codex",
            DetectedPlanTier::Plus
        ));
    }

    #[test]
    fn plan_cache_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("plan-cache.json");
        let payload = ResolvedPlan {
            tier: DetectedPlanTier::Pro,
            source: DetectedPlanSource::Telemetry,
            observed_at: Utc.timestamp_opt(10, 0).single(),
            raw_plan_type: Some("pro".to_string()),
        };

        save_plan_cache_to_path(&payload, &path).expect("save");
        let loaded = load_plan_cache_from_path(&path).expect("load");
        assert_eq!(loaded.tier, DetectedPlanTier::Pro);
        assert_eq!(loaded.raw_plan_type.as_deref(), Some("pro"));
    }
}
