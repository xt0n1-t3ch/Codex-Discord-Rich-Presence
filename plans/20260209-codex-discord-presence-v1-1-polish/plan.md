# Codex Discord Presence v1.1 Plan

## 1) Discovery & Context

- Existing Rust implementation was functional but had two UX gaps:
  - second launch could exit immediately when lock was already held
  - status could show `running: false` when PID could not be read under lock contention
- Branding/default config needed Codex + OpenAI identity aligned to the selected Discord app.
- Project remains single-binary (no start/stop script workflow).

## 2) Scope & Non-Goals

### Scope

- Add automatic takeover of previous running instance.
- Improve `status` running detection reliability.
- Migrate config schema to v2 with project defaults:
  - `discord_client_id`
  - `discord_public_key` (metadata)
  - asset keys `codex-logo` / `openai`
- Add ASCII hero banner in terminal dashboard.
- Add no-TTY terminal relaunch attempt with safe fallback.
- Update docs and tests for v1.1 behavior.

### Non-Goals

- Persistent daemon mode.
- Script-based lifecycle controls.
- HTTP interaction features requiring Discord public key usage.

## 3) Architecture & Data Flow

- Keep lockfile for single-instance guarantee.
- Introduce a sidecar instance metadata file:
  - stores PID/exe path while lock is held
  - enables robust status and takeover flow
- Startup flow:
  1. parse CLI + load/migrate config
  2. acquire lock or takeover old instance
  3. run foreground dashboard / codex-wrapper mode
  4. cleanup lock + metadata on shutdown

## 4) Interfaces & Schemas

- `PresenceConfig` schema v2:
  - `schema_version`
  - `discord_client_id`
  - `discord_public_key`
  - `privacy`
  - `display`
- New persisted file:
  - `~/.codex/codex-discord-presence.instance.json`
- Internal runtime interfaces:
  - `RunningState::{NotRunning, Running{pid}}`
  - `acquire_or_takeover_single_instance()`

## 5) Implementation Phases

1. Config migration + defaults.
2. Lock and metadata refactor.
3. Takeover orchestration in CLI entrypoint.
4. No-TTY relaunch + headless fallback updates.
5. ASCII banner and UI polish.
6. Docs/spec updates.
7. Validation pass (`fmt`, `clippy`, `test`, `build`, smoke checks).

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`
- Smoke checks:
  - `status` running detection
  - takeover on second launch
  - `doctor` config visibility
  - dashboard rendering path

## 7) Rollout, Risks & Backout

### Rollout

- Publish as `v0.1.1` release artifacts for Windows/Linux/macOS.

### Risks

- PID reuse race during takeover.
- Terminal relaunch differences across desktop environments.

### Mitigation

- PID stored in metadata and lock reacquire verification.
- Soft then hard terminate strategy before failure.
- Headless fallback if terminal relaunch is unavailable.

### Backout

- Stop process.
- Remove stale lock/instance metadata files.
- Revert to prior release binary.
