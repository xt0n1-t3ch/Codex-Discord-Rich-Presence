//! `approve-release` behavior.

use std::path::Path;

use crate::common::{Run, TempDir, Xtask, write};

const REPOSITORY: &str = "xt0n1-t3ch/Codex-Discord-Rich-Presence";

fn approve(fixture: &Path, sha: &str) -> Run {
    Xtask::new("approve-release")
        .flag("--repository", REPOSITORY)
        .flag("--sha", sha)
        .flag("--api-fixture", fixture)
        .arg("--skip-variable-write")
        .run()
}

#[test]
fn enabled_immutable_releases_approve_the_exact_sha() {
    let temp = TempDir::new("approval-enabled");
    let enabled = temp.join("enabled.json");
    write(&enabled, r#"{"enabled":true}"#);
    let sha = "a".repeat(40);
    let report = approve(&enabled, &sha).succeeds().json();
    assert_eq!(report["approved_sha"], sha.as_str());
    assert_eq!(report["immutable_releases"], true);
    assert_eq!(report["repository"], REPOSITORY);
    assert_eq!(report["variable"], "RELEASE_APPROVED_SHA");
    assert_eq!(report["variable_written"], false);
}

#[test]
fn disabled_immutable_releases_fail_closed() {
    let temp = TempDir::new("approval-disabled");
    let sha = "a".repeat(40);
    for (name, body) in [
        ("disabled.json", r#"{"enabled":false}"#),
        ("missing.json", r#"{"enforced_by_owner":true}"#),
        ("text.json", r#"{"enabled":"true"}"#),
    ] {
        let fixture = temp.join(name);
        write(&fixture, body);
        approve(&fixture, &sha).fails_with("not enabled");
    }
}

#[test]
fn a_missing_fixture_fails() {
    let temp = TempDir::new("approval-missing");
    approve(&temp.join("absent.json"), &"a".repeat(40)).fails_with("Immutable release fixture");
}

#[test]
fn the_sha_the_repository_and_the_variable_are_validated() {
    let temp = TempDir::new("approval-validation");
    let enabled = temp.join("enabled.json");
    write(&enabled, r#"{"enabled":true}"#);
    approve(&enabled, &"A".repeat(40)).fails_with("lowercase 40-character commit SHA");
    approve(&enabled, "abc123").fails_with("lowercase 40-character commit SHA");
    Xtask::new("approve-release")
        .flag("--repository", "not a slug")
        .flag("--sha", "a".repeat(40))
        .flag("--api-fixture", &enabled)
        .arg("--skip-variable-write")
        .run()
        .fails_with("Repository slug 'not a slug' is invalid");
    Xtask::new("approve-release")
        .flag("--repository", REPOSITORY)
        .flag("--sha", "a".repeat(40))
        .flag("--variable-name", "lower")
        .flag("--api-fixture", &enabled)
        .arg("--skip-variable-write")
        .run()
        .fails_with("Repository variable 'lower' is invalid");
}
