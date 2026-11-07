//! Codex Discord Rich Presence: Codex session telemetry, cost model, terminal dashboard and Discord IPC.
//!
//! The `codex-discord-presence` binary is a thin command-line wrapper over these modules.

pub mod app;
pub mod cli;
pub mod config;
pub mod cost;
pub mod discord;
pub mod foreign_sqlite;
pub mod git_head;
pub mod metrics;
pub mod model;
pub mod opencode;
pub mod power;
pub mod process_guard;
pub mod session;
pub mod storage;
pub mod telemetry;
pub mod ui;
pub mod util;
