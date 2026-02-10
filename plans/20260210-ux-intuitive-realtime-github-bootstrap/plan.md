# Plan: UX Intuitive Realtime + GitHub Bootstrap
Date: 2026-02-10
Slug: ux-intuitive-realtime-github-bootstrap
Status: implemented

## 1) Discovery & Context
- Existing TUI and Rich Presence still used technical token abbreviations that reduced readability.
- Activity tracking was improved but needed stronger real-time behavior with low overhead.
- Repository metadata and release automation were incomplete for a production-grade open-source presence.
- Release workflow path did not match configured Cargo target dir (`.build/target`).

## 2) Scope & Non-Goals
### Scope
- Human-first copy for token and model telemetry in TUI and Discord.
- Real-time behavior with incremental session parsing, render dedupe, and publish throttling.
- Professional repository bootstrap: README, branding SVGs, CI/CD hardening, release notes config.
- Prepare direct push-to-main delivery and semver tag release workflow.

### Non-Goals
- No change to core CLI command surface.
- No redistribution of official OpenAI logo binaries inside repo.
- No migration to non-terminal UI frameworks.

## 3) Architecture & Data Flow
1. Session collector uses `SessionParseCache` to parse only appended JSONL lines.
2. `SessionAccumulator` persists parsed state and derives snapshots without reparsing full files.
3. `ActivityTracker` keeps pending tool calls and applies `Idle` debounce at finalize time.
4. App loop runs balanced real-time polling and redraws only when frame signature changes.
5. Discord publisher emits action-first payloads using natural labels and deduped sends.

## 4) Interfaces & Schemas
- No breaking config changes; schema stays at `3`.
- Internal structures added/extended:
  - `SessionParseCache`
  - `SessionAccumulator`
  - `SessionActivitySnapshot` fields already in use (`last_active_at`, `idle_candidate_at`, `pending_calls`).
- Public text contract updated:
  - `Tokens: This update X | Last response Y | Session total Z`
  - Discord `state`: `Model ... | Last response ... | Session total ... | 5h left ... | 7d left ...`

## 5) Implementation Phases
1. Add incremental parse cache and accumulator in `src/session.rs`.
2. Update app call sites to pass parse cache in all runtime modes.
3. Replace token copy in `src/util.rs`, `src/ui.rs`, and `src/discord.rs` with intuitive language.
4. Keep balanced real-time defaults and dedupe logic (`2s` polling baseline).
5. Add branding assets under `assets/branding`.
6. Upgrade CI and release workflows; add `.github/release.yml`.
7. Refresh docs (`README`, API/UI/schema docs, changelog references).

## 6) Validation & Acceptance
### Automated
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

### Test Scenarios
- Incremental parser advances cursor and updates snapshot from appended lines.
- Presence state uses human labels and excludes technical shorthand.
- Idle debounce still prevents false idle during active tool flow.
- Frame signature logic supports redraw dedupe.

### Manual Checks
- Discord card readable at a glance for non-technical users.
- TUI remains stable across terminal sizes.
- CPU usage remains low in idle periods.

## 7) Rollout, Risks & Backout
### Rollout
- Commit and push to `main`.
- Trigger CI automatically.
- Create semver tag to trigger release workflow and publish binaries.

### Risks
- Text length constraints in Discord can truncate rich labels.
- Terminal support for image logos varies by emulator.
- Social preview image still requires one manual step in GitHub Settings.

### Mitigations
- Priority-aware truncation in presence payload.
- ASCII and compact layout fallbacks.
- Document manual social preview setup.

### Backout
- Revert copy/format changes independently from parser cache changes.
- Revert workflow modifications if release pipeline regressions are detected.
