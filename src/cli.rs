use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "codex-discord-presence",
    version,
    about = "Show live Codex CLI activity in Discord Rich Presence",
    trailing_var_arg = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run Codex as a child process while presence follows its lifecycle.
    #[command(trailing_var_arg = true)]
    Codex {
        #[arg(
            value_name = "CODEX_ARGS",
            help = "Arguments passed directly to `codex`",
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    /// Print a one-shot operational status snapshot.
    Status,
    /// Run health diagnostics for setup and runtime requirements.
    Doctor,
}
