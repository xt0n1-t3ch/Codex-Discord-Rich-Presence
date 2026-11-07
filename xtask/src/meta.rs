//! GitHub repository description, homepage and topics, compared or applied from .github/repository.toml.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::github;
use crate::text::is_repository_slug;

pub const PATH: &str = ".github/repository.toml";
const MAX_DESCRIPTION_CHARS: usize = 350;
const MAX_TOPICS: usize = 20;
const MAX_TOPIC_CHARS: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RepositoryMeta {
    pub repository: String,
    pub description: String,
    pub homepage: String,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub description: String,
    pub homepage: String,
    pub topics: Vec<String>,
}

fn valid_topic(topic: &str) -> bool {
    let mut bytes = topic.bytes();
    topic.len() <= MAX_TOPIC_CHARS
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub fn parse(text: &str) -> Result<RepositoryMeta> {
    let meta: RepositoryMeta = toml::from_str(text).with_context(|| format!("parsing {PATH}"))?;
    if !is_repository_slug(&meta.repository) {
        bail!(
            "{PATH}: repository '{}' is not an owner/name slug",
            meta.repository
        );
    }
    let description_chars = meta.description.chars().count();
    if description_chars == 0 || description_chars > MAX_DESCRIPTION_CHARS {
        bail!(
            "{PATH}: the description must hold 1 to {MAX_DESCRIPTION_CHARS} characters, found {description_chars}"
        );
    }
    if !meta.homepage.starts_with("https://") {
        bail!("{PATH}: the homepage must be an https URL");
    }
    if meta.topics.is_empty() || meta.topics.len() > MAX_TOPICS {
        bail!(
            "{PATH}: GitHub allows 1 to {MAX_TOPICS} topics, found {}",
            meta.topics.len()
        );
    }
    for (index, topic) in meta.topics.iter().enumerate() {
        if !valid_topic(topic) {
            bail!(
                "{PATH}: topic '{topic}' must be lowercase letters, digits and hyphens, up to {MAX_TOPIC_CHARS} characters"
            );
        }
        if meta.topics[..index].contains(topic) {
            bail!("{PATH}: topic '{topic}' is listed twice");
        }
    }
    Ok(meta)
}

pub fn load(root: &Path) -> Result<RepositoryMeta> {
    let path = root.join(PATH);
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading '{}'", path.display()))?;
    parse(&text)
}

pub fn differences(wanted: &RepositoryMeta, published: &Published) -> Vec<String> {
    let mut problems = Vec::new();
    if wanted.description != published.description {
        problems.push(format!(
            "description differs\n  wanted:    {}\n  published: {}",
            wanted.description, published.description
        ));
    }
    if wanted.homepage != published.homepage {
        problems.push(format!(
            "homepage differs\n  wanted:    {}\n  published: {}",
            wanted.homepage, published.homepage
        ));
    }
    let missing: Vec<&str> = wanted
        .topics
        .iter()
        .filter(|topic| !published.topics.contains(topic))
        .map(String::as_str)
        .collect();
    let extra: Vec<&str> = published
        .topics
        .iter()
        .filter(|topic| !wanted.topics.contains(topic))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        problems.push(format!("topics missing on GitHub: {}", missing.join(", ")));
    }
    if !extra.is_empty() {
        problems.push(format!(
            "topics on GitHub that are not listed: {}",
            extra.join(", ")
        ));
    }
    problems
}

fn text_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn fetch(repository: &str) -> Result<Published> {
    let repo = github::get(&format!("repos/{repository}"), false)?
        .context("GitHub returned no repository")?;
    let topics = github::get(&format!("repos/{repository}/topics"), false)?
        .context("GitHub returned no topics")?;
    let names = topics
        .get("names")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Ok(Published {
        description: text_field(&repo, "description"),
        homepage: text_field(&repo, "homepage"),
        topics: names,
    })
}

pub fn check(root: &Path) -> Result<()> {
    let wanted = load(root)?;
    let problems = differences(&wanted, &fetch(&wanted.repository)?);
    if !problems.is_empty() {
        bail!(
            "GitHub metadata of {} differs from {PATH} (run `cargo xtask meta --apply`):\n- {}",
            wanted.repository,
            problems.join("\n- ")
        );
    }
    Ok(())
}

pub fn apply(root: &Path) -> Result<()> {
    let wanted = load(root)?;
    let endpoint = format!("repos/{}", wanted.repository);
    github::send(
        "PATCH",
        &endpoint,
        &json!({ "description": wanted.description, "homepage": wanted.homepage }),
    )?;
    github::send(
        "PUT",
        &format!("{endpoint}/topics"),
        &json!({ "names": wanted.topics }),
    )?;
    check(root)
}
