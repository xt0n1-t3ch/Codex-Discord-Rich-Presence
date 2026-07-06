# Tests

## Commands

| Gate | Command | Notes |
|:---|:---|:---|
| Format | `cargo fmt --check` | Runs without linker |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` | Requires MSVC `link.exe` on Windows MSVC target |
| Locked compile | `cargo check --locked --all-targets --all-features` | Requires MSVC `link.exe` |
| Full suite | `cargo test --workspace` | Requires MSVC `link.exe` |
| Release compile | `cargo build --release` | Requires MSVC `link.exe` |
| Windows artifact | `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1` | Produces `releases/windows/codex-discord-rich-presence.exe` |

## Regression Coverage

| Area | Module/Test seam |
|:---|:---|
| Pricing catalog | `src/cost.rs` unit tests for GPT-5.4/GPT-5.5 pricing, aliases, OAuth context, API metadata |
| Fast mode | `src/cost.rs` multipliers and `tests/integration/model_display.rs` labels |
| Cache accounting | `src/cost.rs` cached-input savings and `src/metrics.rs` cache hit/savings aggregation |
| Discord branding | `src/discord.rs` sticky desktop surface and Codex App asset tests |
| Terminal layout | `src/ui.rs` layout, banner, footer, spinner, reserved rows |
| Config migration | `tests/integration/config_migration.rs` identity normalization |
| Session parsing | `src/session.rs` and `src/session/*` JSONL, activity, context, ranking |
| OpenCode | `src/opencode.rs` global workspace collection and live GPT session mapping |

Rule: bugs that cross module seams get an integration regression; module-local bugs can stay beside the Rust module.
