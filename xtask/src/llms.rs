//! Generates llms.txt and llms-full.txt from docs/llms.toml, and checks they are current.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::meta;
use crate::text::normalize_newlines;

pub const CONFIG_PATH: &str = "docs/llms.toml";
pub const SUMMARY_FILE: &str = "llms.txt";
pub const FULL_FILE: &str = "llms-full.txt";
const FIRST_DOCUMENT: &str = "README.md";
const LAST_SECTION: &str = "Optional";

#[derive(Debug, Deserialize)]
pub struct Config {
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default, rename = "section")]
    pub sections: Vec<Section>,
}

#[derive(Debug, Deserialize)]
pub struct Section {
    pub title: String,
    #[serde(default, rename = "link")]
    pub links: Vec<Link>,
}

#[derive(Debug, Deserialize)]
pub struct Link {
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Generated {
    pub summary: String,
    pub full: String,
}

pub fn raw_url(repository: &str, path: &str) -> String {
    format!("https://raw.githubusercontent.com/{repository}/main/{path}")
}

fn validate_path(path: &str) -> Result<()> {
    let clean = !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !clean {
        bail!("{CONFIG_PATH}: '{path}' must be a relative path with forward slashes");
    }
    Ok(())
}

fn validate(config: &Config) -> Result<()> {
    let Some(last) = config
        .sections
        .last()
        .filter(|section| section.title == LAST_SECTION)
    else {
        bail!("{CONFIG_PATH}: the last section must be '{LAST_SECTION}'");
    };
    if last.links.last().map(|link| link.path.as_str()) != Some(FULL_FILE) {
        bail!("{CONFIG_PATH}: the last '{LAST_SECTION}' link must point to {FULL_FILE}");
    }
    for link in config.sections.iter().flat_map(|section| &section.links) {
        validate_path(&link.path)?;
    }
    Ok(())
}

fn render_summary(config: &Config, repository: &str) -> String {
    let mut blocks = vec![
        format!("# {}", config.name),
        format!("> {}", config.summary),
    ];
    if !config.facts.is_empty() {
        let facts: Vec<String> = config
            .facts
            .iter()
            .map(|fact| format!("- {fact}"))
            .collect();
        blocks.push(facts.join("\n"));
    }
    for section in &config.sections {
        let mut lines = vec![format!("## {}", section.title), String::new()];
        for link in &section.links {
            let url = raw_url(repository, &link.path);
            if link.description.is_empty() {
                lines.push(format!("- [{}]({url})", link.title));
            } else {
                lines.push(format!("- [{}]({url}): {}", link.title, link.description));
            }
        }
        blocks.push(lines.join("\n"));
    }
    format!("{}\n", blocks.join("\n\n"))
}

fn document_paths(config: &Config) -> Vec<&str> {
    let mut paths = vec![FIRST_DOCUMENT];
    for link in config.sections.iter().flat_map(|section| &section.links) {
        let path = link.path.as_str();
        if path != SUMMARY_FILE && path != FULL_FILE && !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

fn render_full(root: &Path, config: &Config, repository: &str) -> Result<String> {
    let mut documents = Vec::new();
    for path in document_paths(config) {
        let file = root.join(path);
        let content = fs::read_to_string(&file).with_context(|| {
            format!(
                "{CONFIG_PATH} lists '{path}', but '{}' cannot be read",
                file.display()
            )
        })?;
        documents.push(format!(
            "# {path}\n\nSource: {}\n\n{}\n",
            raw_url(repository, path),
            normalize_newlines(&content).trim_end()
        ));
    }
    Ok(documents.join("\n"))
}

pub fn generate(root: &Path) -> Result<Generated> {
    let path = root.join(CONFIG_PATH);
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading '{}'", path.display()))?;
    let config: Config = toml::from_str(&text).with_context(|| format!("parsing {CONFIG_PATH}"))?;
    validate(&config)?;
    let repository = meta::load(root)?.repository;
    Ok(Generated {
        summary: render_summary(&config, &repository),
        full: render_full(root, &config, &repository)?,
    })
}

pub fn write(root: &Path) -> Result<()> {
    let generated = generate(root)?;
    for (name, content) in [
        (SUMMARY_FILE, &generated.summary),
        (FULL_FILE, &generated.full),
    ] {
        let path = root.join(name);
        fs::write(&path, content).with_context(|| format!("writing '{}'", path.display()))?;
    }
    Ok(())
}

pub fn check(root: &Path) -> Result<()> {
    let generated = generate(root)?;
    let stale: Vec<&str> = [
        (SUMMARY_FILE, &generated.summary),
        (FULL_FILE, &generated.full),
    ]
    .into_iter()
    .filter(|(name, content)| {
        fs::read_to_string(root.join(name))
            .map(|existing| normalize_newlines(&existing) != **content)
            .unwrap_or(true)
    })
    .map(|(name, _)| name)
    .collect();
    if !stale.is_empty() {
        bail!(
            "{} out of date; run `cargo xtask llms`",
            stale.join(" and ")
        );
    }
    Ok(())
}
