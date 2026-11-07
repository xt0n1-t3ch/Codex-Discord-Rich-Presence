[Codex Discord Rich Presence](../README.md) / Tests

# Tests

`cargo xtask check` runs the full local gate. Run focused checks while working, then the full gate before proposing a change. Results below are **pending** until the lead records dated validation after merging the parallel lanes.

[Local gates](#local-gates) · [Runtime regressions](#runtime-regressions) · [Release and documentation](#release-and-documentation)

## Local gates

| What it proves | Command | Result |
| --- | --- | --- |
| Formatting matches rustfmt | `cargo fmt --all -- --check` | pending |
| Workspace and targets pass warning-denying lint | `cargo clippy --workspace --all-targets -- -D warnings` | pending |
| Runtime, shared core and tooling regressions pass | `cargo nextest run --workspace` | pending |
| Native optimized binary builds | `cargo build --release` | pending |
| Dependency audit has no unapproved findings | `cargo audit --deny warnings` | pending |

## Runtime regressions

| What it proves | Command | Result |
| --- | --- | --- |
| Catalog identities, aliases, efforts, Standard/Fast pricing and context precedence | `cargo nextest run --test integration model_contract` | pending |
| Model labels preserve reasoning, speed and Codex-owned identities | `cargo nextest run --test integration model_display` | pending |
| Schema 13 migration, privacy defaults and external configuration reload | `cargo nextest run --test integration config_migration` | pending |
| Per-event costs preserve model, speed, cache writes, known subtotals and long-context request boundaries without double counting | `cargo nextest run --lib costing` | pending |
| Session parsing, incremental reads, activity, ranking and scanner resource bounds | `cargo nextest run --lib session` | pending |
| Shared provider lanes, quotas, reset credits and deterministic composition stay isolated | `cargo nextest run -p codex-presence-core` | pending |
| Public Discord fields respect privacy, identity, cost coverage and line limits | `cargo nextest run --lib discord` | pending |
| Terminal layouts, plan picker, design, privacy editor and pause/resume | `cargo nextest run --lib ui` | pending |
| Child ownership, takeover and process identity checks | `cargo nextest run --lib process_guard` | pending |
| Background probes use timeouts and silent Windows launchers | `cargo nextest run --lib util` | pending |
| OpenCode sessions keep their own usage provenance | `cargo nextest run --lib opencode` | pending |
| Windows WSL scanning stays opt-in | `cargo nextest run --lib windows_wsl` | pending |
| Process-local Windows Efficiency mode option and ABI contracts | `cargo nextest run --lib power` | pending |

The integration seams live in [integration/](integration/). Module-local tests stay beside their implementation. Tests that cross module boundaries belong in integration regressions. A passing non-Windows build does not prove Windows runtime behavior.

## Release and documentation

| What it proves | Command | Result |
| --- | --- | --- |
| Release metadata, approval, target, assets, architecture, checksums and workflow contracts | `cargo nextest run -p xtask` | pending |
| Changelog uses the six standard sections, summaries and ordered version links | `cargo xtask changelog check` | pending |
| Published body is the version's changelog section without wrapper headings | `cargo xtask release-notes 1.11.3` | pending |
| Generated language-model maps match their manifest and documents | `cargo xtask llms --check` | pending |
| GitHub description and topics match repository metadata | `cargo xtask meta --check` | pending |

The [release procedure](../docs/releasing.md) owns native smoke checks, PE-machine validation, SPDX software bills of materials, checksums and immutable publication. `discord-proof` is an explicit live operation: it sends an activity and clears it again. It is not part of an offline unit-test run.
