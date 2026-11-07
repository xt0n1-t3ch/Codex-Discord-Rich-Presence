//! Model labels for standard and Fast mode across the GPT model families.

use codex_discord_presence::model::{ReasoningEffort, format_model_display};

#[test]
fn gpt_5_5_has_standard_and_fast_labels() {
    assert_eq!(format_model_display("gpt-5.5", None, false), "GPT-5.5");
    assert_eq!(
        format_model_display("gpt-5.5", None, true),
        "GPT-5.5 · ⚡ Fast"
    );
}

#[test]
fn gpt_5_4_family_has_fast_labels() {
    assert_eq!(
        format_model_display("gpt-5.4", None, true),
        "GPT-5.4 · ⚡ Fast"
    );
    assert_eq!(
        format_model_display("gpt-5.4-mini", None, true),
        "GPT-5.4 Mini"
    );
}

#[test]
fn gpt_5_6_display_matches_codex_app_labels() {
    assert_eq!(
        format_model_display("gpt-5.6", Some(ReasoningEffort::Max), false),
        "GPT-5.6 Sol · Max"
    );
    assert_eq!(
        format_model_display("gpt-5.6-sol", Some(ReasoningEffort::Max), true),
        "GPT-5.6 Sol · Max · ⚡ Fast"
    );
    assert_eq!(
        format_model_display("gpt-5.6-terra", Some(ReasoningEffort::Low), false),
        "GPT-5.6 Terra · Light"
    );
    assert_eq!(
        format_model_display("gpt-5.6-luna", Some(ReasoningEffort::Ultra), false),
        "GPT-5.6 Luna"
    );
}

#[test]
fn gpt_6_family_has_standard_and_fast_labels() {
    assert_eq!(
        format_model_display("gpt-6.1-sol", Some(ReasoningEffort::Medium), false),
        "GPT-6.1 Sol · Medium"
    );
    assert_eq!(
        format_model_display("gpt-6.1-sol-fast", Some(ReasoningEffort::Ultra), true),
        "GPT-6.1 Sol · Ultra · ⚡ Fast"
    );
    assert_eq!(
        format_model_display("gpt-6-sol", Some(ReasoningEffort::Max), false),
        "GPT-6 Sol · Max"
    );
    assert_eq!(
        format_model_display("gpt-6-sol", None, true),
        "GPT-6 Sol · ⚡ Fast"
    );
    assert_eq!(
        format_model_display("gpt-6-luna", Some(ReasoningEffort::Low), false),
        "GPT-6 Luna · Light"
    );
    assert_eq!(
        format_model_display("gpt-6-luna", Some(ReasoningEffort::Ultra), true),
        "GPT-6 Luna · ⚡ Fast"
    );
}

#[test]
fn gpt_6_1_terra_has_standard_fast_and_ultra_labels() {
    assert_eq!(
        format_model_display("gpt-6.1-terra", None, false),
        "GPT-6.1 Terra"
    );
    assert_eq!(
        format_model_display("gpt-6.1-terra", Some(ReasoningEffort::Low), false),
        "GPT-6.1 Terra · Light"
    );
    assert_eq!(
        format_model_display("gpt-6.1-terra", Some(ReasoningEffort::Max), false),
        "GPT-6.1 Terra · Max"
    );
    assert_eq!(
        format_model_display("gpt-6.1-terra", None, true),
        "GPT-6.1 Terra · ⚡ Fast"
    );
    assert_eq!(
        format_model_display("gpt-6.1-terra", Some(ReasoningEffort::Ultra), false),
        "GPT-6.1 Terra · Ultra"
    );
    assert_eq!(
        format_model_display("gpt-6.1-terra-fast", Some(ReasoningEffort::Ultra), true),
        "GPT-6.1 Terra · Ultra · ⚡ Fast"
    );
}

#[test]
fn daybreak_models_have_cyber_labels_and_never_invent_fast() {
    assert_eq!(
        format_model_display("gpt-daybreak-blue", Some(ReasoningEffort::High), false,),
        "GPT-5.6-Cyber-Blue · High"
    );
    assert_eq!(
        format_model_display(
            "gpt-daybreak-blue-latest-fast",
            Some(ReasoningEffort::High),
            true,
        ),
        "GPT-5.6-Cyber-Blue · High"
    );
    assert_eq!(
        format_model_display(
            "gpt-daybreak-red-latest",
            Some(ReasoningEffort::Ultra),
            true,
        ),
        "GPT-5.6-Cyber-Red · Ultra"
    );
}
