//! `InvestigationStrategy` — how the investigation proceeds (`SCHEMA.md`,
//! InvestigationStrategy). Derived by registry lookup keyed on intent, 1:1,
//! never a second inference step.

use serde::{Deserialize, Serialize};

use crate::intent::IntentType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvestigationStrategy {
    #[serde(rename = "type")]
    pub kind: IntentType,
    pub goal: String,
    pub expected_output: String,
}
