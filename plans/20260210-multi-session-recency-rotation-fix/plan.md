# Plan: Multi-Session Recency and Waiting-State Rotation Fix

## 1) Discovery & Context

- Multi-session detection was unstable in real usage with many concurrent Codex windows.
- Recency could be artificially refreshed when sessions transitioned to `Idle`.
- `Waiting for input` sessions were excluded from sticky-window visibility and dropped too quickly.
- Active-session selection favored activity class over recency, which conflicted with expected behavior for multi-window workflows.

## 2) Scope & Non-Goals

### Scope

- Remove synthetic recency inflation during idle transitions.
- Use real activity/timestamp signals to compute session recency.
- Include `Waiting for input` in sticky-window visibility.
- Change active-session selection to recency-first with deterministic tie-breaks.
- Update contracts/docs/changelog to match behavior.

### Non-Goals

- No CLI command/flag changes.
- No config schema changes.
- No new telemetry or external dependencies.

## 3) Architecture & Data Flow

- `ActivityTracker::finalize` keeps `Idle` transitions from rewriting `observed_at` to `now`.
- `session_recency` now derives recency from real signals:
  - `activity.last_effective_signal_at`
  - `activity.last_active_at`
  - `activity.observed_at`
  - `last_token_event_at`
  - fallback: file modified time.
- Sticky inclusion logic now treats `WaitingInput` as sticky-eligible (same window as other active states).
- Session ranking key changed to:
  1. `last_activity` (newest first)
  2. `pending_calls` (higher first)
  3. activity priority (`Working` > `Waiting` > `Idle`)
  4. session id for deterministic ordering.

## 4) Interfaces & Schemas

- Public behavioral contract changes:
  - active-session selection: recency-first.
  - sticky visibility: includes `Waiting for input`.
- No schema migration required:
  - config schema remains `3`.
  - JSON config shape unchanged.

## 5) Implementation Phases

1. Update `src/session.rs`:
- remove idle-time `observed_at = now`.
- include `last_effective_signal_at` in recency calculation.
- include `WaitingInput` in sticky predicate.
- switch ranking key to recency-first with deterministic tie-breaks.

2. Update tests in `src/session.rs`:
- add `idle_transition_does_not_refresh_recency_to_now`.
- replace waiting-sticky exclusion test with waiting-sticky inclusion.
- add `preferred_active_session_prefers_most_recent_signal`.
- add `ranking_tiebreaks_by_pending_then_activity_when_recency_equal`.
- adjust ranking expectations to recency-first.

3. Update docs:
- `docs/api/codex-presence.md`
- `docs/database/schema.md`
- `docs/ui/UI_SITEMAP.md`
- `CHANGELOG.md`

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

Acceptance:
- Active Discord/TUI session follows newest real session signal.
- `Waiting for input` sessions remain visible during sticky window instead of dropping at strict stale cutoff.
- No recency inflation from idle conversion.
- Existing parsing/cache behavior remains stable.

## 7) Rollout, Risks & Backout

### Rollout

- Land code + docs together.
- Validate with real multi-window Codex usage and confirm active card changes within one poll interval.

### Risks

- Active session may switch more often under heavy concurrent activity.
- More waiting sessions may remain visible in recent lists.

### Backout

- Revert ranking policy independently if recency-first is too noisy.
- Keep idle-recency inflation fix even if ranking policy is rolled back.
- Revert waiting-sticky inclusion independently if session list becomes too verbose.
