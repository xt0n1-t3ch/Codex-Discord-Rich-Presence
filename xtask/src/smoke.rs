//! Smoke test of a built executable against a throwaway Codex home and presence folder.

use std::env;
use std::fs;
use std::path::{self, Path, PathBuf};
use std::process::{self, Command};

use anyhow::Result;

use crate::exec;

pub const COMMANDS: [&str; 2] = ["--version", "doctor"];

pub fn run(artifact: &Path) -> Result<()> {
    let artifact = path::absolute(artifact)?;
    let base = env::var_os("RUNNER_TEMP").map_or_else(env::temp_dir, PathBuf::from);
    let root = base.join(format!("codex-presence-smoke-{}", process::id()));
    let homes = isolated_homes(&root);
    fs::create_dir_all(homes[0].1.join("sessions"))?;
    let mut outcome = Ok(());
    for argument in COMMANDS {
        let mut command = Command::new(&artifact);
        command
            .arg(argument)
            .envs(homes.iter().map(|(name, dir)| (*name, dir)));
        outcome = exec::inherited(&mut command, &format!("{argument} smoke"));
        if outcome.is_err() {
            break;
        }
    }
    let _ = fs::remove_dir_all(&root);
    outcome
}

/// Every folder the executable reads or writes, all under `root`, so a smoke
/// run never touches the user's Codex home or presence folder.
fn isolated_homes(root: &Path) -> [(&'static str, PathBuf); 2] {
    [
        ("CODEX_HOME", root.join("codex-home")),
        ("CODEX_PRESENCE_HOME", root.join("presence-home")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_runs_isolate_both_the_codex_home_and_the_presence_home() {
        let root = Path::new("/tmp/smoke-root");
        let homes = isolated_homes(root);
        let names: Vec<_> = homes.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["CODEX_HOME", "CODEX_PRESENCE_HOME"]);
        assert!(homes.iter().all(|(_, dir)| dir.starts_with(root)));
    }
}
