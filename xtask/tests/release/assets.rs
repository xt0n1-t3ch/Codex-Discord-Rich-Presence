//! `release-assets` behavior.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::common::{Run, TempDir, Xtask, pe_image, read, write};

const EXPECTED_NAMES: [&str; 10] = [
    "codex-discord-rich-presence-windows-x64.exe",
    "codex-discord-rich-presence-windows-x64.spdx.json",
    "codex-discord-rich-presence-windows-arm64.exe",
    "codex-discord-rich-presence-windows-arm64.spdx.json",
    "codex-discord-rich-presence-linux-x64",
    "codex-discord-rich-presence-macos-x64",
    "codex-discord-rich-presence-macos-arm64",
    "codex-app-logo.png",
    "chatgpt-app-logo.jpg",
    "SHA256SUMS.txt",
];

const SHA256_OF_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const SHA256_OF_NOTHING: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn assemble(artifacts: &Path, output: &Path) -> Run {
    Xtask::new("release-assets")
        .flag("--artifact-root", artifacts)
        .flag("--output-directory", output)
        .run()
}

fn complete_artifacts(temp: &TempDir) -> PathBuf {
    let root = temp.join("downloaded");
    let windows = root.join("release-x86_64-pc-windows-msvc");
    write(
        &windows.join("codex-discord-rich-presence-windows-x64.exe"),
        pe_image(0x8664),
    );
    write(
        &windows.join("codex-discord-rich-presence-windows-x64.spdx.json"),
        r#"{"spdxVersion":"SPDX-2.3"}"#,
    );
    write(&windows.join("codex-app-logo.png"), "abc");
    write(&windows.join("chatgpt-app-logo.jpg"), "");
    let arm = root.join("release-aarch64-pc-windows-msvc");
    write(
        &arm.join("codex-discord-rich-presence-windows-arm64.exe"),
        pe_image(0xAA64),
    );
    write(
        &arm.join("codex-discord-rich-presence-windows-arm64.spdx.json"),
        r#"{"spdxVersion":"SPDX-2.3"}"#,
    );
    write(
        &root.join("release-x86_64-unknown-linux-gnu/codex-discord-rich-presence-linux-x64"),
        "linux-binary",
    );
    write(
        &root.join("release-x86_64-apple-darwin/codex-discord-rich-presence-macos-x64"),
        "macos-x64-binary",
    );
    write(
        &root.join("release-aarch64-apple-darwin/codex-discord-rich-presence-macos-arm64"),
        "macos-arm64-binary",
    );
    root
}

fn sha256(path: &Path) -> String {
    format!(
        "{:x}",
        Sha256::digest(fs::read(path).expect("the file is readable"))
    )
}

fn names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .expect("the directory is listed")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn an_empty_artifact_set_fails_and_writes_no_manifest() {
    let temp = TempDir::new("assets-empty");
    let empty = temp.join("empty");
    fs::create_dir_all(&empty).expect("created");
    let output = temp.join("empty-output");
    assemble(&empty, &output).fails_with("No release artifacts");
    assert!(!output.join("SHA256SUMS.txt").exists());
}

#[test]
fn a_missing_artifact_root_fails() {
    let temp = TempDir::new("assets-absent");
    assemble(&temp.join("nowhere"), &temp.join("out")).fails_with("does not exist");
}

#[test]
fn a_complete_artifact_set_yields_ten_portable_assets_with_a_manifest() {
    let temp = TempDir::new("assets-complete");
    let artifacts = complete_artifacts(&temp);
    let output = temp.join("release-assets");
    let report = assemble(&artifacts, &output).succeeds().json();
    assert_eq!(report["payload_count"], 9);

    let mut expected: Vec<String> = EXPECTED_NAMES
        .iter()
        .map(|name| (*name).to_string())
        .collect();
    expected.sort();
    assert_eq!(
        names_in(&output),
        expected,
        "published asset names are incomplete"
    );
    for name in EXPECTED_NAMES
        .iter()
        .filter(|name| **name != "SHA256SUMS.txt")
    {
        let portable = name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'.'
        }) && !name.starts_with('-')
            && !name.contains("--");
        assert!(portable, "release asset '{name}' is not portable");
    }

    let manifest = read(&output.join("SHA256SUMS.txt"));
    assert!(manifest.ends_with('\n') && !manifest.contains('\r'));
    let lines: Vec<&str> = manifest.lines().collect();
    assert_eq!(
        lines.len(),
        9,
        "the manifest must cover each published payload"
    );
    let mut sorted_names: Vec<&str> = EXPECTED_NAMES[..9].to_vec();
    sorted_names.sort_unstable();
    for (line, name) in lines.iter().zip(sorted_names) {
        assert_eq!(*line, format!("{}  {name}", sha256(&output.join(name))));
    }
    assert!(manifest.contains(&format!("{SHA256_OF_ABC}  codex-app-logo.png\n")));
    assert!(manifest.contains(&format!("{SHA256_OF_NOTHING}  chatgpt-app-logo.jpg\n")));
}

#[test]
fn a_duplicate_or_missing_binary_fails() {
    let temp = TempDir::new("assets-duplicate");
    let artifacts = complete_artifacts(&temp);
    write(
        &artifacts.join("extra/codex-discord-rich-presence-linux-x64"),
        "second linux binary",
    );
    assemble(&artifacts, &temp.join("out")).fails_with(
        "Expected exactly one artifact named 'codex-discord-rich-presence-linux-x64', found 2",
    );

    let temp = TempDir::new("assets-missing");
    let artifacts = complete_artifacts(&temp);
    fs::remove_file(
        artifacts.join("release-aarch64-apple-darwin/codex-discord-rich-presence-macos-arm64"),
    )
    .expect("removed");
    assemble(&artifacts, &temp.join("out")).fails_with(
        "Expected exactly one artifact named 'codex-discord-rich-presence-macos-arm64', found 0",
    );
}

#[test]
fn a_missing_logo_fails() {
    let temp = TempDir::new("assets-logo");
    let artifacts = complete_artifacts(&temp);
    fs::remove_file(artifacts.join("release-x86_64-pc-windows-msvc/chatgpt-app-logo.jpg"))
        .expect("removed");
    assemble(&artifacts, &temp.join("out"))
        .fails_with("Expected at least one chatgpt-app-logo.jpg artifact");
}

#[test]
fn a_windows_executable_for_the_wrong_machine_fails() {
    let temp = TempDir::new("assets-pe");
    let artifacts = complete_artifacts(&temp);
    write(
        &artifacts
            .join("release-aarch64-pc-windows-msvc/codex-discord-rich-presence-windows-arm64.exe"),
        pe_image(0x8664),
    );
    let output = temp.join("out");
    assemble(&artifacts, &output).fails_with("expected 0xAA64");
    assert!(
        !output.exists(),
        "nothing is published before every check passes"
    );
}

#[test]
fn an_output_directory_with_files_is_refused() {
    let temp = TempDir::new("assets-output");
    let artifacts = complete_artifacts(&temp);
    let output = temp.join("out");
    write(&output.join("stale.txt"), "stale");
    assemble(&artifacts, &output).fails_with("must be empty");
    assert_eq!(names_in(&output), vec!["stale.txt".to_string()]);
}
