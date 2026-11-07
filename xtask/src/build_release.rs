//! Local release build: compiles each target, validates Windows binaries and writes the SBOM and checksums.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::assets::{self, LOGOS, ReleaseTarget, TARGETS};
use crate::exec;
use crate::hash;
use crate::metadata::{self, Dependencies};
use crate::pe;
use crate::sbom;

pub const PACKAGE_NAME: &str = "codex-discord-presence";

pub fn cargo_build_args(triple: &str) -> Vec<String> {
    [
        "build",
        "--locked",
        "--workspace",
        "--release",
        "--all-features",
        "--target",
        triple,
    ]
    .iter()
    .map(|argument| (*argument).to_string())
    .collect()
}

pub fn parse_host(rustc_verbose: &str) -> Option<String> {
    rustc_verbose
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(|host| host.trim().to_string())
}

pub fn host_triple() -> Result<String> {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = exec::captured(std::process::Command::new(rustc).arg("-vV"))?;
    if !output.status.success() {
        bail!("rustc -vV failed: {}", exec::failure_text(&output));
    }
    parse_host(&String::from_utf8_lossy(&output.stdout))
        .context("rustc -vV did not report a host triple")
}

pub fn resolve_targets(requested: &[String], host: Option<&str>) -> Result<Vec<ReleaseTarget>> {
    let wanted: Vec<&str> = if requested.is_empty() {
        host.into_iter().collect()
    } else {
        requested.iter().map(String::as_str).collect()
    };
    let mut targets: Vec<ReleaseTarget> = Vec::new();
    for triple in wanted {
        let Some(target) = TARGETS.iter().find(|target| target.triple == triple) else {
            let supported: Vec<&str> = TARGETS.iter().map(|target| target.triple).collect();
            bail!(
                "Unsupported release target '{triple}'. Supported targets: {}.",
                supported.join(", ")
            );
        };
        if targets.iter().all(|known| known.triple != target.triple) {
            targets.push(*target);
        }
    }
    let Some(first) = targets.first() else {
        bail!("No release target was selected.");
    };
    if targets.iter().any(|target| target.os_name != first.os_name) {
        bail!("Targets for different operating systems must be built in separate runs.");
    }
    Ok(targets)
}

fn binary_file_name(target: &ReleaseTarget) -> String {
    if target.is_windows() {
        format!("{PACKAGE_NAME}.exe")
    } else {
        PACKAGE_NAME.to_string()
    }
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn mark_executable(_path: &Path) -> Result<()> {
    Ok(())
}

pub fn run(root: &Path, requested: &[String]) -> Result<Vec<PathBuf>> {
    let host = if requested.is_empty() {
        Some(host_triple()?)
    } else {
        None
    };
    let targets = resolve_targets(requested, host.as_deref())?;
    let output = root.join("releases").join(targets[0].os_name);
    assets::ensure_empty_or_create(&output, "Release directory")?;

    let metadata = metadata::load(root, Dependencies::Excluded)?;
    let version = metadata
        .packages
        .iter()
        .find(|package| package.name == PACKAGE_NAME)
        .with_context(|| format!("Cargo metadata does not contain package '{PACKAGE_NAME}'."))?
        .version
        .clone();

    let mut names: Vec<String> = Vec::new();
    for target in &targets {
        println!("Building release binary for {}...", target.triple);
        let mut build = exec::cargo();
        build
            .args(cargo_build_args(target.triple))
            .current_dir(root);
        exec::inherited(
            &mut build,
            &format!("locked Cargo release build for {}", target.triple),
        )?;

        let binary = metadata
            .target_directory
            .join(target.triple)
            .join("release")
            .join(binary_file_name(target));
        if !binary.is_file() {
            bail!("Release binary not found at {}", binary.display());
        }
        let artifact = output.join(target.artifact);
        fs::copy(&binary, &artifact).with_context(|| format!("copying '{}'", binary.display()))?;
        mark_executable(&artifact)?;
        names.push(target.artifact.to_string());

        if let Some(sbom_name) = target.sbom.filter(|_| target.is_windows()) {
            println!(
                "{}",
                serde_json::to_string(&pe::check(&artifact, target.architecture)?)?
            );
            let sbom_path = output.join(sbom_name);
            println!(
                "{}",
                sbom::generate(&sbom::Request {
                    root,
                    artifact: &artifact,
                    output: &sbom_path,
                    package_name: PACKAGE_NAME,
                    package_version: &version,
                    architecture: target.architecture,
                })?
            );
            println!(
                "{}",
                sbom::verify(&artifact, &sbom_path, PACKAGE_NAME, &version)?
            );
            names.push(sbom_name.to_string());
        }
    }

    for (name, source) in LOGOS {
        let source = root.join(source);
        if !source.is_file() {
            bail!(
                "Required release payload '{}' is missing.",
                source.display()
            );
        }
        fs::copy(&source, output.join(name))
            .with_context(|| format!("copying '{}'", source.display()))?;
        names.push(name.to_string());
    }

    let manifest = hash::write_checksum_manifest(&output, &names)?;
    let mut ready: Vec<PathBuf> = names.iter().map(|name| output.join(name)).collect();
    ready.push(manifest);
    Ok(ready)
}
