//! Relation-graph validation (E18–E23, W07–W09). Runs in the registry, not
//! the graph crate, because every check needs the vocabulary the registry
//! owns. Operates on the raw relations config; the typed graph is built
//! separately for traversal.

use std::collections::{BTreeMap, BTreeSet};

use samaritan_graph::config::RelationsConfig;

use crate::config::RegistryConfig;
use crate::finding::{Code, Finding};

/// A flattened view of the vocabulary, built once per validation.
struct Vocab<'a> {
    /// subject -> (observable -> (type, unit)).
    observables: BTreeMap<&'a str, BTreeMap<&'a str, (&'a str, Option<&'a str>)>>,
    /// subject -> attribute names.
    attributes: BTreeMap<&'a str, BTreeSet<&'a str>>,
    /// subjects listed by at least one analyzer.
    reachable: BTreeSet<&'a str>,
}

impl<'a> Vocab<'a> {
    fn build(cfg: &'a RegistryConfig) -> Vocab<'a> {
        let mut observables = BTreeMap::new();
        let mut attributes = BTreeMap::new();
        for (sname, s) in &cfg.subjects {
            let obs: BTreeMap<&str, (&str, Option<&str>)> = s
                .observables
                .iter()
                .map(|(o, spec)| (o.as_str(), (spec.ty.as_str(), spec.unit.as_deref())))
                .collect();
            observables.insert(sname.as_str(), obs);
            attributes.insert(
                sname.as_str(),
                s.attributes.keys().map(String::as_str).collect(),
            );
        }
        let reachable = cfg
            .analyzers
            .iter()
            .flat_map(|a| a.subjects.iter().map(String::as_str))
            .collect();
        Vocab {
            observables,
            attributes,
            reachable,
        }
    }

    /// Does `subject.field` name an observable or attribute in the vocabulary?
    fn resolves(&self, subject: &str, field: &str) -> bool {
        self.observables
            .get(subject)
            .is_some_and(|o| o.contains_key(field))
            || self
                .attributes
                .get(subject)
                .is_some_and(|a| a.contains(field))
    }

    fn observable(&self, subject: &str, field: &str) -> Option<(&str, Option<&str>)> {
        self.observables
            .get(subject)
            .and_then(|o| o.get(field).copied())
    }
}

/// Split a qualified ref `subject.field`. Malformed refs (no dot) are treated
/// as unresolved and surface as E18.
fn split(qualified: &str) -> (&str, &str) {
    qualified.split_once('.').unwrap_or((qualified, ""))
}

fn is_numeric(ty: &str) -> bool {
    matches!(
        ty,
        "duration" | "mass" | "distance" | "speed" | "angle" | "ratio" | "count" | "float"
    )
}

pub fn validate_relations(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    let rel: &RelationsConfig = &cfg.relations;
    let vocab = Vocab::build(cfg);

    // Track which qualified observable refs appear anywhere in the relations,
    // for W07.
    let mut used_refs: BTreeSet<String> = BTreeSet::new();

    // E18 helper: a qualified ref must resolve, and is noted as used.
    let check_ref = |q: &str, ctx: &str, f: &mut Vec<Finding>, used: &mut BTreeSet<String>| {
        let (s, field) = split(q);
        used.insert(q.to_owned());
        if !vocab.resolves(s, field) {
            f.push(Finding::new(
                Code::E18RelationUnknownRef,
                format!("{ctx} references '{q}', not an observable or attribute"),
            ));
        }
    };

    // ---- decomposes: E18, E19, E20 ----------------------------------------
    for d in &rel.decomposes {
        check_ref(&d.whole, "decomposition whole", f, &mut used_refs);
        for p in &d.parts {
            check_ref(p, "decomposition part", f, &mut used_refs);
        }
        match d
            .mode
            .as_deref()
            .and_then(samaritan_graph::DecompMode::parse)
        {
            Some(samaritan_graph::DecompMode::Additive) => {
                additive_units(&vocab, d, f);
            }
            Some(samaritan_graph::DecompMode::Multiplicative) => {}
            None => f.push(Finding::new(
                Code::E19DecompositionBadMode,
                format!(
                    "decomposition of '{}' has {} mode",
                    d.whole,
                    match &d.mode {
                        Some(m) => format!("invalid '{m}'"),
                        None => "no".into(),
                    }
                ),
            )),
        }
    }

    // ---- partitions: E18 (metric), E21 (keys) -----------------------------
    for p in &rel.partitions {
        check_ref(&p.metric, "partition metric", f, &mut used_refs);
        let (subject, _) = split(&p.metric);
        let attrs = vocab.attributes.get(subject);
        for key in &p.by {
            let known = attrs.is_some_and(|a| a.contains(key.as_str()));
            if !known {
                f.push(Finding::new(
                    Code::E21PartitionUnknownAttribute,
                    format!(
                        "partition of '{}' names '{key}', not an attribute of '{subject}'",
                        p.metric
                    ),
                ));
            }
        }
    }

    // ---- confounds: E18, E22 ----------------------------------------------
    for c in &rel.confounds {
        check_ref(&c.factor, "confounder factor", f, &mut used_refs);
        check_ref(&c.affects, "confounder affects", f, &mut used_refs);
        reachable_subject(&vocab, &c.factor, "confounder factor", f);
        reachable_subject(&vocab, &c.affects, "confounder affects", f);
    }

    // ---- influences: E18, E22, W08 ----------------------------------------
    for i in &rel.influences {
        check_ref(&i.from, "influence from", f, &mut used_refs);
        check_ref(&i.to, "influence to", f, &mut used_refs);
        reachable_subject(&vocab, &i.from, "influence from", f);
        reachable_subject(&vocab, &i.to, "influence to", f);
        if i.lag.is_none() {
            f.push(Finding::new(
                Code::W08InfluenceWithoutLag,
                format!("influence {} -> {} declares no lag", i.from, i.to),
            ));
        }
    }

    // ---- rolls_up: E23 ----------------------------------------------------
    for r in &rel.rolls_up {
        for (subject, role) in [(&r.from, "from"), (&r.to, "to")] {
            if !vocab.observables.contains_key(subject.as_str()) {
                f.push(Finding::new(
                    Code::E23RollsUpUnknownSubject,
                    format!("rolls_up {role} names unknown subject '{subject}'"),
                ));
            }
        }
    }

    // ---- W07: numeric observable in no relation ---------------------------
    for (subject, obs) in &vocab.observables {
        for (oname, (ty, _)) in obs {
            let qualified = format!("{subject}.{oname}");
            if is_numeric(ty) && !used_refs.contains(&qualified) {
                f.push(Finding::new(
                    Code::W07NumericObservableInNoRelation,
                    format!("numeric observable '{qualified}' appears in no relation"),
                ));
            }
        }
    }

    // ---- W09: subject with no partitions ----------------------------------
    let partitioned: BTreeSet<&str> = rel.partitions.iter().map(|p| split(&p.metric).0).collect();
    for subject in vocab.observables.keys() {
        if !partitioned.contains(subject) {
            f.push(Finding::new(
                Code::W09SubjectWithoutPartitions,
                format!("subject '{subject}' has no partitions declared"),
            ));
        }
    }
}

/// E20: an additive decomposition's whole and parts must share a unit.
fn additive_units(
    vocab: &Vocab,
    d: &samaritan_graph::config::DecomposeConfig,
    f: &mut Vec<Finding>,
) {
    let (ws, wf) = split(&d.whole);
    let Some(whole_unit) = vocab.observable(ws, wf) else {
        return; // unresolved whole is E18's problem
    };
    for p in &d.parts {
        let (ps, pf) = split(p);
        if let Some(part_unit) = vocab.observable(ps, pf)
            && part_unit != whole_unit
        {
            f.push(Finding::new(
                Code::E20AdditivePartsUnitMismatch,
                format!(
                    "additive decomposition of '{}' mixes units: '{p}' is {:?}, whole is {:?}",
                    d.whole, part_unit, whole_unit
                ),
            ));
        }
    }
}

/// E22: a confounder or influence must name reachable subjects.
fn reachable_subject(vocab: &Vocab, qualified: &str, ctx: &str, f: &mut Vec<Finding>) {
    let (subject, _) = split(qualified);
    if !vocab.reachable.contains(subject) {
        f.push(Finding::new(
            Code::E22RelationUnreachableSubject,
            format!("{ctx} '{qualified}' names subject '{subject}', which no analyzer reaches"),
        ));
    }
}
