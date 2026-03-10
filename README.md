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

**A polished Discord Rich Presence runtime for Codex CLI, Codex VS Code Extension, and Codex App.**

</div>

> Local-first by design: the runtime reads your Codex session files on disk, talks to Discord over local IPC. Nothing is stored in the cloud.

## Overview

Codex Discord Rich Presence reads local Codex session telemetry, detects the active Codex surface, renders a live terminal dashboard, and publishes a clean Discord presence with activity, model, Fast mode, reasoning effort, account plan, token usage, cost, context window, and quota visibility.

## What it shows

| Signal       | Example                   | Notes                                                                 |
| ------------ | ------------------------- | --------------------------------------------------------------------- |
| Surface      | `Codex App` / `Codex CLI` | Auto-routed from `session_meta.originator` and `session_meta.source`  |
| Activity     | `Reading session.rs`      | Derived from response items, tool calls, and commentary signals       |
| Model        | `⚡ GPT-5.4 (Extra High)` | Fast mode adds the lightning prefix; reasoning effort adds the suffix |
| Plan         | `Pro ($200/month)`        | Auto-detected from telemetry or forced manually from the TUI/config   |
| Live context | `Ctx 64% left`            | Derived from active-turn usage and model context window               |
| Limits       | `5h 100% • 7d 100%`       | Tracks global account quota freshness and remaining percentages       |
| Cost         | `$7.13`                   | Uses pricing aliases and overrides from local config                  |

## Highlights

- Surface-aware Discord profile switching for Codex CLI / VS Code and Codex App.
- Fast mode visibility driven by Codex global state, with a lightning-marked model label.
- Reasoning effort visibility from live `turn_context` telemetry, including `Extra High`.
- Full-screen plan selector in the TUI, with instant save to config.
- Stable activity interpretation for long sessions, commentary turns, waiting states, and idle transitions.
- Deterministic active-session ranking when multiple Codex sessions are open at once.
- Compact but information-dense Discord payload formatting that stays within Discord limits.
- Local build and release layout that keeps Cargo cache under `.build/target` and final artifacts under `releases/<platform>/`.

## Quick Start

### 1. Configure Discord assets

Create or update your Discord Developer applications with the image keys used by the runtime:

- Codex CLI / VS Code profile:
  - `codex-logo`
  - `openai`
- Codex App desktop profile:
  - `codex-app`
  - `openai`

### 2. Build

- Windows:

```powershell
./scripts/build-release.ps1
```

- Linux / macOS:

```bash
./scripts/build-release.sh
```

Or build directly with Cargo:

```bash
cargo build --release
```

### 3. Run

```bash
codex-discord-presence
```

### 4. Use the TUI

While the terminal UI is open:

- Press `P` to open the full-screen account plan selector.
- Use arrow keys or `1-7` to select:
  - `Auto Detect`
  - `Free`
  - `Go`
  - `Plus`
  - `Pro`
  - `Business`
  - `Enterprise`
- Press `Enter` to apply and save immediately.
- Press `P` or `Esc` to close without changing the current plan.
- Press `q` or `Ctrl+C` to quit the runtime.

### 5. Validate health

```bash
codex-discord-presence status
codex-discord-presence doctor
```

## Feature Spotlight

### Fast mode visibility

The runtime reads `~/.codex/.codex-global-state.json` and checks:

- `electron-persisted-atom-state.default-service-tier`

If the value is `fast`, the model is shown with a lightning prefix:

```text
⚡ GPT-5.4
```

### Reasoning effort visibility

Reasoning effort is resolved from the active session in this order:

1. `turn_context.payload.effort`
2. `turn_context.payload.collaboration_mode.settings.reasoning_effort`

Rendered examples:

```text
GPT-5.4 (Low)
GPT-5.4 (High)
⚡ GPT-5.4 (Extra High)
```

### Interactive plan selector

Plan display supports both automatic detection and manual override.

- `Auto Detect` uses session telemetry, in-memory cache, and persisted cache fallback.
- Manual mode writes the selected plan to `~/.codex/discord-presence-config.json`.
- The selector is designed for live correction when Codex telemetry says `Unknown`, lags behind, or you simply want Discord/TUI to reflect the plan you actually use.

