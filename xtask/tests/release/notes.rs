//! `release-notes` title, body and long-section behavior.

use xtask::notes::{self, LONG_SECTION_LINES, LongNotes};

use crate::common::{TempDir, Xtask, read, release_fixture, write};

const SUMMARY_CHANGELOG: &str = "# Changelog\n\n## [Unreleased]\n\n## [1.7.2] - 2026-07-09\n\nThis release gates publication behind complete validation. Roll back by restoring the previous executable.\n\n### Added\n\n- Add a release gate.\n\n### Fixed\n\n- Gate release publication behind complete validation.\n\n## [1.7.1] - 2026-07-06\n\nPrevious release.\n\n### Fixed\n\n- Older fix.\n";

const EXPECTED_NOTES: &str = "This release gates publication behind complete validation. Roll back by restoring the previous executable.\n\n### Added\n\n- Add a release gate.\n\n### Fixed\n\n- Gate release publication behind complete validation.\n";

fn assert_plain_notes(body: &str) {
    assert!(
        !body.lines().any(|line| line.starts_with("# ")),
        "notes must not repeat the title as a heading: {body}"
    );
    let lowered = body.to_lowercase();
    for forbidden in [
        "## what changed",
        "## release assets",
        "## integrity",
        "## full changelog",
        "stable release for",
        "prerelease for",
        "compare/",
        "sha256sums.txt",
    ] {
        assert!(
            !lowered.contains(forbidden),
            "notes contain '{forbidden}': {body}"
        );
    }
    for asset in xtask::assets::payload_names() {
        assert!(
            !body.contains(asset),
            "notes list the asset '{asset}': {body}"
        );
    }
}

#[test]
fn release_notes_are_exactly_the_changelog_section() {
    let temp = TempDir::new("notes-section");
    let root = release_fixture(&temp, "fixture", "1.7.2", SUMMARY_CHANGELOG);
    let run = Xtask::new("release-notes")
        .root(&root)
        .arg("1.7.2")
        .run()
        .succeeds();
    assert_eq!(run.stdout, EXPECTED_NOTES);
    assert_plain_notes(&run.stdout);
}

#[test]
fn release_notes_can_be_written_to_a_file() {
    let temp = TempDir::new("notes-file");
    let root = release_fixture(&temp, "fixture", "1.7.2", SUMMARY_CHANGELOG);
    let output = temp.join("RELEASE_NOTES.md");
    let run = Xtask::new("release-notes")
        .root(&root)
        .arg("v1.7.2")
        .flag("--output", &output)
        .run()
        .succeeds();
    assert_eq!(run.stdout, "");
    assert_eq!(read(&output), EXPECTED_NOTES);
}

#[test]
fn the_release_title_is_the_product_and_version() {
    let temp = TempDir::new("notes-title");
    let root = release_fixture(&temp, "fixture", "1.7.2", SUMMARY_CHANGELOG);
    let run = Xtask::new("release-notes")
        .root(&root)
        .arg("1.7.2")
        .arg("--title")
        .run()
        .succeeds();
    assert_eq!(run.stdout, "CDRP 1.7.2\n");
    assert_eq!(notes::title("1.8.0+build.7"), "CDRP 1.8.0+build.7");
}

#[test]
fn build_metadata_selects_its_changelog_section_literally() {
    let temp = TempDir::new("notes-build");
    let root = release_fixture(
        &temp,
        "fixture",
        "1.8.0+build.7",
        "# Changelog\n\n## [1.8.0buildX7] - 2026-07-09\n\n- This section must not match the release version.\n\n## [1.8.0+build.7] - 2026-07-09\n\n- Literal build metadata selects this section.\n",
    );
    let body = Xtask::new("release-notes")
        .root(&root)
        .arg("1.8.0+build.7")
        .run()
        .succeeds()
        .stdout;
    assert!(
        body.contains("Literal build metadata selects this section."),
        "{body}"
    );
    assert!(!body.contains("This section must not match"), "{body}");
    assert_plain_notes(&body);
}

#[test]
fn a_missing_section_fails() {
    let temp = TempDir::new("notes-missing");
    let root = release_fixture(&temp, "fixture", "1.7.2", SUMMARY_CHANGELOG);
    Xtask::new("release-notes")
        .root(&root)
        .arg("9.9.9")
        .run()
        .fails_with("non-empty CHANGELOG.md section for [9.9.9]");
}

