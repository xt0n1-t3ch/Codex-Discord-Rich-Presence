//! Release payload layout and assembly of downloaded build artifacts into one checksummed folder.

use std::fs;
use std::path::{self, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::hash;
use crate::pe::{self, Architecture};

#[derive(Debug, Clone, Copy)]
pub struct ReleaseTarget {
    pub triple: &'static str,
    pub os_name: &'static str,
    pub architecture: Architecture,
    pub artifact: &'static str,
    pub sbom: Option<&'static str>,
}

impl ReleaseTarget {
    pub fn is_windows(&self) -> bool {
        self.os_name == "windows"
    }
}

pub const TARGETS: [ReleaseTarget; 5] = [
    ReleaseTarget {
        triple: "x86_64-pc-windows-msvc",
        os_name: "windows",
        architecture: Architecture::X64,
        artifact: "codex-discord-rich-presence-windows-x64.exe",
        sbom: Some("codex-discord-rich-presence-windows-x64.spdx.json"),
    },
    ReleaseTarget {
        triple: "aarch64-pc-windows-msvc",
        os_name: "windows",
        architecture: Architecture::Arm64,
        artifact: "codex-discord-rich-presence-windows-arm64.exe",
        sbom: Some("codex-discord-rich-presence-windows-arm64.spdx.json"),
    },
    ReleaseTarget {
        triple: "x86_64-unknown-linux-gnu",
        os_name: "linux",
        architecture: Architecture::X64,
        artifact: "codex-discord-rich-presence-linux-x64",
        sbom: None,
    },
    ReleaseTarget {
        triple: "x86_64-apple-darwin",
        os_name: "macos",
        architecture: Architecture::X64,
        artifact: "codex-discord-rich-presence-macos-x64",
        sbom: None,
    },
    ReleaseTarget {
        triple: "aarch64-apple-darwin",
        os_name: "macos",
        architecture: Architecture::Arm64,
        artifact: "codex-discord-rich-presence-macos-arm64",
        sbom: None,
    },
];

pub const LOGOS: [(&str, &str); 2] = [
    ("codex-app-logo.png", "assets/branding/codex-app.png"),
    ("chatgpt-app-logo.jpg", "assets/branding/chatgpt-app.jpg"),
];

pub fn binary_payloads() -> Vec<&'static str> {
    let mut names = Vec::new();
    for target in &TARGETS {
        names.push(target.artifact);
        names.extend(target.sbom);
    }
    names
}

pub fn payload_names() -> Vec<&'static str> {
    let mut names = binary_payloads();
    names.extend(LOGOS.iter().map(|(name, _)| *name));
    names
}

#[derive(Debug, Serialize)]
pub struct Assembled {
    pub payload_count: usize,
    pub checksum_manifest: PathBuf,
}

fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries =
        fs::read_dir(directory).with_context(|| format!("listing '{}'", directory.display()))?;
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
}

pub fn ensure_empty_or_create(directory: &Path, label: &str) -> Result<()> {
    if directory.exists() {
        let mut entries = fs::read_dir(directory)
            .with_context(|| format!("listing '{}'", directory.display()))?;
        if entries.next().is_some() {
            bail!("{label} '{}' must be empty.", directory.display());
        }
        return Ok(());
    }
    fs::create_dir_all(directory).with_context(|| format!("creating '{}'", directory.display()))
}

pub fn assemble(artifact_root: &Path, output: &Path) -> Result<Assembled> {
    let artifact_root = path::absolute(artifact_root)?;
    if !artifact_root.is_dir() {
        bail!(
            "Artifact root '{}' does not exist.",
            artifact_root.display()
        );
    }
    let mut files = Vec::new();
    collect_files(&artifact_root, &mut files)?;
    files.sort();
    if files.is_empty() {
        bail!("No release artifacts were downloaded.");
    }

    let mut sources: Vec<(&str, &PathBuf)> = Vec::new();
    for name in binary_payloads() {
        let matches: Vec<&PathBuf> = files
            .iter()
            .filter(|file| file_name(file) == name)
            .collect();
        if matches.len() != 1 {
            bail!(
                "Expected exactly one artifact named '{name}', found {}.",
                matches.len()
            );
        }
        sources.push((name, matches[0]));
    }
    for (name, _) in LOGOS {
        let Some(found) = files
            .iter()
            .find(|file| file_name(file).eq_ignore_ascii_case(name))
        else {
            bail!("Expected at least one {name} artifact, found none.");
        };
        sources.push((name, found));
    }

    for target in TARGETS.iter().filter(|target| target.is_windows()) {
        let source = sources
            .iter()
            .find(|(name, _)| *name == target.artifact)
            .map(|(_, path)| *path)
            .expect("every Windows artifact was collected above");
        pe::check(source, target.architecture)?;
    }

    let output = path::absolute(output)?;
    ensure_empty_or_create(&output, "Output directory")?;
    let mut names = Vec::new();
    for (name, source) in &sources {
        fs::copy(source, output.join(name))
            .with_context(|| format!("copying '{}'", source.display()))?;
        names.push((*name).to_string());
    }
    let checksum_manifest = hash::write_checksum_manifest(&output, &names)?;
    Ok(Assembled {
        payload_count: sources.len(),
        checksum_manifest,
    })
}
