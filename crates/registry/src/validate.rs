//! The validation suite. Each check maps to one code in `REGISTRY.md`.
//!
//! Stage 2 covers the registry-owned checks: E01–E07, E09–E17, W01–W06.
//! The graph-owned checks (E08, E18–E23, W07–W09) run in the graph crate
//! (stage 3), which validates the relations block against this vocabulary.
//!
//! Every check appends to a shared finding list; nothing short-circuits, so a
//! single load reports every problem at once.

use std::collections::{BTreeMap, BTreeSet};

use crate::calendar;
use crate::config::RegistryConfig;
use crate::finding::{Code, Finding};

/// Run every stage-2 check. Findings are returned sorted by code then detail,
/// so output is deterministic.
pub fn validate(cfg: &RegistryConfig) -> Vec<Finding> {
    let mut f = Vec::new();

    let subjects: BTreeSet<&str> = cfg.subjects.keys().map(String::as_str).collect();
    let domains: BTreeSet<&str> = cfg.domains.iter().map(String::as_str).collect();
    let intents: BTreeSet<&str> = cfg.intents.iter().map(String::as_str).collect();
    let enums: BTreeSet<&str> = cfg.enumerations.keys().map(String::as_str).collect();

    analyzers(cfg, &subjects, &domains, &intents, &mut f);
    enum_attributes(cfg, &enums, &mut f);
    ratio_definitions(cfg, &mut f);
    derived_observables(cfg, &mut f);
    observable_units(cfg, &mut f);
    zones(cfg, &mut f);
    calendars(cfg, &mut f);
    strategies_and_baselines(cfg, &intents, &mut f);
    coverage_warnings(cfg, &mut f);
    crate::validate_relations::validate_relations(cfg, &mut f);

    f.sort_by(|a, b| {
        a.code
            .as_str()
            .cmp(b.code.as_str())
            .then(a.detail.cmp(&b.detail))
    });
    f
}

/// E01–E04: analyzer declarations against the closed sets.
fn analyzers(
    cfg: &RegistryConfig,
    subjects: &BTreeSet<&str>,
    domains: &BTreeSet<&str>,
    intents: &BTreeSet<&str>,
    f: &mut Vec<Finding>,
) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for a in &cfg.analyzers {
        if !seen.insert(a.name.as_str()) {
            f.push(Finding::new(
                Code::E04DuplicateAnalyzerName,
                format!("analyzer '{}' declared more than once", a.name),
            ));
        }
        for s in &a.subjects {
            if !subjects.contains(s.as_str()) {
                f.push(Finding::new(
                    Code::E01AnalyzerUnknownSubject,
                    format!("analyzer '{}' declares unknown subject '{s}'", a.name),
                ));
            }
        }
        for d in &a.domains {
            if !domains.contains(d.as_str()) {
                f.push(Finding::new(
                    Code::E02AnalyzerUnknownDomain,
                    format!("analyzer '{}' declares unknown domain '{d}'", a.name),
                ));
            }
        }
        for i in &a.intents {
            if !intents.contains(i.as_str()) {
                f.push(Finding::new(
                    Code::E03AnalyzerUnknownIntent,
                    format!("analyzer '{}' declares unknown intent '{i}'", a.name),
                ));
            }
        }
    }
}

/// E05: every enum-typed attribute names an existing enumeration.
fn enum_attributes(cfg: &RegistryConfig, enums: &BTreeSet<&str>, f: &mut Vec<Finding>) {
    for (sname, s) in &cfg.subjects {
        for (aname, attr) in &s.attributes {
            if attr.ty == "enum" {
                match &attr.of {
                    Some(e) if enums.contains(e.as_str()) => {}
                    Some(e) => f.push(Finding::new(
                        Code::E05AttributeUnknownEnumeration,
                        format!("{sname}.{aname} names unknown enumeration '{e}'"),
                    )),
                    None => f.push(Finding::new(
                        Code::E05AttributeUnknownEnumeration,
                        format!("{sname}.{aname} is enum-typed but names no enumeration"),
                    )),
                }
            }
        }
    }
}

