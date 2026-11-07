//! `release-contract` behavior.

use std::path::Path;

use serde_json::{Value, json};

use crate::common::{Run, TempDir, Xtask, generate_lockfile, read, release_fixture, write};

const STABLE_CHANGELOG: &str = "# Changelog\n\n## [1.7.2] - 2026-07-09\n\n### Fixed\n\n- Gate release publication behind complete validation.\n";

fn contract(root: &Path, tag: &str) -> Run {
    Xtask::new("release-contract")
        .root(root)
        .flag("--tag", tag)
        .flag("--ref-type", "tag")
        .run()
}

#[test]
fn a_matching_stable_release_passes_and_writes_github_outputs() {
    let temp = TempDir::new("contract-stable");
    let root = release_fixture(&temp, "stable", "1.7.2", STABLE_CHANGELOG);
    let output_path = root.join("github-output.txt");
    let run = Xtask::new("release-contract")
        .root(&root)
        .flag("--tag", "v1.7.2")
        .flag("--ref-type", "tag")
        .flag("--github-output", &output_path)
        .run()
        .succeeds();
    let metadata = run.json();
    assert_eq!(metadata["version"], "1.7.2");
    assert_eq!(metadata["tag_name"], "v1.7.2");
    assert_eq!(metadata["release_name"], "CDRP 1.7.2");
    assert_eq!(metadata["is_prerelease"], false);
    assert_eq!(metadata["make_latest"], true);
    let output = read(&output_path);
    for expected in [
        "tag_name=v1.7.2",
        "version=1.7.2",
        "release_name=CDRP 1.7.2",
        "is_prerelease=false",
        "make_latest=true",
    ] {
        assert!(
            output.lines().any(|line| line == expected),
            "GitHub output is missing '{expected}': {output}"
        );
    }
}

#[test]
fn a_malformed_tag_fails() {
    let temp = TempDir::new("contract-malformed");
    let root = release_fixture(&temp, "stable", "1.7.2", STABLE_CHANGELOG);
    for tag in ["release-1.7.2", "1.7.2", "v1.7", "v01.7.2", "v1.7.2-"] {
        contract(&root, tag).fails_with("valid SemVer release tag");
    }
}

#[test]
fn a_tag_that_differs_from_cargo_fails() {
    let temp = TempDir::new("contract-mismatch");
    let root = release_fixture(&temp, "stable", "1.7.2", STABLE_CHANGELOG);
    contract(&root, "v1.7.3").fails_with("does not match Cargo package version");
}

#[test]
fn a_ref_that_is_not_a_tag_fails() {
    let temp = TempDir::new("contract-branch");
    let root = release_fixture(&temp, "stable", "1.7.2", STABLE_CHANGELOG);
    Xtask::new("release-contract")
        .root(&root)
        .flag("--tag", "v1.7.2")
        .flag("--ref-type", "branch")
        .run()
        .fails_with("requires a tag ref, received 'branch'");
}

#[test]
fn a_stale_readme_badge_fails() {
    let temp = TempDir::new("contract-readme");
    let root = release_fixture(
        &temp,
        "readme",
        "1.7.2",
        "# Changelog\n\n## [1.7.2] - 2026-07-09\n\n- Release notes.\n",
    );
    write(
        &root.join("README.md"),
        "# Fixture\n\nRelease v1.7.1\n\n## What's New in v1.7.1\n",
    );
    contract(&root, "v1.7.2").fails_with("README.md release badge");
}

#[test]
fn a_missing_changelog_section_fails() {
    let temp = TempDir::new("contract-changelog");
    let root = release_fixture(
        &temp,
        "changelog",
        "1.7.2",
        "# Changelog\n\n## [1.7.1] - 2026-07-06\n\n- Previous release.\n",
    );
    contract(&root, "v1.7.2").fails_with("non-empty CHANGELOG.md section");
}

#[test]
fn a_matching_prerelease_is_not_latest() {
    let temp = TempDir::new("contract-prerelease");
    let root = release_fixture(
        &temp,
        "prerelease",
        "1.8.0-rc.1",
        "# Changelog\n\n## [1.8.0-rc.1] - 2026-07-09\n\n- Release candidate.\n",
    );
    let metadata = contract(&root, "v1.8.0-rc.1").succeeds().json();
    assert_eq!(metadata["is_prerelease"], true);
    assert_eq!(metadata["make_latest"], false);
}

