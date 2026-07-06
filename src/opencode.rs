use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde_json::Value;

use crate::config::{self, PricingConfig};
use crate::cost::{compute_total_cost, default_model_context_window};
use crate::session::{
    CodexSessionSnapshot, ContextWindowSnapshot, ContextWindowSource, RateLimits,
    SessionActivityKind, SessionActivitySnapshot,
};
use crate::util::truncate;

#[derive(Debug)]
struct OpenCodeSessionRow {
    id: String,
    directory: String,
    title: String,
    agent: Option<String>,
    model: Option<String>,
    cost: f64,
    tokens_input: u64,
    tokens_output: u64,
    tokens_reasoning: u64,
    tokens_cache_read: u64,
    tokens_cache_write: u64,
    time_created: i64,
    time_updated: i64,
}

pub fn collect_opencode_sessions(
    stale_threshold: Duration,
    active_sticky_window: Duration,
    pricing_config: &PricingConfig,
) -> Vec<CodexSessionSnapshot> {
    opencode_database_paths()
        .into_iter()
        .filter_map(|path| {
            collect_from_database(&path, stale_threshold, active_sticky_window, pricing_config).ok()
        })
        .flatten()
        .collect()
}

fn opencode_database_paths() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(
        config::codex_home()
            .join("..")
            .join(".local")
            .join("share")
            .join("opencode"),
    );
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".local").join("share").join("opencode"));
    }
    database_paths_from_data_dirs(dirs)
}

fn database_paths_from_data_dirs(dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for dir in dirs {
        paths.push(dir.join("opencode.db"));
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut channel_paths: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| is_opencode_database_path(path))
            .collect();
        channel_paths.sort();
        paths.extend(channel_paths);
    }
    dedupe_existing_paths(paths)
}

fn is_opencode_database_path(path: &Path) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some("db")
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name == "opencode.db" || name.starts_with("opencode-"))
}

fn dedupe_existing_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        let key = path
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if path.exists() && seen.insert(key) {
            out.push(path);
        }
    }
    out
}

fn collect_from_database(
    db_path: &Path,
    stale_threshold: Duration,
    active_sticky_window: Duration,
    pricing_config: &PricingConfig,
) -> rusqlite::Result<Vec<CodexSessionSnapshot>> {
    let connection =
        Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(
        "select id, directory, title, agent, model, cost, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, time_created, time_updated from session order by time_updated desc limit 32",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(OpenCodeSessionRow {
            id: row.get(0)?,
            directory: row.get(1)?,
            title: row.get(2)?,
            agent: row.get(3)?,
            model: row.get(4)?,
            cost: row.get(5)?,
            tokens_input: row.get::<_, i64>(6)?.max(0) as u64,
            tokens_output: row.get::<_, i64>(7)?.max(0) as u64,
            tokens_reasoning: row.get::<_, i64>(8)?.max(0) as u64,
            tokens_cache_read: row.get::<_, i64>(9)?.max(0) as u64,
            tokens_cache_write: row.get::<_, i64>(10)?.max(0) as u64,
            time_created: row.get(11)?,
            time_updated: row.get(12)?,
        })
    })?;

    let now = SystemTime::now();
    let mut snapshots = Vec::new();
    for row in rows.flatten() {
        let Some(snapshot) = row_to_snapshot(
            &connection,
            db_path,
            row,
            now,
            stale_threshold,
            active_sticky_window,
            pricing_config,
        )?
        else {
            continue;
        };
        snapshots.push(snapshot);
    }
    Ok(snapshots)
}

