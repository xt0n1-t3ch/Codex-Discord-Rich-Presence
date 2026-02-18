use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use discord_rich_presence::activity::{Activity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use serde::Deserialize;
use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::config::PresenceConfig;
use crate::session::{CodexSessionSnapshot, RateLimits, SessionActivityKind};
use crate::util::{format_cost, format_tokens};

pub struct DiscordPresence {
    client_id: Option<String>,
    client: Option<DiscordIpcClient>,
    last_status: String,
    last_sent: Option<(String, String)>,
    last_publish_at: Option<Instant>,
    known_asset_keys: Option<HashSet<String>>,
    last_asset_refresh_at: Option<Instant>,
    last_heartbeat_at: Option<Instant>,
    reconnect_backoff: Duration,
    last_reconnect_attempt: Option<Instant>,
    consecutive_errors: u32,
    idle_start_epoch: Option<i64>,
}

const DISCORD_MIN_PUBLISH_INTERVAL: Duration = Duration::from_secs(2);
const DISCORD_ASSET_REFRESH_INTERVAL: Duration = Duration::from_secs(300);
const DISCORD_ASSET_FETCH_TIMEOUT: Duration = Duration::from_secs(2);
const DISCORD_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const RECONNECT_MIN_BACKOFF: Duration = Duration::from_secs(5);
const RECONNECT_MAX_BACKOFF: Duration = Duration::from_secs(60);

impl DiscordPresence {
    pub fn new(client_id: Option<String>) -> Self {
        let last_status = if client_id.is_some() {
            "Disconnected".to_string()
        } else {
            "Missing CODEX_DISCORD_CLIENT_ID".to_string()
        };
        Self {
            client_id,
            client: None,
            last_status,
            last_sent: None,
            last_publish_at: None,
            known_asset_keys: None,
            last_asset_refresh_at: None,
            last_heartbeat_at: None,
            reconnect_backoff: RECONNECT_MIN_BACKOFF,
            last_reconnect_attempt: None,
            consecutive_errors: 0,
            idle_start_epoch: None,
        }
    }

    pub fn status(&self) -> &str {
        &self.last_status
    }

    pub fn update(
        &mut self,
        active_session: Option<&CodexSessionSnapshot>,
        effective_limits: Option<&RateLimits>,
        config: &PresenceConfig,
    ) -> Result<()> {
        if self.client_id.is_none() {
            self.last_status = "Missing CODEX_DISCORD_CLIENT_ID".to_string();
            return Ok(());
        }

        if let Err(_err) = self.ensure_connected() {
            return Ok(());
        }
        if self.client.is_none() {
            return Ok(());
        }

        self.refresh_asset_keys_if_needed();
        let needs_heartbeat = self
            .last_heartbeat_at
            .map(|value| value.elapsed() >= DISCORD_HEARTBEAT_INTERVAL)
            .unwrap_or(true);

        match active_session {
            Some(session) => {
                self.idle_start_epoch = None;
                let (details, state) = presence_lines(session, effective_limits, config);
                let payload = (details.clone(), state.clone());

                if self.last_sent.as_ref() == Some(&payload) && !needs_heartbeat {
                    self.last_status = "Connected".to_string();
                    return Ok(());
                }
                if let Some(last_publish) = self.last_publish_at
                    && last_publish.elapsed() < DISCORD_MIN_PUBLISH_INTERVAL
                {
                    self.last_status = "Connected".to_string();
                    return Ok(());
                }

                let (small_image_key, small_text) = small_asset_for_activity(session, config);
                let resolved_large_key = resolve_image_key(
                    &config.display.large_image_key,
                    self.known_asset_keys.as_ref(),
                );
                let resolved_small_key =
                    resolve_image_key(&small_image_key, self.known_asset_keys.as_ref());
                let (large_image_key, small_image_key) =
                    normalize_asset_pair(resolved_large_key, resolved_small_key);

                let activity = build_activity(
                    &details,
                    &state,
                    session,
                    large_image_key.as_deref(),
                    non_empty_trimmed(&config.display.large_text),
                    small_image_key.as_deref(),
                    non_empty_trimmed(&small_text),
                );
                let client = self
                    .client
                    .as_mut()
                    .ok_or_else(|| anyhow!("Discord IPC client unexpectedly missing"))?;
                if let Err(err) = client
                    .set_activity(activity)
                    .context("failed to set Discord activity")
                {
                    self.handle_ipc_error(&err.to_string());
                    return Err(err);
                }

                self.last_sent = Some(payload);
                self.last_publish_at = Some(Instant::now());
                self.last_heartbeat_at = Some(Instant::now());
                self.last_status = "Connected".to_string();
            }
            None => {
                let idle_start = *self
                    .idle_start_epoch
                    .get_or_insert_with(|| Utc::now().timestamp().max(0));

                let details = "Codex CLI".to_string();
                let state = "Waiting for session".to_string();
                let payload = (details.clone(), state.clone());

                if self.last_sent.as_ref() == Some(&payload) && !needs_heartbeat {
                    self.last_status = "Connected (idle)".to_string();
                    return Ok(());
                }
                if let Some(last_publish) = self.last_publish_at
                    && last_publish.elapsed() < DISCORD_MIN_PUBLISH_INTERVAL
                {
                    self.last_status = "Connected (idle)".to_string();
                    return Ok(());
                }

                let resolved_large_key = resolve_image_key(
                    &config.display.large_image_key,
                    self.known_asset_keys.as_ref(),
                );

                let mut activity = Activity::new()
                    .details(&details)
                    .state(&state)
                    .timestamps(Timestamps::new().start(idle_start));

                if let Some(large_key) = resolved_large_key.as_deref() {
                    let mut assets = Assets::new().large_image(large_key);
                    if let Some(text) = non_empty_trimmed(&config.display.large_text) {
                        assets = assets.large_text(text);
                    }
                    activity = activity.assets(assets);
                }

                let client = self
                    .client
                    .as_mut()
                    .ok_or_else(|| anyhow!("Discord IPC client unexpectedly missing"))?;
                if let Err(err) = client
                    .set_activity(activity)
                    .context("failed to set Discord idle activity")
                {
                    self.handle_ipc_error(&err.to_string());
                    return Err(err);
                }

                self.last_sent = Some(payload);
                self.last_publish_at = Some(Instant::now());
                self.last_heartbeat_at = Some(Instant::now());
                self.last_status = "Connected (idle)".to_string();
            }
        }

        Ok(())
    }

    pub fn shutdown(&mut self) {
        let _ = self.clear_activity();
        if let Some(client) = self.client.as_mut() {
            let _ = client.close();
        }
        self.client = None;
        self.last_sent = None;
        self.last_publish_at = None;
        self.last_heartbeat_at = None;
        self.last_asset_refresh_at = None;
        self.idle_start_epoch = None;
        self.reconnect_backoff = RECONNECT_MIN_BACKOFF;
        self.consecutive_errors = 0;
        if self.client_id.is_some() {
            self.last_status = "Disconnected".to_string();
        }
    }

    fn clear_activity(&mut self) -> Result<()> {
        if let Some(client) = self.client.as_mut()
            && let Err(err) = client
                .clear_activity()
                .context("failed to clear Discord activity")
        {
            self.handle_ipc_error(&err.to_string());
            return Err(err);
        }
        Ok(())
    }

    fn ensure_connected(&mut self) -> Result<()> {
        if self.client.is_some() {
            return Ok(());
        }

        let Some(client_id) = self.client_id.clone() else {
            return Ok(());
        };

        if let Some(last_attempt) = self.last_reconnect_attempt
            && last_attempt.elapsed() < self.reconnect_backoff
        {
            return Ok(());
        }

        self.last_reconnect_attempt = Some(Instant::now());
        let mut client = DiscordIpcClient::new(&client_id);
        match client
            .connect()
            .context("failed to connect to Discord IPC (is Discord desktop open?)")
        {
            Ok(()) => {
                self.client = Some(client);
                self.last_sent = None;
                self.last_heartbeat_at = None;
                self.reconnect_backoff = RECONNECT_MIN_BACKOFF;
                self.consecutive_errors = 0;
                self.last_status = "Connected".to_string();
                Ok(())
            }
            Err(err) => {
                self.increase_backoff();
                self.last_status =
                    format!("Reconnecting in {}s...", self.reconnect_backoff.as_secs());
                Err(err)
            }
        }
    }

    fn refresh_asset_keys_if_needed(&mut self) {
        let Some(client_id) = self.client_id.as_deref() else {
            return;
        };
        if let Some(last_refresh) = self.last_asset_refresh_at
            && last_refresh.elapsed() < DISCORD_ASSET_REFRESH_INTERVAL
        {
            return;
        }

        self.last_asset_refresh_at = Some(Instant::now());
        if let Ok(asset_keys) = fetch_discord_asset_keys(client_id) {
            self.known_asset_keys = Some(asset_keys);
        }
    }

    fn handle_ipc_error(&mut self, message: &str) {
        self.client = None;
        self.increase_backoff();
        self.last_status = format!("Discord error: {}", compact_error(message));
    }

    fn increase_backoff(&mut self) {
        self.consecutive_errors = self.consecutive_errors.saturating_add(1);
        let secs = RECONNECT_MIN_BACKOFF
            .as_secs()
            .saturating_mul(1u64 << self.consecutive_errors.min(4));
        self.reconnect_backoff = Duration::from_secs(secs.min(RECONNECT_MAX_BACKOFF.as_secs()));
    }
}

fn compact_error(input: &str) -> String {
    truncate_for_limit(input, 96)
}

fn build_activity<'a>(
    details: &'a str,
    state: &'a str,
    session: &'a CodexSessionSnapshot,
    large_image_key: Option<&'a str>,
    large_text: Option<&'a str>,
    small_image_key: Option<&'a str>,
    small_text: Option<&'a str>,
) -> Activity<'a> {
    let start = session
        .started_at
        .unwrap_or_else(Utc::now)
        .timestamp()
        .max(0);

    let mut activity = Activity::new()
        .details(details)
        .state(state)
        .timestamps(Timestamps::new().start(start));

    let mut assets = Assets::new();
    let mut has_assets = false;

    if let Some(image_key) = large_image_key {
        assets = assets.large_image(image_key);
        has_assets = true;
        if let Some(text) = large_text {
            assets = assets.large_text(text);
        }
    }

    if let Some(image_key) = small_image_key {
        assets = assets.small_image(image_key);
        has_assets = true;
        if let Some(text) = small_text {
            assets = assets.small_text(text);
        }
    }

    if has_assets {
        activity = activity.assets(assets);
    }

    activity
}

