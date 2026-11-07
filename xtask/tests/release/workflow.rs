//! Guards on the release workflow file: pinned actions, least privilege and locked cargo commands.

use std::collections::HashSet;

use crate::common::{read, repository_root};

fn workflow() -> String {
    read(&repository_root().join(".github/workflows/release.yml"))
}

fn job(workflow: &str, name: &str) -> String {
    let header = format!("\n  {name}:\n");
    let start = workflow
        .find(&header)
        .unwrap_or_else(|| panic!("workflow job '{name}' is missing"))
        + header.len();
    let rest = &workflow[start..];
    let end = rest
        .match_indices("\n  ")
        .find(|(index, _)| {
            let line = rest[index + 3..].lines().next().unwrap_or("");
            !line.starts_with(' ') && line.trim_end().ends_with(':')
        })
        .map_or(rest.len(), |(index, _)| index);
    rest[..end].to_string()
}

fn assert_has(text: &str, needle: &str, message: &str) {
    assert!(text.contains(needle), "{message}: missing {needle:?}");
}

fn assert_lacks(text: &str, needle: &str, message: &str) {
    assert!(!text.contains(needle), "{message}: found {needle:?}");
}

#[test]
fn the_toolchain_and_cargo_wiring_match_the_tooling_decision() {
    let root = repository_root();
    let toolchain = read(&root.join("rust-toolchain.toml"));
    assert_has(
        &toolchain,
        "channel = \"1.99.1\"",
        "Rust must be pinned to 1.99.1",
    );
    assert_has(
        &toolchain,
        "components = [\"clippy\", \"rustfmt\"]",
        "clippy and rustfmt are installed",
    );
    assert_has(
        &toolchain,
        "profile = \"minimal\"",
        "the minimal profile is used",
    );
    let config = read(&root.join(".cargo/config.toml"));
    assert_has(&config, "xtask = \"run -q -p xtask --\"", "the xtask alias");
    let manifest: toml::Table = read(&root.join("Cargo.toml"))
        .parse()
        .expect("Cargo.toml is TOML");
    let members: Vec<&str> = manifest["workspace"]["members"]
        .as_array()
        .expect("workspace members")
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(
        members.contains(&"xtask"),
        "xtask must be a workspace member: {members:?}"
    );
}

#[test]
fn every_action_is_pinned_to_a_commit() {
    let workflow = workflow();
    let uses: Vec<&str> = workflow
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("uses:"))
        .map(|rest| rest.split_whitespace().next().expect("an action reference"))
        .collect();
    assert!(!uses.is_empty(), "the workflow has no actions to validate");
    for reference in uses {
        let (name, revision) = reference
            .split_once('@')
            .unwrap_or_else(|| panic!("'{reference}' has no revision"));
        assert!(name.contains('/'), "'{reference}' is not owner/name");
        assert!(
            revision.len() == 40
                && revision
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
            "'{reference}' uses a mutable Action reference"
        );
    }
}

#[test]
fn the_workflow_stays_manual_least_privilege_and_serialized() {
    let workflow = workflow();
    assert_has(
        &workflow,
        "\non:\n  workflow_dispatch:\n",
        "the release workflow must remain manual-only",
    );
    for trigger in [
        "push:",
        "pull_request:",
        "schedule:",
        "workflow_run:",
        "release:",
    ] {
        assert_lacks(
            &workflow,
            &format!("\n  {trigger}"),
            "no automatic trigger is allowed",
        );
    }
    assert_has(
        &workflow,
        "\npermissions:\n  contents: read\n",
        "the default permission is contents: read",
    );
    assert_has(
        &workflow,
        "group: release-${{ github.repository }}",
        "one release at a time per repository",
    );
    assert_has(
        &workflow,
        "cancel-in-progress: false",
        "an active publication is never cancelled",
    );
    assert_has(
        &workflow,
        "persist-credentials: false",
        "checkout credentials do not persist",
    );
    assert_has(
        &workflow,
        "toolchain: 1.99.1",
        "the workflow installs Rust 1.99.1",
    );
    assert_has(
        &workflow,
        "RUSTFLAGS: \"\"",
        "mold is not installed on the runners",
    );
}

#[test]
fn no_step_depends_on_powershell() {
    let workflow = workflow();
    for needle in ["pwsh", "powershell", ".ps1", "Taskfile"] {
        assert_lacks(
            &workflow.to_lowercase(),
            &needle.to_lowercase(),
            "the workflow must not use PowerShell",
        );
    }
    assert_has(
        &workflow,
        "defaults:\n  run:\n    shell: bash\n",
        "every step runs in bash",
    );
}

#[test]
fn every_cargo_command_is_locked_except_xtask_and_the_pinned_audit() {
    let workflow = workflow();
    let commands: Vec<&str> = workflow
        .lines()
        .map(|line| {
            let line = line.trim();
            line.strip_prefix("run: ").unwrap_or(line)
        })
        .filter(|line| line.starts_with("cargo "))
        .collect();
    assert!(!commands.is_empty(), "no Cargo command was discovered");
    for command in commands {
        let allowed = command.starts_with("cargo xtask ")
            || command == "cargo install cargo-audit --version 0.22.2 --locked"
            || command == "cargo audit --deny warnings"
            || command.starts_with("cargo --locked ");
        assert!(allowed, "Cargo command is not lockfile-enforced: {command}");
    }
}

