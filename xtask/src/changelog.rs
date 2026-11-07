//! CHANGELOG.md parsing and validation against Keep a Changelog 2.0.0 and the repository's tags.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::exec;
use crate::meta;
use crate::semver::Version;

pub const HEADER_LINES: [&str; 2] = [
    "The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),",
    "and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).",
];

pub const SECTION_ORDER: [&str; 6] = [
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
];

struct Entry<'a> {
    label: &'a str,
    tail: &'a str,
    body: Vec<&'a str>,
}

struct Parsed<'a> {
    preamble: Vec<&'a str>,
    entries: Vec<Entry<'a>>,
    links: Vec<(&'a str, &'a str)>,
}

fn bracket_heading(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("##")?;
    let trimmed = rest.trim_start();
    if trimmed.len() == rest.len() {
        return None;
    }
    trimmed.strip_prefix('[')?.split_once(']')
}

fn is_iso_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    let shaped = bytes.len() == 10
        && bytes.iter().enumerate().all(|(index, byte)| {
            if index == 4 || index == 7 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        });
    if !shaped {
        return false;
    }
    let month: u32 = text[5..7].parse().unwrap_or(0);
    let day: u32 = text[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn parse_tail(tail: &str) -> Option<Option<&str>> {
    let tail = tail.trim_end_matches([' ', '\t']);
    if tail.is_empty() {
        return Some(None);
    }
    let after_space = tail.trim_start();
    if after_space.len() == tail.len() {
        return None;
    }
    let after_dash = after_space.strip_prefix('-')?;
    let date = after_dash.trim_start();
    if date.len() == after_dash.len() {
        return None;
    }
    is_iso_date(date).then_some(Some(date))
}

fn link_definition(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim().strip_prefix('[')?;
    let (label, target) = rest.split_once("]:")?;
    let url = target.split_whitespace().next()?;
    (!label.is_empty() && !label.contains(']')).then_some((label, url))
}

fn parse(text: &str) -> Parsed<'_> {
    let mut lines: Vec<&str> = text.lines().collect();
    let mut links = Vec::new();
    while let Some(&last) = lines.last() {
        if last.trim().is_empty() {
            lines.pop();
        } else if let Some(link) = link_definition(last) {
            links.push(link);
            lines.pop();
        } else {
            break;
        }
    }
    links.reverse();
    let mut preamble = Vec::new();
    let mut entries: Vec<Entry> = Vec::new();
    for line in lines {
        if let Some((label, tail)) = bracket_heading(line) {
            entries.push(Entry {
                label,
                tail,
                body: Vec::new(),
            });
        } else if let Some(entry) = entries.last_mut() {
            entry.body.push(line);
        } else {
            preamble.push(line);
        }
    }
    Parsed {
        preamble,
        entries,
        links,
    }
}

pub fn section(changelog: &str, version: &str) -> Result<String> {
    let parsed = parse(changelog);
    let body = parsed
        .entries
        .iter()
        .find(|entry| entry.label == version && parse_tail(entry.tail).is_some())
        .map(|entry| entry.body.join("\n"))
        .unwrap_or_default();
    let body = body.trim();
    if body.is_empty() {
        bail!("A non-empty CHANGELOG.md section for [{version}] is required.");
    }
    Ok(body.to_string())
}

pub fn read_section(root: &Path, version: &str) -> Result<String> {
    let path = root.join("CHANGELOG.md");
    if !path.is_file() {
        bail!("CHANGELOG.md is missing at '{}'.", path.display());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading '{}'", path.display()))?;
    section(&text, version)
}

pub struct CheckInput<'a> {
    pub text: &'a str,
    pub repository: &'a str,
    pub tags: Option<&'a [String]>,
}

fn report_empty(open: Option<(&str, usize)>, label: &str, problems: &mut Vec<String>) {
    if let Some((name, 0)) = open {
        problems.push(format!("section '{name}' of [{label}] is empty"));
    }
}

