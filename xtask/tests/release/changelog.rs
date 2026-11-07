//! `changelog` validation and section extraction.

use xtask::changelog::{self, CheckInput};

const REPOSITORY: &str = "xt0n1-t3ch/Codex-Discord-Rich-Presence";

const GOOD: &str = "# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The major version changes when the public contract breaks; the minor version adds features; the patch version fixes defects.

## [Unreleased]

### Added

- Something new.

## [1.1.0] - 2026-02-10

This release adds a feature and keeps the configuration schema.

### Added

- A feature.

### Fixed

- A defect.

## [1.0.0] - 2026-02-01

First stable release.

### Added

- Everything.

[Unreleased]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/tag/v1.0.0
";

fn tags() -> Vec<String> {
    vec!["v1.0.0".to_string(), "v1.1.0".to_string()]
}

fn problems(text: &str, tags: Option<&[String]>) -> Vec<String> {
    changelog::check(&CheckInput {
        text,
        repository: REPOSITORY,
        tags,
    })
}

fn only_problem(text: &str) -> String {
    let found = problems(text, Some(&tags()));
    assert_eq!(found.len(), 1, "expected exactly one problem: {found:?}");
    found.into_iter().next().expect("one problem")
}

#[test]
fn a_changelog_in_the_project_format_has_no_problems() {
    assert_eq!(problems(GOOD, Some(&tags())), Vec::<String>::new());
    assert_eq!(problems(GOOD, None), Vec::<String>::new());
}

#[test]
fn the_two_format_lines_are_required() {
    let text = GOOD.replace(
        "https://keepachangelog.com/en/2.0.0/",
        "https://keepachangelog.com/en/1.1.0/",
    );
    assert!(only_problem(&text).contains("the header must contain the line"));
}

#[test]
fn unreleased_must_come_first() {
    let text = GOOD.replace("## [Unreleased]\n\n### Added\n\n- Something new.\n\n", "");
    assert!(only_problem(&text).contains("[Unreleased]"));
}

#[test]
fn a_release_must_open_with_a_summary_paragraph() {
    let text = GOOD.replace(
        "This release adds a feature and keeps the configuration schema.\n\n",
        "",
    );
    assert!(only_problem(&text).contains("must open with a summary paragraph"));
}

#[test]
fn only_the_keep_a_changelog_sections_are_allowed() {
    let text = GOOD.replace(
        "### Fixed\n\n- A defect.",
        "### Compatibility and rollback\n\n- A defect.",
    );
    assert!(
        only_problem(&text)
            .contains("allowed sections are Added, Changed, Deprecated, Removed, Fixed, Security")
    );
}

#[test]
fn sections_keep_the_standard_order_and_appear_once() {
    let swapped = GOOD.replace(
        "### Added\n\n- A feature.\n\n### Fixed\n\n- A defect.",
        "### Fixed\n\n- A defect.\n\n### Added\n\n- A feature.",
    );
    assert!(only_problem(&swapped).contains("repeated or out of order"));
    let repeated = GOOD.replace("### Fixed\n\n- A defect.", "### Added\n\n- A defect.");
    assert!(only_problem(&repeated).contains("repeated or out of order"));
}

#[test]
fn an_empty_section_is_rejected() {
    let text = GOOD.replace("### Fixed\n\n- A defect.", "### Fixed\n");
    assert!(only_problem(&text).contains("section 'Fixed' of [1.1.0] is empty"));
}

#[test]
fn releases_are_listed_newest_first_with_a_real_date() {
    let reversed = GOOD.replace("## [1.1.0] - 2026-02-10", "## [1.1.0] - 2026-01-10");
    assert!(only_problem(&reversed).contains("is dated 2026-02-01, later than [1.1.0]"));
    let undated = GOOD.replace("## [1.1.0] - 2026-02-10", "## [1.1.0]");
    assert!(only_problem(&undated).contains("must read `## [1.1.0] - YYYY-MM-DD`"));
    let duplicate = GOOD.replace("## [1.0.0] - 2026-02-01", "## [1.1.0] - 2026-02-01");
    assert!(
        changelog::check(&CheckInput {
            text: &duplicate,
            repository: REPOSITORY,
            tags: None
        })
        .iter()
        .any(|problem| problem.contains("list releases newest first, once each"))
    );
}

