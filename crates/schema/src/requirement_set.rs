//! `RequirementSet` — the terminal artifact of this phase (`SCHEMA.md`,
//! RequirementSet). Also the specification of the engine's request API.

use serde::{Deserialize, Serialize};

use crate::common::{Id, SchemaVersion, Seconds, Timestamp};
use crate::constraints::WorldVersion;
use crate::requirement::{InformationRequirement, Necessity};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementSet {
    pub id: Id,
    pub schema_version: SchemaVersion,
    pub created_at: Timestamp,

    pub plan_id: Id,
    pub question_id: Id,
    /// Every requirement reads this world.
    pub world_version: WorldVersion,
    /// Deduplicated, in normalized order.
    pub requirements: Vec<InformationRequirement>,
    pub unserviceable: Vec<Unserviceable>,
    pub execution: ExecutionReport,
    /// True only when every analyzer completed and every requirement is
    /// serviceable. A partial set is valid but never presented as whole.
    pub complete: bool,
}

/// A well-formed requirement the engine cannot answer. Recorded, never
/// dropped — unavailable is not empty (`ENGINE.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unserviceable {
    pub requirement_id: Id,
    pub requested_by: Vec<String>,
    pub necessity: Necessity,
    pub reason: UnserviceableReason,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnserviceableReason {
    UnavailableSubject,
    UnavailableObservable,
    UnregisteredTerm,
    UnknownZone,
}

/// The fate of every analyzer that ran. The four outcomes are distinct and
/// never collapsed (`SCHEMA.md`, ExecutionReport).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReport {
    pub completed: Vec<AnalyzerOutcome>,
    /// Ran, legitimately needed nothing.
    pub empty: Vec<AnalyzerOutcome>,
    pub failed: Vec<AnalyzerOutcome>,
    pub timed_out: Vec<AnalyzerOutcome>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzerOutcome {
    pub analyzer: String,
    pub version: SchemaVersion,
    pub duration: Seconds,
    pub requirement_count: u32,
    /// Required when this outcome is a failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
