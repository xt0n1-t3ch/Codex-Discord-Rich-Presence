# Terminal Activity + Logo + Author Credit Plan

## 1) Discovery & Context

- Existing TUI used static ASCII branding with no image protocol path.
- Existing Discord state showed model/tokens/limits but no real-time task activity.
- Session parser consumed token/rate-limit events only.
- Config schema was v2 and lacked activity/logo controls.

## 2) Scope & Non-Goals

### Scope

- Add parser support for activity extraction from Codex session events.
- Add activity rendering in TUI and Discord Rich Presence.
- Add hybrid terminal logo mode (`auto` image + fallback ASCII).
- Add semantic color thresholds for remaining-limit bars.
- Add non-invasive author credit footer.
- Upgrade config schema to v3 and document contract updates.

### Non-Goals

- No desktop GUI output.
- No requirement to bundle OpenAI logo binaries in repo.
- No change to single-instance lock/takeover workflow.

## 3) Architecture & Data Flow

1. Parse JSONL session lines.
2. Continue token/limits extraction from `event_msg.token_count`.
3. Extract activity from:
   - `event_msg.agent_reasoning`
   - `response_item.reasoning`
   - `response_item.function_call`
   - `response_item.custom_tool_call`
   - corresponding output events for completion/idle transitions.
4. Store latest activity in `CodexSessionSnapshot.activity`.
5. Render activity in terminal and map to Discord details line.

## 4) Interfaces & Schemas

- New `SessionActivityKind` enum.
- New `SessionActivitySnapshot` struct.
- `CodexSessionSnapshot` adds `activity: Option<SessionActivitySnapshot>`.
- Config schema v3:
  - `privacy.show_activity`
  - `privacy.show_activity_target`
  - `display.terminal_logo_mode`
  - `display.terminal_logo_path`

## 5) Implementation Phases

1. Update dependencies/config schema/example config.
2. Implement activity parsing and heuristics for file-target detection.
3. Redesign TUI output with activity line, colorized bars, and author credit footer.
4. Implement hybrid logo rendering via terminal image protocol support and fallback path.
5. Update Discord presence line composition to prioritize activity.
6. Extend tests for parser/activity/layout thresholds.
7. Update docs (`README`, `docs/ui`, `docs/database`, `docs/api`).

## 6) Validation & Acceptance

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`
- Acceptance:
  - activity appears in TUI + Discord,
  - color thresholds are applied,
  - author credit is responsive and visible,
  - no regressions in token/limit parsing.

## 7) Rollout, Risks & Backout

### Rollout

- Ship as next minor release with schema migration to v3.

### Risks

- Terminal protocol support variance for image rendering.
- Heuristic file-target extraction may be imperfect on uncommon command formats.

### Mitigations

- Automatic fallback to ASCII banner.
- Conservative command parsing with generic fallback labels.

### Backout

- Set `terminal_logo_mode` to `ascii`.
- Disable detailed activity with privacy flags.
- Revert feature commit if runtime regressions are observed.
