//! The full local gate: format, clippy, tests, changelog, llms, the release
//! contract of the checkout and the RustSec audit.

use std::path::Path;

use anyhow::Result;

use crate::changelog;
use crate::contract;
use crate::exec;
use crate::llms;

fn cargo_step(root: &Path, name: &str, args: &[&str]) -> Result<()> {
    println!("== {name} ==");
    let mut command = exec::cargo();
    command.args(args).current_dir(root);
    exec::inherited(&mut command, name)
}

pub fn run(root: &Path) -> Result<()> {
    cargo_step(
        root,
        "cargo fmt --check",
        &["fmt", "--all", "--", "--check"],
    )?;
    cargo_step(
        root,
        "cargo clippy -D warnings",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    cargo_step(
        root,
        "cargo nextest run",
        &["nextest", "run", "--workspace", "--all-features"],
    )?;
    println!("== changelog check ==");
    println!("{}", changelog::run_check(root)?);
    println!("== llms --check ==");
    llms::check(root)?;
    println!("== release contract (checkout) ==");
    println!("{}", contract::checkout(root)?);
    // The release preflight runs the same audit; cargo-audit 0.22.2 must be installed.
    cargo_step(
        root,
        "cargo audit --deny warnings",
        &["audit", "--deny", "warnings"],
    )?;
    println!("All local checks passed.");
    Ok(())
}
