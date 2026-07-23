//! Stage 7 exit criteria:
//! - two analyzers emitting the identical requirement merge into one, with
//!   requested_by unioned
//! - a slow analyzer is abandoned at the deadline and reported timed_out, and
//!   complete is false
//! - an analyzer returning zero requirements is reported empty, never failed
//! - output ordering is identical across repeated runs
//! - a requirement naming an unregistered term is rejected here, recorded
//!   unserviceable, not passed on

use std::sync::Arc;

use samaritan_analyzer::Analyzer;
use samaritan_dispatch::{analyzers_for_plan, dispatch};
use samaritan_planning::{PlanInputs, assemble_plan};
use samaritan_registry::{Registry, RelationGraph};
use samaritan_schema::*;

fn ts(s: &str) -> Timestamp {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn plan_with_runtime(reg: &Registry, max_runtime: f64) -> InvestigationPlan {
    let question = Question {
        id: Id::from("q_01J8XQ7A11"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "Why did efficiency drop yesterday?".into(),
        asked_at: ts("2026-07-21T09:14:00Z"),
        operator: Id::from("op_01J8X0"),
        organization: Id::from("org_01J8X0"),
        site: Id::from("site_01J8X0"),
        locale: "en".into(),
    };
    let mut plan = assemble_plan(
        reg,
        PlanInputs {
            question,
            normalized_question: "Why did efficiency decrease yesterday?".into(),
            intent: Intent {
                kind: IntentType::Explain,
                confidence: Confidence::new(0.96).unwrap(),
                rationale: "x".into(),
            },
            ranked_domains: vec![RankedDomain {
                domain: DomainType::OperationalPerformance,
                rank: 0,
                confidence: Confidence::new(0.94).unwrap(),
                rationale: "x".into(),
            }],
            time_expr: "yesterday".into(),
            scope_phrase: None,
            entities: vec![],
            world_version: WorldVersion {
                log_position: 1_284_662,
                as_of: ts("2026-07-21T09:14:00Z"),
                snapshot: None,
            },
            priority: Priority::Normal,
            plan_id: Id::from("plan_01J8XQ7K3M"),
            created_at: ts("2026-07-21T09:14:00Z"),
            provenance: PlanProvenance {
                model_id: "m".into(),
                prompt_template_version: "p".into(),
                registry_version: SchemaVersion::from("1.0.0"),
                cache_hit: false,
            },
        },
    )
    .unwrap();
    plan.constraints.max_runtime = Seconds(max_runtime);
    plan
}

// ---- analyzer doubles -----------------------------------------------------

/// Emits one fixed requirement, attributed to `name`.
struct FixedAnalyzer {
    name: String,
    req: InformationRequirement,
}
impl Analyzer for FixedAnalyzer {
    fn name(&self) -> &str {
        &self.name
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn run(
        &self,
        _: &Registry,
        _: &RelationGraph,
        _: &InvestigationPlan,
    ) -> Result<Vec<InformationRequirement>, String> {
        let mut r = self.req.clone();
        r.requested_by = vec![self.name.clone()];
        Ok(vec![r])
    }
}

struct SlowAnalyzer;
impl Analyzer for SlowAnalyzer {
    fn name(&self) -> &str {
        "slow"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn run(
        &self,
        _: &Registry,
        _: &RelationGraph,
        _: &InvestigationPlan,
    ) -> Result<Vec<InformationRequirement>, String> {
        std::thread::sleep(std::time::Duration::from_secs(3));
        Ok(vec![])
    }
}

struct EmptyAnalyzer;
impl Analyzer for EmptyAnalyzer {
    fn name(&self) -> &str {
        "quiet"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn run(
        &self,
        _: &Registry,
        _: &RelationGraph,
        _: &InvestigationPlan,
    ) -> Result<Vec<InformationRequirement>, String> {
        Ok(vec![]) // ran, needed nothing
    }
}

struct FailingAnalyzer;
impl Analyzer for FailingAnalyzer {
    fn name(&self) -> &str {
        "broken"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }
    fn run(
        &self,
        _: &Registry,
        _: &RelationGraph,
        _: &InvestigationPlan,
    ) -> Result<Vec<InformationRequirement>, String> {
        Err("data source unreachable".to_owned())
    }
}

fn availability_req(id: &str) -> InformationRequirement {
    InformationRequirement {
        id: Id::from(id),
        plan_id: Id::from("plan_01J8XQ7K3M"),
        requested_by: vec![],
        purpose: "check availability".into(),
        necessity: Necessity::Preferred,
        subject: "equipment_availability".into(),
        observables: vec!["availability".into()],
        filters: vec![],
        spatial: vec![],
        window: TimeWindow {
            start: ts("2026-07-20T05:00:00Z"),
            end: ts("2026-07-21T05:00:00Z"),
            resolved_from: "yesterday".into(),
            calendar: Id::from("northern_pit@v2"),
            timezone: "Africa/Lagos".into(),
        },
        baseline: None,
        scope: SpatialScope {
            kind: ScopeKind::Unspecified,
            reference: None,
            label: "entire site".into(),
        },
        entities: vec![],
        granularity: Granularity::Shift,
        aggregations: vec![Aggregation {
            op: AggregationOp::Mean,
            field: Some("availability".into()),
            bins: None,
        }],
        ordering: None,
        limit: None,
        expected_shape: Shape::Table,
        round: 0,
        depends_on: vec![],
    }
}

fn boxed(a: impl Analyzer + 'static) -> Arc<dyn Analyzer> {
    Arc::new(a)
}

// ---- tests ----------------------------------------------------------------

#[test]
fn identical_requirements_from_two_analyzers_merge() {
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(plan_with_runtime(&reg, 30.0));

    // efficiency and maintenance both ask the identical availability question.
    let analyzers = vec![
        boxed(FixedAnalyzer {
            name: "efficiency".into(),
            req: availability_req("req_a"),
        }),
        boxed(FixedAnalyzer {
            name: "maintenance".into(),
            req: availability_req("req_b"),
        }),
    ];

    let set = dispatch(reg, graph, plan, analyzers);
    assert_eq!(set.requirements.len(), 1, "the two should merge into one");
    assert_eq!(
        set.requirements[0].requested_by,
        vec!["efficiency".to_string(), "maintenance".to_string()]
    );
    // Earliest id wins.
    assert_eq!(set.requirements[0].id.0, "req_a");
    assert!(set.complete);
}

#[test]
fn slow_analyzer_is_abandoned_at_the_deadline() {
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(plan_with_runtime(&reg, 0.2)); // 200ms deadline

    let analyzers = vec![
        boxed(FixedAnalyzer {
            name: "efficiency".into(),
            req: availability_req("req_a"),
        }),
        boxed(SlowAnalyzer), // sleeps 3s
    ];

    let set = dispatch(reg, graph, plan, analyzers);
    assert_eq!(set.execution.timed_out.len(), 1);
    assert_eq!(set.execution.timed_out[0].analyzer, "slow");
    assert!(!set.complete, "a timed-out analyzer means incomplete");
    // The fast analyzer's requirement still made it.
    assert_eq!(set.requirements.len(), 1);
}

#[test]
fn empty_is_distinct_from_failed() {
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(plan_with_runtime(&reg, 30.0));

    let analyzers = vec![boxed(EmptyAnalyzer), boxed(FailingAnalyzer)];
    let set = dispatch(reg, graph, plan, analyzers);

    assert_eq!(set.execution.empty.len(), 1);
    assert_eq!(set.execution.empty[0].analyzer, "quiet");
    assert_eq!(set.execution.failed.len(), 1);
    assert_eq!(set.execution.failed[0].analyzer, "broken");
    assert!(set.execution.failed[0].error.is_some());
    assert!(!set.complete);
}

#[test]
fn unregistered_term_is_recorded_unserviceable_not_passed_on() {
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(plan_with_runtime(&reg, 30.0));

    let mut bad = availability_req("req_bad");
    bad.observables = vec!["teleportation_rate".into()]; // not a real observable
    bad.aggregations = vec![];
    bad.necessity = Necessity::Required;

    let analyzers = vec![boxed(FixedAnalyzer {
        name: "efficiency".into(),
        req: bad,
    })];
    let set = dispatch(reg, graph, plan, analyzers);

    assert!(
        set.requirements.is_empty(),
        "the bad requirement must not pass"
    );
    assert_eq!(set.unserviceable.len(), 1);
    assert_eq!(
        set.unserviceable[0].reason,
        UnserviceableReason::UnavailableObservable
    );
    assert!(
        !set.complete,
        "an unserviceable Required requirement fails the set"
    );
}

#[test]
fn output_ordering_is_stable_across_runs() {
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(plan_with_runtime(&reg, 30.0));

    // The real efficiency analyzer, run through dispatch twice.
    let build = || analyzers_for_plan(&reg, &plan);
    let a = dispatch(reg.clone(), graph.clone(), plan.clone(), build());
    let b = dispatch(reg.clone(), graph.clone(), plan.clone(), build());

    assert_eq!(
        serde_json::to_string(&a.requirements).unwrap(),
        serde_json::to_string(&b.requirements).unwrap()
    );
    // And the real walk's requirements are all serviceable.
    assert!(
        a.unserviceable.is_empty(),
        "unserviceable: {:?}",
        a.unserviceable
    );
    assert!(!a.requirements.is_empty());
}
