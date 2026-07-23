//! `InformationRequirement` — one analyzer's declaration of one thing it needs
//! to know (`SCHEMA.md`, InformationRequirement). The union of what these can
//! express is the Reality Engine's request API.

use serde::{Deserialize, Serialize};

use crate::common::{Id, Metres, Seconds};
use crate::constraints::{EntityRef, SpatialScope, TimeWindow};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InformationRequirement {
    pub id: Id,
    pub plan_id: Id,
    /// Analyzer names; more than one after deduplication.
    pub requested_by: Vec<String>,
    /// Why this is needed, human readable.
    pub purpose: String,
    pub necessity: Necessity,

    /// Subject vocabulary, `REGISTRY.md`.
    pub subject: String,
    /// Observable vocabulary, `REGISTRY.md`.
    pub observables: Vec<String>,
    pub filters: Vec<Filter>,
    pub spatial: Vec<SpatialPredicate>,

    pub window: TimeWindow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<TimeWindow>,

    pub scope: SpatialScope,
    pub entities: Vec<EntityRef>,

    pub granularity: Granularity,
    pub aggregations: Vec<Aggregation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordering: Option<Ordering>,
    /// `limit` without `ordering` is invalid — checked at dispatch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    pub expected_shape: Shape,

    /// Always 0 in this phase.
    pub round: u32,
    /// Reserved for part 2, always empty in this phase.
    pub depends_on: Vec<Id>,
}

/// How badly the analyzer needs this. Lets the engine degrade under load and
/// lets a partial answer be interpreted rather than discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Necessity {
    /// The analyzer cannot proceed without it.
    Required,
    /// Materially improves the answer.
    Preferred,
    /// Nice to have.
    Optional,
}

/// A declarative predicate, not a query language (`SCHEMA.md`, Filter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    /// Observable or attribute vocabulary.
    pub field: String,
    pub op: FilterOp,
    pub value: FilterValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOp {
    Eq,
    Neq,
    In,
    NotIn,
    Gt,
    Gte,
    Lt,
    Lte,
}

/// A scalar or a list of scalars. Serialized as the bare value — `5`,
/// `"ore"`, `[a, b]` — not wrapped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterValue {
    List(Vec<Scalar>),
    One(Scalar),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

/// Geometric narrowing. Separate from `Filter` because the operand is
/// geometry, not a scalar (`SCHEMA.md`, SpatialPredicate). Multiple predicates
/// combine with AND.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpatialPredicate {
    pub op: SpatialOp,
    pub region: RegionRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialOp {
    Within,
    Intersects,
    Contains,
    Outside,
}

/// A region: a named zone entity, a radius, or a polygon. Samaritan never
/// carries zone geometry — a `Named` region references a zone entity by id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionRef {
    pub kind: RegionKind,
    /// Required when `kind == Named` — a zone entity id.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub reference: Option<Id>,
    /// Required when `kind == Radius`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<Point>,
    /// Required when `kind == Radius`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<Metres>,
    /// Required when `kind == Bounds` — a closed polygon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Vec<Point>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionKind {
    Named,
    Radius,
    Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

/// The grain of the returned data. `Bucketed` carries its interval.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Granularity {
    /// Every individual record or event.
    Raw,
    /// One row per occurrence of the subject.
    PerEvent,
    /// Fixed interval.
    Bucketed { bucket_size: Seconds },
    /// One row per shift.
    Shift,
    /// One row per operational day.
    Daily,
}

/// An aggregation binds an operation to a field. Empty aggregation list means
/// return the underlying records (`SCHEMA.md`, Aggregation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aggregation {
    pub op: AggregationOp,
    /// An observable of the subject; omitted for `Count`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// Required when `op == Histogram`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bins: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AggregationOp {
    Count,
    Sum,
    Mean,
    Median,
    Min,
    Max,
    P90,
    P95,
    P99,
    StdDev,
    Rate,
    Histogram,
}

/// Rank the result, optionally taking only the top. `limit` without `ordering`
/// is invalid (`SCHEMA.md`, Ordering).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ordering {
    /// An observable, or an aggregation of one.
    pub by: String,
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Asc,
    Desc,
}

/// What the analyzer expects back — a declaration of expectation, not a
/// rendering instruction (`SCHEMA.md`, Shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    Scalar,
    Series,
    Set,
    Table,
    Histogram,
}
