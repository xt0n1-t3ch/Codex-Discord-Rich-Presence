//! Helpers for running child processes with readable errors.

use std::process::{Command, Output};

use anyhow::{Context, Result, ensure};

pub fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

pub fn captured(command: &mut Command) -> Result<Output> {
    command
        .output()
        .with_context(|| format!("starting {:?}", command.get_program()))
}

pub fn failure_text(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        stderr.to_string()
    }
}

pub fn inherited(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("starting {label}"))?;
    ensure!(status.success(), "{label} failed ({status})");
    Ok(())
}
