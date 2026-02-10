# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Changed

- README restructured with concise, professional copy and clearer install/use/config flow.
- Branding visuals refreshed:
  - `assets/branding/social-card.svg` now uses dark social-card styling with OpenAI-style mark.
  - `assets/branding/project-mark.svg` now matches the dark branding system.
- Branding documentation wording updated for trademark clarity.
- Added spec-kit plan:
  - `plans/20260210-readme-branding-professional-polish/plan.md`

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