#[test]
fn every_version_needs_a_compare_link_starting_with_unreleased() {
    let missing = GOOD.replace(
        "[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/tag/v1.0.0\n",
        "",
    );
    assert!(only_problem(&missing).contains("missing link definition for [1.0.0]"));
    let reordered = GOOD.replace(
        "[Unreleased]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.1.0...HEAD\n[1.1.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.0.0...v1.1.0\n",
        "[1.1.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.0.0...v1.1.0\n[Unreleased]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.1.0...HEAD\n",
    );
    assert!(only_problem(&reordered).contains("must start with [Unreleased]"));
}

#[test]
fn links_use_the_current_repository_name() {
    let text = GOOD.replace(
        "xt0n1-t3ch/Codex-Discord-Rich-Presence/compare/v1.0.0...v1.1.0",
        "xt0n1-t3ch/Codex-Discord-Presence/compare/v1.0.0...v1.1.0",
    );
    assert!(only_problem(&text).contains(
        "[1.1.0] must link to https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/"
    ));
}

#[test]
fn links_may_only_name_tags_that_exist() {
    let text = GOOD.replace("compare/v1.0.0...v1.1.0", "compare/v0.9.0...v1.1.0");
    assert!(only_problem(&text).contains("links to tag 'v0.9.0', which does not exist"));
    assert_eq!(problems(&text, None), Vec::<String>::new());
}

#[test]
fn the_newest_release_may_wait_for_its_tag() {
    let only_older = vec!["v1.0.0".to_string()];
    assert_eq!(problems(GOOD, Some(&only_older)), Vec::<String>::new());

    let older_missing = vec!["v1.1.0".to_string()];
    assert!(
        problems(GOOD, Some(&older_missing))
            .iter()
            .any(|problem| problem.contains("links to tag 'v1.0.0', which does not exist"))
    );
}

#[test]
fn unreleased_compares_the_newest_release_with_head() {
    let text = GOOD.replace("compare/v1.1.0...HEAD", "compare/v1.0.0...HEAD");
    assert!(
        only_problem(&text)
            .contains("[Unreleased] must compare the newest release tag with HEAD or main")
    );
}

#[test]
fn a_section_stops_at_the_next_release_and_drops_trailing_links() {
    let oldest = changelog::section(GOOD, "1.0.0").expect("1.0.0 has a section");
    assert_eq!(
        oldest,
        "First stable release.\n\n### Added\n\n- Everything."
    );
    let newer = changelog::section(GOOD, "1.1.0").expect("1.1.0 has a section");
    assert!(newer.starts_with("This release adds a feature"));
    assert!(!newer.contains("First stable release"));
}

#[test]
fn an_empty_or_absent_section_is_an_error() {
    let error = changelog::section(GOOD, "2.0.0").expect_err("2.0.0 is absent");
    assert!(
        error
            .to_string()
            .contains("non-empty CHANGELOG.md section for [2.0.0]")
    );
    let empty = "## [1.0.0] - 2026-02-01\n\n## [0.9.0] - 2026-01-01\n\nText.\n";
    assert!(changelog::section(empty, "1.0.0").is_err());
}

#[test]
fn an_untagged_version_may_link_to_the_commit_that_shipped_it() {
    let sha = "a4c6d9f6206a9dcbe3a941d27ce6aa08beeaf9bb";
    let text = GOOD.replace(
        "[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/tag/v1.0.0",
        &format!("[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/commit/{sha}"),
    );
    let only_newer_tag = vec!["v1.1.0".to_string()];
    let text = text.replace(
        "compare/v1.0.0...v1.1.0",
        &format!("compare/{sha}...v1.1.0"),
    );
    assert_eq!(problems(&text, Some(&only_newer_tag)), Vec::<String>::new());
}

#[test]
fn a_tagged_version_still_needs_its_tag_link_instead_of_a_commit() {
    let text = GOOD.replace(
        "[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/tag/v1.0.0",
        "[1.0.0]: https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/commit/a4c6d9f",
    );
    assert!(only_problem(&text).contains("[1.0.0] must link to"));
}
