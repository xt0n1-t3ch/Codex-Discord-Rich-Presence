# Codex Discord Presence v1.2 Plan

## 1) Discovery & Context

- Existing app worked but showed `used_percent` while Codex CLI presents remaining rate limits.
- Token display favored session lifetime totals and lacked clear per-turn/per-update context.
- Discord updates were invoked with ignored errors in runtime loops.
- Release artifacts were not grouped in user-friendly OS/arch folder structure.

## 2) Scope & Non-Goals

### Scope

- Real-time limit parity with Codex CLI (`remaining` semantics).
- Real token metrics from active sessions: `delta | last | total`.
- Improved terminal UX and compact Discord state formatting.
- Dist layout organized by platform and architecture.

### Non-Goals

- No daemon mode by default.
- No start/stop script lifecycle.
- No dependency on private Codex APIs.

## 3) Architecture & Data Flow

1. Scan active JSONL sessions.
2. Parse token events into session snapshots.
3. Derive token metrics per session:
   - total tokens
   - last turn tokens
   - delta from last two totals
4. Build effective limits from freshest token event among active sessions.
5. Render TUI and publish Discord activity with deterministic truncation.

## 4) Interfaces & Schemas

- `CodexSessionSnapshot` adds:
  - `session_total_tokens`
  - `last_turn_tokens`
  - `session_delta_tokens`
  - `last_token_event_at`
- `UsageWindow` adds:
  - `remaining_percent`
- New helper:
  - `latest_limits_source(&[CodexSessionSnapshot]) -> Option<&CodexSessionSnapshot>`

## 5) Implementation Phases

1. Extend parser and session model in `src/session.rs`.
2. Update runtime loops/status in `src/app.rs`.
3. Harden Discord update pipeline and formatting in `src/discord.rs`.
4. Redesign TUI sections and remaining-limit bars in `src/ui.rs`.
5. Add build output relocation via `.cargo/config.toml`.
6. Update release workflow to `dist/<os>/<arch>`.
7. Refresh docs and tests.

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`
- Manual checks:
  - status shows `remaining` semantics.
  - Discord state shows real token triplet.
  - second launch takeover remains stable.

## 7) Rollout, Risks & Backout

### Rollout

- Publish as `v0.1.2` with per-platform release assets.

### Risks

- JSONL event structure drift.
- Terminal rendering variance across emulators.

### Mitigation

- Tolerant parser with fallback logic.
- compact-mode UI fallback for narrow terminals.
- deterministic truncation for Discord payloads.

### Backout

- Revert to prior tag/binary.
- remove local lock/instance metadata if stale state appears.
