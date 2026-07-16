use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::warn;

use codex_presence_core::{PresenceFieldId, PresenceLayoutConfig};

use crate::util::write_json_pretty_atomic;

const DEFAULT_STALE_SECONDS: u64 = 90;
const DEFAULT_POLL_SECONDS: u64 = 2;
const DEFAULT_ACTIVE_STICKY_SECONDS: u64 = 3600;
const MIN_ACTIVE_STICKY_SECONDS: u64 = 60;
const CONFIG_SCHEMA_VERSION: u32 = 13;
pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1470480085453770854";
pub const DEFAULT_DISCORD_DESKTOP_CLIENT_ID: &str = "1478395304624652345";
pub const DEFAULT_DISCORD_PUBLIC_KEY: &str =
    "29e563eeb755ae71d940c1b11d49dd3282a8886cd8b8cab829b2a14fcedad247";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PresenceConfig {
    pub schema_version: u32,
    pub presence_enabled: bool,
    pub discord_client_id: Option<String>,
    pub discord_client_id_desktop: Option<String>,
    pub discord_public_key: Option<String>,
    pub privacy: PrivacyConfig,
    pub display: DisplayConfig,
    pub pricing: PricingConfig,
    pub openai_plan: OpenAiPlanDisplayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PrivacyConfig {
    pub enabled: bool,
    pub show_project_name: bool,
    pub show_git_branch: bool,
    pub show_model: bool,
    pub show_tokens: bool,
    pub show_cost: bool,
    pub show_limits: bool,
    pub show_credits: bool,
    pub show_context: bool,
    pub show_activity: bool,
    pub show_activity_target: bool,
    pub show_systems: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyField {
    ProjectName,
    GitBranch,
    Model,
    Activity,
    TokenCount,
    Cost,
    SessionLimits,
    Credits,
    ContextUsage,
    Systems,
}

impl PrivacyField {
    pub const ALL: [Self; 10] = [
        Self::ProjectName,
        Self::GitBranch,
        Self::Model,
        Self::Activity,
        Self::TokenCount,
        Self::Cost,
        Self::SessionLimits,
        Self::Credits,
        Self::ContextUsage,
        Self::Systems,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::ProjectName => "Project name",
            Self::GitBranch => "Git branch",
            Self::Model => "Model",
            Self::Activity => "Activity",
            Self::TokenCount => "Token count",
            Self::Cost => "Cost",
            Self::SessionLimits => "Session limits",
            Self::Credits => "Credits available",
            Self::ContextUsage => "Context usage",
            Self::Systems => "Systems",
        }
    }

    pub const fn presence_field(self) -> PresenceFieldId {
        match self {
            Self::ProjectName => PresenceFieldId::Project,
            Self::GitBranch => PresenceFieldId::Branch,
            Self::Model => PresenceFieldId::Model,
            Self::Activity => PresenceFieldId::Activity,
            Self::TokenCount => PresenceFieldId::Tokens,
            Self::Cost => PresenceFieldId::Cost,
            Self::SessionLimits => PresenceFieldId::Quotas,
            Self::Credits => PresenceFieldId::Credits,
            Self::ContextUsage => PresenceFieldId::Context,
            Self::Systems => PresenceFieldId::Systems,
        }
    }

    pub const fn from_presence_field(field: PresenceFieldId) -> Self {
        match field {
            PresenceFieldId::Project => Self::ProjectName,
            PresenceFieldId::Branch => Self::GitBranch,
            PresenceFieldId::Model => Self::Model,
            PresenceFieldId::Activity => Self::Activity,
            PresenceFieldId::Tokens => Self::TokenCount,
            PresenceFieldId::Cost => Self::Cost,
            PresenceFieldId::Quotas => Self::SessionLimits,
            PresenceFieldId::Credits => Self::Credits,
            PresenceFieldId::Context => Self::ContextUsage,
            PresenceFieldId::Systems => Self::Systems,
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::ProjectName => "Repository or folder name",
            Self::GitBranch => "Current checked-out ref",
            Self::Model => "Model, reasoning, speed, and plan",
            Self::Activity => "Current Codex activity",
            Self::TokenCount => "Cumulative session tokens",
            Self::Cost => "Known session subtotal",
            Self::SessionLimits => "Available quota windows",
            Self::Credits => "Current Codex credit balance",
            Self::ContextUsage => "Current context-window percentage",
            Self::Systems => "Activity icon and workflow signal",
        }
    }

    pub const fn is_enabled(self, privacy: &PrivacyConfig) -> bool {
        match self {
            Self::ProjectName => privacy.show_project_name,
            Self::GitBranch => privacy.show_git_branch,
            Self::Model => privacy.show_model,
            Self::Activity => privacy.show_activity,
            Self::TokenCount => privacy.show_tokens,
            Self::Cost => privacy.show_cost,
            Self::SessionLimits => privacy.show_limits,
            Self::Credits => privacy.show_credits,
            Self::ContextUsage => privacy.show_context,
            Self::Systems => privacy.show_systems,
        }
    }

    pub fn toggle(self, privacy: &mut PrivacyConfig) {
        let value = !self.is_enabled(privacy);
        match self {
            Self::ProjectName => privacy.show_project_name = value,
            Self::GitBranch => privacy.show_git_branch = value,
            Self::Model => privacy.show_model = value,
            Self::Activity => privacy.show_activity = value,
            Self::TokenCount => privacy.show_tokens = value,
            Self::Cost => privacy.show_cost = value,
            Self::SessionLimits => privacy.show_limits = value,
            Self::Credits => privacy.show_credits = value,
            Self::ContextUsage => privacy.show_context = value,
            Self::Systems => privacy.show_systems = value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct PricingConfig {
    pub aliases: BTreeMap<String, String>,
    pub overrides: BTreeMap<String, ModelPricingOverride>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiPlanTier {
    Free,
    Go,
    Plus,
    Business,
    Enterprise,
    #[serde(
        rename = "pro_5x",
        alias = "pro5x",
        alias = "pro-5x",
        alias = "pro_100",
        alias = "pro-100"
    )]
    Pro5x,
    #[default]
    #[serde(
        rename = "pro_20x",
        alias = "pro",
        alias = "pro20x",
        alias = "pro-20x",
        alias = "pro_200",
        alias = "pro-200"
    )]
    Pro20x,
}

impl OpenAiPlanTier {
    pub fn title(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Go => "Go",
            Self::Plus => "Plus",
            Self::Business => "Business",
            Self::Enterprise => "Enterprise",
            Self::Pro5x => "Pro 5x",
            Self::Pro20x => "Pro 20x",
        }
    }

    pub fn monthly_price_usd(self) -> Option<u32> {
        match self {
            Self::Free => Some(0),
            Self::Go => Some(8),
            Self::Plus => Some(20),
            Self::Pro5x => Some(100),
            Self::Pro20x => Some(200),
            Self::Business | Self::Enterprise => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiPlanMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct OpenAiPlanDisplayConfig {
    pub mode: OpenAiPlanMode,
    pub tier: OpenAiPlanTier,
    pub show_price: bool,
}

impl OpenAiPlanDisplayConfig {
    // Legacy display helper kept for backwards compatibility; runtime now uses telemetry plan.
    pub fn label(&self) -> String {
        if self.show_price
            && let Some(monthly) = self.tier.monthly_price_usd()
        {
            return format!("{} (${monthly}/month)", self.tier.title());
        }
        self.tier.title().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanPreset {
    pub mode: OpenAiPlanMode,
    pub tier: Option<OpenAiPlanTier>,
    pub label: &'static str,
}

const PLAN_PRESETS: [PlanPreset; 8] = [
    PlanPreset {
        mode: OpenAiPlanMode::Auto,
        tier: None,
        label: "Auto Detect",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Free),
        label: "Free",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Go),
        label: "Go",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Plus),
        label: "Plus",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Pro5x),
        label: "Pro 5x ($100/month)",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Pro20x),
        label: "Pro 20x ($200/month)",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Business),
        label: "Business",
    },
    PlanPreset {
        mode: OpenAiPlanMode::Manual,
        tier: Some(OpenAiPlanTier::Enterprise),
        label: "Enterprise",
    },
];

