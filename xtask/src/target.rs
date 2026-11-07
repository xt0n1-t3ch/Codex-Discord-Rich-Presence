//! Release target checks: annotated tag, approved commit, main-branch ancestry, passing checks and the latest-release guard.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::exec;
use crate::github;
use crate::semver::Version;
use crate::text::{is_commit_sha, is_repository_slug};

pub const REQUIRED_CHECKS: [&str; 6] = [
    "Release preflight",
    "Build x86_64-unknown-linux-gnu",
    "Build x86_64-apple-darwin",
    "Build aarch64-apple-darwin",
    "Build x86_64-pc-windows-msvc",
    "Build aarch64-pc-windows-msvc",
];

#[derive(Debug, Clone)]
pub enum Api {
    Live,
    Fixtures(PathBuf),
}

fn read_fixture(path: &Path) -> Result<Value> {
    if !path.is_file() {
        bail!("GitHub API fixture '{}' is missing.", path.display());
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading '{}'", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing '{}'", path.display()))
}

impl Api {
    fn check_runs(&self, repository: &str, sha: &str) -> Result<Value> {
        match self {
            Self::Fixtures(directory) => read_fixture(&directory.join("check-runs.json")),
            Self::Live => github::get(
                &format!("repos/{repository}/commits/{sha}/check-runs?per_page=100&filter=latest"),
                false,
            )?
            .context("GitHub returned no check runs"),
        }
    }

    fn latest_release(&self, repository: &str) -> Result<Option<Value>> {
        match self {
            Self::Fixtures(directory) => {
                let path = directory.join("latest-release.json");
                if !path.is_file() && directory.join("latest-release.not-found").is_file() {
                    return Ok(None);
                }
                read_fixture(&path).map(Some)
            }
            Self::Live => github::get(&format!("repos/{repository}/releases/latest"), true),
        }
    }
}

pub struct Request<'a> {
    pub repository: &'a str,
    pub tag: &'a str,
    pub version: &'a str,
    pub sha: &'a str,
    pub approved_sha: &'a str,
    pub is_prerelease: bool,
    pub root: &'a Path,
    pub main_ref: &'a str,
    pub api: &'a Api,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub repository: String,
    pub tag: String,
    pub sha: String,
    pub main_ref: String,
    pub release_approval: bool,
    pub required_checks: usize,
    pub latest_guard: String,
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let output = exec::captured(Command::new("git").arg("-C").arg(root).args(args))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            exec::failure_text(&output)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn split_main_ref(main_ref: &str) -> Option<(&str, &str)> {
    let (remote, branch) = main_ref.split_once('/')?;
    let remote_valid = !remote.is_empty()
        && remote
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
    let branch_valid = !branch.is_empty()
        && branch
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'/' | b'-'));
    (remote_valid && branch_valid).then_some((remote, branch))
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str()
}

fn check_git(request: &Request) -> Result<()> {
    let (root, tag, sha, main_ref) = (request.root, request.tag, request.sha, request.main_ref);
    if let Some((remote, branch)) = split_main_ref(main_ref) {
        let refspec = format!("+refs/heads/{branch}:refs/remotes/{remote}/{branch}");
        git(root, &["fetch", "--no-tags", remote, &refspec])?;
    }
    if git(root, &["cat-file", "-t", tag])? != "tag" {
        bail!("Release tag '{tag}' must be an annotated tag.");
    }
    let tag_commit = git(root, &["rev-parse", &format!("{tag}^{{commit}}")])?;
    if tag_commit != *sha {
        bail!("Release SHA '{sha}' does not match tag '{tag}' commit '{tag_commit}'.");
    }
    git(root, &["rev-parse", &format!("{main_ref}^{{commit}}")])?;
    let ancestor = exec::captured(Command::new("git").arg("-C").arg(root).args([
        "merge-base",
        "--is-ancestor",
        sha,
        main_ref,
    ]))?;
    match ancestor.status.code() {
        Some(0) => Ok(()),
        Some(1) => bail!("Tag '{tag}' commit '{sha}' is not an ancestor of {main_ref}."),
        other => bail!(
            "Unable to verify '{sha}' against {main_ref} (git exit {}).",
            other.map_or_else(|| "unknown".to_string(), |code| code.to_string())
        ),
    }
}

