use std::{collections::BTreeMap, time::SystemTime};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RateLimitScope {
    #[serde(rename = "global")]
    GlobalCodex,
    #[serde(rename = "model")]
    ModelScoped,
    #[serde(rename = "other")]
    #[default]
    Other,
}

impl RateLimitScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlobalCodex => "global",
            Self::ModelScoped => "model",
            Self::Other => "other",
        }
    }

    pub const fn as_slug(self) -> &'static str {
        match self {
            Self::GlobalCodex => "global_codex",
            Self::ModelScoped => "model_scoped",
            Self::Other => "other",
        }
    }

    pub const fn preference(self) -> u8 {
        match self {
            Self::GlobalCodex => 3,
            Self::ModelScoped => 2,
            Self::Other => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UsageWindow {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_minutes: u64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RateLimits {
    pub primary: Option<UsageWindow>,
    pub secondary: Option<UsageWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct CreditBalance {
    pub balance: Option<String>,
    pub has_credits: bool,
    pub unlimited: bool,
}

impl CreditBalance {
    pub fn display_value(&self) -> Option<&str> {
        if self.unlimited {
            Some("Unlimited")
        } else {
            self.balance.as_deref()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RateLimitEnvelope {
    pub limit_id: Option<String>,
    pub limit_name: Option<String>,
    pub plan_type: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
    pub scope: RateLimitScope,
    pub limits: RateLimits,
    pub credits: Option<CreditBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaWindow {
    pub window_minutes: u64,
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

impl From<&UsageWindow> for QuotaWindow {
    fn from(value: &UsageWindow) -> Self {
        Self {
            window_minutes: value.window_minutes,
            used_percent: value.used_percent,
            remaining_percent: value.remaining_percent,
            resets_at: value.resets_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuotaScope {
    pub id: Option<String>,
    pub name: Option<String>,
    pub kind: RateLimitScope,
    pub windows: Vec<QuotaWindow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageSnapshot {
    pub provider: String,
    pub scopes: Vec<QuotaScope>,
    pub credits: Option<CreditBalance>,
    pub observed_at: Option<DateTime<Utc>>,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct SessionLimitCandidate {
    pub session_id: String,
    pub session_last_activity: SystemTime,
    pub envelope: RateLimitEnvelope,
}

#[derive(Debug, Clone)]
pub struct EffectiveLimitSelection {
    pub source_session_id: String,
    pub source_limit_id: Option<String>,
    pub source_scope: RateLimitScope,
    pub observed_at: Option<DateTime<Utc>>,
    pub limits: RateLimits,
    pub credits: Option<CreditBalance>,
}

impl EffectiveLimitSelection {
    pub fn source_label(&self) -> String {
        match self.source_scope {
            RateLimitScope::GlobalCodex => "Global account quota (/codex)".to_string(),
            RateLimitScope::ModelScoped => format!(
                "Model-specific quota ({})",
                self.source_limit_id.as_deref().unwrap_or("unknown")
            ),
            RateLimitScope::Other => format!(
                "Quota stream ({})",
                self.source_limit_id.as_deref().unwrap_or("unknown")
            ),
        }
    }
}

pub fn limits_present(limits: &RateLimits) -> bool {
    limits.primary.is_some() || limits.secondary.is_some()
}

pub fn classify_limit_scope(limit_id: Option<&str>) -> RateLimitScope {
    let normalized = limit_id
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if normalized == "codex" {
        RateLimitScope::GlobalCodex
    } else if normalized.starts_with("codex_") {
        RateLimitScope::ModelScoped
    } else {
        RateLimitScope::Other
    }
}

pub fn parse_rate_limit_envelope(
    value: Option<&Value>,
    observed_at: Option<DateTime<Utc>>,
) -> Option<RateLimitEnvelope> {
    let value = value?;
    let limits = RateLimits {
        primary: parse_usage_window(value.get("primary")),
        secondary: parse_usage_window(value.get("secondary")),
    };
    let credits = parse_credits(value.get("credits"));
    if !limits_present(&limits) && credits.is_none() {
        return None;
    }

    let limit_id = str_at(value, &["limit_id"]);
    Some(RateLimitEnvelope {
        scope: classify_limit_scope(limit_id.as_deref()),
        limit_id,
        limit_name: str_at(value, &["limit_name"]),
        plan_type: str_at(value, &["plan_type"]),
        observed_at,
        limits,
        credits,
    })
}

pub fn usage_snapshot_from_envelopes(
    provider: impl Into<String>,
    source: impl Into<String>,
    envelopes: &[RateLimitEnvelope],
) -> UsageSnapshot {
    let mut latest_by_scope: BTreeMap<(u8, String, String), &RateLimitEnvelope> = BTreeMap::new();
    for envelope in envelopes {
        let key = (
            envelope.scope.preference(),
            envelope.limit_id.clone().unwrap_or_default(),
            envelope.limit_name.clone().unwrap_or_default(),
        );
        let should_replace = latest_by_scope
            .get(&key)
            .is_none_or(|current| envelope.observed_at >= current.observed_at);
        if should_replace {
            latest_by_scope.insert(key, envelope);
        }
    }
    let mut ordered: Vec<&RateLimitEnvelope> = latest_by_scope.into_values().collect();
    ordered.sort_by_key(|envelope| {
        (
            std::cmp::Reverse(envelope.scope.preference()),
            envelope.limit_name.clone().unwrap_or_default(),
            envelope.limit_id.clone().unwrap_or_default(),
        )
    });
    let scopes = ordered
        .iter()
        .filter(|envelope| limits_present(&envelope.limits))
        .map(|envelope| {
            let windows = [&envelope.limits.primary, &envelope.limits.secondary]
                .into_iter()
                .filter_map(|window| window.as_ref().map(QuotaWindow::from))
                .collect();
            QuotaScope {
                id: envelope.limit_id.clone(),
                name: envelope.limit_name.clone(),
                kind: envelope.scope,
                windows,
            }
        })
        .collect();
    let newest = envelopes.iter().max_by_key(|envelope| envelope.observed_at);
    let credits = envelopes
        .iter()
        .filter_map(|envelope| envelope.credits.as_ref().map(|credits| (envelope, credits)))
        .max_by_key(|(envelope, _)| envelope.observed_at)
        .map(|(_, credits)| credits.clone());
    UsageSnapshot {
        provider: provider.into(),
        scopes,
        credits,
        observed_at: newest.and_then(|envelope| envelope.observed_at),
        source: source.into(),
    }
}

pub fn select_session_envelope_global_first(
    envelopes: &[RateLimitEnvelope],
) -> Option<RateLimitEnvelope> {
    let global = envelopes
        .iter()
        .filter(|item| item.scope == RateLimitScope::GlobalCodex)
        .filter(|item| limits_present(&item.limits) || item.credits.is_some())
        .max_by_key(|item| envelope_rank_key(item));
    global.cloned().or_else(|| {
        envelopes
            .iter()
            .filter(|item| limits_present(&item.limits) || item.credits.is_some())
            .max_by_key(|item| envelope_rank_key(item))
            .cloned()
    })
}

pub fn select_credits_global_first(envelopes: &[RateLimitEnvelope]) -> Option<CreditBalance> {
    let has_global = envelopes
        .iter()
        .any(|item| item.scope == RateLimitScope::GlobalCodex && item.credits.is_some());
    envelopes
        .iter()
        .filter(|item| item.credits.is_some())
        .filter(|item| !has_global || item.scope == RateLimitScope::GlobalCodex)
        .max_by_key(|item| envelope_rank_key(item))
        .and_then(|item| item.credits.clone())
}

pub fn select_effective_limits_global_first(
    candidates: &[SessionLimitCandidate],
) -> Option<EffectiveLimitSelection> {
    let has_global = candidates.iter().any(|item| {
        item.envelope.scope == RateLimitScope::GlobalCodex
            && (limits_present(&item.envelope.limits) || item.envelope.credits.is_some())
    });
    let selected = candidates
        .iter()
        .filter(|item| !has_global || item.envelope.scope == RateLimitScope::GlobalCodex)
        .max_by_key(|item| {
            (
                envelope_rank_key(&item.envelope),
                system_time_rank(item.session_last_activity),
            )
        })?;
    let has_global_credits = candidates.iter().any(|item| {
        item.envelope.scope == RateLimitScope::GlobalCodex && item.envelope.credits.is_some()
    });
    let credits = candidates
        .iter()
        .filter(|item| item.envelope.credits.is_some())
        .filter(|item| !has_global_credits || item.envelope.scope == RateLimitScope::GlobalCodex)
        .max_by_key(|item| {
            (
                envelope_rank_key(&item.envelope),
                system_time_rank(item.session_last_activity),
            )
        })
        .and_then(|item| item.envelope.credits.clone());
    Some(EffectiveLimitSelection {
        source_session_id: selected.session_id.clone(),
        source_limit_id: selected.envelope.limit_id.clone(),
        source_scope: selected.envelope.scope,
        observed_at: selected.envelope.observed_at,
        limits: selected.envelope.limits.clone(),
        credits,
    })
}

pub fn format_window_label(minutes: u64) -> String {
    match minutes {
        300 => "5h".to_string(),
        1_440 => "24h".to_string(),
        10_080 => "7d".to_string(),
        value if value > 0 && value % 1_440 == 0 => format!("{}d", value / 1_440),
        value if value > 0 && value % 60 == 0 => format!("{}h", value / 60),
        value => format!("{value}m"),
    }
}

fn parse_usage_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?.as_object()?;
    let window_minutes = value.get("window_minutes").and_then(uint_value)?;
    if window_minutes == 0 {
        return None;
    }
    let used_percent = clamp_percent(value.get("used_percent").and_then(number_at).unwrap_or(0.0));
    Some(UsageWindow {
        used_percent,
        remaining_percent: clamp_percent(100.0 - used_percent),
        window_minutes,
        resets_at: value
            .get("resets_at")
            .and_then(int_value)
            .and_then(|epoch| Utc.timestamp_opt(epoch, 0).single()),
    })
}

fn parse_credits(value: Option<&Value>) -> Option<CreditBalance> {
    let value = value?.as_object()?;
    let balance = value.get("balance").and_then(decimal_text);
    let explicit_has_credits = value.get("has_credits").and_then(Value::as_bool);
    let explicit_unlimited = value.get("unlimited").and_then(Value::as_bool);
    if balance.is_none() && explicit_has_credits.is_none() && explicit_unlimited.is_none() {
        return None;
    }
    let has_credits =
        explicit_has_credits.unwrap_or_else(|| balance.as_deref().is_some_and(|item| item != "0"));
    let unlimited = explicit_unlimited.unwrap_or(false);
    Some(CreditBalance {
        balance,
        has_credits,
        unlimited,
    })
}

fn decimal_text(value: &Value) -> Option<String> {
    match value {
        Value::String(item) => {
            let item = item.trim();
            (!item.is_empty()).then(|| item.to_string())
        }
        Value::Number(item) => Some(item.to_string()),
        _ => None,
    }
}

fn clamp_percent(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn str_at(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    cursor
        .as_str()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn number_at(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_u64().map(|item| item as f64))
}

fn uint_value(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value
            .as_i64()
            .and_then(|item| (item >= 0).then_some(item as u64))
    })
}

fn int_value(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|item| i64::try_from(item).ok()))
}

fn envelope_rank_key(envelope: &RateLimitEnvelope) -> (i64, u8, String, String) {
    (
        envelope
            .observed_at
            .map(|ts| ts.timestamp_millis())
            .unwrap_or(i64::MIN),
        envelope.scope.preference(),
        envelope.limit_id.clone().unwrap_or_default(),
        envelope.plan_type.clone().unwrap_or_default(),
    )
}

fn system_time_rank(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_only_is_semantic_and_keeps_credits() {
        let payload = serde_json::json!({
            "limit_id": "codex",
            "primary": {"used_percent": 4.0, "window_minutes": 10080, "resets_at": 1784780201},
            "secondary": null,
            "credits": {"has_credits": true, "unlimited": false, "balance": "2500"}
        });
        let envelope = parse_rate_limit_envelope(Some(&payload), None).expect("envelope");
        assert_eq!(
            format_window_label(envelope.limits.primary.as_ref().unwrap().window_minutes),
            "7d"
        );
        assert_eq!(envelope.credits.unwrap().balance.as_deref(), Some("2500"));
    }

    #[test]
    fn snapshot_preserves_global_and_model_scopes() {
        let global = parse_rate_limit_envelope(
            Some(&serde_json::json!({
                "limit_id":"codex", "primary":{"used_percent":4,"window_minutes":10080}
            })),
            None,
        )
        .unwrap();
        let spark = parse_rate_limit_envelope(
            Some(&serde_json::json!({
                "limit_id":"codex_bengalfox", "limit_name":"GPT-5.3-Codex-Spark",
                "primary":{"used_percent":0,"window_minutes":10080}
            })),
            None,
        )
        .unwrap();
        let snapshot = usage_snapshot_from_envelopes("codex", "fixture", &[spark, global]);
        assert_eq!(snapshot.scopes.len(), 2);
        assert_eq!(snapshot.scopes[0].kind, RateLimitScope::GlobalCodex);
        let wire = serde_json::to_value(&snapshot).expect("serialize snapshot");
        assert_eq!(wire["scopes"][0]["kind"], "global");
        assert_eq!(wire["scopes"][1]["kind"], "model");
    }

    #[test]
    fn quota_scope_wire_values_are_exact() {
        for (scope, expected) in [
            (RateLimitScope::GlobalCodex, "global"),
            (RateLimitScope::ModelScoped, "model"),
            (RateLimitScope::Other, "other"),
        ] {
            assert_eq!(serde_json::to_value(scope).expect("serialize"), expected);
            assert_eq!(
                serde_json::from_value::<RateLimitScope>(serde_json::json!(expected))
                    .expect("deserialize"),
                scope
            );
        }
    }

    #[test]
    fn credits_accept_zero_unlimited_and_numbers() {
        for (raw, expected) in [
            (
                serde_json::json!({"credits":{"has_credits":false,"balance":"0"}}),
                "0",
            ),
            (
                serde_json::json!({"credits":{"has_credits":true,"balance":12.5}}),
                "12.5",
            ),
        ] {
            let parsed = parse_rate_limit_envelope(Some(&raw), None).unwrap();
            assert_eq!(parsed.credits.unwrap().balance.as_deref(), Some(expected));
        }
        let raw = serde_json::json!({"credits":{"unlimited":true}});
        assert_eq!(
            parse_rate_limit_envelope(Some(&raw), None)
                .unwrap()
                .credits
                .unwrap()
                .display_value(),
            Some("Unlimited")
        );
    }

    #[test]
    fn malformed_or_absent_credits_are_unavailable() {
        for raw in [
            serde_json::json!({}),
            serde_json::json!({"credits": null}),
            serde_json::json!({"credits": {}}),
            serde_json::json!({"credits": {"balance": []}}),
            serde_json::json!({"credits": {"balance": ""}}),
            serde_json::json!({"credits": {"has_credits": "true"}}),
            serde_json::json!({"credits": {"unlimited": 1}}),
        ] {
            assert!(parse_rate_limit_envelope(Some(&raw), None).is_none());
        }
    }

    #[test]
    fn newest_quota_does_not_hide_older_global_credits() {
        let older = RateLimitEnvelope {
            limit_id: Some("codex".to_string()),
            observed_at: Utc.timestamp_opt(100, 0).single(),
            scope: RateLimitScope::GlobalCodex,
            credits: Some(CreditBalance {
                balance: Some("2500".to_string()),
                has_credits: true,
                unlimited: false,
            }),
            ..RateLimitEnvelope::default()
        };
        let newer = RateLimitEnvelope {
            limit_id: Some("codex".to_string()),
            observed_at: Utc.timestamp_opt(200, 0).single(),
            scope: RateLimitScope::GlobalCodex,
            limits: RateLimits {
                primary: Some(UsageWindow {
                    window_minutes: 10_080,
                    ..UsageWindow::default()
                }),
                secondary: None,
            },
            ..RateLimitEnvelope::default()
        };
        let candidates = [older, newer]
            .into_iter()
            .enumerate()
            .map(|(index, envelope)| SessionLimitCandidate {
                session_id: index.to_string(),
                session_last_activity: SystemTime::UNIX_EPOCH,
                envelope,
            })
            .collect::<Vec<_>>();
        let selected = select_effective_limits_global_first(&candidates).expect("selection");
        assert_eq!(
            selected.credits.and_then(|value| value.balance).as_deref(),
            Some("2500")
        );
    }
}
