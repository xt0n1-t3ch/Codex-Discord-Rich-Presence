use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

const MAX_MODEL_CACHE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_MODEL_CACHE_ENTRIES: usize = 512;
const MAX_MODEL_ID_BYTES: usize = 128;
const MAX_CONTEXT_TOKENS: u64 = 10_000_000;
const MAX_RATE_PER_MILLION: f64 = 1_000_000_000.0;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelCatalog {
    pub schema_version: u32,
    pub verified_on: String,
    pub sources: Vec<SourceVerification>,
    pub prompt_cache_policy: PromptCachePolicy,
    pub models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceVerification {
    pub kind: String,
    pub url: String,
    pub verified_on: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct PromptCachePolicy {
    pub minimum_eligible_tokens: u64,
    pub minimum_lifetime_minutes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub display_name: String,
    #[serde(default)]
    pub reasoning_efforts: Vec<ReasoningEffort>,
    pub supports_fast: bool,
    pub fast_usage_multiplier: Option<f64>,
    pub context: Option<CatalogContext>,
    pub pricing_model: Option<String>,
    pub api_pricing: Option<CatalogRates>,
    pub credit_rates: Option<CatalogRates>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
pub struct CatalogRates {
    pub input_per_million: f64,
    pub cache_write_per_million: Option<f64>,
    pub cached_input_per_million: f64,
    pub output_per_million: f64,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct CatalogContext {
    pub raw_tokens: u64,
    pub effective_percent: u8,
    pub api_tokens: Option<u64>,
    pub long_context_input_threshold: Option<u64>,
    pub max_output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh", alias = "x_high")]
    XHigh,
    Max,
    Ultra,
}

impl ReasoningEffort {
    pub fn parse(raw: Option<&str>) -> Option<Self> {
        let normalized = raw
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .filter(|value| !value.is_empty())?;
        match normalized.as_str() {
            "minimal" => Some(Self::Minimal),
            "low" | "light" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" | "x-high" | "extra_high" | "extra-high" | "extra high" => Some(Self::XHigh),
            "max" => Some(Self::Max),
            "ultra" => Some(Self::Ultra),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Minimal => "Minimal",
            Self::Low => "Light",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "Extra High",
            Self::Max => "Max",
            Self::Ultra => "Ultra",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeedMode {
    #[default]
    Standard,
    Fast,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeedSource {
    #[default]
    Unknown,
    ModelSuffix,
    ThreadSettings,
    OpenCodeDescriptor,
    LegacyDefault,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SessionSpeed {
    pub mode: SpeedMode,
    pub source: SpeedSource,
    pub known: bool,
}

impl SessionSpeed {
    pub const fn explicit(mode: SpeedMode, source: SpeedSource) -> Self {
        Self {
            mode,
            source,
            known: true,
        }
    }
}

impl SpeedMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Fast => "Fast",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    #[default]
    #[serde(rename = "observed_jsonl", alias = "event")]
    ObservedJsonl,
    #[serde(alias = "model_cache")]
    LocalModelCache,
    #[serde(alias = "catalog")]
    BundledCatalog,
}

impl ContextSource {
    #[allow(non_upper_case_globals)]
    pub const Event: Self = Self::ObservedJsonl;
    #[allow(non_upper_case_globals)]
    pub const Catalog: Self = Self::BundledCatalog;

    pub const fn label(self) -> &'static str {
        match self {
            Self::ObservedJsonl => "observed JSONL",
            Self::LocalModelCache => "models_cache.json",
            Self::BundledCatalog => "bundled catalog",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedContextWindow {
    pub raw_tokens: u64,
    pub effective_tokens: u64,
    pub effective_percent: Option<u8>,
    pub source: ContextSource,
    pub raw_source: ContextSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelResolutionSource {
    Exact,
    Alias,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelResolution {
    model: &'static CatalogModel,
    source: ModelResolutionSource,
}

impl ModelResolution {
    pub fn canonical_id(self) -> &'static str {
        self.model.id.as_str()
    }

    pub fn display_name(self) -> &'static str {
        self.model.display_name.as_str()
    }

    pub fn source(self) -> ModelResolutionSource {
        self.source
    }

    pub fn supports_effort(self, effort: ReasoningEffort) -> bool {
        self.model.reasoning_efforts.contains(&effort)
    }

    pub fn resolve_speed(self, fast_requested: bool) -> SpeedMode {
        if fast_requested && self.model.supports_fast {
            SpeedMode::Fast
        } else {
            SpeedMode::Standard
        }
    }

    pub fn supports_fast(self) -> bool {
        self.model.supports_fast
    }

    pub fn fast_usage_multiplier(self) -> Option<f64> {
        self.model.fast_usage_multiplier
    }

    pub fn api_rates(self) -> Option<CatalogRates> {
        resolve_pricing_model(self.model).and_then(|model| model.api_pricing)
    }

    pub fn credit_rates(self) -> Option<CatalogRates> {
        self.model.credit_rates
    }

    pub fn context(self) -> Option<CatalogContext> {
        self.model.context
    }
}

pub fn model_catalog() -> &'static ModelCatalog {
    static CATALOG: OnceLock<ModelCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let parsed: ModelCatalog = serde_json::from_str(include_str!("model_catalog.json"))
            .expect("bundled model catalog must be valid JSON");
        validate_bundled_catalog(&parsed).expect("bundled model catalog must satisfy invariants");
        parsed
    })
}

pub fn prompt_cache_policy() -> &'static PromptCachePolicy {
    &model_catalog().prompt_cache_policy
}

pub fn normalize_model_key(model_id: &str) -> String {
    let key = model_id.trim().to_ascii_lowercase();
    if let Some(base) = key.strip_suffix("-fast")
        && base.starts_with("gpt-")
    {
        return base.to_string();
    }
    key
}

pub fn model_requests_fast(model_id: &str) -> bool {
    let normalized = model_id.trim().to_ascii_lowercase();
    normalized.starts_with("gpt-") && normalized.ends_with("-fast")
}

pub fn resolve_model(model_id: &str) -> Option<ModelResolution> {
    let key = normalize_model_key(model_id);
    let catalog = model_catalog();
    if let Some(model) = catalog.models.iter().find(|model| model.id == key) {
        return Some(ModelResolution {
            model,
            source: ModelResolutionSource::Exact,
        });
    }
    catalog
        .models
        .iter()
        .find(|model| model.aliases.iter().any(|alias| alias == &key))
        .map(|model| ModelResolution {
            model,
            source: ModelResolutionSource::Alias,
        })
}

pub fn canonical_model_key(model_id: &str) -> String {
    resolve_model(model_id)
        .map(|model| model.canonical_id().to_string())
        .unwrap_or_else(|| normalize_model_key(model_id))
}

pub fn format_model_display(
    model_id: &str,
    reasoning_effort: Option<ReasoningEffort>,
    fast_active: bool,
) -> String {
    let resolution = resolve_model(model_id);
    let base = resolution
        .map(ModelResolution::display_name)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| generic_model_label(model_id));
    let effort = reasoning_effort
        .filter(|effort| resolution.is_none_or(|model| model.supports_effort(*effort)));
    let speed = resolution
        .map(|model| model.resolve_speed(fast_active || model_requests_fast(model_id)))
        .unwrap_or_default();

    let mut label = if resolution.is_some_and(|model| model.canonical_id().starts_with("gpt-"))
        && !base.starts_with("GPT-")
    {
        format!("GPT-{base}")
    } else {
        base
    };
    if let Some(effort) = effort {
        label.push_str(" · ");
        label.push_str(effort.label());
    }
    if speed == SpeedMode::Fast {
        label.push_str(" · ⚡ Fast");
    }
    label
}

pub fn resolve_context_window(
    model_id: &str,
    observed_window_tokens: Option<u64>,
) -> Option<ResolvedContextWindow> {
    let cache_path = crate::config::codex_home().join("models_cache.json");
    resolve_context_window_from_cache_path(model_id, observed_window_tokens, &cache_path)
}

pub fn resolve_context_window_from_cache_path(
    model_id: &str,
    observed_window_tokens: Option<u64>,
    cache_path: &Path,
) -> Option<ResolvedContextWindow> {
    let inventory = context_from_local_cache(model_id, cache_path).or_else(|| {
        let context = resolve_model(model_id)?.context()?;
        Some(context_resolution(
            context.raw_tokens,
            context.effective_percent,
            ContextSource::BundledCatalog,
        ))
    });

    if let Some(observed) = observed_window_tokens.filter(|value| valid_context(*value)) {
        if let Some(inventory) = inventory
            && inventory.effective_tokens == observed
        {
            return Some(ResolvedContextWindow {
                raw_tokens: inventory.raw_tokens,
                effective_tokens: observed,
                effective_percent: inventory.effective_percent,
                source: ContextSource::ObservedJsonl,
                raw_source: inventory.raw_source,
            });
        }
        return Some(ResolvedContextWindow {
            raw_tokens: observed,
            effective_tokens: observed,
            effective_percent: None,
            source: ContextSource::ObservedJsonl,
            raw_source: ContextSource::ObservedJsonl,
        });
    }

    inventory
}

fn resolve_pricing_model(model: &'static CatalogModel) -> Option<&'static CatalogModel> {
    let Some(pricing_model) = model.pricing_model.as_deref() else {
        return Some(model);
    };
    model_catalog()
        .models
        .iter()
        .find(|candidate| candidate.id == pricing_model)
}

fn context_from_local_cache(model_id: &str, path: &Path) -> Option<ResolvedContextWindow> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_MODEL_CACHE_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_MODEL_CACHE_BYTES {
        return None;
    }
    let parsed: LocalModelCache = serde_json::from_slice(&bytes).ok()?;
    if parsed.models.len() > MAX_MODEL_CACHE_ENTRIES {
        return None;
    }
    let resolution = resolve_model(model_id)?;
    let entry = parsed.models.into_iter().find(|entry| {
        entry.slug.len() <= MAX_MODEL_ID_BYTES
            && normalize_model_key(&entry.slug) == resolution.canonical_id()
    })?;
    if !valid_context(entry.context_window)
        || !(1..=100).contains(&entry.effective_context_window_percent)
    {
        return None;
    }
    Some(context_resolution(
        entry.context_window,
        entry.effective_context_window_percent,
        ContextSource::LocalModelCache,
    ))
}

fn context_resolution(
    raw_tokens: u64,
    effective_percent: u8,
    source: ContextSource,
) -> ResolvedContextWindow {
    let effective_tokens = raw_tokens.saturating_mul(u64::from(effective_percent)) / 100;
    ResolvedContextWindow {
        raw_tokens,
        effective_tokens,
        effective_percent: Some(effective_percent),
        source,
        raw_source: source,
    }
}

fn valid_context(tokens: u64) -> bool {
    (1..=MAX_CONTEXT_TOKENS).contains(&tokens)
}

fn validate_bundled_catalog(catalog: &ModelCatalog) -> Result<(), &'static str> {
    if catalog.schema_version != 1
        || catalog.models.is_empty()
        || catalog.verified_on.trim().is_empty()
        || catalog.sources.is_empty()
        || catalog.prompt_cache_policy.minimum_eligible_tokens == 0
        || catalog.prompt_cache_policy.minimum_lifetime_minutes == 0
        || catalog.sources.iter().any(|source| {
            source.kind.trim().is_empty()
                || source.url.trim().is_empty()
                || source.verified_on.trim().is_empty()
        })
    {
        return Err("unsupported or empty catalog");
    }
    let mut identifiers = std::collections::HashSet::new();
    for model in &catalog.models {
        if model.id.is_empty()
            || model.id.len() > MAX_MODEL_ID_BYTES
            || normalize_model_key(&model.id) != model.id
            || model.display_name.trim().is_empty()
            || !identifiers.insert(model.id.as_str())
            || model
                .fast_usage_multiplier
                .is_some_and(|multiplier| !valid_positive_rate(multiplier))
            || model.api_pricing.is_some_and(|rates| !valid_rates(rates))
            || model.credit_rates.is_some_and(|rates| !valid_rates(rates))
        {
            return Err("invalid model metadata");
        }
        let mut efforts = std::collections::HashSet::new();
        if model
            .reasoning_efforts
            .iter()
            .any(|effort| !efforts.insert(*effort))
        {
            return Err("duplicate reasoning effort");
        }
        for alias in &model.aliases {
            if alias.is_empty()
                || alias.len() > MAX_MODEL_ID_BYTES
                || normalize_model_key(alias) != *alias
                || !identifiers.insert(alias.as_str())
            {
                return Err("invalid or duplicate model alias");
            }
        }
        if let Some(context) = model.context
            && (!valid_context(context.raw_tokens)
                || !(1..=100).contains(&context.effective_percent)
                || context
                    .api_tokens
                    .is_some_and(|tokens| !valid_context(tokens))
                || context
                    .long_context_input_threshold
                    .is_some_and(|tokens| !valid_context(tokens))
                || context
                    .max_output_tokens
                    .is_some_and(|tokens| !valid_context(tokens)))
        {
            return Err("invalid context metadata");
        }
    }
    for model in &catalog.models {
        if let Some(pricing_model) = model.pricing_model.as_deref()
            && (pricing_model == model.id
                || !catalog.models.iter().any(|candidate| {
                    candidate.id == pricing_model && candidate.api_pricing.is_some()
                }))
        {
            return Err("invalid pricing model reference");
        }
    }
    Ok(())
}

fn valid_rates(rates: CatalogRates) -> bool {
    valid_non_negative_rate(rates.input_per_million)
        && rates
            .cache_write_per_million
            .is_none_or(valid_non_negative_rate)
        && valid_non_negative_rate(rates.cached_input_per_million)
        && valid_non_negative_rate(rates.output_per_million)
}

fn valid_non_negative_rate(rate: f64) -> bool {
    rate.is_finite() && (0.0..=MAX_RATE_PER_MILLION).contains(&rate)
}

fn valid_positive_rate(rate: f64) -> bool {
    rate.is_finite() && rate > 0.0
}

fn generic_model_label(model_id: &str) -> String {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    trimmed
        .split('-')
        .filter(|part| !part.is_empty() && *part != "fast")
        .map(|part| match part.to_ascii_lowercase().as_str() {
            "gpt" => "GPT".to_string(),
            "codex" => "Codex".to_string(),
            "mini" => "Mini".to_string(),
            "max" => "Max".to_string(),
            other if other.chars().all(|ch| ch.is_ascii_digit() || ch == '.') => other.to_string(),
            other => {
                let mut chars = other.chars();
                chars
                    .next()
                    .map(|first| format!("{}{}", first.to_ascii_uppercase(), chars.as_str()))
                    .unwrap_or_default()
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

#[derive(Debug, Deserialize)]
struct LocalModelCache {
    #[serde(default)]
    models: Vec<LocalModelEntry>,
}

#[derive(Debug, Deserialize)]
struct LocalModelEntry {
    slug: String,
    context_window: u64,
    effective_context_window_percent: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_is_valid() {
        assert_eq!(model_catalog().schema_version, 1);
    }

    #[test]
    fn rejects_oversized_local_cache() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("models_cache.json");
        std::fs::write(&path, "x".repeat(MAX_MODEL_CACHE_BYTES as usize + 1)).expect("fixture");
        let context = resolve_context_window_from_cache_path("gpt-5.6", None, &path)
            .expect("bundled fallback");
        assert_eq!(context.source, ContextSource::BundledCatalog);
    }
}
