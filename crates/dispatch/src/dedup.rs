//! Deduplication. Two requirements are duplicates when everything about *what*
//! they ask matches — subject, observables, filters, spatial, window, baseline,
//! scope, entities, granularity, aggregations, ordering, limit, shape. On merge
//! the earliest id wins, requesters union, purposes concatenate, and the
//! strongest necessity is kept (`SCHEMA.md`, Deduplication).
//!
//! Overlap is a signal, not waste: two independent analyzers asking the same
//! thing is evidence the fact matters.

use samaritan_schema::{InformationRequirement, Necessity};

/// A comparison key over the semantic content of a requirement — everything
/// except id, requester, purpose, and necessity (which merge rather than
/// distinguish).
fn key(req: &InformationRequirement) -> String {
    let mut probe = req.clone();
    probe.id = samaritan_schema::Id(String::new());
    probe.plan_id = samaritan_schema::Id(String::new());
    probe.requested_by = Vec::new();
    probe.purpose = String::new();
    probe.necessity = Necessity::Required;
    serde_json::to_string(&probe).expect("requirement serializes")
}

fn strength(n: Necessity) -> u8 {
    match n {
        Necessity::Required => 2,
        Necessity::Preferred => 1,
        Necessity::Optional => 0,
    }
}

/// Merge duplicates. Output is ordered by (subject, first requester, id) so the
/// result is stable regardless of the order analyzers finished in.
pub fn merge(requirements: Vec<InformationRequirement>) -> Vec<InformationRequirement> {
    // Preserve first-seen order of distinct keys; merge into the first.
    let mut order: Vec<String> = Vec::new();
    let mut by_key: std::collections::HashMap<String, InformationRequirement> =
        std::collections::HashMap::new();

    for req in requirements {
        let k = key(&req);
        match by_key.get_mut(&k) {
            None => {
                order.push(k.clone());
                by_key.insert(k, req);
            }
            Some(existing) => {
                // Earliest id wins.
                if req.id.0 < existing.id.0 {
                    existing.id = req.id.clone();
                }
                for r in req.requested_by {
                    if !existing.requested_by.contains(&r) {
                        existing.requested_by.push(r);
                    }
                }
                if !existing.purpose.contains(&req.purpose) {
                    existing.purpose = format!("{} {}", existing.purpose, req.purpose);
                }
                if strength(req.necessity) > strength(existing.necessity) {
                    existing.necessity = req.necessity;
                }
            }
        }
    }

    let mut merged: Vec<InformationRequirement> = order
        .into_iter()
        .map(|k| {
            let mut r = by_key.remove(&k).unwrap();
            r.requested_by.sort();
            r.requested_by.dedup();
            r
        })
        .collect();

    merged.sort_by(|a, b| {
        a.subject
            .cmp(&b.subject)
            .then_with(|| a.requested_by.first().cmp(&b.requested_by.first()))
            .then_with(|| a.id.0.cmp(&b.id.0))
    });
    merged
}
