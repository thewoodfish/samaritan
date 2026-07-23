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

/// The full "why did efficiency drop yesterday" plan — three analyzers.
fn full_plan(reg: &Registry) -> InvestigationPlan {
    let question = Question {
        id: Id::from("q_full"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "Why did efficiency drop yesterday?".into(),
        asked_at: ts("2026-07-21T09:14:00Z"),
        operator: Id::from("op_01J8X0"),
        organization: Id::from("org_01J8X0"),
        site: Id::from("site_01J8X0"),
        locale: "en".into(),
    };
    assemble_plan(
        reg,
        PlanInputs {
            question,
            normalized_question: "Why did efficiency decrease yesterday?".into(),
            intent: Intent {
                kind: IntentType::Explain,
                confidence: Confidence::new(0.96).unwrap(),
                rationale: "x".into(),
            },
            ranked_domains: vec![
                RankedDomain {
                    domain: DomainType::OperationalPerformance,
                    rank: 0,
                    confidence: Confidence::new(0.94).unwrap(),
                    rationale: "x".into(),
                },
                RankedDomain {
                    domain: DomainType::MaterialFlow,
                    rank: 0,
                    confidence: Confidence::new(0.81).unwrap(),
                    rationale: "x".into(),
                },
                RankedDomain {
                    domain: DomainType::Equipment,
                    rank: 0,
                    confidence: Confidence::new(0.77).unwrap(),
                    rationale: "x".into(),
                },
            ],
            time_expr: "yesterday".into(),
            scope_phrase: None,
            entities: vec![],
            world_version: WorldVersion {
                log_position: 1,
                as_of: ts("2026-07-21T09:14:00Z"),
                snapshot: None,
            },
            priority: Priority::Normal,
            plan_id: Id::from("plan_full"),
            created_at: ts("2026-07-21T09:14:00Z"),
            provenance: PlanProvenance {
                model_id: "m".into(),
                prompt_template_version: "p".into(),
                registry_version: SchemaVersion::from("1.0.0"),
                cache_hit: false,
            },
        },
    )
    .unwrap()
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
        group_by: vec![],
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
fn full_efficiency_investigation_is_complete_and_multi_analyzer() {
    // The whole "why did efficiency drop yesterday" question, three real
    // analyzers, through dispatch. (Stage 8: the pipeline end to end.)
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());

    let question = Question {
        id: Id::from("q_full"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "Why did efficiency drop yesterday?".into(),
        asked_at: ts("2026-07-21T09:14:00Z"),
        operator: Id::from("op_01J8X0"),
        organization: Id::from("org_01J8X0"),
        site: Id::from("site_01J8X0"),
        locale: "en".into(),
    };
    let plan = Arc::new(
        assemble_plan(
            &reg,
            PlanInputs {
                question,
                normalized_question: "Why did efficiency decrease yesterday?".into(),
                intent: Intent {
                    kind: IntentType::Explain,
                    confidence: Confidence::new(0.96).unwrap(),
                    rationale: "x".into(),
                },
                ranked_domains: vec![
                    RankedDomain {
                        domain: DomainType::OperationalPerformance,
                        rank: 0,
                        confidence: Confidence::new(0.94).unwrap(),
                        rationale: "x".into(),
                    },
                    RankedDomain {
                        domain: DomainType::MaterialFlow,
                        rank: 0,
                        confidence: Confidence::new(0.81).unwrap(),
                        rationale: "x".into(),
                    },
                    RankedDomain {
                        domain: DomainType::Equipment,
                        rank: 0,
                        confidence: Confidence::new(0.77).unwrap(),
                        rationale: "x".into(),
                    },
                ],
                time_expr: "yesterday".into(),
                scope_phrase: None,
                entities: vec![],
                world_version: WorldVersion {
                    log_position: 1,
                    as_of: ts("2026-07-21T09:14:00Z"),
                    snapshot: None,
                },
                priority: Priority::Normal,
                plan_id: Id::from("plan_full"),
                created_at: ts("2026-07-21T09:14:00Z"),
                provenance: PlanProvenance {
                    model_id: "m".into(),
                    prompt_template_version: "p".into(),
                    registry_version: SchemaVersion::from("1.0.0"),
                    cache_hit: false,
                },
            },
        )
        .unwrap(),
    );

    let analyzers = analyzers_for_plan(&reg, &plan);
    assert_eq!(analyzers.len(), 3, "efficiency, flow, maintenance");

    let set = dispatch(reg, graph, plan, analyzers);

    assert!(set.complete, "unserviceable: {:?}", set.unserviceable);
    assert_eq!(set.execution.completed.len(), 3);
    let requesters: std::collections::BTreeSet<&str> = set
        .requirements
        .iter()
        .flat_map(|r| r.requested_by.iter().map(String::as_str))
        .collect();
    assert!(requesters.contains("efficiency"));
    assert!(requesters.contains("flow"));
    assert!(requesters.contains("maintenance"));
    // efficiency's decomposition put the six cycle phases in one requirement.
    assert!(
        set.requirements.iter().any(|r| r.subject == "haul_cycle"
            && r.observables.contains(&"queue_time".to_string()))
    );
}

#[test]
fn requirement_set_replays_identically() {
    // The plan pins its world_version, so re-running dispatch — even "later",
    // after the real world has advanced — reproduces the identical set. This is
    // replay: a stored artifact reconstructs bit-for-bit (Stage 9).
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(full_plan(&reg));

    // The semantic artifact — requirements, unserviceable, completeness, and the
    // identity fields. Execution durations are wall-clock measurements and are
    // deliberately excluded; they are observability, not content.
    let semantic = |s: &samaritan_schema::RequirementSet| {
        serde_json::to_string(&(
            &s.id,
            &s.plan_id,
            &s.question_id,
            &s.world_version,
            &s.requirements,
            &s.unserviceable,
            s.complete,
        ))
        .unwrap()
    };

    let stored = semantic(&dispatch(
        reg.clone(),
        graph.clone(),
        plan.clone(),
        analyzers_for_plan(&reg, &plan),
    ));
    let replayed = semantic(&dispatch(
        reg.clone(),
        graph.clone(),
        plan.clone(),
        analyzers_for_plan(&reg, &plan),
    ));

    assert_eq!(stored, replayed, "the pinned plan must replay identically");
}

#[test]
fn provenance_chain_resolves_for_every_requirement() {
    // Every requirement traces back: requester -> analyzer -> domain -> plan ->
    // question. Nothing appears without provenance (PIPELINE.md).
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());
    let plan = Arc::new(full_plan(&reg));

    let plan_analyzers: std::collections::BTreeSet<&str> =
        plan.analyzers.iter().map(|a| a.name.as_str()).collect();
    let plan_domains: std::collections::HashSet<DomainType> =
        plan.domains.domains.iter().map(|d| d.domain).collect();

    let set = dispatch(
        reg.clone(),
        graph,
        plan.clone(),
        analyzers_for_plan(&reg, &plan),
    );

    // The set links back to the plan and question.
    assert_eq!(set.plan_id, plan.id);
    assert_eq!(set.question_id, plan.question_id);
    assert_eq!(
        set.world_version.log_position,
        plan.constraints.world_version.log_position
    );

    for req in &set.requirements {
        // requirement -> plan
        assert_eq!(req.plan_id, plan.id, "requirement names its plan");
        // requirement -> analyzer(s) that asked for it, each named in the plan
        assert!(!req.requested_by.is_empty());
        for requester in &req.requested_by {
            assert!(
                plan_analyzers.contains(requester.as_str()),
                "requester '{requester}' is not an analyzer in the plan"
            );
            // analyzer -> domain: at least one of its domains is in the plan
            let decl = reg.analyzer(requester).unwrap();
            let covers = decl
                .domains
                .iter()
                .filter_map(|d| parse_domain(d))
                .any(|d| plan_domains.contains(&d));
            assert!(covers, "analyzer '{requester}' covers no ranked domain");
        }
    }
}

/// Local domain parser for the provenance check.
fn parse_domain(s: &str) -> Option<DomainType> {
    Some(match s {
        "OperationalPerformance" => DomainType::OperationalPerformance,
        "Production" => DomainType::Production,
        "Equipment" => DomainType::Equipment,
        "MaterialFlow" => DomainType::MaterialFlow,
        "Infrastructure" => DomainType::Infrastructure,
        "Personnel" => DomainType::Personnel,
        "Safety" => DomainType::Safety,
        "Security" => DomainType::Security,
        "Environment" => DomainType::Environment,
        "Logistics" => DomainType::Logistics,
        _ => return None,
    })
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
