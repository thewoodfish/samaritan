//! Stage 5 — THE THESIS TEST.
//!
//! One analyzer (efficiency), one intent (Explain), walking the relation
//! graph, producing real requirements. The go/no-go for the generic-analyzer
//! idea: does a graph walk reproduce the requirements hand-written in
//! ANALYZER.md?
//!
//! Exit criteria:
//! - the walk produces the requirements ANALYZER.md predicted — same subjects,
//!   observables, aggregations
//! - the confounder step emits the availability requirement, marked Preferred
//! - every emitted requirement validates against the registry
//! - the decomposition observables come from the graph, not a hardcoded list

use samaritan_analyzer::GraphAnalyzer;
use samaritan_planning::{PlanInputs, assemble_plan};
use samaritan_registry::Registry;
use samaritan_schema::*;

fn ts(s: &str) -> Timestamp {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn ranked(domain: DomainType, conf: f64) -> RankedDomain {
    RankedDomain {
        domain,
        rank: 0,
        confidence: Confidence::new(conf).unwrap(),
        rationale: "t".into(),
    }
}

/// The "why did efficiency drop yesterday?" plan, assembled for real.
fn efficiency_plan(reg: &Registry) -> InvestigationPlan {
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
    let inputs = PlanInputs {
        question,
        normalized_question: "Why did efficiency decrease yesterday?".into(),
        intent: Intent {
            kind: IntentType::Explain,
            confidence: Confidence::new(0.96).unwrap(),
            rationale: "cause of a decrease".into(),
        },
        ranked_domains: vec![
            ranked(DomainType::OperationalPerformance, 0.94),
            ranked(DomainType::MaterialFlow, 0.81),
            ranked(DomainType::Equipment, 0.77),
        ],
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
    };
    assemble_plan(reg, inputs).unwrap()
}

/// Validate a requirement against the registry vocabulary — every subject and
/// observable must be real.
fn validates(reg: &Registry, req: &InformationRequirement) -> bool {
    let Some(subject) = reg.config().subjects.get(&req.subject) else {
        return false;
    };
    req.observables
        .iter()
        .all(|o| subject.observables.contains_key(o))
        && req
            .aggregations
            .iter()
            .filter_map(|a| a.field.as_ref())
            .all(|f| subject.observables.contains_key(f))
}

#[test]
fn efficiency_explain_reproduces_hand_written_requirements() {
    let reg = Registry::mining().unwrap();
    let graph = reg.relation_graph();
    let plan = efficiency_plan(&reg);

    let efficiency = GraphAnalyzer::from_registry(&reg, "efficiency").unwrap();
    let reqs = efficiency.requirements(&reg, &graph, &plan);

    // Every requirement is valid against the registry.
    for r in &reqs {
        assert!(validates(&reg, r), "invalid requirement: {}", r.purpose);
        assert_eq!(r.requested_by, vec!["efficiency".to_string()]);
        assert_eq!(r.plan_id, plan.id);
        // Requirements inherit the plan's resolved window.
        assert_eq!(r.window.start, ts("2026-07-20T05:00:00Z"));
    }

    // ---- establish -------------------------------------------------------
    let establish = &reqs[0];
    assert_eq!(establish.subject, "haul_cycle");
    assert_eq!(establish.observables, vec!["cycle_time".to_string()]);
    assert_eq!(establish.necessity, Necessity::Required);
    let ops: Vec<AggregationOp> = establish.aggregations.iter().map(|a| a.op).collect();
    assert!(ops.contains(&AggregationOp::Mean));
    assert!(ops.contains(&AggregationOp::P95));
    assert!(ops.contains(&AggregationOp::Count));

    // ---- decompose: the six phases come FROM THE GRAPH -------------------
    let decompose = reqs
        .iter()
        .find(|r| r.purpose.starts_with("Decompose"))
        .expect("a decomposition requirement");
    assert_eq!(decompose.subject, "haul_cycle");
    assert_eq!(decompose.necessity, Necessity::Required);
    // The observables must equal the graph's decomposition parts exactly —
    // proving they were walked, not hardcoded.
    let graph_parts: Vec<String> = graph
        .decomposition(&samaritan_graph::NodeRef::parse("haul_cycle.cycle_time").unwrap())
        .unwrap()
        .parts
        .iter()
        .map(|p| p.field.clone())
        .collect();
    assert_eq!(decompose.observables, graph_parts);
    assert_eq!(
        decompose.observables,
        vec![
            "queue_time",
            "spot_time",
            "load_time",
            "haul_time",
            "dump_time",
            "return_time"
        ]
    );

    // ---- confound: fleet availability, marked Preferred ------------------
    let avail = reqs
        .iter()
        .find(|r| r.subject == "equipment_availability")
        .expect("a requirement ruling out fleet availability");
    assert_eq!(avail.necessity, Necessity::Preferred);
    assert!(avail.observables.contains(&"availability".to_string()));
    // Ratio expansion pulled in the definition's terms from the graph/registry.
    assert!(avail.observables.contains(&"available_time".to_string()));
    assert!(avail.observables.contains(&"scheduled_time".to_string()));
    assert!(avail.purpose.contains("Rule out"));
}

#[test]
fn walk_is_deterministic() {
    let reg = Registry::mining().unwrap();
    let graph = reg.relation_graph();
    let plan = efficiency_plan(&reg);
    let efficiency = GraphAnalyzer::from_registry(&reg, "efficiency").unwrap();

    let a = efficiency.requirements(&reg, &graph, &plan);
    let b = efficiency.requirements(&reg, &graph, &plan);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}

#[test]
fn analyzer_declining_an_intent_returns_nothing() {
    // environment declares Explain/Compare/Summarize, not Predict. Given a
    // Predict plan it produces nothing — a legitimate empty outcome, not a
    // failure.
    let reg = Registry::mining().unwrap();
    let graph = reg.relation_graph();
    let mut plan = efficiency_plan(&reg);
    plan.intent.kind = IntentType::Predict;
    let env = GraphAnalyzer::from_registry(&reg, "environment").unwrap();
    assert!(env.requirements(&reg, &graph, &plan).is_empty());
}
