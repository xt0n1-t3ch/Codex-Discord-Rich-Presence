//! Cargo metadata loading for the workspace packages and their dependencies.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::exec;

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub license: Option<String>,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub packages: Vec<Package>,
    pub target_directory: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dependencies {
    Included,
    Excluded,
}

pub fn load(root: &Path, dependencies: Dependencies) -> Result<Metadata> {
    let manifest = root.join("Cargo.toml");
    if !manifest.is_file() {
        bail!("Cargo.toml is missing at '{}'.", manifest.display());
    }
    let mut command = exec::cargo();
    command
        .args(["--locked", "metadata", "--format-version", "1"])
        .current_dir(root);
    if dependencies == Dependencies::Excluded {
        command.arg("--no-deps");
    }
    command.arg("--manifest-path").arg(&manifest);
    let output = exec::captured(&mut command)?;
    if !output.status.success() {
        bail!("cargo metadata failed: {}", exec::failure_text(&output));
    }
    serde_json::from_slice(&output.stdout).context("parsing cargo metadata output")
}

impl Metadata {
    pub fn root_package(&self, root: &Path) -> Result<&Package> {
        let manifest = root.join("Cargo.toml");
        let wanted = fs::canonicalize(&manifest)
            .with_context(|| format!("resolving '{}'", manifest.display()))?;
        self.packages
            .iter()
            .find(|package| fs::canonicalize(&package.manifest_path).is_ok_and(|p| p == wanted))
            .with_context(|| {
                format!(
                    "Cargo metadata does not contain the root package from '{}'.",
                    manifest.display()
                )
            })
    }
}
