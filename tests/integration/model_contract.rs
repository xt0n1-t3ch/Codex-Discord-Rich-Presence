use std::fs;

use codex_discord_presence::config::PricingConfig;
use codex_discord_presence::cost::{
    PricingSource, PricingStatus, TokenUsage, compute_cost, format_presentable_cost,
    resolve_model_pricing,
};
use codex_discord_presence::model::{
    ContextSource, ReasoningEffort, SessionSpeed, SpeedMode, SpeedSource, model_catalog,
    prompt_cache_policy, resolve_context_window_from_cache_path, resolve_model,
};

#[test]
fn bundled_catalog_is_machine_readable_and_carries_source_metadata() {
    let catalog = model_catalog();
    assert!(catalog.models.len() >= 7);
    assert_eq!(catalog.verified_on, "2026-07-09");
    assert!(
        catalog
            .sources
            .iter()
            .any(|source| source.kind == "openai_model_guide")
    );
    assert!(
        catalog
            .sources
            .iter()
            .any(|source| source.kind == "codex_app_catalog")
    );
}

#[test]
fn gpt_5_6_aliases_capabilities_and_efforts_are_centralized() {
    let sol = resolve_model("gpt-5.6").expect("5.6 alias");
    assert_eq!(sol.canonical_id(), "gpt-5.6-sol");
    assert_eq!(sol.display_name(), "5.6 Sol");
    assert!(sol.supports_effort(ReasoningEffort::Ultra));
    assert_eq!(sol.resolve_speed(true), SpeedMode::Fast);
    assert_eq!(sol.fast_usage_multiplier(), None);

    let terra = resolve_model("gpt-5.6-terra").expect("terra");
    assert!(terra.supports_effort(ReasoningEffort::Ultra));

    let luna = resolve_model("gpt-5.6-luna").expect("luna");
    assert!(!luna.supports_effort(ReasoningEffort::Ultra));
    assert!(luna.supports_effort(ReasoningEffort::Max));
}

#[test]
fn reasoning_effort_parses_codex_app_modes_and_uses_light_label() {
    assert_eq!(
        ReasoningEffort::parse(Some("low")),
        Some(ReasoningEffort::Low)
    );
    assert_eq!(ReasoningEffort::Low.label(), "Light");
    assert_eq!(
        ReasoningEffort::parse(Some("max")),
        Some(ReasoningEffort::Max)
    );
    assert_eq!(
        ReasoningEffort::parse(Some("ultra")),
        Some(ReasoningEffort::Ultra)
    );
}

#[test]
fn gpt_5_6_context_prefers_observed_then_local_cache_then_bundle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("models_cache.json");
    fs::write(
        &cache,
        r#"{"models":[{"slug":"gpt-5.6-sol","context_window":380000,"effective_context_window_percent":90}]}"#,
    )
    .expect("cache fixture");

    let observed = resolve_context_window_from_cache_path("gpt-5.6", Some(353_400), &cache)
        .expect("observed context");
    assert_eq!(observed.effective_tokens, 353_400);
    assert_eq!(observed.source, ContextSource::ObservedJsonl);

    let local =
        resolve_context_window_from_cache_path("gpt-5.6", None, &cache).expect("local context");
    assert_eq!(local.raw_tokens, 380_000);
    assert_eq!(local.effective_tokens, 342_000);
    assert_eq!(local.source, ContextSource::LocalModelCache);

    fs::write(&cache, r#"{"models":[]}"#).expect("empty cache");
    let bundled =
        resolve_context_window_from_cache_path("gpt-5.6", None, &cache).expect("bundled context");
    assert_eq!(bundled.raw_tokens, 372_000);
    assert_eq!(bundled.effective_tokens, 353_400);
    assert_eq!(bundled.source, ContextSource::BundledCatalog);
}

#[test]
fn gpt_5_6_prices_and_credits_match_verified_catalog() {
    let config = PricingConfig::default();
    let cases = [
        ("gpt-5.6-sol", 5.0, 6.25, 0.5, 30.0, 125.0, 12.5, 750.0),
        ("gpt-5.6-terra", 2.5, 3.125, 0.25, 15.0, 62.5, 6.25, 375.0),
        ("gpt-5.6-luna", 1.0, 1.25, 0.1, 6.0, 25.0, 2.5, 150.0),
    ];

    for (id, input, write, read, output, credit_input, credit_read, credit_output) in cases {
        let resolved = resolve_model_pricing(id, &config);
        let pricing = resolved.pricing.expect("catalog pricing");
        assert_eq!(resolved.source, PricingSource::Exact);
        assert_eq!(pricing.input_per_million, input);
        assert_eq!(pricing.cache_write_per_million, Some(write));
        assert_eq!(pricing.cached_input_per_million, read);
        assert_eq!(pricing.output_per_million, output);

        let credits = resolve_model(id).and_then(|model| model.credit_rates());
        let credits = credits.expect("credit rates");
        assert_eq!(credits.input_per_million, credit_input);
        assert_eq!(credits.cached_input_per_million, credit_read);
        assert_eq!(credits.output_per_million, credit_output);
        assert_eq!(credits.cache_write_per_million, None);
    }
}

