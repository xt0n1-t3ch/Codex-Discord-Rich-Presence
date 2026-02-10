# Codex Discord Presence

<p align="center">
  <img src="assets/branding/social-card.svg" alt="Codex Discord Presence" width="900" />
</p>

<p align="center">
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml"><img src="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases"><img src="https://img.shields.io/github/v/release/xt0n1-t3ch/Codex-Discord-Rich-Presence?display_name=tag" alt="Latest Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/xt0n1-t3ch/Codex-Discord-Rich-Presence" alt="License"></a>
  <img src="https://img.shields.io/badge/Rust-2024-orange" alt="Rust 2024">
  <img src="https://img.shields.io/badge/Discord-Rich%20Presence-5865F2" alt="Discord Rich Presence">
</p>

Human-friendly, real-time Discord Rich Presence for Codex CLI sessions.

`codex-discord-presence` reads local Codex JSONL sessions (`~/.codex/sessions`), detects what the model is doing (`Thinking`, `Reading`, `Editing`, `Waiting for input`), renders a clean terminal dashboard, and updates Discord in near real-time.

## Why this project

- Clear status language for humans (no cryptic telemetry abbreviations).
- Stable activity detection with false-idle protection.
- Low overhead runtime with incremental parsing and render dedupe.
- Single binary per platform.

## Key Features

- Adaptive TUI layouts: `Full`, `Compact`, `Minimal`.
- Fixed footer credit with responsive formatting.
- Semantic limits bars:
  - green `>=60%`
  - yellow `>=30%`
  - red `<30%`
- Action-first Discord card:
  - `details`: `<activity> • <project>`
  - `state`: `Model ... | Last response ... | Session total ... | 5h left ... | 7d left ...`
- Balanced real-time defaults:
  - polling every `2s`
  - incremental JSONL parsing cache
  - frame diff redraws
  - deduped Discord IPC updates

## Commands

```bash
codex-discord-presence
codex-discord-presence codex [args...]
codex-discord-presence status
codex-discord-presence doctor
```

## Quick Start

1. Build:

```bash
cargo build --release
```

2. Run dashboard mode:

```bash
codex-discord-presence
```

3. Optional wrapper mode:

```bash
codex-discord-presence codex
```

## Discord Setup

- Keep Discord desktop app open.
- Configure app assets in Discord Developer Portal:
  - large image key: `codex-logo`
  - small image key: `openai`

Default IDs in config:

- `discord_client_id`: `1470480085453770854`
- `discord_public_key`: `29e563eeb755ae71d940c1b11d49dd3282a8886cd8b8cab829b2a14fcedad247`

## Configuration

Config path:

- `~/.codex/discord-presence-config.json`

Schema:

- `3`

Environment overrides:

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_HOME`

## Release Artifacts

Tagging `vX.Y.Z` triggers GitHub Actions release builds.

Output folders:

- `dist/linux/x64`
- `dist/macos/x64`
- `dist/macos/arm64`
- `dist/windows/x64`

Each includes compressed binaries and SHA256 checksums.

## CI/CD

- CI: fmt + clippy + tests + release build matrix.
- Release: semver-tag driven artifact packaging and GitHub Release publishing.
- Release note categories configured in `.github/release.yml`.

## Branding and OpenAI Mark Usage

- Project SVG assets are in `assets/branding/`.
- Official OpenAI logo files are intentionally **not redistributed** in this repository.
- Use officially sourced OpenAI brand assets in your own Discord app setup.
- OpenAI brand guidelines: https://openai.com/brand/

## Credits

- Author: **XT0N1.T3CH**
- Discord: `@XT0N1.T3CH`
- User ID: `211189703641268224`

## Security and Privacy

- Reads local Codex session files only.
- No external telemetry.
- See `PRIVACY.md` and `SECURITY.md`.

## License

MIT (`LICENSE`)
