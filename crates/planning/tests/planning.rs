//! Stage 4 exit criteria:
//! - "yesterday" resolves to the exact shift-aligned window in PLANNING.md,
//!   not midnight
//! - a window resolved against a historical date picks the calendar version in
//!   force then, not now
//! - an unresolvable expression errors (-> Incomplete), never a guess
//! - a complete InvestigationPlan is assembled deterministically: same input,
//!   byte-identical plan

use samaritan_planning::{PlanInputs, ResolveError, assemble_plan, cull_domains, resolve_time};
use samaritan_registry::Registry;
use samaritan_schema::*;

fn ts(s: &str) -> Timestamp {
    chrono::DateTime::parse_from_rfc3339(s)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

const SITE: &str = "site_01J8X0";

// ---- time resolution ------------------------------------------------------

#[test]
fn yesterday_is_shift_aligned_not_midnight() {
    let reg = Registry::mining().unwrap();
    // asked_at 2026-07-21T09:14Z, Africa/Lagos (+1), v2 op-day starts 06:00.
    let w = resolve_time(&reg, SITE, "yesterday", ts("2026-07-21T09:14:00Z")).unwrap();
    assert_eq!(w.start, ts("2026-07-20T05:00:00Z")); // 06:00 +01:00
    assert_eq!(w.end, ts("2026-07-21T05:00:00Z"));
    assert_eq!(w.resolved_from, "yesterday");
    assert_eq!(w.timezone, "Africa/Lagos");
    assert_eq!(w.calendar.0, "northern_pit@v2");
    // Emphatically not midnight.
    assert_ne!(w.start, ts("2026-07-20T00:00:00Z"));
}

#[test]
fn today_runs_from_op_day_start_to_asked_at() {
    let reg = Registry::mining().unwrap();
    let asked = ts("2026-07-21T09:14:00Z");
    let w = resolve_time(&reg, SITE, "today", asked).unwrap();
    assert_eq!(w.start, ts("2026-07-21T05:00:00Z"));
    assert_eq!(w.end, asked);
}

#[test]
fn historical_date_uses_the_calendar_version_in_force_then() {
    let reg = Registry::mining().unwrap();
    // 2026-02-15 falls under v1, whose op day starts 07:00 (+01:00 -> 06:00Z).
    // The clock today is v2 (06:00); resolution must use v1, not v2.
    let w = resolve_time(&reg, SITE, "2026-02-15", ts("2026-07-21T09:14:00Z")).unwrap();
    assert_eq!(w.start, ts("2026-02-15T06:00:00Z"));
    assert_eq!(w.end, ts("2026-02-16T06:00:00Z"));
    assert_eq!(w.calendar.0, "northern_pit@v1");
}

#[test]
fn unresolvable_expression_errors_never_guesses() {
    let reg = Registry::mining().unwrap();
    let err = resolve_time(
        &reg,
        SITE,
        "sometime around the blast",
        ts("2026-07-21T09:14:00Z"),
    )
    .unwrap_err();
    assert!(matches!(err, ResolveError::Unresolvable(_)));
}

#[test]
fn window_spanning_a_calendar_change_is_rejected() {
    let reg = Registry::mining().unwrap();
    let err = resolve_time(
        &reg,
        SITE,
        "2026-03-30 to 2026-04-02",
        ts("2026-07-21T09:14:00Z"),
    )
    .unwrap_err();
    assert!(matches!(err, ResolveError::SpansCalendarChange(_)));
}

#[test]
fn last_shift_is_the_previous_completed_shift() {
    let reg = Registry::mining().unwrap();
    // At 10:14 local (day shift, 06:00-18:00), the last completed shift is the
    // prior night shift: 2026-07-20 18:00 -> 2026-07-21 06:00 local.
    let w = resolve_time(&reg, SITE, "last shift", ts("2026-07-21T09:14:00Z")).unwrap();
    assert_eq!(w.start, ts("2026-07-20T17:00:00Z")); // 18:00 +01:00
    assert_eq!(w.end, ts("2026-07-21T05:00:00Z")); // 06:00 +01:00
}

#[test]
fn unknown_place_does_not_resolve() {
    let reg = Registry::mining().unwrap();
    let err = samaritan_planning::resolve_scope(&reg, Some("Atlantis")).unwrap_err();
    assert!(matches!(err, ResolveError::UnknownPlace(_)));
}

#[test]
fn known_place_resolves_to_zone_entity() {
    let reg = Registry::mining().unwrap();
    let scope = samaritan_planning::resolve_scope(&reg, Some("Pit 3")).unwrap();
    assert_eq!(scope.kind, ScopeKind::Named);
    assert_eq!(scope.reference.as_ref().unwrap().0, "ent_01J8XZ01");
    assert_eq!(scope.label, "Pit 3");
}

// ---- domain culling -------------------------------------------------------

fn ranked(domain: DomainType, conf: f64) -> RankedDomain {
    RankedDomain {
        domain,
        rank: 0,
        confidence: Confidence::new(conf).unwrap(),
        rationale: "test".into(),
    }
}

#[test]
fn culling_drops_below_floor_and_reranks() {
    let reg = Registry::mining().unwrap();
    let culled = cull_domains(
        &reg,
        vec![
            ranked(DomainType::OperationalPerformance, 0.94),
            ranked(DomainType::Environment, 0.40), // below 0.50 floor -> dropped
            ranked(DomainType::MaterialFlow, 0.81),
        ],
    );
    let doms: Vec<DomainType> = culled.iter().map(|d| d.domain).collect();
    assert_eq!(
        doms,
        vec![DomainType::OperationalPerformance, DomainType::MaterialFlow]
    );
    // Rank reassigned dense, strictly increasing from 1.
    assert_eq!(culled[0].rank, 1);
    assert_eq!(culled[1].rank, 2);
}

// ---- plan assembly --------------------------------------------------------

fn sample_inputs() -> PlanInputs {
    let question = Question {
        id: Id::from("q_01J8XQ7A11"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "Why did efficiency drop yesterday?".into(),
        asked_at: ts("2026-07-21T09:14:00Z"),
        operator: Id::from("op_01J8X0"),
        organization: Id::from("org_01J8X0"),
        site: Id::from(SITE),
        locale: "en".into(),
    };
    PlanInputs {
        question,
        normalized_question: "Why did efficiency decrease yesterday?".into(),
        intent: Intent {
            kind: IntentType::Explain,
            confidence: Confidence::new(0.96).unwrap(),
            rationale: "asks for the cause of a decrease".into(),
        },
        ranked_domains: vec![
            ranked(DomainType::OperationalPerformance, 0.94),
            ranked(DomainType::MaterialFlow, 0.81),
            ranked(DomainType::Equipment, 0.77),
        ],
        time_expr: "yesterday".into(),
        scope_phrase: None,
        entities: vec![],
        world_version: WorldVersion {
            log_position: 1_284_662,
            as_of: ts("2026-07-21T09:14:00Z"),
            snapshot: None,
        },
        priority: Priority::Normal,
        plan_id: Id::from("plan_01J8XQ7K3M"),
        created_at: ts("2026-07-21T09:14:00Z"),
        provenance: PlanProvenance {
            model_id: "claude-opus-4-8".into(),
            prompt_template_version: "planning/2026-07-01".into(),
            registry_version: SchemaVersion::from("1.0.0"),
            cache_hit: false,
        },
    }
}

#[test]
fn assembled_plan_has_expected_shape() {
    let reg = Registry::mining().unwrap();
    let plan = assemble_plan(&reg, sample_inputs()).unwrap();

    assert_eq!(plan.intent.kind, IntentType::Explain);
    assert_eq!(plan.strategy.kind, IntentType::Explain);
    assert_eq!(
        plan.strategy.goal,
        "identify the causes of the observed change"
    );

    // Three domains selected efficiency, flow, maintenance — sorted by name.
    let names: Vec<&str> = plan.analyzers.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, vec!["efficiency", "flow", "maintenance"]);

    // yesterday resolved shift-aligned.
    assert_eq!(plan.constraints.time.start, ts("2026-07-20T05:00:00Z"));
    // Explain carries a default 30-day baseline.
    let baseline = plan.constraints.baseline.as_ref().unwrap();
    assert_eq!(baseline.resolved_from, "default baseline");
    assert_eq!(baseline.end, plan.constraints.time.start);
    // Scope unspecified, recorded explicitly.
    assert_eq!(plan.constraints.spatial_scope.kind, ScopeKind::Unspecified);
}

#[test]
fn assembly_is_deterministic() {
    let reg = Registry::mining().unwrap();
    let a = assemble_plan(&reg, sample_inputs()).unwrap();
    let b = assemble_plan(&reg, sample_inputs()).unwrap();
    // Byte-identical when serialized — the whole determinism claim.
    let ja = serde_json::to_string(&a).unwrap();
    let jb = serde_json::to_string(&b).unwrap();
    assert_eq!(ja, jb);
}
