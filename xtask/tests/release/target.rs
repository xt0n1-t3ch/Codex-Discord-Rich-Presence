//! `release-target` behavior against API fixtures.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::common::{Run, TempDir, Xtask, git, write};

const REPOSITORY: &str = "xt0n1-t3ch/Codex-Discord-Rich-Presence";
const REQUIRED_CHECKS: [&str; 6] = [
    "Release preflight",
    "Build x86_64-unknown-linux-gnu",
    "Build x86_64-apple-darwin",
    "Build aarch64-apple-darwin",
    "Build x86_64-pc-windows-msvc",
    "Build aarch64-pc-windows-msvc",
];
const FAILED_CHECK: &str = "Build aarch64-pc-windows-msvc";

struct Repo {
    path: PathBuf,
    sha: String,
}

fn repository(temp: &TempDir) -> Repo {
    let path = temp.join("repository");
    std::fs::create_dir_all(&path).expect("the repository folder is created");
    git(&path, &["init", "--initial-branch=main"]);
    write(&path.join("state.txt"), "main\n");
    git(&path, &["add", "state.txt"]);
    git(&path, &["commit", "-m", "test: seed release target"]);
    let sha = git(&path, &["rev-parse", "HEAD"]);
    git(&path, &["tag", "-a", "v1.7.2", "-m", "v1.7.2"]);
    Repo { path, sha }
}

struct ApiOptions<'a> {
    failed_check: Option<&'a str>,
    stale_success: bool,
    latest_tag: Option<&'a str>,
}

fn api_fixture(temp: &TempDir, name: &str, sha: &str, options: &ApiOptions) -> PathBuf {
    let directory = temp.join(name);
    let run = |check: &str, id: u64, success: bool| -> Value {
        json!({
            "id": id,
            "name": check,
            "status": "completed",
            "conclusion": if success { "success" } else { "failure" },
            "head_sha": sha,
            "app": { "slug": "github-actions" },
        })
    };
    let mut runs: Vec<Value> = REQUIRED_CHECKS
        .iter()
        .map(|check| run(check, 200, Some(*check) != options.failed_check))
        .collect();
    if let (true, Some(check)) = (options.stale_success, options.failed_check) {
        runs.push(run(check, 100, true));
    }
    write(
        &directory.join("check-runs.json"),
        json!({ "total_count": runs.len(), "check_runs": runs }).to_string(),
    );
    match options.latest_tag {
        Some(tag) => write(
            &directory.join("latest-release.json"),
            json!({ "tag_name": tag, "draft": false, "prerelease": false }).to_string(),
        ),
        None => write(&directory.join("latest-release.not-found"), ""),
    }
    directory
}

fn plain_api(temp: &TempDir, name: &str, sha: &str) -> PathBuf {
    api_fixture(
        temp,
        name,
        sha,
        &ApiOptions {
            failed_check: None,
            stale_success: false,
            latest_tag: None,
        },
    )
}

struct Target<'a> {
    repo: &'a Repo,
    api: &'a Path,
    tag: &'a str,
    version: &'a str,
    sha: &'a str,
    approved_sha: &'a str,
    prerelease: &'a str,
}

fn check(target: &Target) -> Run {
    Xtask::new("release-target")
        .root(&target.repo.path)
        .flag("--repository", REPOSITORY)
        .flag("--tag", target.tag)
        .flag("--version", target.version)
        .flag("--sha", target.sha)
        .flag("--approved-sha", target.approved_sha)
        .flag("--is-prerelease", target.prerelease)
        .flag("--main-ref", "main")
        .flag("--api-fixture-dir", target.api)
        .run()
}

fn standard<'a>(repo: &'a Repo, api: &'a Path) -> Target<'a> {
    Target {
        repo,
        api,
        tag: "v1.7.2",
        version: "1.7.2",
        sha: &repo.sha,
        approved_sha: &repo.sha,
        prerelease: "false",
    }
}

#[test]
fn a_protected_main_commit_with_every_check_green_passes() {
    let temp = TempDir::new("target-valid");
    let repo = repository(&temp);
    let api = plain_api(&temp, "api-valid", &repo.sha);
    let report = check(&standard(&repo, &api)).succeeds().json();
    assert_eq!(report["required_checks"], 6);
    assert_eq!(report["release_approval"], true);
    assert_eq!(report["latest_guard"], "first-stable-release");
    assert_eq!(report["sha"], repo.sha.as_str());
    assert_eq!(report["main_ref"], "main");
}

#[test]
fn a_sha_without_the_exact_operator_approval_fails() {
    let temp = TempDir::new("target-unapproved");
    let repo = repository(&temp);
    let api = plain_api(&temp, "api-valid", &repo.sha);
    let zeros = "0".repeat(40);
    check(&Target {
        approved_sha: &zeros,
        ..standard(&repo, &api)
    })
    .fails_with("approved release SHA");
}

#[test]
fn a_failed_protected_context_blocks_the_release() {
    let temp = TempDir::new("target-failed");
    let repo = repository(&temp);
    let api = api_fixture(
        &temp,
        "api-failed",
        &repo.sha,
        &ApiOptions {
            failed_check: Some(FAILED_CHECK),
            stale_success: false,
            latest_tag: None,
        },
    );
    check(&standard(&repo, &api)).fails_with(FAILED_CHECK);
}

#[test]
fn a_stale_success_does_not_hide_a_newer_failure() {
    let temp = TempDir::new("target-stale");
    let repo = repository(&temp);
    let api = api_fixture(
        &temp,
        "api-stale",
        &repo.sha,
        &ApiOptions {
            failed_check: Some(FAILED_CHECK),
            stale_success: true,
            latest_tag: None,
        },
    );
    check(&standard(&repo, &api)).fails_with(FAILED_CHECK);
}