/// E06: a ratio's definition references only observables on its own subject.
fn ratio_definitions(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    for (sname, s) in &cfg.subjects {
        let own: BTreeSet<&str> = s.observables.keys().map(String::as_str).collect();
        for (oname, o) in &s.observables {
            let Some(def) = &o.definition else { continue };
            for token in identifiers(def) {
                if !own.contains(token.as_str()) {
                    f.push(Finding::new(
                        Code::E06RatioDefinitionUnknownObservable,
                        format!("{sname}.{oname} definition references '{token}', not on {sname}"),
                    ));
                }
            }
        }
    }
}

/// E07: every derived observable is an observable of some subject.
fn derived_observables(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    let all: BTreeSet<&str> = cfg
        .subjects
        .values()
        .flat_map(|s| s.observables.keys().map(String::as_str))
        .collect();
    for name in cfg.derived_observables.keys() {
        if !all.contains(name.as_str()) {
            f.push(Finding::new(
                Code::E07DerivedObservableOrphan,
                format!("derived observable '{name}' is an observable of no subject"),
            ));
        }
    }
}

/// E15–E17: an observable's unit must agree with its type; durations must not
/// be named in hours or minutes; ratios must not be percentages.
fn observable_units(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    for (sname, s) in &cfg.subjects {
        for (oname, o) in &s.observables {
            let unit = o.unit.as_deref();
            let canonical = match o.ty.as_str() {
                "duration" => Some("s"),
                "mass" => Some("kg"),
                "distance" => Some("m"),
                "speed" => Some("m/s"),
                "angle" => Some("deg"),
                _ => None,
            };
            if let Some(c) = canonical {
                if unit != Some(c) {
                    f.push(Finding::new(
                        Code::E15ObservableUnitInconsistent,
                        format!(
                            "{sname}.{oname} is {} but its unit is {}, not '{c}'",
                            o.ty,
                            unit.map(|u| format!("'{u}'"))
                                .unwrap_or_else(|| "absent".into()),
                        ),
                    ));
                }
                if o.ty == "duration" && (oname.ends_with("_hours") || oname.ends_with("_minutes"))
                {
                    f.push(Finding::new(
                        Code::E16DurationNamedHoursOrMinutes,
                        format!("duration '{sname}.{oname}' is named in hours or minutes"),
                    ));
                }
            } else if o.ty == "ratio"
                && let Some(u) = unit
            {
                if is_percent(u) {
                    f.push(Finding::new(
                        Code::E17RatioPercentageUnit,
                        format!("ratio '{sname}.{oname}' declares a percentage unit '{u}'"),
                    ));
                } else {
                    f.push(Finding::new(
                        Code::E15ObservableUnitInconsistent,
                        format!(
                            "ratio '{sname}.{oname}' declares a unit '{u}'; ratios are unitless"
                        ),
                    ));
                }
            }
        }
    }
}

/// E09–E10: zones carry an engine entity id and a known operational role.
fn zones(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    let roles: BTreeSet<&str> = cfg
        .enumerations
        .get("OperationalRole")
        .map(|v| v.iter().map(String::as_str).collect())
        .unwrap_or_default();
    for z in &cfg.zones {
        if z.entity.is_none() {
            f.push(Finding::new(
                Code::E09ZoneMissingEntity,
                format!("zone '{}' omits its engine entity id", z.key),
            ));
        }
        if !roles.contains(z.operational_role.as_str()) {
            f.push(Finding::new(
                Code::E10ZoneUnknownRole,
                format!(
                    "zone '{}' has unknown operational_role '{}'",
                    z.key, z.operational_role
                ),
            ));
        }
    }
}

/// E11–E12: calendar families are contiguous, and every site names one.
fn calendars(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    for (family, versions) in &cfg.calendars {
        if let Err(detail) = calendar::check_contiguous(versions) {
            f.push(Finding::new(
                Code::E11CalendarOverlapOrGap,
                format!("calendar family '{family}': {detail}"),
            ));
        }
    }
    for site in &cfg.sites {
        if !cfg.calendars.contains_key(&site.calendar_family) {
            f.push(Finding::new(
                Code::E12SiteUnknownCalendarFamily,
                format!(
                    "site '{}' names unknown calendar family '{}'",
                    site.id, site.calendar_family
                ),
            ));
        }
    }
}

