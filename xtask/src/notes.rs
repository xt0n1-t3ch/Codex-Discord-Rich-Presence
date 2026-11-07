//! Release notes: the title and the body taken from the version's changelog section.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::assets;
use crate::changelog;
use crate::exec;
use crate::hash::CHECKSUM_MANIFEST;
use crate::meta;
use crate::text::percent_encode;

/// Short product name for release titles, so GitHub's Releases list shows the version.
pub const PRODUCT: &str = "CDRP";
pub const LONG_SECTION_LINES: usize = 150;

pub fn title(version: &str) -> String {
    format!("{PRODUCT} {version}")
}

pub fn is_long(section: &str) -> bool {
    section.trim().lines().count() > LONG_SECTION_LINES
}

pub struct LongNotes<'a> {
    pub repository: &'a str,
    pub tag: &'a str,
    pub previous_tag: Option<&'a str>,
}

pub fn full_changelog_url(long: &LongNotes) -> String {
    let tag = percent_encode(long.tag);
    match long.previous_tag {
        Some(previous) => format!(
            "https://github.com/{}/compare/{}...{tag}",
            long.repository,
            percent_encode(previous)
        ),
        None => format!("https://github.com/{}/commits/{tag}", long.repository),
    }
}

pub fn render(section: &str, long: Option<&LongNotes>) -> String {
    let mut body = format!("{}\n", section.trim());
    if let Some(long) = long {
        body.push_str("\n## Release assets\n\n");
        for name in assets::payload_names() {
            body.push_str(&format!("- {name}\n"));
        }
        body.push_str(&format!("- {CHECKSUM_MANIFEST}\n"));
        body.push_str(&format!(
            "\n## Integrity\n\nVerify every downloaded payload against {CHECKSUM_MANIFEST}.\n"
        ));
        body.push_str(&format!(
            "\n## Full changelog\n\n{}\n",
            full_changelog_url(long)
        ));
    }
    body
}

fn previous_tag(root: &Path, tag: &str) -> Option<String> {
    let output = exec::captured(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["describe", "--tags", "--abbrev=0"])
            .arg(format!("{tag}^")),
    )
    .ok()?;
    let found = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !found.is_empty()).then_some(found)
}

pub fn build(root: &Path, version: &str, previous_override: Option<&str>) -> Result<String> {
    let section = changelog::read_section(root, version)?;
    if !is_long(&section) {
        return Ok(render(&section, None));
    }
    let repository = meta::load(root)?.repository;
    let tag = format!("v{version}");
    let previous = previous_override
        .map(str::to_string)
        .or_else(|| previous_tag(root, &tag));
    Ok(render(
        &section,
        Some(&LongNotes {
            repository: &repository,
            tag: &tag,
            previous_tag: previous.as_deref(),
        }),
    ))
}

pub fn write(path: &Path, body: &str) -> Result<()> {
    fs::write(path, body).with_context(|| format!("writing '{}'", path.display()))
}