#[test]
fn a_missing_required_check_blocks_the_release() {
    let temp = TempDir::new("target-missing");
    let repo = repository(&temp);
    let api = plain_api(&temp, "api-missing", &repo.sha);
    let mut response: Value =
        serde_json::from_str(&crate::common::read(&api.join("check-runs.json")))
            .expect("the fixture is JSON");
    response["check_runs"]
        .as_array_mut()
        .expect("runs")
        .retain(|run| run["name"] != "Release preflight");
    write(&api.join("check-runs.json"), response.to_string());
    check(&standard(&repo, &api)).fails_with("Release prerequisite 'Release preflight'");
}

#[test]
fn checks_from_another_app_or_commit_do_not_count() {
    let temp = TempDir::new("target-foreign");
    let repo = repository(&temp);
    let api = plain_api(&temp, "api-foreign", &repo.sha);
    let mut response: Value =
        serde_json::from_str(&crate::common::read(&api.join("check-runs.json")))
            .expect("the fixture is JSON");
    response["check_runs"][0]["app"]["slug"] = json!("some-other-app");
    write(&api.join("check-runs.json"), response.to_string());
    check(&standard(&repo, &api)).fails_with("Release prerequisite 'Release preflight'");
}

#[test]
fn a_tag_that_points_elsewhere_fails() {
    let temp = TempDir::new("target-sha");
    let repo = repository(&temp);
    let zeros = "0".repeat(40);
    let api = plain_api(&temp, "api-valid", &zeros);
    check(&Target {
        sha: &zeros,
        approved_sha: &zeros,
        ..standard(&repo, &api)
    })
    .fails_with("does not match tag");
}

#[test]
fn a_tag_outside_main_fails() {
    let temp = TempDir::new("target-side");
    let repo = repository(&temp);
    git(&repo.path, &["switch", "-c", "release-side"]);
    write(&repo.path.join("state.txt"), "side\n");
    git(&repo.path, &["add", "state.txt"]);
    git(
        &repo.path,
        &["commit", "-m", "test: create non-main release commit"],
    );
    let side_sha = git(&repo.path, &["rev-parse", "HEAD"]);
    git(&repo.path, &["tag", "-a", "v1.7.3", "-m", "v1.7.3"]);
    let api = plain_api(&temp, "api-side", &side_sha);
    check(&Target {
        tag: "v1.7.3",
        version: "1.7.3",
        sha: &side_sha,
        approved_sha: &side_sha,
        ..standard(&repo, &api)
    })
    .fails_with("not an ancestor of main");
}

#[test]
fn a_lightweight_tag_fails() {
    let temp = TempDir::new("target-lightweight");
    let repo = repository(&temp);
    git(&repo.path, &["tag", "v1.7.4"]);
    let api = plain_api(&temp, "api-valid", &repo.sha);
    check(&Target {
        tag: "v1.7.4",
        version: "1.7.4",
        ..standard(&repo, &api)
    })
    .fails_with("must be an annotated tag");
}

#[test]
fn an_older_stable_release_cannot_replace_a_newer_latest() {
    let temp = TempDir::new("target-latest");
    let repo = repository(&temp);
    for latest in ["v1.8.0", "v1.7.2"] {
        let api = api_fixture(
            &temp,
            &format!("api-latest-{latest}"),
            &repo.sha,
            &ApiOptions {
                failed_check: None,
                stale_success: false,
                latest_tag: Some(latest),
            },
        );
        check(&standard(&repo, &api)).fails_with("not newer than latest stable");
    }
}

#[test]
fn a_newer_release_records_the_latest_it_replaces() {
    let temp = TempDir::new("target-newer");
    let repo = repository(&temp);
    let api = api_fixture(
        &temp,
        "api-newer",
        &repo.sha,
        &ApiOptions {
            failed_check: None,
            stale_success: false,
            latest_tag: Some("v1.7.1"),
        },
    );
    let report = check(&standard(&repo, &api)).succeeds().json();
    assert_eq!(report["latest_guard"], "newer-than-v1.7.1");
}

#[test]
fn a_prerelease_never_becomes_latest() {
    let temp = TempDir::new("target-prerelease");
    let repo = repository(&temp);
    git(
        &repo.path,
        &["tag", "-a", "v1.8.0-rc.1", "-m", "v1.8.0-rc.1"],
    );
    let api = api_fixture(
        &temp,
        "api-prerelease",
        &repo.sha,
        &ApiOptions {
            failed_check: None,
            stale_success: false,
            latest_tag: Some("v9.9.9"),
        },
    );
    let report = check(&Target {
        tag: "v1.8.0-rc.1",
        version: "1.8.0-rc.1",
        prerelease: "true",
        ..standard(&repo, &api)
    })
    .succeeds()
    .json();
    assert_eq!(report["latest_guard"], "prerelease-not-latest");
}

#[test]
fn the_prerelease_flag_must_match_the_version() {
    let temp = TempDir::new("target-flag");
    let repo = repository(&temp);
    let api = plain_api(&temp, "api-valid", &repo.sha);
    check(&Target {
        prerelease: "true",
        ..standard(&repo, &api)
    })
    .fails_with("Prerelease flag 'true' does not match version '1.7.2'");
}

#[test]
fn the_tag_must_be_the_validated_version() {
    let temp = TempDir::new("target-version");
    let repo = repository(&temp);
    let api = plain_api(&temp, "api-valid", &repo.sha);
    check(&Target {
        version: "1.7.3",
        ..standard(&repo, &api)
    })
    .fails_with("Tag 'v1.7.2' does not match validated version '1.7.3'");
}
