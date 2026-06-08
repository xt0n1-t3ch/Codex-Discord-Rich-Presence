use codex_discord_presence::config::{
    ActivitySmallImageKeys, DEFAULT_DISCORD_CLIENT_ID, DEFAULT_DISCORD_DESKTOP_CLIENT_ID,
    DisplayConfig, PresenceConfig,
};

#[test]
fn non_codex_identity_is_rewritten_to_codex_identity() {
    let mut config = PresenceConfig {
        discord_client_id: Some("000000000000000000".to_string()),
        discord_client_id_desktop: Some("111111111111111111".to_string()),
        display: DisplayConfig {
            large_image_key: "old-code".to_string(),
            large_text: "Old Code".to_string(),
            desktop_large_image_key: "old-app".to_string(),
            desktop_large_text: "Old App".to_string(),
            small_image_key: "old".to_string(),
            small_text: "Old".to_string(),
            activity_small_image_keys: ActivitySmallImageKeys {
                thinking: Some("thinking".to_string()),
                reading: Some("reading".to_string()),
                editing: Some("editing".to_string()),
                running: Some("running".to_string()),
                waiting: Some("waiting".to_string()),
                idle: Some("idle".to_string()),
            },
            ..DisplayConfig::default()
        },
        ..PresenceConfig::default()
    };

    assert!(config.normalize_for_runtime());
    assert_eq!(
        config.discord_client_id.as_deref(),
        Some(DEFAULT_DISCORD_CLIENT_ID)
    );
    assert_eq!(
        config.discord_client_id_desktop.as_deref(),
        Some(DEFAULT_DISCORD_DESKTOP_CLIENT_ID)
    );
    assert_eq!(config.display.large_image_key, "codex-logo");
    assert_eq!(config.display.large_text, "Codex");
    assert_eq!(config.display.small_image_key, "openai");
    assert_eq!(config.display.small_text, "OpenAI");
    assert_eq!(config.display.activity_small_image_keys.thinking, None);
    assert_eq!(config.display.activity_small_image_keys.reading, None);
    assert_eq!(config.display.activity_small_image_keys.editing, None);
    assert_eq!(config.display.activity_small_image_keys.running, None);
    assert_eq!(config.display.activity_small_image_keys.waiting, None);
    assert_eq!(config.display.activity_small_image_keys.idle, None);
}
