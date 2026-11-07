//! `build-release` target resolution and build arguments.

use std::fs;

use xtask::assets::TARGETS;
use xtask::build_release::{cargo_build_args, host_triple, parse_host, resolve_targets};
use xtask::smoke;

use crate::common::{TempDir, Xtask, generate_lockfile, read, write};

fn requested(triples: &[&str]) -> Vec<String> {
    triples.iter().map(|triple| (*triple).to_string()).collect()
}

#[test]
fn the_release_build_is_locked_target_specific_and_covers_the_workspace() {
    assert_eq!(
        cargo_build_args("aarch64-pc-windows-msvc"),
        [
            "build",
            "--locked",
            "--workspace",
            "--release",
            "--all-features",
            "--target",
            "aarch64-pc-windows-msvc"
        ]
    );
}

#[test]
fn windows_arm64_has_its_own_executable_and_sbom() {
    let targets =
        resolve_targets(&requested(&["aarch64-pc-windows-msvc"]), None).expect("supported target");
    assert_eq!(targets.len(), 1);
    assert!(targets[0].is_windows());
    assert_eq!(
        targets[0].artifact,
        "codex-discord-rich-presence-windows-arm64.exe"
    );
    assert_eq!(
        targets[0].sbom,
        Some("codex-discord-rich-presence-windows-arm64.spdx.json")
    );
}

#[test]
fn only_windows_targets_carry_an_sbom() {
    for target in TARGETS {
        assert_eq!(
            target.sbom.is_some(),
            target.is_windows(),
            "{}",
            target.triple
        );
    }
}

#[test]
fn the_host_is_built_when_no_target_is_requested() {
    let targets = resolve_targets(&[], Some("x86_64-unknown-linux-gnu")).expect("supported host");
    assert_eq!(targets[0].artifact, "codex-discord-rich-presence-linux-x64");
    let error = resolve_targets(&[], None).expect_err("nothing selected");
    assert!(error.to_string().contains("No release target was selected"));
}

#[test]
fn unsupported_targets_are_rejected() {
    let error =
        resolve_targets(&requested(&["aarch64-unknown-linux-gnu"]), None).expect_err("unsupported");
    let message = error.to_string();
    assert!(
        message.contains("Unsupported release target 'aarch64-unknown-linux-gnu'"),
        "{message}"
    );
    assert!(message.contains("x86_64-pc-windows-msvc"), "{message}");
}

#[test]
fn one_run_builds_one_operating_system() {
    let error = resolve_targets(
        &requested(&["x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu"]),
        None,
    )
    .expect_err("mixed systems");
    assert!(error.to_string().contains("separate runs"));
    let both = resolve_targets(
        &requested(&[
            "x86_64-pc-windows-msvc",
            "aarch64-pc-windows-msvc",
            "x86_64-pc-windows-msvc",
        ]),
        None,
    )
    .expect("both Windows targets");
    assert_eq!(both.len(), 2);
}

#[test]
fn the_host_triple_comes_from_rustc_verbose_output() {
    let sample = "rustc 1.99.1 (0123456789 2026-10-01)\nbinary: rustc\ncommit-hash: 0123456789\nhost: aarch64-apple-darwin\nrelease: 1.99.1\nLLVM version: 23.0.0\n";
    assert_eq!(parse_host(sample).as_deref(), Some("aarch64-apple-darwin"));
    assert_eq!(parse_host("rustc 1.99.1\n"), None);
}

#[test]
fn the_native_smoke_runs_version_and_doctor() {
    assert_eq!(smoke::COMMANDS, ["--version", "doctor"]);
}

#[test]
fn the_host_target_is_built_packaged_and_checksummed() {
    let host = host_triple().expect("rustc reports its host");
    let Some(target) = TARGETS.iter().find(|target| target.triple == host) else {
        eprintln!("skipped: {host} is not a release target");
        return;
    };
    let temp = TempDir::new("build-release");
    let root = temp.join("fixture");
    write(
        &root.join("Cargo.toml"),
        "[package]\nname = \"codex-discord-presence\"\nversion = \"1.0.0\"\nedition = \"2024\"\n\n[dependencies]\ndep-a = { path = \"dep-a\" }\n",
    );
    write(&root.join("src/main.rs"), "fn main() {}\n");
    write(
        &root.join("dep-a/Cargo.toml"),
        "[package]\nname = \"dep-a\"\nversion = \"0.3.0\"\nedition = \"2024\"\n",
    );
    write(&root.join("dep-a/src/lib.rs"), "");
    write(&root.join("assets/branding/codex-app.png"), "png-bytes");
    write(&root.join("assets/branding/chatgpt-app.jpg"), "jpg-bytes");
    generate_lockfile(&root);

    let build = || {
        Xtask::new("build-release")
            .root(&root)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .run()
    };
    let run = build().succeeds();
    assert!(run.stdout.contains("Ready:"), "{}", run.stdout);

    let output = root.join("releases").join(target.os_name);
    let mut expected: Vec<&str> = vec![
        target.artifact,
        "codex-app-logo.png",
        "chatgpt-app-logo.jpg",
        "SHA256SUMS.txt",
    ];
    expected.extend(target.sbom);
    expected.sort_unstable();
    let mut found: Vec<String> = fs::read_dir(&output)
        .expect("the release folder exists")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    found.sort();
    assert_eq!(found, expected);

    assert_eq!(read(&output.join("codex-app-logo.png")), "png-bytes");
    assert_eq!(read(&output.join("chatgpt-app-logo.jpg")), "jpg-bytes");
    let manifest = read(&output.join("SHA256SUMS.txt"));
    assert_eq!(manifest.lines().count(), expected.len() - 1);
    assert!(manifest.contains("  codex-app-logo.png\n"));
    assert!(manifest.contains(&format!("  {}\n", target.artifact)));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(output.join(target.artifact))
            .expect("artifact")
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "the artifact must be executable");
    }

    build().fails_with("must be empty");
}
