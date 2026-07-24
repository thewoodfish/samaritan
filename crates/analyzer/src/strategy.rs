//! The six generic strategies, each a walk over the relation graph. No mining
//! knowledge lives here — every subject, observable, and edge comes from the
//! analyzer's target metric, the graph, and the registry.
//!
//! - Explain   — establish → decompose → confound
//! - Compare   — establish (window vs baseline) → decompose
//! - Locate    — rank the subject by the metric, grouped by an entity
//! - Recommend — explain, then rank the worst offender to act on
//! - Predict   — establish the trend, then its upstream drivers
//! - Summarize — one coarse aggregate over the scope

use std::collections::BTreeSet;

use chrono::Duration;

use samaritan_graph::{NodeRef, RelationGraph};
use samaritan_registry::Registry;
use samaritan_schema::{
    Aggregation, AggregationOp, Direction, Granularity, Id, InformationRequirement,
    InvestigationPlan, Necessity, Ordering, Seconds, Shape,
};

/// Dispatch to a strategy by intent. `target` is the analyzer's seed metric.
pub fn requirements(
    reg: &Registry,
    graph: &RelationGraph,
    plan: &InvestigationPlan,
    analyzer: &str,
    target: &NodeRef,
) -> Vec<InformationRequirement> {
    use samaritan_schema::IntentType::*;
    let mut ctx = Ctx {
        plan,
        analyzer,
        seq: 0,
    };
    match plan.intent.kind {
        Explain => explain(&mut ctx, reg, graph, target),
        Compare => compare(&mut ctx, graph, target),
        Locate => locate(&mut ctx, graph, target),
        Recommend => recommend(&mut ctx, reg, graph, target),
        Predict => predict(&mut ctx, graph, target),
        Summarize => summarize(&mut ctx, target),
    }
}

// ---- strategies -----------------------------------------------------------

fn explain(
    ctx: &mut Ctx,
    reg: &Registry,
    graph: &RelationGraph,
    target: &NodeRef,
) -> Vec<InformationRequirement> {
    let mut reqs = vec![establish(ctx, graph, target)];
    if let Some(dec) = graph.decomposition(target) {
        reqs.extend(decompose(ctx, target, &dec.parts));
    }
    for c in confounders(graph, target) {
        if let Some(r) = confound(ctx, reg, &c) {
            reqs.push(r);
        }
    }
    reqs
}

fn compare(ctx: &mut Ctx, graph: &RelationGraph, target: &NodeRef) -> Vec<InformationRequirement> {
    // The window/baseline split is the comparison; establish carries both.
    let mut reqs = vec![establish(ctx, graph, target)];
    if let Some(dec) = graph.decomposition(target) {
        reqs.extend(decompose(ctx, target, &dec.parts));
    }
    reqs
}

fn locate(ctx: &mut Ctx, graph: &RelationGraph, target: &NodeRef) -> Vec<InformationRequirement> {
    // Rank entities by the metric; fall back to a plain establish if the
    // metric has no diagnostic grouping.
    match partition_rank(ctx, graph, target) {
        Some(r) => vec![r],
        None => vec![establish(ctx, graph, target)],
    }
}

fn recommend(
    ctx: &mut Ctx,
    reg: &Registry,
    graph: &RelationGraph,
    target: &NodeRef,
) -> Vec<InformationRequirement> {
    // Find the causes, then rank the worst offender to act on.
    let mut reqs = explain(ctx, reg, graph, target);
    if let Some(r) = partition_rank(ctx, graph, target) {
        reqs.push(r);
    }
    reqs
}

fn predict(ctx: &mut Ctx, graph: &RelationGraph, target: &NodeRef) -> Vec<InformationRequirement> {
    // The trend, plus the upstream drivers that forecast it.
    let mut reqs = vec![establish(ctx, graph, target)];
    for edge in graph.upstream(target) {
        reqs.push(driver(ctx, &edge.from, edge.widening()));
    }
    reqs
}

fn summarize(ctx: &mut Ctx, target: &NodeRef) -> Vec<InformationRequirement> {
    let mut req = ctx.base(target.subject.clone());
    req.purpose = format!(
        "Summarize {} across the requested scope.",
        target.qualified()
    );
    req.granularity = Granularity::Shift;
    req.expected_shape = Shape::Table;
    req.observables = vec![target.field.clone()];
    req.aggregations = vec![
        agg(AggregationOp::Mean, Some(&target.field)),
        agg(AggregationOp::Sum, Some(&target.field)),
        agg(AggregationOp::Count, None),
    ];
    vec![req]
}

