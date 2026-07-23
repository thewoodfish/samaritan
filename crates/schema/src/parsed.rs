//! `ParsedQuestion` — the result of validation and normalization
//! (`SCHEMA.md`, ParsedQuestion). Four states, not a boolean.

use serde::{Deserialize, Serialize};

use crate::common::{Confidence, Id, SchemaVersion, Timestamp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedQuestion {
    pub id: Id,
    pub schema_version: SchemaVersion,
    pub created_at: Timestamp,

    pub question_id: Id,
    pub status: ValidationStatus,
    /// Present only when `status == Valid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_question: Option<String>,
    pub confidence: Confidence,
    /// BCP-47, detected.
    pub language: String,
    /// Required when `status != Valid`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// What would make the question answerable. Specific enough to act on.
    pub missing: Vec<String>,
}

/// The four outcomes of validation. `Invalid` is terminal; `Ambiguous` and
/// `Incomplete` are re-askable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// Suitable for investigation.
    Valid,
    /// Not an operational question at all.
    Invalid,
    /// Operational, but could mean several different investigations.
    Ambiguous,
    /// Operational and unambiguous, but missing required context.
    Incomplete,
}
