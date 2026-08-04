use std::{collections::BTreeMap, time::SystemTime};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ACCOUNT_RATE_LIMITS_METHOD: &str = "account/rateLimits/read";

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum UsageLane {
    CodexSubscription,
    OpenAiApi,
    ClaudeSubscription,
    AnthropicApi,
    #[default]
    Unknown,
}

impl UsageLane {
    pub const fn is_known(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum UsageSignal {
    CodexSessionJsonl,
    CodexSubscriptionUsage,
    OpenAiApiUsage,
    OpenAiApiRateLimit,
    ClaudeSubscriptionUsage,
    ClaudeSubscriptionOAuth,
    AnthropicApiUsage,
    AnthropicApiRateLimit,
}

impl UsageSignal {
    const fn lane(self) -> Option<UsageLane> {
        match self {
            Self::CodexSessionJsonl => None,
            Self::CodexSubscriptionUsage => Some(UsageLane::CodexSubscription),
            Self::OpenAiApiUsage | Self::OpenAiApiRateLimit => Some(UsageLane::OpenAiApi),
            Self::ClaudeSubscriptionUsage | Self::ClaudeSubscriptionOAuth => {
                Some(UsageLane::ClaudeSubscription)
            }
            Self::AnthropicApiUsage | Self::AnthropicApiRateLimit => Some(UsageLane::AnthropicApi),
        }
    }
}

pub fn classify_usage_signals(signals: &[UsageSignal]) -> UsageLane {
    let mut lane = None;
    for signal in signals.iter().copied().filter_map(UsageSignal::lane) {
        match lane {
            None => lane = Some(signal),
            Some(current) if current == signal => {}
            Some(_) => return UsageLane::Unknown,
        }
    }
    lane.unwrap_or(UsageLane::Unknown)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct UsageSource {
    pub lane: UsageLane,
    pub stream_id: String,
    pub signals: Vec<UsageSignal>,
}

impl UsageSource {
    pub fn new(
        stream_id: impl Into<String>,
        signals: impl IntoIterator<Item = UsageSignal>,
    ) -> Self {
        let mut signals = signals.into_iter().collect::<Vec<_>>();
        signals.sort_unstable();
        signals.dedup();
        let lane = classify_usage_signals(&signals);
        Self {
            lane,
            stream_id: stream_id.into(),
            signals,
        }
    }

    pub fn is_selectable(&self) -> bool {
        self.lane.is_known()
            && classify_usage_signals(&self.signals) == self.lane
            && !self.stream_id.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitScope {
    #[serde(alias = "global")]
    GlobalAccount,
    IndividualAccount,
    #[serde(rename = "model")]
    ModelScoped,
    #[serde(rename = "other")]
    #[default]
    Other,
}

impl RateLimitScope {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GlobalAccount => "global account",
            Self::IndividualAccount => "individual account",
            Self::ModelScoped => "model",
            Self::Other => "other",
        }
    }

    pub const fn as_slug(self) -> &'static str {
        match self {
            Self::GlobalAccount => "global_account",
            Self::IndividualAccount => "individual_account",
            Self::ModelScoped => "model_scoped",
            Self::Other => "other",
        }
    }

    pub const fn preference(self) -> u8 {
        match self {
            Self::GlobalAccount => 4,
            Self::IndividualAccount => 3,
            Self::ModelScoped => 2,
            Self::Other => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct UsageWindow {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub window_minutes: u64,
    pub resets_at: Option<DateTime<Utc>>,
}

impl<'de> Deserialize<'de> for UsageWindow {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            used_percent: Option<f64>,
            remaining_percent: Option<f64>,
            window_minutes: Option<u64>,
            resets_at: Option<DateTime<Utc>>,
        }

        let wire = Wire::deserialize(deserializer)?;
        let window_minutes = wire
            .window_minutes
            .filter(|minutes| *minutes > 0)
            .ok_or_else(|| serde::de::Error::custom("usage window requires a positive duration"))?;
        let used_percent = wire
            .used_percent
            .or_else(|| wire.remaining_percent.map(|remaining| 100.0 - remaining))
            .ok_or_else(|| serde::de::Error::custom("usage window requires a percentage"))?;
        if !used_percent.is_finite()
            || wire
                .remaining_percent
                .is_some_and(|remaining| !remaining.is_finite())
        {
            return Err(serde::de::Error::custom(
                "usage window percentages must be finite",
            ));
        }
        let remaining_percent = wire.remaining_percent.unwrap_or(100.0 - used_percent);
        Ok(Self {
            used_percent: clamp_percent(used_percent),
            remaining_percent: clamp_percent(remaining_percent),
            window_minutes,
            resets_at: wire.resets_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct RateLimits {
    #[serde(default)]
    pub windows: Vec<UsageWindow>,
}

impl RateLimits {
    pub fn new(windows: Vec<UsageWindow>) -> Self {
        Self { windows }
    }

    pub fn from_primary_secondary(
        primary: Option<UsageWindow>,
        secondary: Option<UsageWindow>,
    ) -> Self {
        Self {
            windows: primary.into_iter().chain(secondary).collect(),
        }
    }

    pub fn primary(&self) -> Option<&UsageWindow> {
        self.windows.first()
    }

    pub fn secondary(&self) -> Option<&UsageWindow> {
        self.windows.get(1)
    }

    pub fn into_primary_secondary(self) -> (Option<UsageWindow>, Option<UsageWindow>) {
        let mut windows = self.windows.into_iter();
        (windows.next(), windows.next())
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCreditsSummary {
    pub available_count: u64,
    pub credits: Option<Vec<RateLimitResetCredit>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitResetCredit {
    pub id: String,
    pub reset_type: String,
    pub status: String,
    pub granted_at: i64,
    pub expires_at: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountRateLimitsRead {
    pub envelopes: Vec<RateLimitEnvelope>,
    pub rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
    pub observed_at: DateTime<Utc>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UsageStream {
    pub source: UsageSource,
    pub envelopes: Vec<RateLimitEnvelope>,
}

impl UsageStream {
    pub fn new(source: UsageSource, envelopes: Vec<RateLimitEnvelope>) -> Self {
        Self { source, envelopes }
    }
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
    pub source: UsageSource,
    pub scopes: Vec<QuotaScope>,
    pub credits: Option<CreditBalance>,
    pub observed_at: Option<DateTime<Utc>>,
    pub provenance_source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UsageSnapshotCollection {
    pub snapshots: Vec<UsageSnapshot>,
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
            RateLimitScope::GlobalAccount => "Global account quota (/codex)".to_string(),
            RateLimitScope::IndividualAccount => "Individual account quota".to_string(),
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
    !limits.windows.is_empty()
}

pub fn classify_limit_scope(limit_id: Option<&str>) -> RateLimitScope {
    let normalized = limit_id
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if matches!(normalized.as_str(), "codex" | "global" | "global_account") {
        RateLimitScope::GlobalAccount
    } else if matches!(
        normalized.as_str(),
        "account" | "individual" | "individual_account"
    ) {
        RateLimitScope::IndividualAccount
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
    let windows = value
        .get("windows")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| parse_usage_window(Some(item)))
                .collect()
        })
        .unwrap_or_else(|| {
            ["primary", "secondary"]
                .into_iter()
                .filter_map(|key| parse_usage_window(value.get(key)))
                .collect()
        });
    let limits = RateLimits::new(windows);
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

#[derive(Debug, Deserialize)]
struct AccountRateLimitsRpcWire {
    result: Option<AccountRateLimitsResultWire>,
    error: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountRateLimitsResultWire {
    rate_limits: Option<WireRateLimitSnapshot>,
    rate_limits_by_limit_id: Option<BTreeMap<String, WireRateLimitSnapshot>>,
    rate_limit_reset_credits: Option<RateLimitResetCreditsSummary>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireRateLimitSnapshot {
    limit_id: Option<String>,
    limit_name: Option<String>,
    plan_type: Option<String>,
    primary: Option<WireUsageWindow>,
    secondary: Option<WireUsageWindow>,
    credits: Option<WireCredits>,
    individual_limit: Option<WireSpendControlLimit>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireUsageWindow {
    used_percent: Option<f64>,
    remaining_percent: Option<f64>,
    window_duration_mins: Option<u64>,
    resets_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCredits {
    balance: Option<Value>,
    has_credits: Option<bool>,
    unlimited: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireSpendControlLimit {
    limit: Option<String>,
    used: Option<String>,
    remaining_percent: Option<f64>,
    resets_at: Option<i64>,
}

pub fn parse_account_rate_limits_response(
    response: &str,
    observed_at: DateTime<Utc>,
) -> Result<AccountRateLimitsRead, String> {
    let rpc: AccountRateLimitsRpcWire = serde_json::from_str(response)
        .map_err(|error| format!("failed to decode Codex account quota response: {error}"))?;
    if let Some(error) = rpc.error {
        return Err(format!("Codex account quota request failed: {error}"));
    }
    let result = rpc
        .result
        .ok_or_else(|| "Codex account quota response has no result".to_string())?;
    let snapshots: Vec<WireRateLimitSnapshot> = match result.rate_limits_by_limit_id {
        Some(by_id) if !by_id.is_empty() => by_id
            .into_iter()
            .map(|(limit_id, mut snapshot)| {
                if snapshot.limit_id.is_none() {
                    snapshot.limit_id = Some(limit_id);
                }
                snapshot
            })
            .collect(),
        _ => result.rate_limits.into_iter().collect(),
    };
    let envelopes = snapshots
        .into_iter()
        .flat_map(|snapshot| wire_envelopes(snapshot, observed_at))
        .collect::<Vec<_>>();
    if envelopes.is_empty() && result.rate_limit_reset_credits.is_none() {
        return Err(
            "Codex account quota response contains no quota windows or credits".to_string(),
        );
    }

    Ok(AccountRateLimitsRead {
        envelopes,
        rate_limit_reset_credits: result.rate_limit_reset_credits,
        observed_at,
    })
}

fn wire_envelopes(
    snapshot: WireRateLimitSnapshot,
    observed_at: DateTime<Utc>,
) -> Vec<RateLimitEnvelope> {
    let scope = classify_limit_scope(snapshot.limit_id.as_deref());
    let base_limit_id = snapshot.limit_id.clone();
    let primary = snapshot.primary.and_then(wire_window);
    let secondary = snapshot.secondary.and_then(wire_window);
    let credits = wire_credits(snapshot.credits);
    let mut envelopes = Vec::new();
    if primary.is_some() || secondary.is_some() || credits.is_some() {
        envelopes.push(RateLimitEnvelope {
            scope,
            limit_id: base_limit_id.clone(),
            limit_name: snapshot.limit_name.clone(),
            plan_type: snapshot.plan_type.clone(),
            observed_at: Some(observed_at),
            limits: RateLimits::from_primary_secondary(primary, secondary),
            credits,
        });
    }

    if let Some(individual) = snapshot.individual_limit
        && individual.remaining_percent.is_some_and(f64::is_finite)
    {
        let remaining = individual.remaining_percent.unwrap().clamp(0.0, 100.0);
        let limit_name = match (individual.used.as_deref(), individual.limit.as_deref()) {
            (Some(used), Some(limit)) => {
                Some(format!("Individual spend limit ({used} of {limit})"))
            }
            _ => Some("Individual spend limit".to_string()),
        };
        envelopes.push(RateLimitEnvelope {
            scope: RateLimitScope::IndividualAccount,
            limit_id: Some(format!(
                "{}:individual",
                base_limit_id.as_deref().unwrap_or("account")
            )),
            limit_name,
            plan_type: snapshot.plan_type,
            observed_at: Some(observed_at),
            limits: RateLimits::new(vec![UsageWindow {
                used_percent: 100.0 - remaining,
                remaining_percent: remaining,
                window_minutes: 0,
                resets_at: individual
                    .resets_at
                    .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
            }]),
            credits: None,
        });
    }

    envelopes
}

fn wire_window(window: WireUsageWindow) -> Option<UsageWindow> {
    let used_percent = window
        .used_percent
        .or_else(|| window.remaining_percent.map(|remaining| 100.0 - remaining))?;
    let remaining_percent = window.remaining_percent.unwrap_or(100.0 - used_percent);
    if !used_percent.is_finite() || !remaining_percent.is_finite() {
        return None;
    }
    Some(UsageWindow {
        used_percent: used_percent.clamp(0.0, 100.0),
        remaining_percent: remaining_percent.clamp(0.0, 100.0),
        window_minutes: window.window_duration_mins.unwrap_or(0),
        resets_at: window
            .resets_at
            .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
    })
}

fn wire_credits(credits: Option<WireCredits>) -> Option<CreditBalance> {
    let credits = credits?;
    let balance = credits.balance.as_ref().and_then(decimal_text);
    if balance.is_none() && credits.has_credits.is_none() && credits.unlimited.is_none() {
        return None;
    }
    let has_credits = credits
        .has_credits
        .unwrap_or_else(|| balance.as_deref().is_some_and(|item| item != "0"));
    Some(CreditBalance {
        balance,
        has_credits,
        unlimited: credits.unlimited.unwrap_or(false),
    })
}

pub fn usage_snapshot_from_envelopes(
    source: UsageSource,
    provenance_source: impl Into<String>,
    envelopes: &[RateLimitEnvelope],
) -> UsageSnapshot {
    let stream = UsageStream::new(source, envelopes.to_vec());
    snapshot_from_stream_with_provenance(&stream, provenance_source)
}

pub fn snapshot_from_stream(stream: &UsageStream) -> UsageSnapshot {
    snapshot_from_stream_with_provenance(stream, "")
}

pub fn snapshot_from_stream_with_provenance(
    stream: &UsageStream,
    provenance_source: impl Into<String>,
) -> UsageSnapshot {
    let envelopes = &stream.envelopes;
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
            let windows = envelope
                .limits
                .windows
                .iter()
                .map(QuotaWindow::from)
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
        .filter(|(envelope, _)| {
            !envelopes.iter().any(|candidate| {
                candidate.scope == RateLimitScope::GlobalAccount && candidate.credits.is_some()
            }) || envelope.scope == RateLimitScope::GlobalAccount
        })
        .max_by_key(|(envelope, _)| envelope_rank_key(envelope))
        .map(|(_, credits)| credits.clone());
    UsageSnapshot {
        source: stream.source.clone(),
        scopes,
        credits,
        observed_at: newest.and_then(|envelope| envelope.observed_at),
        provenance_source: provenance_source.into(),
    }
}

pub fn snapshots_from_streams(streams: &[UsageStream]) -> UsageSnapshotCollection {
    UsageSnapshotCollection {
        snapshots: streams.iter().map(snapshot_from_stream).collect(),
    }
}

impl UsageSnapshotCollection {
    pub fn from_streams(streams: &[UsageStream]) -> Self {
        snapshots_from_streams(streams)
    }
}

pub fn select_session_envelope_global_first(
    envelopes: &[RateLimitEnvelope],
) -> Option<RateLimitEnvelope> {
    let global = envelopes
        .iter()
        .filter(|item| item.scope == RateLimitScope::GlobalAccount)
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
        .any(|item| item.scope == RateLimitScope::GlobalAccount && item.credits.is_some());
    envelopes
        .iter()
        .filter(|item| item.credits.is_some())
        .filter(|item| !has_global || item.scope == RateLimitScope::GlobalAccount)
        .max_by_key(|item| envelope_rank_key(item))
        .and_then(|item| item.credits.clone())
}

pub fn select_effective_limits_global_first(
    candidates: &[SessionLimitCandidate],
) -> Option<EffectiveLimitSelection> {
    let has_global = candidates.iter().any(|item| {
        item.envelope.scope == RateLimitScope::GlobalAccount
            && (limits_present(&item.envelope.limits) || item.envelope.credits.is_some())
    });
    let selected = candidates
        .iter()
        .filter(|item| !has_global || item.envelope.scope == RateLimitScope::GlobalAccount)
        .max_by_key(|item| {
            (
                envelope_rank_key(&item.envelope),
                system_time_rank(item.session_last_activity),
            )
        })?;
    let has_global_credits = candidates.iter().any(|item| {
        item.envelope.scope == RateLimitScope::GlobalAccount && item.envelope.credits.is_some()
    });
    let credits = candidates
        .iter()
        .filter(|item| item.envelope.credits.is_some())
        .filter(|item| !has_global_credits || item.envelope.scope == RateLimitScope::GlobalAccount)
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

impl<'de> Deserialize<'de> for RateLimits {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            windows: Option<Vec<UsageWindow>>,
            primary: Option<UsageWindow>,
            secondary: Option<UsageWindow>,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            windows: wire
                .windows
                .unwrap_or_else(|| wire.primary.into_iter().chain(wire.secondary).collect()),
        })
    }
}

pub fn select_effective_limits_by_source(
    streams: &[UsageStream],
    source: &UsageSource,
) -> Option<EffectiveLimitSelection> {
    if !source.is_selectable() {
        return None;
    }

    let candidates = streams
        .iter()
        .filter(|stream| {
            stream.source.is_selectable()
                && stream.source.lane == source.lane
                && stream.source.stream_id == source.stream_id
        })
        .flat_map(|stream| {
            stream
                .envelopes
                .iter()
                .filter(|envelope| limits_present(&envelope.limits) || envelope.credits.is_some())
                .map(|envelope| SessionLimitCandidate {
                    session_id: stream.source.stream_id.clone(),
                    session_last_activity: SystemTime::UNIX_EPOCH,
                    envelope: envelope.clone(),
                })
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    let has_global_limits = candidates.iter().any(|candidate| {
        candidate.envelope.scope == RateLimitScope::GlobalAccount
            && limits_present(&candidate.envelope.limits)
    });
    let limits_pool = candidates
        .iter()
        .filter(|candidate| {
            !has_global_limits || candidate.envelope.scope == RateLimitScope::GlobalAccount
        })
        .collect::<Vec<_>>();
    let selected = select_unique_ranked(&limits_pool)?;

    let has_global_credits = candidates.iter().any(|candidate| {
        candidate.envelope.scope == RateLimitScope::GlobalAccount
            && candidate.envelope.credits.is_some()
    });
    let credits = candidates
        .iter()
        .filter(|candidate| candidate.envelope.credits.is_some())
        .filter(|candidate| {
            !has_global_credits || candidate.envelope.scope == RateLimitScope::GlobalAccount
        })
        .map(|candidate| &candidate.envelope)
        .collect::<Vec<_>>();
    let credits = select_unique_ranked_ref(&credits).and_then(|envelope| envelope.credits.clone());

    Some(EffectiveLimitSelection {
        source_session_id: source.stream_id.clone(),
        source_limit_id: selected.envelope.limit_id.clone(),
        source_scope: selected.envelope.scope,
        observed_at: selected.envelope.observed_at,
        limits: selected.envelope.limits.clone(),
        credits,
    })
}

fn select_unique_ranked<'a>(
    candidates: &[&'a SessionLimitCandidate],
) -> Option<&'a SessionLimitCandidate> {
    let best = candidates.iter().max_by_key(|candidate| {
        (
            canonical_envelope_rank_key(&candidate.envelope),
            system_time_rank(candidate.session_last_activity),
        )
    })?;
    let best_key = (
        canonical_envelope_rank_key(&best.envelope),
        system_time_rank(best.session_last_activity),
    );
    let tied = candidates
        .iter()
        .filter(|candidate| {
            (
                canonical_envelope_rank_key(&candidate.envelope),
                system_time_rank(candidate.session_last_activity),
            ) == best_key
        })
        .map(|candidate| &candidate.envelope)
        .collect::<Vec<_>>();
    let first = tied.first()?;
    tied.iter()
        .all(|candidate| *candidate == *first)
        .then_some(*best)
}

fn select_unique_ranked_ref<'a>(
    candidates: &[&'a RateLimitEnvelope],
) -> Option<&'a RateLimitEnvelope> {
    let best = candidates
        .iter()
        .max_by_key(|envelope| canonical_envelope_rank_key(envelope))?;
    let best_key = canonical_envelope_rank_key(best);
    let tied = candidates
        .iter()
        .filter(|envelope| canonical_envelope_rank_key(envelope) == best_key)
        .collect::<Vec<_>>();
    let first = tied.first()?;
    tied.iter()
        .all(|candidate| *candidate == *first)
        .then_some(*best)
}

pub fn format_window_label(minutes: u64) -> String {
    if minutes > 0 && minutes.is_multiple_of(1_440) {
        format!("{}d", minutes / 1_440)
    } else if minutes > 0 && minutes.is_multiple_of(60) {
        format!("{}h", minutes / 60)
    } else {
        format!("{minutes}m")
    }
}

fn parse_usage_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?.as_object()?;
    let window_minutes = value.get("window_minutes").and_then(uint_value)?;
    if window_minutes == 0 {
        return None;
    }
    let used_percent = value.get("used_percent").and_then(number_at);
    let remaining_percent = value.get("remaining_percent").and_then(number_at);
    let used_percent = match (used_percent, remaining_percent) {
        (Some(used), _) => clamp_percent(used),
        (None, Some(remaining)) => clamp_percent(100.0 - remaining),
        (None, None) => return None,
    };
    let remaining_percent = remaining_percent
        .map(clamp_percent)
        .unwrap_or_else(|| clamp_percent(100.0 - used_percent));
    Some(UsageWindow {
        used_percent,
        remaining_percent,
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

fn canonical_envelope_rank_key(envelope: &RateLimitEnvelope) -> (i64, u8) {
    (
        envelope
            .observed_at
            .map(|ts| ts.timestamp_millis())
            .unwrap_or(i64::MIN),
        envelope.scope.preference(),
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
    fn account_rate_limits_transport_method_is_canonical() {
        assert_eq!(ACCOUNT_RATE_LIMITS_METHOD, "account/rateLimits/read");
    }

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
            format_window_label(envelope.limits.primary().unwrap().window_minutes),
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
        let snapshot = usage_snapshot_from_envelopes(
            UsageSource::new("codex", [UsageSignal::CodexSubscriptionUsage]),
            "fixture",
            &[spark, global],
        );
        assert_eq!(snapshot.scopes.len(), 2);
        assert_eq!(snapshot.source.lane, UsageLane::CodexSubscription);
        assert_eq!(snapshot.scopes[0].kind, RateLimitScope::GlobalAccount);
        let wire = serde_json::to_value(&snapshot).expect("serialize snapshot");
        assert_eq!(wire["scopes"][0]["kind"], "global_account");
        assert_eq!(wire["scopes"][1]["kind"], "model");
    }

    #[test]
    fn quota_scope_wire_values_are_exact() {
        for (scope, expected) in [
            (RateLimitScope::GlobalAccount, "global_account"),
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
    fn usage_window_deserialization_rejects_incomplete_or_invalid_windows() {
        for wire in [
            serde_json::json!({"window_minutes": 300}),
            serde_json::json!({"used_percent": 25.0}),
            serde_json::json!({"used_percent": 25.0, "window_minutes": 0}),
            serde_json::json!({"used_percent": "NaN", "window_minutes": 300}),
        ] {
            assert!(
                serde_json::from_value::<UsageWindow>(wire).is_err(),
                "invalid usage window was accepted"
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
            scope: RateLimitScope::GlobalAccount,
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
            scope: RateLimitScope::GlobalAccount,
            limits: RateLimits::new(vec![UsageWindow {
                window_minutes: 10_080,
                ..UsageWindow::default()
            }]),
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

    fn codex_source(stream_id: &str, signals: &[UsageSignal]) -> UsageSource {
        UsageSource::new(stream_id, signals.iter().cloned())
    }

    #[test]
    fn adaptive_weekly_only_window_is_not_positional() {
        let payload = serde_json::json!({
            "limit_id": "codex",
            "windows": [{"used_percent": 4.0, "window_minutes": 10080}]
        });
        let envelope = parse_rate_limit_envelope(Some(&payload), None).expect("envelope");
        assert_eq!(envelope.limits.windows.len(), 1);
        assert_eq!(envelope.limits.windows[0].window_minutes, 10080);
        assert_eq!(
            format_window_label(envelope.limits.windows[0].window_minutes),
            "7d"
        );
    }

    #[test]
    fn adaptive_arbitrary_windows_are_preserved() {
        let payload = serde_json::json!({
            "limit_id": "codex",
            "windows": [
                {"used_percent": 4.0, "window_minutes": 17},
                {"used_percent": 12.0, "window_minutes": 4321}
            ]
        });
        let envelope = parse_rate_limit_envelope(Some(&payload), None).expect("envelope");
        assert_eq!(
            envelope
                .limits
                .windows
                .iter()
                .map(|window| window.window_minutes)
                .collect::<Vec<_>>(),
            vec![17, 4321]
        );
    }

    #[test]
    fn malformed_duration_does_not_fabricate_a_window() {
        for raw in [
            serde_json::json!({"limit_id": "codex", "windows": [{"window_minutes": 0}]}),
            serde_json::json!({"limit_id": "codex", "windows": [{"window_minutes": -1}]}),
            serde_json::json!({"limit_id": "codex", "windows": [{"window_minutes": "300"}]}),
        ] {
            assert!(parse_rate_limit_envelope(Some(&raw), None).is_none());
        }
    }

    #[test]
    fn missing_percentage_does_not_fabricate_full_remaining_usage() {
        assert!(
            parse_rate_limit_envelope(
                Some(&serde_json::json!({
                    "limit_id": "codex",
                    "primary": {"window_minutes": 300}
                })),
                None,
            )
            .is_none()
        );
    }

    #[test]
    fn remaining_only_wire_percentage_preserves_codex_remaining_semantics() {
        let envelope = parse_rate_limit_envelope(
            Some(&serde_json::json!({
                "limit_id": "codex",
                "primary": {"window_minutes": 10080, "remaining_percent": 17.0}
            })),
            None,
        )
        .expect("remaining-only quota");
        let window = envelope.limits.primary().expect("weekly window");
        assert_eq!(window.used_percent, 83.0);
        assert_eq!(window.remaining_percent, 17.0);
    }

    #[test]
    fn credits_only_stream_is_retained_without_quota_windows() {
        let payload = serde_json::json!({
            "limit_id": "codex",
            "credits": {"has_credits": true, "balance": "2500"}
        });
        let envelope = parse_rate_limit_envelope(Some(&payload), None).expect("credits envelope");
        assert!(envelope.limits.windows.is_empty());
        let stream = UsageStream::new(
            codex_source("codex-account", &[UsageSignal::CodexSubscriptionUsage]),
            vec![envelope],
        );
        let snapshot = snapshot_from_stream(&stream);
        assert_eq!(snapshot.scopes.len(), 0);
        assert_eq!(
            snapshot.credits.and_then(|credits| credits.balance),
            Some("2500".into())
        );
    }

    #[test]
    fn provider_classification_requires_explicit_non_session_signal() {
        assert_eq!(
            classify_usage_signals(&[UsageSignal::CodexSessionJsonl]),
            UsageLane::Unknown
        );
        assert_eq!(
            classify_usage_signals(&[UsageSignal::CodexSubscriptionUsage]),
            UsageLane::CodexSubscription
        );
        assert_eq!(
            classify_usage_signals(&[
                UsageSignal::CodexSubscriptionUsage,
                UsageSignal::OpenAiApiUsage
            ]),
            UsageLane::Unknown
        );
    }

    #[test]
    fn hybrid_streams_remain_isolated_by_lane_and_stream_id() {
        let envelope = |scope| RateLimitEnvelope {
            scope,
            limits: RateLimits::new(vec![UsageWindow {
                window_minutes: 17,
                used_percent: 10.0,
                remaining_percent: 90.0,
                resets_at: None,
            }]),
            ..RateLimitEnvelope::default()
        };
        let codex = UsageStream::new(
            codex_source("codex-account", &[UsageSignal::CodexSubscriptionUsage]),
            vec![envelope(RateLimitScope::GlobalAccount)],
        );
        let openai = UsageStream::new(
            UsageSource::new("openai-key", [UsageSignal::OpenAiApiRateLimit]),
            vec![envelope(RateLimitScope::GlobalAccount)],
        );
        let collection = snapshots_from_streams(&[codex.clone(), openai.clone()]);
        assert_eq!(collection.snapshots.len(), 2);
        assert_eq!(
            select_effective_limits_by_source(
                &[codex, openai],
                &codex_source("codex-account", &[UsageSignal::CodexSubscriptionUsage])
            )
            .expect("codex selection")
            .source_scope,
            RateLimitScope::GlobalAccount
        );
    }

    #[test]
    fn account_credits_precede_individual_credits_within_one_stream() {
        let account = RateLimitEnvelope {
            scope: RateLimitScope::GlobalAccount,
            observed_at: Utc.timestamp_opt(100, 0).single(),
            credits: Some(CreditBalance {
                balance: Some("2500".to_string()),
                has_credits: true,
                unlimited: false,
            }),
            ..RateLimitEnvelope::default()
        };
        let individual = RateLimitEnvelope {
            scope: RateLimitScope::IndividualAccount,
            observed_at: Utc.timestamp_opt(200, 0).single(),
            credits: Some(CreditBalance {
                balance: Some("10".to_string()),
                has_credits: true,
                unlimited: false,
            }),
            ..RateLimitEnvelope::default()
        };
        let stream = UsageStream::new(
            codex_source("codex-account", &[UsageSignal::CodexSubscriptionUsage]),
            vec![account, individual],
        );
        let selected =
            select_effective_limits_by_source(std::slice::from_ref(&stream), &stream.source)
                .expect("selection");
        assert_eq!(
            selected
                .credits
                .and_then(|credits| credits.balance)
                .as_deref(),
            Some("2500")
        );
    }

    #[test]
    fn equal_rank_envelopes_fail_closed_instead_of_picking_an_arbitrary_one() {
        let observed_at = Utc.timestamp_opt(100, 0).single();
        let make = |used_percent| RateLimitEnvelope {
            scope: RateLimitScope::GlobalAccount,
            observed_at,
            limits: RateLimits::new(vec![UsageWindow {
                used_percent,
                remaining_percent: 100.0 - used_percent,
                window_minutes: 17,
                resets_at: None,
            }]),
            ..RateLimitEnvelope::default()
        };
        let stream = UsageStream::new(
            codex_source("codex-account", &[UsageSignal::CodexSubscriptionUsage]),
            vec![make(10.0), make(20.0)],
        );
        assert!(
            select_effective_limits_by_source(std::slice::from_ref(&stream), &stream.source)
                .is_none()
        );
    }

    #[test]
    fn source_serde_is_stable_and_provenance_source_is_opaque() {
        let source = codex_source(
            "codex-account",
            &[
                UsageSignal::CodexSessionJsonl,
                UsageSignal::CodexSubscriptionUsage,
            ],
        );
        let stream = UsageStream::new(source.clone(), Vec::new());
        let snapshot = snapshot_from_stream_with_provenance(&stream, "adapter://local");
        let wire = serde_json::to_value(&snapshot).expect("snapshot wire");
        assert_eq!(wire["source"]["lane"], "codex_subscription");
        assert_eq!(wire["source"]["stream_id"], "codex-account");
        assert_eq!(wire["provenance_source"], "adapter://local");
        assert_eq!(
            serde_json::from_value::<UsageSnapshot>(wire).expect("snapshot roundtrip"),
            snapshot
        );
    }

    #[test]
    fn snapshot_wire_preserves_every_dynamic_access_window_field() {
        let observed_at = Utc.timestamp_opt(1_785_000_000, 0).single();
        let stream = UsageStream::new(
            codex_source("codex-account", &[UsageSignal::CodexSubscriptionUsage]),
            vec![RateLimitEnvelope {
                limit_id: Some("codex".to_string()),
                limit_name: Some("Codex".to_string()),
                plan_type: Some("pro_20x".to_string()),
                observed_at,
                scope: RateLimitScope::GlobalAccount,
                limits: RateLimits::new(vec![UsageWindow {
                    used_percent: 83.0,
                    remaining_percent: 17.0,
                    window_minutes: 10_080,
                    resets_at: observed_at,
                }]),
                credits: None,
            }],
        );
        let snapshot = snapshot_from_stream_with_provenance(&stream, "Codex account API");
        let window = &snapshot.scopes[0].windows[0];
        assert_eq!(window.window_minutes, 10_080);
        assert_eq!(window.used_percent, 83.0);
        assert_eq!(window.remaining_percent, 17.0);
        assert_eq!(window.resets_at, observed_at);
        let wire = serde_json::to_value(snapshot).expect("snapshot wire");
        assert_eq!(wire["provenance_source"], "Codex account API");
        assert_eq!(wire["scopes"][0]["windows"][0]["window_minutes"], 10_080);
        assert_eq!(wire["scopes"][0]["windows"][0]["used_percent"], 83.0);
        assert_eq!(wire["scopes"][0]["windows"][0]["remaining_percent"], 17.0);
        assert_eq!(
            wire["scopes"][0]["windows"][0]["resets_at"],
            observed_at
                .map(|value| {
                    serde_json::Value::String(
                        value.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
                    )
                })
                .unwrap()
        );
    }

    #[test]
    fn legacy_positional_limits_deserialize_into_dynamic_windows() {
        let limits = serde_json::from_value::<RateLimits>(serde_json::json!({
            "primary": {"window_minutes": 300, "used_percent": 1.0},
            "secondary": {"window_minutes": 10080, "used_percent": 2.0}
        }))
        .expect("legacy limits");
        assert_eq!(
            limits
                .windows
                .iter()
                .map(|window| window.window_minutes)
                .collect::<Vec<_>>(),
            vec![300, 10080]
        );
        assert!(
            serde_json::to_value(limits)
                .expect("dynamic limits")
                .get("primary")
                .is_none()
        );
    }

    #[test]
    fn legacy_used_only_window_deserialization_keeps_codex_remaining_semantics() {
        let limits = serde_json::from_value::<RateLimits>(serde_json::json!({
            "windows": [{"used_percent": 83.0, "window_minutes": 10080}]
        }))
        .expect("legacy used-only limits");
        let window = limits.primary().expect("weekly window");
        assert_eq!(window.used_percent, 83.0);
        assert_eq!(window.remaining_percent, 17.0);
    }

    #[test]
    fn app_server_rate_limit_reset_credits_are_kept_separate() {
        let observed_at = Utc.timestamp_opt(1_785_000_000, 0).single().unwrap();
        let response = serde_json::json!({
            "id": 2,
            "result": {
                "rateLimits": {
                    "limitId": "codex",
                    "primary": {
                        "usedPercent": 83.0,
                        "windowDurationMins": 10080,
                        "resetsAt": 1785369546
                    }
                },
                "rateLimitsByLimitId": {
                    "codex": {
                        "limitId": "codex",
                        "primary": {
                            "usedPercent": 83.0,
                            "windowDurationMins": 10080,
                            "resetsAt": 1785369546
                        }
                    }
                },
                "rateLimitResetCredits": {
                    "availableCount": 1,
                    "credits": [{
                        "id": "opaque-credit-id",
                        "resetType": "codexRateLimits",
                        "status": "available",
                        "grantedAt": 1784488000,
                        "expiresAt": 1784677005,
                        "title": "Full reset",
                        "description": "Weekly and session windows"
                    }]
                }
            }
        });

        let parsed = parse_account_rate_limits_response(&response.to_string(), observed_at)
            .expect("account rate-limit response");
        assert_eq!(parsed.envelopes.len(), 1);
        assert_eq!(
            parsed.envelopes[0].limits.primary().unwrap().used_percent,
            83.0
        );
        let resets = parsed
            .rate_limit_reset_credits
            .expect("reset-credit summary");
        assert_eq!(resets.available_count, 1);
        assert_eq!(
            resets.credits.as_ref().unwrap()[0].reset_type,
            "codexRateLimits"
        );
        assert_eq!(resets.credits.as_ref().unwrap()[0].granted_at, 1784488000);
        assert_eq!(
            resets.credits.as_ref().unwrap()[0].description.as_deref(),
            Some("Weekly and session windows")
        );
    }

    #[test]
    fn reset_credit_details_preserve_null_and_empty_wire_states() {
        let null_details =
            serde_json::from_value::<RateLimitResetCreditsSummary>(serde_json::json!({
                "availableCount": 1,
                "credits": null
            }))
            .expect("null reset-credit details");
        assert!(null_details.credits.is_none());

        let empty_details =
            serde_json::from_value::<RateLimitResetCreditsSummary>(serde_json::json!({
                "availableCount": 0,
                "credits": []
            }))
            .expect("empty reset-credit details");
        assert_eq!(empty_details.credits, Some(Vec::new()));
    }

    #[test]
    fn reset_credit_only_response_remains_available_without_quota_windows() {
        let response = serde_json::json!({
            "id": 2,
            "result": {
                "rateLimits": null,
                "rateLimitsByLimitId": {},
                "rateLimitResetCredits": {"availableCount": 1, "credits": null}
            }
        });
        let parsed = parse_account_rate_limits_response(
            &response.to_string(),
            Utc.timestamp_opt(1_785_000_000, 0).single().unwrap(),
        )
        .expect("reset-credit-only response");
        assert!(parsed.envelopes.is_empty());
        assert_eq!(
            parsed
                .rate_limit_reset_credits
                .expect("reset-credit summary")
                .available_count,
            1
        );
    }
}
