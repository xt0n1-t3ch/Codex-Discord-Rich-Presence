# Tests

## Commands

| Gate | Command | Notes |
|:---|:---|:---|
| Format | `cargo --locked fmt --check` | Runs without linker |
| Lint | `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings` | Requires MSVC `link.exe` on Windows MSVC target |
| Locked compile | `cargo --locked check --workspace --all-targets --all-features` | Requires MSVC `link.exe` |
| Full suite | `cargo --locked test --workspace --all-features` | Requires MSVC `link.exe` |
| Release compile | `cargo --locked build --workspace --release --all-features` | Requires MSVC `link.exe` |
| Release metadata | `pwsh -NoProfile -File tests/release_contract.tests.ps1` | Covers SemVer, Cargo parity, changelog, prerelease, build metadata, and generated notes |
| Release approval | `pwsh -NoProfile -File tests/release_approval.tests.ps1` | Covers the local immutable-release check and exact-SHA approval contract without storing an admin token in Actions |
| Release target | `pwsh -NoProfile -File tests/release_target.tests.ps1` | Covers annotated tag ancestry, exact-SHA approval, latest protected checks, and latest-release ordering |
| Release assets | `pwsh -NoProfile -File tests/release_assets.tests.ps1` | Covers portable filenames, required payloads, and SHA-256 manifest generation |
| Release workflow | `pwsh -NoProfile -File tests/release_workflow.tests.ps1` | Covers Action pins, least privilege, job dependencies, toolchain, and lockfile enforcement |
| Windows artifact | `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1` | Produces `releases/windows/codex-discord-rich-presence.exe` |

## Regression Coverage

| Area | Module/Test seam |
|:---|:---|
| Pricing catalog | `src/cost.rs` unit tests for GPT-5.4/GPT-5.5 pricing, aliases, OAuth context, API metadata |
| Fast mode | `src/cost.rs` multipliers and `tests/integration/model_display.rs` labels |
| Cache accounting | `src/cost.rs` cached-input savings and `src/metrics.rs` cache hit/savings aggregation |
| Discord branding | `src/discord.rs` sticky desktop surface and Codex App asset tests |
| Terminal layout | `src/ui.rs` layout, monochrome Codex wordmark, plan picker, footer, spinner, reserved rows |
| Plan display tiers | `src/config.rs` + `src/telemetry/plan.rs` cover Pro 5x / Pro 20x presets, legacy `pro` migration, and manual override resolution |
| Config migration | `tests/integration/config_migration.rs` identity normalization |
| Session parsing | `src/session.rs` and `src/session/*` JSONL, activity, context, ranking |
| OpenCode | `src/opencode.rs` global workspace collection and live GPT session mapping |
| Windows WSL safety | `src/config.rs::windows_wsl_roots_are_explicit_opt_in` + `windows_wsl_probe_commands_use_hidden_launcher` keep WSL scanning off by default and hidden when explicitly enabled |
| Release integrity | `tests/release_*.tests.ps1` drives metadata, local approval, repository state, workflow, portable artifact, digest, and immutable publication contracts |

Rule: bugs that cross module seams get an integration regression; module-local bugs can stay beside the Rust module.
