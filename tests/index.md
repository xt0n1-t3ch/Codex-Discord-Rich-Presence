# Tests

## Commands

| Gate | Command |
|:---|:---|
| Format | `cargo fmt --check` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Locked compile | `cargo check --locked --all-targets --all-features` |
| Full suite | `cargo test --workspace` |
| Release compile | `cargo build --release` |
| Windows artifact | `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1` |

## Integration Regressions

| File | Contract |
|:---|:---|
| [integration/config_migration.rs](integration/config_migration.rs) | Non-Codex identity is rewritten to Codex IDs and assets |
| [integration/model_display.rs](integration/model_display.rs) | GPT-5.4 and GPT-5.5 standard/Fast labels stay stable |

## Unit Areas

| Module | Coverage |
|:---|:---|
| `src/app.rs` | Runtime surface fallback, OpenCode process detection, status orchestration |
| `src/config.rs` | Defaults, migration, plan presets, identity normalization, path discovery |
| `src/cost.rs` | Pricing, aliases, overrides, GPT-5 context-window defaults |
| `src/discord.rs` | Payload formatting, priority heartbeat, surface detection, assets |
| `src/opencode.rs` | SQLite discovery, GPT Fast models, token/cost/context/activity mapping |
| `src/session.rs` and `src/session/*` | JSONL parsing, activity tracking, ranking, context windows, limits |
| `src/telemetry/*` | Plan, service tier, quota-envelope selection |
| `src/ui.rs` | TUI layout and rendering contracts |
| `src/util.rs` | Formatting, truncation, atomic writes |

Rule: bugs that cross module seams get an integration regression; module-local bugs can stay beside the Rust module.