#[test]
fn pricing_status_distinguishes_exact_partial_and_unavailable() {
    let config = PricingConfig::default();
    let partial = compute_cost(
        "gpt-5.6-sol",
        TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 100_000,
            cache_write_tokens: None,
            output_tokens: 200_000,
        },
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(partial.status, PricingStatus::Partial);
    assert!(partial.known_total_cost_usd.is_some());

    let exact = compute_cost(
        "gpt-5.6-sol",
        TokenUsage {
            cache_write_tokens: Some(25_000),
            ..TokenUsage::default()
        },
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(exact.status, PricingStatus::Exact);

    let unavailable = compute_cost(
        "gpt-future-unknown",
        TokenUsage::default(),
        SessionSpeed::default(),
        &config,
    );
    assert_eq!(unavailable.status, PricingStatus::Unavailable);
    assert_eq!(unavailable.known_total_cost_usd, None);
    assert_eq!(unavailable.source, PricingSource::Unavailable);
    assert!(
        resolve_model_pricing("gpt-future-unknown", &config)
            .pricing
            .is_none()
    );
}

#[test]
fn prompt_cache_policy_is_explicit_and_verified() {
    let policy = prompt_cache_policy();
    assert_eq!(policy.minimum_eligible_tokens, 1_024);
    assert_eq!(policy.minimum_lifetime_minutes, 30);
}

#[test]
fn public_cost_presentation_preserves_completeness() {
    assert_eq!(
        format_presentable_cost(Some(0.0065), PricingStatus::Exact),
        Some("$0.0065".to_string())
    );
    assert_eq!(
        format_presentable_cost(Some(0.0065), PricingStatus::Partial),
        Some(">=$0.0065".to_string())
    );
    assert_eq!(
        format_presentable_cost(None, PricingStatus::Unavailable),
        None
    );
}

#[test]
fn fast_economics_are_applied_or_marked_partial() {
    let config = PricingConfig::default();
    let usage = TokenUsage {
        input_tokens: 1_000_000,
        output_tokens: 100_000,
        ..TokenUsage::default()
    };
    let known_fast = compute_cost(
        "gpt-5.5",
        usage,
        SessionSpeed::explicit(SpeedMode::Fast, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(known_fast.status, PricingStatus::Exact);
    assert_eq!(known_fast.known_total_cost_usd, Some(20.0));

    let unpublished_fast = compute_cost(
        "gpt-5.6-sol",
        usage,
        SessionSpeed::explicit(SpeedMode::Fast, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(unpublished_fast.status, PricingStatus::Partial);
    assert_eq!(unpublished_fast.known_total_cost_usd, Some(8.0));
    assert_eq!(unpublished_fast.source, PricingSource::Exact);
}

#[test]
fn cache_write_component_reconciles_with_known_subtotal() {
    let computed = compute_cost(
        "gpt-5.6-terra",
        TokenUsage {
            input_tokens: 1_000_000,
            cached_input_tokens: 200_000,
            cache_write_tokens: Some(100_000),
            output_tokens: 100_000,
        },
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings),
        &PricingConfig::default(),
    );
    assert_eq!(computed.status, PricingStatus::Exact);
    assert_eq!(computed.breakdown.cache_write_cost_usd, 0.3125);
    assert!(
        computed
            .breakdown
            .reconciles_with(computed.known_total_cost_usd)
    );
}

#[test]
fn context_source_accepts_legacy_wire_values_and_writes_canonical_values() {
    let event: ContextSource = serde_json::from_str(r#""event""#).expect("legacy event");
    let catalog: ContextSource = serde_json::from_str(r#""catalog""#).expect("legacy catalog");
    assert_eq!(event, ContextSource::ObservedJsonl);
    assert_eq!(catalog, ContextSource::BundledCatalog);
    assert_eq!(
        serde_json::to_string(&event).expect("event serialize"),
        r#""observed_jsonl""#
    );
    assert_eq!(
        serde_json::to_string(&catalog).expect("catalog serialize"),
        r#""bundled_catalog""#
    );
}
