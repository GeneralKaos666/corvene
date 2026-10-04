//! The Flags screen: every registered flag with its definition, current
//! value and preset, as Settings › Flags shows them on the desktop.

use corvene_core::AppState;
use corvene_core::flags::{Category, Kind, Nature, Value};

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct FlagOptionVm {
    pub value: String,
    pub label: String,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct FlagVm {
    /// `201-commit-templates`
    pub ident: String,
    pub slug: String,
    pub id: u16,
    pub title: String,
    pub summary: String,
    pub ghd_behaviour: String,
    pub category: String,
    /// "Feature" or "Bug fix".
    pub nature: String,
    /// "toggle", "select", "number" or "text".
    pub kind: String,
    pub options: Vec<FlagOptionVm>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub unit: Option<String>,
    /// The current value as text ("true"/"false", a number, or the text).
    pub value: String,
    pub value_label: String,
    /// The value differs from GHD's (the deviation is active).
    pub is_on: bool,
    /// The user set it (not the preset's value).
    pub overridden: bool,
    pub preset_value: String,
    pub ghd_value: String,
    pub restart: bool,
    pub restart_pending: bool,
    pub available: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PresetVm {
    pub slug: String,
    pub title: String,
    pub description: String,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct FlagsVm {
    pub preset: String,
    pub presets: Vec<PresetVm>,
    pub categories: Vec<String>,
    pub flags: Vec<FlagVm>,
}

fn text_of(value: &Value) -> String {
    match value {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Text(t) => t.to_string(),
    }
}

pub fn flags(s: &AppState) -> FlagsVm {
    use corvene_core::flags::Preset;
    let flags = &s.flags;
    let preset = flags.preset();
    let pending = flags.restart_pending(&s.flags_at_launch);
    let defs = corvene_core::flags::REGISTRY
        .iter()
        .filter(|def| def.visible)
        .map(|def| {
            let value = flags.value(def.id);
            let (options, min, max, unit) = match &def.kind {
                Kind::Select { options } => (
                    options
                        .iter()
                        .map(|o| FlagOptionVm {
                            value: o.value.to_string(),
                            label: o.label.to_string(),
                        })
                        .collect(),
                    None,
                    None,
                    None,
                ),
                Kind::Number { min, max, unit } => {
                    (Vec::new(), Some(*min), Some(*max), unit.map(str::to_string))
                }
                Kind::Bool | Kind::Text { .. } => (Vec::new(), None, None, None),
            };
            let preset_value = match preset {
                Preset::GitHubDesktop => &def.ghd,
                Preset::Familiar => &def.familiar,
                Preset::Corvene => &def.corvene,
                Preset::Max => &def.max,
            };
            FlagVm {
                ident: def.ident(),
                slug: def.slug.to_string(),
                id: def.id.0,
                title: def.title.to_string(),
                summary: def.summary.to_string(),
                ghd_behaviour: def.ghd_behaviour.to_string(),
                category: def.category().title().to_string(),
                nature: match def.nature {
                    Nature::Feature => "Feature".into(),
                    Nature::BugFix => "Bug fix".into(),
                },
                kind: def.kind.name().to_string(),
                options,
                min,
                max,
                unit,
                value: text_of(value),
                value_label: def.label_for(value),
                is_on: def.is_on(value),
                overridden: flags.is_overridden(def.id),
                preset_value: text_of(preset_value),
                ghd_value: text_of(&def.ghd),
                restart: def.restart,
                restart_pending: pending.contains(&def.id),
                available: flags.is_available(def.id),
            }
        })
        .collect();
    FlagsVm {
        preset: preset.slug().to_string(),
        presets: [
            Preset::GitHubDesktop,
            Preset::Familiar,
            Preset::Corvene,
            Preset::Max,
        ]
        .into_iter()
        .map(|p| PresetVm {
            slug: p.slug().to_string(),
            title: p.title().to_string(),
            description: p.description().to_string(),
        })
        .collect(),
        categories: Category::ALL
            .iter()
            .map(|c| c.title().to_string())
            .collect(),
        flags: defs,
    }
}
