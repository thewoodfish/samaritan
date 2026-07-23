//! # samaritan-dispatch
//!
//! Fans the `InvestigationPlan` out to its analyzers, runs them in parallel
//! against one pinned `world_version`, and collects a single `RequirementSet`
//! — deduplicated, ordered, with a truthful execution report.
//!
//! Orchestration only. Dispatch performs no reasoning and never alters the
//! semantic content of a requirement. See `PIPELINE.md`, Stage 2.
//!
//! Built in stage 7.

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {}
}
