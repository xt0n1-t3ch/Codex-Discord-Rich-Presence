# Codex Discord Presence

<p align="center">
  <img src="assets/branding/social-card.svg" alt="Codex Discord Presence social card" width="960" />
</p>

<p align="center">
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml"><img src="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases"><img src="https://img.shields.io/github/v/release/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat" alt="License"></a>
  <img src="https://img.shields.io/badge/Rust-2024-black?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/Discord-Rich%20Presence-5865F2?logo=discord" alt="Discord Rich Presence">
</p>

<p align="center"><strong>Real-time Discord Rich Presence for Codex CLI sessions.</strong></p>

## Overview

`codex-discord-presence` reads local Codex session JSONL files from discovered session roots (for example `~/.codex/sessions`, Windows `%USERPROFILE%\\.codex\\sessions`, and WSL homes when available), detects live activity (`Thinking`, `Reading`, `Editing`, `Running`, `Waiting for input`), renders an adaptive terminal dashboard, and updates Discord Rich Presence with low overhead.

## Core Capabilities

- Stable multi-session detection with anti-false-idle behavior.
- Discord Rich Presence keepalive in idle mode (persistent idle card + heartbeat + reconnect backoff).
- Always-visible `Recent Sessions` section with adaptive compact fallback.
- Action-first Discord details/state with deterministic truncation and compact token/cost telemetry.
- Full metrics tracking with per-model aggregation and persisted reports (`JSON` + `Markdown`).
- File activity labels sanitized to basename (`Editing test.ts` instead of full path).
- Semantic remaining-limit bars (`5h`, `7d`) with color thresholds.
- Incremental parse cache and render/publish dedupe for low CPU and memory use.

## Install

### Build from source

```bash
cargo build --release
```

Local build output (Cargo target directory):

- Windows: `releases/.cargo-target/release/codex-discord-presence.exe`
- Linux/macOS: `releases/.cargo-target/release/codex-discord-presence`

Windows convenience build (writes only under `releases/`):

```powershell
./build-dist.ps1
```

Direct executable paths after `build-dist.ps1`:

- `releases/windows/codex-discord-presence.exe`
- `releases/windows/x64/executables/codex-discord-presence.exe`

Release artifact structure:

- Windows: `releases/windows/x64/executables/codex-discord-presence.exe`
- Windows archives/checksums: `releases/windows/x64/archives/*`
- Linux: `releases/linux/distros/x64/executables/codex-discord-presence`
- Linux archives/checksums: `releases/linux/distros/x64/archives/*`
- macOS x64: `releases/macos/x64/executables/codex-discord-presence`
- macOS arm64: `releases/macos/arm64/executables/codex-discord-presence`

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

- `schema_version`: `4`
- `discord_client_id`: `1470480085453770854`
- `display.large_image_key`: `codex-logo`
- `display.small_image_key`: `openai`
- `privacy.show_cost`: `true`
- `poll_interval_seconds`: `2`
- `active_sticky_window_seconds`: `3600` (runtime env)
- `pricing.aliases.gpt-5.3-codex`: `gpt-5.2-codex`

Pricing config supports:

- `pricing.aliases`: map model ids to another model's catalog pricing.
- `pricing.overrides`: explicit per-model pricing override in USD per million tokens.

Metrics artifacts:

- `~/.codex/discord-presence-metrics.json`
- `~/.codex/discord-presence-metrics.md`

Environment overrides:

- `CODEX_DISCORD_CLIENT_ID`
- `CODEX_PRESENCE_STALE_SECONDS`
- `CODEX_PRESENCE_POLL_SECONDS`
- `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`
- `CODEX_HOME`

## Discord Asset Setup

1. Open Discord desktop app.
2. In Discord Developer Portal, configure image assets for the same `discord_client_id`:
   - `codex-logo` (large image)
   - `openai` (small image fallback)
3. Optional: add per-activity small image keys in config (`display.activity_small_image_keys`).
4. `display.*_image_key` can be an uploaded asset key or an `https://` image URL.
5. If a configured key does not exist in the app asset catalog, the app now falls back to a safe icon payload (prevents `?` placeholder on Discord mobile).

## Documentation

- API: `docs/api/codex-presence.md`
- UI: `docs/ui/UI_SITEMAP.md`
- Config schema: `docs/database/schema.md`

## Credits

<p align="center">
  <img src="assets/branding/credits-ribbon.svg" alt="Project credits" width="900" />
</p>

## OpenAI Brand Notice

- OpenAI marks and logos are trademarks of OpenAI.
- Follow official guidelines when distributing or configuring assets:
  - https://openai.com/brand/

## Security and Privacy

- Reads local Codex session files only.
- No external telemetry pipeline is implemented.
- See `PRIVACY.md` and `SECURITY.md`.

## License

MIT (`LICENSE`)
