//! Stage 8 exit criteria:
//! - each intent has a strategy and a producing question
//! - the full "why did efficiency drop" produces a multi-analyzer RequirementSet
//! - the environment analyzer widens its window back by the persistence figure,
//!   asking about rainfall from before the window began
//! - Locate produces a valid plan with no time window (defaults to now)

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

/// A plan for a given intent, domains, and time expression.
fn plan_for(
    reg: &Registry,
    intent: IntentType,
    domains: Vec<RankedDomain>,
    time_expr: &str,
) -> InvestigationPlan {
    let question = Question {
        id: Id::from("q_01J8XQ7A11"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "operator question".into(),
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
            normalized_question: "normalized".into(),
            intent: Intent {
                kind: intent,
                confidence: Confidence::new(0.9).unwrap(),
                rationale: "t".into(),
            },
            ranked_domains: domains,
            time_expr: time_expr.into(),
            scope_phrase: None,
            entities: vec![],
            world_version: WorldVersion {
                log_position: 1,
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
    .unwrap()
}

fn run(reg: &Registry, analyzer: &str, plan: &InvestigationPlan) -> Vec<InformationRequirement> {
    let graph = reg.relation_graph();
    GraphAnalyzer::from_registry(reg, analyzer)
        .unwrap()
        .requirements(reg, &graph, plan)
}

// ---- one strategy per intent ----------------------------------------------

#[test]
fn explain_produces_establish_decompose_confound() {
    let reg = Registry::mining().unwrap();
    let plan = plan_for(
        &reg,
        IntentType::Explain,
        vec![ranked(DomainType::OperationalPerformance, 0.9)],
        "yesterday",
    );
    let reqs = run(&reg, "efficiency", &plan);
    assert!(reqs.iter().any(|r| r.purpose.starts_with("Establish")));
    assert!(reqs.iter().any(|r| r.purpose.starts_with("Decompose")));
    assert!(reqs.iter().any(|r| r.purpose.starts_with("Rule out")));
}

#[test]
fn compare_establishes_and_decomposes() {
    let reg = Registry::mining().unwrap();
    let plan = plan_for(
        &reg,
        IntentType::Compare,
        vec![ranked(DomainType::OperationalPerformance, 0.9)],
        "yesterday",
    );
    let reqs = run(&reg, "efficiency", &plan);
    assert!(reqs.iter().any(|r| r.purpose.starts_with("Establish")));
    assert!(reqs.iter().any(|r| r.purpose.starts_with("Decompose")));
    // Compare has no default baseline — the operator must name both sides.
    assert!(reqs[0].baseline.is_none());
}

#[test]
fn locate_ranks_by_a_group() {
    let reg = Registry::mining().unwrap();
    // flow's metric (queue_event.wait_time) has partitions declared.
    let plan = plan_for(
        &reg,
        IntentType::Locate,
        vec![ranked(DomainType::MaterialFlow, 0.9)],
        "yesterday",
    );
    let reqs = run(&reg, "flow", &plan);
    let r = &reqs[0];
    assert!(!r.group_by.is_empty(), "Locate must group by an entity");
    assert!(r.ordering.is_some());
    assert_eq!(r.limit, Some(5));
    assert_eq!(r.expected_shape, Shape::Set);
}

#[test]
fn summarize_is_one_coarse_aggregate() {
    let reg = Registry::mining().unwrap();
    let plan = plan_for(
        &reg,
        IntentType::Summarize,
        vec![ranked(DomainType::OperationalPerformance, 0.9)],
        "yesterday",
    );
    let reqs = run(&reg, "efficiency", &plan);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].expected_shape, Shape::Table);
    assert_eq!(reqs[0].granularity, Granularity::Shift);
}

#[test]
fn predict_adds_upstream_drivers() {
    let reg = Registry::mining().unwrap();
    // maintenance targets equipment_availability.availability, which has an
    // upstream driver (downtime_event.duration -> availability).
    let plan = plan_for(
        &reg,
        IntentType::Predict,
        vec![ranked(DomainType::Equipment, 0.9)],
        "yesterday",
    );
    let reqs = run(&reg, "maintenance", &plan);
    assert!(reqs.iter().any(|r| r.purpose.contains("upstream driver")));
}

// ---- environment widening (the rain case) ---------------------------------

#[test]
fn environment_widens_window_for_rain_persistence() {
    let reg = Registry::mining().unwrap();
    let plan = plan_for(
        &reg,
        IntentType::Explain,
        vec![ranked(DomainType::Environment, 0.9)],
        "yesterday",
    );
    let reqs = run(&reg, "environment", &plan);

    let rain = reqs
        .iter()
        .find(|r| r.subject == "weather_observation")
        .expect("environment establishes rainfall");
    // rainfall influences ground_condition with persistence 21600s, so the
    // window must reach 6h before the plan's window start.
    let plan_start = plan.constraints.time.start;
    assert_eq!(
        rain.window.start,
        plan_start - chrono::Duration::seconds(21_600)
    );
    assert!(rain.window.resolved_from.contains("widened"));
}

// ---- Locate with no window ------------------------------------------------

#[test]
fn locate_plan_needs_no_time_window() {
    let reg = Registry::mining().unwrap();
    // "now" stands in for the window-optional Locate case; the deterministic
    // resolver gives a zero-width window at the pinned world.
    let plan = plan_for(
        &reg,
        IntentType::Locate,
        vec![ranked(DomainType::MaterialFlow, 0.9)],
        "now",
    );
    let reqs = run(&reg, "flow", &plan);
    assert!(!reqs.is_empty());
    // window is instantaneous — start equals end at asked_at.
    assert_eq!(plan.constraints.time.start, plan.constraints.time.end);
}
