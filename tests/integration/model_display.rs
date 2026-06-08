use codex_discord_presence::util::format_model_display;

#[test]
fn gpt_5_5_has_standard_and_fast_labels() {
    assert_eq!(format_model_display("gpt-5.5", None, false), "GPT-5.5");
    assert_eq!(format_model_display("gpt-5.5", None, true), "⚡ GPT-5.5");
}

#[test]
fn gpt_5_4_family_has_fast_labels() {
    assert_eq!(format_model_display("gpt-5.4", None, true), "⚡ GPT-5.4");
    assert_eq!(
        format_model_display("gpt-5.4-mini", None, true),
        "⚡ GPT-5.4-Mini"
    );
}
