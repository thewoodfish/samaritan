//! `Intent` — the operator's objective (`SCHEMA.md`, Intent). Six types,
//! exactly one primary per question.

use serde::{Deserialize, Serialize};

use crate::common::Confidence;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    #[serde(rename = "type")]
    pub kind: IntentType,
    pub confidence: Confidence,
    /// Why this classification, for explainability. An intent without a
    /// stated reason is not explainable and fails review.
    pub rationale: String,
}

/// The closed set of intents. `Investigate` was removed — it was
/// indistinguishable from `Explain`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntentType {
    /// Why did something happen.
    Explain,
    /// How do two things differ.
    Compare,
    /// Where is something / which thing is it.
    Locate,
    /// What should be done.
    Recommend,
    /// What will happen.
    Predict,
    /// What is the overall state.
    Summarize,
}
