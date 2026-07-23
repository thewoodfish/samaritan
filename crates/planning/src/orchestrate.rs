//! The planning pipeline: a `Question` in, an `InvestigationPlan` or a
//! rejection out. Runs the model stages, routes the four validation states,
//! and hands the resolved inputs to the deterministic assembler (stage 4).
//!
//! Planning holds no session: a non-`Valid` question returns a rejected
//! `ParsedQuestion` with a reason and `missing[]`, and the caller re-asks.

use serde::Serialize;

use samaritan_registry::Registry;
use samaritan_schema::{
    Confidence, Id, Intent, InvestigationPlan, ParsedQuestion, PlanProvenance, Priority, Question,
    RankedDomain, SchemaVersion, ValidationStatus, WorldVersion,
};

use crate::assemble::{PlanInputs, assemble_plan};
use crate::error::PlanningError;
use crate::model::{Model, PROMPT_TEMPLATE_VERSION};
use crate::{keys, stages};

/// The result of planning a question.
#[derive(Debug, Clone, Serialize)]
pub enum PlanOutcome {
    /// A complete plan.
    Plan(Box<InvestigationPlan>),
    /// The question was not answerable as asked; re-ask with the guidance in
    /// the `ParsedQuestion`.
    Rejected(ParsedQuestion),
}

/// Plan a question end to end.
pub fn plan_question(
    model: &dyn Model,
    reg: &Registry,
    question: &Question,
    world_version: WorldVersion,
) -> Result<PlanOutcome, PlanningError> {
    let core = question.id.0.strip_prefix("q_").unwrap_or(&question.id.0);
    let created_at = question.asked_at;

    // Stage 1 — validation.
    let v = stages::validate(model, &question.text)?;
    let status = parse_status(&v.status);
    if status != ValidationStatus::Valid {
        return Ok(PlanOutcome::Rejected(reject(
            core,
            question,
            created_at,
            status,
            conf(v.confidence),
            v.language,
            v.reason,
            v.missing,
        )));
    }

    // Stage 2 — normalization.
    let normalized = stages::normalize(model, &question.text)?.normalized_question;

    // Stage 3 — intent, with the confidence floor between classifying and
    // guessing.
    let i = stages::intent(model, &normalized)?;
    let Some(intent_kind) = keys::parse_intent(&i.kind) else {
        return Ok(PlanOutcome::Rejected(reject(
            core,
            question,
            created_at,
            ValidationStatus::Ambiguous,
            conf(i.confidence),
            v.language,
            Some(format!("unrecognized intent '{}'", i.kind)),
            vec!["a clearer statement of what you want to know".into()],
        )));
    };
    if i.confidence < reg.thresholds().intent_confidence_floor {
        return Ok(PlanOutcome::Rejected(reject(
            core,
            question,
            created_at,
            ValidationStatus::Ambiguous,
            conf(i.confidence),
            v.language,
            Some("the intent of the question is unclear".into()),
            vec!["whether you want an explanation, a comparison, a location, …".into()],
        )));
    }
    let intent = Intent {
        kind: intent_kind,
        confidence: conf(i.confidence),
        rationale: i.rationale,
    };

    // Stage 4 — domains.
    let d = stages::domains(model, &normalized)?;
    let ranked_domains: Vec<RankedDomain> = d
        .domains
        .into_iter()
        .filter_map(|o| {
            keys::parse_domain(&o.domain).map(|domain| RankedDomain {
                domain,
                rank: 0,
                confidence: conf(o.confidence),
                rationale: o.rationale,
            })
        })
        .collect();

    // Stage 5 — constraint extraction, then the window-required gate.
    let c = stages::constraints(model, &normalized)?;
    let window_required = reg
        .config()
        .window_required
        .get(keys::intent_key(intent_kind))
        .copied()
        .unwrap_or(true);
    let time_expr = c.time_expr.filter(|s| !s.trim().is_empty());
    let time_expr = match time_expr {
        Some(t) => t,
        None if window_required => {
            return Ok(PlanOutcome::Rejected(reject(
                core,
                question,
                created_at,
                ValidationStatus::Incomplete,
                conf(v.confidence),
                v.language,
                Some("no time range was given".into()),
                vec!["a time range, e.g. \"yesterday\" or \"last shift\"".into()],
            )));
        }
        // Locate and other window-optional intents default to the pinned world.
        None => "now".to_owned(),
    };

    let entities = c
        .entities
        .into_iter()
        .map(|e| samaritan_schema::EntityRef {
            kind: e.kind,
            reference: None,
            label: e.label,
        })
        .collect();

    let provenance = PlanProvenance {
        model_id: model.id().to_owned(),
        prompt_template_version: PROMPT_TEMPLATE_VERSION.to_owned(),
        registry_version: SchemaVersion(reg.config().registry_version.clone()),
        cache_hit: false,
    };

    let inputs = PlanInputs {
        question: question.clone(),
        normalized_question: normalized,
        intent,
        ranked_domains,
        time_expr,
        scope_phrase: c.scope_phrase.filter(|s| !s.trim().is_empty()),
        entities,
        world_version,
        priority: parse_priority(&reg.thresholds().default_priority),
        plan_id: Id(format!("plan_{core}")),
        created_at,
        provenance,
    };

    let plan = assemble_plan(reg, inputs)?;
    Ok(PlanOutcome::Plan(Box::new(plan)))
}

