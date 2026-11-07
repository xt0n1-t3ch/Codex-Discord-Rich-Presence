//! Command-line definition and dispatch for `cargo xtask`.

use std::env;
use std::path::{self, Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

use crate::approve;
use crate::assets;
use crate::build_release::{self, PACKAGE_NAME};
use crate::changelog;
use crate::check;
use crate::contract;
use crate::efficiency;
use crate::llms;
use crate::meta;
use crate::notes;
use crate::pe::{self, Architecture};
use crate::sbom;
use crate::semver::Version;
use crate::smoke;
use crate::target::{self, Api};

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    about = "Verification, documentation and release commands"
)]
pub struct Cli {
    /// Repository root (defaults to the parent folder of the xtask crate)
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run every local gate: format, clippy, tests, changelog and llms checks
    Check,
    /// Generate llms.txt and llms-full.txt from docs/llms.toml
    Llms {
        /// Verify the generated files are current instead of writing them
        #[arg(long)]
        check: bool,
    },
    /// Validate CHANGELOG.md or print one version's section
    Changelog {
        #[command(subcommand)]
        action: ChangelogAction,
    },
    /// Compare or apply the GitHub description, homepage and topics from .github/repository.toml
    Meta(MetaMode),
    /// Print or write the release notes body of a version
    ReleaseNotes {
        /// Version to describe, with or without a leading v
        version: String,
        /// Write the notes to this file instead of stdout
        #[arg(long)]
        output: Option<PathBuf>,
        /// Print only the release title
        #[arg(long)]
        title: bool,
        /// Tag the full-changelog link starts from, used only for a long section (defaults to the previous git tag)
        #[arg(long)]
        previous_tag: Option<String>,
    },
    /// Validate a release tag against Cargo, README, changelog and release contract, then print its metadata as JSON
    ReleaseContract {
        /// Release tag to validate, such as v1.11.3
        #[arg(long)]
        tag: String,
        /// Kind of Git ref being released; anything but `tag` is rejected (defaults to $GITHUB_REF_TYPE)
        #[arg(long)]
        ref_type: Option<String>,
        /// File that receives the metadata as `key=value` lines (defaults to $GITHUB_OUTPUT)
        #[arg(long)]
        github_output: Option<PathBuf>,
    },
    /// Build release binaries into `releases/<os>/` with a checksum manifest
    BuildRelease {
        /// Rust target triple to build; repeat for several (defaults to the host triple)
        #[arg(long = "target")]
        targets: Vec<String>,
    },
    /// Collect downloaded release artifacts into one folder with a checksum manifest
    ReleaseAssets {
        /// Folder holding the downloaded build artifacts
        #[arg(long)]
        artifact_root: PathBuf,
        /// Empty or new folder that receives the release payloads
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Check that an annotated tag points at the approved SHA, is on the main branch, passed its checks and beats the latest release
    ReleaseTarget {
        /// Repository as owner/name (defaults to .github/repository.toml)
        #[arg(long)]
        repository: Option<String>,
        /// Release tag, such as v1.11.3
        #[arg(long)]
        tag: String,
        /// Validated release version without the leading v
        #[arg(long)]
        version: String,
        /// Commit SHA being released (40 lowercase hex characters)
        #[arg(long)]
        sha: String,
        /// Commit SHA recorded as approved for release
        #[arg(long)]
        approved_sha: String,
        /// Whether the version is a prerelease
        #[arg(long, value_parser = ["true", "false"])]
        is_prerelease: String,
        /// Branch the release commit must be an ancestor of
        #[arg(long, default_value = "origin/main")]
        main_ref: String,
        /// Read GitHub API responses from this folder of JSON fixtures instead of calling `gh`
        #[arg(long)]
        api_fixture_dir: Option<PathBuf>,
    },
    /// Approve a commit for release after confirming GitHub immutable releases are on
    ApproveRelease {
        /// Repository as owner/name (defaults to .github/repository.toml)
        #[arg(long)]
        repository: Option<String>,
        /// Commit SHA to approve (40 lowercase hex characters)
        #[arg(long)]
        sha: String,
        /// Repository variable that stores the approved SHA
        #[arg(long, default_value = approve::DEFAULT_VARIABLE)]
        variable_name: String,
        /// Read the GitHub API response from this JSON fixture instead of calling `gh`
        #[arg(long)]
        api_fixture: Option<PathBuf>,
        /// Validate only; do not write the repository variable
        #[arg(long)]
        skip_variable_write: bool,
    },
    /// Check that a Windows executable is a PE image for the given architecture
    CheckPe {
        /// Windows executable to inspect
        #[arg(long)]
        artifact: PathBuf,
        /// CPU architecture the executable must target
        #[arg(long, value_enum)]
        architecture: Architecture,
    },
    /// Generate an SPDX 2.3 SBOM for a release executable
    Sbom {
        /// Executable the SBOM describes
        #[arg(long)]
        artifact: PathBuf,
        /// SBOM file to write
        #[arg(long)]
        output: PathBuf,
        /// Package name recorded in the SBOM
        #[arg(long, default_value = PACKAGE_NAME)]
        package_name: String,
        /// Package version recorded in the SBOM
        #[arg(long)]
        package_version: String,
        /// CPU architecture of the executable
        #[arg(long, value_enum, default_value_t = Architecture::X64)]
        architecture: Architecture,
    },
    /// Verify an SBOM against its executable, package name and version
    CheckSbom {
        /// Executable the SBOM describes
        #[arg(long)]
        artifact: PathBuf,
        /// SBOM file to verify
        #[arg(long)]
        sbom: PathBuf,
        /// Expected package name
        #[arg(long, default_value = PACKAGE_NAME)]
        package_name: String,
        /// Expected package version
        #[arg(long)]
        package_version: String,
    },
    /// Print the Windows power-throttling policy of a running process (Windows only)
    WindowsEfficiency {
        /// Process ID to inspect
        #[arg(long)]
        pid: u32,
    },
    /// Run a built executable with `--version` and `doctor` against a throwaway Codex home
    Smoke {
        /// Executable to run
        #[arg(long)]
        artifact: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ChangelogAction {
    /// Validate the structure of CHANGELOG.md
    Check,
    /// Print the changelog section of a version
    Notes {
        /// Version to print, without a leading v
        version: String,
    },
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
pub struct MetaMode {
    /// Fail when the published GitHub metadata differs from the file
    #[arg(long)]
    pub check: bool,
    /// Write the description, homepage and topics to GitHub, then check
    #[arg(long)]
    pub apply: bool,
}

fn default_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the xtask crate lives inside the repository")
        .to_path_buf()
}

fn repository_or_default(root: &Path, requested: Option<String>) -> Result<String> {
    match requested {
        Some(repository) => Ok(repository),
        None => Ok(meta::load(root)?.repository),
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn release_notes(
    root: &Path,
    version: &str,
    output: Option<&Path>,
    title: bool,
    previous_tag: Option<&str>,
) -> Result<()> {
    let version = version.strip_prefix('v').unwrap_or(version);
    version.parse::<Version>()?;
    if title {
        println!("{}", notes::title(version));
        return Ok(());
    }
    let body = notes::build(root, version, previous_tag)?;
    match output {
        Some(path) => notes::write(path, &body),
        None => {
            print!("{body}");
            Ok(())
        }
    }
}

fn release_contract(
    root: &Path,
    tag: &str,
    ref_type: Option<String>,
    github_output: Option<PathBuf>,
) -> Result<()> {
    let ref_type = ref_type.or_else(|| env::var("GITHUB_REF_TYPE").ok());
    let github_output = github_output.or_else(|| {
        env::var_os("GITHUB_OUTPUT")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    });
    let release = contract::check(root, tag, ref_type.as_deref())?;
    if let Some(path) = github_output {
        contract::append_github_output(&path, &release)?;
    }
    print_json(&release)
}

pub fn run(cli: Cli) -> Result<()> {
    let root = match cli.root {
        Some(root) => path::absolute(root)?,
        None => default_root(),
    };
    match cli.command {
        Command::Check => check::run(&root),
        Command::Llms { check } => {
            if check {
                llms::check(&root)
            } else {
                llms::write(&root)
            }
        }
        Command::Changelog { action } => match action {
            ChangelogAction::Check => {
                println!("{}", changelog::run_check(&root)?);
                Ok(())
            }
            ChangelogAction::Notes { version } => {
                println!("{}", changelog::read_section(&root, &version)?);
                Ok(())
            }
        },
        Command::Meta(mode) => {
            if mode.apply {
                meta::apply(&root)
            } else {
                meta::check(&root)
            }
        }
        Command::ReleaseNotes {
            version,
            output,
            title,
            previous_tag,
        } => release_notes(
            &root,
            &version,
            output.as_deref(),
            title,
            previous_tag.as_deref(),
        )
        .context("release notes"),
        Command::ReleaseContract {
            tag,
            ref_type,
            github_output,
        } => release_contract(&root, &tag, ref_type, github_output).context("release contract"),
        Command::BuildRelease { targets } => {
            let ready = build_release::run(&root, &targets).context("release build")?;
            println!("Ready:");
            for path in ready {
                println!(" - {}", path.display());
            }
            Ok(())
        }
        Command::ReleaseAssets {
            artifact_root,
            output_directory,
        } => print_json(
            &assets::assemble(&artifact_root, &output_directory).context("release assets")?,
        ),
        Command::ReleaseTarget {
            repository,
            tag,
            version,
            sha,
            approved_sha,
            is_prerelease,
            main_ref,
            api_fixture_dir,
        } => {
            let repository = repository_or_default(&root, repository)?;
            let api = api_fixture_dir.map_or(Api::Live, Api::Fixtures);
            let report = target::check(&target::Request {
                repository: &repository,
                tag: &tag,
                version: &version,
                sha: &sha,
                approved_sha: &approved_sha,
                is_prerelease: is_prerelease == "true",
                root: &root,
                main_ref: &main_ref,
                api: &api,
            })
            .context("release target")?;
            print_json(&report)
        }
        Command::ApproveRelease {
            repository,
            sha,
            variable_name,
            api_fixture,
            skip_variable_write,
        } => {
            let repository = repository_or_default(&root, repository)?;
            let report = approve::run(&approve::Request {
                repository: &repository,
                sha: &sha,
                variable_name: &variable_name,
                api_fixture: api_fixture.as_deref(),
                write_variable: !skip_variable_write,
            })
            .context("release approval")?;
            print_json(&report)
        }
        Command::CheckPe {
            artifact,
            architecture,
        } => print_json(&pe::check(&artifact, architecture)?),
        Command::Sbom {
            artifact,
            output,
            package_name,
            package_version,
            architecture,
        } => {
            let line = sbom::generate(&sbom::Request {
                root: &root,
                artifact: &artifact,
                output: &output,
                package_name: &package_name,
                package_version: &package_version,
                architecture,
            })?;
            println!("{line}");
            Ok(())
        }
        Command::CheckSbom {
            artifact,
            sbom,
            package_name,
            package_version,
        } => {
            println!(
                "{}",
                sbom::verify(&artifact, &sbom, &package_name, &package_version)?
            );
            Ok(())
        }
        Command::WindowsEfficiency { pid } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&efficiency::inspect(pid)?)?
            );
            Ok(())
        }
        Command::Smoke { artifact } => smoke::run(&artifact).context("smoke"),
    }
}
