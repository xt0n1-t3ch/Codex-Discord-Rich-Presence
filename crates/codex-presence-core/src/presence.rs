use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PresenceFieldId {
    Project,
    Branch,
    Model,
    Activity,
    Tokens,
    Cost,
    Quotas,
    Credits,
    Context,
    Systems,
}

impl PresenceFieldId {
    pub const ALL: [Self; 10] = [
        Self::Project,
        Self::Branch,
        Self::Model,
        Self::Activity,
        Self::Tokens,
        Self::Cost,
        Self::Quotas,
        Self::Credits,
        Self::Context,
        Self::Systems,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Branch => "branch",
            Self::Model => "model",
            Self::Activity => "activity",
            Self::Tokens => "tokens",
            Self::Cost => "cost",
            Self::Quotas => "quotas",
            Self::Credits => "credits",
            Self::Context => "context",
            Self::Systems => "systems",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "project" => Some(Self::Project),
            "branch" => Some(Self::Branch),
            "model" => Some(Self::Model),
            "activity" => Some(Self::Activity),
            "tokens" => Some(Self::Tokens),
            "cost" => Some(Self::Cost),
            "quotas" | "limits" => Some(Self::Quotas),
            "credits" => Some(Self::Credits),
            "context" => Some(Self::Context),
            "systems" => Some(Self::Systems),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresenceZone {
    Details,
    State,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LabelStyle {
    #[default]
    Compact,
    Descriptive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PresencePreset {
    Minimal,
    Standard,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresenceFieldConfig {
    pub field: PresenceFieldId,
    pub enabled: bool,
    pub zone: PresenceZone,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PresenceLayoutConfig {
    pub label_style: LabelStyle,
    pub fields: Vec<PresenceFieldConfig>,
}

impl Default for PresenceLayoutConfig {
    fn default() -> Self {
        use PresenceFieldId::*;
        use PresenceZone::*;
        Self {
            label_style: LabelStyle::Compact,
            fields: vec![
                PresenceFieldConfig {
                    field: Activity,
                    enabled: true,
                    zone: Details,
                },
                PresenceFieldConfig {
                    field: Project,
                    enabled: true,
                    zone: Details,
                },
                PresenceFieldConfig {
                    field: Branch,
                    enabled: true,
                    zone: Details,
                },
                PresenceFieldConfig {
                    field: Model,
                    enabled: true,
                    zone: State,
                },
                PresenceFieldConfig {
                    field: Cost,
                    enabled: true,
                    zone: State,
                },
                PresenceFieldConfig {
                    field: Tokens,
                    enabled: true,
                    zone: State,
                },
                PresenceFieldConfig {
                    field: Context,
                    enabled: true,
                    zone: State,
                },
                PresenceFieldConfig {
                    field: Quotas,
                    enabled: true,
                    zone: State,
                },
                PresenceFieldConfig {
                    field: Credits,
                    enabled: true,
                    zone: State,
                },
                PresenceFieldConfig {
                    field: Systems,
                    enabled: true,
                    zone: State,
                },
            ],
        }
    }
}

impl PresenceLayoutConfig {
    pub fn for_preset(preset: PresencePreset) -> Self {
        let mut layout = Self::default();
        match preset {
            PresencePreset::Minimal => {
                for item in &mut layout.fields {
                    item.enabled = matches!(
                        item.field,
                        PresenceFieldId::Activity
                            | PresenceFieldId::Project
                            | PresenceFieldId::Model
                    );
                }
            }
            PresencePreset::Standard => {}
            PresencePreset::Full => layout.label_style = LabelStyle::Descriptive,
        }
        layout
    }

    pub fn normalize(&mut self) -> bool {
        let mut changed = false;
        let mut seen = HashSet::new();
        self.fields.retain(|item| {
            let keep = seen.insert(item.field);
            changed |= !keep;
            keep
        });
        for field in PresenceFieldId::ALL {
            if !seen.contains(&field) {
                let default = PresenceLayoutConfig::default()
                    .fields
                    .into_iter()
                    .find(|item| item.field == field)
                    .unwrap();
                self.fields.push(default);
                changed = true;
            }
        }
        changed
    }
}

#[derive(Debug, Clone, Default)]
pub struct PresenceValues(pub BTreeMap<PresenceFieldId, String>);

impl PresenceValues {
    pub fn insert(&mut self, field: PresenceFieldId, value: impl Into<String>) {
        let value = value.into();
        if !value.trim().is_empty() {
            self.0.insert(field, value);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceLines {
    pub details: String,
    pub state: String,
}

pub fn compose_presence(
    layout: &PresenceLayoutConfig,
    values: &PresenceValues,
    details_fallback: &str,
    state_fallback: &str,
) -> PresenceLines {
    let mut details = Vec::new();
    let mut state = Vec::new();
    for field in &layout.fields {
        if !field.enabled {
            continue;
        }
        let Some(value) = values.0.get(&field.field) else {
            continue;
        };
        let rendered = render_value(layout.label_style, field.field, value);
        match field.zone {
            PresenceZone::Details => details.push(rendered),
            PresenceZone::State => state.push(rendered),
        }
    }
    PresenceLines {
        details: compact_join(&details, details_fallback, " · ", 128),
        state: compact_join(&state, state_fallback, " • ", 128),
    }
}

fn render_value(style: LabelStyle, field: PresenceFieldId, value: &str) -> String {
    if style == LabelStyle::Compact {
        return value.to_string();
    }
    let label = match field {
        PresenceFieldId::Project => "Project",
        PresenceFieldId::Branch => "Branch",
        PresenceFieldId::Model => "Model",
        PresenceFieldId::Activity => "Activity",
        PresenceFieldId::Tokens => "Tokens",
        PresenceFieldId::Cost => "Cost",
        PresenceFieldId::Quotas => "Quotas",
        PresenceFieldId::Credits => "Credits",
        PresenceFieldId::Context => "Context",
        PresenceFieldId::Systems => "Systems",
    };
    let normalized_value = value.trim().to_ascii_lowercase();
    if normalized_value.starts_with(&label.to_ascii_lowercase()) {
        value.to_string()
    } else {
        format!("{label}: {value}")
    }
}

fn compact_join(parts: &[String], fallback: &str, separator: &str, limit: usize) -> String {
    let mut accepted: Vec<&str> = Vec::new();
    for part in parts {
        let candidate = if accepted.is_empty() {
            part.clone()
        } else {
            format!("{}{}{}", accepted.join(separator), separator, part)
        };
        if candidate.chars().count() <= limit {
            accepted.push(part.as_str());
        }
    }
    let value = if accepted.is_empty() {
        fallback.to_string()
    } else {
        accepted.join(separator)
    };
    truncate_chars(&value, limit)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut output: String = value.chars().take(limit.saturating_sub(1)).collect();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composer_honors_visibility_order_and_missing_values() {
        let mut layout = PresenceLayoutConfig::default();
        layout.fields.retain(|field| {
            matches!(
                field.field,
                PresenceFieldId::Model | PresenceFieldId::Credits
            )
        });
        let mut values = PresenceValues::default();
        values.insert(PresenceFieldId::Model, "GPT-5.6 Sol · Extra High · ⚡ Fast");
        values.insert(PresenceFieldId::Credits, "Credits 2,500");
        let lines = compose_presence(&layout, &values, "Coding", "Idle");
        assert_eq!(
            lines.state,
            "GPT-5.6 Sol · Extra High · ⚡ Fast • Credits 2,500"
        );
    }

    #[test]
    fn normalize_removes_duplicates_and_restores_missing_fields() {
        let mut layout = PresenceLayoutConfig {
            label_style: LabelStyle::Compact,
            fields: vec![
                PresenceFieldConfig {
                    field: PresenceFieldId::Model,
                    enabled: true,
                    zone: PresenceZone::State,
                },
                PresenceFieldConfig {
                    field: PresenceFieldId::Model,
                    enabled: false,
                    zone: PresenceZone::Details,
                },
            ],
        };
        assert!(layout.normalize());
        assert_eq!(layout.fields.len(), PresenceFieldId::ALL.len());
    }

    #[test]
    fn presets_control_density_and_descriptive_labels() {
        let minimal = PresenceLayoutConfig::for_preset(PresencePreset::Minimal);
        assert!(
            minimal
                .fields
                .iter()
                .find(|item| item.field == PresenceFieldId::Credits)
                .is_some_and(|item| !item.enabled)
        );
        let standard = PresenceLayoutConfig::for_preset(PresencePreset::Standard);
        assert!(
            standard
                .fields
                .iter()
                .find(|item| item.field == PresenceFieldId::Credits)
                .is_some_and(|item| item.enabled)
        );
        let full = PresenceLayoutConfig::for_preset(PresencePreset::Full);
        let mut values = PresenceValues::default();
        values.insert(PresenceFieldId::Model, "GPT-5.6 Sol");
        let lines = compose_presence(&full, &values, "Coding", "Idle");
        assert!(lines.state.contains("Model: GPT-5.6 Sol"));
    }

    #[test]
    fn composer_never_exceeds_discord_line_limit() {
        let layout = PresenceLayoutConfig::default();
        let mut values = PresenceValues::default();
        for field in PresenceFieldId::ALL {
            values.insert(field, "x".repeat(127));
        }
        let lines = compose_presence(&layout, &values, "Coding", "Idle");
        assert!(lines.details.chars().count() <= 128);
        assert!(lines.state.chars().count() <= 128);
    }
}