/// Build a rejected `ParsedQuestion`.
#[allow(clippy::too_many_arguments)]
fn reject(
    core: &str,
    question: &Question,
    created_at: samaritan_schema::Timestamp,
    status: ValidationStatus,
    confidence: Confidence,
    language: String,
    reason: Option<String>,
    missing: Vec<String>,
) -> ParsedQuestion {
    ParsedQuestion {
        id: Id(format!("pq_{core}")),
        schema_version: SchemaVersion("1.0.0".into()),
        created_at,
        question_id: question.id.clone(),
        status,
        normalized_question: None,
        confidence,
        language,
        reason,
        missing,
    }
}

fn parse_status(s: &str) -> ValidationStatus {
    match s {
        "Valid" => ValidationStatus::Valid,
        "Ambiguous" => ValidationStatus::Ambiguous,
        "Incomplete" => ValidationStatus::Incomplete,
        // Anything unrecognized is treated as not answerable, never guessed.
        _ => ValidationStatus::Invalid,
    }
}

fn parse_priority(s: &str) -> Priority {
    match s {
        "Low" => Priority::Low,
        "High" => Priority::High,
        "Critical" => Priority::Critical,
        _ => Priority::Normal,
    }
}

/// Clamp a model-reported confidence into range rather than failing — an
/// out-of-range number is the model's slip, not the operator's.
fn conf(x: f64) -> Confidence {
    Confidence::new(x.clamp(0.0, 1.0)).expect("clamped into range")
}

// ---- determinism harness --------------------------------------------------

/// The outcome of running the same questions repeatedly (`PIPELINE.md`,
/// Determinism). Reproducibility is a measured property, not an assumption.
#[derive(Debug, Clone)]
pub struct DeterminismReport {
    pub total: usize,
    pub stable: usize,
    /// Ids of questions whose plan changed between runs.
    pub unstable: Vec<String>,
}

impl DeterminismReport {
    pub fn all_stable(&self) -> bool {
        self.unstable.is_empty()
    }
}

/// Run each question `runs` times and report which produced an identical plan
/// every time.
pub fn determinism_report(
    model: &dyn Model,
    reg: &Registry,
    questions: &[Question],
    world_version: &WorldVersion,
    runs: usize,
) -> DeterminismReport {
    let mut unstable = Vec::new();
    for q in questions {
        let mut fingerprints = std::collections::BTreeSet::new();
        for _ in 0..runs.max(1) {
            let out = plan_question(model, reg, q, world_version.clone());
            let fp = match out {
                Ok(o) => serde_json::to_string(&o).unwrap_or_default(),
                Err(e) => format!("ERR:{e}"),
            };
            fingerprints.insert(fp);
        }
        if fingerprints.len() > 1 {
            unstable.push(q.id.0.clone());
        }
    }
    DeterminismReport {
        total: questions.len(),
        stable: questions.len() - unstable.len(),
        unstable,
    }
}
