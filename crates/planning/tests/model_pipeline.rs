//! Stage 6 exit criteria (hermetic — driven by a scripted model, no server):
//! - the four validation states each have a question that produces them
//! - a rejected question returns a reason and missing[], never a partial plan
//! - the determinism harness reports all-stable for a fixed set
//! - a full plan matches a stage-4 hand-assembled plan byte-for-byte
//!
//! A live Ollama check is provided too, ignored by default.

use std::cell::RefCell;
use std::collections::HashMap;

use samaritan_planning::{
    Model, ModelError, PROMPT_TEMPLATE_VERSION, PlanInputs, PlanOutcome, assemble_plan,
    determinism_report, plan_question,
};
use samaritan_registry::Registry;
use samaritan_schema::*;

fn ts(s: &str) -> Timestamp {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

/// A model double: canned JSON per stage, returned every call. Deterministic by
/// construction, so it exercises the pipeline without a server.
struct ScriptedModel {
    replies: HashMap<String, serde_json::Value>,
    calls: RefCell<Vec<String>>,
}

impl ScriptedModel {
    fn new(replies: &[(&str, serde_json::Value)]) -> Self {
        ScriptedModel {
            replies: replies
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl Model for ScriptedModel {
    fn id(&self) -> &str {
        "scripted"
    }
    fn complete_json(
        &self,
        stage: &str,
        _system: &str,
        _user: &str,
    ) -> Result<serde_json::Value, ModelError> {
        self.calls.borrow_mut().push(stage.to_owned());
        self.replies
            .get(stage)
            .cloned()
            .ok_or_else(|| ModelError::MissingField(stage.to_owned()))
    }
}

use serde_json::json;

/// A model that plans the sample efficiency question successfully.
fn efficiency_model() -> ScriptedModel {
    ScriptedModel::new(&[
        (
            "validate",
            json!({"status":"Valid","confidence":0.94,"language":"en"}),
        ),
        (
            "normalize",
            json!({"normalized_question":"Why did efficiency decrease yesterday?"}),
        ),
        (
            "intent",
            json!({"type":"Explain","confidence":0.96,"rationale":"cause of a decrease"}),
        ),
        (
            "domains",
            json!({"domains":[
                {"domain":"OperationalPerformance","confidence":0.94,"rationale":"efficiency is the subject"},
                {"domain":"MaterialFlow","confidence":0.81,"rationale":"flow losses"},
                {"domain":"Equipment","confidence":0.77,"rationale":"availability"}
            ]}),
        ),
        (
            "constraints",
            json!({"time_expr":"yesterday","scope_phrase":null,"entities":[]}),
        ),
    ])
}

fn sample_question() -> Question {
    Question {
        id: Id::from("q_01J8XQ7A11"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "Why did efficiency drop yesterday?".into(),
        asked_at: ts("2026-07-21T09:14:00Z"),
        operator: Id::from("op_01J8X0"),
        organization: Id::from("org_01J8X0"),
        site: Id::from("site_01J8X0"),
        locale: "en".into(),
    }
}

fn world() -> WorldVersion {
    WorldVersion {
        log_position: 1_284_662,
        as_of: ts("2026-07-21T09:14:00Z"),
        snapshot: None,
    }
}

// ---- the four validation states -------------------------------------------

#[test]
fn valid_question_produces_a_plan() {
    let reg = Registry::mining().unwrap();
    let out = plan_question(&efficiency_model(), &reg, &sample_question(), world()).unwrap();
    let PlanOutcome::Plan(plan) = out else {
        panic!("expected a plan");
    };
    assert_eq!(plan.intent.kind, IntentType::Explain);
    let names: Vec<&str> = plan.analyzers.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["efficiency", "flow", "maintenance"]);
}

#[test]
fn invalid_question_is_rejected_with_reason() {
    let reg = Registry::mining().unwrap();
    let model = ScriptedModel::new(&[(
        "validate",
        json!({"status":"Invalid","confidence":0.9,"language":"en",
               "reason":"not an operational question","missing":[]}),
    )]);
    let mut q = sample_question();
    q.text = "What is Bitcoin trading at?".into();
    let out = plan_question(&model, &reg, &q, world()).unwrap();
    let PlanOutcome::Rejected(p) = out else {
        panic!("expected rejection");
    };
    assert_eq!(p.status, ValidationStatus::Invalid);
    assert!(p.reason.is_some());
    assert!(p.normalized_question.is_none());
}

#[test]
fn incomplete_question_names_what_is_missing() {
    let reg = Registry::mining().unwrap();
    // Valid + Explain, but constraint extraction finds no time range. Explain
    // requires a window, so the question is Incomplete.
    let model = ScriptedModel::new(&[
        (
            "validate",
            json!({"status":"Valid","confidence":0.8,"language":"en"}),
        ),
        (
            "normalize",
            json!({"normalized_question":"Why did efficiency decrease?"}),
        ),
        (
            "intent",
            json!({"type":"Explain","confidence":0.9,"rationale":"cause"}),
        ),
        (
            "domains",
            json!({"domains":[{"domain":"OperationalPerformance","confidence":0.9,"rationale":"x"}]}),
        ),
        (
            "constraints",
            json!({"time_expr":null,"scope_phrase":null,"entities":[]}),
        ),
    ]);
    let out = plan_question(&model, &reg, &sample_question(), world()).unwrap();
    let PlanOutcome::Rejected(p) = out else {
        panic!("expected Incomplete rejection");
    };
    assert_eq!(p.status, ValidationStatus::Incomplete);
    assert!(!p.missing.is_empty());
    assert!(p.missing.iter().any(|m| m.contains("time")));
}

#[test]
fn low_intent_confidence_is_ambiguous() {
    let reg = Registry::mining().unwrap();
    // Below the 0.60 intent floor -> Ambiguous.
    let model = ScriptedModel::new(&[
        (
            "validate",
            json!({"status":"Valid","confidence":0.7,"language":"en"}),
        ),
        (
            "normalize",
            json!({"normalized_question":"Show me the activity"}),
        ),
        (
            "intent",
            json!({"type":"Summarize","confidence":0.35,"rationale":"unclear"}),
        ),
    ]);
    let out = plan_question(&model, &reg, &sample_question(), world()).unwrap();
    let PlanOutcome::Rejected(p) = out else {
        panic!("expected Ambiguous rejection");
    };
    assert_eq!(p.status, ValidationStatus::Ambiguous);
}

// ---- byte-identical to a stage-4 hand-assembled plan ----------------------

#[test]
fn model_plan_matches_direct_assembly() {
    let reg = Registry::mining().unwrap();
    let model = efficiency_model();
    let PlanOutcome::Plan(via_model) =
        plan_question(&model, &reg, &sample_question(), world()).unwrap()
    else {
        panic!("expected a plan");
    };

    // The same inputs the model produced, assembled directly (stage 4).
    let direct = assemble_plan(
        &reg,
        PlanInputs {
            question: sample_question(),
            normalized_question: "Why did efficiency decrease yesterday?".into(),
            intent: Intent {
                kind: IntentType::Explain,
                confidence: Confidence::new(0.96).unwrap(),
                rationale: "cause of a decrease".into(),
            },
            ranked_domains: vec![
                RankedDomain {
                    domain: DomainType::OperationalPerformance,
                    rank: 0,
                    confidence: Confidence::new(0.94).unwrap(),
                    rationale: "efficiency is the subject".into(),
                },
                RankedDomain {
                    domain: DomainType::MaterialFlow,
                    rank: 0,
                    confidence: Confidence::new(0.81).unwrap(),
                    rationale: "flow losses".into(),
                },
                RankedDomain {
                    domain: DomainType::Equipment,
                    rank: 0,
                    confidence: Confidence::new(0.77).unwrap(),
                    rationale: "availability".into(),
                },
            ],
            time_expr: "yesterday".into(),
            scope_phrase: None,
            entities: vec![],
            world_version: world(),
            priority: Priority::Normal,
            plan_id: Id::from("plan_01J8XQ7A11"),
            created_at: ts("2026-07-21T09:14:00Z"),
            provenance: PlanProvenance {
                model_id: "scripted".into(),
                prompt_template_version: PROMPT_TEMPLATE_VERSION.into(),
                registry_version: SchemaVersion::from("1.0.0"),
                cache_hit: false,
            },
        },
    )
    .unwrap();

    assert_eq!(
        serde_json::to_string(&*via_model).unwrap(),
        serde_json::to_string(&direct).unwrap()
    );
}

// ---- determinism harness --------------------------------------------------

#[test]
fn determinism_harness_reports_all_stable() {
    let reg = Registry::mining().unwrap();
    let model = efficiency_model();
    let report = determinism_report(&model, &reg, &[sample_question()], &world(), 5);
    assert!(report.all_stable(), "unstable: {:?}", report.unstable);
    assert_eq!(report.stable, 1);
}

// ---- live Ollama (ignored; run with `cargo test -- --ignored`) ------------

#[test]
#[ignore = "requires a local Ollama server; set SAMARITAN_OLLAMA_MODEL"]
fn ollama_plans_the_sample_question() {
    let reg = Registry::mining().unwrap();
    let model_name =
        std::env::var("SAMARITAN_OLLAMA_MODEL").unwrap_or_else(|_| "llama3.1".to_owned());
    let model = samaritan_planning::OllamaModel::new(model_name);

    // Determinism across 3 runs at temperature 0.
    let report = determinism_report(&model, &reg, &[sample_question()], &world(), 3);
    println!(
        "ollama determinism: {}/{} stable, unstable={:?}",
        report.stable, report.total, report.unstable
    );

    match plan_question(&model, &reg, &sample_question(), world()).unwrap() {
        PlanOutcome::Plan(p) => {
            println!("intent: {:?}", p.intent.kind);
            println!(
                "analyzers: {:?}",
                p.analyzers.iter().map(|a| &a.name).collect::<Vec<_>>()
            );
            println!("window start: {}", p.constraints.time.start);
        }
        PlanOutcome::Rejected(r) => {
            println!("rejected: {:?} — {:?}", r.status, r.reason);
        }
    }
}
