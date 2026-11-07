//! `llms` generation and the `--check` mode.

use std::path::PathBuf;

use crate::common::{REPOSITORY_TOML, TempDir, Xtask, read, write};

const RAW: &str = "https://raw.githubusercontent.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/main";

const CONFIG: &str = r#"name = "Codex Discord Rich Presence"
summary = "Discord Rich Presence for Codex."
facts = ["One binary.", "No telemetry."]

[[section]]
title = "Docs"

[[section.link]]
path = "docs/index.md"
title = "Documentation index"
description = "Task-based guide."

[[section]]
title = "Development"

[[section.link]]
path = "docs/releasing.md"
title = "Releasing"

[[section]]
title = "Optional"

[[section.link]]
path = "CHANGELOG.md"
title = "Changelog"
description = "Release history."

[[section.link]]
path = "llms-full.txt"
title = "Full documentation"
description = "Every document above in one file."
"#;

fn fixture(temp: &TempDir, config: &str) -> PathBuf {
    let root = temp.join("fixture");
    write(&root.join("docs/llms.toml"), config);
    write(&root.join(".github/repository.toml"), REPOSITORY_TOML);
    write(&root.join("README.md"), "# Readme\r\n\r\nHello.\r\n");
    write(&root.join("docs/index.md"), "# Index\n\nIndex text.\n\n\n");
    write(&root.join("docs/releasing.md"), "# Releasing\n\nSteps.\n");
    write(&root.join("CHANGELOG.md"), "# Changelog\n\nHistory.\n");
    root
}

fn llms(root: &std::path::Path, check: bool) -> crate::common::Run {
    let command = Xtask::new("llms").root(root);
    if check {
        command.arg("--check").run()
    } else {
        command.run()
    }
}

#[test]
fn the_summary_follows_llmstxt_org_with_raw_links_on_main() {
    let temp = TempDir::new("llms-summary");
    let root = fixture(&temp, CONFIG);
    llms(&root, false).succeeds();
    let expected = format!(
        "# Codex Discord Rich Presence\n\n> Discord Rich Presence for Codex.\n\n- One binary.\n- No telemetry.\n\n## Docs\n\n- [Documentation index]({RAW}/docs/index.md): Task-based guide.\n\n## Development\n\n- [Releasing]({RAW}/docs/releasing.md)\n\n## Optional\n\n- [Changelog]({RAW}/CHANGELOG.md): Release history.\n- [Full documentation]({RAW}/llms-full.txt): Every document above in one file.\n"
    );
    assert_eq!(read(&root.join("llms.txt")), expected);
}

#[test]
fn the_full_file_holds_the_readme_then_each_linked_document_under_its_path() {
    let temp = TempDir::new("llms-full");
    let root = fixture(&temp, CONFIG);
    llms(&root, false).succeeds();
    let expected = format!(
        "# README.md\n\nSource: {RAW}/README.md\n\n# Readme\n\nHello.\n\n# docs/index.md\n\nSource: {RAW}/docs/index.md\n\n# Index\n\nIndex text.\n\n# docs/releasing.md\n\nSource: {RAW}/docs/releasing.md\n\n# Releasing\n\nSteps.\n\n# CHANGELOG.md\n\nSource: {RAW}/CHANGELOG.md\n\n# Changelog\n\nHistory.\n"
    );
    assert_eq!(read(&root.join("llms-full.txt")), expected);
}

#[test]
fn check_passes_after_generation_and_fails_when_a_source_changes() {
    let temp = TempDir::new("llms-check");
    let root = fixture(&temp, CONFIG);
    llms(&root, true).fails_with("out of date");
    llms(&root, false).succeeds();
    llms(&root, true).succeeds();
    write(&root.join("README.md"), "# Readme\n\nChanged.\n");
    llms(&root, true).fails_with("llms-full.txt out of date; run `cargo xtask llms`");
    llms(&root, false).succeeds();
    std::fs::remove_file(root.join("llms.txt")).expect("removed");
    llms(&root, true).fails_with("llms.txt out of date");
}

#[test]
fn line_endings_do_not_make_the_check_fail() {
    let temp = TempDir::new("llms-crlf");
    let root = fixture(&temp, CONFIG);
    llms(&root, false).succeeds();
    for name in ["llms.txt", "llms-full.txt"] {
        let text = read(&root.join(name)).replace('\n', "\r\n");
        write(&root.join(name), text);
    }
    llms(&root, true).succeeds();
}

#[test]
fn the_last_optional_link_must_point_to_the_full_file() {
    let temp = TempDir::new("llms-rules");
    let root = fixture(&temp, &CONFIG.replace("llms-full.txt", "other.txt"));
    llms(&root, false).fails_with("must point to llms-full.txt");
    let root = fixture(
        &temp,
        &CONFIG.replace("title = \"Optional\"", "title = \"Extras\""),
    );
    llms(&root, false).fails_with("the last section must be 'Optional'");
}

#[test]
fn links_must_exist_and_stay_inside_the_repository() {
    let temp = TempDir::new("llms-paths");
    let root = fixture(
        &temp,
        &CONFIG.replace("docs/releasing.md", "docs/missing.md"),
    );
    llms(&root, false).fails_with("docs/missing.md");
    let root = fixture(&temp, &CONFIG.replace("docs/releasing.md", "../secret.md"));
    llms(&root, false).fails_with("must be a relative path with forward slashes");
    let root = fixture(&temp, &CONFIG.replace("docs/releasing.md", "/etc/passwd"));
    llms(&root, false).fails_with("must be a relative path with forward slashes");
}
