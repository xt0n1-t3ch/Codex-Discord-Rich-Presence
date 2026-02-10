# Plan: Recent Sessions Always Visible + Premium Presence Polish

## 1) Discovery & Context

- `Recent Sessions` could disappear because render order consumed available rows before the section.
- Activity could fall back to `Idle` even with recent signals due to debounce reference precedence.
- Active session selection previously relied on list head only.
- README/social assets needed clipping and credits polish.

## 2) Scope & Non-Goals

### Scope

- Keep `Recent Sessions` visible in all layout modes with compact fallback.
- Improve anti-false-idle behavior and active-session ranking.
- Polish Discord presence lines and optional activity image mapping.
- Unify credits format (`XT0N1.T3CH | Discord @XT0N1.T3CH | ID 211189703641268224`).
- Update docs and branding assets.

### Non-Goals

- No breaking CLI changes.
- No GUI migration.
- No hard dependency on custom Discord activity image assets.

## 3) Architecture & Data Flow

- `ActivityTracker` tracks `last_effective_signal_at` and uses it in idle debounce.
- Session ranking uses priority: pending calls > non-idle > recency.
- UI reserves bottom frame rows for `Recent Sessions` and degrades to single-line compact entries when needed.
- Discord payload remains deduped/rate-limited and uses action-first details/state with deterministic truncation.

## 4) Interfaces & Schemas

- `SessionActivitySnapshot` adds `last_effective_signal_at` (non-breaking optional field).
- `display.activity_small_image_keys` added as optional per-activity mapping in config:
  - `thinking`, `reading`, `editing`, `running`, `waiting`, `idle`.
- `schema_version` remains `3`.

## 5) Implementation Phases

1. Update session parsing/activity tracking and ranking.
2. Wire active-session selection to ranking in app flows.
3. Implement TUI row reservation and compact recent-session fallback.
4. Polish Discord details/state and optional per-activity small-image key handling.
5. Update README + branding assets + documentation.

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

Acceptance targets:

- `Recent Sessions` remains visible with useful content in constrained heights.
- No false `Idle` during recent/ongoing activity.
- Credits formatting is consistent across terminal, README, and social card.

## 7) Rollout, Risks & Backout

### Rollout

- Land code/docs updates on `main`.
- Keep CI/CD release process documentation on `development` branch only.

### Risks

- Smaller terminals may show less detail in runtime/active sections due to reserved recent rows.
- Optional activity asset keys may be missing in Discord app settings.

### Backout

- Revert UI reservation logic independently.
- Revert activity image mapping while keeping payload text improvements.
- Revert ranking/debounce changes if regressions appear.
