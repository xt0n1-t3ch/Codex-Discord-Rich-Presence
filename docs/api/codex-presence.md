# Codex Presence Runtime API

This document owns the local runtime contract exported by the daemon modules.

## Public Runtime Surfaces

| Module | Contract |
|:---|:---|
| `src/app.rs` | Runs the foreground/daemon loop, detects process surface hints, collects sessions, updates Discord |
| `src/session.rs` | Produces `CodexSessionSnapshot` from Codex JSONL and local OpenCode data |
| `src/cost.rs` | Resolves model pricing, context metadata, Fast multipliers, and token-cost breakdowns |
| `src/metrics.rs` | Aggregates session snapshots into a unified cost/cache/context report |
| `src/discord.rs` | Converts runtime state into Discord IPC activities and Codex asset identities |
| `src/ui.rs` | Renders the Ratatui terminal view from `RenderData` |

## Context Metadata

| Function | Value for GPT-5/Codex family |
|:---|---:|
| `default_model_context_window()` | `400_000` OAuth-visible context |
| `api_model_context_window()` | `1_050_000` API-only context metadata |
| `long_context_input_threshold()` | `272_000` input threshold |
| `max_output_tokens()` | `128_000` max output metadata |

The daemon deliberately keeps Codex/ChatGPT OAuth display at 400K even when API-only models support longer windows.

## Fast Multipliers

| Function | Contract |
|:---|:---|
| `speed_multiplier("gpt-5.5", true)` | `2.5` |
| `speed_multiplier("gpt-5.4", true)` | `2.0` |
| `speed_multiplier(_, false)` | `1.0` |

## Unified Cost Snapshot

`TokenCostBreakdown` now carries:

| Field | Meaning |
|:---|:---|
| `input_cost_usd` | Non-cached input cost |
| `cached_input_cost_usd` | Cached input read cost |
| `output_cost_usd` | Output token cost |
| `cached_input_savings_usd` | Difference between full input price and cached-input price |

`MetricsSnapshot` exposes total cache hit ratio and cached-input savings, plus per-model cache hit ratios.

## Discord Surface Policy

`PresenceSurface::Desktop` publishes `Codex App` and `codex-app`. The detector promotes a session to Desktop when `originator` or `source` looks like Codex Desktop/OpenCode. When the runtime transitions to idle, Discord keeps the last Desktop surface instead of falling back to CLI/VS Code branding.

## Local Files

| File | Purpose |
|:---|:---|
| `~/.codex/discord-presence-config.json` | Runtime config |
| `~/.codex/discord-presence-metrics.json` | Latest metrics snapshot |
| `~/.codex/discord-presence-metrics.md` | Human-readable metrics report |
| `~/.codex/projects/**/*.jsonl` | Codex sessions |
| `~/.local/share/opencode/opencode*.db` | OpenCode-hosted Codex sessions |
