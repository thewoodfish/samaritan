//! The model-backed planning stages: validate, normalize, extract intent,
//! resolve domains, extract constraints. Each builds a pinned prompt, calls the
//! model, and parses a typed reply (`PLANNING.md`, Stages 1–4).
//!
//! The prompt templates here are versioned by [`PROMPT_TEMPLATE_VERSION`]; any
//! change to them bumps that version and invalidates cached plans.

use serde::Deserialize;

use crate::model::{Model, ModelError};

// ---- typed replies --------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ValidateOut {
    pub status: String,
    pub confidence: f64,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub missing: Vec<String>,
}

fn default_lang() -> String {
    "en".to_owned()
}

#[derive(Debug, Deserialize)]
pub struct NormalizeOut {
    pub normalized_question: String,
}

#[derive(Debug, Deserialize)]
pub struct IntentOut {
    #[serde(rename = "type")]
    pub kind: String,
    pub confidence: f64,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Deserialize)]
pub struct DomainsOut {
    pub domains: Vec<DomainOut>,
}

#[derive(Debug, Deserialize)]
pub struct DomainOut {
    pub domain: String,
    pub confidence: f64,
    #[serde(default)]
    pub rationale: String,
}

#[derive(Debug, Deserialize)]
pub struct ConstraintsOut {
    #[serde(default)]
    pub time_expr: Option<String>,
    #[serde(default)]
    pub scope_phrase: Option<String>,
    #[serde(default)]
    pub entities: Vec<EntityOut>,
}

#[derive(Debug, Deserialize)]
pub struct EntityOut {
    pub kind: String,
    pub label: String,
}

// ---- stage calls ----------------------------------------------------------

fn parse<T: for<'de> Deserialize<'de>>(v: serde_json::Value) -> Result<T, ModelError> {
    serde_json::from_value(v).map_err(|e| ModelError::BadJson(e.to_string()))
}

pub fn validate(model: &dyn Model, question: &str) -> Result<ValidateOut, ModelError> {
    parse(model.complete_json("validate", VALIDATE_SYSTEM, question)?)
}

pub fn normalize(model: &dyn Model, question: &str) -> Result<NormalizeOut, ModelError> {
    parse(model.complete_json("normalize", NORMALIZE_SYSTEM, question)?)
}

pub fn intent(model: &dyn Model, question: &str) -> Result<IntentOut, ModelError> {
    parse(model.complete_json("intent", INTENT_SYSTEM, question)?)
}

pub fn domains(model: &dyn Model, question: &str) -> Result<DomainsOut, ModelError> {
    parse(model.complete_json("domains", DOMAINS_SYSTEM, question)?)
}

pub fn constraints(model: &dyn Model, question: &str) -> Result<ConstraintsOut, ModelError> {
    parse(model.complete_json("constraints", CONSTRAINTS_SYSTEM, question)?)
}

// ---- prompt templates (pinned by PROMPT_TEMPLATE_VERSION) ------------------

const VALIDATE_SYSTEM: &str = "\
You are the validation stage of an operational-intelligence planner for a \
surface mining operation. Classify the operator's question. Respond with JSON \
only, no prose:
{\"status\": one of \"Valid\"|\"Invalid\"|\"Ambiguous\"|\"Incomplete\", \
\"confidence\": number 0..1, \"language\": BCP-47 code, \"reason\": string or \
null, \"missing\": array of strings}.
Definitions:
- Valid: a specific, answerable operational question about the mine.
- Invalid: not about mining operations at all.
- Ambiguous: operational, but could mean several different investigations.
- Incomplete: operational and unambiguous, but missing required context such \
as what outcome to explain or which time range.
Set reason and missing only when status is not Valid. missing lists exactly \
what the operator must supply.";

const NORMALIZE_SYSTEM: &str = "\
You are the normalization stage of a mining operational-intelligence planner. \
Rewrite the operator's question into a single canonical operational sentence, \
preserving meaning and adding no information. Do not resolve times or places. \
Respond with JSON only: {\"normalized_question\": string}.";

const INTENT_SYSTEM: &str = "\
You are the intent stage of a mining operational-intelligence planner. \
Classify the question into exactly one intent. Respond with JSON only: \
{\"type\": one of \"Explain\"|\"Compare\"|\"Locate\"|\"Recommend\"|\"Predict\"\
|\"Summarize\", \"confidence\": number 0..1, \"rationale\": string}.
- Explain: why did something happen.
- Compare: how do two things differ.
- Locate: where is something / which thing is it.
- Recommend: what should be done.
- Predict: what will happen.
- Summarize: what is the overall state.";

const DOMAINS_SYSTEM: &str = "\
You are the domain stage of a mining operational-intelligence planner. Rank \
the operational domains the question concerns, most relevant first. Respond \
with JSON only: {\"domains\": [{\"domain\": name, \"confidence\": number 0..1, \
\"rationale\": string}]}. Valid domain names: OperationalPerformance, \
Production, Equipment, MaterialFlow, Infrastructure, Personnel, Safety, \
Security, Environment, Logistics. Include only genuinely relevant domains.";

const CONSTRAINTS_SYSTEM: &str = "\
You are the constraint-extraction stage of a mining operational-intelligence \
planner. Extract execution constraints from the question, verbatim — do not \
resolve them. Respond with JSON only: {\"time_expr\": string or null (the time \
phrase exactly as written, e.g. \"yesterday\", \"last shift\"), \
\"scope_phrase\": string or null (a named place, e.g. \"Pit 3\"), \
\"entities\": [{\"kind\": string, \"label\": string}] (specific named things, \
e.g. truck 14)}. Use null when the operator gave no such constraint.";
