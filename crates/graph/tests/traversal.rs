//! Stage 3 exit criteria for the graph crate:
//! - a multiplicative decomposition is distinguishable from an additive one in
//!   the type, not a string
//! - a depth-bounded walk from cycle_time terminates and visits the expected
//!   nodes
//! - walking rainfall -> ground -> speed backward widens the window by exactly
//!   the persistence sum

use samaritan_graph::{DecompMode, NodeRef, RelationGraph, RelationsConfig};

/// A slice of the mining relations, enough to exercise every primitive.
const RELATIONS: &str = r#"
decomposes:
  - whole: production_shift.mass_moved
    mode: multiplicative
    parts: [production_shift.cycles_completed, production_shift.mean_payload]
  - whole: haul_cycle.cycle_time
    mode: additive
    parts: [haul_cycle.queue_time, haul_cycle.spot_time, haul_cycle.load_time, haul_cycle.haul_time, haul_cycle.dump_time, haul_cycle.return_time]
  - whole: haul_cycle.load_time
    mode: multiplicative
    parts: [loading_event.bucket_count, loading_event.swing_time]
partitions:
  - metric: haul_cycle.cycle_time
    by: [equipment_id, destination, shift_id]
confounds:
  - factor: equipment_availability.availability
    affects: production_shift.mass_moved
influences:
  - { from: weather_observation.rainfall, to: ground_condition.state, lag: 0, persistence: 21600 }
  - { from: ground_condition.state, to: route_segment.mean_speed, lag: 0 }
  - { from: route_segment.mean_speed, to: haul_cycle.haul_time, lag: 300, persistence: 0 }
rolls_up:
  - { from: haul_cycle, to: production_shift }
"#;

fn graph() -> RelationGraph {
    let cfg: RelationsConfig = serde_yaml::from_str(RELATIONS).unwrap();
    RelationGraph::from_config(&cfg).expect("relations build")
}

fn n(s: &str) -> NodeRef {
    NodeRef::parse(s).unwrap()
}

#[test]
fn decomposition_mode_is_typed() {
    let g = graph();
    let mass = g.decomposition(&n("production_shift.mass_moved")).unwrap();
    assert_eq!(mass.mode, DecompMode::Multiplicative);
    let cycle = g.decomposition(&n("haul_cycle.cycle_time")).unwrap();
    assert_eq!(cycle.mode, DecompMode::Additive);
}

#[test]
fn depth_one_decomposition_walk_from_cycle_time() {
    let g = graph();
    // Depth 1: only the immediate six phases, not their sub-parts.
    let reached = g.decomposition_walk(&n("haul_cycle.cycle_time"), 1);
    let got: Vec<String> = reached.iter().map(NodeRef::qualified).collect();
    assert_eq!(
        got,
        vec![
            "haul_cycle.dump_time",
            "haul_cycle.haul_time",
            "haul_cycle.load_time",
            "haul_cycle.queue_time",
            "haul_cycle.return_time",
            "haul_cycle.spot_time",
        ]
    );
}

#[test]
fn deeper_walk_expands_load_time() {
    let g = graph();
    // Depth 2: load_time further decomposes into bucket_count x swing_time.
    let reached = g.decomposition_walk(&n("haul_cycle.cycle_time"), 2);
    let got: Vec<String> = reached.iter().map(NodeRef::qualified).collect();
    assert!(got.contains(&"loading_event.bucket_count".to_string()));
    assert!(got.contains(&"loading_event.swing_time".to_string()));
    // The six phases are still present.
    assert!(got.contains(&"haul_cycle.queue_time".to_string()));
}

#[test]
fn depth_bound_stops_expansion() {
    let g = graph();
    // At depth 1, load_time's sub-parts must NOT appear.
    let reached = g.decomposition_walk(&n("haul_cycle.cycle_time"), 1);
    let got: Vec<String> = reached.iter().map(NodeRef::qualified).collect();
    assert!(!got.contains(&"loading_event.bucket_count".to_string()));
}

#[test]
fn backward_influence_walk_widens_by_persistence_sum() {
    let g = graph();
    // From haul_time, walk back: mean_speed (lag 300) <- ground (0) <- rainfall
    // (persistence 21600). Total widening to reach rainfall = 300 + 0 + 21600.
    let reached = g.backward_influence_walk(&n("haul_cycle.haul_time"), 5);
    let by_node = |q: &str| {
        reached
            .iter()
            .find(|r| r.node.qualified() == q)
            .unwrap_or_else(|| panic!("{q} not reached"))
    };
    assert_eq!(by_node("route_segment.mean_speed").window_widening, 300);
    assert_eq!(by_node("ground_condition.state").window_widening, 300);
    assert_eq!(
        by_node("weather_observation.rainfall").window_widening,
        21_900
    );
}

#[test]
fn backward_walk_respects_depth_bound() {
    let g = graph();
    // Depth 1 from haul_time reaches only mean_speed, not further back.
    let reached = g.backward_influence_walk(&n("haul_cycle.haul_time"), 1);
    let nodes: Vec<String> = reached.iter().map(|r| r.node.qualified()).collect();
    assert_eq!(nodes, vec!["route_segment.mean_speed".to_string()]);
}

#[test]
fn missing_mode_is_a_build_error() {
    let cfg: RelationsConfig =
        serde_yaml::from_str("decomposes:\n  - whole: a.b\n    parts: [a.c]\n").unwrap();
    assert!(matches!(
        RelationGraph::from_config(&cfg),
        Err(samaritan_graph::BuildError::MissingMode(_))
    ));
}

#[test]
fn confounders_and_partitions_resolve() {
    let g = graph();
    let confs = g.confounders_of(&n("production_shift.mass_moved"));
    assert_eq!(confs.len(), 1);
    assert_eq!(
        confs[0].factor.qualified(),
        "equipment_availability.availability"
    );

    let keys = g.partition_keys(&n("haul_cycle.cycle_time"));
    assert_eq!(keys, ["equipment_id", "destination", "shift_id"]);
}
