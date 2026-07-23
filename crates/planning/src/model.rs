//! The model seam. The four inference stages depend on this trait, not on any
//! particular provider — so the pipeline is testable without a server, and the
//! provider is swappable (`PIPELINE.md`, Determinism).
//!
//! Reproducibility is the implementation's responsibility: temperature 0, a
//! pinned model, JSON output. The trait only promises a JSON reply for a
//! `(stage, system, user)` prompt.

/// The version of the planning prompt templates. Pinned into `PlanProvenance`
/// and, in stage 9, the cache key. Bump when any template below changes.
pub const PROMPT_TEMPLATE_VERSION: &str = "planning/2026-07-01";

/// A language model the planner can call. `stage` is a machine tag
/// (`"validate"`, `"intent"`, …) — providers may ignore it; test doubles key
/// their canned replies on it.
pub trait Model {
    /// An identifier for provenance: the model actually used.
    fn id(&self) -> &str;

    /// Complete a prompt to a JSON value. Must be deterministic for a fixed
    /// `(stage, system, user)` — temperature 0.
    fn complete_json(
        &self,
        stage: &str,
        system: &str,
        user: &str,
    ) -> Result<serde_json::Value, ModelError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model transport failed: {0}")]
    Transport(String),
    #[error("model returned invalid JSON: {0}")]
    BadJson(String),
    #[error("model reply missing field '{0}'")]
    MissingField(String),
}
