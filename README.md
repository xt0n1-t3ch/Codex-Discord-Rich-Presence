# Codex Discord Rich Presence

<p align="center">
  <img src="assets/branding/social-card.svg" alt="Codex Discord Rich Presence" width="100%" />
</p>

<p align="center">
  <b>Discord Rich Presence for Codex CLI, Codex VS Code, Codex App, and OpenCode-hosted Codex sessions.</b><br/>
  Local-first Rust runtime. Reads local session state, publishes one clean Discord IPC payload, and never phones home.
</p>

<p align="center">
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat&color=0a0a0a&logo=github&logoColor=white"></a>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/xt0n1-t3ch/Codex-Discord-Rich-Presence/ci.yml?branch=main&style=flat&color=0a0a0a&label=ci&logo=githubactions&logoColor=white"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/github/license/xt0n1-t3ch/Codex-Discord-Rich-Presence?style=flat&color=0a0a0a"></a>
  <a href="https://xt0n1.com"><img alt="Author" src="https://img.shields.io/badge/by-xt0n1-0a0a0a?style=flat"></a>
</p>

## What It Shows

| Signal | Example | Source |
|:---|:---|:---|
| Activity | `Running command cargo test` | Codex JSONL or OpenCode SQLite parts |
| Surface | `Codex App` | Codex/OpenCode host detection |
| Model | `⚡ GPT-5.5` | Session model plus Fast mode |
| Plan | `Pro ($200/month)` | Codex telemetry or local override |
| Tokens | `31.5M tok` | Local token events |
| Cost | `$7.13` | Local pricing catalog |
| Context | `Ctx 19% used` | Active context-window snapshot |
| Limits | `5h 100% • 7d 100%` | Codex quota envelopes |

Discord state example:

```text
⚡ GPT-5.5 | Pro ($200/month) • $7.13 • 31.5M tok • Ctx 19% used • 5h 100% • 7d 100%
```

## Presence Priority

Codex wins the Discord activity stack. The runtime republishes the active Codex payload every two seconds, so browser presences such as PreMiD can appear only until the next Codex tick. The session start timestamp stays stable, so elapsed time still reads correctly.

## Identity

Only Codex identities publish.

| Surface | Discord app | Client ID | Large asset |
|:---|:---|:---|:---|
| Codex CLI | `Codex` | `1470480085453770854` | `codex-logo` |
| Codex VS Code | `Codex` | `1470480085453770854` | `codex-logo` |
| Codex App | `Codex App` | `1478395304624652345` | `codex-app` |
| OpenCode host | `Codex App` | `1478395304624652345` | `codex-app` |

Persisted non-Codex IDs and assets are normalized before publish.

## Install

Download Windows, Linux, or macOS binaries from [GitHub Releases](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest).

Windows local artifact:

```pwsh
.\releases\windows\codex-discord-rich-presence.exe
```

Health checks:

```pwsh
codex-discord-presence status
codex-discord-presence doctor
```

## Build

Prerequisite: Rust stable.

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

Release outputs:

| Platform | Artifact |
|:---|:---|
| Windows x64 | `releases/windows/codex-discord-rich-presence.exe` |
| Linux x64 | `releases/linux/codex-discord-rich-presence` |
| macOS x64 | `releases/macos/codex-discord-rich-presence-x64` |
| macOS arm64 | `releases/macos/codex-discord-rich-presence-arm64` |

## Config

Config lives at `~/.codex/discord-presence-config.json`.

| Variable | Purpose |
|:---|:---|
| `CODEX_HOME` | Alternate Codex home |
| `CODEX_PRESENCE_POLL_SECONDS` | Poll interval |
| `CODEX_PRESENCE_STALE_SECONDS` | Session stale cutoff |
| `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` | Active-session stickiness window |

OpenCode data is read from `~/.local/share/opencode/opencode*.db`, including channel-specific databases such as `opencode-prod.db`.

## Project Map

| Path | Purpose |
|:---|:---|
| `src/` | Runtime, Discord IPC, TUI, parsers, pricing, telemetry |
| `tests/` | Cross-module regressions and test map |
| `docs/` | Runtime, database, and UI contracts |
| `scripts/` | Release build scripts |
| `assets/branding/` | Codex/OpenAI app assets |

Docs start at [docs/index.md](docs/index.md). Tests start at [tests/index.md](tests/index.md).

## License

MIT. See [LICENSE](LICENSE).
