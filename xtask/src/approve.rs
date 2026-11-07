//! Release approval: confirms GitHub immutable releases are on and records the approved commit SHA.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;

use crate::exec;
use crate::github;
use crate::text::{is_commit_sha, is_repository_slug, is_variable_name};

pub const DEFAULT_VARIABLE: &str = "RELEASE_APPROVED_SHA";

pub struct Request<'a> {
    pub repository: &'a str,
    pub sha: &'a str,
    pub variable_name: &'a str,
    pub api_fixture: Option<&'a Path>,
    pub write_variable: bool,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub repository: String,
    pub approved_sha: String,
    pub immutable_releases: bool,
    pub variable: String,
    pub variable_written: bool,
}

fn immutable_state(request: &Request) -> Result<Value> {
    match request.api_fixture {
        Some(path) => {
            if !path.is_file() {
                bail!("Immutable release fixture '{}' is missing.", path.display());
            }
            let text = fs::read_to_string(path)
                .with_context(|| format!("reading '{}'", path.display()))?;
            serde_json::from_str(&text).with_context(|| format!("parsing '{}'", path.display()))
        }
        None => github::get(
            &format!("repos/{}/immutable-releases", request.repository),
            false,
        )
        .and_then(|value| value.context("GitHub returned no immutable release state"))
        .map_err(|error| anyhow!("Unable to verify immutable releases: {error}")),
    }
}

fn write_variable(request: &Request) -> Result<()> {
    let name = request.variable_name;
    let set = exec::captured(Command::new("gh").args([
        "variable",
        "set",
        name,
        "--repo",
        request.repository,
        "--body",
        request.sha,
    ]))?;
    if !set.status.success() {
        bail!("Unable to set repository variable '{name}'.");
    }
    let get = exec::captured(Command::new("gh").args([
        "variable",
        "get",
        name,
        "--repo",
        request.repository,
    ]))?;
    let stored = String::from_utf8_lossy(&get.stdout).trim().to_string();
    if !get.status.success() || stored != request.sha {
        bail!("Repository variable '{name}' did not preserve the approved SHA.");
    }
    Ok(())
}

pub fn run(request: &Request) -> Result<Report> {
    if !is_repository_slug(request.repository) {
        bail!("Repository slug '{}' is invalid.", request.repository);
    }
    if !is_commit_sha(request.sha) {
        bail!(
            "Release SHA '{}' must be a lowercase 40-character commit SHA.",
            request.sha
        );
    }
    if !is_variable_name(request.variable_name) {
        bail!(
            "Repository variable '{}' is invalid.",
            request.variable_name
        );
    }
    let state = immutable_state(request)?;
    if state.get("enabled").and_then(Value::as_bool) != Some(true) {
        bail!(
            "GitHub immutable releases are not enabled for '{}'.",
            request.repository
        );
    }
    if request.write_variable {
        write_variable(request)?;
    }
    Ok(Report {
        repository: request.repository.to_string(),
        approved_sha: request.sha.to_string(),
        immutable_releases: true,
        variable: request.variable_name.to_string(),
        variable_written: request.write_variable,
    })
}
