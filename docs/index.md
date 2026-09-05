# Docs

| Doc | Contract |
|:---|:---|
| [api/codex-presence.md](api/codex-presence.md) | Adaptive provider-neutral usage lanes/streams, canonical `account/rateLimits/read` parsing with separate reset credits, dynamic quota windows, credits precedence, Daybreak/GPT-5.6 pricing and context provenance, session-scoped Fast, exact-cost completeness, Discord/UI policy, and opt-in live Discord wire proof |
| [database/schema.md](database/schema.md) | Schema-12 shared presence control, desktop design, privacy, session speed/surface, cost completeness, metrics, and context provenance |
| [releasing.md](releasing.md) | Exact-SHA operator approval, annotated tags, immutable publication, and cleanup |
| [ui/UI_SITEMAP.md](ui/UI_SITEMAP.md) | Ratatui layout modes, widgets, theme, animation, logo fallback, footer behavior |
| [../assets/branding/README.md](../assets/branding/README.md) | README visual assets, badge crops, section icons, and brand-use notes |

Root references: [README.md](../README.md), [CHANGELOG.md](../CHANGELOG.md), [tests/index.md](../tests/index.md).

## Rules

| Rule | Standard |
|:---|:---|
| Runtime facts | Backed by source or tests |
| External facts | Include source URL and access date |
| Release facts | Match `.github/workflows/release.yml` |
| Scope | Durable contracts only; no plans or handoffs |

## GPT-6 Astra

El catálogo nativo registra Astra con ventana total de 1.050.000 tokens, entrada máxima de 922.000, salida máxima de 128.000 y tarifas Standard/Fast. La telemetría incompleta mantiene costes parciales. Consulte la [ficha oficial](https://developers.openai.com/api/docs/models/gpt-6-astra). El runtime corresponde a v1.11.0.

## Current release v1.11.0

[Windows Efficiency mode](windows-efficiency.md) documents the process policy and read-only validation. See the root README and changelog for the release contract.