fn row_to_snapshot(
    connection: &Connection,
    db_path: &Path,
    row: OpenCodeSessionRow,
    now: SystemTime,
    stale_threshold: Duration,
    active_sticky_window: Duration,
    pricing_config: &PricingConfig,
) -> rusqlite::Result<Option<CodexSessionSnapshot>> {
    let updated_at = millis_to_system_time(row.time_updated).unwrap_or(now);
    if now.duration_since(updated_at).unwrap_or_default()
        > active_sticky_window.max(stale_threshold)
    {
        return Ok(None);
    }
    let Some(model) = parse_model_id(row.model.as_deref()) else {
        return Ok(None);
    };
    if !model.to_ascii_lowercase().starts_with("gpt-") {
        return Ok(None);
    }

    let activity = latest_activity(connection, &row.id)?;
    let context_window = latest_context_window(connection, &row.id, &model)?;
    let input_total = row
        .tokens_input
        .saturating_add(row.tokens_cache_read)
        .saturating_add(row.tokens_cache_write);
    let output_total = row.tokens_output.saturating_add(row.tokens_reasoning);
    let token_total = input_total.saturating_add(output_total);
    let computed = compute_total_cost(
        &model,
        input_total,
        row.tokens_cache_read,
        output_total,
        pricing_config,
    );
    let total_cost_usd = if row.cost.is_finite() && row.cost > 0.0 {
        row.cost
    } else {
        computed.total_cost_usd
    };

    Ok(Some(CodexSessionSnapshot {
        session_id: format!("opencode:{}", row.id),
        cwd: PathBuf::from(&row.directory),
        project_name: project_name(&row.directory, &row.title),
        git_branch: None,
        originator: Some("opencode".to_string()),
        source: Some(row.agent.unwrap_or_else(|| "opencode".to_string())),
        model: Some(model),
        reasoning_effort: None,
        approval_policy: None,
        sandbox_policy: None,
        session_total_tokens: (token_total > 0).then_some(token_total),
        last_turn_tokens: None,
        session_delta_tokens: None,
        input_tokens_total: input_total,
        cached_input_tokens_total: row.tokens_cache_read,
        output_tokens_total: output_total,
        last_input_tokens: None,
        last_cached_input_tokens: None,
        last_output_tokens: None,
        total_cost_usd,
        cost_breakdown: computed.breakdown,
        pricing_source: computed.source,
        context_window,
        limits: RateLimits::default(),
        rate_limit_envelopes: Vec::new(),
        activity,
        started_at: millis_to_datetime(row.time_created),
        last_token_event_at: millis_to_datetime(row.time_updated),
        last_activity: updated_at,
        source_file: db_path.to_path_buf(),
    }))
}

fn latest_context_window(
    connection: &Connection,
    session_id: &str,
    model: &str,
) -> rusqlite::Result<Option<ContextWindowSnapshot>> {
    let mut statement = connection.prepare(
        "select data from part where session_id = ?1 order by time_updated desc limit 64",
    )?;
    let rows = statement.query_map(params![session_id], |row| row.get::<_, String>(0))?;

    for row in rows.flatten() {
        let Some(context) = context_window_from_part(&row, model) else {
            continue;
        };
        return Ok(Some(context));
    }
    Ok(None)
}

fn context_window_from_part(data: &str, model: &str) -> Option<ContextWindowSnapshot> {
    let parsed = serde_json::from_str::<Value>(data).ok()?;
    if parsed.get("type").and_then(Value::as_str) != Some("step-finish") {
        return None;
    }
    let used_tokens = step_finish_token_total(parsed.get("tokens")?)?;
    let window_tokens = default_model_context_window(model)?;
    if window_tokens == 0 {
        return None;
    }
    let used_tokens = used_tokens.min(window_tokens);
    let remaining_tokens = window_tokens.saturating_sub(used_tokens);
    let remaining_percent =
        ((remaining_tokens as f64 / window_tokens as f64) * 100.0).clamp(0.0, 100.0);
    Some(ContextWindowSnapshot {
        window_tokens,
        used_tokens,
        remaining_tokens,
        remaining_percent,
        source: ContextWindowSource::Catalog,
    })
}

fn step_finish_token_total(tokens: &Value) -> Option<u64> {
    if let Some(total) = uint_value(tokens.get("total")) {
        return (total > 0).then_some(total);
    }
    let total = uint_value(tokens.get("input"))
        .unwrap_or(0)
        .saturating_add(uint_value(tokens.get("output")).unwrap_or(0))
        .saturating_add(uint_value(tokens.get("reasoning")).unwrap_or(0))
        .saturating_add(uint_value(tokens.pointer("/cache/read")).unwrap_or(0))
        .saturating_add(uint_value(tokens.pointer("/cache/write")).unwrap_or(0));
    (total > 0).then_some(total)
}

