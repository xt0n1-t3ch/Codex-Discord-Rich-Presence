# Codex Presence Runtime API

This document owns the local runtime contract exported by the daemon modules.

## Public Runtime Surfaces

| Module | Contract |
|:---|:---|
| `src/model.rs` | Resolves the bundled model catalog, aliases, display labels, effort/speed capabilities, context provenance, and verified rates |
| `src/session.rs` | Produces `CodexSessionSnapshot` from Codex JSONL and local OpenCode data |
| `src/cost.rs` | Applies local overrides and computes exact, partial, or unavailable token costs |
| `src/metrics.rs` | Aggregates session snapshots into a unified cost/cache/context report |
| `src/config.rs` | Owns schema-13 persistence, last-good runtime reload, the master presence switch, ordered composer, design, privacy, plan, and pricing overrides |
| `crates/codex-presence-core` | Owns semantic quota/credit telemetry plus deterministic two-line Discord composition without UI dependencies |
| `src/discord.rs` | Converts runtime state into Discord IPC activities and Codex asset identities, including idempotent pause/resume transitions |
| `src/ui.rs` | Renders the Ratatui terminal view from `RenderData` |

`src/model_catalog.json` is the single machine-readable owner for bundled model facts. It includes source URLs and a verification date. Consumers must use the exported model API instead of rebuilding model names, capabilities, or rates.

`discord::active_presence_presentation` and `idle_presence_presentation` own the public activity title, details, state, large asset, and optional small system signal. Discord IPC, previews, and vendored consumers must use this contract instead of rebuilding presentation strings.

## Opt-in live Discord proof

`discord-proof` is a diagnostic-only backend command. With Discord desktop
already running, it publishes one activity through the same surface selection,
asset resolution, and `DiscordIpcClient` path as the daemon, waits for the
`SET_ACTIVITY` acknowledgement, clears the activity, waits for the clear
acknowledgement, then emits a sanitized JSON artifact:

```text
codex-discord-presence discord-proof --output proof.json
```

The artifact records the exact outbound `SET_ACTIVITY` DTO, both accepted
replies, the selected public surface, and cost assertions. Exact pricing is
emitted only when `PricingStatus::Exact`; partial or unavailable subtotals are
omitted. The proof fails closed if the public state contains `>=`, if an exact
cost label is missing, or if a partial/unavailable subtotal leaks into state.
The command does not acquire the daemon single-instance lock and does not stop
Discord, Pulse, or any existing presence process.

## Adaptive Usage Contract

`codex-presence-core::usage` is the single owner for provider-neutral usage
data. `UsageLane` is one of `codex_subscription`, `open_ai_api`,
`claude_subscription`, `anthropic_api`, or `unknown`. A lane is selected only
from explicit `UsageSignal` evidence: a session JSONL marker by itself stays
`unknown`, and evidence from more than one provider lane is ambiguous and
stays fail-closed.

`UsageSource` carries the selected lane, a stable non-secret `stream_id`, and
the sorted/deduplicated signals. `UsageStream` keeps that source with its
envelopes, while `UsageSnapshotCollection` keeps each stream's snapshot
separate. Selection matches the requested `lane + stream_id` exactly; it never
falls back to another provider, account, or stream. The upstream runtime
continues to collect only local Codex session JSONL; no Claude or Anthropic
collector is enabled by this contract.

`RateLimits.windows` is an ordered dynamic list. The legacy primary/secondary
wire shape is accepted on read and normalized to this list, but new JSON emits
only `windows`. Labels are derived from each window's duration, so a missing
window is never fabricated. Within one stream, `GlobalAccount` credits take
precedence over individual/model credits. Equal-rank conflicting envelopes,
unknown lanes, and ambiguous sources return no effective selection.

The authenticated Codex app-server `account/rateLimits/read` response is parsed
by `parse_account_rate_limits_response` in the same core crate. The
`rateLimits` snapshot (including `rateLimitsByLimitId`) becomes `envelopes`,
while `result.rateLimitResetCredits` remains a separate
`RateLimitResetCreditsSummary`. Its `available_count` is authoritative; the
optional `credits` list preserves the protocol distinction between `null`
(only the count is known) and an empty list (details were fetched and none are
available). A Discord consumer may show only the count when it is positive and
must omit the field for a missing or zero summary; it must not infer access from
expiry, title, description, or reset type.

The wire contract is tracked against the official
[`RateLimitResetCreditsSummary`](https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitResetCreditsSummary.ts),
[`RateLimitResetCredit`](https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitResetCredit.ts),
[`RateLimitResetType`](https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitResetType.ts),
and
[`RateLimitResetCreditStatus`](https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitResetCreditStatus.ts)
definitions (verified 2026-08-02).

## GPT-5.6 Contract

| ID | Codex App label | Effective context | Ultra | Fast |
|:---|:---|---:|:---:|:---:|
| `gpt-5.6`, `gpt-5.6-sol` | `5.6 Sol` | `353_400` | Yes | Yes |
| `gpt-5.6-terra` | `5.6 Terra` | `353_400` | Yes | Yes |
| `gpt-5.6-luna` | `5.6 Luna` | `353_400` | No | Yes |

