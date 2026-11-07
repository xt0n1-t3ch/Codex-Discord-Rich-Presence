//! Files that Codex Discord Rich Presence owns. They live in one folder,
//! `~/.codex-discord-presence` (or `CODEX_PRESENCE_HOME`); the Codex home is
//! only read.
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The folder for every file this program writes.
pub fn home() -> PathBuf {
    std::env::var_os("CODEX_PRESENCE_HOME")
        .filter(|value| !value.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".codex-discord-presence")
        })
}

pub fn config_path() -> PathBuf {
    home().join("config.json")
}

pub fn plan_cache_path() -> PathBuf {
    home().join("plan-cache.json")
}

pub fn lock_path() -> PathBuf {
    home().join("instance.lock")
}

pub fn instance_meta_path() -> PathBuf {
    home().join("instance.json")
}

/// Files that releases up to 1.11 kept in the Codex home, and their place now.
const MOVED: &[(&str, &str)] = &[
    ("discord-presence-config.json", "config.json"),
    ("discord-presence-plan-cache.json", "plan-cache.json"),
];

/// Files of releases up to 1.11 with no successor.
const OBSOLETE: &[&str] = &[
    "discord-presence-metrics.json",
    "discord-presence-metrics.md",
    "codex-discord-presence.lock",
    "codex-discord-presence.instance.json",
];

/// Moves this program's files out of the Codex home. A file already in the
/// new folder wins, and its older copy is removed.
pub fn migrate_from(codex_home: &Path) -> Result<()> {
    let home = home();
    for (old, new) in MOVED {
        let old = codex_home.join(old);
        if !old.is_file() {
            continue;
        }
        let new = home.join(new);
        if !new.exists() {
            fs::create_dir_all(&home)
                .with_context(|| format!("failed to create {}", home.display()))?;
            // A rename fails across file systems; a copy is enough there.
            if fs::rename(&old, &new).is_err() {
                fs::copy(&old, &new)
                    .with_context(|| format!("failed to move {}", old.display()))?;
            }
        }
        let _ = fs::remove_file(&old);
    }
    for name in OBSOLETE {
        // A lock still held by an older running instance cannot be removed on
        // Windows; the next start retries.
        let _ = fs::remove_file(codex_home.join(name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_leave_the_codex_home_and_the_newer_copy_wins() {
        let _lock = crate::config::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let codex = root.path().join("codex-home");
        let own = root.path().join("presence-home");
        fs::create_dir_all(&codex).unwrap();
        fs::create_dir_all(&own).unwrap();
        fs::write(codex.join("discord-presence-config.json"), "old config").unwrap();
        fs::write(codex.join("discord-presence-plan-cache.json"), "old plan").unwrap();
        fs::write(own.join("plan-cache.json"), "newer plan").unwrap();
        fs::write(codex.join("discord-presence-metrics.md"), "x").unwrap();
        fs::write(codex.join("codex-discord-presence.lock"), "").unwrap();
        fs::write(codex.join("auth.json"), "codex owns this").unwrap();
        unsafe { std::env::set_var("CODEX_PRESENCE_HOME", &own) };

        migrate_from(&codex).unwrap();
        unsafe { std::env::remove_var("CODEX_PRESENCE_HOME") };

        assert_eq!(
            fs::read_to_string(own.join("config.json")).unwrap(),
            "old config"
        );
        assert_eq!(
            fs::read_to_string(own.join("plan-cache.json")).unwrap(),
            "newer plan"
        );
        let left: Vec<String> = fs::read_dir(&codex)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["auth.json"], "only Codex's own file stays");
    }
}
