//! Stage 1 exit criterion: a hand-written example of each schema round-trips
//! through serde unchanged. Exercises the public re-exports, so this also
//! proves the crate's surface is usable from outside.

use samaritan_schema::*;

fn ts(s: &str) -> Timestamp {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
        .unwrap()
        .and_utc()
}

/// Serialize, deserialize, assert identical. The heart of every case.
fn roundtrip<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(value, &back, "round-trip changed the value\njson: {json}");
}

fn sample_window() -> TimeWindow {
    TimeWindow {
        start: ts("2026-07-20T05:00:00Z"),
        end: ts("2026-07-21T05:00:00Z"),
        resolved_from: "yesterday".into(),
        calendar: Id::from("cal_northern_pit_v2"),
        timezone: "Africa/Lagos".into(),
    }
}

fn sample_world() -> WorldVersion {
    WorldVersion {
        log_position: 1_284_662,
        as_of: ts("2026-07-21T09:14:00Z"),
        snapshot: Some(Id::from("snap_01J8XP")),
    }
}

#[test]
fn question_roundtrips() {
    roundtrip(&Question {
        id: Id::from("q_01J8XQ7A11"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        text: "Why did efficiency drop yesterday?".into(),
        asked_at: ts("2026-07-21T09:14:00Z"),
        operator: Id::from("op_01J8X0"),
        organization: Id::from("org_01J8X0"),
        site: Id::from("site_01J8X0"),
        locale: "en".into(),
    });
}

#[test]
fn parsed_question_roundtrips_both_shapes() {
    // Valid: normalized present, reason absent.
    roundtrip(&ParsedQuestion {
        id: Id::from("pq_01"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        question_id: Id::from("q_01J8XQ7A11"),
        status: ValidationStatus::Valid,
        normalized_question: Some("Why did efficiency decrease yesterday?".into()),
        confidence: Confidence::new(0.94).unwrap(),
        language: "en".into(),
        reason: None,
        missing: vec![],
    });

    // Incomplete: reason and missing present, normalized absent.
    roundtrip(&ParsedQuestion {
        id: Id::from("pq_02"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        question_id: Id::from("q_02"),
        status: ValidationStatus::Incomplete,
        normalized_question: None,
        confidence: Confidence::new(0.4).unwrap(),
        language: "en".into(),
        reason: Some("no time range given".into()),
        missing: vec!["what outcome to explain".into(), "time range".into()],
    });
}

#[test]
fn full_plan_roundtrips() {
    let plan = InvestigationPlan {
        id: Id::from("plan_01J8XQ7K3M"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        question_id: Id::from("q_01J8XQ7A11"),
        question_text: "Why did efficiency decrease yesterday?".into(),
        intent: Intent {
            kind: IntentType::Explain,
            confidence: Confidence::new(0.96).unwrap(),
            rationale: "asks for the cause of an observed decrease".into(),
        },
        domains: OperationalDomains {
            domains: vec![
                RankedDomain {
                    domain: DomainType::OperationalPerformance,
                    rank: 1,
                    confidence: Confidence::new(0.94).unwrap(),
                    rationale: "efficiency is the direct subject".into(),
                },
                RankedDomain {
                    domain: DomainType::MaterialFlow,
                    rank: 2,
                    confidence: Confidence::new(0.81).unwrap(),
                    rationale: "losses commonly originate in flow".into(),
                },
            ],
        },
        strategy: InvestigationStrategy {
            kind: IntentType::Explain,
            goal: "identify the causes of the observed change".into(),
            expected_output: "ranked causal hypotheses".into(),
        },
        analyzers: vec![AnalyzerRef {
            name: "efficiency".into(),
            version: SchemaVersion::from("1.0.0"),
            domains: vec![DomainType::OperationalPerformance],
            rationale: "declares coverage of OperationalPerformance".into(),
        }],
        constraints: InvestigationConstraints {
            time: sample_window(),
            baseline: Some(TimeWindow {
                resolved_from: "default baseline".into(),
                ..sample_window()
            }),
            spatial_scope: SpatialScope {
                kind: ScopeKind::Unspecified,
                reference: None,
                label: "entire site".into(),
            },
            entity_scope: vec![],
            world_version: sample_world(),
            priority: Priority::Normal,
            max_runtime: Seconds(30.0),
            operator: Id::from("op_01J8X0"),
            organization: Id::from("org_01J8X0"),
        },
        provenance: PlanProvenance {
            model_id: "claude-opus-4-8".into(),
            prompt_template_version: "planning/2026-07-01".into(),
            registry_version: SchemaVersion::from("1.0.0"),
            cache_hit: false,
        },
    };
    roundtrip(&plan);
}

#[test]
fn full_requirement_roundtrips() {
    let req = InformationRequirement {
        id: Id::from("req_01J8XR0001"),
        plan_id: Id::from("plan_01J8XQ7K3M"),
        requested_by: vec!["efficiency".into(), "maintenance".into()],
        purpose: "establish whether cycle time degraded".into(),
        necessity: Necessity::Required,
        subject: "haul_cycle".into(),
        observables: vec!["cycle_time".into(), "payload_mass".into()],
        filters: vec![Filter {
            field: "equipment_class".into(),
            op: FilterOp::In,
            value: FilterValue::List(vec![
                Scalar::Str("equipment.haul_truck".into()),
                Scalar::Str("equipment.excavator".into()),
            ]),
        }],
        spatial: vec![SpatialPredicate {
            op: SpatialOp::Within,
            region: RegionRef {
                kind: RegionKind::Named,
                reference: Some(Id::from("ent_01J8XZ04")),
                center: None,
                radius: None,
                bounds: None,
            },
        }],
        window: sample_window(),
        baseline: Some(sample_window()),
        scope: SpatialScope {
            kind: ScopeKind::Named,
            reference: Some(Id::from("ent_01J8XZ01")),
            label: "Pit 3".into(),
        },
        entities: vec![EntityRef {
            kind: "equipment".into(),
            reference: None,
            label: "truck 14".into(),
        }],
        granularity: Granularity::Bucketed {
            bucket_size: Seconds(3600.0),
        },
        aggregations: vec![
            Aggregation {
                op: AggregationOp::Mean,
                field: Some("cycle_time".into()),
                bins: None,
            },
            Aggregation {
                op: AggregationOp::Count,
                field: None,
                bins: None,
            },
        ],
        ordering: Some(Ordering {
            by: "cycle_time".into(),
            direction: Direction::Desc,
        }),
        limit: Some(5),
        expected_shape: Shape::Series,
        round: 0,
        depends_on: vec![],
    };
    roundtrip(&req);
}

#[test]
fn requirement_set_roundtrips() {
    let set = RequirementSet {
        id: Id::from("reqset_01"),
        schema_version: SchemaVersion::from("1.0.0"),
        created_at: ts("2026-07-21T09:14:00Z"),
        plan_id: Id::from("plan_01J8XQ7K3M"),
        question_id: Id::from("q_01J8XQ7A11"),
        world_version: sample_world(),
        requirements: vec![],
        unserviceable: vec![Unserviceable {
            requirement_id: Id::from("req_09"),
            requested_by: vec!["safety".into()],
            necessity: Necessity::Required,
            reason: UnserviceableReason::UnavailableSubject,
            detail: "incident_event not registered at this site".into(),
        }],
        execution: ExecutionReport {
            completed: vec![AnalyzerOutcome {
                analyzer: "efficiency".into(),
                version: SchemaVersion::from("1.0.0"),
                duration: Seconds(0.8),
                requirement_count: 5,
                error: None,
            }],
            empty: vec![],
            failed: vec![AnalyzerOutcome {
                analyzer: "safety".into(),
                version: SchemaVersion::from("1.0.0"),
                duration: Seconds(0.1),
                requirement_count: 0,
                error: Some("subject unavailable".into()),
            }],
            timed_out: vec![],
        },
        complete: false,
    };
    roundtrip(&set);
}

#[test]
fn filter_values_serialize_bare() {
    // A scalar filter value is the bare JSON value, not a wrapped variant.
    let one = Filter {
        field: "planned".into(),
        op: FilterOp::Eq,
        value: FilterValue::One(Scalar::Bool(true)),
    };
    let json = serde_json::to_string(&one).unwrap();
    assert!(json.contains(r#""value":true"#), "got {json}");
    roundtrip(&one);
}

#[test]
fn out_of_range_confidence_rejected_on_deserialize() {
    // deny drift: a confidence over 1.0 must fail, not clamp.
    let json = r#"{"type":"Explain","confidence":1.4,"rationale":"x"}"#;
    assert!(serde_json::from_str::<Intent>(json).is_err());
}

#[test]
fn unknown_field_rejected() {
    // deny_unknown_fields guards against silent schema drift.
    let json = r#"{"type":"Explain","confidence":0.9,"rationale":"x","extra":1}"#;
    assert!(serde_json::from_str::<Intent>(json).is_err());
}
