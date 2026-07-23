//! `Question` — the raw operator request, the root of all provenance
//! (`SCHEMA.md`, Question).

use serde::{Deserialize, Serialize};

use crate::common::{Id, SchemaVersion, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub id: Id,
    pub schema_version: SchemaVersion,
    pub created_at: Timestamp,

    /// Verbatim operator text. Never altered — normalization writes a new
    /// field on a new schema.
    pub text: String,
    /// The resolution clock for relative time. An input, never `now()`.
    pub asked_at: Timestamp,
    pub operator: Id,
    pub organization: Id,
    /// Resolves timezone and shift calendar. Required — "yesterday" is
    /// meaningless without it.
    pub site: Id,
    /// BCP-47, default "en".
    pub locale: String,
}
