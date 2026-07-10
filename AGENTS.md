# Codex Discord Rich Presence Agents Guide

This repo ships a Rust Discord Rich Presence runtime for Codex. Work from source and tests. Do not use GitNexus for this project unless Tony explicitly asks for it in the current conversation.

## Product Contract

| Area | Contract |
|:---|:---|
| Discord title | Must be `Codex CLI`, `Codex VS Code Extension`, or the selected desktop design: `Codex App` / `ChatGPT App` |
| Identity policy | Only Codex app names, Codex app IDs, and Codex/OpenAI assets may publish |
| Runtime data | Read local Codex session JSONL and global state only |
| Privacy | No telemetry or cloud storage; only the configured Discord Rich Presence fields leave the machine |
| Release binary | Windows artifact lives at `releases/windows/codex-discord-rich-presence.exe` |

## Repo Layout

| Path | Purpose |
|:---|:---|
| `src/` | Rust runtime, CLI, Discord IPC, TUI, session parser, telemetry, cost model |
| `tests/integration/` | Cross-module regression tests |
| `tests/index.md` | Test suite map |
| `docs/index.md` | Documentation map |
| `docs/api/` | Runtime and payload contracts |
| `docs/database/` | Local file and snapshot schemas |
| `docs/ui/` | Terminal UI behavior |
| `scripts/` | Build scripts |
| `assets/branding/` | Codex app images and social assets |
| `releases/` | Ignored local release outputs |
| `.build/target/` | Ignored Cargo target dir from `.cargo/config.toml` |

## Commands

| Task | Command |
|:---|:---|
| Test all | `cargo test --workspace` |
| Test integration regressions | `cargo test --test integration` |
| Format check | `cargo fmt --check` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Release compile | `cargo build --release` |
| Windows artifact | `pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1` |

`Taskfile.yml` mirrors the core commands for machines with Task installed.

## Coding Rules

Use the smallest correct change. Keep runtime behavior behind narrow functions and test the public seam when a bug crosses modules.

No comments in new code unless a compiler directive or generated format requires one. Prefer names and structure that explain the code.

Do not add dependencies unless the bug cannot be fixed with the existing stack.

Do not write secrets, tokens, or credentials into config, docs, tests, logs, or commits.

## Testing Rules

Every Discord identity change needs a regression test.

Every model-label change needs a display test for both standard and Fast mode when applicable.

Every pricing catalog change needs a pricing-resolution test and a context-window test when the model belongs to the GPT-5 family.

## Release Rule

Before saying a Windows exe is fixed, run the build script and verify the produced `releases/windows/codex-discord-rich-presence.exe` with `status` or `doctor`.
