# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

No unreleased changes.

## [1.5.0] - 2026-05-21

### Added

- Official pricing catalog entries for `gpt-5.5` ($5 / $0.50 / $30 per 1M) and `gpt-5.5-pro` ($30 / $3 / $180 per 1M), so Codex sessions on GPT-5.5 resolve to the correct OpenAI pricing the moment the model is selected.
- Lightning `⚡` Fast-mode prefix coverage is now confirmed for the entire GPT-5.5 family (inherited from the existing service-tier resolver — no new wiring required).
- Unit tests covering `gpt-5.5`, `gpt-5.5-pro`, case-and-trim normalization on the new keys, and context-window resolution for the GPT-5.5 family. Test suite total now sits at 114 passing.
- README highlight banner calling out GPT-5.5 + GPT-5.5 Pro support, including pricing chips, rendered identically on github.com and in local Markdown previews.

### Changed

- Pricing rows in `src/cost.rs` are now centralized as named `ModelPricing` constants (`GPT_5_5`, `GPT_5_5_PRO`, `GPT_5_4`, `GPT_5_2_FAMILY`, `GPT_5_1_FAMILY`, `GPT_5_MINI_FAMILY`, `GPT_5_NANO`, `CODEX_MINI_LATEST`); the Codex session context window is centralized as `CODEX_CONTEXT_WINDOW: u64 = 400_000`.
- README badge row upgraded to `for-the-badge` style with real brand logos (GitHub Actions, semver, Rust, Discord, OpenAI) — same visual language across github.com and Markdown previews.
- README model-label examples and Fast-mode visibility examples refer to GPT-5.5.

### Fixed

- `clippy::collapsible_match` warning in `src/session/activity.rs` (surfaced on Rust 1.95.0 stable) is collapsed into a single match guard so `cargo clippy -- -D warnings` stays green on the current toolchain.
- `.gitignore` now excludes the GitNexus MCP boilerplate (`CLAUDE.md`, `AGENTS.md`, `.claude/`) that GitNexus injects per workspace and that previously surfaced as untracked.

## [1.1.0] - 2026-03-07

### Added

- Full-screen TUI plan selector with instant save support for `Auto Detect`, `Free`, `Go`, `Plus`, `Pro`, `Business`, and `Enterprise`.
- Fast-mode visibility derived from Codex global state, including the lightning-prefixed model label.
- Reasoning-effort visibility across TUI, status output, and Discord presence.
- Official pricing catalog entries for `gpt-5.4`, `gpt-5.4-2026-03-05`, `gpt-5.3-codex`, and `gpt-5.3-codex-latest`.

### Changed

- Release outputs are standardized under `releases/<platform>/`, with Cargo cache kept under `.build/target`.
- Active-session selection is recency-first and uses deterministic tie-breakers based on pending calls, activity priority, and session ID.
- Discord and TUI model labels now compose model, Fast mode, reasoning effort, and resolved account plan more cleanly.
- Session parsing internals are now split into focused activity and parser modules for easier maintenance.
- README, schema docs, API contract, and UI docs were refreshed to reflect plan selection, Fast mode, reasoning visibility, and current build/release layout.

### Fixed

- Idle transitions no longer fabricate recency or prematurely hide active sessions.
- Commentary and tool-call activity tracking now preserves working-state visibility more accurately.
- Incremental JSONL parsing safely retains partial lines across updates.
- Legacy `gpt-5.3-codex -> gpt-5.2-codex` pricing aliasing is migrated away; Spark variants now map to `gpt-5.3-codex`.

## [0.2.2] - 2026-02-10

### Changed

- Release publishing now includes only packaged archives (`.tar.gz` / `.zip`) and `.sha256` checksum files.

## [0.2.1] - 2026-02-10

### Changed

- Release workflow macOS x64 runner updated to `macos-15-intel` for current GitHub Actions compatibility.

## [0.2.0] - 2026-02-10

### Added

- Incremental session parsing cache:
  - per-file cursor tracking
  - appended-line parsing only
  - cached snapshot reuse for unchanged files
- New spec-kit plan:
  - `plans/20260210-ux-intuitive-realtime-github-bootstrap/plan.md`
- Branding assets:
  - `assets/branding/project-mark.svg`
  - `assets/branding/social-card.svg`
  - `assets/branding/README.md`
- Release note categorization config:
  - `.github/release.yml`
- Config schema v3:
  - `privacy.show_activity`
  - `privacy.show_activity_target`
  - `display.terminal_logo_mode`
  - `display.terminal_logo_path`
- Session activity extraction and snapshot model:
  - `SessionActivityKind`
  - `SessionActivitySnapshot`
  - `CodexSessionSnapshot.activity`
- Terminal footer author credit:
  - `By XT0N1.T3CH | Discord @XT0N1.T3CH | ID 211189703641268224`

### Changed

- Discord details now prioritize live activity (`Thinking`, `Reading`, `Editing`, etc.).
- Discord state copy now uses natural labels:
  - `Last response`
  - `Session total`
  - `This update`
  - `5h left`
  - `7d left`
- TUI/status token copy now uses human-readable `Tokens: ...` format.
- Smart foreground TUI now shows:
  - live activity line,
  - semantic colorized remaining-limit bars (green/yellow/red thresholds),
  - hybrid official-logo rendering with ASCII fallback.
- `status` output now includes activity summary when enabled.
- Default polling interval updated to `2s` for balanced real-time behavior.
- CI workflow now includes Rust dependency caching for faster runs.
- Release workflow now uses `.build/target` to match Cargo target-dir config.

## [0.1.2] - 2026-02-09

### Added

- Session token metrics from Codex JSONL:
  - `session_delta_tokens`
  - `last_turn_tokens`
  - `session_total_tokens`
  - `last_token_event_at`
- Global limits-source selection based on most recent active session token event.
- `.cargo/config.toml` to route build output to `.build/target`.
- New spec-kit plan:
  - `plans/20260209-codex-discord-presence-v1-2-usage-parity-ui/plan.md`

### Changed

- 5h/7d semantics now match Codex CLI display:
  - show **remaining** percent, not used percent.
- Discord state formatting now prioritizes model + real token triplet + remaining limits with deterministic truncation.
- Discord update pipeline is now error-aware (no silent update failures).
- TUI layout redesigned with cleaner runtime/session sections and remaining-limit bars.
- `status` command now reports token triplet and remaining-limit snapshot.
- Release workflow now produces grouped artifact structure by platform/arch under `dist/<os>/<arch>`.

## [0.1.1] - 2026-02-09

### Added

- Config schema v2 with project defaults:
  - `discord_client_id = 1470480085453770854`
  - `discord_public_key` metadata
  - default asset keys `codex-logo` / `openai`
- ASCII hero banner in TUI (OpenAI-style icon + `CODEX` wordmark).
- Instance metadata file `~/.codex/codex-discord-presence.instance.json`.

### Changed

- Smart mode and `codex` wrapper now use automatic takeover:
  - new launch terminates older running instance and acquires lock.
- `status` running detection now reports authoritative lock state.
- No-TTY startup now attempts terminal relaunch before headless fallback.
- Discord missing-client-id status text standardized to `Missing CODEX_DISCORD_CLIENT_ID`.

## [0.1.0] - 2026-02-09

### Added

- Initial Rust implementation with single-binary UX.
- Commands:
  - default smart foreground mode
  - `codex` child wrapper mode
  - `status`
  - `doctor`
- Cross-platform Discord IPC integration.
- Codex session JSONL parser with tolerance for missing fields.
- Live usage windows (5h/7d) and token display.
- Single-instance lock handling.
- Open source docs and CI/release workflows.