pub fn plan_presets() -> &'static [PlanPreset] {
    &PLAN_PRESETS
}

pub fn plan_preset_index(plan: &OpenAiPlanDisplayConfig) -> usize {
    if matches!(plan.mode, OpenAiPlanMode::Auto) {
        return 0;
    }

    PLAN_PRESETS
        .iter()
        .position(|preset| {
            matches!(preset.mode, OpenAiPlanMode::Manual) && preset.tier == Some(plan.tier)
        })
        .unwrap_or(5)
}

pub fn apply_plan_preset(plan: &mut OpenAiPlanDisplayConfig, preset_index: usize) {
    let Some(preset) = PLAN_PRESETS.get(preset_index).copied() else {
        return;
    };

    plan.mode = preset.mode;
    if let Some(tier) = preset.tier {
        plan.tier = tier;
    }
}

impl Default for OpenAiPlanDisplayConfig {
    fn default() -> Self {
        Self {
            mode: OpenAiPlanMode::Auto,
            tier: OpenAiPlanTier::Pro20x,
            show_price: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModelPricingOverride {
    pub input_per_million: f64,
    pub cached_input_per_million: Option<f64>,
    pub output_per_million: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalLogoMode {
    #[default]
    Auto,
    Ascii,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceSurface {
    Cli,
    VsCode,
    Desktop,
}

impl PresenceSurface {
    pub fn detect(originator: Option<&str>, source: Option<&str>) -> Option<Self> {
        originator
            .and_then(classify_surface_signal)
            .or_else(|| source.and_then(classify_surface_signal))
    }

    pub const fn label(self, desktop_design: DesktopPresenceDesign) -> &'static str {
        match self {
            Self::Cli => "Codex CLI",
            Self::VsCode => "Codex VS Code Extension",
            Self::Desktop => desktop_design.label(),
        }
    }
}

fn classify_surface_signal(value: &str) -> Option<PresenceSurface> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    if normalized.contains("codex desktop")
        || normalized.contains("openai.codex")
        || normalized.contains("opencode")
        || normalized == "desktop"
    {
        return Some(PresenceSurface::Desktop);
    }
    if normalized.contains("vscode") || normalized.contains("visual studio code") {
        return Some(PresenceSurface::VsCode);
    }
    if normalized.contains("codex-tui")
        || normalized.contains("codex cli")
        || matches!(normalized.as_str(), "cli" | "terminal")
    {
        return Some(PresenceSurface::Cli);
    }
    None
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DesktopPresenceDesign {
    #[default]
    CodexApp,
    ChatGptApp,
}

impl DesktopPresenceDesign {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CodexApp => "Codex App",
            Self::ChatGptApp => "ChatGPT App",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::CodexApp => Self::ChatGptApp,
            Self::ChatGptApp => Self::CodexApp,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DisplayConfig {
    pub desktop_presence_design: DesktopPresenceDesign,
    pub large_image_key: String,
    pub large_text: String,
    pub desktop_large_image_key: String,
    pub desktop_large_text: String,
    pub small_image_key: String,
    pub small_text: String,
    pub activity_small_image_keys: ActivitySmallImageKeys,
    pub terminal_logo_mode: TerminalLogoMode,
    pub terminal_logo_path: Option<String>,
    pub presence_layout: PresenceLayoutConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
pub struct ActivitySmallImageKeys {
    pub thinking: Option<String>,
    pub reading: Option<String>,
    pub editing: Option<String>,
    pub running: Option<String>,
    pub waiting: Option<String>,
    pub idle: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeSettings {
    pub stale_threshold: Duration,
    pub active_sticky_window: Duration,
    pub poll_interval: Duration,
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            presence_enabled: true,
            discord_client_id: Some(DEFAULT_DISCORD_CLIENT_ID.to_string()),
            discord_client_id_desktop: Some(DEFAULT_DISCORD_DESKTOP_CLIENT_ID.to_string()),
            discord_public_key: Some(DEFAULT_DISCORD_PUBLIC_KEY.to_string()),
            privacy: PrivacyConfig::default(),
            display: DisplayConfig::default(),
            pricing: PricingConfig::default(),
            openai_plan: OpenAiPlanDisplayConfig::default(),
        }
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            show_project_name: true,
            show_git_branch: true,
            show_model: true,
            show_tokens: true,
            show_cost: true,
            show_limits: true,
            show_credits: true,
            show_context: true,
            show_activity: true,
            show_activity_target: true,
            show_systems: true,
        }
    }
}

impl Default for ModelPricingOverride {
    fn default() -> Self {
        Self {
            input_per_million: 0.0,
            cached_input_per_million: Some(0.0),
            output_per_million: 0.0,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            desktop_presence_design: DesktopPresenceDesign::CodexApp,
            large_image_key: "codex-logo".to_string(),
            large_text: "Codex".to_string(),
            desktop_large_image_key: "codex-app".to_string(),
            desktop_large_text: "Codex App".to_string(),
            small_image_key: "openai".to_string(),
            small_text: "OpenAI".to_string(),
            activity_small_image_keys: ActivitySmallImageKeys::default(),
            terminal_logo_mode: TerminalLogoMode::Auto,
            terminal_logo_path: None,
            presence_layout: PresenceLayoutConfig::default(),
        }
    }
}

impl PresenceConfig {
    pub fn load_or_init() -> Result<Self> {
        let cfg_path = config_path();
        if let Some(parent) = cfg_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }

        if cfg_path.exists() {
            Self::load_from_path(&cfg_path)
        } else {
            let cfg = PresenceConfig::default();
            cfg.save()?;
            Ok(cfg)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path();
        self.save_to_path(&path)
    }

    pub fn reload_from_disk(&mut self) -> bool {
        self.reload_from_path(&config_path())
    }

    pub fn reload_from_path(&mut self, path: &Path) -> bool {
        match Self::load_from_path(path) {
            Ok(reloaded) => {
                let changed = *self != reloaded;
                *self = reloaded;
                changed
            }
            Err(error) => {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "presence config reload failed; keeping the last valid configuration"
                );
                false
            }
        }
    }

    pub fn toggle_presence(&mut self) -> Result<()> {
        self.toggle_presence_at_path(&config_path())
    }

    pub fn toggle_presence_at_path(&mut self, path: &Path) -> Result<()> {
        self.reload_from_path(path);
        self.presence_enabled = !self.presence_enabled;
        self.save_to_path(path)
    }

    fn load_from_path(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut parsed: PresenceConfig = serde_json::from_str(&raw)
            .with_context(|| format!("invalid JSON in {}", path.display()))?;
        if parsed.normalize_for_runtime() {
            parsed.save_to_path(path)?;
        }
        Ok(parsed)
    }

    fn save_to_path(&self, path: &Path) -> Result<()> {
        write_json_pretty_atomic(path, self)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn effective_client_id(&self) -> Option<String> {
        self.effective_client_id_for_surface(PresenceSurface::Cli)
    }

    pub fn effective_client_id_for_surface(&self, surface: PresenceSurface) -> Option<String> {
        Some(codex_client_id_for_surface(surface, self.display.desktop_presence_design).to_string())
    }

    pub fn normalize_for_runtime(&mut self) -> bool {
        self.normalize_and_migrate()
    }

    fn normalize_and_migrate(&mut self) -> bool {
        let mut changed = false;
        let default_display = DisplayConfig::default();

        if self.schema_version < CONFIG_SCHEMA_VERSION {
            self.schema_version = CONFIG_SCHEMA_VERSION;
            changed = true;
        }

        if self.discord_client_id.as_deref() != Some(DEFAULT_DISCORD_CLIENT_ID) {
            self.discord_client_id = Some(DEFAULT_DISCORD_CLIENT_ID.to_string());
            changed = true;
        }
        if self.discord_client_id_desktop.as_deref() != Some(DEFAULT_DISCORD_DESKTOP_CLIENT_ID) {
            self.discord_client_id_desktop = Some(DEFAULT_DISCORD_DESKTOP_CLIENT_ID.to_string());
            changed = true;
        }
        if is_missing(&self.discord_public_key) {
            self.discord_public_key = Some(DEFAULT_DISCORD_PUBLIC_KEY.to_string());
            changed = true;
        }
        if normalize_codex_display(&mut self.display, &default_display) {
            changed = true;
        }
        if self.display.presence_layout.normalize() {
            changed = true;
        }

        for item in &mut self.display.presence_layout.fields {
            let enabled = match item.field {
                PresenceFieldId::Project => self.privacy.show_project_name,
                PresenceFieldId::Branch => self.privacy.show_git_branch,
                PresenceFieldId::Model => self.privacy.show_model,
                PresenceFieldId::Activity => self.privacy.show_activity,
                PresenceFieldId::Tokens => self.privacy.show_tokens,
                PresenceFieldId::Cost => self.privacy.show_cost,
                PresenceFieldId::Quotas => self.privacy.show_limits,
                PresenceFieldId::Credits => self.privacy.show_credits,
                PresenceFieldId::Context => self.privacy.show_context,
                PresenceFieldId::Systems => self.privacy.show_systems,
            };
            if item.enabled != enabled {
                item.enabled = enabled;
                changed = true;
            }
        }

        if self.display.large_image_key.trim().is_empty() {
            self.display.large_image_key = default_display.large_image_key;
            changed = true;
        }
        if self.display.large_text.trim().is_empty() {
            self.display.large_text = default_display.large_text;
            changed = true;
        }
        if self.display.desktop_large_image_key.trim().is_empty() {
            self.display.desktop_large_image_key = default_display.desktop_large_image_key;
            changed = true;
        }
        if self.display.desktop_large_text.trim().is_empty() {
            self.display.desktop_large_text = default_display.desktop_large_text;
            changed = true;
        }
        if self.display.small_image_key.trim().is_empty() {
            self.display.small_image_key = default_display.small_image_key;
            changed = true;
        }
        if self.display.small_text.trim().is_empty() {
            self.display.small_text = default_display.small_text;
            changed = true;
        }
        for item in [
            &mut self.display.activity_small_image_keys.thinking,
            &mut self.display.activity_small_image_keys.reading,
            &mut self.display.activity_small_image_keys.editing,
            &mut self.display.activity_small_image_keys.running,
            &mut self.display.activity_small_image_keys.waiting,
            &mut self.display.activity_small_image_keys.idle,
        ] {
            if normalize_optional_string(item) {
                changed = true;
            }
        }
        if self
            .display
            .terminal_logo_path
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            self.display.terminal_logo_path = None;
            changed = true;
        }
        if normalize_pricing_config(&mut self.pricing) {
            changed = true;
        }

        changed
    }
}

fn codex_client_id_for_surface(
    surface: PresenceSurface,
    desktop_design: DesktopPresenceDesign,
) -> &'static str {
    match (surface, desktop_design) {
        (PresenceSurface::Desktop, DesktopPresenceDesign::CodexApp) => {
            DEFAULT_DISCORD_DESKTOP_CLIENT_ID
        }
        (PresenceSurface::Cli | PresenceSurface::VsCode, _)
        | (PresenceSurface::Desktop, DesktopPresenceDesign::ChatGptApp) => {
            DEFAULT_DISCORD_CLIENT_ID
        }
    }
}

pub fn runtime_settings() -> RuntimeSettings {
    let sticky_seconds = env_u64(
        "CODEX_PRESENCE_ACTIVE_STICKY_SECONDS",
        DEFAULT_ACTIVE_STICKY_SECONDS,
    )
    .max(MIN_ACTIVE_STICKY_SECONDS);
    RuntimeSettings {
        stale_threshold: Duration::from_secs(env_u64(
            "CODEX_PRESENCE_STALE_SECONDS",
            DEFAULT_STALE_SECONDS,
        )),
        active_sticky_window: Duration::from_secs(sticky_seconds),
        poll_interval: Duration::from_secs(env_u64(
            "CODEX_PRESENCE_POLL_SECONDS",
            DEFAULT_POLL_SECONDS,
        )),
    }
}

pub fn codex_home() -> PathBuf {
    if let Ok(custom) = env::var("CODEX_HOME") {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
}

pub fn sessions_path() -> PathBuf {
    codex_home().join("sessions")
}

pub fn sessions_paths() -> Vec<PathBuf> {
    let mut ordered: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    push_unique_path(&mut ordered, &mut seen, sessions_path());

    #[cfg(windows)]
    {
        if include_wsl_session_roots() {
            for candidate in windows_wsl_sessions_candidates() {
                push_unique_path(&mut ordered, &mut seen, candidate);
            }
        }
    }

    #[cfg(all(unix, not(windows)))]
    {
        for candidate in wsl_windows_sessions_candidates() {
            push_unique_path(&mut ordered, &mut seen, candidate);
        }
    }

    ordered
}

pub fn config_path() -> PathBuf {
    codex_home().join("discord-presence-config.json")
}

pub fn global_state_paths() -> Vec<PathBuf> {
    let mut ordered: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    push_unique_path(
        &mut ordered,
        &mut seen,
        codex_home().join(".codex-global-state.json"),
    );
    for sessions_root in sessions_paths() {
        if let Some(home) = sessions_root.parent() {
            push_unique_path(
                &mut ordered,
                &mut seen,
                home.join(".codex-global-state.json"),
            );
        }
    }

    ordered
}

pub fn lock_path() -> PathBuf {
    codex_home().join("codex-discord-presence.lock")
}

pub fn instance_meta_path() -> PathBuf {
    codex_home().join("codex-discord-presence.instance.json")
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn is_missing(value: &Option<String>) -> bool {
    value.as_ref().map(|v| v.trim().is_empty()).unwrap_or(true)
}

fn normalize_codex_display(display: &mut DisplayConfig, default_display: &DisplayConfig) -> bool {
    let mut changed = false;
    if display.large_image_key.as_str() != default_display.large_image_key {
        display.large_image_key = default_display.large_image_key.clone();
        changed = true;
    }
    if display.large_text.as_str() != default_display.large_text {
        display.large_text = default_display.large_text.clone();
        changed = true;
    }
    if display.desktop_large_image_key.as_str() != default_display.desktop_large_image_key {
        display.desktop_large_image_key = default_display.desktop_large_image_key.clone();
        changed = true;
    }
    if display.desktop_large_text.as_str() != default_display.desktop_large_text {
        display.desktop_large_text = default_display.desktop_large_text.clone();
        changed = true;
    }
    if display.small_image_key.as_str() != default_display.small_image_key {
        display.small_image_key = default_display.small_image_key.clone();
        changed = true;
    }
    if display.small_text.as_str() != default_display.small_text {
        display.small_text = default_display.small_text.clone();
        changed = true;
    }
    if has_activity_image_overrides(&display.activity_small_image_keys) {
        display.activity_small_image_keys = ActivitySmallImageKeys::default();
        changed = true;
    }
    changed
}

fn has_activity_image_overrides(keys: &ActivitySmallImageKeys) -> bool {
    keys.thinking.is_some()
        || keys.reading.is_some()
        || keys.editing.is_some()
        || keys.running.is_some()
        || keys.waiting.is_some()
        || keys.idle.is_some()
}

fn normalize_optional_string(value: &mut Option<String>) -> bool {
    if let Some(item) = value.as_mut() {
        let trimmed = item.trim().to_string();
        if trimmed.is_empty() {
            *value = None;
            return true;
        }
        if *item != trimmed {
            *item = trimmed;
            return true;
        }
    }
    false
}

fn normalize_pricing_config(pricing: &mut PricingConfig) -> bool {
    let mut changed = false;

    let mut normalized_aliases: BTreeMap<String, String> = BTreeMap::new();
    for (raw_key, raw_target) in pricing.aliases.iter() {
        let key = raw_key.trim().to_ascii_lowercase();
        let target = raw_target.trim().to_ascii_lowercase();
        if matches!(key.as_str(), "gpt-5.3-codex" | "gpt-5.3-codex-latest")
            && target == "gpt-5.2-codex"
        {
            changed = true;
            continue;
        }
        if matches!(
            key.as_str(),
            "gpt-5.3-codex-spark" | "gpt-5.3-codex-spark-latest"
        ) && matches!(target.as_str(), "gpt-5.2-codex" | "gpt-5.3-codex")
        {
            changed = true;
            continue;
        }
        if key.is_empty() || target.is_empty() || key == target {
            if !raw_key.trim().is_empty() || !raw_target.trim().is_empty() {
                changed = true;
            }
            continue;
        }
        if normalized_aliases
            .insert(key.clone(), target.clone())
            .is_none()
            && (key != raw_key.trim() || target != raw_target.trim())
        {
            changed = true;
        }
    }
    if pricing.aliases != normalized_aliases {
        pricing.aliases = normalized_aliases;
        changed = true;
    }

    let mut normalized_overrides: BTreeMap<String, ModelPricingOverride> = BTreeMap::new();
    for (raw_key, source_pricing) in pricing.overrides.iter() {
        let mut override_pricing = source_pricing.clone();
        let key = raw_key.trim().to_ascii_lowercase();
        if key.is_empty() {
            changed = true;
            continue;
        }

        if !override_pricing.input_per_million.is_finite()
            || override_pricing.input_per_million < 0.0
        {
            override_pricing.input_per_million = 0.0;
            changed = true;
        }
        if let Some(value) = override_pricing.cached_input_per_million
            && (!value.is_finite() || value < 0.0)
        {
            override_pricing.cached_input_per_million = Some(0.0);
            changed = true;
        }
        if !override_pricing.output_per_million.is_finite()
            || override_pricing.output_per_million < 0.0
        {
            override_pricing.output_per_million = 0.0;
            changed = true;
        }

        if key != raw_key.trim() {
            changed = true;
        }
        normalized_overrides.insert(key, override_pricing);
    }
    if pricing.overrides != normalized_overrides {
        pricing.overrides = normalized_overrides;
        changed = true;
    }

    changed
}

fn push_unique_path(paths: &mut Vec<PathBuf>, seen: &mut HashSet<String>, candidate: PathBuf) {
    if candidate.as_os_str().is_empty() {
        return;
    }
    let key = path_key(&candidate);
    if seen.insert(key) {
        paths.push(candidate);
    }
}

fn path_key(path: &Path) -> String {
    #[cfg(windows)]
    {
        path.to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase()
    }

    #[cfg(not(windows))]
    {
        path.to_string_lossy().to_string()
    }
}

#[cfg(all(unix, not(windows)))]
fn wsl_windows_sessions_candidates() -> Vec<PathBuf> {
    if !running_in_wsl() {
        return Vec::new();
    }

    let mut candidates = Vec::new();

    if let Ok(profile) = env::var("USERPROFILE") {
        let profile = profile.trim();
        if !profile.is_empty() {
            candidates.push(PathBuf::from(profile).join(".codex").join("sessions"));
        }
    }

    if let Ok(username) = env::var("USERNAME").or_else(|_| env::var("USER")) {
        let username = username.trim();
        if !username.is_empty() {
            candidates.push(
                PathBuf::from("/mnt/c/Users")
                    .join(username)
                    .join(".codex")
                    .join("sessions"),
            );
        }
    }

    candidates
}

#[cfg(all(unix, not(windows)))]
fn running_in_wsl() -> bool {
    if env::var_os("WSL_DISTRO_NAME").is_some() {
        return true;
    }
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|value| value.to_ascii_lowercase().contains("microsoft"))
        .unwrap_or(false)
}

#[cfg(windows)]
fn include_wsl_session_roots() -> bool {
    parse_bool_flag(env::var("CODEX_PRESENCE_INCLUDE_WSL").ok().as_deref())
        || parse_bool_flag(env::var("CC_PRESENCE_INCLUDE_WSL").ok().as_deref())
}

#[cfg(any(windows, test))]
fn parse_bool_flag(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

#[cfg(windows)]
fn windows_wsl_sessions_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let distros = windows_wsl_distro_names();
    for distro in distros {
        if let Some(home) = wsl_home_for_distro(&distro) {
            candidates.push(wsl_home_to_unc_sessions_path(&distro, &home));
            continue;
        }
        if let Some(username) = env::var("USERNAME")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            let fallback = format!(
                r"\\wsl.localhost\{}\home\{}\.codex\sessions",
                distro, username
            );
            candidates.push(PathBuf::from(fallback));
        }
    }
    candidates
}

#[cfg(windows)]
fn windows_wsl_distro_names() -> Vec<String> {
    let output = crate::util::silent_command("wsl.exe")
        .args(["-l", "-q"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    decode_windows_text_output(&output.stdout)
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(windows)]
fn wsl_home_for_distro(distro: &str) -> Option<String> {
    let output = crate::util::silent_command("wsl.exe")
        .args(["-d", distro, "--", "sh", "-lc", "printf %s \"$HOME\""])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let home = decode_windows_text_output(&output.stdout)
        .trim()
        .to_string();
    if home.is_empty() { None } else { Some(home) }
}

#[cfg(windows)]
fn wsl_home_to_unc_sessions_path(distro: &str, home: &str) -> PathBuf {
    let mut unc = format!(r"\\wsl.localhost\{}", distro);
    for part in home.trim().trim_start_matches('/').split('/') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        unc.push('\\');
        unc.push_str(part);
    }
    unc.push_str(r"\.codex\sessions");
    PathBuf::from(unc)
}

#[cfg(windows)]
fn decode_windows_text_output(bytes: &[u8]) -> String {
    let has_interleaved_nuls = bytes
        .iter()
        .skip(1)
        .step_by(2)
        .take(64)
        .any(|byte| *byte == 0);

    if bytes.starts_with(&[0xFF, 0xFE]) || has_interleaved_nuls {
        let mut utf16: Vec<u16> = Vec::with_capacity(bytes.len() / 2);
        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            utf16.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        return String::from_utf16_lossy(&utf16);
    }

    String::from_utf8_lossy(bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_client_id_is_rewritten_to_codex_default() {
        let cfg = PresenceConfig {
            discord_client_id: Some("from-config".to_string()),
            discord_client_id_desktop: None,
            ..PresenceConfig::default()
        };
        assert_eq!(
            cfg.effective_client_id().as_deref(),
            Some(DEFAULT_DISCORD_CLIENT_ID)
        );
    }

    #[test]
    fn migration_sets_default_client_id_when_missing() {
        let mut cfg = PresenceConfig {
            schema_version: 2,
            presence_enabled: true,
            discord_client_id: None,
            discord_client_id_desktop: None,
            discord_public_key: None,
            privacy: PrivacyConfig::default(),
            display: DisplayConfig::default(),
            pricing: PricingConfig::default(),
            openai_plan: OpenAiPlanDisplayConfig::default(),
        };

        let changed = cfg.normalize_and_migrate();

        assert!(changed);
        assert_eq!(cfg.schema_version, 13);
        assert!(cfg.presence_enabled);
        assert_eq!(
            cfg.discord_client_id.as_deref(),
            Some(DEFAULT_DISCORD_CLIENT_ID)
        );
        assert_eq!(
            cfg.discord_client_id_desktop.as_deref(),
            Some(DEFAULT_DISCORD_DESKTOP_CLIENT_ID)
        );
        assert_eq!(
            cfg.discord_public_key.as_deref(),
            Some(DEFAULT_DISCORD_PUBLIC_KEY)
        );
        assert_eq!(cfg.openai_plan.mode, OpenAiPlanMode::Auto);
        assert_eq!(cfg.openai_plan.tier, OpenAiPlanTier::Pro20x);
        assert!(cfg.openai_plan.show_price);
    }

    #[test]
    fn privacy_defaults_cover_every_user_visible_presence_field() {
        let privacy = PrivacyConfig::default();

        assert!(privacy.show_project_name);
        assert!(privacy.show_git_branch);
        assert!(privacy.show_model);
        assert!(privacy.show_activity);
        assert!(privacy.show_tokens);
        assert!(privacy.show_cost);
        assert!(privacy.show_limits);
        assert!(privacy.show_context);
        assert!(privacy.show_systems);
    }

    #[test]
    fn privacy_fields_toggle_through_one_canonical_contract() {
        let mut privacy = PrivacyConfig::default();

        for field in PrivacyField::ALL {
            assert!(
                field.is_enabled(&privacy),
                "{} should default on",
                field.label()
            );
            field.toggle(&mut privacy);
            assert!(
                !field.is_enabled(&privacy),
                "{} should toggle off",
                field.label()
            );
        }
    }

    #[test]
    fn display_defaults_to_auto_logo_mode() {
        let cfg = PresenceConfig::default();
        assert_eq!(cfg.display.terminal_logo_mode, TerminalLogoMode::Auto);
        assert_eq!(cfg.display.terminal_logo_path, None);
        assert_eq!(
            cfg.display.desktop_presence_design,
            DesktopPresenceDesign::CodexApp
        );
        assert_eq!(cfg.display.desktop_large_image_key, "codex-app");
        assert_eq!(cfg.display.desktop_large_text, "Codex App");
    }

    #[test]
    fn desktop_presence_design_toggles_between_codex_and_chatgpt() {
        let mut design = DesktopPresenceDesign::CodexApp;
        assert_eq!(design.label(), "Codex App");
        design = design.toggled();
        assert_eq!(design, DesktopPresenceDesign::ChatGptApp);
        assert_eq!(design.label(), "ChatGPT App");
        assert_eq!(design.toggled(), DesktopPresenceDesign::CodexApp);
    }

    #[test]
    fn desktop_presence_design_survives_json_and_schema_migration() {
        let mut cfg = PresenceConfig {
            schema_version: 9,
            ..PresenceConfig::default()
        };
        cfg.display.desktop_presence_design = DesktopPresenceDesign::ChatGptApp;

        let json = serde_json::to_string(&cfg).expect("serialize config");
        let mut restored: PresenceConfig = serde_json::from_str(&json).expect("deserialize config");
        assert!(restored.normalize_and_migrate());
        assert_eq!(restored.schema_version, CONFIG_SCHEMA_VERSION);
        assert_eq!(
            restored.display.desktop_presence_design,
            DesktopPresenceDesign::ChatGptApp
        );
    }

    #[test]
    fn desktop_surface_client_id_uses_codex_app_default() {
        let mut cfg = PresenceConfig {
            discord_client_id: Some("default-id".to_string()),
            discord_client_id_desktop: Some("desktop-id".to_string()),
            ..PresenceConfig::default()
        };
        assert_eq!(
            cfg.effective_client_id_for_surface(PresenceSurface::Cli)
                .as_deref(),
            Some(DEFAULT_DISCORD_CLIENT_ID)
        );
        assert_eq!(
            cfg.effective_client_id_for_surface(PresenceSurface::VsCode)
                .as_deref(),
            Some(DEFAULT_DISCORD_CLIENT_ID)
        );
        assert_eq!(
            cfg.effective_client_id_for_surface(PresenceSurface::Desktop)
                .as_deref(),
            Some(DEFAULT_DISCORD_DESKTOP_CLIENT_ID)
        );

        cfg.display.desktop_presence_design = DesktopPresenceDesign::ChatGptApp;
        assert_eq!(
            cfg.effective_client_id_for_surface(PresenceSurface::Desktop)
                .as_deref(),
            Some(DEFAULT_DISCORD_CLIENT_ID)
        );
    }

    #[test]
    fn pricing_defaults_leave_builtin_aliases_to_model_catalog() {
        let cfg = PresenceConfig::default();
        assert!(cfg.pricing.aliases.is_empty());
    }

    #[test]
    fn pricing_normalization_lowercases_alias_and_migrates_legacy_gpt_5_3_targets() {
        let mut cfg = PresenceConfig::default();
        cfg.pricing.aliases.clear();
        cfg.pricing
            .aliases
            .insert(" GPT-5.3-CODEX ".to_string(), " GPT-5.2-CODEX ".to_string());
        cfg.pricing.aliases.insert(
            " GPT-5.3-CODEX-SPARK ".to_string(),
            " GPT-5.2-CODEX ".to_string(),
        );
        cfg.pricing.overrides.clear();
        cfg.pricing.overrides.insert(
            " GPT-5.2-CODEX ".to_string(),
            ModelPricingOverride {
                input_per_million: 1.0,
                cached_input_per_million: Some(0.1),
                output_per_million: 2.0,
            },
        );

        let changed = cfg.normalize_and_migrate();
        assert!(changed);
        assert!(!cfg.pricing.aliases.contains_key("gpt-5.3-codex"));
        assert!(!cfg.pricing.aliases.contains_key("gpt-5.3-codex-spark"));
        assert!(cfg.pricing.overrides.contains_key("gpt-5.2-codex"));
    }

    #[test]
    fn default_openai_plan_is_pro_20x_with_price() {
        let cfg = PresenceConfig::default();
        assert_eq!(cfg.openai_plan.mode, OpenAiPlanMode::Auto);
        assert_eq!(cfg.openai_plan.tier, OpenAiPlanTier::Pro20x);
        assert!(cfg.openai_plan.show_price);
        assert_eq!(cfg.openai_plan.label(), "Pro 20x ($200/month)");
    }

    #[test]
    fn plan_presets_include_distinct_pro_usage_tiers() {
        let labels: Vec<&str> = plan_presets().iter().map(|preset| preset.label).collect();

        assert!(labels.contains(&"Pro 5x ($100/month)"));
        assert!(labels.contains(&"Pro 20x ($200/month)"));
        assert_eq!(
            plan_presets()
                .iter()
                .filter(|preset| matches!(
                    preset.tier,
                    Some(OpenAiPlanTier::Pro5x | OpenAiPlanTier::Pro20x)
                ))
                .count(),
            2
        );
    }

    #[test]
    fn legacy_pro_plan_deserializes_to_pro_20x() {
        let raw = r#"{"mode":"manual","tier":"pro","show_price":true}"#;
        let plan: OpenAiPlanDisplayConfig = serde_json::from_str(raw).expect("plan");

        assert_eq!(plan.tier, OpenAiPlanTier::Pro20x);
        assert_eq!(plan.label(), "Pro 20x ($200/month)");
    }

    #[test]
    fn openai_plan_label_without_price_uses_tier_name_only() {
        let cfg = OpenAiPlanDisplayConfig {
            mode: OpenAiPlanMode::Manual,
            tier: OpenAiPlanTier::Go,
            show_price: false,
        };
        assert_eq!(cfg.label(), "Go");
    }

    #[test]
    fn plan_preset_index_tracks_auto_and_manual_modes() {
        assert_eq!(plan_preset_index(&PresenceConfig::default().openai_plan), 0);
        let plan = OpenAiPlanDisplayConfig {
            mode: OpenAiPlanMode::Manual,
            tier: OpenAiPlanTier::Business,
            show_price: true,
        };
        assert_eq!(plan_preset_index(&plan), 6);
    }

    #[test]
    fn apply_plan_preset_switches_between_auto_and_manual() {
        let mut plan = PresenceConfig::default().openai_plan;
        apply_plan_preset(&mut plan, 3);
        assert_eq!(plan.mode, OpenAiPlanMode::Manual);
        assert_eq!(plan.tier, OpenAiPlanTier::Plus);

        apply_plan_preset(&mut plan, 0);
        assert_eq!(plan.mode, OpenAiPlanMode::Auto);
        assert_eq!(plan.tier, OpenAiPlanTier::Plus);
    }

    #[test]
    fn windows_wsl_probe_commands_use_hidden_launcher() {
        let source = include_str!("config.rs");
        let direct_spawn = ["Command::new(", "\"wsl.exe\"", ")"].concat();
        let hidden_spawn = ["crate::util::silent_command(", "\"wsl.exe\"", ")"].concat();

        assert!(
            !source.contains(&direct_spawn),
            "WSL probes must not use visible Windows subprocess launches"
        );
        assert_eq!(
            source.matches(&hidden_spawn).count(),
            2,
            "both WSL discovery probes must use the hidden command helper"
        );
    }

    #[test]
    fn windows_wsl_roots_are_explicit_opt_in() {
        let source = include_str!("config.rs");

        assert!(!parse_bool_flag(None));
        assert!(!parse_bool_flag(Some("")));
        assert!(!parse_bool_flag(Some("0")));
        assert!(!parse_bool_flag(Some("false")));
        assert!(parse_bool_flag(Some("1")));
        assert!(parse_bool_flag(Some("true")));
        assert!(parse_bool_flag(Some("yes")));
        assert!(parse_bool_flag(Some("on")));
        assert!(
            source.contains("if include_wsl_session_roots()"),
            "Windows WSL session scanning must stay opt-in before invoking wsl.exe"
        );
    }
}
