use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_STALE_SECONDS: u64 = 90;
const DEFAULT_POLL_SECONDS: u64 = 2;
const DEFAULT_ACTIVE_STICKY_SECONDS: u64 = 3600;
const MIN_ACTIVE_STICKY_SECONDS: u64 = 60;
const CONFIG_SCHEMA_VERSION: u32 = 3;
pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1470480085453770854";
pub const DEFAULT_DISCORD_PUBLIC_KEY: &str =
    "29e563eeb755ae71d940c1b11d49dd3282a8886cd8b8cab829b2a14fcedad247";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PresenceConfig {
    pub schema_version: u32,
    pub discord_client_id: Option<String>,
    pub discord_public_key: Option<String>,
    pub privacy: PrivacyConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacyConfig {
    pub enabled: bool,
    pub show_project_name: bool,
    pub show_git_branch: bool,
    pub show_model: bool,
    pub show_tokens: bool,
    pub show_limits: bool,
    pub show_activity: bool,
    pub show_activity_target: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalLogoMode {
    #[default]
    Auto,
    Ascii,
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
    pub large_image_key: String,
    pub large_text: String,
    pub small_image_key: String,
    pub small_text: String,
    pub activity_small_image_keys: ActivitySmallImageKeys,
    pub terminal_logo_mode: TerminalLogoMode,
    pub terminal_logo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ActivitySmallImageKeys {
    pub thinking: Option<String>,
    pub reading: Option<String>,
    pub editing: Option<String>,
    pub running: Option<String>,
    pub waiting: Option<String>,
    pub idle: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub stale_threshold: Duration,
    pub active_sticky_window: Duration,
    pub poll_interval: Duration,
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            discord_client_id: Some(DEFAULT_DISCORD_CLIENT_ID.to_string()),
            discord_public_key: Some(DEFAULT_DISCORD_PUBLIC_KEY.to_string()),
            privacy: PrivacyConfig::default(),
            display: DisplayConfig::default(),
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_project_name: true,
            show_git_branch: true,
            show_model: true,
            show_tokens: true,
            show_limits: true,
            show_activity: true,
            show_activity_target: true,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            large_image_key: "codex-logo".to_string(),
            large_text: "Codex".to_string(),
            small_image_key: "openai".to_string(),
            small_text: "OpenAI".to_string(),
            activity_small_image_keys: ActivitySmallImageKeys::default(),
            terminal_logo_mode: TerminalLogoMode::Auto,
            terminal_logo_path: None,
        }
    }
}

impl PresenceConfig {
    pub fn load_or_init() -> Result<Self> {
        let cfg_path = config_path();
        if let Some(parent) = cfg_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }

        if cfg_path.exists() {
            let raw = fs::read_to_string(&cfg_path)
                .with_context(|| format!("failed to read {}", cfg_path.display()))?;
            let mut parsed: PresenceConfig = serde_json::from_str(&raw)
                .with_context(|| format!("invalid JSON in {}", cfg_path.display()))?;
            if parsed.normalize_and_migrate() {
                parsed.save()?;
            }
            Ok(parsed)
        } else {
            let cfg = PresenceConfig::default();
            cfg.save()?;
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }

        let data = serde_json::to_string_pretty(self)?;
        fs::write(&path, data).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn effective_client_id(&self) -> Option<String> {
        let from_env = env::var("CODEX_DISCORD_CLIENT_ID")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        if from_env.is_some() {
            return from_env;
        }

        self.discord_client_id
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    fn normalize_and_migrate(&mut self) -> bool {
        let mut changed = false;

        if self.schema_version < CONFIG_SCHEMA_VERSION {
            self.schema_version = CONFIG_SCHEMA_VERSION;
            changed = true;
        }

        if is_missing(&self.discord_client_id) {
            self.discord_client_id = Some(DEFAULT_DISCORD_CLIENT_ID.to_string());
            changed = true;
        }
        if is_missing(&self.discord_public_key) {
            self.discord_public_key = Some(DEFAULT_DISCORD_PUBLIC_KEY.to_string());
            changed = true;
        }

        if self.display.large_image_key.trim().is_empty() {
            self.display.large_image_key = DisplayConfig::default().large_image_key;
            changed = true;
        }
        if self.display.large_text.trim().is_empty() {
            self.display.large_text = DisplayConfig::default().large_text;
            changed = true;
        }
        if self.display.small_image_key.trim().is_empty() {
            self.display.small_image_key = DisplayConfig::default().small_image_key;
            changed = true;
        }
        if self.display.small_text.trim().is_empty() {
            self.display.small_text = DisplayConfig::default().small_text;
            changed = true;
        }
        for item in [
            &mut self.display.activity_small_image_keys.thinking,
            &mut self.display.activity_small_image_keys.reading,
            &mut self.display.activity_small_image_keys.editing,
            &mut self.display.activity_small_image_keys.running,
            &mut self.display.activity_small_image_keys.waiting,
            &mut self.display.activity_small_image_keys.idle,
        ] {
            if normalize_optional_string(item) {
                changed = true;
            }
        }
        if self
            .display
            .terminal_logo_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            self.display.terminal_logo_path = None;
            changed = true;
        }

        changed
    }
}

pub fn runtime_settings() -> RuntimeSettings {
    let sticky_seconds = env_u64(
        "CODEX_PRESENCE_ACTIVE_STICKY_SECONDS",
        DEFAULT_ACTIVE_STICKY_SECONDS,
    )
    .max(MIN_ACTIVE_STICKY_SECONDS);
    RuntimeSettings {
        stale_threshold: Duration::from_secs(env_u64(
            "CODEX_PRESENCE_STALE_SECONDS",
            DEFAULT_STALE_SECONDS,
        )),
        active_sticky_window: Duration::from_secs(sticky_seconds),
        poll_interval: Duration::from_secs(env_u64(
            "CODEX_PRESENCE_POLL_SECONDS",
            DEFAULT_POLL_SECONDS,
        )),
    }
}

pub fn codex_home() -> PathBuf {
    if let Ok(custom) = env::var("CODEX_HOME") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
}

pub fn sessions_path() -> PathBuf {
    codex_home().join("sessions")
}

pub fn config_path() -> PathBuf {
    codex_home().join("discord-presence-config.json")
}

pub fn lock_path() -> PathBuf {
    codex_home().join("codex-discord-presence.lock")
}

pub fn instance_meta_path() -> PathBuf {
    codex_home().join("codex-discord-presence.instance.json")
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn is_missing(value: &Option<String>) -> bool {
    value.as_ref().map(|v| v.trim().is_empty()).unwrap_or(true)
}

fn normalize_optional_string(value: &mut Option<String>) -> bool {
    if let Some(item) = value.as_mut() {
        let trimmed = item.trim().to_string();
        if trimmed.is_empty() {
            *value = None;
            return true;
        }
        if *item != trimmed {
            *item = trimmed;
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_client_id_is_returned() {
        let cfg = PresenceConfig {
            discord_client_id: Some("from-config".to_string()),
            ..PresenceConfig::default()
        };
        assert_eq!(cfg.effective_client_id().as_deref(), Some("from-config"));
    }

    #[test]
    fn migration_sets_default_client_id_when_missing() {
        let mut cfg = PresenceConfig {
            schema_version: 2,
            discord_client_id: None,
            discord_public_key: None,
            privacy: PrivacyConfig::default(),
            display: DisplayConfig::default(),
        };

        let changed = cfg.normalize_and_migrate();

        assert!(changed);
        assert_eq!(cfg.schema_version, 3);
        assert_eq!(
            cfg.discord_client_id.as_deref(),
            Some(DEFAULT_DISCORD_CLIENT_ID)
        );
        assert_eq!(
            cfg.discord_public_key.as_deref(),
            Some(DEFAULT_DISCORD_PUBLIC_KEY)
        );
    }

    #[test]
    fn display_defaults_to_auto_logo_mode() {
        let cfg = PresenceConfig::default();
        assert_eq!(cfg.display.terminal_logo_mode, TerminalLogoMode::Auto);
        assert_eq!(cfg.display.terminal_logo_path, None);
    }
}
