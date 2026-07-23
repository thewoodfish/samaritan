//! Domain culling and analyzer selection — the deterministic lookups
//! (`PLANNING.md`, Stages 6 and 8; `REGISTRY.md`, Thresholds/Precedence).
//!
//! The culling order is load-bearing: drop below the floor, rank, truncate to
//! `max_domains`, resolve analyzers, then truncate to `max_analyzers` without
//! ever leaving a domain half-served.

use samaritan_registry::Registry;
use samaritan_schema::{AnalyzerRef, DomainType, RankedDomain, SchemaVersion};

use crate::keys::domain_key;

/// Steps 1–3 of the precedence: drop domains below the relevance floor, rank
/// the survivors by confidence, and truncate to `max_domains`. Rank is
/// reassigned 1.. so it is always dense and strictly increasing.
pub fn cull_domains(reg: &Registry, mut ranked: Vec<RankedDomain>) -> Vec<RankedDomain> {
    let th = reg.thresholds();
    ranked.retain(|d| d.confidence.get() >= th.domain_relevance_floor);
    // Rank by confidence desc; break ties by domain name for a stable order.
    ranked.sort_by(|a, b| {
        b.confidence
            .get()
            .partial_cmp(&a.confidence.get())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| domain_key(a.domain).cmp(domain_key(b.domain)))
    });
    ranked.truncate(th.max_domains as usize);
    for (i, d) in ranked.iter_mut().enumerate() {
        d.rank = (i + 1) as u32;
    }
    ranked
}

/// Steps 4–5: resolve analyzers from the ranked domains, then bound to
/// `max_analyzers` by dropping whole domains from the lowest rank upward — a
/// domain is fully covered or not at all.
///
/// Returns the analyzers sorted by name (the pipeline's normalized order).
pub fn select_analyzers(reg: &Registry, domains: &[RankedDomain]) -> Vec<AnalyzerRef> {
    let th = reg.thresholds();
    // name -> the domains that selected it, in encounter order.
    let mut chosen: Vec<(String, Vec<DomainType>)> = Vec::new();
    let index = |name: &str, chosen: &[(String, Vec<DomainType>)]| {
        chosen.iter().position(|(n, _)| n == name)
    };

    for d in domains {
        let names = reg.analyzers_for(domain_key(d.domain));
        // How many genuinely new analyzers this domain introduces.
        let new: Vec<&String> = names
            .iter()
            .filter(|n| index(n, &chosen).is_none())
            .collect();
        if chosen.len() + new.len() > th.max_analyzers as usize {
            break; // adding this domain would overflow — stop, never split it
        }
        for name in names {
            match index(name, &chosen) {
                Some(i) => chosen[i].1.push(d.domain),
                None => chosen.push((name.clone(), vec![d.domain])),
            }
        }
    }

    let mut refs: Vec<AnalyzerRef> = chosen
        .into_iter()
        .map(|(name, mut doms)| {
            doms.dedup();
            let version = reg
                .analyzer(&name)
                .map(|a| SchemaVersion(a.version.clone()))
                .unwrap_or_else(|| SchemaVersion("0.0.0".into()));
            let rationale = format!(
                "declares coverage of {}",
                doms.iter()
                    .map(|d| domain_key(*d))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            AnalyzerRef {
                name,
                version,
                domains: doms,
                rationale,
            }
        })
        .collect();
    refs.sort_by(|a, b| a.name.cmp(&b.name));
    refs
}
