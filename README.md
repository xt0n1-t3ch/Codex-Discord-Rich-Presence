# Codex Discord Presence

<p align="center">
  <img src="assets/branding/social-card.svg" alt="Codex Discord Presence social card" width="960" />
</p>

<p align="center">
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml"><img src="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases"><img src="https://img.shields.io/github/v/release/xt0n1-t3ch/Codex-Discord-Rich-Presence?display_name=tag&style=flat" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat" alt="License"></a>
  <img src="https://img.shields.io/badge/Rust-2024-black?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/Discord-Rich%20Presence-5865F2?logo=discord&logoColor=white" alt="Discord Rich Presence">
</p>

Real-time Discord Rich Presence for Codex CLI sessions.

## Overview

`codex-discord-presence` reads local Codex JSONL sessions (`~/.codex/sessions`), classifies live activity (`Thinking`, `Reading`, `Editing`, `Running`, `Waiting for input`), renders a terminal dashboard, and publishes updates to Discord with low overhead.

## Core Capabilities

- Live activity tracking with false-idle protection.
- Adaptive terminal layout (`Full`, `Compact`, `Minimal`) with semantic limit bars.
- Action-first Discord status lines with deterministic truncation.
- Incremental session parsing and render/update dedupe for low CPU and memory pressure.

## Install

### Build from source

```bash
cargo build --release
```

Binary output:

- Windows: `dist/windows/x64/codex-discord-presence.exe`
- Linux: `dist/linux/x64/codex-discord-presence`
- macOS x64: `dist/macos/x64/codex-discord-presence`
- macOS arm64: `dist/macos/arm64/codex-discord-presence`

### Download release binaries

- Releases: `https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases`

## Usage

```bash
codex-discord-presence
codex-discord-presence codex [args...]
codex-discord-presence status
codex-discord-presence doctor
```

## Configuration

Config file:

- `~/.codex/discord-presence-config.json`

Defaults:

- `schema_version`: `3`
- `discord_client_id`: `1470480085453770854`
- `display.large_image_key`: `codex-logo`
- `display.small_image_key`: `openai`
- `poll_interval_seconds`: `2`

Environment overrides:

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_HOME`

## Discord Asset Setup

1. Open Discord desktop app.
2. In Discord Developer Portal, configure image assets used by your application:
   - `codex-logo` for large image
   - `openai` for small image
3. Keep keys in sync with `display.large_image_key` and `display.small_image_key`.

## CI/CD and Releases

- CI runs formatting, linting, tests, and release build checks.
- Tagging `vX.Y.Z` publishes compressed artifacts and checksums.
- Release note categories are configured in `.github/release.yml`.

## Documentation

- API: `docs/api/codex-presence.md`
- UI: `docs/ui/UI_SITEMAP.md`
- Config schema: `docs/database/schema.md`

## Credits

- By **XT0N1.T3CH**
- Discord: `@XT0N1.T3CH`
- User ID: `211189703641268224`

## OpenAI Brand Notice

- OpenAI marks and logos are trademarks of OpenAI.
- Follow official brand guidelines when distributing or configuring assets:
  - https://openai.com/brand/

## Security and Privacy

- Reads local Codex session files only.
- No external telemetry pipeline is implemented.
- See `PRIVACY.md` and `SECURITY.md`.

## License

MIT (`LICENSE`)
