//! Release contract: ties a tag to the Cargo version, README, changelog and the release contract file.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::assets::TARGETS;
use crate::changelog;
use crate::hash::CHECKSUM_MANIFEST;
use crate::metadata::{self, Dependencies, Metadata};
use crate::notes;
use crate::semver::parse_release_tag;

pub const CONTRACT_PATH: &str = "scripts/release-contract.json";

#[derive(Debug, Serialize)]
pub struct ReleaseMetadata {
    pub tag_name: String,
    pub version: String,
    pub release_name: String,
    pub is_prerelease: bool,
    pub make_latest: bool,
}

#[derive(Debug, Deserialize)]
struct Contract {
    schema_version: u32,
    product: Product,
    core: Core,
    configuration: Configuration,
    release: Release,
}

#[derive(Debug, Deserialize)]
struct Product {
    version: String,
}

#[derive(Debug, Deserialize)]
struct Core {
    package: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct Configuration {
    schema_version: u32,
    source: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    windows_sbom: String,
    checksum_manifest: String,
    windows_targets: Vec<WindowsTarget>,
}

#[derive(Debug, Deserialize)]
struct WindowsTarget {
    target: String,
    architecture: String,
    artifact: String,
    sbom: String,
    pe_machine: String,
}

pub fn declares_schema_version(source: &str, schema: u32) -> bool {
    source
        .match_indices("SCHEMA_VERSION")
        .any(|(index, matched)| {
            let rest = source[index + matched.len()..].trim_start();
            let Some(rest) = rest.strip_prefix(':') else {
                return false;
            };
            let Some(rest) = rest.trim_start().strip_prefix("u32") else {
                return false;
            };
            let Some(rest) = rest.trim_start().strip_prefix('=') else {
                return false;
            };
            let rest = rest.trim_start();
            let digits = rest
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len());
            rest[..digits] == schema.to_string() && rest[digits..].trim_start().starts_with(';')
        })
}

fn check_readme(root: &Path, version: &str) -> Result<()> {
    let path = root.join("README.md");
    if !path.is_file() {
        bail!("README.md is missing at '{}'.", path.display());
    }
    let readme = fs::read_to_string(&path)
        .with_context(|| format!("reading '{}'", path.display()))?
        .to_lowercase();
    let lowered = version.to_lowercase();
    if !readme.contains(&format!("release v{lowered}"))
        && !readme.contains(&format!("release-v{lowered}"))
    {
        bail!("README.md release badge does not match version '{version}'.");
    }
    Ok(())
}

fn expected_windows_targets() -> Vec<String> {
    let mut expected: Vec<String> = TARGETS
        .iter()
        .filter(|target| target.is_windows())
        .map(|target| {
            format!(
                "{}|{}|{}|{}|0x{:04X}",
                target.triple,
                target.architecture.name(),
                target.artifact,
                target.sbom.unwrap_or_default(),
                target.architecture.machine()
            )
        })
        .collect();
    expected.sort();
    expected
}

fn check_repository_contract(root: &Path, version: &str, metadata: &Metadata) -> Result<()> {
    let path = root.join(CONTRACT_PATH);
    if !path.is_file() {
        return Ok(());
    }
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading '{}'", path.display()))?;
    let contract: Contract = serde_json::from_str(&text)
        .map_err(|error| anyhow!("{CONTRACT_PATH} is not valid JSON: {error}"))?;
    if contract.schema_version != 1 {
        bail!("{CONTRACT_PATH} schema_version must be 1.");
    }
    if contract.product.version != version {
        bail!(
            "Release contract product version '{}' does not match '{version}'.",
            contract.product.version
        );
    }
    let cores: Vec<_> = metadata
        .packages
        .iter()
        .filter(|package| package.name == contract.core.package)
        .collect();
    if cores.len() != 1 || cores[0].version != contract.core.version {
        bail!(
            "Canonical core must resolve exactly once as {}@{}.",
            contract.core.package,
            contract.core.version
        );
    }
    let source_path = root.join(&contract.configuration.source);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("reading '{}'", source_path.display()))?;
    if !declares_schema_version(&source, contract.configuration.schema_version) {
        bail!(
            "Config source does not declare schema {}.",
            contract.configuration.schema_version
        );
    }
    if !contract.release.windows_sbom.ends_with(".spdx.json")
        || contract.release.checksum_manifest != CHECKSUM_MANIFEST
    {
        bail!("Release integrity contract must require an SPDX JSON SBOM and {CHECKSUM_MANIFEST}.");
    }
    let mut actual: Vec<String> = contract
        .release
        .windows_targets
        .iter()
        .map(|target| {
            format!(
                "{}|{}|{}|{}|{}",
                target.target, target.architecture, target.artifact, target.sbom, target.pe_machine
            )
        })
        .collect();
    actual.sort();
    if actual != expected_windows_targets() {
        bail!(
            "Release contract must declare the exact x64 and ARM64 Windows artifacts, SBOMs, targets, and PE machines."
        );
    }
    Ok(())
}

pub fn check(root: &Path, tag: &str, ref_type: Option<&str>) -> Result<ReleaseMetadata> {
    if let Some(kind) = ref_type.map(str::trim).filter(|kind| !kind.is_empty())
        && kind != "tag"
    {
        bail!("Release validation requires a tag ref, received '{kind}'.");
    }
    let parsed = parse_release_tag(tag)?;
    let version = parsed.to_string();
    let metadata = metadata::load(root, Dependencies::Excluded)?;
    let package = metadata.root_package(root)?;
    if package.version != version {
        bail!(
            "Tag version '{version}' does not match Cargo package version '{}'.",
            package.version
        );
    }
    check_readme(root, &version)?;
    check_repository_contract(root, &version, &metadata)?;
    changelog::read_section(root, &version)?;
    Ok(ReleaseMetadata {
        tag_name: tag.to_string(),
        release_name: notes::title(&version),
        is_prerelease: parsed.is_prerelease(),
        make_latest: !parsed.is_prerelease(),
        version,
    })
}

pub fn append_github_output(path: &Path, release: &ReleaseMetadata) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening '{}'", path.display()))?;
    writeln!(file, "tag_name={}", release.tag_name)?;
    writeln!(file, "version={}", release.version)?;
    writeln!(file, "release_name={}", release.release_name)?;
    writeln!(file, "is_prerelease={}", release.is_prerelease)?;
    writeln!(file, "make_latest={}", release.make_latest)?;
    Ok(())
}
