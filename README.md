# Codex Discord Rich Presence

<p align="center">
  <img src="assets/branding/codex-readme-hero.png" alt="Codex Discord Rich Presence — soft Codex App gradient with dashboard and Discord Rich Presence preview" width="100%" />
</p>

<p align="center">
  <b>Local-first Discord Rich Presence for Codex App, Codex CLI, VS Code, and OpenCode-hosted sessions.</b><br/>
  Rust daemon · Ratatui dashboard · real Codex App branding · zero cloud telemetry.
</p>

<p align="center">
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=for-the-badge&color=111827&logo=github&logoColor=white"></a>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/xt0n1-t3ch/Codex-Discord-Rich-Presence/ci.yml?branch=main&style=for-the-badge&color=6478ff&label=ci&logo=githubactions&logoColor=white"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=for-the-badge&color=0f172a"></a>
  <img alt="Ratatui" src="https://img.shields.io/badge/tui-ratatui-7c8cff?style=for-the-badge">
  <img alt="Local first" src="https://img.shields.io/badge/privacy-local--first-111827?style=for-the-badge">
  <a href="https://xt0n1.com"><img alt="Author" src="https://img.shields.io/badge/by-xt0n1-8b5cf6?style=for-the-badge"></a>
</p>

<p align="center">
  <img src="assets/screenshots/codex-discord-rich-presence.png" alt="Discord profile card showing Codex App activity, GPT-5.5 model, cost, token usage, context usage, and quota windows" width="460" />
</p>

## Why It Feels Better

<p>
  <img alt="Identity" src="https://img.shields.io/badge/identity-sticky%20Codex%20App-6478ff?style=flat-square">
  <img alt="Cost" src="https://img.shields.io/badge/cost-GPT--5%20aware-7c8cff?style=flat-square">
  <img alt="Cache" src="https://img.shields.io/badge/cache-savings%20tracked-8b5cf6?style=flat-square">
  <img alt="Terminal" src="https://img.shields.io/badge/terminal-Ratatui-111827?style=flat-square">
</p>

<table>
  <tr>
    <td><b>Codex App identity</b><br/>Sticky desktop branding keeps <code>Codex App</code> and <code>codex-app</code> visible even after the active session ages into idle.</td>
    <td><b>Cost + context truth</b><br/>One model catalog owns GPT-5.5/GPT-5.4 pricing, Fast multipliers, 400K OAuth display caps, and API-only long-context metadata.</td>
  </tr>
  <tr>
    <td><b>Cache-aware presence</b><br/>Input, cached input, output, cache hit ratio, cached-input savings, and context use resolve into one snapshot before Discord or UI rendering.</td>
    <td><b>Beautiful terminal</b><br/>Ratatui widgets render responsive Codex dark layouts with gauges, sparklines, quota cards, recent sessions, and tick-driven motion.</td>
  </tr>
</table>

Discord state example:

```text
⚡ GPT-5.5 | Pro ($200/month) • $7.13 • 31.5M tok • Ctx 19% used • 5h 100% • 7d 100%
```

## Model + Context Contract

<p>
  <img alt="OAuth" src="https://img.shields.io/badge/OAuth-400K-6478ff?style=flat-square">
  <img alt="API" src="https://img.shields.io/badge/API-1.05M%20metadata-7c8cff?style=flat-square">
  <img alt="Output" src="https://img.shields.io/badge/output-128K-8b5cf6?style=flat-square">
  <img alt="Fast" src="https://img.shields.io/badge/Fast-2.5x%20%2F%202x-111827?style=flat-square">
</p>

| Runtime lane | Context | Notes |
|:---|---:|:---|
| Codex / ChatGPT OAuth | 400K | Default visible cap because most users run Codex through OAuth |
| OpenAI API metadata | 1,050,000 | Tracked separately for GPT-5.4/GPT-5.5 API-only long-context capability |
| API input threshold | 272K | Long-context threshold before reserving the 128K output budget |
| API max output | 128K | Displayed as metadata, not the OAuth runtime cap |
| GPT-5.5 Fast | 2.5x | Applied to Fast service-tier cost display |
| GPT-5.4 Fast | 2x | Applied to Fast service-tier cost display |

## Identity

<p>
  <img alt="Codex App" src="https://img.shields.io/badge/Codex%20App-codex--app-6478ff?style=flat-square">
  <img alt="CLI" src="https://img.shields.io/badge/CLI-codex--logo-7c8cff?style=flat-square">
  <img alt="VS Code" src="https://img.shields.io/badge/VS%20Code-codex--logo-8b5cf6?style=flat-square">
  <img alt="Idle" src="https://img.shields.io/badge/idle-Idling...-111827?style=flat-square">
</p>

Only Codex identities publish.

