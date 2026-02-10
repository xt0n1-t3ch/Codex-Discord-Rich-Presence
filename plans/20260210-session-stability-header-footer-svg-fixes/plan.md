# Plan: Session Stability + Header/Footer + SVG Polish

## 1) Discovery & Context

- Session visibility currently drops on strict file `modified` stale cutoff.
- Header ASCII can look ambiguous (`COSEX`) in some terminal fonts.
- Footer credit is centered and not anchored to bottom-right.
- Social card text layout can clip at GitHub preview widths.
- README subtitle line is not centered.

## 2) Scope & Non-Goals

### Scope

- Add sticky active-session visibility window (default 60 minutes).
- Replace ambiguous header wordmark with legible block `CODEX`.
- Anchor footer with left quit hint and right credit.
- Reflow social card text and center README subtitle.
- Ship updated Windows executable in `dist/windows/x64/`.

### Non-Goals

- No breaking config schema changes.
- No lock/IPC architecture changes.
- No migration away from terminal UI.

## 3) Architecture & Data Flow

- Session selection uses dual thresholds:
  - strict stale cutoff,
  - sticky window for non-idle sessions.
- Session recency is computed from strongest signal:
  - file modified,
  - `last_token_event_at`,
  - `activity.last_active_at`,
  - `activity.observed_at`.
- UI frame keeps existing dedupe flow; only header/footer rendering behavior changes.

## 4) Interfaces & Schemas

- New runtime env var:
  - `CODEX_PRESENCE_ACTIVE_STICKY_SECONDS` (default `3600`, min clamp).
- No new required config file fields.
- Docs updated:
  - `docs/ui/UI_SITEMAP.md`
  - `docs/api/codex-presence.md`
  - `docs/database/schema.md`
  - `README.md`

## 5) Implementation Phases

1. Runtime settings:
   - add sticky window runtime setting and env parsing.
2. Session filtering:
   - remove strict pre-parse stale hard-drop,
   - apply dual-threshold include policy.
3. Header/footer:
   - replace wordmark ASCII,
   - implement anchored single-row footer composition.
4. Branding/readme:
   - reflow social card text with centered safe-width lines,
   - center subtitle in README.
5. Docs/changelog:
   - add behavior and env documentation updates.
6. Build/package:
   - release build and refresh Windows dist binary.

## 6) Validation & Acceptance

### Automated

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

### New tests

- sticky policy includes non-idle sessions within sticky window.
- sticky policy excludes idle sessions beyond strict stale cutoff.
- sessions outside sticky window are excluded.
- strict stale path still includes recent sessions.
- footer composition prevents overlap on narrow widths.

### Manual acceptance

- Header reads `CODEX` clearly.
- Footer stays bottom-right/bottom-left aligned on terminal resize.
- Quiet active sessions do not disappear after a few seconds.
- README/social card render without clipping.

## 7) Rollout, Risks & Backout

### Rollout

- Commit and push to `main`.
- Validate local release build.
- Replace `dist/windows/x64/codex-discord-presence.exe`.

### Risks

- Sticky window can keep sessions visible longer than expected.
- Terminal font differences can still alter ASCII appearance.

### Mitigation

- Sticky window is env-configurable.
- Wordmark prioritizes simple legible glyph shapes.

### Backout

- Revert sticky visibility commit independently.
- Revert header/footer styling commit independently.
