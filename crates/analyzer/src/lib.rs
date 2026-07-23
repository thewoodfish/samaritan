//! # samaritan-analyzer
//!
//! Turns an `InvestigationPlan` into the `InformationRequirement`s one analyzer
//! needs — by walking the relation graph, not by knowing about mining.
//!
//! An analyzer here is a **view over the graph**: a name, the subjects and
//! intents it covers, and a target metric to seed the walk. All of that is
//! registry data. The strategy code ([`explain`]) contains no mining literal.
//!
//! The [`Analyzer`] trait is the contract dispatch runs against — real graph
//! analyzers and, in tests, doubles that are slow, empty, or failing. Stage 5
//! implemented the `Explain` strategy for one analyzer (the thesis test); the
//! remaining strategies arrive in stage 8.

mod explain;

use samaritan_graph::{NodeRef, RelationGraph};
use samaritan_registry::Registry;
use samaritan_schema::{InformationRequirement, IntentType, InvestigationPlan};

pub use explain::explain;

/// The contract every analyzer implements. `Send + Sync` so dispatch can run a
/// set of them in parallel. `run` returns `Err` for a genuine failure; an empty
/// `Ok(vec![])` is a legitimate "not my concern".
pub trait Analyzer: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn run(
        &self,
        reg: &Registry,
        graph: &RelationGraph,
        plan: &InvestigationPlan,
    ) -> Result<Vec<InformationRequirement>, String>;
}

/// An analyzer built from its registry declaration. Holds only its view of the
/// graph — the mining knowledge is in the graph itself.
#[derive(Debug, Clone)]
pub struct GraphAnalyzer {
    name: String,
    version: String,
    /// The metric that seeds a walk, if the analyzer declares one.
    target: Option<NodeRef>,
}

impl GraphAnalyzer {
    /// Build an analyzer from the registry by name. `None` if no such analyzer.
    pub fn from_registry(reg: &Registry, name: &str) -> Option<GraphAnalyzer> {
        let decl = reg.analyzer(name)?;
        let target = decl.metric.as_deref().and_then(NodeRef::parse);
        Some(GraphAnalyzer {
            name: decl.name.clone(),
            version: decl.version.clone(),
            target,
        })
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

impl Analyzer for GraphAnalyzer {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn run(
        &self,
        reg: &Registry,
        graph: &RelationGraph,
        plan: &InvestigationPlan,
    ) -> Result<Vec<InformationRequirement>, String> {
        // A graph walk does not fail; it may legitimately produce nothing.
        Ok(self.requirements(reg, graph, plan))
    }
}