// ---- steps ----------------------------------------------------------------

/// Establish whether the metric changed. Widens the window backward by the
/// longest persistence of the metric's outgoing influences — the mechanism by
/// which the environment analyzer asks about rain from before the shift began.
fn establish(ctx: &mut Ctx, graph: &RelationGraph, target: &NodeRef) -> InformationRequirement {
    let widening = graph
        .downstream(target)
        .iter()
        .map(|e| e.persistence)
        .max()
        .unwrap_or(0);

    let mut req = ctx.base(target.subject.clone());
    req.observables = vec![target.field.clone()];
    req.aggregations = vec![
        agg(AggregationOp::Mean, Some(&target.field)),
        agg(AggregationOp::P95, Some(&target.field)),
        agg(AggregationOp::Count, None),
    ];
    if widening > 0 {
        req.window.start -= Duration::seconds(widening as i64);
        req.window.resolved_from = format!(
            "{} (widened {widening}s for persistence)",
            req.window.resolved_from
        );
        req.purpose = format!(
            "Establish {} over the window, widened to capture its lingering effect.",
            target.qualified()
        );
    } else {
        req.purpose = format!(
            "Establish whether {} changed in the window relative to the baseline.",
            target.qualified()
        );
    }
    req
}

/// Decompose the metric into its parts — one requirement per distinct
/// part-subject.
fn decompose(ctx: &mut Ctx, target: &NodeRef, parts: &[NodeRef]) -> Vec<InformationRequirement> {
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

/// The confounders of a target: its direct confounders plus the sibling causes
/// of whatever it influences.
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

/// A requirement to rule out one confounder. Only observable confounders yield
/// one; an attribute confounder is a stratification hint for later stages.
fn confound(ctx: &mut Ctx, reg: &Registry, confounder: &NodeRef) -> Option<InformationRequirement> {
    let subject = reg.config().subjects.get(&confounder.subject)?;
    let spec = subject.observables.get(&confounder.field)?;

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

/// Rank the subject by the metric, grouped by the first diagnostic partition
/// key. `None` if the metric has no declared partitions.
fn partition_rank(
    ctx: &mut Ctx,
    graph: &RelationGraph,
    target: &NodeRef,
) -> Option<InformationRequirement> {
    let key = graph.partition_keys(target).first()?.clone();
    let mut req = ctx.base(target.subject.clone());
    req.purpose = format!(
        "Rank {} by {} to locate the worst {key}.",
        target.subject, target.field
    );
    req.observables = vec![target.field.clone()];
    req.group_by = vec![key];
    req.aggregations = vec![agg(AggregationOp::Mean, Some(&target.field))];
    req.ordering = Some(Ordering {
        by: target.field.clone(),
        direction: Direction::Desc,
    });
    req.limit = Some(5);
    req.expected_shape = Shape::Set;
    Some(req)
}

/// A requirement for one upstream driver, its window widened by the influence's
/// lag + persistence.
fn driver(ctx: &mut Ctx, from: &NodeRef, widening: u64) -> InformationRequirement {
    let mut req = ctx.base(from.subject.clone());
    req.purpose = format!(
        "Track {} as an upstream driver of the forecast.",
        from.qualified()
    );
    req.necessity = Necessity::Preferred;
    req.observables = vec![from.field.clone()];
    req.aggregations = vec![agg(AggregationOp::Mean, Some(&from.field))];
    if widening > 0 {
        req.window.start -= Duration::seconds(widening as i64);
    }
    req
}

// ---- context and helpers --------------------------------------------------

struct Ctx<'a> {
    plan: &'a InvestigationPlan,
    analyzer: &'a str,
    seq: u32,
}

impl Ctx<'_> {
    fn next_id(&mut self) -> Id {
        self.seq += 1;
        // Include the analyzer so ids are unique within a RequirementSet — two
        // analyzers both number from 1, so plan + seq alone would collide.
        let core = self
            .plan
            .id
            .0
            .strip_prefix("plan_")
            .unwrap_or(&self.plan.id.0);
        Id(format!("req_{core}_{}_{:04}", self.analyzer, self.seq))
    }

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
            group_by: Vec::new(),
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