#[test]
fn preflight_validates_the_contract_the_notes_and_the_quality_gates() {
    let workflow = workflow();
    let preflight = job(&workflow, "preflight");
    for command in [
        "cargo install cargo-audit --version 0.22.2 --locked",
        "cargo audit --deny warnings",
        "cargo xtask release-contract --tag \"$RELEASE_TAG\" --ref-type \"$RELEASE_REF_TYPE\"",
        "cargo xtask release-notes \"$VERSION\" --output RELEASE_NOTES.md",
        "cargo --locked fmt --all -- --check",
        "cargo --locked clippy --workspace --all-targets --all-features -- -D warnings",
        "cargo --locked test --workspace --all-features --verbose",
        "cargo --locked build --workspace --release --all-features",
    ] {
        assert_has(&preflight, command, "preflight command");
    }
    assert_has(
        &preflight,
        "name: Release preflight",
        "the required check name",
    );
    assert_has(
        &preflight,
        "Upload validated release metadata",
        "the notes cross the workflow as an artifact",
    );
    assert_has(
        &preflight,
        "path: RELEASE_NOTES.md",
        "the uploaded file is the release notes",
    );
    assert_lacks(
        &preflight,
        "--repository",
        "the release notes do not need the repository slug",
    );
}

#[test]
fn builds_run_from_xtask_on_every_native_runner() {
    let workflow = workflow();
    let build = job(&workflow, "build");
    assert_has(
        &build,
        "needs: preflight\n",
        "matrix builds depend on preflight",
    );
    assert_has(
        &build,
        "windows-11-arm",
        "the native public Windows ARM64 runner",
    );
    assert_has(
        &build,
        "aarch64-pc-windows-msvc",
        "the Windows ARM64 MSVC target",
    );
    assert_has(
        &build,
        "cargo xtask build-release --target ${{ matrix.target }}",
        "build and package from xtask",
    );
    assert_has(
        &build,
        "cargo xtask smoke --artifact releases/windows/codex-discord-rich-presence-windows-${{ matrix.arch }}.exe",
        "each Windows architecture runs a native smoke test",
    );
    assert_has(
        &build,
        "path: releases/${{ matrix.os_name }}/",
        "the packaged folder is uploaded",
    );
    for target in xtask::assets::TARGETS {
        assert_has(
            &build,
            &format!("target: {}", target.triple),
            "the build matrix covers every release target",
        );
    }
}

#[test]
fn publish_checks_the_target_before_it_creates_the_draft() {
    let workflow = workflow();
    let publish = job(&workflow, "publish");
    assert_has(
        &publish,
        "needs: [preflight, build]\n",
        "publish depends on every build",
    );
    assert_has(
        &publish,
        "    permissions:\n      checks: read\n      contents: write\n",
        "only publish gets checks: read and contents: write",
    );
    assert_has(
        &publish,
        "RELEASE_APPROVED_SHA: ${{ vars.RELEASE_APPROVED_SHA }}",
        "the operator-approved exact SHA",
    );
    assert_has(
        &publish,
        "cargo xtask release-assets --artifact-root downloaded-artifacts --output-directory release-assets",
        "the checked artifact assembler",
    );
    for argument in [
        "--repository \"$GITHUB_REPOSITORY\"",
        "--tag \"$TAG_NAME\"",
        "--version \"$VERSION\"",
        "--sha \"$RELEASE_SHA\"",
        "--approved-sha \"$RELEASE_APPROVED_SHA\"",
        "--is-prerelease \"$IS_PRERELEASE\"",
        "--main-ref origin/main",
    ] {
        assert_has(&publish, argument, "release target wiring");
    }
    let gate = publish
        .find("cargo xtask release-target")
        .expect("the target gate exists");
    let draft = publish
        .find("Create draft and upload release assets")
        .expect("the draft step exists");
    assert!(
        gate < draft,
        "repository target validation must run before draft creation"
    );
    assert_has(
        &publish,
        "downloaded-artifacts/release-metadata/RELEASE_NOTES.md",
        "the notes validated in preflight",
    );
    for needle in [
        "fail_on_unmatched_files: true",
        "overwrite_files: false",
        "SHA256SUMS.txt",
        "draft: true",
        "steps.draft-release.outputs.id",
        "Verify draft release assets",
        "test \"${#actual_assets[@]}\" -eq 10",
        ".digest",
        "Finalize release once",
        "{draft: false, prerelease: $prerelease, make_latest: $make_latest}",
        "Refuse release mutation",
    ] {
        assert_has(&publish, needle, "publication safeguard");
    }
    assert_has(
        &publish,
        ".immutable",
        "finalization confirms the release is immutable",
    );
    assert_lacks(
        &publish,
        "always(",
        "publish must not bypass a failed dependency",
    );
}

#[test]
fn the_required_checks_match_the_workflow_jobs() {
    let workflow = workflow();
    let names: HashSet<String> = xtask::target::REQUIRED_CHECKS
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    assert!(names.contains("Release preflight"));
    assert_has(
        &workflow,
        "name: Release preflight",
        "the preflight job name",
    );
    assert_has(
        &workflow,
        "name: Build ${{ matrix.target }}",
        "the build job name pattern",
    );
    for target in xtask::assets::TARGETS {
        assert!(
            names.contains(&format!("Build {}", target.triple)),
            "{} is not a required check",
            target.triple
        );
    }
    assert_eq!(names.len(), xtask::assets::TARGETS.len() + 1);
}
