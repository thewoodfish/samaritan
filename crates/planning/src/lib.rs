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
mod error;
mod keys;
mod scope;
mod select;
mod time;

pub use assemble::{PlanInputs, assemble_plan, derive_strategy};
pub use error::{PlanningError, ResolveError};
pub use scope::resolve_scope;
pub use select::{cull_domains, select_analyzers};
pub use time::{baseline_window, resolve_time};
