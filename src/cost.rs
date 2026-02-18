use serde::{Deserialize, Serialize};

use crate::config::{ModelPricingOverride, PricingConfig};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub cached_input_per_million: f64,
    pub output_per_million: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PricingSource {
    Exact,
    Alias,
    Override,
    #[default]
    Fallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenCostBreakdown {
    pub input_cost_usd: f64,
    pub cached_input_cost_usd: f64,
    pub output_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostComputation {
    pub pricing: ModelPricing,
    pub source: PricingSource,
    pub resolved_model: String,
    pub breakdown: TokenCostBreakdown,
    pub total_cost_usd: f64,
}

pub fn compute_total_cost(
    model_id: &str,
    input_tokens_total: u64,
    cached_input_tokens_total: u64,
    output_tokens_total: u64,
    pricing_config: &PricingConfig,
) -> CostComputation {
    let resolved = resolve_model_pricing(model_id, pricing_config);
    let non_cached_input_tokens = input_tokens_total.saturating_sub(cached_input_tokens_total);

    let input_cost_usd =
        (non_cached_input_tokens as f64 / 1_000_000.0) * resolved.pricing.input_per_million;
    let cached_input_cost_usd = (cached_input_tokens_total as f64 / 1_000_000.0)
        * resolved.pricing.cached_input_per_million;
    let output_cost_usd =
        (output_tokens_total as f64 / 1_000_000.0) * resolved.pricing.output_per_million;

    let breakdown = TokenCostBreakdown {
        input_cost_usd,
        cached_input_cost_usd,
        output_cost_usd,
    };
    let total_cost_usd = input_cost_usd + cached_input_cost_usd + output_cost_usd;

    CostComputation {
        pricing: resolved.pricing,
        source: resolved.source,
        resolved_model: resolved.resolved_model,
        breakdown,
        total_cost_usd,
    }
}

#[derive(Debug, Clone)]
pub struct PricingResolution {
    pub pricing: ModelPricing,
    pub source: PricingSource,
    pub resolved_model: String,
}

pub fn resolve_model_pricing(model_id: &str, pricing_config: &PricingConfig) -> PricingResolution {
    let key = normalize_model_key(model_id);

    if let Some(override_pricing) = lookup_override(&key, &pricing_config.overrides) {
        return PricingResolution {
            pricing: override_pricing,
            source: PricingSource::Override,
            resolved_model: key,
        };
    }

    if let Some(alias_target) = pricing_config
        .aliases
        .get(&key)
        .map(|v| normalize_model_key(v))
    {
        if let Some(override_pricing) = lookup_override(&alias_target, &pricing_config.overrides) {
            return PricingResolution {
                pricing: override_pricing,
                source: PricingSource::Override,
                resolved_model: alias_target,
            };
        }
        if let Some(pricing) = default_model_pricing(&alias_target) {
            return PricingResolution {
                pricing,
                source: PricingSource::Alias,
                resolved_model: alias_target,
            };
        }
    }

    if let Some(pricing) = default_model_pricing(&key) {
        return PricingResolution {
            pricing,
            source: PricingSource::Exact,
            resolved_model: key,
        };
    }

    if let Some(alias_target) = default_alias_target(&key)
        && let Some(pricing) = default_model_pricing(alias_target)
    {
        return PricingResolution {
            pricing,
            source: PricingSource::Alias,
            resolved_model: alias_target.to_string(),
        };
    }

    PricingResolution {
        pricing: fallback_pricing(),
        source: PricingSource::Fallback,
        resolved_model: "gpt-5-codex".to_string(),
    }
}

fn lookup_override(
    model_key: &str,
    overrides: &std::collections::BTreeMap<String, ModelPricingOverride>,
) -> Option<ModelPricing> {
    let entry = overrides.get(model_key)?;
    if !entry.input_per_million.is_finite() || entry.input_per_million < 0.0 {
        return None;
    }
    if !entry.output_per_million.is_finite() || entry.output_per_million < 0.0 {
        return None;
    }

    let cached_input_per_million = entry.cached_input_per_million.unwrap_or(0.0).max(0.0);

    Some(ModelPricing {
        input_per_million: entry.input_per_million,
        cached_input_per_million,
        output_per_million: entry.output_per_million,
    })
}

pub fn normalize_model_key(model: &str) -> String {
    model.trim().to_ascii_lowercase()
}

fn fallback_pricing() -> ModelPricing {
    // Official OpenAI API pricing fallback (gpt-5-codex)
    ModelPricing {
        input_per_million: 1.25,
        cached_input_per_million: 0.125,
        output_per_million: 10.0,
    }
}

fn default_alias_target(model: &str) -> Option<&'static str> {
    match model {
        // User-approved policy: gpt-5.3-codex inherits gpt-5.2-codex pricing.
        "gpt-5.3-codex" | "gpt-5.3-codex-latest" => Some("gpt-5.2-codex"),
        _ => None,
    }
}

fn default_model_pricing(model: &str) -> Option<ModelPricing> {
    // Source: https://platform.openai.com/docs/pricing (Feb 2026)
    let pricing = match model {
        "gpt-5.2" | "gpt-5.2-chat-latest" | "gpt-5.2-codex" => ModelPricing {
            input_per_million: 1.75,
            cached_input_per_million: 0.175,
            output_per_million: 14.0,
        },
        "gpt-5.1"
        | "gpt-5.1-chat-latest"
        | "gpt-5.1-codex"
        | "gpt-5.1-codex-max"
        | "gpt-5"
        | "gpt-5-chat-latest"
        | "gpt-5-codex" => ModelPricing {
            input_per_million: 1.25,
            cached_input_per_million: 0.125,
            output_per_million: 10.0,
        },
        "gpt-5-mini" | "gpt-5.1-codex-mini" => ModelPricing {
            input_per_million: 0.25,
            cached_input_per_million: 0.025,
            output_per_million: 2.0,
        },
        "gpt-5-nano" => ModelPricing {
            input_per_million: 0.05,
            cached_input_per_million: 0.005,
            output_per_million: 0.4,
        },
        "codex-mini-latest" => ModelPricing {
            input_per_million: 1.5,
            cached_input_per_million: 0.375,
            output_per_million: 6.0,
        },
        _ => return None,
    };

    Some(pricing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PricingConfig;

    #[test]
    fn resolves_exact_pricing_from_catalog() {
        let config = PricingConfig::default();
        let resolved = resolve_model_pricing("gpt-5.2-codex", &config);
        assert_eq!(resolved.source, PricingSource::Exact);
        assert!((resolved.pricing.input_per_million - 1.75).abs() < 0.0001);
        assert!((resolved.pricing.cached_input_per_million - 0.175).abs() < 0.0001);
        assert!((resolved.pricing.output_per_million - 14.0).abs() < 0.0001);
    }

    #[test]
    fn aliases_gpt_5_3_codex_to_gpt_5_2_codex() {
        let config = PricingConfig::default();
        let resolved = resolve_model_pricing("gpt-5.3-codex", &config);
        assert_eq!(resolved.source, PricingSource::Alias);
        assert_eq!(resolved.resolved_model, "gpt-5.2-codex");
        assert!((resolved.pricing.input_per_million - 1.75).abs() < 0.0001);
        assert!((resolved.pricing.output_per_million - 14.0).abs() < 0.0001);
    }

    #[test]
    fn override_takes_precedence_over_defaults() {
        let mut config = PricingConfig::default();
        config.overrides.insert(
            "gpt-5.3-codex".to_string(),
            ModelPricingOverride {
                input_per_million: 3.0,
                cached_input_per_million: Some(0.3),
                output_per_million: 30.0,
            },
        );

        let resolved = resolve_model_pricing("gpt-5.3-codex", &config);
        assert_eq!(resolved.source, PricingSource::Override);
        assert!((resolved.pricing.input_per_million - 3.0).abs() < 0.0001);
        assert!((resolved.pricing.cached_input_per_million - 0.3).abs() < 0.0001);
        assert!((resolved.pricing.output_per_million - 30.0).abs() < 0.0001);
    }

    #[test]
    fn computes_cost_with_cached_input_split() {
        let config = PricingConfig::default();
        let computed = compute_total_cost("gpt-5.2-codex", 1_500_000, 500_000, 250_000, &config);

        let expected_input = (1_000_000.0 / 1_000_000.0) * 1.75;
        let expected_cached = (500_000.0 / 1_000_000.0) * 0.175;
        let expected_output = (250_000.0 / 1_000_000.0) * 14.0;
        let expected_total = expected_input + expected_cached + expected_output;

        assert!((computed.breakdown.input_cost_usd - expected_input).abs() < 0.0001);
        assert!((computed.breakdown.cached_input_cost_usd - expected_cached).abs() < 0.0001);
        assert!((computed.breakdown.output_cost_usd - expected_output).abs() < 0.0001);
        assert!((computed.total_cost_usd - expected_total).abs() < 0.0001);
    }
}
