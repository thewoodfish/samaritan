//! Plan caching and replay (`PIPELINE.md`, Determinism).
//!
//! Planning is reproducible, so a plan is a pure function of its pinned inputs.
//! The cache key is a hash of exactly those pins — model, prompt template,
//! registry version, the question, and the world it was asked against. Change
//! any pin and the key changes; change nothing and it hits.
//!
//! On a hit the model is never called, and the served plan is marked
//! `cache_hit: true` so a plan can always be shown to have come from an
//! identical pipeline.

use std::collections::HashMap;
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use samaritan_registry::Registry;
use samaritan_schema::{Question, WorldVersion};

use crate::error::PlanningError;
use crate::model::{Model, PROMPT_TEMPLATE_VERSION};
use crate::orchestrate::{PlanOutcome, plan_question};

/// A content-addressed key over the pins that determine a plan.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey(String);

impl CacheKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Compute the cache key for a question. Uses the raw pins available before any
/// model call, so a hit skips the pipeline entirely.
///
/// Note: two different phrasings that would normalize to the same question hash
/// to different keys here. Collapsing them needs a normalized-question key,
/// which requires running the model first — a deliberate later refinement.
pub fn cache_key(
    reg: &Registry,
    model: &dyn Model,
    question: &Question,
    world: &WorldVersion,
) -> CacheKey {
    let mut h = Sha256::new();
    for part in [
        model.id(),
        PROMPT_TEMPLATE_VERSION,
        reg.config().registry_version.as_str(),
        question.text.as_str(),
        question.site.0.as_str(),
        question.operator.0.as_str(),
        question.organization.0.as_str(),
        &question.asked_at.to_rfc3339(),
        &world.log_position.to_string(),
    ] {
        h.update(part.as_bytes());
        h.update([0u8]); // domain separator so fields can't run together
    }
    CacheKey(format!("{:x}", h.finalize()))
}

/// A store for planned outcomes. Swappable — in-memory for now, disk or a
/// shared store later.
pub trait PlanCache {
    fn get(&self, key: &CacheKey) -> Option<PlanOutcome>;
    fn put(&self, key: CacheKey, outcome: PlanOutcome);
}

/// A process-local cache.
#[derive(Default)]
pub struct InMemoryPlanCache {
    map: Mutex<HashMap<CacheKey, PlanOutcome>>,
}

impl InMemoryPlanCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.map.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl PlanCache for InMemoryPlanCache {
    fn get(&self, key: &CacheKey) -> Option<PlanOutcome> {
        self.map.lock().unwrap().get(key).cloned()
    }
    fn put(&self, key: CacheKey, outcome: PlanOutcome) {
        self.map.lock().unwrap().insert(key, outcome);
    }
}

/// Plan a question, serving from cache when the pins match. On a hit the served
/// plan carries `cache_hit: true`; a miss plans fresh, stores it, and returns
/// it with `cache_hit: false`.
pub fn plan_question_cached(
    model: &dyn Model,
    reg: &Registry,
    question: &Question,
    world: WorldVersion,
    cache: &dyn PlanCache,
) -> Result<PlanOutcome, PlanningError> {
    let key = cache_key(reg, model, question, &world);
    if let Some(mut hit) = cache.get(&key) {
        if let PlanOutcome::Plan(plan) = &mut hit {
            plan.provenance.cache_hit = true;
        }
        return Ok(hit);
    }
    let outcome = plan_question(model, reg, question, world)?;
    cache.put(key, outcome.clone());
    Ok(outcome)
}
