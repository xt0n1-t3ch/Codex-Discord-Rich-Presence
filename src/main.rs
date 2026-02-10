use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use codex_discord_presence::app::{self, AppMode};
use codex_discord_presence::cli::{Cli, Commands};
use codex_discord_presence::config::{self, PresenceConfig};
use codex_discord_presence::process_guard;
use codex_discord_presence::util::setup_tracing;

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(err) => {
            eprintln!("codex-discord-presence error: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<u8> {
    setup_tracing();
    let cli = Cli::parse();
    let config = PresenceConfig::load_or_init()?;

    match cli.command {
        Some(Commands::Status) => {
            app::print_status(&config)?;
            Ok(0)
        }
        Some(Commands::Doctor) => app::doctor(&config),
        Some(Commands::Codex { args }) => {
            let acquired = process_guard::acquire_or_takeover_single_instance()?;
            if let Some(pid) = acquired.takeover_pid {
                println!("Existing instance detected (PID {pid}); takeover completed.");
            }
            let _guard = acquired.guard;
            let runtime = config::runtime_settings();
            app::run(config, AppMode::CodexChild { args }, runtime)?;
            Ok(0)
        }
        None => {
            let acquired = process_guard::acquire_or_takeover_single_instance()?;
            if let Some(pid) = acquired.takeover_pid {
                println!("Existing instance detected (PID {pid}); takeover completed.");
            }
            let _guard = acquired.guard;
            let runtime = config::runtime_settings();
            app::run(config, AppMode::SmartForeground, runtime)?;
            Ok(0)
        }
    }
}
