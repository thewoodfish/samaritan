//! `InvestigationConstraints` and its parts — execution limits and scope
//! (`SCHEMA.md`, InvestigationConstraints). One vocabulary, used everywhere.

use serde::{Deserialize, Serialize};

use crate::common::{Id, Seconds, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvestigationConstraints {
    pub time: TimeWindow,
    /// Reference period, when the intent needs one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<TimeWindow>,
    pub spatial_scope: SpatialScope,
    /// Specific named entities, may be empty.
    pub entity_scope: Vec<EntityRef>,
    pub world_version: WorldVersion,
    pub priority: Priority,
    /// Deadline for the whole investigation.
    pub max_runtime: Seconds,
    pub operator: Id,
    pub organization: Id,
}

/// A resolved, absolute time window. Relative time is resolved once, in
/// Planning, and never again (`SCHEMA.md`, TimeWindow).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeWindow {
    /// Inclusive, UTC.
    pub start: Timestamp,
    /// Exclusive, UTC.
    pub end: Timestamp,
    /// Verbatim phrase that produced this window, e.g. "yesterday".
    pub resolved_from: String,
    /// Which shift calendar resolved it.
    pub calendar: Id,
    /// IANA timezone, e.g. "Africa/Lagos".
    pub timezone: String,
}

/// Which projection of the world the investigation was asked against. Pinning
/// this makes an investigation repeatable and disagreement legible
/// (`SCHEMA.md`, WorldVersion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldVersion {
    /// Event log sequence number.
    pub log_position: u64,
    /// The moment that position represents.
    pub as_of: Timestamp,
    /// Nearest snapshot, if one was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<Id>,
}

/// Recorded and propagated only; carries no scheduling semantics in this phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

/// Spatial scope of the investigation. `Unspecified` means the whole site and
/// must be recorded explicitly, so "did not narrow" differs from "never
/// considered" (`SCHEMA.md`, SpatialScope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialScope {
    pub kind: ScopeKind,
    /// Required unless `kind` is `Site` or `Unspecified`.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<Id>,
    /// Human readable, e.g. "Pit 3".
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScopeKind {
    Site,
    Area,
    Named,
    Unspecified,
}

/// A named entity the operator referred to. An unresolvable `EntityRef` is not
/// an error in Planning — the label is passed through (`SCHEMA.md`, EntityRef).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntityRef {
    /// From the subject vocabulary in `REGISTRY.md`.
    pub kind: String,
    /// Null when named but unresolvable.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<Id>,
    /// As the operator said it, e.g. "truck 14".
    pub label: String,
}
