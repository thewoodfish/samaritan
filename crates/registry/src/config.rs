//! The registry config, as it appears on disk (YAML). These types mirror
//! `mining.yaml` one-to-one and carry no validation — validation runs over
//! them in [`crate::validate`].
//!
//! Unknown top-level keys are ignored, so the `relations` block (consumed by
//! the graph crate in stage 3) passes through without a parse error here.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The whole registry as parsed. Field order is irrelevant; validation gives
/// it meaning.
#[derive(Debug, Clone, Deserialize)]
pub struct RegistryConfig {
    pub registry_version: String,
    pub intents: Vec<String>,
    pub strategies: BTreeMap<String, StrategyConfig>,
    pub domains: Vec<String>,
    pub analyzers: Vec<AnalyzerConfig>,
    pub subjects: BTreeMap<String, SubjectConfig>,
    pub enumerations: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub derived_observables: BTreeMap<String, DerivedObservableConfig>,
    pub zones: Vec<ZoneConfig>,
    pub sites: Vec<SiteConfig>,
    pub calendars: BTreeMap<String, Vec<CalendarVersionConfig>>,
    pub baseline_defaults: BTreeMap<String, BaselineDefault>,
    pub window_required: BTreeMap<String, bool>,
    pub thresholds: ThresholdsConfig,
    /// The relation graph. Parsed via the graph crate; validated here (E18–E23,
    /// W07–W09) against the vocabulary above.
    #[serde(default)]
    pub relations: samaritan_graph::RelationsConfig,
    // `models` is intentionally not modelled in stage 2 — it is exercised in
    // stage 6. Unknown keys are ignored (no deny_unknown_fields).
}

#[derive(Debug, Clone, Deserialize)]
pub struct StrategyConfig {
    pub goal: String,
    pub expected_output: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalyzerConfig {
    pub name: String,
    pub version: String,
    pub domains: Vec<String>,
    pub intents: Vec<String>,
    pub subjects: Vec<String>,
    /// The primary metric the analyzer investigates, `subject.observable`. The
    /// seed of a graph walk — the analyzer's view, not code. Optional so an
    /// analyzer with no single target still parses.
    #[serde(default)]
    pub metric: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubjectConfig {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub spatial: bool,
    pub observables: BTreeMap<String, ObservableConfig>,
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeConfig>,
}

/// An observable spec: `{ type, unit }` or `{ type: ratio, definition }`.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservableConfig {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default)]
    pub unit: Option<String>,
    /// Present on ratios: `available_time / scheduled_time`.
    #[serde(default)]
    pub definition: Option<String>,
}

/// An attribute spec: `{ type }` or `{ type: enum, of: EnumName }`.
#[derive(Debug, Clone, Deserialize)]
pub struct AttributeConfig {
    #[serde(rename = "type")]
    pub ty: String,
    /// The enumeration named, when `ty == "enum"`.
    #[serde(default)]
    pub of: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DerivedObservableConfig {
    #[serde(default)]
    pub derived_from: Vec<String>,
    #[serde(default)]
    pub emits: Option<String>,
    #[serde(default)]
    pub definition: String,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ZoneConfig {
    /// The engine entity id. Optional in the type so a missing one is a
    /// validation error (E09), not a parse error.
    #[serde(default)]
    pub entity: Option<String>,
    pub key: String,
    pub label: String,
    pub operational_role: String,
    #[serde(default)]
    pub excluded_from_productivity: bool,
    #[serde(default)]
    pub restricted: bool,
    #[serde(default)]
    pub restricted_to: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SiteConfig {
    pub id: String,
    pub name: String,
    pub organization: String,
    pub timezone: String,
    pub calendar_family: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CalendarVersionConfig {
    pub version: String,
    pub effective_from: String,
    /// `null` for the currently-in-force version.
    #[serde(default)]
    pub effective_until: Option<String>,
    pub operational_day_starts: String,
    pub shifts: Vec<ShiftConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShiftConfig {
    pub name: String,
    pub start: String,
    /// Duration in seconds.
    pub duration: u64,
}

/// A default reference period for an intent. `none` (a bare string) means the
/// intent supplies no default; otherwise a trailing operational-day window.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum BaselineDefault {
    Trailing { trailing_operational_days: u32 },
    Keyword(String),
}

impl BaselineDefault {
    /// The trailing-day count, if this is a real default.
    pub fn trailing_days(&self) -> Option<u32> {
        match self {
            BaselineDefault::Trailing {
                trailing_operational_days,
            } => Some(*trailing_operational_days),
            BaselineDefault::Keyword(_) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdsConfig {
    pub intent_confidence_floor: f64,
    pub domain_relevance_floor: f64,
    pub max_domains: u32,
    pub max_analyzers: u32,
    pub max_relation_depth: u32,
    pub default_max_runtime: u64,
    pub default_priority: String,
}
