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
