//! Shared, provider-neutral quota and credit telemetry.
//!
//! The implementation lives in `codex-presence-core` so Pulse, the TUI, and
//! the daemon cannot assign different meanings to the same Codex envelope.
pub use codex_presence_core::usage::*;
