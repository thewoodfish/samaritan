//! Plan assembly — the deterministic composition of an `InvestigationPlan`
//! from resolved parts. Given the same inputs and registry, the output is
//! byte-identical (`PLANNING.md`, Investigation Plan).
//!
//! The model-backed inputs (intent, ranked domains, extracted constraint
//! phrases, normalized text) are supplied here; stage 6 produces them.

use samaritan_registry::Registry;
use samaritan_schema::{
    EntityRef, Id, Intent, IntentType, InvestigationConstraints, InvestigationPlan,
    InvestigationStrategy, OperationalDomains, PlanProvenance, Priority, Question, RankedDomain,
    SchemaVersion, Seconds, Timestamp, WorldVersion,
};

use crate::error::PlanningError;
use crate::keys::intent_key;
use crate::{scope, select, time};

/// Everything the deterministic assembler needs. The model layer fills the
/// intent, domains, and constraint phrases; here they arrive resolved-ready.
pub struct PlanInputs {
    pub question: Question,
    /// The normalized question text (from the model).
    pub normalized_question: String,
    pub intent: Intent,
    /// Ranked domains from the model, before culling.
    pub ranked_domains: Vec<RankedDomain>,
    /// The verbatim time phrase, e.g. "yesterday".
    pub time_expr: String,
    /// The verbatim place phrase, if the operator narrowed scope.
    pub scope_phrase: Option<String>,
    pub entities: Vec<EntityRef>,
    pub world_version: WorldVersion,
    pub priority: Priority,
    /// Identity and provenance — derived deterministically upstream (a hash of
    /// the inputs in stage 9); provided here.
    pub plan_id: Id,
    pub created_at: Timestamp,
    pub provenance: PlanProvenance,
}

/// The strategy for an intent — a registry lookup, never a second inference.
pub fn derive_strategy(
    reg: &Registry,
    intent: IntentType,
) -> Result<InvestigationStrategy, PlanningError> {
    let key = intent_key(intent);
    let s = reg
        .strategy(key)
        .ok_or_else(|| PlanningError::MissingStrategy(key.to_owned()))?;
    Ok(InvestigationStrategy {
        kind: intent,
        goal: s.goal.clone(),
        expected_output: s.expected_output.clone(),
    })
}

/// Assemble a complete `InvestigationPlan`. Deterministic in its inputs.
pub fn assemble_plan(
    reg: &Registry,
    inputs: PlanInputs,
) -> Result<InvestigationPlan, PlanningError> {
    let site_id = inputs.question.site.0.as_str();

    // Constraints — resolve time, baseline, scope.
    let time = time::resolve_time(reg, site_id, &inputs.time_expr, inputs.question.asked_at)?;
    let baseline = match reg
        .baseline_default(intent_key(inputs.intent.kind))
        .and_then(|b| b.trailing_days())
    {
        Some(days) => Some(time::baseline_window(reg, site_id, days, time.start)?),
        None => None,
    };
    let spatial_scope = scope::resolve_scope(reg, inputs.scope_phrase.as_deref())?;

    // Domains and analyzers — cull then select, in precedence order.
    let domains = select::cull_domains(reg, inputs.ranked_domains);
    if domains.is_empty() {
        return Err(PlanningError::NoDomains);
    }
    let analyzers = select::select_analyzers(reg, &domains);
    if analyzers.is_empty() {
        return Err(PlanningError::NoAnalyzers);
    }

    let strategy = derive_strategy(reg, inputs.intent.kind)?;

    let constraints = InvestigationConstraints {
        time,
        baseline,
        spatial_scope,
        entity_scope: inputs.entities,
        world_version: inputs.world_version,
        priority: inputs.priority,
        max_runtime: Seconds(reg.thresholds().default_max_runtime as f64),
        operator: inputs.question.operator.clone(),
        organization: inputs.question.organization.clone(),
    };

    Ok(InvestigationPlan {
        id: inputs.plan_id,
        schema_version: SchemaVersion("1.0.0".into()),
        created_at: inputs.created_at,
        question_id: inputs.question.id.clone(),
        question_text: inputs.normalized_question,
        intent: inputs.intent,
        domains: OperationalDomains { domains },
        strategy,
        analyzers,
        constraints,
        provenance: inputs.provenance,
    })
}