The raw context is `372_000`; Codex App exposes 95% as usable context. Context resolution order is observed JSONL `model_context_window`, valid local `~/.codex/models_cache.json`, then the bundled catalog. Snapshots preserve `raw_window_tokens`, usable `window_tokens`, `effective_percent`, the selected `source`, and `raw_source` separately. The local cache reader is size- and count-bounded and falls back closed on malformed or implausible data.

No public GPT-5.6 API context, max-output, long-context surcharge threshold, cache-write credit rate, or Fast usage multiplier was verified on 2026-07-09. Those fields remain absent rather than inheriting older GPT-5 constants.

## Presentation

`ReasoningEffort` is owned by `model` and accepts `low`, `medium`, `high`, `xhigh`, `max`, and `ultra`. The `low` display label is `Light`. Unsupported model/effort combinations are omitted from display.

Fast is session-scoped and stored independently from the canonical model id. JSONL `thread_settings_applied.thread_settings.service_tier=priority` sets `SessionSpeed::Fast` only when that model declares Fast support. Later turn-context records without a speed signal do not overwrite it. Presentation examples:

- `GPT-5.6 Sol · Max`
- `GPT-5.6 Sol · Max · ⚡ Fast`
- `GPT-5.6 Terra · Light`

`gpt-5.6` is an alias of Sol. No `gpt-5.6-pro` model is invented; Pro remains a plan/reasoning concept outside the model family.

## Surface Identity

Session metadata is authoritative. `Codex Desktop` and OpenCode map to desktop, `codex_vscode` maps to `Codex VS Code Extension`, and `codex-tui` maps to `Codex CLI`. When metadata is absent, the runtime requires an extension-host process, an OpenCode marker, or the explicit `CODEX_PRESENCE_SURFACE=cli|vscode|desktop` override; generic VS Code terminal variables and unrelated open apps never change the identity.

Config schema 13 stores the shared `presence_enabled` master switch, `display.desktop_presence_design`, and the ordered ten-field composer:

| Value | Desktop label | Discord client id |
|:---|:---|:---|
| `codex_app` | `Codex App` | `1478395304624652345` |
| `chat_gpt_app` | `ChatGPT App` | `1470480085453770854` |

CLI and VS Code always use the shared `1470480085453770854` identity. Pressing `D` in Ratatui toggles and saves the desktop value; Discord reconnects when the selected client id changes.

Foreground TUI, headless, and Codex-wrapper loops reload `~/.codex/discord-presence-config.json` before every poll. Valid external edits from Pulse replace the complete runtime config together. Invalid, missing, or transiently replaced files are logged and ignored so the process keeps its last valid configuration.

`presence_enabled` defaults to `true` during schema-11 migration. Pressing `M` in Ratatui or changing the same field in Pulse clears Discord activity once and reports `Paused`; session parsing, metrics, and the terminal dashboard continue locally. Re-enabling forces a fresh publish from the current session.

## Privacy Fields

Press `V` in Ratatui to edit the ten persisted Discord fields: project name, Git branch, model, activity, token count, cost, semantic quotas, Credits, context usage, and systems. `Shift+↑/↓` reorders the selected field. Context is independent from token count. Systems controls the small activity asset and its tooltip. Every edit is saved atomically and triggers a fresh public presentation before Discord publication.

Quota labels come only from each dynamic window's `window_minutes` value; an absent or malformed duration is omitted and no positional window is fabricated. Credits preserve the received decimal text; explicit zero and unlimited are displayable, while an absent or malformed object remains unavailable.

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

Unknown models never inherit a fallback rate. Discord and the terminal omit partial subtotals and hide unavailable costs. OpenCode can produce an exact GPT-5.6 total because its database reports cache-write tokens separately.

Because Codex JSONL exposes cumulative session totals rather than the size of every billed prompt, a GPT-5.5 or GPT-5.4 session whose cumulative input exceeds the published 272K long-context threshold keeps a partial lower-bound subtotal internally, but that subtotal is omitted from public presence. GPT-5.3 Codex Spark remains unavailable instead of inheriting GPT-5.3 Codex rates because its current Codex credit rates are explicitly non-final.

## Prompt Cache Policy

The bundled policy records a 1,024-token eligibility minimum and a 30-minute minimum lifetime. It is metadata for analysis; the daemon does not infer unobserved cache writes.

## Sources

| Fact | Source | Verified |
|:---|:---|:---|
| Family IDs and alias | <https://developers.openai.com/api/docs/guides/latest-model> | 2026-07-09 |
| API rates | <https://openai.com/index/previewing-gpt-5-6-sol/> | 2026-07-09 |
| Current model rates and API windows | <https://developers.openai.com/api/docs/models> | 2026-07-09 |
| Prompt caching | <https://developers.openai.com/api/docs/guides/prompt-caching> | 2026-07-09 |
| Codex credit rates | <https://help.openai.com/en/articles/20001106-codex-rate-card-2> | 2026-07-09 |
| App-server reset-credit shape | <https://raw.githubusercontent.com/openai/codex/main/codex-rs/app-server-protocol/schema/typescript/v2/RateLimitResetCreditsSummary.ts> and related v2 types | 2026-08-02 |
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
