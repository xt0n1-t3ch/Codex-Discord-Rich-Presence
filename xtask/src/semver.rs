//! Semantic Versioning 2.0.0 parsing and precedence.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use anyhow::{Result, anyhow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
    pre: Vec<String>,
    build: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidVersion(String);

impl fmt::Display for InvalidVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}' is not a valid SemVer version", self.0)
    }
}

impl std::error::Error for InvalidVersion {}

fn numeric(part: &str) -> Option<u64> {
    let canonical = !part.is_empty()
        && part.bytes().all(|byte| byte.is_ascii_digit())
        && (part.len() == 1 || !part.starts_with('0'));
    if canonical { part.parse().ok() } else { None }
}

fn valid_prerelease_identifier(identifier: &str) -> bool {
    if identifier.is_empty()
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return false;
    }
    let all_digits = identifier.bytes().all(|byte| byte.is_ascii_digit());
    !all_digits || numeric(identifier).is_some()
}

fn valid_build_identifier(identifier: &str) -> bool {
    !identifier.is_empty()
        && identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn compare_identifier(left: &str, right: &str) -> Ordering {
    let left_numeric = left.bytes().all(|byte| byte.is_ascii_digit());
    let right_numeric = right.bytes().all(|byte| byte.is_ascii_digit());
    match (left_numeric, right_numeric) {
        (true, true) => left.len().cmp(&right.len()).then_with(|| left.cmp(right)),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.cmp(right),
    }
}

impl FromStr for Version {
    type Err = InvalidVersion;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidVersion(text.to_string());
        let (rest, build) = match text.split_once('+') {
            Some((rest, build)) => (rest, Some(build)),
            None => (text, None),
        };
        let (core, pre) = match rest.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (rest, None),
        };
        let mut parts = core.split('.');
        let (Some(major), Some(minor), Some(patch), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        let major = numeric(major).ok_or_else(invalid)?;
        let minor = numeric(minor).ok_or_else(invalid)?;
        let patch = numeric(patch).ok_or_else(invalid)?;
        let pre: Vec<String> = match pre {
            Some(pre) => pre.split('.').map(str::to_string).collect(),
            None => Vec::new(),
        };
        if !pre.iter().all(|id| valid_prerelease_identifier(id)) {
            return Err(invalid());
        }
        if let Some(build) = build
            && !build.split('.').all(valid_build_identifier)
        {
            return Err(invalid());
        }
        Ok(Self {
            major,
            minor,
            patch,
            pre,
            build: build.map(str::to_string),
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        if let Some(build) = &self.build {
            write!(f, "+{build}")?;
        }
        Ok(())
    }
}

impl Version {
    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    pub fn precedence(&self, other: &Self) -> Ordering {
        let core =
            (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch));
        if core != Ordering::Equal {
            return core;
        }
        match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                for (left, right) in self.pre.iter().zip(&other.pre) {
                    let order = compare_identifier(left, right);
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        }
    }
}

pub fn parse_release_tag(tag: &str) -> Result<Version> {
    tag.strip_prefix('v')
        .and_then(|version| version.parse::<Version>().ok())
        .ok_or_else(|| {
            anyhow!(
                "Tag '{tag}' is not a valid SemVer release tag (expected vMAJOR.MINOR.PATCH with optional prerelease/build metadata)."
            )
        })
}
