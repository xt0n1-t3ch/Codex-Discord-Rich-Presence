//! `meta` parsing and difference reporting.

use xtask::meta::{self, Published, RepositoryMeta};

use crate::common::{REPOSITORY_TOML, repository_root};

fn wanted() -> RepositoryMeta {
    meta::parse(REPOSITORY_TOML).expect("the fixture is valid")
}

fn rejected(text: &str) -> String {
    format!(
        "{:#}",
        meta::parse(text).expect_err("the metadata must be rejected")
    )
}

#[test]
fn the_repository_file_holds_the_agreed_description_homepage_and_topics() {
    let loaded = meta::load(&repository_root()).expect(".github/repository.toml is valid");
    assert_eq!(loaded.repository, "xt0n1-t3ch/Codex-Discord-Rich-Presence");
    assert_eq!(
        loaded.description,
        "Discord Rich Presence for OpenAI Codex: shows your Codex CLI, VS Code extension, Codex App, ChatGPT App and OpenCode sessions on Discord with model, effort, context, cost and plan limits, plus a live terminal dashboard. One local Rust binary for Windows, macOS and Linux, no telemetry."
    );
    assert_eq!(
        loaded.homepage,
        "https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest"
    );
    assert_eq!(
        loaded.topics,
        [
            "codex",
            "codex-cli",
            "codex-app",
            "openai",
            "chatgpt",
            "opencode",
            "vscode",
            "discord",
            "discord-rich-presence",
            "rich-presence",
            "discord-rpc",
            "tui",
            "ratatui",
            "cli",
            "rust",
            "usage-monitor",
            "cost-tracking",
            "local-first",
            "developer-tools",
            "ai-tools",
        ]
    );
}

#[test]
fn github_limits_are_enforced_before_anything_is_sent() {
    let long = "x".repeat(351);
    assert!(
        rejected(&REPOSITORY_TOML.replace("Fixture description.", &long))
            .contains("1 to 350 characters")
    );
    assert!(
        rejected(&REPOSITORY_TOML.replace("Fixture description.", ""))
            .contains("1 to 350 characters")
    );
    assert!(
        rejected(&REPOSITORY_TOML.replace("https://github.com", "http://github.com"))
            .contains("https URL")
    );
    assert!(
        rejected(&REPOSITORY_TOML.replace("xt0n1-t3ch/Codex-Discord-Rich-Presence\"", "no slug\""))
            .contains("owner/name slug")
    );
    let topics: Vec<String> = (0..21).map(|index| format!("\"topic-{index}\"")).collect();
    let too_many = REPOSITORY_TOML.replace(
        "[\"codex\", \"discord\"]",
        &format!("[{}]", topics.join(", ")),
    );
    assert!(rejected(&too_many).contains("1 to 20 topics, found 21"));
    assert!(
        rejected(&REPOSITORY_TOML.replace("[\"codex\", \"discord\"]", "[]"))
            .contains("1 to 20 topics, found 0")
    );
    assert!(
        rejected(&REPOSITORY_TOML.replace("\"discord\"", "\"Discord\""))
            .contains("topic 'Discord'")
    );
    assert!(
        rejected(&REPOSITORY_TOML.replace("\"discord\"", "\"codex\"")).contains("listed twice")
    );
    assert!(rejected(&REPOSITORY_TOML.replace("homepage", "site")).contains("homepage"));
}

fn published(wanted: &RepositoryMeta) -> Published {
    Published {
        description: wanted.description.clone(),
        homepage: wanted.homepage.clone(),
        topics: wanted.topics.iter().rev().cloned().collect(),
    }
}

#[test]
fn matching_metadata_has_no_differences_whatever_the_topic_order() {
    let wanted = wanted();
    assert_eq!(
        meta::differences(&wanted, &published(&wanted)),
        Vec::<String>::new()
    );
}

#[test]
fn every_difference_is_reported() {
    let wanted = wanted();
    let drifted = Published {
        description: "Old description.".to_string(),
        homepage: String::new(),
        topics: vec!["codex".to_string(), "legacy".to_string()],
    };
    let problems = meta::differences(&wanted, &drifted);
    assert_eq!(problems.len(), 4, "{problems:?}");
    assert!(problems[0].starts_with("description differs"));
    assert!(problems[1].starts_with("homepage differs"));
    assert_eq!(problems[2], "topics missing on GitHub: discord");
    assert_eq!(problems[3], "topics on GitHub that are not listed: legacy");
}
