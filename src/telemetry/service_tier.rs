use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceTier {
    Fast,
    #[default]
    Standard,
}

impl ServiceTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Standard => "Standard",
        }
    }

    pub fn fast_mode_label(self) -> &'static str {
        match self {
            Self::Fast => "On",
            Self::Standard => "Off",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServiceTier {
    pub tier: ServiceTier,
    pub raw_tier: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
    pub source_path: Option<PathBuf>,
}

impl Default for ResolvedServiceTier {
    fn default() -> Self {
        Self {
            tier: ServiceTier::Standard,
            raw_tier: None,
            observed_at: None,
            source_path: None,
        }
    }
}

impl ResolvedServiceTier {
    pub fn is_fast(&self) -> bool {
        matches!(self.tier, ServiceTier::Fast)
    }

    pub fn fast_mode_label(&self) -> &'static str {
        self.tier.fast_mode_label()
    }
}

pub fn resolve_service_tier() -> ResolvedServiceTier {
    let config_path = config::codex_home().join("config.toml");
    resolve_service_tier_from_sources(&config_path, &config::global_state_paths())
}

fn resolve_service_tier_from_sources(
    config_path: &Path,
    global_state_paths: &[PathBuf],
) -> ResolvedServiceTier {
    load_service_tier_from_toml(config_path)
        .unwrap_or_else(|| resolve_service_tier_from_paths(global_state_paths))
}

fn resolve_service_tier_from_paths(paths: &[PathBuf]) -> ResolvedServiceTier {
    paths
        .iter()
        .filter_map(|path| load_service_tier_from_path(path))
        .max_by_key(|candidate| {
            candidate
                .observed_at
                .map(|value| value.timestamp_millis())
                .unwrap_or(i64::MIN)
        })
        .unwrap_or_default()
}

fn load_service_tier_from_path(path: &Path) -> Option<ResolvedServiceTier> {
    let modified = fs::metadata(path).ok()?.modified().ok();
    let raw = fs::read_to_string(path).ok()?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;
    let raw_tier = parsed
        .get("electron-persisted-atom-state")
        .and_then(|value| value.get("default-service-tier"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_ascii_lowercase();

    let tier = if raw_tier == "fast" {
        ServiceTier::Fast
    } else {
        ServiceTier::Standard
    };

    Some(ResolvedServiceTier {
        tier,
        raw_tier: Some(raw_tier),
        observed_at: modified.map(DateTime::<Utc>::from),
        source_path: Some(path.to_path_buf()),
    })
}

fn load_service_tier_from_toml(path: &Path) -> Option<ResolvedServiceTier> {
    let modified = fs::metadata(path).ok()?.modified().ok();
    let raw = fs::read_to_string(path).ok()?;
    let raw_tier = raw.lines().find_map(|line| {
        let line = line.split('#').next()?.trim();
        let (key, value) = line.split_once('=')?;
        if key.trim() != "service_tier" {
            return None;
        }
        let value = value.trim().trim_matches(['\'', '"']).to_ascii_lowercase();
        (!value.is_empty()).then_some(value)
    })?;
    let tier = match raw_tier.as_str() {
        "priority" | "fast" => ServiceTier::Fast,
        "default" | "standard" | "auto" => ServiceTier::Standard,
        _ => return None,
    };
    Some(ResolvedServiceTier {
        tier,
        raw_tier: Some(raw_tier),
        observed_at: modified.map(DateTime::<Utc>::from),
        source_path: Some(path.to_path_buf()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn defaults_to_standard_when_nothing_is_found() {
        let resolved = resolve_service_tier_from_paths(&[]);
        assert_eq!(resolved.tier, ServiceTier::Standard);
        assert!(!resolved.is_fast());
    }

    #[test]
    fn config_toml_priority_precedes_legacy_global_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config.toml");
        let state = dir.path().join("state.json");
        fs::write(&config, "service_tier = \"priority\"\n").expect("config");
        fs::write(
            &state,
            r#"{"electron-persisted-atom-state":{"default-service-tier":"standard"}}"#,
        )
        .expect("state");
        let resolved = resolve_service_tier_from_sources(&config, &[state]);
        assert_eq!(resolved.tier, ServiceTier::Fast);
        assert_eq!(resolved.raw_tier.as_deref(), Some("priority"));
    }

    #[test]
    fn reads_fast_service_tier_from_global_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".codex-global-state.json");
        fs::write(
            &path,
            r#"{"electron-persisted-atom-state":{"default-service-tier":"fast"}}"#,
        )
        .expect("write");

        let resolved = resolve_service_tier_from_paths(&[path]);
        assert_eq!(resolved.tier, ServiceTier::Fast);
        assert_eq!(resolved.raw_tier.as_deref(), Some("fast"));
        assert!(resolved.is_fast());
    }

    #[test]
    fn prefers_the_freshest_valid_global_state_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let older = dir.path().join("older.json");
        let newer = dir.path().join("newer.json");
        fs::write(
            &older,
            r#"{"electron-persisted-atom-state":{"default-service-tier":"standard"}}"#,
        )
        .expect("write older");
        std::thread::sleep(Duration::from_millis(10));
        fs::write(
            &newer,
            r#"{"electron-persisted-atom-state":{"default-service-tier":"fast"}}"#,
        )
        .expect("write newer");

        let resolved = resolve_service_tier_from_paths(&[older, newer]);
        assert_eq!(resolved.tier, ServiceTier::Fast);
        assert_eq!(resolved.raw_tier.as_deref(), Some("fast"));
    }
}