fn check_required_runs(request: &Request) -> Result<()> {
    let response = request.api.check_runs(request.repository, request.sha)?;
    let Some(runs) = response.get("check_runs").and_then(Value::as_array) else {
        bail!("GitHub check-runs response is missing check_runs.");
    };
    for required in REQUIRED_CHECKS {
        let latest = runs
            .iter()
            .filter(|run| {
                str_field(run, "name") == Some(required)
                    && str_field(run, "head_sha") == Some(request.sha)
                    && run.pointer("/app/slug").and_then(Value::as_str) == Some("github-actions")
            })
            .max_by_key(|run| run.get("id").and_then(Value::as_u64).unwrap_or(0));
        let passed = latest.is_some_and(|run| {
            str_field(run, "status") == Some("completed")
                && str_field(run, "conclusion") == Some("success")
        });
        if !passed {
            bail!(
                "Release prerequisite '{required}' is missing or unsuccessful for '{}'.",
                request.sha
            );
        }
    }
    Ok(())
}

fn latest_guard(request: &Request, current: &Version) -> Result<String> {
    if request.is_prerelease {
        return Ok("prerelease-not-latest".to_string());
    }
    let Some(latest) = request.api.latest_release(request.repository)? else {
        return Ok("first-stable-release".to_string());
    };
    let Some(latest_tag) = str_field(&latest, "tag_name") else {
        bail!("Latest release response has an invalid tag_name.");
    };
    let Some(latest_version) = latest_tag.strip_prefix('v').filter(|rest| !rest.is_empty()) else {
        bail!("Latest release response has an invalid tag_name.");
    };
    let Ok(latest_version) = latest_version.parse::<Version>() else {
        bail!("Latest stable release tag '{latest_tag}' is not valid SemVer.");
    };
    if current.precedence(&latest_version) != std::cmp::Ordering::Greater {
        bail!(
            "Release '{}' is not newer than latest stable '{latest_tag}'.",
            request.tag
        );
    }
    Ok(format!("newer-than-{latest_tag}"))
}

pub fn check(request: &Request) -> Result<Report> {
    if !is_repository_slug(request.repository) {
        bail!("Repository slug '{}' is invalid.", request.repository);
    }
    if !is_commit_sha(request.sha) {
        bail!(
            "Release SHA '{}' must be a lowercase 40-character commit SHA.",
            request.sha
        );
    }
    if request.approved_sha != request.sha {
        bail!(
            "Release SHA '{}' does not match the approved release SHA '{}'.",
            request.sha,
            request.approved_sha
        );
    }
    if request.tag != format!("v{}", request.version) {
        bail!(
            "Tag '{}' does not match validated version '{}'.",
            request.tag,
            request.version
        );
    }
    let Ok(current) = request.version.parse::<Version>() else {
        bail!(
            "Validated version '{}' is not valid SemVer.",
            request.version
        );
    };
    if request.is_prerelease != current.is_prerelease() {
        bail!(
            "Prerelease flag '{}' does not match version '{}'.",
            request.is_prerelease,
            request.version
        );
    }
    check_git(request)?;
    check_required_runs(request)?;
    let guard = latest_guard(request, &current)?;
    Ok(Report {
        repository: request.repository.to_string(),
        tag: request.tag.to_string(),
        sha: request.sha.to_string(),
        main_ref: request.main_ref.to_string(),
        release_approval: true,
        required_checks: REQUIRED_CHECKS.len(),
        latest_guard: guard,
    })
}
