//! Bundled model catalog contract: aliases, efforts, context windows and pricing.

use std::fs;

use codex_discord_presence::config::PricingConfig;
use codex_discord_presence::cost::{
    PricingSource, PricingStatus, TokenUsage, compute_cost, compute_cost_for_request,
    format_presentable_cost, resolve_model_pricing,
};
use codex_discord_presence::model::{
    ContextSource, ReasoningEffort, SessionSpeed, SpeedMode, SpeedSource, model_catalog,
    prompt_cache_policy, resolve_context_window_from_cache_path, resolve_model,
};

#[test]
fn bundled_catalog_is_machine_readable_and_carries_source_metadata() {
    let catalog = model_catalog();
    assert!(catalog.models.len() >= 7);
    assert_eq!(catalog.verified_on, "2026-11-07");
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
    assert!(catalog.sources.iter().any(|source| {
        source.kind == "openai_gpt_5_6_cyber"
            && source.url == "https://developers.openai.com/api/docs/models/gpt-5.6-cyber"
            && source.verified_on == "2026-08-12"
    }));
    for (kind, id) in [
        ("openai_gpt_6_1_sol", "gpt-6.1-sol"),
        ("openai_gpt_6_sol", "gpt-6-sol"),
        ("openai_gpt_6_luna", "gpt-6-luna"),
    ] {
        assert!(catalog.sources.iter().any(|source| {
            source.kind == kind
                && source.url == format!("https://developers.openai.com/api/docs/models/{id}")
                && source.verified_on == "2026-10-16"
        }));
    }
    for (kind, url) in [
        (
            "openai_gpt_6_1_terra",
            "https://developers.openai.com/api/docs/models/gpt-6.1-terra",
        ),
        (
            "openai_api_pricing",
            "https://developers.openai.com/api/docs/pricing",
        ),
        (
            "openai_gpt_5_6_sol_promotional_price_until_2026_11_30",
            "https://developers.openai.com/api/docs/pricing",
        ),
        (
            "codex_models_json_126a0ee92e",
            "https://github.com/openai/codex/blob/126a0ee92e/codex-rs/models-manager/models.json",
        ),
    ] {
        assert!(
            catalog.sources.iter().any(|source| {
                source.kind == kind && source.url == url && source.verified_on == "2026-11-07"
            }),
            "source {kind}"
        );
    }
    assert!(catalog.sources.iter().any(|source| {
        source.kind == "openai_daybreak_trusted_access"
            && source.url == "https://learn.chatgpt.com/docs/cyber-safety"
            && source.verified_on == "2026-08-12"
    }));
}

