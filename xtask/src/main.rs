//! Binary entry point of `cargo xtask`.

use std::process::ExitCode;

use clap::Parser;
use xtask::cli::{Cli, run};

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
