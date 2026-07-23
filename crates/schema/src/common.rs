//! Common types used across every schema. Defined once (`SCHEMA.md`, Common
//! Types), including the canonical unit vocabulary from `REGISTRY.md`.
//!
//! The unit newtypes exist so a duration can never be assigned a mass. The
//! unit discipline is a type error, not a convention.

use serde::{Deserialize, Serialize};

/// An opaque, type-prefixed ULID string: `q_…`, `plan_…`, `req_…`.
///
/// Kept as a single newtype rather than one type per prefix — `SCHEMA.md`
/// treats `Id` as one type, and cross-references (`question_id: Id`) would
/// otherwise need constant conversion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub String);

impl Id {
    /// The prefix before the first underscore, e.g. `q` for `q_01J…`.
    /// Present for provenance tooling; not validated at construction.
    pub fn prefix(&self) -> Option<&str> {
        self.0.split_once('_').map(|(p, _)| p)
    }
}

impl From<&str> for Id {
    fn from(s: &str) -> Self {
        Id(s.to_owned())
    }
}

/// A point in time. Always UTC, serialized RFC 3339. Never read from the wall
/// clock inside the pipeline — it is always an input (`SCHEMA.md`, Question).
pub type Timestamp = chrono::DateTime<chrono::Utc>;

/// Semver string, e.g. `1.0.0`. Carried by every top-level schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(pub String);

impl From<&str> for SchemaVersion {
    fn from(s: &str) -> Self {
        SchemaVersion(s.to_owned())
    }
}

/// Raised when a bounded value is constructed outside its range.
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum UnitError {
    #[error("confidence {0} is outside [0.0, 1.0]")]
    ConfidenceOutOfRange(f64),
    #[error("ratio {0} is outside [0.0, 1.0]")]
    RatioOutOfRange(f64),
}

/// A confidence on the single documented scale (`SCHEMA.md`, Confidence bands).
/// Constrained to `[0.0, 1.0]` at construction and on deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(into = "f64", try_from = "f64")]
pub struct Confidence(f64);

impl Confidence {
    pub fn new(v: f64) -> Result<Self, UnitError> {
        if (0.0..=1.0).contains(&v) {
            Ok(Confidence(v))
        } else {
            Err(UnitError::ConfidenceOutOfRange(v))
        }
    }
    pub fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Confidence {
    type Error = UnitError;
    fn try_from(v: f64) -> Result<Self, Self::Error> {
        Confidence::new(v)
    }
}

impl From<Confidence> for f64 {
    fn from(c: Confidence) -> f64 {
        c.0
    }
}

/// A dimensionless ratio in `[0.0, 1.0]`. Never a percentage (`REGISTRY.md`,
/// Units and types). Validated like `Confidence`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(into = "f64", try_from = "f64")]
pub struct Ratio(f64);

impl Ratio {
    pub fn new(v: f64) -> Result<Self, UnitError> {
        if (0.0..=1.0).contains(&v) {
            Ok(Ratio(v))
        } else {
            Err(UnitError::RatioOutOfRange(v))
        }
    }
    pub fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Ratio {
    type Error = UnitError;
    fn try_from(v: f64) -> Result<Self, Self::Error> {
        Ratio::new(v)
    }
}

impl From<Ratio> for f64 {
    fn from(r: Ratio) -> f64 {
        r.0
    }
}

/// Canonical dimensioned quantities. Each wraps its value in its canonical
/// unit (`REGISTRY.md`): seconds, kilograms, metres, m/s, degrees. Distinct
/// types so the compiler rejects a duration used where a mass is meant.
macro_rules! quantity {
    ($(#[$m:meta])* $name:ident, $unit:literal) => {
        $(#[$m])*
        #[doc = concat!("Quantity in ", $unit, ".")]
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub f64);
    };
}

quantity!(Seconds, "seconds");
quantity!(Kilograms, "kilograms");
quantity!(Metres, "metres");
quantity!(MetresPerSecond, "metres per second");
quantity!(Degrees, "degrees, 0–360 clockwise from true north");

/// A non-negative count. Integral by nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Count(pub u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_rejects_out_of_range() {
        assert!(Ratio::new(-0.01).is_err());
        assert!(Ratio::new(1.01).is_err());
        assert_eq!(Ratio::new(0.0).unwrap().get(), 0.0);
        assert_eq!(Ratio::new(1.0).unwrap().get(), 1.0);
    }

    #[test]
    fn confidence_rejects_out_of_range() {
        assert!(Confidence::new(-0.1).is_err());
        assert!(Confidence::new(1.5).is_err());
        assert!(Confidence::new(0.94).is_ok());
    }

    #[test]
    fn ratio_deserialization_validates() {
        // A value outside range must fail to deserialize, not silently clamp.
        assert!(serde_json::from_str::<Ratio>("1.5").is_err());
        let r: Ratio = serde_json::from_str("0.5").unwrap();
        assert_eq!(r.get(), 0.5);
    }

    #[test]
    fn id_prefix() {
        assert_eq!(Id::from("plan_01J8XQ").prefix(), Some("plan"));
        assert_eq!(Id::from("bare").prefix(), None);
    }
}
