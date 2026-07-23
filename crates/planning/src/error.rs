//! Planning errors. The deterministic layer returns these; the model layer
//! (stage 6) maps `Unresolvable` and friends into an `Incomplete`
//! `ParsedQuestion` for the caller to re-ask.

/// Why a constraint could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    #[error("unknown site '{0}'")]
    UnknownSite(String),
    #[error("site '{0}' names calendar family '{1}', which is not in the registry")]
    UnknownCalendarFamily(String, String),
    /// A phrase not in the resolution table. Becomes `Incomplete` — planning
    /// never guesses.
    #[error("cannot resolve time expression '{0}'")]
    Unresolvable(String),
    #[error("time window spans a calendar change and cannot be one comparison: {0}")]
    SpansCalendarChange(String),
    #[error("no calendar version covers {0}")]
    Uncovered(String),
    #[error("timezone '{0}' is not a known IANA zone")]
    BadTimezone(String),
    #[error("could not construct a local time for {0}")]
    AmbiguousLocalTime(String),
    /// A named place the registry has not been taught.
    #[error("unknown place '{0}'")]
    UnknownPlace(String),
}

/// Why plan assembly failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanningError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error("no analyzer covers any ranked domain — nothing to investigate")]
    NoAnalyzers,
    #[error("no domain survived the relevance floor")]
    NoDomains,
    #[error("intent '{0}' has no strategy in the registry")]
    MissingStrategy(String),
}
