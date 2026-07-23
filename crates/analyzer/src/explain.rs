//! The generic `Explain` strategy, as a walk over the relation graph.
//!
//! There is no mining knowledge in this file — no subject, observable, or
//! confounder is named. Everything comes from the analyzer's target metric,
//! the graph's edges, and the registry's vocabulary. That is the thesis: an
//! expert investigation expressed as a graph traversal.
//!
//! The walk, from a target metric M:
//!
//! - establish — did M change? (M over window + baseline)
//! - decompose — which part of M grew? (M's decomposition parts)
//! - confound — is something else to blame? (M's confounders and the sibling
//!   causes of what M influences)

use std::collections::BTreeSet;

use samaritan_graph::{NodeRef, RelationGraph};
use samaritan_registry::Registry;
use samaritan_schema::{
    Aggregation, AggregationOp, Granularity, Id, InformationRequirement, InvestigationPlan,
    Necessity, Seconds, Shape,
};

/// Emit the requirements for an `Explain` investigation of `target`.
pub fn explain(
    reg: &Registry,
    graph: &RelationGraph,
    plan: &InvestigationPlan,
    analyzer: &str,
    target: &NodeRef,
) -> Vec<InformationRequirement> {
    let mut ctx = Ctx {
        plan,
        analyzer,
        seq: 0,
    };
    let mut reqs = Vec::new();

    reqs.push(establish(&mut ctx, target));

    if let Some(dec) = graph.decomposition(target) {
        reqs.extend(decompose(&mut ctx, target, &dec.parts));
    }

    for confounder in confounders(graph, target) {
        if let Some(req) = confound(&mut ctx, reg, target, &confounder) {
            reqs.push(req);
        }
    }

    reqs
}

/// Per-walk state: the plan being served and a counter for deterministic ids.
struct Ctx<'a> {
    plan: &'a InvestigationPlan,
    analyzer: &'a str,
    seq: u32,
}

impl Ctx<'_> {
    fn next_id(&mut self) -> Id {
        self.seq += 1;
        Id(format!("req_{}_{:04}", self.plan.id.0, self.seq))
    }

    /// A requirement pre-filled with everything drawn from the plan: id,
    /// plan_id, requester, the window/baseline/scope/entities, round 0.
    fn base(&mut self, subject: String) -> InformationRequirement {
        let c = &self.plan.constraints;
        InformationRequirement {
            id: self.next_id(),
            plan_id: self.plan.id.clone(),
            requested_by: vec![self.analyzer.to_owned()],
            purpose: String::new(),
            necessity: Necessity::Required,
            subject,
            observables: Vec::new(),
            filters: Vec::new(),
            spatial: Vec::new(),
            window: c.time.clone(),
            baseline: c.baseline.clone(),
            scope: c.spatial_scope.clone(),
            entities: c.entity_scope.clone(),
            granularity: Granularity::PerEvent,
            aggregations: Vec::new(),
            ordering: None,
            limit: None,
            expected_shape: Shape::Series,
            round: 0,
            depends_on: Vec::new(),
        }
    }
}

fn agg(op: AggregationOp, field: Option<&str>) -> Aggregation {
    Aggregation {
        op,
        field: field.map(str::to_owned),
        bins: None,
    }
}

/// Step 1 — establish whether the metric changed.
fn establish(ctx: &mut Ctx, target: &NodeRef) -> InformationRequirement {
    let mut req = ctx.base(target.subject.clone());
    req.purpose = format!(
        "Establish whether {} changed in the window relative to the baseline.",
        target.qualified()
    );
    req.observables = vec![target.field.clone()];
    req.aggregations = vec![
        agg(AggregationOp::Mean, Some(&target.field)),
        agg(AggregationOp::P95, Some(&target.field)),
        agg(AggregationOp::Count, None),
    ];
    req
}

/// Step 2 — decompose the metric into its parts. One requirement per distinct
/// part-subject (parts of an additive whole share the whole's subject; a
/// multiplicative whole may reach another subject).
fn decompose(ctx: &mut Ctx, target: &NodeRef, parts: &[NodeRef]) -> Vec<InformationRequirement> {
    // Group parts by subject, preserving first-seen order.
    let mut subjects: Vec<String> = Vec::new();
    for p in parts {
        if !subjects.contains(&p.subject) {
            subjects.push(p.subject.clone());
        }
    }

    subjects
        .into_iter()
        .map(|subject| {
            let fields: Vec<String> = parts
                .iter()
                .filter(|p| p.subject == subject)
                .map(|p| p.field.clone())
                .collect();
            let mut req = ctx.base(subject);
            req.purpose = format!(
                "Decompose {} into its parts to locate which grew.",
                target.qualified()
            );
            req.granularity = Granularity::Bucketed {
                bucket_size: Seconds(3600.0),
            };
            req.aggregations = fields
                .iter()
                .map(|f| agg(AggregationOp::Mean, Some(f)))
                .collect();
            req.observables = fields;
            req
        })
        .collect()
}

/// The confounders of a target: its direct confounders, plus the sibling
/// causes of whatever it influences — other explanations for the same outcome.
fn confounders(graph: &RelationGraph, target: &NodeRef) -> Vec<NodeRef> {
    let mut set: BTreeSet<NodeRef> = BTreeSet::new();
    for c in graph.confounders_of(target) {
        set.insert(c.factor.clone());
    }
    for edge in graph.downstream(target) {
        for cause in graph.upstream(&edge.to) {
            if cause.from != *target {
                set.insert(cause.from.clone());
            }
        }
    }
    set.into_iter().collect()
}

/// Step 3 — a requirement to rule out one confounder. Only observable
/// confounders yield a requirement; an attribute confounder (a mix that may
/// have shifted) is a stratification hint, not a fetch, and is left to later
/// stages.
fn confound(
    ctx: &mut Ctx,
    reg: &Registry,
    _target: &NodeRef,
    confounder: &NodeRef,
) -> Option<InformationRequirement> {
    let subject = reg.config().subjects.get(&confounder.subject)?;
    let spec = subject.observables.get(&confounder.field)?; // skip attribute confounders

    // A ratio confounder pulls in its definition's terms, so the ratio can be
    // seen decomposed.
    let mut observables = vec![confounder.field.clone()];
    if let Some(def) = &spec.definition {
        for term in identifiers(def) {
            if subject.observables.contains_key(&term) && !observables.contains(&term) {
                observables.push(term);
            }
        }
    }

    let mut req = ctx.base(confounder.subject.clone());
    req.purpose = format!(
        "Rule out {} as an alternative explanation.",
        confounder.qualified()
    );
    req.necessity = Necessity::Preferred;
    req.granularity = Granularity::Shift;
    req.expected_shape = Shape::Table;
    req.aggregations = observables
        .iter()
        .map(|f| agg(AggregationOp::Mean, Some(f)))
        .collect();
    req.observables = observables;
    Some(req)
}

/// Identifier-like tokens in a ratio definition (`available_time`,
/// `scheduled_time` from `available_time / scheduled_time`).
fn identifiers(def: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in def.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
