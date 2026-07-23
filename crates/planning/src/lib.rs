//! # samaritan-planning
//!
//! Transforms a `Question` into an `InvestigationPlan`.
//!
//! Stage 4 builds the **deterministic** half: constraint resolution (time,
//! baseline, scope), the registry lookups (strategy, analyzer selection), and
//! plan assembly. Given the same inputs and registry, the plan is
//! byte-identical.
//!
//! The **model** half — validation, normalization, intent extraction, domain
//! ranking — arrives in stage 6 and feeds its outputs into [`PlanInputs`].
//!
//! Planning holds no session and never reads the wall clock: `asked_at` is an
//! input, and an unresolvable expression returns an error the caller turns
//! into an `Incomplete` re-ask (`PLANNING.md`).

mod assemble;
mod cache;
mod error;
mod keys;
mod model;
mod ollama;
mod orchestrate;
mod scope;
mod select;
mod stages;
mod time;

pub use assemble::{PlanInputs, assemble_plan, derive_strategy};
pub use cache::{CacheKey, InMemoryPlanCache, PlanCache, cache_key, plan_question_cached};
pub use error::{PlanningError, ResolveError};
pub use model::{Model, ModelError, PROMPT_TEMPLATE_VERSION};
pub use ollama::OllamaModel;
pub use orchestrate::{DeterminismReport, PlanOutcome, determinism_report, plan_question};
pub use scope::resolve_scope;
pub use select::{cull_domains, select_analyzers};
pub use time::{baseline_window, resolve_time};
