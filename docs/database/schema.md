# Local Schema Map

All data stays on the user's machine. The daemon reads Codex/OpenCode local state and writes only config plus local report artifacts.

## Config

Path: `~/.codex/discord-presence-config.json`

Current schema version: `11`.

| Owner | Fields |
|:---|:---|
| Identity | Shared CLI/VS Code client id, Codex App desktop client id, and asset keys |
| Runtime | Poll interval, stale cutoff, active sticky window |
| Display | `desktop_presence_design` (`codex_app` or `chat_gpt_app`), terminal logo mode/path, and large/small image text |
| Pricing | Model aliases and overrides |
| Plan | Local plan override and preset selection. Manual tiers include `Pro 5x ($100/month)` and `Pro 20x ($200/month)`; legacy `pro` maps to Pro 20x. |
| Privacy | Project, branch, model, activity, tokens, cost, limits, context, systems, activity target, and global private-mode flags. |

## Session Snapshot

`CodexSessionSnapshot` is the canonical active-session shape.

| Group | Fields |
|:---|:---|
| Identity | `session_id`, `cwd`, `project_name`, `git_branch`, `originator`, `source` |
| Model | `model`, `reasoning_effort`, session-scoped `speed`, and `context_window` with source |
| Usage | token totals, backward-compatible `total_cost_usd`, optional `known_cost_usd`, `pricing_source`, `pricing_status`, `cost_attribution`, reconciliation flag, and `cost_breakdown` |
| Activity | `activity`, `last_activity`, token-event timestamps |
| Limits | `limits`, `rate_limit_envelopes` |
| Source | `source_file` |

## Cost Breakdown

| Field | Type | Notes |
|:---|:---|:---|
| `input_cost_usd` | number | Non-cached input cost |
| `cache_write_cost_usd` | number | Known cache-write cost; zero is not presented as exact when write telemetry is absent |
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
| `totals.known_cost_usd` | Known subtotal, absent when no verified component can be priced |
| `totals.pricing_status` | Aggregate `exact`, `partial`, or `unavailable` state |
| `totals.complete_sessions` | Sessions with complete verified pricing |
| `totals.incomplete_sessions` | Sessions with missing pricing components or unavailable pricing |
| `totals.unavailable_sessions` | Subset with no known subtotal |
| `totals.pricing_sources` | Counts for catalog, alias, user override, provider report, unavailable, and legacy provenance |
| `totals.cache_hit_ratio` | `cached_input_tokens / input_tokens` |
| `cost_breakdown.cached_input_savings_usd` | Aggregate cached-input savings |
| `by_model[].cache_hit_ratio` | Per-model cache health |

## Context Windows

`ContextWindowSnapshot` stores usable window, used/remaining tokens, remaining percentage, and source. GPT-5.6 has 372,000 raw inventory tokens and 353,400 usable tokens at 95% in Codex 0.144.0. Resolution order is observed JSONL, valid local `models_cache.json`, then the bundled catalog. The runtime does not invent GPT-5.6 API context, long-context thresholds, or max output values when those facts are not published.
