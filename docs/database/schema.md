# Data Schema

This project has no relational database.

## Persisted Local Files

### 1) Config file

- Path: `~/.codex/discord-presence-config.json`
- Schema version: `3`
- Fields:
  - `schema_version: number`
  - `discord_client_id: string | null`
  - `discord_public_key: string | null` (metadata only, not used for IPC auth)
  - `privacy: object`
    - `enabled: bool`
    - `show_project_name: bool`
    - `show_git_branch: bool`
    - `show_model: bool`
    - `show_tokens: bool`
    - `show_limits: bool`
    - `show_activity: bool`
    - `show_activity_target: bool`
  - `display: object`
    - `large_image_key: string`
    - `large_text: string`
    - `small_image_key: string`
    - `small_text: string`
    - `terminal_logo_mode: "auto" | "ascii" | "image"`
    - `terminal_logo_path: string | null`

### 2) Lock file

- Path: `~/.codex/codex-discord-presence.lock`
- Purpose: single-instance lock.

### 3) Instance metadata file

- Path: `~/.codex/codex-discord-presence.instance.json`
- Fields:
  - `pid: number`
  - `exe_path: string | null`
- Purpose:
  - status visibility under lock contention,
  - takeover of prior instance.

## External Read-Only Session Input (`~/.codex/sessions/**/*.jsonl`)

Primary fields consumed:

- `session_meta.payload.id`
- `session_meta.payload.timestamp`
- `session_meta.payload.cwd`
- `turn_context.payload.model`
- `turn_context.payload.approval_policy`
- `turn_context.payload.sandbox_policy`
- `event_msg.payload.type == "token_count"`
- `event_msg.payload.type == "agent_reasoning"`
- `response_item.payload.type == "reasoning"`
- `response_item.payload.type == "function_call"`
- `response_item.payload.type == "custom_tool_call"`
- `response_item.payload.call_id`
- `response_item.payload.arguments`
- `response_item.payload.input`
- `event_msg.payload.info.total_token_usage.total_tokens`
- `event_msg.payload.info.last_token_usage.total_tokens`
- `event_msg.payload.rate_limits.primary.used_percent`
- `event_msg.payload.rate_limits.secondary.used_percent`

## Derived Runtime Metrics

Per active session:

- `session_total_tokens`
- `last_turn_tokens`
- `session_delta_tokens`
- `last_token_event_at`
- `activity.kind`
- `activity.target`
- `activity.observed_at`
- `activity.last_active_at`
- `activity.idle_candidate_at`
- `activity.pending_calls`

Activity lifecycle notes:

- Tool output events no longer force immediate `Idle`.
- `Idle` is derived only when:
  - there are no pending tool calls, and
  - no active signal has been observed for 45 seconds.
- `Waiting for input` remains explicit after assistant message completion.

Per usage window:

- `used_percent` (raw from Codex event)
- `remaining_percent = clamp(100 - used_percent, 0..100)` (display semantics)

Global effective limits:

- selected from the most recent token event among active sessions.

## Runtime Parse Cache (In-memory)

To reduce CPU usage, session file parsing uses an in-memory incremental cache:

- cache key: session JSONL file path
- cached values:
  - cursor offset (byte position)
  - file length and modified time
  - accumulated parsed state for metrics/activity
  - last derived snapshot

Behavior:

- unchanged files reuse cached snapshots without reparsing;
- changed files parse only appended lines from the cached cursor;
- truncated/rotated files reset cache state and parse from start.
- session visibility uses dual thresholds:
  - strict stale cutoff (`CODEX_PRESENCE_STALE_SECONDS`),
  - sticky non-idle window (`CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`, default 3600s).
