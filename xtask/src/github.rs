//! Thin wrapper over `gh api` for the GitHub REST calls the release commands need.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::exec;

const API_VERSION: &str = "2026-03-10";

fn gh_api() -> Command {
    let mut command = Command::new("gh");
    command
        .arg("api")
        .args(["-H", "Accept: application/vnd.github+json", "-H"])
        .arg(format!("X-GitHub-Api-Version: {API_VERSION}"));
    command
}

pub fn get(endpoint: &str, allow_not_found: bool) -> Result<Option<Value>> {
    let output = exec::captured(gh_api().arg(endpoint))?;
    if !output.status.success() {
        let text = exec::failure_text(&output);
        if allow_not_found && text.contains("HTTP 404") {
            return Ok(None);
        }
        bail!("GitHub API request '{endpoint}' failed: {text}");
    }
    let value = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("parsing the response of '{endpoint}'"))?;
    Ok(Some(value))
}

pub fn send(method: &str, endpoint: &str, body: &Value) -> Result<()> {
    let mut child = gh_api()
        .args(["--method", method])
        .arg(endpoint)
        .args(["--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("starting gh")?;
    child
        .stdin
        .take()
        .context("opening the stdin of gh")?
        .write_all(body.to_string().as_bytes())
        .context("sending the request body to gh")?;
    let output = child.wait_with_output().context("waiting for gh")?;
    if !output.status.success() {
        bail!(
            "GitHub API {method} '{endpoint}' failed: {}",
            exec::failure_text(&output)
        );
    }
    Ok(())
}