fn uint_value(value: Option<&Value>) -> Option<u64> {
    match value? {
        Value::Number(number) => number.as_u64(),
        Value::String(raw) => raw.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn parse_model_id(raw: Option<&str>) -> Option<String> {
    let value = raw?.trim();
    if value.is_empty() {
        return None;
    }
    if let Ok(json) = serde_json::from_str::<Value>(value)
        && let Some(id) = json.get("id").and_then(Value::as_str)
    {
        return Some(id.trim().to_ascii_lowercase());
    }
    Some(value.to_ascii_lowercase())
}

fn latest_activity(
    connection: &Connection,
    session_id: &str,
) -> rusqlite::Result<Option<SessionActivitySnapshot>> {
    let mut statement = connection.prepare(
        "select data, time_created from part where session_id = ?1 order by time_updated desc limit 24",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for row in rows.flatten() {
        let Some(activity) = activity_from_part(&row.0, row.1) else {
            continue;
        };
        return Ok(Some(activity));
    }
    Ok(None)
}

fn activity_from_part(data: &str, time_created: i64) -> Option<SessionActivitySnapshot> {
    let parsed = serde_json::from_str::<Value>(data).ok()?;
    let observed_at = millis_to_datetime(time_created);
    let part_type = parsed
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match part_type {
        "tool" => tool_activity(&parsed, observed_at),
        "reasoning" | "step-start" => Some(activity(
            SessionActivityKind::Thinking,
            None,
            observed_at,
            0,
        )),
        "text" => Some(activity(
            SessionActivityKind::WaitingInput,
            None,
            observed_at,
            0,
        )),
        _ => None,
    }
}

fn tool_activity(
    parsed: &Value,
    observed_at: Option<DateTime<Utc>>,
) -> Option<SessionActivitySnapshot> {
    let tool = parsed.get("tool").and_then(Value::as_str).unwrap_or("tool");
    let input = parsed.get("state").and_then(|state| state.get("input"));
    let status = parsed
        .get("state")
        .and_then(|state| state.get("status"))
        .and_then(Value::as_str);
    let pending = usize::from(status == Some("running"));
    let kind = match tool {
        "read" | "view" | "glob" | "grep" => SessionActivityKind::ReadingFile,
        "write" | "edit" | "apply_patch" => SessionActivityKind::EditingFile,
        _ => SessionActivityKind::RunningCommand,
    };
    let target = tool_target(tool, input);
    Some(activity(kind, target, observed_at, pending))
}

fn tool_target(tool: &str, input: Option<&Value>) -> Option<String> {
    let input = input?;
    for key in ["filePath", "path", "command", "description", "pattern"] {
        if let Some(value) = input
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(truncate(value, 72));
        }
    }
    Some(truncate(tool, 72))
}

fn activity(
    kind: SessionActivityKind,
    target: Option<String>,
    observed_at: Option<DateTime<Utc>>,
    pending_calls: usize,
) -> SessionActivitySnapshot {
    SessionActivitySnapshot {
        kind,
        target,
        observed_at,
        last_active_at: observed_at,
        last_effective_signal_at: observed_at,
        idle_candidate_at: None,
        pending_calls,
    }
}

fn project_name(directory: &str, title: &str) -> String {
    Path::new(directory)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| title.trim().to_string())
}

fn millis_to_system_time(value: i64) -> Option<SystemTime> {
    if value < 0 {
        return None;
    }
    SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(value as u64))
}

