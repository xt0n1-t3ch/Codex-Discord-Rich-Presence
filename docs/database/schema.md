# Local Schema Map

All data stays on the user's machine. The daemon reads Codex/OpenCode local state and writes only config plus local report artifacts.

## Config

Path: `~/.codex/discord-presence-config.json`

| Owner | Fields |
|:---|:---|
| Identity | Discord client IDs, Codex asset keys, desktop asset keys |
| Runtime | Poll interval, stale cutoff, active sticky window |
| Display | Terminal logo mode, logo path, large/small image text |
| Pricing | Model aliases and overrides |
| Plan | Local plan override and preset selection |

## Session Snapshot

`CodexSessionSnapshot` is the canonical active-session shape.

| Group | Fields |
|:---|:---|
| Identity | `session_id`, `cwd`, `project_name`, `git_branch`, `originator`, `source` |
| Model | `model`, `reasoning_effort`, `pricing_source`, `context_window` |
| Usage | `input_tokens_total`, `cached_input_tokens_total`, `output_tokens_total`, `total_cost_usd`, `cost_breakdown` |
| Activity | `activity`, `last_activity`, token-event timestamps |
| Limits | `limits`, `rate_limit_envelopes` |
| Source | `source_file` |

## Cost Breakdown

| Field | Type | Notes |
|:---|:---|:---|
| `input_cost_usd` | number | Non-cached input cost |
| `cached_input_cost_usd` | number | Cached-input read cost |
| `output_cost_usd` | number | Output cost |
| `cached_input_savings_usd` | number | Cached discount versus full input price |

## Metrics Snapshot

Path: `~/.codex/discord-presence-metrics.json`

| Field | Notes |
|:---|:---|
| `totals.input_tokens` | Total input including cached tokens |
| `totals.cached_input_tokens` | Prompt-cache read tokens |
| `totals.output_tokens` | Output tokens |
| `totals.cache_hit_ratio` | `cached_input_tokens / input_tokens` |
| `cost_breakdown.cached_input_savings_usd` | Aggregate cached-input savings |
| `by_model[].cache_hit_ratio` | Per-model cache health |

## Context Windows

OAuth-visible Codex context uses 400K for GPT-5/Codex family models. API-only metadata is tracked as 1,050,000 context, 272K long-context input threshold, and 128K max output.
