//! Emit a real RequirementSet as pretty JSON — the terminal artifact of part 1
//! and the specification of the Reality Engine's request API.
//!
//! Run: `cargo run -p samaritan --example dump_requirementset`

use std::sync::Arc;

use samaritan_dispatch::{analyzers_for_plan, dispatch};
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
        rationale: "relevant to the question".into(),
    }
}

fn main() {
    let reg = Arc::new(Registry::mining().unwrap());
    let graph = Arc::new(reg.relation_graph());

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

    let plan = Arc::new(
        assemble_plan(
            &reg,
            PlanInputs {
                question,
                normalized_question: "Why did efficiency decrease yesterday?".into(),
                intent: Intent {
                    kind: IntentType::Explain,
                    confidence: Confidence::new(0.96).unwrap(),
                    rationale: "asks for the cause of an observed decrease".into(),
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
                    snapshot: Some(Id::from("snap_01J8XP")),
                },
                priority: Priority::Normal,
                plan_id: Id::from("plan_01J8XQ7K3M"),
                created_at: ts("2026-07-21T09:14:00Z"),
                provenance: PlanProvenance {
                    model_id: "qwen3:4b".into(),
                    prompt_template_version: "planning/2026-07-01".into(),
                    registry_version: SchemaVersion::from("1.0.0"),
                    cache_hit: false,
                },
            },
        )
        .unwrap(),
    );

    let analyzers = analyzers_for_plan(&reg, &plan);
    let set = dispatch(reg, graph, plan, analyzers);

    println!("{}", serde_json::to_string_pretty(&set).unwrap());
}
