# Codex Discord Presence v1 Plan

## 1) Discovery & Context

- Base behavior referenced from `cc-discord-presence`.
- Codex runtime data source confirmed: `~/.codex/sessions/**/*.jsonl`.
- Product direction fixed to single-binary UX per platform.
- Bootstrap dependencies handled on local machine, outside repository scope.

## 2) Scope & Non-Goals

### Scope

- Rust implementation for Windows/Linux/macOS.
- Single process runtime with terminal UI + Discord Rich Presence.
- Commands: default smart mode, `codex`, `status`, `doctor`.
- OSS docs and CI/release setup.

### Non-Goals

- Persistent daemon service by default.
- Auto-provisioning of Discord app IDs.
- Shipping OS dependency installers in the repo.

## 3) Architecture & Data Flow

- Session scanner reads active JSONL files under stale threshold.
- Session parser extracts project/model/tokens/rate limits.
- Usage aggregator derives 5h/7d windows for display and state text.
- Discord IPC publisher sends presence payloads.
- Foreground UI renders operational state and active session metrics.

## 4) Interfaces & Schemas

- Config: `~/.codex/discord-presence-config.json`.
- Internal key types:
  - `CodexSessionSnapshot`
  - `UsageWindow`
  - `RateLimits`
  - `PresenceConfig`
- Command contract documented in `docs/api/codex-presence.md`.

## 5) Implementation Phases

1. Scaffold Rust crate and module boundaries.
2. Implement config + runtime settings.
3. Implement session parser and active-session scanning.
4. Implement Discord IPC integration and payload mapping.
5. Implement foreground UI and optional child-wrapper mode.
6. Add lock-based single-instance control.
7. Add docs, tests, CI and release automation.

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `cargo build --release`
- Manual smoke:
  - `status` and `doctor`
  - smart mode rendering
  - graceful shutdown clears activity

## 7) Rollout, Risks & Backout

### Rollout

- Publish `v0.1.0` with cross-platform release artifacts and checksums.

### Risks

- JSONL schema drift in Codex events.
- OS-specific Discord IPC edge cases.

### Mitigation

- Defensive parsing and fallback logic.
- CI matrix with all primary targets.

### Backout

- Stop foreground app.
- Delete local lock/config if needed.
- Revert to previous binary release.
