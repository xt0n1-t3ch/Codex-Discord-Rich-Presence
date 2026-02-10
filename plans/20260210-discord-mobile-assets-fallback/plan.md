# Plan: Discord Mobile Asset Reliability + Missing-Asset Fallback

## 1) Discovery & Context
- Discord desktop was rendering images while Discord mobile sometimes showed `?` placeholder.
- Configured app ID (`1470480085453770854`) currently returned an empty asset catalog from:
  - `https://discord.com/api/v10/oauth2/applications/{app_id}/assets`
- Current payload always sent `large_image`/`small_image` keys without validation.
- Invalid keys in payload are the direct trigger for missing-image placeholders on mobile.

## 2) Scope & Non-Goals
### Scope
- Validate configured image keys against Discord app asset catalog.
- Keep support for direct `https://` image URLs.
- Omit invalid keys from payload to prevent mobile `?` icons.
- Document new runtime behavior and setup guidance.

### Non-Goals
- No breaking config schema changes.
- No CLI command changes.
- No GUI/terminal redesign in this pass.

## 3) Architecture & Data Flow
1. `DiscordPresence` refreshes known asset keys on interval (5 min) using app ID.
2. On each presence publish:
   - resolve configured large/small keys,
   - treat `https://` / `http://` / `mp:` values as valid URLs,
   - if key is missing from known catalog, drop it.
3. Build activity payload with assets only when at least one valid image remains.
4. If no valid images remain, send payload without image keys (Discord falls back to app icon instead of `?`).

## 4) Interfaces & Schemas
- No schema bump (`schema_version` remains `3`).
- Existing fields unchanged:
  - `display.large_image_key`
  - `display.small_image_key`
  - `display.activity_small_image_keys.*`
- Behavioral contract update:
  - image keys may be Discord asset keys or direct `https://` URLs.
  - invalid configured keys are automatically omitted at runtime.

## 5) Implementation Phases
1. Extend `src/discord.rs` with:
   - periodic asset-catalog fetch,
   - key resolution helpers,
   - conditional asset payload builder.
2. Add tests for:
   - invalid key removal,
   - URL acceptance,
   - key-pair normalization,
   - catalog JSON parse.
3. Update docs:
   - `README.md`
   - `docs/api/codex-presence.md`
   - `docs/database/schema.md`

## 6) Validation & Acceptance
### Automated
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`
- `cargo build --release`

### Acceptance
- With missing/invalid asset keys, mobile no longer shows `?`; presence falls back to app icon.
- With valid keys or URL images, both desktop and mobile render images.
- No regressions in presence updates, rate-limiting, or dedupe behavior.

## 7) Rollout, Risks & Backout
### Rollout
- Ship as patch release with updated docs.

### Risks
- Asset catalog request can fail transiently (network).
- URL images may be blocked/slow for some hosts.

### Mitigation
- Keep last known successful catalog.
- Treat unknown catalog as non-blocking fallback path.
- Use short fetch timeout.

### Backout
- Revert Discord asset validation commit if unexpected regressions appear.