### Example Discord state

A typical compact presence state can look like:

```text
⚡ GPT-5.4 (Extra High) | Pro ($200/month) • $7.13 • 31.5M tok • Ctx 64% left • 5h 100% • 7d 100%
```

## Surface Routing 

<p align="center">
  <img src="assets/branding/surface-map.svg" alt="Surface routing flow" width="960" />
</p>

| Active Surface                      | Discord App Profile | Client ID Field             | Main Large Asset |
| ----------------------------------- | ------------------- | --------------------------- | ---------------- |
| Codex CLI / Codex VS Code Extension | `Codex`             | `discord_client_id`         | `codex-logo`     |
| Codex App Desktop                   | `Codex App`         | `discord_client_id_desktop` | `codex-app`      |

Detection priority:

1. `session_meta.originator` contains `desktop`
2. fallback: `session_meta.source` contains `desktop`
3. otherwise: Codex CLI / Codex VS Code Extension profile

## Command Reference

| Command                                  | Purpose                                                                                                |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `codex-discord-presence`                 | Starts the runtime, TUI, and Discord IPC loop                                                          |
| `codex-discord-presence codex [args...]` | Runs as a Codex passthrough wrapper while keeping presence active                                      |
| `codex-discord-presence status`          | Prints health, active session state, limits source, model label, plan, Fast mode, and reasoning effort |
| `codex-discord-presence doctor`          | Runs diagnostics for session roots, config, Discord IDs, and command availability                      |

## Configuration

Config file location:

- `~/.codex/discord-presence-config.json`

Essential defaults:

| Key                               | Value                 |
| --------------------------------- | --------------------- |
| `schema_version`                  | `8`                   |
| `discord_client_id`               | `1470480085453770854` |
| `discord_client_id_desktop`       | `1478395304624652345` |
| `display.large_image_key`         | `codex-logo`          |
| `display.desktop_large_image_key` | `codex-app`           |
| `display.desktop_large_text`      | `Codex App`           |
| `display.small_image_key`         | `openai`              |
| `privacy.show_cost`               | `true`                |
| `openai_plan.mode`                | `auto`                |
| `openai_plan.tier`                | `pro`                 |
| `openai_plan.show_price`          | `true`                |
| `poll_interval_seconds`           | `2`                   |

Example:

```json
{
  "schema_version": 8,
  "discord_client_id": "1470480085453770854",
  "discord_client_id_desktop": "1478395304624652345",
  "poll_interval_seconds": 2,
  "privacy": {
    "show_cost": true
  },
  "openai_plan": {
    "mode": "auto",
    "tier": "pro",
    "show_price": true
  }
}
```

Plan and model display notes:

- `openai_plan.mode = "manual"` makes `openai_plan.tier` the displayed account plan.
- `openai_plan.mode = "auto"` keeps telemetry and cache-based plan detection enabled.
- The TUI plan selector writes the same config file used at startup.
- Fast mode is derived from `~/.codex/.codex-global-state.json`.
- Reasoning effort is derived from live `turn_context` telemetry.

Environment overrides:

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_DISCORD_CLIENT_ID_DESKTOP`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`

## Build and Artifacts

Published binaries are available in [GitHub Releases](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases).

Local Cargo build cache is stored under `.build/target` and final release binaries are copied into `releases/<platform>/`.

Expected artifact layout:

- `releases/windows/codex-discord-rich-presence.exe`
- `releases/linux/codex-discord-rich-presence`
- `releases/macos/codex-discord-rich-presence`
- `releases/macos/codex-discord-rich-presence-x64`
- `releases/macos/codex-discord-rich-presence-arm64`

Windows executable icon source:

- `assets/branding/codex-app.png`

## Documentation

- [Docs Index](docs/README.md)
- [CLI and Presence Contract](docs/api/codex-presence.md)
- [Local Data and Schema Contracts](docs/database/schema.md)
- [TUI Information Architecture](docs/ui/UI_SITEMAP.md)

## Security and Privacy

- Reads local Codex session files only.
- Uses Discord local IPC for activity publishing.
- Does not add an external telemetry backend.
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
