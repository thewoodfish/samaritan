//! # samaritan-graph
//!
//! The relation graph as a walkable structure — the machine-checkable subset
//! of `GRAPH.md`, loaded from `REGISTRY.md`'s relations block.
//!
//! Pure and traversable. An analyzer walks this; it contains no mining
//! knowledge of its own — every edge comes from the registry. The crate holds
//! no vocabulary: the registry, which owns the vocabulary, validates the
//! references (E18–E23) against a graph built here.
//!
//! ## What lives here
//!
//! - [`config::RelationsConfig`] — the raw relations block (serde)
//! - [`RelationGraph`] — the typed graph, built via [`RelationGraph::from_config`]
//! - traversal: [`RelationGraph::decomposition_walk`],
//!   [`RelationGraph::backward_influence_walk`], and the neighbour lookups
//! - [`DecompMode`] — additive vs multiplicative, in the type not a string

pub mod config;
mod model;

pub use config::RelationsConfig;
pub use model::{
    BuildError, Confound, DecompMode, Decompose, Influence, NodeRef, Partition, Reached,
    RelationGraph, RollsUp,
};