fn presence_lines(
    session: &CodexSessionSnapshot,
    effective_limits: Option<&RateLimits>,
    config: &PresenceConfig,
) -> (String, String) {
    if config.privacy.enabled {
        return ("Using Codex".to_string(), "In a coding session".to_string());
    }

    let project_label = if config.privacy.show_project_name {
        if config.privacy.show_git_branch {
            if let Some(branch) = &session.git_branch {
                format!("{} ({branch})", session.project_name)
            } else {
                session.project_name.clone()
            }
        } else {
            session.project_name.clone()
        }
    } else {
        "private project".to_string()
    };

    let details = if config.privacy.show_activity {
        if let Some(activity) = &session.activity {
            format!(
                "{} • {}",
                activity.to_text(config.privacy.show_activity_target),
                project_label
            )
        } else if config.privacy.show_project_name {
            format!("Working on {}", project_label)
        } else {
            "Working in Codex".to_string()
        }
    } else if config.privacy.show_project_name {
        format!("Working on {}", project_label)
    } else {
        "Working in Codex".to_string()
    };

    let limits = effective_limits.unwrap_or(&session.limits);

    let mut state_parts: Vec<String> = Vec::new();
    if config.privacy.show_model
        && let Some(model) = &session.model
    {
        state_parts.push(model.clone());
    }
    if config.privacy.show_tokens {
        state_parts.extend(token_state_parts(session));
    }
    if config.privacy.show_cost && session.total_cost_usd > 0.0 {
        state_parts.push(format!("Cost {}", format_cost(session.total_cost_usd)));
    }
    if config.privacy.show_limits {
        if let Some(primary) = &limits.primary {
            state_parts.push(format!("5h left {:.0}%", primary.remaining_percent));
        }
        if let Some(secondary) = &limits.secondary {
            state_parts.push(format!("7d left {:.0}%", secondary.remaining_percent));
        }
    }

    let fallback = if config.privacy.show_project_name {
        project_label.as_str()
    } else {
        "Codex session"
    };
    let state = compact_join_prioritized(&state_parts, 128, fallback);
    (truncate_for_limit(&details, 128), state)
}

