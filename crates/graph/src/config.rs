//! The relations block, as it appears in the registry YAML. Raw strings; the
//! typed [`crate::RelationGraph`] is built from these, and the registry
//! validates them against its vocabulary.

use serde::Deserialize;

/// The `relations:` block. Every list defaults to empty so a registry without
/// relations still parses.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RelationsConfig {
    #[serde(default)]
    pub decomposes: Vec<DecomposeConfig>,
    #[serde(default)]
    pub partitions: Vec<PartitionConfig>,
    #[serde(default)]
    pub confounds: Vec<ConfoundConfig>,
    #[serde(default)]
    pub influences: Vec<InfluenceConfig>,
    #[serde(default)]
    pub rolls_up: Vec<RollsUpConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecomposeConfig {
    pub whole: String,
    /// Parsed and validated as a mode; kept as a string so a missing or
    /// invalid value is a validation finding (E19), not a parse error.
    #[serde(default)]
    pub mode: Option<String>,
    pub parts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartitionConfig {
    pub metric: String,
    pub by: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfoundConfig {
    pub factor: String,
    pub affects: String,
    #[serde(default)]
    pub conditional: Option<String>,
    #[serde(default)]
    pub why: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InfluenceConfig {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub lag: Option<u64>,
    #[serde(default)]
    pub persistence: Option<u64>,
    #[serde(default)]
    pub why: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RollsUpConfig {
    pub from: String,
    pub to: String,
}