#[test]
fn link_definitions_below_the_oldest_release_are_not_part_of_its_notes() {
    let temp = TempDir::new("notes-links");
    let root = release_fixture(
        &temp,
        "fixture",
        "1.7.2",
        "# Changelog\n\n## [1.7.2] - 2026-07-09\n\nSummary.\n\n### Fixed\n\n- Fix.\n\n[Unreleased]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.2...HEAD\n[1.7.2]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/tag/v1.7.2\n",
    );
    let body = Xtask::new("release-notes")
        .root(&root)
        .arg("1.7.2")
        .run()
        .succeeds()
        .stdout;
    assert_eq!(body, "Summary.\n\n### Fixed\n\n- Fix.\n");
}

fn long_section(lines: usize) -> String {
    let mut section = String::from("Summary of a very large release.\n\n### Added\n\n");
    while section.lines().count() < lines {
        section.push_str(&format!("- Entry {}\n", section.lines().count()));
    }
    section
}

#[test]
fn only_a_section_over_the_limit_gets_the_extra_sections() {
    assert_eq!(LONG_SECTION_LINES, 150);
    assert!(!notes::is_long(&long_section(150)));
    assert!(notes::is_long(&long_section(151)));
}

#[test]
fn a_long_section_appends_assets_integrity_and_the_full_changelog() {
    let long = LongNotes {
        repository: "xt0n1-t3ch/Codex-Discord-Rich-Presence",
        tag: "v1.8.0+build.7",
        previous_tag: Some("v1.7.2"),
    };
    let body = notes::render(&long_section(151), Some(&long));
    assert!(body.starts_with("Summary of a very large release.\n"));
    for expected in [
        "\n## Release assets\n",
        "- codex-discord-rich-presence-windows-x64.exe\n",
        "- codex-discord-rich-presence-windows-x64.spdx.json\n",
        "- codex-discord-rich-presence-windows-arm64.exe\n",
        "- codex-discord-rich-presence-windows-arm64.spdx.json\n",
        "- codex-discord-rich-presence-linux-x64\n",
        "- codex-discord-rich-presence-macos-x64\n",
        "- codex-discord-rich-presence-macos-arm64\n",
        "- codex-app-logo.png\n",
        "- chatgpt-app-logo.jpg\n",
        "- SHA256SUMS.txt\n",
        "\n## Integrity\n\nVerify every downloaded payload against SHA256SUMS.txt.\n",
        "\n## Full changelog\n\nhttps://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.7.2...v1.8.0%2Bbuild.7\n",
    ] {
        assert!(body.contains(expected), "missing {expected:?} in {body}");
    }
    assert!(!body.lines().any(|line| line.starts_with("# ")));
}

#[test]
fn without_a_previous_tag_the_full_changelog_lists_the_tag_commits() {
    let long = LongNotes {
        repository: "xt0n1-t3ch/Codex-Discord-Rich-Presence",
        tag: "v1.8.0",
        previous_tag: None,
    };
    let body = notes::render(&long_section(151), Some(&long));
    assert!(body.ends_with(
        "\n## Full changelog\n\nhttps://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/commits/v1.8.0\n"
    ));
}

#[test]
fn the_cli_appends_the_extras_for_a_long_section() {
    let temp = TempDir::new("notes-long");
    let changelog = format!(
        "# Changelog\n\n## [1.7.2] - 2026-07-09\n\n{}",
        long_section(151)
    );
    let root = release_fixture(&temp, "fixture", "1.7.2", &changelog);
    let body = Xtask::new("release-notes")
        .root(&root)
        .arg("1.7.2")
        .flag("--previous-tag", "v1.7.1")
        .run()
        .succeeds()
        .stdout;
    assert!(body.contains("\n## Release assets\n"), "{body}");
    assert!(
        body.contains("compare/v1.7.1...v1.7.2\n"),
        "the compare link is missing: {body}"
    );
}

#[test]
fn a_long_section_without_repository_metadata_fails() {
    let temp = TempDir::new("notes-long-meta");
    let changelog = format!(
        "# Changelog\n\n## [1.7.2] - 2026-07-09\n\n{}",
        long_section(151)
    );
    let root = release_fixture(&temp, "fixture", "1.7.2", &changelog);
    write(&root.join(".github/repository.toml"), "");
    Xtask::new("release-notes")
        .root(&root)
        .arg("1.7.2")
        .run()
        .fails_with("repository.toml");
}
