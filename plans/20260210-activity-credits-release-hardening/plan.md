# Plan: Activity Detection Hardening + Credits SVG Premium + Release Badge Fix

## 1) Discovery & Context

- False `Waiting for input` states were observed while sessions were still active.
- `response_item.message` events with `phase=commentary` were being treated like terminal waiting states.
- `WaitingInput` was treated as sticky-active, causing stale waiting sessions to stay visible longer than intended.
- Session ranking did not distinguish working activity vs waiting activity.
- README release badge had inconsistent rendering in preview.
- Credits ribbon visual quality did not match the premium dark style of the project branding.

## 2) Scope & Non-Goals

### Scope

- Harden activity parsing and ranking in `src/session.rs`.
- Add tests for commentary/final-answer/web-search and updated ranking/sticky behavior.
- Redesign `assets/branding/credits-ribbon.svg` with dark Discord-style tactical-grid direction.
- Stabilize release badge rendering in `README.md`.
- Update required docs:
  - `docs/api/codex-presence.md`
  - `docs/database/schema.md`
  - `docs/ui/UI_SITEMAP.md`
- Record changes in `CHANGELOG.md`.

### Non-Goals

- No CLI contract changes.
- No config schema bump.
- No IPC architecture changes.
- No full social-card redesign.

## 3) Architecture & Data Flow

- Assistant messages are now interpreted phase-aware:
  - `phase=commentary`: secondary progress signal.
  - `phase=final_answer`: `Waiting for input`.
- Commentary policy:
  - does not replace active working activity labels.
  - can reactivate `WaitingInput`/`Idle` to `Thinking`.
- Web search events (`web_search_call`, `web_search_result`) are treated as working signals.
- Sticky visibility excludes `WaitingInput`; sticky applies to working activity kinds.
- Active session ranking order:
  1. pending calls
  2. activity class (`working` > `waiting` > `idle`)
  3. recency

## 4) Interfaces & Schemas

- Public interfaces: unchanged (`codex-discord-presence`, `status`, `doctor`).
- Config schema: unchanged (`schema_version = 3`).
- Internal runtime semantics updated:
  - phase-aware assistant message interpretation
  - waiting excluded from sticky extension
  - refined session ranking priority

## 5) Implementation Phases

1. Activity parser hardening in `src/session.rs`:
   - add commentary signal path
   - phase-aware assistant message handling
   - web search signal handling
2. Sticky/ranking updates:
   - exclude waiting from sticky-active logic
   - prioritize working > waiting > idle in rank key
3. Tests:
   - commentary retention/reactivation behavior
   - final-answer waiting behavior
   - web search working signal
   - sticky/ranking ordering expectations
4. Branding:
   - premium redesign for `assets/branding/credits-ribbon.svg`
5. README:
   - robust dynamic release badge URL with simplified query parameters
6. Docs + changelog:
   - update `docs/*` contracts and `CHANGELOG.md`

## 6) Validation & Acceptance

### Automated

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

### Acceptance Criteria

- No false `Waiting for input` while commentary/progress signals indicate ongoing work.
- Working sessions outrank waiting/idle sessions deterministically.
- `WaitingInput` no longer extends long sticky visibility.
- README `Release` badge renders reliably.
- Credits ribbon appears visually aligned with the project dark premium brand.

## 7) Rollout, Risks & Backout

### Rollout

- Land code + docs + branding in one cohesive release set.
- Run full Rust validation before packaging.

### Risks

- Older session logs without `phase` can still map to conservative waiting behavior.
- Ranking changes may alter active-session selection in edge recency ties.
- Badge rendering still depends on external Shields/GitHub availability.

### Mitigations

- Conservative fallback for unknown assistant message phase.
- Deterministic rank key and explicit tests for ordering.
- Use simplified dynamic badge URL with minimal query params.

### Backout

- Revert activity/ranking logic independently (`src/session.rs`).
- Revert branding asset independently (`assets/branding/credits-ribbon.svg`).
- Revert README/docs independently (`README.md`, `docs/*`, `CHANGELOG.md`).
