# Codex Discord Rich Presence

<div align="center">

<picture>
  <img src="assets/branding/codex-readme-hero.png" alt="Codex Discord Rich Presence hero with Codex App-inspired gradient and Discord Rich Presence preview" width="100%">
</picture>

### Local-first activity for Codex App, CLI, VS Code, and OpenCode.

One Rust runtime for **identity**, **model**, **cost**, **cache**, **context**, and **quota visibility** — with no cloud telemetry.

<p>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest"><img src="assets/readme/badges/release.png" alt="Release v1.7.1" height="47"></a>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml"><img src="assets/readme/badges/ci-ready.png" alt="CI ready" height="47"></a>
  <a href="https://openai.com/codex/"><img src="assets/readme/badges/openai-codex.png" alt="OpenAI Codex" height="47"></a>
  <a href="https://discord.com/developers/docs/rich-presence/overview"><img src="assets/readme/badges/discord-rpc.png" alt="Discord RPC" height="47"></a>
  <br>
  <a href="https://www.rust-lang.org/"><img src="assets/readme/badges/rust-daemon.png" alt="Rust daemon" height="47"></a>
  <a href="https://ratatui.rs/"><img src="assets/readme/badges/ratatui-ui.png" alt="Ratatui UI" height="47"></a>
  <img src="assets/readme/badges/local-first.png" alt="Local-first privacy" height="47">
  <img src="assets/readme/badges/platforms.png" alt="Windows macOS Linux" height="47">
</p>

<sub>
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest">Release</a> ·
  <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/actions/workflows/ci.yml">CI</a> ·
  <a href="https://openai.com/codex/">OpenAI Codex</a> ·
  <a href="https://discord.com/developers/docs/rich-presence/overview">Discord RPC</a> ·
  <a href="https://www.rust-lang.org/">Rust</a> ·
  <a href="https://ratatui.rs/">Ratatui</a>
</sub>

<a href="#install"><b>Install</b></a>&nbsp; · &nbsp;<a href="#whats-new"><b>What's New</b></a>&nbsp; · &nbsp;<a href="#about"><b>About</b></a>&nbsp; · &nbsp;<a href="#screenshots"><b>Screenshots</b></a>&nbsp; · &nbsp;<a href="#features"><b>Features</b></a>&nbsp; · &nbsp;<a href="#usage"><b>Usage</b></a>&nbsp; · &nbsp;<a href="docs/"><b>Docs</b></a>

</div>

---

<h2 id="whats-new"><img src="assets/readme/icons/sparkles.png" alt="" width="28" align="center"> &nbsp;What's New in v1.7.1</h2>

- **Codex App stays Codex App** — idle no longer falls back to the generic `Codex CLI / VS Code Extension` card after a tiny activity gap. If your last real surface was Codex App, Discord keeps the Codex App identity and shows `Idling...`.
- **Windows WSL is quiet by default** — the runtime no longer invokes `wsl.exe` unless you explicitly opt in with `CODEX_PRESENCE_INCLUDE_WSL=1` or `CC_PRESENCE_INCLUDE_WSL=1`.
- **Pro tiers are explicit** — the terminal plan picker now separates `Pro 5x ($100/month)` from `Pro 20x ($200/month)`, while old `pro` configs migrate to Pro 20x.
- **Minimal Ratatui dashboard** — the foreground terminal now opens with a large centered Codex wordmark and a black/white UI treatment that matches Codex/OpenAI better than the old neon panel set.
- **Brand-true README badges** — OpenAI, Discord, Rust, Windows, macOS, and Linux badges use real logos and brand colors instead of generated pastel pills.

**[Download the latest release](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest)** &nbsp;·&nbsp; **[Full changelog](CHANGELOG.md)**

<h2 id="about"><img src="assets/readme/icons/info.png" alt="" width="28" align="center"> &nbsp;About</h2>

Codex already knows what you are doing: which session is active, which model is working, how much context is used, how many tokens came from cache, and whether the session is still alive. Discord sees none of that by default.

**Codex Discord Rich Presence bridges that gap locally.** It reads Codex and OpenCode session data from your machine, normalizes it into one usage snapshot, and renders two outputs from the same truth:

- a Discord Rich Presence card with the right Codex identity and clean activity text;
- a Ratatui terminal dashboard for live model, plan, cost, cache, context, and quota visibility.

No hosted service. No account sync. No transcript upload. Just a Rust daemon that watches local files and publishes the presence fields you choose.

<h2 id="screenshots"><img src="assets/readme/icons/image.png" alt="" width="28" align="center"> &nbsp;Screenshots</h2>

<div align="center">

<img src="assets/screenshots/codex-discord-rich-presence.png" alt="Discord card showing Codex App activity with GPT-5.5 model, cost, tokens, context usage, and quota windows" width="520">

<sub><b>Discord Rich Presence</b> — Codex App branding · GPT model · plan · tokens · cost · context · quota windows.</sub>



</div>

<h2 id="install"><img src="assets/readme/icons/download.png" alt="" width="28" align="center"> &nbsp;Install</h2>

