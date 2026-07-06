# Terminal UI Sitemap

The foreground terminal is a Ratatui dashboard over the same daemon runtime that publishes Discord Rich Presence.

## Layout Modes

| Mode | Trigger | Shape |
|:---|:---|:---|
| Full | `>=118x32` | Header, active session, quota/context, usage snapshot, spend sparkline, recent sessions |
| Compact | `>=72x18` | Header, active session, quota/context, recent sessions |
| Minimal | Smaller terminals | Header, active session, short recent list |

## Widgets

| Widget | Contract |
|:---|:---|
| Runtime header | Large centered Codex wordmark, local-first subtitle, animated spinner, mode, Discord status, poll cadence |
| Active session | Project, branch, model, reasoning effort, Fast state, cost, token triplet, context use, activity target |
| Quota + context | Primary/secondary gauges, plan label, Fast label, source freshness, OAuth/API context copy |
| Usage snapshot | Total cost, cache hit ratio, cached-input savings, uptime, spend sparkline by model |
| Recent sessions | Responsive list of recent project/model/token summaries |
| Plan picker | Centered selector with Auto Detect plus Free, Go, Plus, Pro 5x, Pro 20x, Business, and Enterprise presets |
| Footer | Author credit plus available keyboard actions; collapses safely on narrow terminals |

## Theme

The theme is Codex dark with a restrained black-and-white terminal palette:

| Role | Use |
|:---|:---|
| White | Codex identity, selected plan, active model, connected state, sparklines |
| Gray | Metadata, inactive states, medium quota, footer text |
| Dark gray | Panel borders and structural separation |

## Motion

Animation is tick-driven by `RenderData.banner_phase`. The frame signature includes `banner_phase`, active session fields, limits, metrics, and plan picker state so the runtime redraws only when visible state changes.

## Logo Policy

`assets/branding/codex-app.png` remains the real Codex App source image for README and Discord preview assets. The terminal header intentionally uses a text wordmark instead of duplicating the app icon, so Codex stays large, centered, and readable on every platform terminal.

## Accessibility + Resilience

- No mouse dependency.
- Compact and minimal modes avoid horizontal overflow.
- Quota colors are paired with text labels and percentages.
- Idle copy explicitly explains sticky Codex App branding.
- If metrics are not ready, the widget renders a warm-up state instead of blank space.
