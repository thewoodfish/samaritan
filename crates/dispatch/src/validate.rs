//! Requirement validation at the boundary. A well-formed requirement can still
//! be impossible to answer — an observable with no source, a subject the domain
//! layer never registered, a zone Samaritan was not taught. Those are recorded
//! as unserviceable, never dropped: unavailable is not empty (`ENGINE.md`).
//!
//! This is also where E08 lives — a spatial predicate is only valid on a
//! spatial subject — since it is a requirement-time rule, not a registry one.

use samaritan_registry::Registry;
use samaritan_schema::{InformationRequirement, RegionKind, UnserviceableReason};

/// Check a requirement against the registry vocabulary. `None` if it can be
/// served; otherwise the reason and a human-readable detail.
pub fn check(
    reg: &Registry,
    req: &InformationRequirement,
) -> Option<(UnserviceableReason, String)> {
    let subject = match reg.config().subjects.get(&req.subject) {
        Some(s) => s,
        None => {
            return Some((
                UnserviceableReason::UnavailableSubject,
                format!("subject '{}' is not registered", req.subject),
            ));
        }
    };

    // Every observable must belong to the subject.
    for o in &req.observables {
        if !subject.observables.contains_key(o) {
            return Some((
                UnserviceableReason::UnavailableObservable,
                format!("'{o}' is not an observable of '{}'", req.subject),
            ));
        }
    }

    // Group-by keys must be attributes of the subject.
    for key in &req.group_by {
        if !subject.attributes.contains_key(key) {
            return Some((
                UnserviceableReason::UnregisteredTerm,
                format!("group_by '{key}' is not an attribute of '{}'", req.subject),
            ));
        }
    }

    // Filter, aggregation, and ordering fields must be registry terms.
    for f in &req.filters {
        if !subject.observables.contains_key(&f.field) && !subject.attributes.contains_key(&f.field)
        {
            return Some((
                UnserviceableReason::UnregisteredTerm,
                format!("filter field '{}' is unknown to '{}'", f.field, req.subject),
            ));
        }
    }
    for a in &req.aggregations {
        if let Some(field) = &a.field
            && !subject.observables.contains_key(field)
        {
            return Some((
                UnserviceableReason::UnregisteredTerm,
                format!(
                    "aggregation field '{field}' is unknown to '{}'",
                    req.subject
                ),
            ));
        }
    }
    if let Some(ord) = &req.ordering
        && !subject.observables.contains_key(&ord.by)
    {
        return Some((
            UnserviceableReason::UnregisteredTerm,
            format!(
                "ordering field '{}' is unknown to '{}'",
                ord.by, req.subject
            ),
        ));
    }
    // limit without ordering is meaningless — an arbitrary subset (SCHEMA.md).
    if req.limit.is_some() && req.ordering.is_none() {
        return Some((
            UnserviceableReason::UnregisteredTerm,
            "limit without ordering is not a meaningful request".to_owned(),
        ));
    }

    // Spatial predicates: only on a spatial subject (E08), and a named region
    // must be a registered zone.
    if !req.spatial.is_empty() && !subject.spatial {
        return Some((
            UnserviceableReason::UnregisteredTerm,
            format!(
                "subject '{}' is not spatial; no spatial predicate applies",
                req.subject
            ),
        ));
    }
    for p in &req.spatial {
        if p.region.kind == RegionKind::Named {
            let ok = p.region.reference.as_ref().is_some_and(|r| {
                reg.zones()
                    .iter()
                    .any(|z| z.entity.as_deref() == Some(r.0.as_str()))
            });
            if !ok {
                return Some((
                    UnserviceableReason::UnknownZone,
                    "named region does not reference a registered zone".to_owned(),
                ));
            }
        }
    }

    None
}
