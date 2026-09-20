# Codex Discord Rich Presence

A local Rust runtime that publishes Codex activity to Discord and shows the same state in a terminal dashboard. Release v1.11.2 corrects per-event session costs and keeps enabled monetary amounts visible in Discord.

<div align="center">
<img src="assets/branding/codex-readme-hero.png" alt="Codex Discord Rich Presence" width="100%">

[![Release v1.11.2](https://img.shields.io/badge/Release-v1.11.2-171717)](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest)
[![MIT](https://img.shields.io/badge/License-MIT-171717)](LICENSE)

[Install](#install) · [What's new](#whats-new-in-v1112) · [Controls](#terminal-controls) · [Configuration](#configuration) · [Documentation](docs/index.md)
</div>

## What's new in v1.11.2

- Session costs accumulate each observed model, speed and cache-write delta. Repeated cumulative events do not add cost.
- Long-context pricing uses observed request input instead of accumulated session input.
- Discord shows enabled known amounts as currency, including partial subtotals. Snapshots retain coverage and provenance; unavailable amounts stay absent.
- The shared compositor reserves space for enabled cost fields when model labels are long.

The shared `codex-presence-core` version is 2.0.1. Its public API and configuration schema 13 remain compatible. See the [changelog](CHANGELOG.md) for previous releases and rollback.

## Install

Download your architecture from [GitHub Releases](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest). Published release assets include Windows x64/ARM64, Linux x64 and macOS Intel/Apple Silicon binaries, Windows SPDX software bills of materials, branding files and SHA-256 checksums.

For Windows x64, run:

```powershell
.\codex-discord-rich-presence-windows-x64.exe doctor
.\codex-discord-rich-presence-windows-x64.exe status
.\codex-discord-rich-presence-windows-x64.exe
```

The last command opens the foreground terminal dashboard. Keep Discord open for publication. `status` and `doctor` are one-shot diagnostics, not the dashboard.

If your launcher hides the console, open a Windows Terminal window explicitly:

```powershell
wt -w new new-tab .\codex-discord-rich-presence-windows-x64.exe
```

## What it reads and publishes

The runtime reads local Codex JSONL sessions and local metadata. It can also read supported GPT sessions from OpenCode's local store. Session metadata has priority over launcher hints when identifying Codex App, ChatGPT App, CLI or VS Code activity.

The Discord identity remains Codex-owned. For a desktop analytics UI, arbitrary OpenCode providers and OpenCode Go account limits, use [Pulse](https://github.com/xt0n1-t3ch/Pulse-Claude-Code-Analytics).

| Data | Rule |
| --- | --- |
| Model and effort | Preserve the recorded model and supported observed effort |
| Speed | Explicit session speed wins over local fallback settings |
| Context | Use observed context before local model cache or catalog defaults |
| Cost | Keep exact, partial and unavailable states separate |
| Quotas | Preserve actual provider windows, scope and reset timestamps |
| Credits | Show only reported balances or explicit unlimited status |
| Privacy | Publish only the fields enabled in the local configuration |

A formatted `$0.00` is a rounded known amount, not a substitute for missing cost. Unpublished pricing and incomplete telemetry remain unavailable or partial.

## Terminal controls

| Key | Action |
| --- | --- |
| `P` | Choose automatic detection or a manual plan label |
| `D` | Switch the desktop identity between Codex App and ChatGPT App |
| `M` | Pause or resume Discord publication |
| `Q` / `Ctrl+C` | Exit the foreground runtime |

Plan and presence settings persist in `~/.codex/discord-presence-config.json`. Pausing publication does not stop local monitoring. The terminal selects its layout from the available width; no terminal image protocol is required.

## Configuration

The runtime keeps configuration under `CODEX_HOME`, or `~/.codex` by default.

| Variable | Purpose |
| --- | --- |
| `CODEX_HOME` | Select a Codex home directory |
| `CODEX_PRESENCE_POLL_SECONDS` | Set the polling interval |
| `CODEX_PRESENCE_STALE_SECONDS` | Set the stale-session cutoff |
| `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` | Set the active-session retention window |
| `CODEX_PRESENCE_SURFACE` | Set a fallback surface: `cli`, `vscode` or `desktop` |
| `CODEX_PRESENCE_INCLUDE_WSL=1` | Opt in to WSL transcript discovery on Windows |
| `CODEX_PRESENCE_EFFICIENCY_MODE=0` | Opt out of Windows Efficiency mode before launch |

WSL scanning stays off unless requested. Surface overrides do not replace authoritative session metadata.

### Windows Efficiency mode

On supported Windows versions, the runtime requests EcoQoS and Idle process priority for itself. It does not change the machine's power plan or another application's priority. Windows controls whether Task Manager displays a leaf indicator. Unsupported systems continue without this policy.

The read-only verifier checks the actual process flags:

```powershell
.\scripts\check-windows-efficiency.ps1 -ProcessId 1234
```

Replace `1234` with the running runtime's process ID. See [Windows Efficiency mode](docs/windows-efficiency.md) for the API contract.

## Build and validate

The repository uses local verification for normal development. The manual release workflow additionally validates native platform builds and immutable publication.

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo audit --deny warnings
.\scriptsuild-release.ps1 -Architecture all
```

The Windows build writes architecture-qualified binaries and SPDX files under `releases/windows/`. Native Windows ARM64 execution is verified by the release workflow; an x64-hosted cross-build alone does not prove it.

Read the [release procedure](docs/releasing.md) before creating a tag. Releases use annotated tags, an approved exact `main` commit and verified checksums. Published tags and assets are not replaced.

## Privacy and security

The runtime does not upload transcripts or send analytics telemetry. It publishes the Discord fields you enable and performs the network requests required by its configured integration. Keep credentials and private prompts out of public issues.

See [PRIVACY.md](PRIVACY.md) and report vulnerabilities through [GitHub Security Advisories](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/security/advisories/new).

## Project map

- [Runtime contract](docs/api/codex-presence.md)
- [Local schema](docs/database/schema.md)
- [Terminal UI](docs/ui/UI_SITEMAP.md)
- [Test map](tests/index.md)
- [Model catalog](src/model_catalog.json)

## License

[MIT](LICENSE), copyright 2026 xt0n1-t3ch.
