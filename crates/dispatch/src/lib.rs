//! # samaritan-dispatch
//!
//! Fans the `InvestigationPlan` out to its analyzers, runs them in parallel
//! against one pinned `world_version`, and collects a single `RequirementSet`
//! — deduplicated, ordered, with a truthful execution report.
//!
//! Orchestration only. Dispatch performs no reasoning and never alters the
//! semantic content of a requirement. It may merge two identical requirements,
//! and it records which requirements cannot be served — but unavailable is
//! never presented as empty (`PIPELINE.md`, Stage 2; `ENGINE.md`).

mod dedup;
mod validate;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use samaritan_analyzer::{Analyzer, GraphAnalyzer};
use samaritan_graph::RelationGraph;
use samaritan_registry::Registry;
use samaritan_schema::{
    AnalyzerOutcome, ExecutionReport, Id, InformationRequirement, InvestigationPlan,
    RequirementSet, SchemaVersion, Seconds, Unserviceable,
};

/// Build the analyzers a plan names, from the registry.
pub fn analyzers_for_plan(reg: &Registry, plan: &InvestigationPlan) -> Vec<Arc<dyn Analyzer>> {
    plan.analyzers
        .iter()
        .filter_map(|a| GraphAnalyzer::from_registry(reg, &a.name))
        .map(|g| Arc::new(g) as Arc<dyn Analyzer>)
        .collect()
}

/// What one analyzer produced, tagged for the execution report.
enum Outcome {
    Completed(Vec<InformationRequirement>),
    Failed(String),
}

/// Fan out the plan to `analyzers`, honour the plan's `max_runtime` deadline,
/// and collect one `RequirementSet`. Analyzers run on detached threads; any
/// that miss the deadline are abandoned (their partial output discarded) and
/// reported as timed out.
pub fn dispatch(
    reg: Arc<Registry>,
    graph: Arc<RelationGraph>,
    plan: Arc<InvestigationPlan>,
    analyzers: Vec<Arc<dyn Analyzer>>,
) -> RequirementSet {
    let deadline = Duration::from_secs_f64(plan.constraints.max_runtime.0.max(0.0));
    let (tx, rx) = mpsc::channel::<(String, String, Outcome, Seconds)>();

    for analyzer in &analyzers {
        let tx = tx.clone();
        let analyzer = Arc::clone(analyzer);
        let reg = Arc::clone(&reg);
        let graph = Arc::clone(&graph);
        let plan = Arc::clone(&plan);
        std::thread::spawn(move || {
            let start = Instant::now();
            let result = catch_unwind(AssertUnwindSafe(|| analyzer.run(&reg, &graph, &plan)));
            let outcome = match result {
                Ok(Ok(reqs)) => Outcome::Completed(reqs),
                Ok(Err(e)) => Outcome::Failed(e),
                Err(_) => Outcome::Failed("analyzer panicked".to_owned()),
            };
            let dur = Seconds(start.elapsed().as_secs_f64());
            let _ = tx.send((
                analyzer.name().to_owned(),
                analyzer.version().to_owned(),
                outcome,
                dur,
            ));
        });
    }
    drop(tx); // so the channel closes once every worker has reported

    // Collect until the deadline. Whatever has not reported by then is
    // abandoned — no join, so a slow analyzer never blocks the others.
    let mut reported: std::collections::HashMap<String, (String, Outcome, Seconds)> =
        std::collections::HashMap::new();
    let start = Instant::now();
    while reported.len() < analyzers.len() {
        let remaining = match deadline.checked_sub(start.elapsed()) {
            Some(r) if !r.is_zero() => r,
            _ => break,
        };
        match rx.recv_timeout(remaining) {
            Ok((name, version, outcome, dur)) => {
                reported.insert(name, (version, outcome, dur));
            }
            Err(_) => break, // timed out or disconnected
        }
    }

    collect(&reg, &plan, &analyzers, reported)
}

/// Turn the reported outcomes into a `RequirementSet`: partition into the four
/// execution buckets, dedup the serviceable requirements, record the
/// unserviceable ones, and set `complete` honestly.
fn collect(
    reg: &Registry,
    plan: &InvestigationPlan,
    analyzers: &[Arc<dyn Analyzer>],
    mut reported: std::collections::HashMap<String, (String, Outcome, Seconds)>,
) -> RequirementSet {
    let mut completed = Vec::new();
    let mut empty = Vec::new();
    let mut failed = Vec::new();
    let mut timed_out = Vec::new();
    let mut all_requirements = Vec::new();

    // Process analyzers in a stable (name) order for deterministic output.
    let mut names: Vec<&str> = analyzers.iter().map(|a| a.name()).collect();
    names.sort_unstable();
    names.dedup();

    for name in names {
        match reported.remove(name) {
            Some((version, Outcome::Completed(reqs), dur)) => {
                let outcome = AnalyzerOutcome {
                    analyzer: name.to_owned(),
                    version: SchemaVersion(version),
                    duration: dur,
                    requirement_count: reqs.len() as u32,
                    error: None,
                };
                if reqs.is_empty() {
                    empty.push(outcome);
                } else {
                    completed.push(outcome);
                    all_requirements.extend(reqs);
                }
            }
            Some((version, Outcome::Failed(err), dur)) => {
                failed.push(AnalyzerOutcome {
                    analyzer: name.to_owned(),
                    version: SchemaVersion(version),
                    duration: dur,
                    requirement_count: 0,
                    error: Some(err),
                });
            }
            None => {
                // Never reported by the deadline.
                let version = analyzers
                    .iter()
                    .find(|a| a.name() == name)
                    .map(|a| a.version().to_owned())
                    .unwrap_or_default();
                timed_out.push(AnalyzerOutcome {
                    analyzer: name.to_owned(),
                    version: SchemaVersion(version),
                    duration: Seconds(plan.constraints.max_runtime.0),
                    requirement_count: 0,
                    error: None,
                });
            }
        }
    }

    // Merge duplicates, then split serviceable from unserviceable.
    let merged = dedup::merge(all_requirements);
    let mut requirements = Vec::new();
    let mut unserviceable = Vec::new();
    for req in merged {
        match validate::check(reg, &req) {
            None => requirements.push(req),
            Some((reason, detail)) => unserviceable.push(Unserviceable {
                requirement_id: req.id.clone(),
                requested_by: req.requested_by.clone(),
                necessity: req.necessity,
                reason,
                detail,
            }),
        }
    }

    let complete = failed.is_empty() && timed_out.is_empty() && unserviceable.is_empty();
    let core = plan.id.0.strip_prefix("plan_").unwrap_or(&plan.id.0);

    RequirementSet {
        id: Id(format!("reqset_{core}")),
        schema_version: SchemaVersion("1.0.0".into()),
        created_at: plan.created_at,
        plan_id: plan.id.clone(),
        question_id: plan.question_id.clone(),
        world_version: plan.constraints.world_version.clone(),
        requirements,
        unserviceable,
        execution: ExecutionReport {
            completed,
            empty,
            failed,
            timed_out,
        },
        complete,
    }
}
