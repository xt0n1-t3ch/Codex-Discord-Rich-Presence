# UI Sitemap

## 1. Smart Foreground Dashboard (`codex-discord-presence`)

Render pipeline is height-budgeted and responsive by mode:

- `Full` (wide/tall terminals): full OpenAI+CODEX banner, richer runtime/active details.
- `Compact` (medium terminals): condensed banner and metadata.
- `Minimal` (small terminals): essentials only, no overflow.

### Render order (top -> bottom)

1. Banner
- Hybrid logo behavior:
  - official OpenAI image when terminal protocol supports inline images and config allows it.
  - deterministic ASCII fallback otherwise.
- Wordmark/subtitle are centered and constrained by terminal width.

2. Runtime
- Mode, time, uptime, Discord state.
- Client ID + polling/stale data in `Full`/`Compact`.
- Limits semantic label in `Full`.

3. Active Session
- Project, model, branch (+ path in `Full`).
- Live activity (privacy-aware): `Thinking`, `Reading <file>`, `Editing <file>`, `Running command`, `Waiting for input`, `Idle`.
- Token summary with natural labels:
  - `Tokens: This update X | Last response Y | Session total Z`
- Colored remaining bars.
- Color thresholds:
  - green `>=60%`
  - yellow `>=30%`
  - red `<30%`

4. Recent Sessions
- Session rows are trimmed to remaining vertical budget.
- `Full`/`Compact`: header row + detail row.
- `Minimal`: header row only.

5. Footer (fixed bottom anchor)
- Always rendered in the final terminal rows (non-flow).
- Responsive credit line:
  - full: `By XT0N1.T3CH | Discord @XT0N1.T3CH | ID 211189703641268224`
  - medium: `By XT0N1.T3CH | @XT0N1.T3CH`
  - narrow: `By XT0N1.T3CH`
- Quit hint (`q` / `Ctrl+C`) always on last line.

## 2. Status Snapshot (`codex-discord-presence status`)

One-shot textual output:

- running state (+ PID when available)
- config/sessions paths
- active session summary
- activity summary (when enabled)
- token summary and remaining limits
- limits source session ID (when available)

## 3. Doctor (`codex-discord-presence doctor`)

Diagnostics view:

- Codex sessions path existence
- Discord client ID configuration
- `codex` command availability
- `git` command availability

## 4. Non-TTY Bootstrap

If launched without interactive TTY:

1. Attempt terminal relaunch (platform-native).
2. If relaunch fails, continue headless foreground mode.
