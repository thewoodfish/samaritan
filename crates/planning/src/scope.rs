//! Spatial resolution — an operator's place name to a zone entity id
//! (`PLANNING.md`, Stage 5). Samaritan may only reason about places it has been
//! taught; an unknown place does not resolve.

use samaritan_registry::Registry;
use samaritan_schema::{Id, ScopeKind, SpatialScope};

use crate::error::ResolveError;

/// Resolve an optional place phrase to a `SpatialScope`.
///
/// `None` means the operator did not narrow scope — recorded explicitly as
/// `Unspecified`, so "did not narrow" is distinguishable from "never
/// considered".
pub fn resolve_scope(reg: &Registry, phrase: Option<&str>) -> Result<SpatialScope, ResolveError> {
    let Some(phrase) = phrase else {
        return Ok(SpatialScope {
            kind: ScopeKind::Unspecified,
            reference: None,
            label: "entire site".to_owned(),
        });
    };

    let key = normalize(phrase);
    let zone = reg.zones().iter().find(|z| {
        normalize(&z.key) == key
            || normalize(&z.label) == key
            || z.label.eq_ignore_ascii_case(phrase)
    });

    match zone {
        Some(z) => Ok(SpatialScope {
            kind: ScopeKind::Named,
            reference: z.entity.as_deref().map(Id::from),
            label: z.label.clone(),
        }),
        None => Err(ResolveError::UnknownPlace(phrase.to_owned())),
    }
}

/// Fold a label to a comparable key: lowercase, spaces to underscores.
fn normalize(s: &str) -> String {
    s.trim().to_lowercase().replace([' ', '-'], "_")
}
