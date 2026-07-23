//! # samaritan-registry
//!
//! Loads the registry and rejects an invalid one, with the registry-owned
//! checks from `REGISTRY.md`. This is where drift between spec and code
//! surfaces first: if the vocabulary and the mappings disagree with
//! themselves, the loader says so.
//!
//! ## Scope (stage 2)
//!
//! Implemented here: **E01–E07, E09–E17, W01–W06** — every check that depends
//! only on registry data. The graph-owned checks (E08, E18–E23, W07–W09)
//! validate the relations block and run in the graph crate (stage 3), which
//! the registry hands its vocabulary to.

pub mod calendar;
pub mod config;
pub mod finding;
mod validate;
mod validate_relations;

pub use samaritan_graph::{RelationGraph, RelationsConfig};

use std::collections::BTreeMap;

pub use calendar::CalendarError;
pub use config::RegistryConfig;
pub use finding::{Code, Finding};

/// The canonical mining registry, embedded for tests and the default binary.
pub const MINING_YAML: &str = include_str!("../mining.yaml");

/// A validated registry. Constructing one guarantees every stage-2 error check
/// passed; warnings are carried alongside, not fatal.
#[derive(Debug, Clone)]
pub struct Registry {
    config: RegistryConfig,
    warnings: Vec<Finding>,
    reverse_index: BTreeMap<String, Vec<String>>,
}

/// The reason a registry failed to load: a parse failure or one or more
/// validation errors.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("registry YAML did not parse: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("registry failed validation with {} error(s)", .0.len())]
    Invalid(Vec<Finding>),
}

impl LoadError {
    /// The validation errors, if this was a validation failure.
    pub fn errors(&self) -> &[Finding] {
        match self {
            LoadError::Invalid(v) => v,
            LoadError::Parse(_) => &[],
        }
    }

    /// Whether a given code is among the errors.
    pub fn has(&self, code: Code) -> bool {
        self.errors().iter().any(|f| f.code == code)
    }
}

impl Registry {
    /// Parse and validate YAML. `Ok` guarantees no errors; any warnings are
    /// available via [`Registry::warnings`].
    pub fn load(yaml: &str) -> Result<Registry, LoadError> {
        let config: RegistryConfig = serde_yaml::from_str(yaml)?;
        Self::from_config(config)
    }

    /// Validate an already-parsed config. Splits findings into errors (fatal)
    /// and warnings (carried).
    pub fn from_config(config: RegistryConfig) -> Result<Registry, LoadError> {
        let findings = validate::validate(&config);
        let (errors, warnings): (Vec<_>, Vec<_>) =
            findings.into_iter().partition(|f| f.code.is_error());
        if !errors.is_empty() {
            return Err(LoadError::Invalid(errors));
        }
        let reverse_index = build_reverse_index(&config);
        Ok(Registry {
            config,
            warnings,
            reverse_index,
        })
    }

    /// The canonical mining registry.
    pub fn mining() -> Result<Registry, LoadError> {
        Self::load(MINING_YAML)
    }

    pub fn config(&self) -> &RegistryConfig {
        &self.config
    }

    pub fn warnings(&self) -> &[Finding] {
        &self.warnings
    }

    /// Domain → the analyzers covering it, names sorted. The derived reverse
    /// index Planning uses for analyzer selection.
    pub fn analyzers_for(&self, domain: &str) -> &[String] {
        self.reverse_index
            .get(domain)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The full reverse index, for inspection and tests.
    pub fn reverse_index(&self) -> &BTreeMap<String, Vec<String>> {
        &self.reverse_index
    }

    /// Build the typed relation graph for traversal. Infallible on a validated
    /// registry — every reference resolved and every mode parsed at load.
    pub fn relation_graph(&self) -> RelationGraph {
        RelationGraph::from_config(&self.config.relations)
            .expect("a validated registry always builds its graph")
    }

    /// A site by id.
    pub fn site(&self, id: &str) -> Option<&config::SiteConfig> {
        self.config.sites.iter().find(|s| s.id == id)
    }

    /// The version list for a calendar family.
    pub fn calendar_versions(&self, family: &str) -> Option<&[config::CalendarVersionConfig]> {
        self.config.calendars.get(family).map(Vec::as_slice)
    }

    /// The strategy config for an intent name.
    pub fn strategy(&self, intent: &str) -> Option<&config::StrategyConfig> {
        self.config.strategies.get(intent)
    }

    /// The baseline default for an intent name.
    pub fn baseline_default(&self, intent: &str) -> Option<&config::BaselineDefault> {
        self.config.baseline_defaults.get(intent)
    }

    pub fn thresholds(&self) -> &config::ThresholdsConfig {
        &self.config.thresholds
    }

    /// The analyzer declaration by name.
    pub fn analyzer(&self, name: &str) -> Option<&config::AnalyzerConfig> {
        self.config.analyzers.iter().find(|a| a.name == name)
    }

    pub fn zones(&self) -> &[config::ZoneConfig] {
        &self.config.zones
    }
}

/// Build domain → sorted analyzer names from the analyzer declarations.
fn build_reverse_index(cfg: &RegistryConfig) -> BTreeMap<String, Vec<String>> {
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for a in &cfg.analyzers {
        for d in &a.domains {
            index.entry(d.clone()).or_default().push(a.name.clone());
        }
    }
    for names in index.values_mut() {
        names.sort();
        names.dedup();
    }
    index
}
