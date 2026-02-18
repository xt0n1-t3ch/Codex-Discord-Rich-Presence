# Data Schema

This project has no relational database.

## Persisted Local Files

### 1) Config file

- Path: `~/.codex/discord-presence-config.json`
- Schema version: `4` (non-breaking additions only)
- Fields:
  - `schema_version: number`
  - `discord_client_id: string | null`
  - `discord_public_key: string | null` (metadata only)
  - `privacy: object`
    - `enabled`, `show_project_name`, `show_git_branch`, `show_model`
    - `show_tokens`, `show_cost`, `show_limits`, `show_activity`, `show_activity_target`
  - `display: object`
    - `large_image_key: string` (asset key or `https://` URL)
    - `large_text: string`
    - `small_image_key: string` (asset key or `https://` URL)
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
  - `pricing: object`
    - `aliases: Record<string, string>` (normalized lowercase model keys)
    - `overrides: Record<string, { input_per_million: number, cached_input_per_million?: number, output_per_million: number }>`

### 2) Lock file

- Path: `~/.codex/codex-discord-presence.lock`
- Purpose: single-instance guard.

### 3) Instance metadata file

- Path: `~/.codex/codex-discord-presence.instance.json`
- Fields:
  - `pid: number`
  - `exe_path: string | null`

### 4) Metrics JSON snapshot

- Path: `~/.codex/discord-presence-metrics.json`
- Persist strategy: write `.tmp` then atomic rename.
- Fields:
  - `daemon_started_at: datetime (UTC)`
  - `snapshot_at: datetime (UTC)`
  - `uptime_seconds: number`
  - `totals: { cost_usd, input_tokens, cached_input_tokens, output_tokens, total_tokens }`
  - `cost_breakdown: { input_cost_usd, cached_input_cost_usd, output_cost_usd }`
  - `by_model: Array<{ model_id, cost_usd, input_tokens, cached_input_tokens, output_tokens, session_count }>`
  - `active_sessions: number`

### 5) Metrics Markdown report

- Path: `~/.codex/discord-presence-metrics.md`
- Persist strategy: write `.tmp` then atomic rename.
- Content: totals, token split, cost split, per-model table, uptime.

## External Read-Only Input

- Session logs: `~/.codex/sessions/**/*.jsonl`
- Main consumed event families:
  - `session_meta`
  - `turn_context`
  - `event_msg` (`token_count`, `agent_reasoning`, `agent_message`, `user_message`)
  - `response_item` (`reasoning`, `function_call`, `custom_tool_call`, outputs, messages, `web_search_call`, `web_search_result`)

## Derived Runtime Session Snapshot

Per session:

- `session_total_tokens`
- `last_turn_tokens`
- `session_delta_tokens`
- `input_tokens_total`
- `cached_input_tokens_total`
- `output_tokens_total`
- `last_input_tokens`
- `last_cached_input_tokens`
- `last_output_tokens`
- `total_cost_usd`
- `cost_breakdown`
- `pricing_source`
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
- Effective signals include reasoning, tool call/outputs, web search signals, and assistant messaging signals.
- `Idle` transitions do not rewrite timestamps to `now`; recency stays tied to real observed/effective signals.
- Assistant activity interpretation:
  - `phase=commentary`: live progress signal (secondary), does not replace active working label.
  - `phase=final_answer`: `Waiting for input`.
  - unknown/missing phase: conservative fallback to `Waiting for input`.
- `event_msg.agent_message` is treated as progress commentary, not immediate waiting state.

## Session Visibility + Ranking

Visibility uses dual thresholds:

- strict stale cutoff (`CODEX_PRESENCE_STALE_SECONDS`),
- sticky working-activity window (`CODEX_PRESENCE_ACTIVE_STICKY_SECONDS`, default 3600s).
  - sticky applies to: `Thinking`, `Reading`, `Editing`, `Running`, `Waiting for input`.

Active session ranking:

1. latest recency (`last_activity`),
2. pending calls (higher first),
3. activity class priority:
   - working (`Thinking`, `Reading`, `Editing`, `Running command`)
   - `Waiting for input`
   - `Idle`
4. stable `session_id` tiebreak.

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

## Discord Asset Validation (Runtime)

- The app periodically checks `https://discord.com/api/v10/oauth2/applications/{app_id}/assets`.
- If configured image keys are missing from the catalog, invalid keys are omitted from payload.
- Omitted invalid keys prevent `?` placeholders on Discord mobile and fall back to standard app icon rendering.
