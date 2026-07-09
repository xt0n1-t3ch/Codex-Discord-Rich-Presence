# Codex Presence Runtime API

This document owns the local runtime contract exported by the daemon modules.

## Public Runtime Surfaces

| Module | Contract |
|:---|:---|
| `src/model.rs` | Resolves the bundled model catalog, aliases, display labels, effort/speed capabilities, context provenance, and verified rates |
| `src/session.rs` | Produces `CodexSessionSnapshot` from Codex JSONL and local OpenCode data |
| `src/cost.rs` | Applies local overrides and computes exact, partial, or unavailable token costs |
| `src/metrics.rs` | Aggregates session snapshots into a unified cost/cache/context report |
| `src/discord.rs` | Converts runtime state into Discord IPC activities and Codex asset identities |
| `src/ui.rs` | Renders the Ratatui terminal view from `RenderData` |

`src/model_catalog.json` is the single machine-readable owner for bundled model facts. It includes source URLs and a verification date. Consumers must use the exported model API instead of rebuilding model names, capabilities, or rates.

## GPT-5.6 Contract

| ID | Codex App label | Effective context | Ultra | Fast |
|:---|:---|---:|:---:|:---:|
| `gpt-5.6`, `gpt-5.6-sol` | `5.6 Sol` | `353_400` | Yes | Yes |
| `gpt-5.6-terra` | `5.6 Terra` | `353_400` | Yes | Yes |
| `gpt-5.6-luna` | `5.6 Luna` | `353_400` | No | Yes |

The raw context is `372_000`; Codex App exposes 95% as usable context. Context resolution order is observed JSONL `model_context_window`, valid local `~/.codex/models_cache.json`, then the bundled catalog. The local cache reader is size- and count-bounded and falls back closed on malformed or implausible data.

No public GPT-5.6 API context, max-output, long-context surcharge threshold, cache-write credit rate, or Fast usage multiplier was verified on 2026-07-09. Those fields remain absent rather than inheriting older GPT-5 constants.

## Presentation

`ReasoningEffort` is owned by `model` and accepts `low`, `medium`, `high`, `xhigh`, `max`, and `ultra`. The `low` display label is `Light`. Unsupported model/effort combinations are omitted from display.

Fast is session-scoped. JSONL `thread_settings_applied.thread_settings.service_tier=priority` produces a `-fast` session model only when that model declares Fast support. Presentation examples:

- `5.6 Sol Max`
- `5.6 Sol Max · Fast`
- `5.6 Terra Light`

## Pricing

API rates per one million tokens, verified 2026-07-09:

| Model | Input | Cache write | Cache read | Output |
|:---|---:|---:|---:|---:|
| Sol | `$5.00` | `$6.25` | `$0.50` | `$30.00` |
| Terra | `$2.50` | `$3.125` | `$0.25` | `$15.00` |
| Luna | `$1.00` | `$1.25` | `$0.10` | `$6.00` |

`compute_cost()` takes `TokenUsage`, clamps cache reads to total input, and returns `PricingStatus`:

| Status | Meaning |
|:---|:---|
| `exact` | Every published price component has observed token telemetry |
| `partial` | The known subtotal excludes a published component absent from telemetry, currently GPT-5.6 cache writes in Codex JSONL |
| `unavailable` | No verified pricing or valid user override exists |

Unknown models never inherit a fallback rate. Discord and the terminal render partial subtotals with a `>=` prefix and hide unavailable costs. OpenCode can produce an exact GPT-5.6 total because its database reports cache-write tokens separately.

## Prompt Cache Policy

The bundled policy records a 1,024-token eligibility minimum and a 30-minute minimum lifetime. It is metadata for analysis; the daemon does not infer unobserved cache writes.

## Sources

| Fact | Source | Verified |
|:---|:---|:---|
| Family IDs and alias | <https://developers.openai.com/api/docs/guides/latest-model.md> | 2026-07-09 |
| API rates | <https://openai.com/index/previewing-gpt-5-6-sol/> | 2026-07-09 |
| Prompt caching | <https://developers.openai.com/api/docs/guides/prompt-caching> | 2026-07-09 |
| Codex credit rates | <https://help.openai.com/en/articles/20001106-codex-rate-card-2> | 2026-07-09 |
| App capabilities/context | Local Codex 0.144.0 `models_cache.json` | 2026-07-09 |

## Local Files

| File | Purpose |
|:---|:---|
| `~/.codex/models_cache.json` | Current Codex App model context metadata |
| `~/.codex/discord-presence-config.json` | Runtime config and pricing overrides |
| `~/.codex/discord-presence-metrics.json` | Latest metrics snapshot |
| `~/.codex/discord-presence-metrics.md` | Human-readable metrics report |
| `~/.codex/sessions/**/*.jsonl` | Codex sessions |
| `~/.local/share/opencode/opencode*.db` | OpenCode-hosted Codex sessions |
