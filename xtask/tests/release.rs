//! Behavior tests of the release commands, run against the `xtask` binary.

#[path = "release/approval.rs"]
mod approval;

#[path = "release/assets.rs"]
mod assets;

#[path = "release/build_release.rs"]
mod build_release;

#[path = "release/changelog.rs"]
mod changelog;

#[path = "release/common.rs"]
mod common;

#[path = "release/contract.rs"]
mod contract;

#[path = "release/llms.rs"]
mod llms;

#[path = "release/meta.rs"]
mod meta;

#[path = "release/notes.rs"]
mod notes;

#[path = "release/pe.rs"]
mod pe;

#[path = "release/sbom.rs"]
mod sbom;

#[path = "release/semver.rs"]
mod semver;

#[path = "release/target.rs"]
mod target;

#[path = "release/workflow.rs"]
mod workflow;
