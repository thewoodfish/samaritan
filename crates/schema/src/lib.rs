//! # samaritan-schema
//!
//! Every contract in the Samaritan pipeline, as a type. No behaviour beyond
//! the invariants a contract must guarantee (bounded values validate at
//! construction).
//!
//! This crate is the single source of truth in code, mirroring `SCHEMA.md`.
//! It depends on nothing else in the workspace — the language cannot be
//! defined in terms of the things that speak it.
//!
//! ## Schema checklist (`SCHEMA.md` → type)
//!
//! Pipeline artifacts, each carrying the envelope (`id`, `schema_version`,
//! `created_at`):
//!
//! - [`Question`]
//! - [`ParsedQuestion`] (+ [`ValidationStatus`])
//! - [`InvestigationPlan`] (+ [`AnalyzerRef`], [`PlanProvenance`])
//! - [`InformationRequirement`]
//! - [`RequirementSet`] (+ [`Unserviceable`], [`ExecutionReport`],
//!   [`AnalyzerOutcome`])
//!
//! Embedded value objects:
//!
//! - [`Intent`] (+ [`IntentType`])
//! - [`OperationalDomains`] (+ [`RankedDomain`], [`DomainType`])
//! - [`InvestigationConstraints`] (+ [`TimeWindow`], [`WorldVersion`],
//!   [`Priority`], [`SpatialScope`], [`EntityRef`])
//! - [`InvestigationStrategy`]
//! - Requirement parts: [`Necessity`], [`Filter`], [`SpatialPredicate`],
//!   [`RegionRef`], [`Point`], [`Granularity`], [`Aggregation`],
//!   [`Ordering`], [`Shape`]
//!
//! Common: [`Id`], [`Timestamp`], [`SchemaVersion`], [`Confidence`],
//! [`Ratio`], and the unit newtypes ([`Seconds`], [`Kilograms`], [`Metres`],
//! [`MetresPerSecond`], [`Degrees`], [`Count`]).

pub mod common;
pub mod constraints;
pub mod domains;
pub mod intent;
pub mod parsed;
pub mod plan;
pub mod question;
pub mod requirement;
pub mod requirement_set;
pub mod strategy;

pub use common::{
    Confidence, Count, Degrees, Id, Kilograms, Metres, MetresPerSecond, Ratio, SchemaVersion,
    Seconds, Timestamp, UnitError,
};
pub use constraints::{
    EntityRef, InvestigationConstraints, Priority, ScopeKind, SpatialScope, TimeWindow,
    WorldVersion,
};
pub use domains::{DomainType, OperationalDomains, RankedDomain};
pub use intent::{Intent, IntentType};
pub use parsed::{ParsedQuestion, ValidationStatus};
pub use plan::{AnalyzerRef, InvestigationPlan, PlanProvenance};
pub use question::Question;
pub use requirement::{
    Aggregation, AggregationOp, Direction, Filter, FilterOp, FilterValue, Granularity,
    InformationRequirement, Necessity, Ordering, Point, RegionKind, RegionRef, Scalar, Shape,
    SpatialOp, SpatialPredicate,
};
pub use requirement_set::{
    AnalyzerOutcome, ExecutionReport, RequirementSet, Unserviceable, UnserviceableReason,
};
pub use strategy::InvestigationStrategy;
