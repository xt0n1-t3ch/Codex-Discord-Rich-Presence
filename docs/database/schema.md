# Data Schema

This project has no relational database.

## Persisted Local Files

### 1) Config file

- Path: `~/.codex/discord-presence-config.json`
- Schema version: `3` (non-breaking additions only)
- Fields:
  - `schema_version: number`
  - `discord_client_id: string | null`
  - `discord_public_key: string | null` (metadata only)
  - `privacy: object`
    - `enabled`, `show_project_name`, `show_git_branch`, `show_model`
    - `show_tokens`, `show_limits`, `show_activity`, `show_activity_target`
  - `display: object`
    - `large_image_key: string`
    - `large_text: string`
    - `small_image_key: string`
    - `small_text: string`
    - `activity_small_image_keys: object` (optional)
      - `thinking?: string`
      - `reading?: string`
      - `editing?: string`
      - `running?: string`
      - `waiting?: string`
      - `idle?: string`
    - `terminal_logo_mode: "auto" | "ascii" | "image"`
    - `terminal_logo_path: string | null`

### 2) Lock file

- Path: `~/.codex/codex-discord-presence.lock`
- Purpose: single-instance guard.

### 3) Instance metadata file

- Path: `~/.codex/codex-discord-presence.instance.json`
- Fields:
  - `pid: number`
  - `exe_path: string | null`

## External Read-Only Input

- Session logs: `~/.codex/sessions/**/*.jsonl`
- Main consumed event families:
  - `session_meta`
  - `turn_context`
  - `event_msg` (`token_count`, `agent_reasoning`, `agent_message`, `user_message`)
  - `response_item` (`reasoning`, `function_call`, `custom_tool_call`, outputs, messages)

## Derived Runtime Session Snapshot

Per session:

- `session_total_tokens`
- `last_turn_tokens`
- `session_delta_tokens`
- `last_token_event_at`
- `last_activity`
- `activity.kind`
- `activity.target`
- `activity.observed_at`
- `activity.last_active_at`
- `activity.last_effective_signal_at` (new non-breaking runtime field)
- `activity.idle_candidate_at`
- `activity.pending_calls`

## Activity Lifecycle Rules

- Tool output events do not force immediate `Idle`.
- `Idle` is derived only when:
  - there are no pending calls, and
  - debounce window elapsed since latest effective signal reference.
- Effective signals include reasoning, tool call/outputs, and assistant messaging signals.

## Session Visibility + Ranking

Visibility uses dual thresholds:

- strict stale cutoff (`CODEX_PRESENCE_STALE_SECONDS`),
- sticky non-idle window (`CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`, default 3600s).

Active session ranking:

1. pending calls (higher first),
2. non-idle over idle,
3. latest recency.

## Runtime Parse Cache (In-memory)

Incremental parser cache stores:

- cursor offset per JSONL,
- file length + modified timestamp,
- accumulated parsed state,
- last built snapshot.

Behavior:

- unchanged files reuse cached snapshots,
- changed files parse appended lines only,
- truncated/rotated files reset and reparse from start.