/// E13–E14: every intent has a strategy; baseline defaults name real intents.
fn strategies_and_baselines(cfg: &RegistryConfig, intents: &BTreeSet<&str>, f: &mut Vec<Finding>) {
    for intent in &cfg.intents {
        if !cfg.strategies.contains_key(intent) {
            f.push(Finding::new(
                Code::E13StrategyMissingForIntent,
                format!("intent '{intent}' has no strategy"),
            ));
        }
    }
    for intent in cfg.baseline_defaults.keys() {
        if !intents.contains(intent.as_str()) {
            f.push(Finding::new(
                Code::E14BaselineUnknownIntent,
                format!("baseline default names unknown intent '{intent}'"),
            ));
        }
    }
}

/// W01–W06: coverage and reachability warnings. The registry loads regardless.
fn coverage_warnings(cfg: &RegistryConfig, f: &mut Vec<Finding>) {
    // W01: a domain no analyzer covers.
    let covered: BTreeSet<&str> = cfg
        .analyzers
        .iter()
        .flat_map(|a| a.domains.iter().map(String::as_str))
        .collect();
    for d in &cfg.domains {
        if !covered.contains(d.as_str()) {
            f.push(Finding::new(
                Code::W01DomainWithoutAnalyzer,
                format!("domain '{d}' has no analyzer"),
            ));
        }
    }

    // Reachable subjects = any subject in some analyzer's list.
    let reachable: BTreeSet<&str> = cfg
        .analyzers
        .iter()
        .flat_map(|a| a.subjects.iter().map(String::as_str))
        .collect();

    // W02: a subject in no analyzer's list.
    for s in cfg.subjects.keys() {
        if !reachable.contains(s.as_str()) {
            f.push(Finding::new(
                Code::W02UnreachableSubject,
                format!("subject '{s}' is in no analyzer's subject list"),
            ));
        }
    }

    // W03: an observable name carried only by unreachable subjects.
    let mut carriers: BTreeMap<&str, bool> = BTreeMap::new(); // observable -> any reachable carrier
    for (sname, s) in &cfg.subjects {
        let subj_reachable = reachable.contains(sname.as_str());
        for oname in s.observables.keys() {
            let entry = carriers.entry(oname.as_str()).or_insert(false);
            *entry |= subj_reachable;
        }
    }
    for (oname, any_reachable) in &carriers {
        if !any_reachable {
            f.push(Finding::new(
                Code::W03UnreachableObservable,
                format!("observable '{oname}' is on no reachable subject"),
            ));
        }
    }

    // W04: an enumeration referenced by no attribute and no zone field.
    let mut used: BTreeSet<&str> = cfg
        .subjects
        .values()
        .flat_map(|s| s.attributes.values())
        .filter_map(|a| a.of.as_deref())
        .collect();
    // Zones reference OperationalRole (operational_role) and, via
    // restricted_to, EquipmentClass.
    used.insert("OperationalRole");
    if cfg.zones.iter().any(|z| z.restricted_to.is_some()) {
        used.insert("EquipmentClass");
    }
    for e in cfg.enumerations.keys() {
        if !used.contains(e.as_str()) {
            f.push(Finding::new(
                Code::W04UnusedEnumeration,
                format!("enumeration '{e}' is referenced by no attribute or zone field"),
            ));
        }
    }

    // W05: max_analyzers exceeds the number registered.
    if cfg.thresholds.max_analyzers as usize > cfg.analyzers.len() {
        f.push(Finding::new(
            Code::W05MaxAnalyzersExceedsRegistered,
            format!(
                "max_analyzers is {} but {} analyzers are registered",
                cfg.thresholds.max_analyzers,
                cfg.analyzers.len()
            ),
        ));
    }

    // W06: a restricted zone with no restricted_to list.
    for z in &cfg.zones {
        if z.restricted && z.restricted_to.as_ref().is_none_or(Vec::is_empty) {
            f.push(Finding::new(
                Code::W06RestrictedZoneWithoutList,
                format!("restricted zone '{}' has no restricted_to list", z.key),
            ));
        }
    }
}

fn is_percent(u: &str) -> bool {
    u == "%" || u.eq_ignore_ascii_case("percent") || u.eq_ignore_ascii_case("pct")
}

/// Identifier-like tokens in a ratio definition, e.g. `available_time` and
/// `scheduled_time` from `available_time / scheduled_time`.
fn identifiers(def: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in def.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