fn token_state_parts(session: &CodexSessionSnapshot) -> Vec<String> {
    let mut parts = Vec::new();

    if session.input_tokens_total > 0 || session.output_tokens_total > 0 {
        parts.push(format!(
            "I {} C {} O {}",
            format_tokens(session.input_tokens_total),
            format_tokens(session.cached_input_tokens_total),
            format_tokens(session.output_tokens_total)
        ));
    }

    if let Some(last) = session.last_turn_tokens {
        parts.push(format!("Last {}", format_tokens(last)));
    }
    if let Some(total) = session.session_total_tokens {
        parts.push(format!("Total {}", format_tokens(total)));
    }
    if parts.is_empty()
        && let Some(delta) = session.session_delta_tokens
    {
        parts.push(format!("Update {}", format_tokens(delta)));
    }

    parts
}

fn small_asset_for_activity(
    session: &CodexSessionSnapshot,
    config: &PresenceConfig,
) -> (String, String) {
    let fallback_key = config.display.small_image_key.clone();
    let fallback_text = config.display.small_text.clone();
    let Some(activity) = &session.activity else {
        return (fallback_key, fallback_text);
    };

    let mapped_key = match activity.kind {
        SessionActivityKind::Thinking => &config.display.activity_small_image_keys.thinking,
        SessionActivityKind::ReadingFile => &config.display.activity_small_image_keys.reading,
        SessionActivityKind::EditingFile => &config.display.activity_small_image_keys.editing,
        SessionActivityKind::RunningCommand => &config.display.activity_small_image_keys.running,
        SessionActivityKind::WaitingInput => &config.display.activity_small_image_keys.waiting,
        SessionActivityKind::Idle => &config.display.activity_small_image_keys.idle,
    }
    .as_ref()
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
    .unwrap_or(fallback_key);

    let mapped_text =
        truncate_for_limit(&activity.to_text(config.privacy.show_activity_target), 128);
    (mapped_key, mapped_text)
}

