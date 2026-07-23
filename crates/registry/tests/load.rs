//! Stage 2 exit criteria:
//! - the mining registry loads clean (no errors)
//! - every implemented code has a fixture that triggers exactly it
//! - a window spanning a calendar boundary is rejected
//! - the reverse index matches the one written by hand in REGISTRY.md

use chrono::NaiveDate;
use samaritan_registry::config::RegistryConfig;
use samaritan_registry::{Code, Registry, calendar};

fn base_config() -> RegistryConfig {
    serde_yaml::from_str(samaritan_registry::MINING_YAML).expect("mining.yaml parses")
}

/// Validate a config and collect just the error codes it raises.
fn error_codes(cfg: &RegistryConfig) -> Vec<Code> {
    match Registry::from_config(cfg.clone()) {
        Ok(_) => vec![],
        Err(e) => e.errors().iter().map(|f| f.code).collect(),
    }
}

/// Assert a mutated config raises exactly one error, the expected one.
fn assert_exactly(cfg: &RegistryConfig, code: Code) {
    let codes = error_codes(cfg);
    assert_eq!(codes, vec![code], "expected exactly {code}, got {codes:?}");
}

// ---- clean load -----------------------------------------------------------

#[test]
fn mining_registry_loads_clean() {
    let reg = Registry::mining().expect("mining registry must load");
    // Warnings are expected and allowed: three uncovered domains + W05.
    let warn_codes: Vec<Code> = reg.warnings().iter().map(|f| f.code).collect();
    assert!(warn_codes.contains(&Code::W01DomainWithoutAnalyzer));
    assert!(warn_codes.contains(&Code::W05MaxAnalyzersExceedsRegistered));
    // Exactly the four we expect: W01 x3 (Infrastructure, Personnel, Security)
    // plus W05 x1.
    assert_eq!(
        warn_codes
            .iter()
            .filter(|c| **c == Code::W01DomainWithoutAnalyzer)
            .count(),
        3
    );
    assert_eq!(reg.warnings().len(), 4, "warnings: {:?}", reg.warnings());
}

// ---- reverse index --------------------------------------------------------

#[test]
fn reverse_index_matches_spec() {
    let reg = Registry::mining().unwrap();
    let expect = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        reg.analyzers_for("OperationalPerformance"),
        expect(&["efficiency"])
    );
    assert_eq!(reg.analyzers_for("Production"), expect(&["efficiency"]));
    assert_eq!(reg.analyzers_for("MaterialFlow"), expect(&["flow"]));
    assert_eq!(reg.analyzers_for("Logistics"), expect(&["flow"]));
    assert_eq!(reg.analyzers_for("Equipment"), expect(&["maintenance"]));
    assert_eq!(reg.analyzers_for("Environment"), expect(&["environment"]));
    assert_eq!(reg.analyzers_for("Safety"), expect(&["safety"]));
    assert!(reg.analyzers_for("Infrastructure").is_empty());
    assert!(reg.analyzers_for("Security").is_empty());
}

// ---- calendar selection ---------------------------------------------------

#[test]
fn window_within_one_calendar_version_resolves() {
    let cfg = base_config();
    let cal = &cfg.calendars["northern_pit"];
    let v = calendar::covering_range(
        cal,
        NaiveDate::from_ymd_opt(2026, 7, 20).unwrap(),
        NaiveDate::from_ymd_opt(2026, 7, 21).unwrap(),
    )
    .expect("a July 2026 window is entirely within v2");
    assert_eq!(v.version, "v2");
}

#[test]
fn window_spanning_a_calendar_change_is_rejected() {
    let cfg = base_config();
    let cal = &cfg.calendars["northern_pit"];
    // March 2026 is v1, April 2026 is v2 — a window across the boundary fails.
    let err = calendar::covering_range(
        cal,
        NaiveDate::from_ymd_opt(2026, 3, 30).unwrap(),
        NaiveDate::from_ymd_opt(2026, 4, 2).unwrap(),
    )
    .expect_err("a window spanning the v1/v2 boundary must be rejected");
    match err {
        calendar::CalendarError::SpansChange {
            start_version,
            end_version,
            ..
        } => {
            assert_eq!(start_version, "v1");
            assert_eq!(end_version, "v2");
        }
        other => panic!("expected SpansChange, got {other:?}"),
    }
}

// ---- error fixtures: one per implemented code -----------------------------

#[test]
fn e01_unknown_subject() {
    let mut c = base_config();
    c.analyzers[0].subjects.push("not_a_subject".into());
    assert_exactly(&c, Code::E01AnalyzerUnknownSubject);
}

#[test]
fn e02_unknown_domain() {
    let mut c = base_config();
    c.analyzers[0].domains.push("NotADomain".into());
    assert_exactly(&c, Code::E02AnalyzerUnknownDomain);
}

#[test]
fn e03_unknown_intent() {
    let mut c = base_config();
    c.analyzers[0].intents.push("Investigate".into());
    assert_exactly(&c, Code::E03AnalyzerUnknownIntent);
}

#[test]
fn e04_duplicate_analyzer_name() {
    let mut c = base_config();
    let dup = c.analyzers[0].clone();
    c.analyzers.push(dup);
    assert_exactly(&c, Code::E04DuplicateAnalyzerName);
}

#[test]
fn e05_unknown_enumeration() {
    let mut c = base_config();
    let attr = c
        .subjects
        .get_mut("haul_cycle")
        .unwrap()
        .attributes
        .get_mut("material_type")
        .unwrap();
    attr.of = Some("NoSuchEnum".into());
    assert_exactly(&c, Code::E05AttributeUnknownEnumeration);
}

