//! Smoke test of a built executable against a throwaway Codex home.

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
    let codex_home = base.join(format!("codex-home-{}", process::id()));
    fs::create_dir_all(codex_home.join("sessions"))?;
    let mut outcome = Ok(());
    for argument in COMMANDS {
        let mut command = Command::new(&artifact);
        command.arg(argument).env("CODEX_HOME", &codex_home);
        outcome = exec::inherited(&mut command, &format!("{argument} smoke"));
        if outcome.is_err() {
            break;
        }
    }
    let _ = fs::remove_dir_all(&codex_home);
    outcome
}
