//! `InvestigationPlan` — the planning contract, where everything downstream
//! begins (`SCHEMA.md`, InvestigationPlan). Carries no evidence.

use serde::{Deserialize, Serialize};

use crate::common::{Id, SchemaVersion, Timestamp};
use crate::constraints::InvestigationConstraints;
use crate::domains::{DomainType, OperationalDomains};
use crate::intent::Intent;
use crate::strategy::InvestigationStrategy;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvestigationPlan {
    pub id: Id,
    pub schema_version: SchemaVersion,
    pub created_at: Timestamp,

    pub question_id: Id,
    /// Normalized question text.
    pub question_text: String,
    pub intent: Intent,
    pub domains: OperationalDomains,
    pub strategy: InvestigationStrategy,
    pub analyzers: Vec<AnalyzerRef>,
    pub constraints: InvestigationConstraints,
    pub provenance: PlanProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalyzerRef {
    /// Registry key.
    pub name: String,
    pub version: SchemaVersion,
    /// Which matched domains selected this analyzer.
    pub domains: Vec<DomainType>,
    pub rationale: String,
}

/// Proof of which pipeline produced this plan. Exists because Planning is
/// reproducible, not deterministic (`SCHEMA.md`, PlanProvenance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanProvenance {
    pub model_id: String,
    pub prompt_template_version: String,
    pub registry_version: SchemaVersion,
    pub cache_hit: bool,
}
