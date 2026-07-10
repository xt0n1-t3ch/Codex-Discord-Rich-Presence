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
| Runtime header | Large centered Codex wordmark, local-first subtitle, animated spinner, mode, Discord status, poll cadence, selected desktop design |
| Active session | Project, branch, model, reasoning effort, Fast state, cost, token triplet, usable/raw context, activity target |
| Quota + context | Primary/secondary gauges, plan label, Fast label, source freshness, OAuth/API context copy |
| Usage snapshot | Total cost, cache hit ratio, cached-input savings, uptime, spend sparkline by model |
| Recent sessions | Responsive list of recent project/model/token summaries |
| Plan picker | Centered selector with Auto Detect plus Free, Go, Plus, Pro 5x, Pro 20x, Business, and Enterprise presets |
| Master presence | `M` immediately persists pause/resume; paused mode clears Discord once while local monitoring stays active |
| Desktop design | `D` immediately toggles and persists `Codex App` / `ChatGPT App`; the next publish reconnects to the matching Discord application |
| Footer | Author credit plus `M`, `V`, `P`, `D`, and quit actions with the current presence state; collapses safely on narrow terminals |

## Theme

The theme is Codex dark with a restrained black-and-white terminal palette:

| Role | Use |
|:---|:---|
| White | Codex identity, selected plan, active model, connected state, sparklines |
| Gray | Metadata, inactive states, medium quota, footer text |
| Dark gray | Panel borders and structural separation |

## Motion

Animation is tick-driven by `RenderData.banner_phase`. The frame signature includes `banner_phase`, presence enabled/paused state, desktop design, active model/effort/speed, limits, metrics, and picker state so the runtime redraws only when visible state changes.

## Logo Policy

`assets/branding/codex-app.png` and `assets/branding/chatgpt-app.jpg` are the canonical desktop preview sources. The terminal header intentionally uses a text wordmark instead of duplicating either app icon, so the UI stays readable and the selected design remains a semantic status label.

## Accessibility + Resilience

- No mouse dependency.
- Compact and minimal modes avoid horizontal overflow.
- Quota colors are paired with text labels and percentages.
- Idle copy reports the selected desktop design without collapsing CLI and VS Code into one label.
- Paused copy states that Discord publication is off while local monitoring remains active.
- If metrics are not ready, the widget renders a warm-up state instead of blank space.
