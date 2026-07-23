//! # samaritan-planning
//!
//! Transforms a `Question` into an `InvestigationPlan`. Eight stages: the
//! deterministic half (constraint resolution, strategy and analyzer lookup)
//! and the model-backed half (validation, normalization, intent, domains).
//!
//! Reproducible, not deterministic — temperature 0, pinned model, cached.
//! See `PLANNING.md`.
//!
//! Deterministic stages built in stage 4; model stages in stage 6.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {}
}
