use serde::{Deserialize, Serialize};

use crate::config::{ModelPricingOverride, PricingConfig};
use crate::model::{
    CatalogRates, ModelResolutionSource, SessionSpeed, SpeedMode, SpeedSource,
    resolve_context_window, resolve_model,
};

pub use crate::model::normalize_model_key;

const MAX_RATE_PER_MILLION: f64 = 1_000_000_000.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub cache_write_per_million: Option<f64>,
    pub cached_input_per_million: f64,
    pub output_per_million: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    Exact,
    Alias,
    Override,
    ProviderReported,
    Unavailable,
    // Legacy wire values retained for backwards-compatible deserialization.
    Partial,
    #[default]
    Fallback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PricingStatus {
    Exact,
    Partial,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CostAttribution {
    #[default]
    SingleModel,
    MixedModels,
    MixedSpeeds,
    MixedModelsAndSpeeds,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: Option<u64>,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TokenCostBreakdown {
    pub input_cost_usd: f64,
    #[serde(default)]
    pub cache_write_cost_usd: f64,
    pub cached_input_cost_usd: f64,
    pub output_cost_usd: f64,
    #[serde(default)]
    pub cached_input_savings_usd: f64,
}

impl TokenCostBreakdown {
    pub fn known_component_total(&self) -> Option<f64> {
        let components = [
            self.input_cost_usd,
            self.cache_write_cost_usd,
            self.cached_input_cost_usd,
            self.output_cost_usd,
        ];
        if components
            .iter()
            .any(|component| !component.is_finite() || *component < 0.0)
        {
            return None;
        }
        let total = components.into_iter().sum::<f64>();
        total.is_finite().then_some(total)
    }

    pub fn reconciles_with(&self, known_total_cost_usd: Option<f64>) -> bool {
        let Some(known) = known_total_cost_usd.filter(|value| value.is_finite() && *value >= 0.0)
        else {
            return false;
        };
        let Some(components) = self.known_component_total() else {
            return false;
        };
        let tolerance = known
            .abs()
            .max(components.abs())
            .mul_add(0.000_001, 0.000_000_001);
        (known - components).abs() <= tolerance
    }

    fn apply_multiplier(&mut self, multiplier: f64) {
        self.input_cost_usd *= multiplier;
        self.cache_write_cost_usd *= multiplier;
        self.cached_input_cost_usd *= multiplier;
        self.output_cost_usd *= multiplier;
        self.cached_input_savings_usd *= multiplier;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostComputation {
    pub pricing: Option<ModelPricing>,
    pub source: PricingSource,
    pub status: PricingStatus,
    pub resolved_model: Option<String>,
    pub breakdown: TokenCostBreakdown,
    pub known_total_cost_usd: Option<f64>,
    /// Backward-compatible known subtotal. Consult `status` before presenting it as exact.
    pub total_cost_usd: f64,
}

impl CostComputation {
    pub fn mark_partial(&mut self) {
        if self.status != PricingStatus::Unavailable {
            self.status = PricingStatus::Partial;
        }
    }
}

pub fn compute_total_cost(
    model_id: &str,
    input_tokens_total: u64,
    cached_input_tokens_total: u64,
    output_tokens_total: u64,
    pricing_config: &PricingConfig,
) -> CostComputation {
    compute_cost(
        model_id,
        TokenUsage {
            input_tokens: input_tokens_total,
            cached_input_tokens: cached_input_tokens_total,
            cache_write_tokens: None,
            output_tokens: output_tokens_total,
        },
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::LegacyDefault),
        pricing_config,
    )
}

pub fn compute_cost(
    model_id: &str,
    usage: TokenUsage,
    speed: SessionSpeed,
    pricing_config: &PricingConfig,
) -> CostComputation {
    let resolved = resolve_model_pricing(model_id, pricing_config);
    let Some(pricing) = resolved.pricing else {
        return unavailable_computation();
    };

    let cached_input_tokens = usage.cached_input_tokens.min(usage.input_tokens);
    let non_cached_input_tokens = usage.input_tokens.saturating_sub(cached_input_tokens);
    let mut breakdown = TokenCostBreakdown {
        input_cost_usd: per_million(non_cached_input_tokens, pricing.input_per_million),
        cache_write_cost_usd: match (usage.cache_write_tokens, pricing.cache_write_per_million) {
            (Some(tokens), Some(rate)) => per_million(tokens, rate),
            _ => 0.0,
        },
        cached_input_cost_usd: per_million(cached_input_tokens, pricing.cached_input_per_million),
        output_cost_usd: per_million(usage.output_tokens, pricing.output_per_million),
        cached_input_savings_usd: (per_million(cached_input_tokens, pricing.input_per_million)
            - per_million(cached_input_tokens, pricing.cached_input_per_million))
        .max(0.0),
    };

    let model = resolve_model(&resolved.resolved_model);
    let supports_fast = model.is_some_and(|model| model.supports_fast());
    let published_fast_multiplier = model.and_then(|model| model.fast_usage_multiplier());
    let (economic_multiplier, speed_complete) = match speed.mode {
        SpeedMode::Fast => match published_fast_multiplier {
            Some(multiplier) => (multiplier, true),
            None => (1.0, false),
        },
        SpeedMode::Standard if !speed.known && supports_fast => (1.0, false),
        SpeedMode::Standard => (1.0, true),
    };
    breakdown.apply_multiplier(economic_multiplier);

    let cache_complete =
        pricing.cache_write_per_million.is_none() || usage.cache_write_tokens.is_some();
    let Some(known_total) = breakdown.known_component_total() else {
        return unavailable_computation();
    };
    let status = if cache_complete && speed_complete {
        PricingStatus::Exact
    } else {
        PricingStatus::Partial
    };

    CostComputation {
        pricing: Some(pricing),
        source: resolved.source,
        status,
        resolved_model: Some(resolved.resolved_model),
        breakdown,
        known_total_cost_usd: Some(known_total),
        total_cost_usd: known_total,
    }
}

fn unavailable_computation() -> CostComputation {
    CostComputation {
        pricing: None,
        source: PricingSource::Unavailable,
        status: PricingStatus::Unavailable,
        resolved_model: None,
        breakdown: TokenCostBreakdown::default(),
        known_total_cost_usd: None,
        total_cost_usd: 0.0,
    }
}

pub fn format_presentable_cost(
    known_total_cost_usd: Option<f64>,
    status: PricingStatus,
) -> Option<String> {
    let total = known_total_cost_usd.filter(|value| value.is_finite() && *value >= 0.0)?;
    let formatted = crate::util::format_cost(total);
    match status {
        PricingStatus::Exact => Some(formatted),
        PricingStatus::Partial => Some(format!(">={formatted}")),
        PricingStatus::Unavailable => None,
    }
}

#[derive(Debug, Clone)]
pub struct PricingResolution {
    pub pricing: Option<ModelPricing>,
    pub source: PricingSource,
    pub resolved_model: String,
}

pub fn resolve_model_pricing(model_id: &str, pricing_config: &PricingConfig) -> PricingResolution {
    let key = normalize_model_key(model_id);

    if let Some(override_pricing) = lookup_override(&key, &pricing_config.overrides) {
        return PricingResolution {
            pricing: Some(override_pricing),
            source: PricingSource::Override,
            resolved_model: key,
        };
    }

    if let Some(alias_target) = pricing_config
        .aliases
        .get(&key)
        .map(|value| normalize_model_key(value))
    {
        if let Some(override_pricing) = lookup_override(&alias_target, &pricing_config.overrides) {
            return PricingResolution {
                pricing: Some(override_pricing),
                source: PricingSource::Override,
                resolved_model: alias_target,
            };
        }
        if let Some(model) = resolve_model(&alias_target)
            && let Some(rates) = model.api_rates()
        {
            return PricingResolution {
                pricing: Some(rates.into()),
                source: PricingSource::Alias,
                resolved_model: model.canonical_id().to_string(),
            };
        }
    }

    if let Some(model) = resolve_model(&key)
        && let Some(rates) = model.api_rates()
    {
        return PricingResolution {
            pricing: Some(rates.into()),
            source: match model.source() {
                ModelResolutionSource::Exact => PricingSource::Exact,
                ModelResolutionSource::Alias => PricingSource::Alias,
            },
            resolved_model: model.canonical_id().to_string(),
        };
    }

    PricingResolution {
        pricing: None,
        source: PricingSource::Unavailable,
        resolved_model: key,
    }
}

fn lookup_override(
    model_key: &str,
    overrides: &std::collections::BTreeMap<String, ModelPricingOverride>,
) -> Option<ModelPricing> {
    let entry = overrides.get(model_key)?;
    if !valid_rate(entry.input_per_million) || !valid_rate(entry.output_per_million) {
        return None;
    }
    let cached_input_per_million = entry
        .cached_input_per_million
        .filter(|value| valid_rate(*value))
        .unwrap_or(0.0);
    Some(ModelPricing {
        input_per_million: entry.input_per_million,
        cache_write_per_million: None,
        cached_input_per_million,
        output_per_million: entry.output_per_million,
    })
}

fn valid_rate(rate: f64) -> bool {
    rate.is_finite() && (0.0..=MAX_RATE_PER_MILLION).contains(&rate)
}

fn per_million(tokens: u64, rate: f64) -> f64 {
    (tokens as f64 / 1_000_000.0) * rate
}

impl From<CatalogRates> for ModelPricing {
    fn from(value: CatalogRates) -> Self {
        Self {
            input_per_million: value.input_per_million,
            cache_write_per_million: value.cache_write_per_million,
            cached_input_per_million: value.cached_input_per_million,
            output_per_million: value.output_per_million,
        }
    }
}

pub fn default_model_context_window(model_id: &str) -> Option<u64> {
    resolve_context_window(model_id, None).map(|context| context.effective_tokens)
}

pub fn api_model_context_window(model_id: &str) -> Option<u64> {
    resolve_model(model_id)?.context()?.api_tokens
}

pub fn long_context_input_threshold(model_id: &str) -> Option<u64> {
    resolve_model(model_id)?
        .context()?
        .long_context_input_threshold
}

pub fn max_output_tokens(model_id: &str) -> Option<u64> {
    resolve_model(model_id)?.context()?.max_output_tokens
}

pub fn speed_multiplier(model_id: &str, fast_active: bool) -> f64 {
    if !fast_active {
        return 1.0;
    }
    resolve_model(model_id)
        .and_then(|model| model.fast_usage_multiplier())
        .unwrap_or(1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelContextMetadata {
    pub oauth_context_window: u64,
    pub raw_context_window: u64,
    pub effective_context_percent: u8,
    pub api_context_window: Option<u64>,
    pub long_context_input_threshold: Option<u64>,
    pub max_output_tokens: Option<u64>,
}

pub fn model_context_metadata(model_id: &str) -> Option<ModelContextMetadata> {
    let model = resolve_model(model_id)?;
    let context = model.context()?;
    let effective_context = context
        .raw_tokens
        .saturating_mul(u64::from(context.effective_percent))
        / 100;
    Some(ModelContextMetadata {
        oauth_context_window: effective_context,
        raw_context_window: context.raw_tokens,
        effective_context_percent: context.effective_percent,
        api_context_window: context.api_tokens,
        long_context_input_threshold: context.long_context_input_threshold,
        max_output_tokens: context.max_output_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_known_model_cost_remains_exact() {
        let computed = compute_total_cost(
            "gpt-5.2-codex",
            1_500_000,
            500_000,
            250_000,
            &PricingConfig::default(),
        );
        assert_eq!(computed.status, PricingStatus::Exact);
        assert!((computed.total_cost_usd - 5.3375).abs() < 0.0001);
        assert!((computed.breakdown.cached_input_savings_usd - 0.7875).abs() < 0.0001);
        assert!(
            computed
                .breakdown
                .reconciles_with(computed.known_total_cost_usd)
        );
    }

    #[test]
    fn cached_tokens_are_clamped_before_costing() {
        let computed = compute_total_cost("gpt-5.4", 100, 1_000, 0, &PricingConfig::default());
        assert_eq!(computed.breakdown.input_cost_usd, 0.0);
        assert!((computed.breakdown.cached_input_cost_usd - 0.000025).abs() < 0.000001);
    }

    #[test]
    fn gpt_5_6_has_no_invented_api_context_or_fast_multiplier() {
        let metadata = model_context_metadata("gpt-5.6-sol").expect("metadata");
        assert_eq!(metadata.raw_context_window, 372_000);
        assert_eq!(metadata.oauth_context_window, 353_400);
        assert_eq!(metadata.api_context_window, None);
        assert_eq!(metadata.long_context_input_threshold, None);
        assert_eq!(metadata.max_output_tokens, None);
        assert_eq!(speed_multiplier("gpt-5.6-sol", true), 1.0);
    }

    #[test]
    fn partial_and_unavailable_costs_cannot_render_as_exact() {
        assert_eq!(
            format_presentable_cost(Some(0.0065), PricingStatus::Partial),
            Some(">=$0.0065".to_string())
        );
        assert_eq!(
            format_presentable_cost(None, PricingStatus::Unavailable),
            None
        );
        assert_eq!(
            format_presentable_cost(Some(0.0), PricingStatus::Exact),
            Some("$0.00".to_string())
        );
    }
}