fn check_sections(entry: &Entry, needs_summary: bool, problems: &mut Vec<String>) {
    let label = entry.label;
    if needs_summary {
        match entry.body.iter().find(|line| !line.trim().is_empty()) {
            None => problems.push(format!("[{label}] has no content")),
            Some(first) if first.trim_start().starts_with(['#', '-', '*', '|', '>']) => {
                problems.push(format!(
                    "[{label}] must open with a summary paragraph before its first section"
                ));
            }
            Some(_) => {}
        }
    }
    let mut last_index: Option<usize> = None;
    let mut open: Option<(&str, usize)> = None;
    for line in &entry.body {
        if line.starts_with("# ") || line.starts_with("## ") {
            problems.push(format!("[{label}] has an unexpected heading: {line}"));
        } else if let Some(name) = line.strip_prefix("### ") {
            report_empty(open.take(), label, problems);
            let name = name.trim();
            match SECTION_ORDER.iter().position(|known| *known == name) {
                None => problems.push(format!(
                    "[{label}] has the section '{name}'; allowed sections are {}",
                    SECTION_ORDER.join(", ")
                )),
                Some(index) => {
                    if last_index.is_some_and(|previous| index <= previous) {
                        problems.push(format!(
                            "section '{name}' of [{label}] is repeated or out of order ({})",
                            SECTION_ORDER.join(", ")
                        ));
                    }
                    last_index = Some(index);
                }
            }
            open = Some((name, 0));
        } else if !line.trim().is_empty()
            && let Some((_, lines)) = open.as_mut()
        {
            *lines += 1;
        }
    }
    report_empty(open.take(), label, problems);
}

enum LinkTarget<'a> {
    Compare { from: &'a str, to: &'a str },
    Tag(&'a str),
    Commit(&'a str),
}

fn link_target<'a>(url: &'a str, prefix: &str) -> Option<LinkTarget<'a>> {
    let rest = url.strip_prefix(prefix)?;
    if let Some(range) = rest.strip_prefix("compare/") {
        let (from, to) = range.split_once("...")?;
        return Some(LinkTarget::Compare { from, to });
    }
    if let Some(sha) = rest.strip_prefix("commit/") {
        return Some(LinkTarget::Commit(sha));
    }
    rest.strip_prefix("releases/tag/").map(LinkTarget::Tag)
}