| Surface | Discord app | Client ID | Large asset |
|:---|:---|:---|:---|
| Codex CLI | `Codex` | `1470480085453770854` | `codex-logo` |
| Codex VS Code | `Codex` | `1470480085453770854` | `codex-logo` |
| Codex App | `Codex App` | `1478395304624652345` | `codex-app` |
| OpenCode host | `Codex App` | `1478395304624652345` | `codex-app` |

When the active session ages into idle, the runtime keeps the last detected surface. If the last app was Codex App, Discord continues to show `Codex App` and `codex-app` instead of falling back to the generic CLI/VS Code identity.

## Install

<p>
  <img alt="Windows" src="https://img.shields.io/badge/Windows-ready-6478ff?style=flat-square">
  <img alt="macOS" src="https://img.shields.io/badge/macOS-ready-7c8cff?style=flat-square">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-ready-8b5cf6?style=flat-square">
  <img alt="Local" src="https://img.shields.io/badge/local--first-no%20telemetry-111827?style=flat-square">
</p>

Download Windows, Linux, or macOS binaries from [GitHub Releases](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest).

```pwsh
codex-discord-presence status
codex-discord-presence doctor
codex-discord-presence
```

Windows local artifact:

```pwsh
.\releases\windows\codex-discord-rich-presence.exe
```

## Build

<p>
  <img alt="fmt" src="https://img.shields.io/badge/fmt-rustfmt-6478ff?style=flat-square">
  <img alt="clippy" src="https://img.shields.io/badge/clippy-D%20warnings-7c8cff?style=flat-square">
  <img alt="tests" src="https://img.shields.io/badge/tests-workspace-8b5cf6?style=flat-square">
  <img alt="release" src="https://img.shields.io/badge/release-optimized-111827?style=flat-square">
</p>

Prerequisite: Rust stable. Windows builds require Visual Studio Build Tools with the C++ toolchain so `link.exe` is available.

```pwsh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

Windows package:

```pwsh
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/build-release.ps1
```

## Config

<p>
  <img alt="Config" src="https://img.shields.io/badge/config-local%20JSON-6478ff?style=flat-square">
  <img alt="Env" src="https://img.shields.io/badge/env-overrides-7c8cff?style=flat-square">
  <img alt="Privacy" src="https://img.shields.io/badge/privacy-controls-8b5cf6?style=flat-square">
  <img alt="OpenCode" src="https://img.shields.io/badge/OpenCode-supported-111827?style=flat-square">
</p>

Config lives at `~/.codex/discord-presence-config.json`.

| Variable | Purpose |
|:---|:---|
| `CODEX_HOME` | Alternate Codex home |
| `CODEX_PRESENCE_POLL_SECONDS` | Poll interval |
| `CODEX_PRESENCE_STALE_SECONDS` | Session stale cutoff |
| `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` | Active-session stickiness window |
| `CODEX_DISCORD_CLIENT_ID` | Override CLI / VS Code Discord app |
| `CODEX_DISCORD_DESKTOP_CLIENT_ID` | Override Codex App Discord app |

OpenCode data is read from `~/.local/share/opencode/opencode*.db`, including channel-specific databases such as `opencode-prod.db`.

## Project Map

<p>
  <img alt="Runtime" src="https://img.shields.io/badge/runtime-daemon-6478ff?style=flat-square">
  <img alt="Pricing" src="https://img.shields.io/badge/pricing-single%20owner-7c8cff?style=flat-square">
  <img alt="Metrics" src="https://img.shields.io/badge/metrics-cache%20snapshot-8b5cf6?style=flat-square">
  <img alt="Tests" src="https://img.shields.io/badge/tests-regressions-111827?style=flat-square">
</p>

| Path | Purpose |
|:---|:---|
| `src/app.rs` | Daemon loop, process/surface hints, Discord update cadence |
| `src/cost.rs` | Single owner for model pricing, context metadata, Fast multipliers, cache savings |
| `src/discord.rs` | Discord IPC payload, asset policy, sticky surface branding |
| `src/metrics.rs` | Unified usage/cost/cache snapshot and local reports |
| `src/session.rs` + `src/session/*` | Codex JSONL collection, parser, activity, context windows |
| `src/ui.rs` | Ratatui terminal dashboard and layout contracts |
| `assets/branding/` | Real Codex/OpenAI assets used in docs and terminal config |
| `docs/` | Runtime, UI, and local schema contracts |
| `tests/` | Integration map and module regressions |

## Docs

<p>
  <img alt="API" src="https://img.shields.io/badge/API-contract-6478ff?style=flat-square">
  <img alt="Schema" src="https://img.shields.io/badge/schema-local-7c8cff?style=flat-square">
  <img alt="UI" src="https://img.shields.io/badge/UI-sitemap-8b5cf6?style=flat-square">
  <img alt="Tests" src="https://img.shields.io/badge/tests-index-111827?style=flat-square">
</p>

- [Runtime API contract](docs/api/codex-presence.md)
- [Local schema map](docs/database/schema.md)
- [Terminal UI sitemap](docs/ui/UI_SITEMAP.md)
- [Test suite map](tests/index.md)
