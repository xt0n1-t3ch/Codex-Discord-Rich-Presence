use codex_discord_presence::config::{
    ActivitySmallImageKeys, DEFAULT_DISCORD_CLIENT_ID, DEFAULT_DISCORD_DESKTOP_CLIENT_ID,
    DesktopPresenceDesign, DisplayConfig, PresenceConfig, PrivacyField,
};
use std::fs;
use tempfile::tempdir;

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

#[test]
fn schema_11_migrates_to_enabled_shared_presence_without_changing_preferences() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("discord-presence-config.json");
    let mut legacy = serde_json::to_value(PresenceConfig::default()).expect("serialize config");
    let object = legacy.as_object_mut().expect("config object");
    object.insert("schema_version".to_string(), serde_json::json!(11));
    object.remove("presence_enabled");
    object
        .get_mut("privacy")
        .and_then(serde_json::Value::as_object_mut)
        .expect("privacy object")
        .insert("show_git_branch".to_string(), serde_json::json!(false));
    object
        .get_mut("display")
        .and_then(serde_json::Value::as_object_mut)
        .expect("display object")
        .insert(
            "desktop_presence_design".to_string(),
            serde_json::json!("chat_gpt_app"),
        );
    fs::write(
        &path,
        serde_json::to_vec_pretty(&legacy).expect("serialize legacy config"),
    )
    .expect("write legacy config");

    let mut runtime = PresenceConfig {
        presence_enabled: false,
        ..PresenceConfig::default()
    };

    assert!(runtime.reload_from_path(&path));
    assert_eq!(runtime.schema_version, 13);
    assert!(runtime.presence_enabled);
    assert!(!runtime.privacy.show_git_branch);
    assert!(runtime.privacy.show_credits);
    assert_eq!(runtime.display.presence_layout.fields.len(), 10);
    assert!(
        runtime
            .display
            .presence_layout
            .fields
            .iter()
            .any(
                |item| item.field == codex_presence_core::PresenceFieldId::Credits && item.enabled
            )
    );
    assert_eq!(
        runtime.display.desktop_presence_design,
        DesktopPresenceDesign::ChatGptApp
    );

    let persisted: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("read migrated config"))
            .expect("parse migrated config");
    assert_eq!(persisted["schema_version"], 13);
    assert_eq!(persisted["presence_enabled"], true);
    assert_eq!(persisted["privacy"]["show_credits"], true);
}

#[test]
fn runtime_reload_applies_external_controls_and_keeps_last_good_on_invalid_replacement() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("discord-presence-config.json");
    let mut runtime = PresenceConfig::default();

    let mut external = PresenceConfig {
        presence_enabled: false,
        ..PresenceConfig::default()
    };
    external.privacy.enabled = true;
    external.privacy.show_activity_target = false;
    for field in PrivacyField::ALL {
        field.toggle(&mut external.privacy);
    }
    external.display.desktop_presence_design = DesktopPresenceDesign::ChatGptApp;
    fs::write(
        &path,
        serde_json::to_vec_pretty(&external).expect("serialize external config"),
    )
    .expect("write external config");

    assert!(runtime.reload_from_path(&path));
    assert!(!runtime.presence_enabled);
    assert!(runtime.privacy.enabled);
    assert!(!runtime.privacy.show_activity_target);
    for field in PrivacyField::ALL {
        assert!(!field.is_enabled(&runtime.privacy), "{}", field.label());
    }
    assert_eq!(
        runtime.display.desktop_presence_design,
        DesktopPresenceDesign::ChatGptApp
    );

    fs::write(&path, b"{ replaced while incomplete").expect("replace with invalid config");

    assert!(!runtime.reload_from_path(&path));
    assert!(!runtime.presence_enabled);
    assert!(runtime.privacy.enabled);
    assert!(!runtime.privacy.show_activity_target);
    for field in PrivacyField::ALL {
        assert!(!field.is_enabled(&runtime.privacy), "{}", field.label());
    }
    assert_eq!(
        runtime.display.desktop_presence_design,
        DesktopPresenceDesign::ChatGptApp
    );

    fs::remove_file(&path).expect("remove replaced config");

    assert!(!runtime.reload_from_path(&path));
    assert!(!runtime.presence_enabled);
    assert_eq!(
        runtime.display.desktop_presence_design,
        DesktopPresenceDesign::ChatGptApp
    );
}

#[test]
fn master_presence_toggle_persists_through_the_shared_config_boundary() {
    let directory = tempdir().expect("temp directory");
    let path = directory.path().join("discord-presence-config.json");
    let mut runtime = PresenceConfig::default();
    fs::write(
        &path,
        serde_json::to_vec_pretty(&runtime).expect("serialize config"),
    )
    .expect("write config");

    runtime
        .toggle_presence_at_path(&path)
        .expect("persist pause");
    assert!(!runtime.presence_enabled);
    let paused: PresenceConfig =
        serde_json::from_slice(&fs::read(&path).expect("read paused config"))
            .expect("parse paused config");
    assert!(!paused.presence_enabled);

    runtime
        .toggle_presence_at_path(&path)
        .expect("persist resume");
    assert!(runtime.presence_enabled);
    let resumed: PresenceConfig =
        serde_json::from_slice(&fs::read(path).expect("read resumed config"))
            .expect("parse resumed config");
    assert!(resumed.presence_enabled);
}