fn is_commit_sha(name: &str) -> bool {
    (7..=40).contains(&name.len()) && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_moving_ref(name: &str) -> bool {
    name == "HEAD" || name == "main"
}

fn check_links(parsed: &Parsed, released: &[&str], input: &CheckInput, problems: &mut Vec<String>) {
    let prefix = format!("https://github.com/{}/", input.repository);
    if parsed.links.first().map(|(label, _)| *label) != Some("Unreleased") {
        problems
            .push("the link definitions at the bottom must start with [Unreleased]".to_string());
    }
    let mut labels = vec!["Unreleased"];
    labels.extend(released);
    for label in labels {
        let Some((_, url)) = parsed.links.iter().find(|(name, _)| *name == label) else {
            problems.push(format!("missing link definition for [{label}]"));
            continue;
        };
        let expected_tag = format!("v{label}");
        // A version released before the repository had tags links to the commit that shipped it.
        let untagged = input.tags.is_some_and(|tags| !tags.contains(&expected_tag));
        let target = link_target(url, &prefix);
        let refs: Vec<&str> = match (&target, label) {
            (Some(LinkTarget::Compare { from, to }), "Unreleased") => {
                let latest = released.first().map(|version| format!("v{version}"));
                if !is_moving_ref(to) || latest.as_deref().is_some_and(|tag| tag != *from) {
                    problems.push(format!(
                        "[Unreleased] must compare the newest release tag with HEAD or main: {url}"
                    ));
                }
                vec![*from, *to]
            }
            (Some(LinkTarget::Compare { from, to }), _) if *to == expected_tag => vec![*from, *to],
            (Some(LinkTarget::Compare { from, to }), _) if untagged && is_commit_sha(to) => {
                vec![*from]
            }
            (Some(LinkTarget::Commit(sha)), _) if untagged && is_commit_sha(sha) => Vec::new(),
            (Some(LinkTarget::Tag(tag)), _) if label != "Unreleased" && *tag == expected_tag => {
                vec![*tag]
            }
            _ => {
                problems.push(format!(
                    "[{label}] must link to {prefix}compare/<tag>...{expected_tag} or {prefix}releases/tag/{expected_tag}, found {url}"
                ));
                continue;
            }
        };
        if let Some(tags) = input.tags {
            // The newest release may be waiting for its tag; every older one must exist.
            let newest_tag = released.first().map(|version| format!("v{version}"));
            for name in refs.into_iter().filter(|name| {
                !is_moving_ref(name) && !is_commit_sha(name) && newest_tag.as_deref() != Some(*name)
            }) {
                if !tags.iter().any(|tag| tag == name) {
                    problems.push(format!(
                        "[{label}] links to tag '{name}', which does not exist"
                    ));
                }
            }
        }
    }
}

pub fn check(input: &CheckInput) -> Vec<String> {
    let parsed = parse(input.text);
    let mut problems = Vec::new();
    for header in HEADER_LINES {
        if !parsed.preamble.iter().any(|line| line.trim_end() == header) {
            problems.push(format!("the header must contain the line: {header}"));
        }
    }
    let promises_versioning = parsed.preamble.iter().any(|line| {
        let line = line.trim();
        !line.starts_with('#')
            && !HEADER_LINES.contains(&line)
            && line.to_lowercase().contains("version")
    });
    if !promises_versioning {
        problems.push(
            "the header must follow the two format lines with a paragraph on what the version number promises"
                .to_string(),
        );
    }
    if parsed.entries.first().map(|entry| entry.label) != Some("Unreleased") {
        problems.push("the first version heading must be `## [Unreleased]`".to_string());
    }
    let mut released: Vec<&str> = Vec::new();
    let mut previous: Option<(Version, Option<&str>)> = None;
    for (index, entry) in parsed.entries.iter().enumerate() {
        if entry.label == "Unreleased" {
            if index != 0 {
                problems
                    .push("`## [Unreleased]` must appear once, before every release".to_string());
            }
            check_sections(entry, false, &mut problems);
            continue;
        }
        let label = entry.label;
        let Ok(version) = label.parse::<Version>() else {
            problems.push(format!("[{label}] is not a SemVer version"));
            continue;
        };
        let date = match parse_tail(entry.tail) {
            Some(Some(date)) => Some(date),
            _ => {
                problems.push(format!(
                    "the heading of [{label}] must read `## [{label}] - YYYY-MM-DD`"
                ));
                None
            }
        };
        if let Some((newer, newer_date)) = &previous {
            if version.precedence(newer) != std::cmp::Ordering::Less {
                problems.push(format!(
                    "[{label}] must be older than [{newer}]: list releases newest first, once each"
                ));
            }
            if let (Some(date), Some(newer_date)) = (date, newer_date)
                && date > *newer_date
            {
                problems.push(format!(
                    "[{label}] is dated {date}, later than [{newer}] ({newer_date})"
                ));
            }
        }
        previous = Some((version, date));
        released.push(label);
        check_sections(entry, true, &mut problems);
    }
    check_links(&parsed, &released, input, &mut problems);
    problems
}

pub fn git_tags(root: &Path) -> Option<Vec<String>> {
    let output = exec::captured(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["tag", "--list"]),
    )
    .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect()
    })
}

pub fn run_check(root: &Path) -> Result<String> {
    let path = root.join("CHANGELOG.md");
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading '{}'", path.display()))?;
    let repository = meta::load(root)?.repository;
    let tags = git_tags(root);
    let problems = check(&CheckInput {
        text: &text,
        repository: &repository,
        tags: tags.as_deref(),
    });
    if !problems.is_empty() {
        bail!(
            "CHANGELOG.md does not follow the project format:\n- {}",
            problems.join("\n- ")
        );
    }
    let releases = parse(&text).entries.len().saturating_sub(1);
    Ok(format!(
        "CHANGELOG.md follows the project format ({releases} releases)"
    ))
}