#[test]
fn build_metadata_is_part_of_the_version() {
    let temp = TempDir::new("contract-build");
    let root = release_fixture(
        &temp,
        "build",
        "1.8.0+build.7",
        "# Changelog\n\n## [1.8.0+build.7] - 2026-07-09\n\n- Literal build metadata.\n",
    );
    let metadata = contract(&root, "v1.8.0+build.7").succeeds().json();
    assert_eq!(metadata["version"], "1.8.0+build.7");
    assert_eq!(metadata["release_name"], "CDRP 1.8.0+build.7");
}

fn valid_contract() -> Value {
    json!({
        "schema_version": 1,
        "product": { "name": "release-fixture", "version": "1.7.2" },
        "core": { "package": "release-core", "version": "2.0.1", "manifest": "core/Cargo.toml" },
        "configuration": { "schema_version": 13, "source": "src/config.rs" },
        "release": {
            "windows_sbom": "codex-discord-rich-presence-windows-x64.spdx.json",
            "checksum_manifest": "SHA256SUMS.txt",
            "windows_targets": [
                {
                    "target": "x86_64-pc-windows-msvc",
                    "architecture": "x64",
                    "artifact": "codex-discord-rich-presence-windows-x64.exe",
                    "sbom": "codex-discord-rich-presence-windows-x64.spdx.json",
                    "pe_machine": "0x8664"
                },
                {
                    "target": "aarch64-pc-windows-msvc",
                    "architecture": "arm64",
                    "artifact": "codex-discord-rich-presence-windows-arm64.exe",
                    "sbom": "codex-discord-rich-presence-windows-arm64.spdx.json",
                    "pe_machine": "0xAA64"
                }
            ]
        }
    })
}

fn contract_fixture(temp: &TempDir, name: &str, document: &str) -> std::path::PathBuf {
    let root = release_fixture(temp, name, "1.7.2", STABLE_CHANGELOG);
    write(
        &root.join("Cargo.toml"),
        "[package]\nname = \"release-fixture\"\nversion = \"1.7.2\"\nedition = \"2024\"\n\n[workspace]\nmembers = [\"core\"]\n",
    );
    write(
        &root.join("core/Cargo.toml"),
        "[package]\nname = \"release-core\"\nversion = \"2.0.1\"\nedition = \"2024\"\n",
    );
    write(&root.join("core/src/lib.rs"), "");
    write(
        &root.join("src/config.rs"),
        "const CONFIG_SCHEMA_VERSION: u32 = 13;\n",
    );
    write(&root.join("scripts/release-contract.json"), document);
    generate_lockfile(&root);
    root
}

#[test]
fn a_consistent_repository_contract_passes() {
    let temp = TempDir::new("contract-valid");
    let root = contract_fixture(&temp, "valid", &valid_contract().to_string());
    contract(&root, "v1.7.2").succeeds();
}

#[test]
fn a_broken_repository_contract_fails_closed() {
    let cases: [(&str, Value, &str); 7] = [
        ("/schema_version", json!(2), "schema_version must be 1"),
        (
            "/product/version",
            json!("9.9.9"),
            "product version '9.9.9'",
        ),
        (
            "/core/version",
            json!("9.9.9"),
            "Canonical core must resolve exactly once as release-core@9.9.9",
        ),
        (
            "/configuration/schema_version",
            json!(12),
            "does not declare schema 12",
        ),
        (
            "/release/checksum_manifest",
            json!("SUMS.txt"),
            "SPDX JSON SBOM and SHA256SUMS.txt",
        ),
        (
            "/release/windows_sbom",
            json!("sbom.json"),
            "SPDX JSON SBOM and SHA256SUMS.txt",
        ),
        (
            "/release/windows_targets/1/pe_machine",
            json!("0x8664"),
            "exact x64 and ARM64 Windows artifacts",
        ),
    ];
    let temp = TempDir::new("contract-broken");
    for (index, (pointer, replacement, message)) in cases.into_iter().enumerate() {
        let mut document = valid_contract();
        *document.pointer_mut(pointer).expect("the pointer exists") = replacement;
        let root = contract_fixture(&temp, &format!("case-{index}"), &document.to_string());
        contract(&root, "v1.7.2").fails_with(message);
    }
}

#[test]
fn a_contract_that_is_not_json_fails() {
    let temp = TempDir::new("contract-garbage");
    let root = contract_fixture(&temp, "garbage", "{ not json");
    contract(&root, "v1.7.2").fails_with("release-contract.json is not valid JSON");
}