fn millis_to_datetime(value: i64) -> Option<DateTime<Utc>> {
    millis_to_system_time(value).map(DateTime::<Utc>::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::preferred_active_session;

    #[test]
    fn parses_fast_gpt_model_from_opencode_json() {
        assert_eq!(
            parse_model_id(Some(
                r#"{"id":"gpt-5.5-fast","providerID":"openai","variant":"high"}"#
            ))
            .as_deref(),
            Some("gpt-5.5-fast")
        );
    }

    #[test]
    fn finds_default_and_channel_specific_opencode_databases() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("opencode.db"), b"").expect("default db");
        fs::write(dir.path().join("opencode-prod.db"), b"").expect("prod db");
        fs::write(dir.path().join("other.db"), b"").expect("other db");

        let paths = database_paths_from_data_dirs(vec![dir.path().to_path_buf()]);
        let names: Vec<String> = paths
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .map(ToString::to_string)
            .collect();

        assert_eq!(names, vec!["opencode.db", "opencode-prod.db"]);
    }

    #[test]
    fn maps_running_bash_part_to_command_activity() {
        let part = r#"{"type":"tool","tool":"bash","state":{"status":"running","input":{"command":"cargo test --workspace"}}}"#;
        let activity = activity_from_part(part, 1_780_955_191_186).expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::RunningCommand);
        assert_eq!(activity.target.as_deref(), Some("cargo test --workspace"));
        assert_eq!(activity.pending_calls, 1);
    }

    #[test]
    fn maps_read_part_to_file_activity() {
        let part = r#"{"type":"tool","tool":"read","state":{"status":"completed","input":{"filePath":"D:/repo/src/main.rs"}}}"#;
        let activity = activity_from_part(part, 1_780_955_191_186).expect("activity");
        assert_eq!(activity.kind, SessionActivityKind::ReadingFile);
        assert_eq!(activity.target.as_deref(), Some("D:/repo/src/main.rs"));
    }

    #[test]
    fn parses_context_window_from_latest_step_finish_total() {
        let part = r#"{"type":"step-finish","tokens":{"total":231782,"input":185000,"output":10000,"reasoning":20000,"cache":{"read":16782,"write":0}}}"#;
        let context = context_window_from_part(part, "gpt-5.5-fast").expect("context");
        assert_eq!(context.window_tokens, 400_000);
        assert_eq!(context.used_tokens, 231_782);
        assert_eq!(context.remaining_tokens, 168_218);
        assert!((context.remaining_percent - 42.05).abs() < 0.05);
        assert_eq!(context.source, ContextWindowSource::Catalog);
    }

    #[test]
    fn parses_context_window_from_step_finish_token_fields() {
        let part = r#"{"type":"step-finish","tokens":{"input":"210000","output":12000,"reasoning":9000,"cache":{"read":782,"write":0}}}"#;
        let context = context_window_from_part(part, "gpt-5.5").expect("context");
        assert_eq!(context.window_tokens, 400_000);
        assert_eq!(context.used_tokens, 231_782);
        assert!((context.remaining_percent - 42.05).abs() < 0.05);
    }

    #[test]
    fn hides_context_window_when_step_finish_has_no_reliable_tokens() {
        let part = r#"{"type":"step-finish","tokens":{"input":0,"output":0}}"#;
        assert!(context_window_from_part(part, "gpt-5.5").is_none());
    }

    #[test]
    fn clamps_context_window_when_step_finish_exceeds_window() {
        let part = r#"{"type":"step-finish","tokens":{"total":900000}}"#;
        let context = context_window_from_part(part, "gpt-5.5").expect("context");
        assert_eq!(context.used_tokens, 400_000);
        assert_eq!(context.remaining_tokens, 0);
        assert_eq!(context.remaining_percent, 0.0);
    }

    #[test]
    fn collects_live_gpt_sessions_from_all_opencode_workspaces() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("opencode.db");
        let connection = rusqlite::Connection::open(&db_path).expect("database");
        create_opencode_schema(&connection);
        let now = current_millis();

        insert_opencode_session(
            &connection,
            TestOpenCodeSession {
                id: "current-workspace",
                directory: "D:/X/2-Dev/MCP-Servers/Codex-Discord-Rich-Presence",
                title: "Presence",
                model: r#"{"id":"gpt-5.5-fast","providerID":"openai"}"#,
                updated_at: now - 4_000,
            },
        );
        insert_opencode_part(
            &connection,
            "current-workspace",
            r#"{"type":"text"}"#,
            now - 4_000,
        );
        insert_opencode_session(
            &connection,
            TestOpenCodeSession {
                id: "other-workspace",
                directory: "D:/X/1-Work/OpenClaw",
                title: "Auditoria y reparacion integral de sitio web",
                model: r#"{"id":"gpt-5.5-fast","providerID":"openai"}"#,
                updated_at: now - 500,
            },
        );
        insert_opencode_part(
            &connection,
            "other-workspace",
            r#"{"type":"tool","tool":"bash","state":{"status":"running","input":{"command":"pnpm test"}}}"#,
            now - 500,
        );
        insert_opencode_session(
            &connection,
            TestOpenCodeSession {
                id: "stale-workspace",
                directory: "D:/X/1-Work/Stale",
                title: "Stale",
                model: "gpt-5.5",
                updated_at: now - 10_000,
            },
        );
        insert_opencode_session(
            &connection,
            TestOpenCodeSession {
                id: "non-gpt-workspace",
                directory: "D:/X/1-Work/OtherModel",
                title: "Other Model",
                model: "claude-sonnet-4-6",
                updated_at: now - 250,
            },
        );
        drop(connection);

        let sessions = collect_from_database(
            &db_path,
            Duration::from_secs(5),
            Duration::from_secs(5),
            &PricingConfig::default(),
        )
        .expect("sessions");
        let session_ids: Vec<&str> = sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect();

        assert_eq!(sessions.len(), 2);
        assert!(session_ids.contains(&"opencode:current-workspace"));
        assert!(session_ids.contains(&"opencode:other-workspace"));
        let active = preferred_active_session(&sessions).expect("active session");
        assert_eq!(active.session_id, "opencode:other-workspace");
        assert_eq!(active.project_name, "OpenClaw");
        assert_eq!(active.cwd, PathBuf::from("D:/X/1-Work/OpenClaw"));
        assert_eq!(
            active
                .activity
                .as_ref()
                .map(|activity| activity.pending_calls),
            Some(1)
        );
    }

    struct TestOpenCodeSession<'a> {
        id: &'a str,
        directory: &'a str,
        title: &'a str,
        model: &'a str,
        updated_at: i64,
    }

    fn create_opencode_schema(connection: &rusqlite::Connection) {
        connection
            .execute_batch(
                "create table session (
                    id text primary key,
                    directory text not null,
                    title text not null,
                    agent text,
                    model text,
                    cost real not null,
                    tokens_input integer not null,
                    tokens_output integer not null,
                    tokens_reasoning integer not null,
                    tokens_cache_read integer not null,
                    tokens_cache_write integer not null,
                    time_created integer not null,
                    time_updated integer not null
                );
                create table part (
                    session_id text not null,
                    data text not null,
                    time_created integer not null,
                    time_updated integer not null
                );",
            )
            .expect("schema");
    }

    fn insert_opencode_session(
        connection: &rusqlite::Connection,
        session: TestOpenCodeSession<'_>,
    ) {
        connection
            .execute(
                "insert into session (id, directory, title, agent, model, cost, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write, time_created, time_updated) values (?1, ?2, ?3, 'build', ?4, 0.0, 1000, 200, 300, 400, 0, ?5, ?6)",
                params![
                    session.id,
                    session.directory,
                    session.title,
                    session.model,
                    session.updated_at - 1_000,
                    session.updated_at
                ],
            )
            .expect("insert session");
    }

    fn insert_opencode_part(
        connection: &rusqlite::Connection,
        session_id: &str,
        data: &str,
        updated_at: i64,
    ) {
        connection
            .execute(
                "insert into part (session_id, data, time_created, time_updated) values (?1, ?2, ?3, ?3)",
                params![session_id, data, updated_at],
            )
            .expect("insert part");
    }

    fn current_millis() -> i64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time")
            .as_millis() as i64
    }
}
