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

/// The single-instance lock and its PID record of releases up to 1.11.
const LEGACY_LOCK: &str = "codex-discord-presence.lock";
const LEGACY_INSTANCE: &str = "codex-discord-presence.instance.json";

/// Files of releases up to 1.11 with no successor.
const OBSOLETE: &[&str] = &[
    "discord-presence-metrics.json",
    "discord-presence-metrics.md",
    LEGACY_LOCK,
    LEGACY_INSTANCE,
];

/// The legacy instance record in `codex_home`, kept for a takeover.
pub fn legacy_instance_path(codex_home: &Path) -> PathBuf {
    codex_home.join(LEGACY_INSTANCE)
}

/// `Some(pid)` while a release up to 1.11 still holds its lock in
/// `codex_home`; the PID comes from its record when that is readable. Such a
/// process runs alongside a new one unless both locks are honored.
pub fn legacy_holder(codex_home: &Path) -> Option<Option<u32>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(codex_home.join(LEGACY_LOCK))
        .ok()?;
    if file.try_lock().is_ok() {
        let _ = file.unlock();
        return None;
    }
    let pid = fs::read_to_string(legacy_instance_path(codex_home))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|record| record.get("pid")?.as_u64())
        .and_then(|pid| u32::try_from(pid).ok());
    Some(pid)
}

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
    // An older release that is still running keeps its lock and record, so
    // the single-instance check can see it; a later start removes them.
    let older_running = legacy_holder(codex_home).is_some();
    for name in OBSOLETE {
        if older_running && [LEGACY_LOCK, LEGACY_INSTANCE].contains(name) {
            continue;
        }
        let _ = fs::remove_file(codex_home.join(name));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_held_by_an_older_running_release_is_seen_and_kept() {
        let _lock = crate::config::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let codex = root.path().join("codex-home");
        let own = root.path().join("presence-home");
        fs::create_dir_all(&codex).unwrap();
        let legacy = codex.join(LEGACY_LOCK);
        fs::write(
            codex.join(LEGACY_INSTANCE),
            r#"{"pid":4242,"exe_path":null}"#,
        )
        .unwrap();
        let held = fs::File::create(&legacy).unwrap();
        held.lock().unwrap();
        unsafe { std::env::set_var("CODEX_PRESENCE_HOME", &own) };

        migrate_from(&codex).unwrap();
        unsafe { std::env::remove_var("CODEX_PRESENCE_HOME") };

        assert_eq!(legacy_holder(&codex), Some(Some(4242)));
        assert!(legacy.exists(), "a held lock is not removed");
        assert!(codex.join(LEGACY_INSTANCE).exists());

        held.unlock().unwrap();
        drop(held);
        assert_eq!(
            legacy_holder(&codex),
            None,
            "a released lock means no older instance"
        );
    }

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
