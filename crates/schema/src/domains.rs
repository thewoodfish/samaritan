//! `OperationalDomains` — the business areas involved, ranked
//! (`SCHEMA.md`, OperationalDomains).

use serde::{Deserialize, Serialize};

use crate::common::Confidence;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalDomains {
    /// Ordered, at least one. Rank strictly increasing from 1.
    pub domains: Vec<RankedDomain>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RankedDomain {
    pub domain: DomainType,
    /// 1 = most relevant, strictly increasing.
    pub rank: u32,
    pub confidence: Confidence,
    pub rationale: String,
}

/// The closed, registry-versioned set of operational domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainType {
    OperationalPerformance,
    Production,
    Equipment,
    MaterialFlow,
    Infrastructure,
    Personnel,
    Safety,
    Security,
    Environment,
    Logistics,
}