Download a Windows, macOS, or Linux binary from [GitHub Releases](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/releases/latest).

```powershell
codex-discord-presence status
codex-discord-presence doctor
codex-discord-presence
```

Windows local artifact:

```powershell
.\releases\windows\codex-discord-rich-presence.exe
```

### Build from source

```powershell
git clone https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence.git
cd Codex-Discord-Rich-Presence
cargo build --release
```

<h2 id="features"><img src="assets/readme/icons/layers.png" alt="" width="28" align="center"> &nbsp;Features</h2>

<div align="center">

<img src="assets/branding/codex-feature-strip.png" alt="Codex presence capability strip: Discord, privacy, Rust runtime, Codex, terminal dashboard, context, cache, and layout" width="100%">

<sub><b>One local runtime</b> — Discord status, privacy controls, Rust daemon, terminal dashboard, cache, context, and session layout from one snapshot.</sub>

</div>

### Discord Rich Presence

| | |
| :--- | :--- |
| **Sticky Codex identity** | Codex App, CLI, VS Code, and OpenCode-hosted sessions publish through Codex-only Discord apps and keep the last real surface while idle. |
| **Readable activity** | Thinking, reading, editing, running, waiting, and idle states stay short enough for Discord while preserving the useful target when configured. |
| **Model + plan line** | GPT-5.4/GPT-5.5 labels, reasoning effort, Fast markers, and `Pro 5x` / `Pro 20x` display labels resolve from shared contracts. |
| **Cost + cache truth** | Input, cached input, output, cache hit ratio, cached-input savings, and total cost are computed before Discord rendering, not recomputed inside the payload formatter. |
| **Context and quota windows** | OAuth Codex context shows the practical 400K cap; API-only GPT-5.4/GPT-5.5 long-context metadata stays visible separately. |

### Terminal dashboard

| | |
| :--- | :--- |
| **Codex-first header** | Large centered wordmark, local-first subtitle, spinner, mode, Discord state, and poll cadence. |
| **Responsive layouts** | Full, compact, and minimal views keep terminal output readable across Windows Terminal, macOS Terminal, Linux terminals, and small panes. |
| **Plan picker** | Press `P` to choose Auto Detect, Free, Go, Plus, Pro 5x, Pro 20x, Business, or Enterprise. |
| **Usage snapshot** | Cost, cache hit ratio, savings, uptime, spend trend, limits, and recent sessions share the same runtime snapshot as Discord. |
| **No forced image protocol** | The README and Discord use the real Codex App art; the terminal uses text-first rendering so it stays portable. |

### Local diagnostics

| Command | Purpose |
| :--- | :--- |
| `codex-discord-presence status` | Print current detection state, active sessions, surface, model, plan, context, and session roots. |
| `codex-discord-presence doctor` | Check Discord IPC, config, assets, session paths, and runtime assumptions. |
| `codex-discord-presence` | Start the foreground Ratatui dashboard and Discord broadcaster. |

<h2 id="what-makes-it-cool"><img src="assets/readme/icons/brain.png" alt="" width="28" align="center"> &nbsp;What makes it cool</h2>

| Capability | This runtime | Generic presence scripts |
| :--- | :---: | :---: |
| Codex App identity remains sticky while idle | ✓ | — |
| GPT-5.4/GPT-5.5 Fast multipliers | ✓ | — |
| 400K OAuth cap plus API long-context metadata | ✓ | — |
| Cache hit ratio and cached-input savings | ✓ | — |
| Ratatui live dashboard | ✓ | — |
| Local-only session reading | ✓ | varies |
| Cross-platform Rust daemon | ✓ | varies |

<h2 id="model-context-and-cost-tracking"><img src="assets/readme/icons/gauge.png" alt="" width="28" align="center"> &nbsp;Model, context, and cost tracking</h2>

| Runtime lane | Value | Behavior |
|:---|---:|:---|
| Codex / ChatGPT OAuth visible context | 400K | Default display cap because most users run Codex through OAuth. |
| OpenAI API long-context metadata | 1,050,000 | Tracked separately for GPT-5.4/GPT-5.5 API-only long-context capability. |
| API input threshold | 272K | Long-context threshold before reserving the 128K output budget. |
| API max output | 128K | Displayed as metadata, not as the OAuth runtime cap. |
| GPT-5.5 Fast multiplier | 2.5x | Applied to Fast service-tier cost display. |
| GPT-5.4 Fast multiplier | 2x | Applied to Fast service-tier cost display. |

Example Discord state line:

```text
⚡ GPT-5.5 | Pro 20x ($200/month) · $16.58 · 16.7M tok · Ctx 79% used · 5h 53% · 7d 11%
```

<h2 id="usage"><img src="assets/readme/icons/play.png" alt="" width="28" align="center"> &nbsp;Usage</h2>

**First launch** → start `codex-discord-presence` → keep using Codex. The daemon scans local session roots and publishes the current activity to Discord.

