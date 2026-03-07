# Codex Discord Rich Presence

<p align="center">
  <img src="assets/branding/social-card.svg" alt="Codex Discord Rich Presence hero banner" width="980" />
</p>

<div align="center">

[![CI](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml/badge.svg)](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml)
[![Release](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/release.yml/badge.svg)](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/release.yml)
[![Latest Release](https://img.shields.io/github/v/release/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat)](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases)
[![License](https://img.shields.io/github/license/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat)](LICENSE)
![Rust 2024](https://img.shields.io/badge/Rust-2024-black?logo=rust)

**Real-time Discord Rich Presence for Codex CLI, Codex VS Code Extension, and Codex App.**

</div>

## Table of Contents

- [About This Project](#why-this-project)
- [Highlights](#highlights)
- [Quick Start](#quick-start)
- [How Surface Routing Works](#how-surface-routing-works)
- [Command Reference](#command-reference)
- [Configuration](#configuration)
- [Build and Artifacts](#build-and-artifacts)
- [Documentation](#documentation)
- [Security and Privacy](#security-and-privacy)
- [Credits](#credits)
- [License](#license)

## About this project

Codex Discord Rich Presence is a lightweight Rust runtime that reads local Codex session telemetry and publishes clean Discord activity in near real time.

The runtime automatically detects your active surface and applies the correct branding/profile path:

- Codex CLI
- Codex VS Code Extension
- Codex App (desktop)

## Highlights

- Surface-aware profile switching with deterministic fallback logic.
- Readable activity states (`Thinking`, `Reading`, `Editing`, `Running`, `Waiting for input`).
- Low-overhead runtime behavior (adaptive polling, payload dedupe, reconnect backoff).
- Stable idle behavior for long sessions.
- Compact telemetry formatting for model, token, and cost context.

## Quick Start

### 1. Configure Discord assets

Create or update your Discord Developer applications:

- Codex CLI / VS Code app assets: `codex-logo`, `openai`
- Codex App desktop assets: `codex-app`, `openai`

### 2. Build locally

- Windows: `./scripts/build-release.ps1`
- Linux/macOS: `./scripts/build-release.sh`

Or build directly with Cargo:

```bash
cargo build --release
```

### 3. Run

```bash
codex-discord-presence
```

While the TUI is open:

- Press `P` to open the account-plan selector screen.
- Use arrow keys or `1-7` to select `Auto Detect`, `Free`, `Go`, `Plus`, `Pro`, `Business`, or `Enterprise`.
- Press `Enter` to apply and save immediately, or `P` / `Esc` to close without applying.

### 4. Validate health

```bash
codex-discord-presence status
codex-discord-presence doctor
```

## How Surface Routing Works

<p align="center">
  <img src="assets/branding/surface-map.svg" alt="Why this runtime and routing flow" width="960" />
</p>

| Active Surface | Discord App Profile | Client ID Field | Main Large Asset |
| --- | --- | --- | --- |
| Codex CLI / Codex VS Code Extension | `Codex` | `discord_client_id` | `codex-logo` |
| Codex App Desktop | `Codex App` | `discord_client_id_desktop` | `codex-app` |

Detection priority:

1. `session_meta.originator` contains `desktop`.
2. Fallback: `session_meta.source` contains `desktop`.
3. Otherwise: Codex CLI / VS Code profile.

## Command Reference

| Command | Purpose |
| --- | --- |
| `codex-discord-presence` | Starts runtime and Discord IPC loop |
| `codex-discord-presence codex [args...]` | Runs via Codex passthrough mode |
| `codex-discord-presence status` | Prints health and runtime state |
| `codex-discord-presence doctor` | Runs diagnostics checks |

## Configuration

Config file location:

- `~/.codex/discord-presence-config.json`

Essential defaults:

| Key | Value |
| --- | --- |
| `schema_version` | `8` |
| `discord_client_id` | `1470480085453770854` |
| `discord_client_id_desktop` | `1478395304624652345` |
| `display.large_image_key` | `codex-logo` |
| `display.desktop_large_image_key` | `codex-app` |
| `display.desktop_large_text` | `Codex App` |
| `display.small_image_key` | `openai` |
| `privacy.show_cost` | `true` |
| `openai_plan.mode` | `auto` |
| `openai_plan.tier` | `pro` |
| `openai_plan.show_price` | `true` |
| `poll_interval_seconds` | `2` |

Plan and model display notes:

- `openai_plan.mode = "manual"` makes `openai_plan.tier` the displayed account plan.
- `openai_plan.mode = "auto"` keeps telemetry/cache-based plan detection enabled.
- Fast mode is derived from `~/.codex/.codex-global-state.json` (`default-service-tier = "fast"`).
- Reasoning effort is derived from `turn_context.effort` with fallback to nested collaboration-mode settings.

Environment overrides:

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_DISCORD_CLIENT_ID_DESKTOP`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`

## Build and Artifacts

Published binaries are available in [GitHub Releases](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases).

Local Cargo build cache is stored under `.build/target` (gitignored). Final release binaries are copied into `releases/<platform>/`.

Expected artifact layout:

- `releases/windows/codex-discord-rich-presence.exe`
- `releases/linux/codex-discord-rich-presence`
- `releases/macos/codex-discord-rich-presence`
- `releases/macos/codex-discord-rich-presence-x64` (CI matrix artifact)
- `releases/macos/codex-discord-rich-presence-arm64` (CI matrix artifact)

Windows executable icon source:

- `assets/branding/codex-app.png`

## Documentation

- [Docs Index](docs/README.md)
- [CLI and Presence Contract](docs/api/codex-presence.md)
- [Local Data and Schema Contracts](docs/database/schema.md)
- [TUI Information Architecture](docs/ui/UI_SITEMAP.md)

## Security and Privacy

- Reads local Codex session files only.
- No external telemetry pipeline is used by this project.
- See [PRIVACY.md](PRIVACY.md) and [SECURITY.md](SECURITY.md).

## Credits

<p align="center">
  <img src="assets/branding/credits-ribbon.svg" alt="Project credits" width="980" />
</p>

## OpenAI Brand Note

OpenAI marks and logos are trademarks of OpenAI.  
Follow official brand policy: https://openai.com/brand/

## License

MIT ([LICENSE](LICENSE))
