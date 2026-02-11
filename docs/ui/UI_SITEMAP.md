# UI Sitemap

## 1. Smart Foreground Dashboard (`codex-discord-presence`)

Render pipeline is responsive by mode:

- `Full` (wide/tall): full OpenAI + CODEX banner, richer runtime/active context.
- `Compact` (medium): CODEX-first banner fallback and condensed metadata.
- `Minimal` (small): essentials only with compact banner fallback.

Banner selection is now independent from data-density mode and uses this deterministic ladder:

1. `Image` (only when eligible and enough rows exist for image + subtitle lines)
2. `ASCII Dual` (OpenAI + CODEX, full mode only)
3. `ASCII CODEX` (preferred medium/small fallback)
4. `Compact text` (2 lines)
5. `Minimal text` (1 line)

ASCII banners are rendered only when their full block fits, preventing partially clipped wordmarks.

## 2. Layout Budgeting

- Frame body excludes footer row (`height - 1`).
- `Recent Sessions` has reserved rows in every mode:
  - `Full`: 5 rows (header + minimum useful entries).
  - `Compact`: 3 rows.
  - `Minimal`: 1 row.
- Extreme-height exception: when body rows are `<= 12`, reserved rows can drop to `0` to prioritize header + core status.
- Runtime/active sections collapse first when height is constrained.

This keeps `Recent Sessions` visible by default while allowing an explicit extreme-height fallback to prioritize branding and core status.

## 3. Render Order (Top -> Bottom)

1. Banner
- Hybrid behavior:
  - inline image when terminal supports it and config allows it.
  - deterministic ASCII fallback otherwise, with CODEX-only intermediate step.
- High-legibility `CODEX` wordmark, centered subtitle, and no partial ASCII clipping.

2. Runtime
- Mode, current time, uptime, Discord state.
- Client ID + polling/stale details in `Full`/`Compact`.

3. Active Session
- Project, path (full mode), model, branch.
- Activity line (privacy-aware).
- Token triplet (`This update | Last response | Session total`).
- Remaining limit bars (`5h`, `7d`) with semantic color.

4. Recent Sessions
- Always rendered in reserved space.
- Two-line entries when enough space.
- Automatic one-line compact entries in constrained space.

5. Footer
- Bottom-left: quit hint (`Press q or Ctrl+C to quit.`).
- Bottom-right credits:
  - full: `XT0N1.T3CH | Discord @XT0N1.T3CH | ID 211189703641268224`
  - medium: `XT0N1.T3CH | @XT0N1.T3CH`
  - narrow: `XT0N1.T3CH`

## 4. Activity Surface Rules

- Priority labels: `Thinking`, `Reading <target>`, `Editing <target>`, `Running command`, `Waiting for input`, `Idle`.
- Commentary handling:
  - assistant `phase=commentary` is a progress signal and does not overwrite active working labels.
  - commentary can reactivate `Waiting for input` / `Idle` into `Thinking`.
  - assistant `phase=final_answer` maps to `Waiting for input`.
- `Idle` is debounced and shown only when no pending calls and no recent effective activity signal.
- Active session selection favors:
  1. newest recency,
  2. pending calls,
  3. activity class priority:
     - working (`Thinking`, `Reading`, `Editing`, `Running command`)
     - `Waiting for input`
     - `Idle`,
  4. deterministic `session_id` tiebreak.
- Sticky session extension applies to working activity kinds and `Waiting for input`.

## 5. Visual Constraints

- No line overflow by width truncation.
- Footer remains anchored on terminal resize.
- Section spacer rows are preserved in `Full` mode and removed in `Compact`/`Minimal`.
- Progress bars preserve semantic thresholds:
  - green `>= 60%`
  - yellow `>= 30%`
  - red `< 30%`