#[test]
fn daybreak_aliases_efforts_context_and_pricing_match_the_cyber_contract() {
    for alias in [
        "gpt-daybreak-blue-latest",
        "gpt-daybreak-blue",
        "gpt-5.6-cyber-blue",
    ] {
        let blue = resolve_model(alias).expect("Blue alias");
        assert_eq!(blue.canonical_id(), "gpt-daybreak-blue-latest");
        assert_eq!(blue.display_name(), "5.6-Cyber-Blue");
        assert!(!blue.supports_fast());
        assert!(blue.api_rates().is_none());
        assert!(blue.credit_rates().is_none());
    }
    for alias in [
        "gpt-5.6-cyber",
        "gpt-daybreak-red",
        "gpt-daybreak-red-latest",
        "gpt-5.6-cyber-red",
    ] {
        let red = resolve_model(alias).expect("Red alias");
        assert_eq!(red.canonical_id(), "gpt-5.6-cyber");
        assert_eq!(red.display_name(), "5.6-Cyber-Red");
        assert!(!red.supports_fast());
        for effort in [
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
            ReasoningEffort::XHigh,
            ReasoningEffort::Max,
            ReasoningEffort::Ultra,
        ] {
            assert!(red.supports_effort(effort));
        }
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("models_cache.json");
    fs::write(&cache, r#"{"models":[]}"#).expect("empty cache");
    let observed =
        resolve_context_window_from_cache_path("gpt-daybreak-blue-latest", Some(258_400), &cache)
            .expect("runtime context");
    assert_eq!(observed.effective_tokens, 258_400);
    assert_eq!(observed.raw_tokens, 258_400);
    assert_eq!(observed.source, ContextSource::ObservedJsonl);

    let red = resolve_model("gpt-5.6-cyber").expect("Red");
    let context = red.context().expect("published context");
    assert_eq!(context.api_tokens, Some(400_000));
    assert_eq!(context.max_output_tokens, Some(128_000));
    assert_eq!(context.long_context_input_threshold, Some(272_000));
    let rates = red.api_rates().expect("published pricing");
    assert_eq!(rates.input_per_million, 12.5);
    assert_eq!(rates.cached_input_per_million, 1.25);
    assert_eq!(rates.cache_write_per_million, Some(15.625));
    assert_eq!(rates.output_per_million, 75.0);
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
fn codex_app_model_inventory_matches_current_local_catalog() {
    let cases = [
        ("gpt-6.1-sol", 500_000, 475_000, true),
        ("gpt-6.1-terra", 500_000, 475_000, true),
        ("gpt-6-sol", 872_000, 872_000, true),
        ("gpt-6-luna", 872_000, 872_000, true),
        ("gpt-5.5", 272_000, 258_400, true),
        ("gpt-5.6-sol", 372_000, 353_400, true),
        ("gpt-5.6-terra", 372_000, 353_400, true),
        ("gpt-5.6-luna", 372_000, 353_400, true),
        ("gpt-5.4", 272_000, 258_400, true),
        ("gpt-5.4-mini", 272_000, 258_400, false),
        ("gpt-5.3-codex-spark", 128_000, 121_600, false),
    ];

    for (id, raw, effective, supports_fast) in cases {
        let model = resolve_model(id).expect("listed model");
        let context = model.context().expect("listed context");
        assert_eq!(context.raw_tokens, raw, "raw context for {id}");
        assert_eq!(
            context.raw_tokens * u64::from(context.effective_percent) / 100,
            effective,
            "usable context for {id}"
        );
        assert_eq!(model.supports_fast(), supports_fast, "Fast for {id}");
    }
}

#[test]
fn current_non_5_6_rates_do_not_reuse_stale_values() {
    let config = PricingConfig::default();
    let mini = resolve_model_pricing("gpt-5.4-mini", &config)
        .pricing
        .expect("5.4 Mini pricing");
    assert_eq!(mini.input_per_million, 0.75);
    assert_eq!(mini.cached_input_per_million, 0.075);
    assert_eq!(mini.output_per_million, 4.5);

    let spark = resolve_model_pricing("gpt-5.3-codex-spark", &config);
    assert_eq!(spark.pricing, None);
    assert_eq!(spark.source, PricingSource::Unavailable);
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
        r#"{"models":[{"slug":"gpt-5.6-sol","context_window":372000,"effective_context_window_percent":95}]}"#,
    )
    .expect("cache fixture");

    let observed = resolve_context_window_from_cache_path("gpt-5.6", Some(353_400), &cache)
        .expect("observed context");
    assert_eq!(observed.effective_tokens, 353_400);
    assert_eq!(observed.raw_tokens, 372_000);
    assert_eq!(observed.source, ContextSource::ObservedJsonl);
    assert_eq!(observed.raw_source, ContextSource::LocalModelCache);

    let local =
        resolve_context_window_from_cache_path("gpt-5.6", None, &cache).expect("local context");
    assert_eq!(local.raw_tokens, 372_000);
    assert_eq!(local.effective_tokens, 353_400);
    assert_eq!(local.source, ContextSource::LocalModelCache);
    assert_eq!(local.raw_source, ContextSource::LocalModelCache);

    fs::write(&cache, r#"{"models":[]}"#).expect("empty cache");
    let bundled =
        resolve_context_window_from_cache_path("gpt-5.6", None, &cache).expect("bundled context");
    assert_eq!(bundled.raw_tokens, 372_000);
    assert_eq!(bundled.effective_tokens, 353_400);
    assert_eq!(bundled.source, ContextSource::BundledCatalog);
    assert_eq!(bundled.raw_source, ContextSource::BundledCatalog);
}

#[test]
fn observed_context_does_not_inherit_mismatched_raw_inventory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("models_cache.json");
    fs::write(
        &cache,
        r#"{"models":[{"slug":"gpt-5.6-sol","context_window":380000,"effective_context_window_percent":90}]}"#,
    )
    .expect("cache fixture");

    let observed = resolve_context_window_from_cache_path("gpt-5.6", Some(353_400), &cache)
        .expect("observed context");
    assert_eq!(observed.raw_tokens, 353_400);
    assert_eq!(observed.effective_tokens, 353_400);
    assert_eq!(observed.effective_percent, None);
    assert_eq!(observed.source, ContextSource::ObservedJsonl);
    assert_eq!(observed.raw_source, ContextSource::ObservedJsonl);
}

#[test]
fn gpt_5_6_prices_and_credits_match_verified_catalog() {
    let config = PricingConfig::default();
    let cases = [
        ("gpt-5.6-sol", 4.0, 5.0, 0.4, 20.0, 125.0, 12.5, 750.0),
        ("gpt-5.6-terra", 2.0, 2.5, 0.2, 12.0, 62.5, 6.25, 375.0),
        ("gpt-5.6-luna", 0.2, 0.25, 0.02, 1.2, 25.0, 2.5, 150.0),
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
fn gpt_6_family_prices_context_and_efforts_match_verified_catalog() {
    let config = PricingConfig::default();
    let cases = [
        ("gpt-6.1-sol", 2.0, 2.5, 0.1, 10.0, true),
        ("gpt-6.1-terra", 1.0, 1.25, 0.05, 5.0, true),
        ("gpt-6-sol", 2.0, 2.5, 0.2, 10.0, true),
        ("gpt-6-luna", 0.1, 0.125, 0.01, 0.5, false),
    ];

    for (id, input, write, read, output, ultra) in cases {
        let resolved = resolve_model_pricing(id, &config);
        let pricing = resolved.pricing.expect("catalog pricing");
        assert_eq!(resolved.source, PricingSource::Exact, "source for {id}");
        assert_eq!(pricing.input_per_million, input, "input for {id}");
        assert_eq!(
            pricing.cache_write_per_million,
            Some(write),
            "write for {id}"
        );
        assert_eq!(pricing.cached_input_per_million, read, "read for {id}");
        assert_eq!(pricing.output_per_million, output, "output for {id}");

        let model = resolve_model(id).expect("catalog model");
        assert!(model.credit_rates().is_none(), "credits for {id}");
        assert_eq!(model.fast_usage_multiplier(), Some(2.0), "Fast for {id}");
        let long = model.long_context_pricing().expect("long-context pricing");
        assert_eq!(long.input_and_cache, 2.0);
        assert_eq!(long.output, 1.5);
        assert_eq!(model.supports_effort(ReasoningEffort::Ultra), ultra);
        assert!(model.supports_effort(ReasoningEffort::Max));
        assert!(!model.supports_effort(ReasoningEffort::Minimal));

        let context = model.context().expect("catalog context");
        assert_eq!(context.api_tokens, Some(1_050_000));
        assert_eq!(context.max_input_tokens, Some(922_000));
        assert_eq!(context.max_output_tokens, Some(128_000));
        assert_eq!(context.long_context_input_threshold, Some(272_000));
        assert!(context.raw_tokens < 1_050_000, "Codex fallback for {id}");
    }
}

#[test]
fn gpt_6_1_sol_context_prefers_observed_then_local_cache_then_bundle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("models_cache.json");
    fs::write(
        &cache,
        r#"{"models":[{"slug":"gpt-6.1-sol","context_window":272000,"effective_context_window_percent":95}]}"#,
    )
    .expect("cache fixture");

    let observed = resolve_context_window_from_cache_path("gpt-6.1-sol", Some(475_000), &cache)
        .expect("observed context");
    assert_eq!(observed.effective_tokens, 475_000);
    assert_eq!(observed.source, ContextSource::ObservedJsonl);

    let local =
        resolve_context_window_from_cache_path("gpt-6.1-sol", None, &cache).expect("local context");
    assert_eq!(local.raw_tokens, 272_000);
    assert_eq!(local.effective_tokens, 258_400);
    assert_eq!(local.source, ContextSource::LocalModelCache);

    fs::write(&cache, r#"{"models":[]}"#).expect("empty cache");
    let bundled = resolve_context_window_from_cache_path("gpt-6.1-sol", None, &cache)
        .expect("bundled context");
    assert_eq!(bundled.raw_tokens, 500_000);
    assert_eq!(bundled.effective_tokens, 475_000);
    assert_eq!(bundled.source, ContextSource::BundledCatalog);
}

#[test]
fn gpt_6_1_terra_catalog_entry_matches_the_published_contract() {
    let terra = resolve_model("gpt-6.1-terra").expect("Terra catalog entry");
    assert_eq!(terra.canonical_id(), "gpt-6.1-terra");
    assert_eq!(terra.display_name(), "GPT-6.1 Terra");
    assert!(terra.supports_fast());
    assert_eq!(terra.fast_usage_multiplier(), Some(2.0));
    assert!(terra.credit_rates().is_none());
    for effort in [
        ReasoningEffort::Low,
        ReasoningEffort::Medium,
        ReasoningEffort::High,
        ReasoningEffort::XHigh,
        ReasoningEffort::Max,
        ReasoningEffort::Ultra,
    ] {
        assert!(terra.supports_effort(effort), "{effort:?}");
    }
    assert!(!terra.supports_effort(ReasoningEffort::Minimal));

    let context = terra.context().expect("Terra context");
    assert_eq!(context.raw_tokens, 500_000);
    assert_eq!(context.effective_percent, 95);
    assert_eq!(
        context.raw_tokens * u64::from(context.effective_percent) / 100,
        475_000
    );
    assert_eq!(context.api_tokens, Some(1_050_000));
    assert_eq!(context.max_input_tokens, Some(922_000));
    assert_eq!(context.max_output_tokens, Some(128_000));
    assert_eq!(context.long_context_input_threshold, Some(272_000));

    let rates = terra.api_rates().expect("Terra pricing");
    assert_eq!(rates.input_per_million, 1.0);
    assert_eq!(rates.cache_write_per_million, Some(1.25));
    assert_eq!(rates.cached_input_per_million, 0.05);
    assert_eq!(rates.output_per_million, 5.0);
    let long = terra.long_context_pricing().expect("Terra long context");
    assert_eq!(long.input_and_cache, 2.0);
    assert_eq!(long.output, 1.5);
}

#[test]
fn gpt_6_1_terra_context_prefers_observed_then_local_cache_then_bundle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = dir.path().join("models_cache.json");
    fs::write(
        &cache,
        r#"{"models":[{"slug":"gpt-6.1-terra","context_window":500000,"effective_context_window_percent":95}]}"#,
    )
    .expect("cache fixture");

    let observed = resolve_context_window_from_cache_path("gpt-6.1-terra", Some(475_000), &cache)
        .expect("observed context");
    assert_eq!(observed.effective_tokens, 475_000);
    assert_eq!(observed.raw_tokens, 500_000);
    assert_eq!(observed.source, ContextSource::ObservedJsonl);
    assert_eq!(observed.raw_source, ContextSource::LocalModelCache);

    let local = resolve_context_window_from_cache_path("gpt-6.1-terra", None, &cache)
        .expect("local context");
    assert_eq!(local.raw_tokens, 500_000);
    assert_eq!(local.effective_tokens, 475_000);
    assert_eq!(local.source, ContextSource::LocalModelCache);

    fs::write(&cache, r#"{"models":[]}"#).expect("empty cache");
    let bundled = resolve_context_window_from_cache_path("gpt-6.1-terra", None, &cache)
        .expect("bundled context");
    assert_eq!(bundled.raw_tokens, 500_000);
    assert_eq!(bundled.effective_tokens, 475_000);
    assert_eq!(bundled.source, ContextSource::BundledCatalog);
    assert_eq!(bundled.raw_source, ContextSource::BundledCatalog);
}

#[test]
fn gpt_6_1_terra_fast_doubles_every_component() {
    let usage = TokenUsage {
        input_tokens: 100_000,
        cached_input_tokens: 20_000,
        cache_write_tokens: Some(10_000),
        output_tokens: 10_000,
    };
    let config = PricingConfig::default();
    let standard = compute_cost(
        "gpt-6.1-terra",
        usage,
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(standard.status, PricingStatus::Exact);
    assert!((standard.total_cost_usd - 0.1335).abs() < 1e-9);

    let fast = compute_cost(
        "gpt-6.1-terra",
        usage,
        SessionSpeed::explicit(SpeedMode::Fast, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(fast.status, PricingStatus::Exact);
    assert!((fast.total_cost_usd - 0.267).abs() < 1e-9);
}

#[test]
fn gpt_6_1_terra_requests_above_272k_input_double_input_and_cache_and_scale_output() {
    let config = PricingConfig::default();
    let standard = SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings);
    let plain = |input_tokens: u64| TokenUsage {
        input_tokens,
        cached_input_tokens: 0,
        cache_write_tokens: Some(0),
        output_tokens: 10_000,
    };

    let at_threshold = compute_cost_for_request(
        "gpt-6.1-terra",
        plain(272_000),
        standard,
        &config,
        Some(272_000),
    );
    assert_eq!(at_threshold.status, PricingStatus::Exact);
    assert!((at_threshold.total_cost_usd - 0.322).abs() < 1e-9);

    let above = compute_cost_for_request(
        "gpt-6.1-terra",
        plain(272_001),
        standard,
        &config,
        Some(272_001),
    );
    assert_eq!(above.status, PricingStatus::Exact);
    assert!((above.total_cost_usd - 0.619002).abs() < 1e-9);

    let mixed = compute_cost_for_request(
        "gpt-6.1-terra",
        TokenUsage {
            input_tokens: 300_000,
            cached_input_tokens: 100_000,
            cache_write_tokens: Some(50_000),
            output_tokens: 10_000,
        },
        standard,
        &config,
        Some(300_000),
    );
    assert_eq!(mixed.status, PricingStatus::Exact);
    assert!((mixed.total_cost_usd - 0.51).abs() < 1e-9);

    let cumulative = compute_cost("gpt-6.1-terra", plain(272_001), standard, &config);
    assert_eq!(cumulative.status, PricingStatus::Partial);
}

#[test]
fn every_standard_api_rate_matches_the_openai_pricing_table_of_2026_11_07() {
    let config = PricingConfig::default();
    let rates = [
        ("gpt-6-astra", 10.0, 1.0, Some(12.5), 50.0),
        ("gpt-6.1-sol", 2.0, 0.1, Some(2.5), 10.0),
        ("gpt-6.1-terra", 1.0, 0.05, Some(1.25), 5.0),
        ("gpt-6-luna", 0.1, 0.01, Some(0.125), 0.5),
        ("gpt-6-sol", 2.0, 0.2, Some(2.5), 10.0),
        ("gpt-5.6-sol", 4.0, 0.4, Some(5.0), 20.0),
        ("gpt-5.6-terra", 2.0, 0.2, Some(2.5), 12.0),
        ("gpt-5.6-luna", 0.2, 0.02, Some(0.25), 1.2),
        ("gpt-5.5", 5.0, 0.5, None, 30.0),
        ("gpt-5.4", 2.5, 0.25, None, 15.0),
        ("gpt-5.4-mini", 0.75, 0.075, None, 4.5),
        ("gpt-5.3-codex", 1.75, 0.175, None, 14.0),
        ("gpt-5.2", 1.75, 0.175, None, 14.0),
        ("gpt-5.1", 1.25, 0.125, None, 10.0),
        ("gpt-5", 1.25, 0.125, None, 10.0),
        ("gpt-5-mini", 0.25, 0.025, None, 2.0),
        ("gpt-5-nano", 0.05, 0.005, None, 0.4),
        ("gpt-5.6-cyber", 12.5, 1.25, Some(15.625), 75.0),
    ];
    for (id, input, cached, write, output) in rates {
        let resolved = resolve_model_pricing(id, &config);
        let pricing = resolved.pricing.expect("catalog pricing");
        assert_ne!(
            resolved.source,
            PricingSource::Unavailable,
            "source for {id}"
        );
        assert_eq!(pricing.input_per_million, input, "input for {id}");
        assert_eq!(pricing.cached_input_per_million, cached, "cached for {id}");
        assert_eq!(pricing.cache_write_per_million, write, "write for {id}");
        assert_eq!(pricing.output_per_million, output, "output for {id}");
    }
}

#[test]
fn long_context_rates_match_the_openai_pricing_table_of_2026_11_07() {
    let config = PricingConfig::default();
    let standard = SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings);
    let long_rates = [
        ("gpt-6-astra", 20.0, 2.0, 75.0),
        ("gpt-6.1-sol", 4.0, 0.2, 15.0),
        ("gpt-6.1-terra", 2.0, 0.1, 7.5),
        ("gpt-6-luna", 0.2, 0.02, 0.75),
        ("gpt-6-sol", 4.0, 0.4, 15.0),
        ("gpt-5.6-sol", 8.0, 0.8, 30.0),
        ("gpt-5.6-terra", 4.0, 0.4, 18.0),
        ("gpt-5.6-luna", 0.4, 0.04, 1.8),
    ];
    for (id, input, cached, output) in long_rates {
        let usage = TokenUsage {
            input_tokens: 300_000,
            cached_input_tokens: 100_000,
            cache_write_tokens: Some(0),
            output_tokens: 100_000,
        };
        let computed = compute_cost_for_request(id, usage, standard, &config, Some(300_000));
        let expected = 0.2 * input + 0.1 * cached + 0.1 * output;
        assert!(
            (computed.total_cost_usd - expected).abs() < 1e-9,
            "long-context cost for {id}: {} vs {expected}",
            computed.total_cost_usd
        );
        let threshold = resolve_model(id)
            .and_then(|model| model.context())
            .and_then(|context| context.long_context_input_threshold);
        assert_eq!(threshold, Some(272_000), "threshold for {id}");
    }

    // GPT-5.5 and GPT-5.4 publish long-context API rates, but their Codex context is 272,000 tokens, the
    // threshold itself, so no Codex request reaches the long tier.
    for id in [
        "gpt-5.5",
        "gpt-5.4",
        "gpt-5.4-mini",
        "gpt-5.3-codex",
        "gpt-5.2",
        "gpt-5.1",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-5.6-cyber",
    ] {
        let model = resolve_model(id).expect("catalog model");
        assert!(model.long_context_pricing().is_none(), "no tier for {id}");
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
            input_tokens: 25_000,
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
fn public_cost_presentation_uses_currency_only_for_known_subtotals() {
    assert_eq!(
        format_presentable_cost(Some(0.0065), PricingStatus::Exact),
        Some("$0.01".to_string())
    );
    assert_eq!(
        format_presentable_cost(Some(0.0065), PricingStatus::Partial),
        Some("$0.01".to_string())
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
        input_tokens: 100_000,
        output_tokens: 10_000,
        ..TokenUsage::default()
    };
    let known_fast = compute_cost(
        "gpt-5.5",
        usage,
        SessionSpeed::explicit(SpeedMode::Fast, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(known_fast.status, PricingStatus::Exact);
    assert_eq!(known_fast.known_total_cost_usd, Some(2.0));

    let unpublished_fast = compute_cost(
        "gpt-5.6-sol",
        usage,
        SessionSpeed::explicit(SpeedMode::Fast, SpeedSource::ThreadSettings),
        &config,
    );
    assert_eq!(unpublished_fast.status, PricingStatus::Partial);
    let known = unpublished_fast
        .known_total_cost_usd
        .expect("known subtotal");
    assert!(
        (known - 0.6).abs() < 1e-9,
        "100k input at $4 + 10k output at $20"
    );
    assert_eq!(unpublished_fast.source, PricingSource::Exact);
}

#[test]
fn cache_write_component_reconciles_with_known_subtotal() {
    // Below the 272,000-token long-context threshold, so every component is exact;
    // 100,000 cache-write tokens x $2.50 per million = $0.25.
    let computed = compute_cost(
        "gpt-5.6-terra",
        TokenUsage {
            input_tokens: 200_000,
            cached_input_tokens: 20_000,
            cache_write_tokens: Some(100_000),
            output_tokens: 10_000,
        },
        SessionSpeed::explicit(SpeedMode::Standard, SpeedSource::ThreadSettings),
        &PricingConfig::default(),
    );
    assert_eq!(computed.status, PricingStatus::Exact);
    assert_eq!(computed.breakdown.cache_write_cost_usd, 0.25);
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
