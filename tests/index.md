# Tests

## Commands

| Gate | Command | Notes |
|:---|:---|:---|
| Format | `cargo --locked fmt --check` | Runs without linker |
| Lint | `cargo --locked clippy --workspace --all-targets --all-features -- -D warnings` | Requires MSVC `link.exe` on Windows MSVC target |
| Locked compile | `cargo --locked check --workspace --all-targets --all-features` | Requires MSVC `link.exe` |
| Full suite | `cargo --locked test --workspace --all-features` | Requires MSVC `link.exe` |
| Release compile | `cargo --locked build --workspace --release --all-features` | Requires MSVC `link.exe` |
| Dependency audit | `cargo audit --deny warnings` | Uses pinned `cargo-audit 0.22.2`; rejects vulnerabilities, unmaintained, unsound, and yanked crates |
| Release metadata | `pwsh -NoProfile -File tests/release_contract.tests.ps1` | Covers SemVer, Cargo parity, changelog, prerelease, build metadata, and generated notes |
| Release approval | `pwsh -NoProfile -File tests/release_approval.tests.ps1` | Covers the local immutable-release check and exact-SHA approval contract without storing an admin token in Actions |
| Release target | `pwsh -NoProfile -File tests/release_target.tests.ps1` | Covers annotated tag ancestry, exact-SHA approval, latest protected checks, and latest-release ordering |
| Release assets | `pwsh -NoProfile -File tests/release_assets.tests.ps1` | Covers portable filenames, required payloads, and SHA-256 manifest generation |
| Release workflow | `pwsh -NoProfile -File tests/release_workflow.tests.ps1` | Covers Action pins, least privilege, job dependencies, toolchain, lockfile enforcement, and the pinned RustSec gate |
| Windows artifact | `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1` | Produces `releases/windows/codex-discord-rich-presence.exe` |

## Regression Coverage

| Area | Module/Test seam |
|:---|:---|
| Model catalog | `tests/integration/model_contract.rs` validates machine-readable facts, sources, aliases, GPT-5.6 capabilities, usable/raw context provenance, prices, credits, and cache policy |
| Pricing catalog | `src/cost.rs` plus `tests/integration/model_contract.rs` cover exact/partial/unavailable totals, unknown-model fail-closed behavior, and cache clamping |
| Fast mode | `src/session.rs` parses per-session service tier; `tests/integration/model_display.rs` validates Codex App labels and capability gating |
| Cache accounting | `src/cost.rs` cached-input savings and `src/metrics.rs` cache hit/savings aggregation |
| Surface identity | `src/session.rs`, `src/app.rs`, and `src/discord.rs` distinguish CLI, VS Code extension-host, VS Code terminal, desktop, OpenCode, sticky idle, launcher lineage, and selected desktop client ids |
| Discord branding | `src/discord.rs` verifies RPC activity-title overrides, exact surface labels, separated reasoning/speed display, and Codex App / ChatGPT App design assets |
| Terminal layout | `src/ui.rs` covers layout, monochrome wordmark, plan picker, persisted design toggle copy, footer, spinner, and reserved rows |
| Plan display tiers | `src/config.rs` + `src/telemetry/plan.rs` cover Pro 5x / Pro 20x presets, legacy `pro` migration, and manual override resolution |
| Config migration | `src/config.rs` plus `tests/integration/config_migration.rs` cover schema 11, desktop design/privacy round-trip, and identity normalization |
| Session parsing | `src/session.rs` and `src/session/*` cover JSONL, activity, latest model/effort changes, session-scoped speed, context provenance, cache bounds, and ranking |
| OpenCode | `src/opencode.rs` global workspace collection and live GPT session mapping |
| Windows WSL safety | `src/config.rs::windows_wsl_roots_are_explicit_opt_in` + `windows_wsl_probe_commands_use_hidden_launcher` keep WSL scanning off by default and hidden when explicitly enabled |
| Release integrity | `tests/release_*.tests.ps1` drives metadata, local approval, repository state, workflow, portable artifact, digest, and immutable publication contracts |
| Privacy controls | `src/config.rs`, `src/discord.rs`, `src/ui.rs`, and `src/app.rs` cover all nine fields, final-payload enforcement, persisted toggles, and Ratatui interaction |

Rule: bugs that cross module seams get an integration regression; module-local bugs can stay beside the Rust module.
