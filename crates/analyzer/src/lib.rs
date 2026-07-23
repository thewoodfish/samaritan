//! # samaritan-analyzer
//!
//! Turns an `InvestigationPlan` into the `InformationRequirement`s one analyzer
//! needs — by walking the relation graph, not by knowing about mining.
//!
//! An analyzer here is a **view over the graph**: a name, the subjects and
//! intents it covers, and a target metric to seed the walk. All of that is
//! registry data. The strategy code ([`explain`]) contains no mining literal.
//!
//! Stage 5 implements the `Explain` strategy for a single analyzer, the thesis
//! test. The remaining strategies and analyzers arrive in stage 8.

mod explain;

use samaritan_graph::{NodeRef, RelationGraph};
use samaritan_registry::Registry;
use samaritan_schema::{InformationRequirement, IntentType, InvestigationPlan};

pub use explain::explain;

/// One analyzer, built from its registry declaration. Holds only its view of
/// the graph — the mining knowledge is in the graph itself.
#[derive(Debug, Clone)]
pub struct Analyzer {
    name: String,
    /// The metric that seeds a walk, if the analyzer declares one.
    target: Option<NodeRef>,
}

impl Analyzer {
    /// Build an analyzer from the registry by name. `None` if no such analyzer.
    pub fn from_registry(reg: &Registry, name: &str) -> Option<Analyzer> {
        let decl = reg.analyzer(name)?;
        let target = decl.metric.as_deref().and_then(NodeRef::parse);
        Some(Analyzer {
            name: decl.name.clone(),
            target,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The requirements this analyzer needs to serve `plan`. Empty is a
    /// legitimate outcome — an analyzer may conclude the question is not its
    /// concern (recorded as `empty` by dispatch, not a failure).
    pub fn requirements(
        &self,
        reg: &Registry,
        graph: &RelationGraph,
        plan: &InvestigationPlan,
    ) -> Vec<InformationRequirement> {
        let Some(target) = &self.target else {
            return Vec::new();
        };
        match plan.intent.kind {
            IntentType::Explain => explain(reg, graph, plan, &self.name, target),
            // Other strategies arrive in stage 8.
            _ => Vec::new(),
        }
    }
}