#[test]
fn e06_ratio_definition_offsubject() {
    let mut c = base_config();
    let o = c
        .subjects
        .get_mut("haul_cycle")
        .unwrap()
        .observables
        .get_mut("fill_factor")
        .unwrap();
    o.definition = Some("payload_mass / operating_time".into()); // operating_time is elsewhere
    assert_exactly(&c, Code::E06RatioDefinitionUnknownObservable);
}

#[test]
fn e07_orphan_derived_observable() {
    let mut c = base_config();
    c.derived_observables.insert(
        "phantom_measure".into(),
        serde_yaml::from_str("{definition: x, unit: s}").unwrap(),
    );
    assert_exactly(&c, Code::E07DerivedObservableOrphan);
}

#[test]
fn e09_zone_missing_entity() {
    let mut c = base_config();
    c.zones[0].entity = None;
    assert_exactly(&c, Code::E09ZoneMissingEntity);
}

#[test]
fn e10_zone_unknown_role() {
    let mut c = base_config();
    c.zones[0].operational_role = "teleport_pad".into();
    assert_exactly(&c, Code::E10ZoneUnknownRole);
}

#[test]
fn e11_calendar_gap() {
    let mut c = base_config();
    // Push v1's end back a month, opening a gap before v2.
    c.calendars.get_mut("northern_pit").unwrap()[0].effective_until = Some("2026-02-28".into());
    assert_exactly(&c, Code::E11CalendarOverlapOrGap);
}

#[test]
fn e12_site_unknown_calendar_family() {
    let mut c = base_config();
    c.sites[0].calendar_family = "southern_pit".into();
    assert_exactly(&c, Code::E12SiteUnknownCalendarFamily);
}

#[test]
fn e13_strategy_missing_for_intent() {
    let mut c = base_config();
    c.strategies.remove("Predict");
    assert_exactly(&c, Code::E13StrategyMissingForIntent);
}

#[test]
fn e14_baseline_unknown_intent() {
    let mut c = base_config();
    c.baseline_defaults.insert("Ponder".into(), "none".into());
    assert_exactly(&c, Code::E14BaselineUnknownIntent);
}

#[test]
fn e15_unit_inconsistent_with_type() {
    let mut c = base_config();
    c.subjects
        .get_mut("haul_cycle")
        .unwrap()
        .observables
        .get_mut("cycle_time")
        .unwrap()
        .unit = Some("h".into()); // duration must be seconds
    assert_exactly(&c, Code::E15ObservableUnitInconsistent);
}

#[test]
fn e16_duration_named_hours() {
    let mut c = base_config();
    // dump_time is referenced by no ratio definition, so renaming it isolates
    // E16 without tripping E06.
    let obs = &mut c.subjects.get_mut("haul_cycle").unwrap().observables;
    let spec = obs.remove("dump_time").unwrap();
    obs.insert("dump_minutes".into(), spec); // seconds, but named minutes
    assert_exactly(&c, Code::E16DurationNamedHoursOrMinutes);
}

#[test]
fn e17_ratio_percentage_unit() {
    let mut c = base_config();
    c.subjects
        .get_mut("equipment_availability")
        .unwrap()
        .observables
        .get_mut("availability")
        .unwrap()
        .unit = Some("%".into());
    assert_exactly(&c, Code::E17RatioPercentageUnit);
}

// ---- warning fixtures -----------------------------------------------------

/// The mutated config's warning codes (errors, if any, would fail the load).
fn warning_codes(cfg: &RegistryConfig) -> Vec<Code> {
    Registry::from_config(cfg.clone())
        .expect("fixture must still load — warnings do not block")
        .warnings()
        .iter()
        .map(|f| f.code)
        .collect()
}

#[test]
fn w01_domain_without_analyzer() {
    // Present in the base registry: Infrastructure, Personnel, Security.
    assert!(warning_codes(&base_config()).contains(&Code::W01DomainWithoutAnalyzer));
}

#[test]
fn w02_unreachable_subject() {
    let mut c = base_config();
    c.subjects.insert(
        "orphan_subject".into(),
        serde_yaml::from_str("{observables: {something: {type: count}}}").unwrap(),
    );
    assert!(warning_codes(&c).contains(&Code::W02UnreachableSubject));
}

#[test]
fn w03_unreachable_observable() {
    let mut c = base_config();
    // A subject no analyzer lists, carrying an observable found nowhere else.
    c.subjects.insert(
        "orphan_subject".into(),
        serde_yaml::from_str("{observables: {lonely_metric: {type: count}}}").unwrap(),
    );
    let codes = warning_codes(&c);
    assert!(codes.contains(&Code::W03UnreachableObservable));
}

#[test]
fn w04_unused_enumeration() {
    let mut c = base_config();
    c.enumerations
        .insert("UnusedEnum".into(), vec!["x".into(), "y".into()]);
    assert!(warning_codes(&c).contains(&Code::W04UnusedEnumeration));
}

#[test]
fn w05_max_analyzers_exceeds_registered() {
    // Base registry: max_analyzers 8, five analyzers.
    assert!(warning_codes(&base_config()).contains(&Code::W05MaxAnalyzersExceedsRegistered));
}

#[test]
fn w06_restricted_zone_without_list() {
    let mut c = base_config();
    c.zones[3].restricted_to = None; // blast_area_c is restricted
    assert!(warning_codes(&c).contains(&Code::W06RestrictedZoneWithoutList));
}