**Change plan display** → press `P` in the terminal → choose Auto Detect or a manual tier. `Pro 5x ($100/month)` and `Pro 20x ($200/month)` are separate options.

**Hide sensitive fields** → edit `~/.codex/discord-presence-config.json` and toggle privacy fields such as project, branch, activity, tokens, cost, and limits.

**Use WSL sessions on Windows** → opt in explicitly before launch:

```powershell
$env:CODEX_PRESENCE_INCLUDE_WSL = "1"
```

<h2 id="configuration"><img src="assets/readme/icons/sliders.png" alt="" width="28" align="center"> &nbsp;Configuration</h2>

Config lives at `~/.codex/discord-presence-config.json`.

| Variable | Purpose |
|:---|:---|
| `CODEX_HOME` | Use a custom Codex home directory. |
| `CODEX_PRESENCE_POLL_SECONDS` | Override daemon poll interval. |
| `CODEX_PRESENCE_STALE_SECONDS` | Override session stale cutoff. |
| `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` | Override active-session stickiness window. |
| `CODEX_DISCORD_CLIENT_ID` | Override the Codex CLI / VS Code Discord app ID. |
| `CODEX_DISCORD_DESKTOP_CLIENT_ID` | Override the Codex App Discord app ID. |
| `CODEX_PRESENCE_INCLUDE_WSL=1` | Opt in to scanning WSL Codex session roots on Windows. Off by default. |
| `CC_PRESENCE_INCLUDE_WSL=1` | Compatibility alias for the same WSL opt-in. |

<h2 id="project-map"><img src="assets/readme/icons/folder.png" alt="" width="28" align="center"> &nbsp;Project map</h2>

| Path | Purpose |
|:---|:---|
| `src/app.rs` | Daemon loop, process and surface hints, Discord update cadence. |
| `src/config.rs` | Runtime configuration, session roots, identity defaults, migration, and WSL opt-in policy. |
| `src/cost.rs` | Model pricing, context metadata, Fast multipliers, and cache savings. |
| `src/discord.rs` | Discord IPC payloads, asset policy, and sticky surface branding. |
| `src/metrics.rs` | Usage, cost, cache, and context metrics. |
| `src/session.rs` + `src/session/*` | Codex JSONL collection, parsing, activity, and context-window state. |
| `src/ui.rs` | Ratatui terminal dashboard and layout contracts. |
| `assets/branding/` | Codex App visual assets, README art, and badge policy. |
| `docs/` | Runtime, UI, and local schema contracts. |
| `tests/` | Integration map and regression coverage. |

<h2 id="docs"><img src="assets/readme/icons/info.png" alt="" width="28" align="center"> &nbsp;Documentation</h2>

- [Runtime API contract](docs/api/codex-presence.md)
- [Local schema map](docs/database/schema.md)
- [Terminal UI sitemap](docs/ui/UI_SITEMAP.md)
- [Test suite map](tests/index.md)

<h2 id="roadmap"><img src="assets/readme/icons/roadmap.png" alt="" width="28" align="center"> &nbsp;Roadmap</h2>

- **Signed release installers** — publish first-class Windows/macOS/Linux packages with checksums and a cleaner install path.
- **Discord field presets** — expose Minimal, Standard, Full, and privacy-first templates directly in the standalone runtime.
- **Terminal screenshots in CI** — render Ratatui buffers as deterministic preview assets for README and release notes.
- **Pulse sync gate** — keep the standalone Codex core and Pulse mirror in lockstep with a contract diff check.

<h2 id="contributing"><img src="assets/readme/icons/code.png" alt="" width="28" align="center"> &nbsp;Contributing</h2>

PRs are welcome. Please keep changes local-first, tested, and focused. Use the repo-native validators before opening a PR:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release
```

<h2 id="security"><img src="assets/readme/icons/shield.png" alt="" width="28" align="center"> &nbsp;Security</h2>

This runtime reads local Codex/OpenCode session files and publishes configured Discord Rich Presence fields. Do not include secrets, transcripts, tokens, or private prompts in public issues. Use [GitHub Security Advisories](https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence/security/advisories/new) for private reports.

<h2 id="privacy"><img src="assets/readme/icons/lock.png" alt="" width="28" align="center"> &nbsp;Privacy</h2>

Codex Discord Rich Presence is local-first:

- reads local Codex and OpenCode session files;
- publishes only Discord Rich Presence fields you configure;
- does not run a telemetry server;
- does not sync transcripts to a cloud dashboard;
- keeps WSL scanning disabled unless you opt in.

See [PRIVACY.md](PRIVACY.md) for the short policy.

<h2 id="license"><img src="assets/readme/icons/shield.png" alt="" width="28" align="center"> &nbsp;License</h2>

[MIT](LICENSE) © 2026 xt0n1-t3ch.

---

<div align="center">
<sub>Built with Rust, Ratatui, Discord Rich Presence, and Codex. &nbsp; · &nbsp; <a href="https://github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence">github.com/xt0n1-t3ch/Codex-Discord-Rich-Presence</a></sub>
</div>