fn resolve_image_key(
    configured_key: &str,
    known_asset_keys: Option<&HashSet<String>>,
) -> Option<String> {
    let key = configured_key.trim();
    if key.is_empty() {
        return None;
    }
    if looks_like_image_url(key) {
        return Some(key.to_string());
    }
    if let Some(keys) = known_asset_keys {
        return keys.contains(key).then(|| key.to_string());
    }
    Some(key.to_string())
}

fn normalize_asset_pair(
    large_image_key: Option<String>,
    small_image_key: Option<String>,
) -> (Option<String>, Option<String>) {
    if large_image_key.is_none() {
        return (small_image_key, None);
    }

    if large_image_key == small_image_key {
        return (large_image_key, None);
    }

    (large_image_key, small_image_key)
}

fn looks_like_image_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://") || value.starts_with("mp:")
}

fn non_empty_trimmed(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[derive(Deserialize)]
struct DiscordAssetResponse {
    name: String,
}

fn fetch_discord_asset_keys(client_id: &str) -> Result<HashSet<String>> {
    let url = format!("https://discord.com/api/v10/oauth2/applications/{client_id}/assets");
    let agent = ureq::AgentBuilder::new()
        .timeout(DISCORD_ASSET_FETCH_TIMEOUT)
        .build();
    let body = agent
        .get(&url)
        .call()
        .with_context(|| {
            format!(
                "failed to fetch Discord assets for application {}",
                client_id
            )
        })?
        .into_string()
        .context("failed to decode Discord assets response as UTF-8")?;
    parse_discord_asset_keys(&body)
}

fn parse_discord_asset_keys(body: &str) -> Result<HashSet<String>> {
    let parsed: Vec<DiscordAssetResponse> =
        serde_json::from_str(body).context("failed to parse Discord assets response JSON")?;
    Ok(parsed
        .into_iter()
        .map(|asset| asset.name.trim().to_string())
        .filter(|name| !name.is_empty())
        .collect())
}

fn compact_join_prioritized(parts: &[String], max: usize, fallback: &str) -> String {
    let mut out = String::new();
    for part in parts {
        if part.trim().is_empty() {
            continue;
        }

        if out.is_empty() {
            if part.len() <= max {
                out.push_str(part);
            } else {
                out.push_str(&truncate_for_limit(part, max));
            }
            continue;
        }

        if out.len() + 3 + part.len() <= max {
            out.push_str(" | ");
            out.push_str(part);
        }
    }

    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

fn truncate_for_limit(input: &str, max: usize) -> String {
    if input.len() <= max {
        return input.to_string();
    }
    let mut end = max.saturating_sub(3);
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &input[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::PresenceConfig;
    use crate::cost::{PricingSource, TokenCostBreakdown};
    use crate::session::{RateLimits, UsageWindow};
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn sample_session() -> CodexSessionSnapshot {
        CodexSessionSnapshot {
            session_id: "abc".to_string(),
            cwd: PathBuf::from("."),
            project_name: "project-alpha".to_string(),
            git_branch: Some("feature/main".to_string()),
            model: Some("gpt-5.3-codex".to_string()),
            approval_policy: None,
            sandbox_policy: None,
            session_total_tokens: Some(30_000),
            last_turn_tokens: Some(1_700),
            session_delta_tokens: Some(600),
            input_tokens_total: 24_000,
            cached_input_tokens_total: 15_000,
            output_tokens_total: 6_000,
            last_input_tokens: Some(1_500),
            last_cached_input_tokens: Some(900),
            last_output_tokens: Some(200),
            total_cost_usd: 1.234,
            cost_breakdown: TokenCostBreakdown {
                input_cost_usd: 0.5,
                cached_input_cost_usd: 0.2,
                output_cost_usd: 0.534,
            },
            pricing_source: PricingSource::Alias,
            limits: RateLimits {
                primary: Some(UsageWindow {
                    used_percent: 36.0,
                    remaining_percent: 64.0,
                    window_minutes: 300,
                    resets_at: None,
                }),
                secondary: Some(UsageWindow {
                    used_percent: 82.0,
                    remaining_percent: 18.0,
                    window_minutes: 10080,
                    resets_at: None,
                }),
            },
            started_at: None,
            last_token_event_at: None,
            activity: None,
            last_activity: SystemTime::now(),
            source_file: PathBuf::from("session.jsonl"),
        }
    }

    #[test]
    fn state_uses_remaining_limits_and_cost_tokens() {
        let session = sample_session();
        let config = PresenceConfig::default();
        let (_details, state) = presence_lines(&session, Some(&session.limits), &config);
        assert!(state.contains("5h left 64%"));
        assert!(state.contains("7d left 18%"));
        assert!(state.contains("Cost"));
        assert!(state.contains("I 24.0K"));
    }

    #[test]
    fn prioritized_join_truncates_tail() {
        let parts = vec![
            "model".to_string(),
            "token-summary".to_string(),
            "very-long-tail-that-should-not-fit".to_string(),
        ];
        let state = compact_join_prioritized(&parts, 22, "fallback");
        assert_eq!(state, "model | token-summary");
    }

    #[test]
    fn activity_is_prioritized_in_details() {
        let mut session = sample_session();
        session.activity = Some(crate::session::SessionActivitySnapshot {
            kind: crate::session::SessionActivityKind::EditingFile,
            target: Some("main.rs".to_string()),
            observed_at: None,
            last_active_at: None,
            last_effective_signal_at: None,
            idle_candidate_at: None,
            pending_calls: 0,
        });
        let config = PresenceConfig::default();
        let (details, state) = presence_lines(&session, Some(&session.limits), &config);
        assert!(details.starts_with("Editing"));
        assert!(details.contains("project-alpha"));
        assert!(state.contains("gpt-5.3-codex"));
    }

    #[test]
    fn small_asset_falls_back_to_default_when_activity_key_is_missing() {
        let session = sample_session();
        let config = PresenceConfig::default();
        let (key, text) = small_asset_for_activity(&session, &config);
        assert_eq!(key, config.display.small_image_key);
        assert_eq!(text, config.display.small_text);
    }

    #[test]
    fn small_asset_uses_activity_mapping_when_configured() {
        let mut session = sample_session();
        session.activity = Some(crate::session::SessionActivitySnapshot {
            kind: crate::session::SessionActivityKind::Thinking,
            target: None,
            observed_at: None,
            last_active_at: None,
            last_effective_signal_at: None,
            idle_candidate_at: None,
            pending_calls: 0,
        });
        let mut config = PresenceConfig::default();
        config.display.activity_small_image_keys.thinking = Some("thinking-icon".to_string());
        let (key, text) = small_asset_for_activity(&session, &config);
        assert_eq!(key, "thinking-icon");
        assert_eq!(text, "Thinking");
    }

    #[test]
    fn invalid_asset_key_is_removed_when_catalog_is_known() {
        let key = resolve_image_key("missing-key", Some(&HashSet::new()));
        assert_eq!(key, None);
    }

    #[test]
    fn https_image_url_is_accepted_without_asset_catalog() {
        let key = resolve_image_key("https://example.com/logo.png", Some(&HashSet::new()));
        assert_eq!(key.as_deref(), Some("https://example.com/logo.png"));
    }

    #[test]
    fn normalize_asset_pair_promotes_small_when_large_is_missing() {
        let (large, small) = normalize_asset_pair(None, Some("openai".to_string()));
        assert_eq!(large.as_deref(), Some("openai"));
        assert_eq!(small, None);
    }

    #[test]
    fn parse_discord_asset_keys_reads_names() {
        let json = r#"
            [
                {"id":"1","name":"codex-logo","type":1},
                {"id":"2","name":"openai","type":1}
            ]
        "#;
        let keys = parse_discord_asset_keys(json).expect("keys");
        assert!(keys.contains("codex-logo"));
        assert!(keys.contains("openai"));
    }
}
