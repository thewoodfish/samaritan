# GRAPH.md

> The mining relation graph, in full.
>
> This is the worked example — what a complete domain model looks like when
> the knowledge is written down instead of coded in.
>
> `REGISTRY.md` holds the specimen. This holds the argument.

---

# What This Is

Everything in this document is **hand-written domain knowledge**. Nothing
here was learned, mined, or inferred. Someone who understands a truck-and-
shovel operation typed it, and someone who understands yours should argue
with it.

Its job is to let a generic investigator behave like a mining engineer
without containing a single line of mining code.

An analyzer asked *"why did production fall?"* walks this graph. The graph
tells it that production is cycles times payload, that cycle time is the sum
of six phases, that fleet availability is an alternative explanation it must
rule out, and that rain three hours before the shift could still be the
cause.

None of that lives in the analyzer.

---

# How To Read An Edge

Every edge carries a **basis** — where the knowledge came from, and therefore
how much you should trust it before checking against your own site.

```
definitional   arithmetic or definition. Cannot be wrong.
               cycle_time is the sum of its phases.

practice       standard mining engineering. True almost everywhere.
               Fleet availability confounds production output.

local          your operation only. Almost certainly wrong as written.
               Rain persists in the ground for six hours.
```

**Read every `local` edge as a question, not a statement.** They are the
lines that need a superintendent's eye. There are 23 of them, marked
throughout.

---

# The Production Spine

The main causal chain. Almost every operational question resolves somewhere
on it.

```
                        mass_moved
                             │
              ┌──────────────┴──────────────┐   × multiplicative
              │                             │
       cycles_completed                mean_payload
              │                             │
      ┌───────┴───────┐              ┌──────┴──────┐
      │               │              │             │
 operating_time   cycle_time    fill_factor   material_density
      │               │              │
      │               │              └── fragmentation ◀── blast_quality
      │               │
      │       ┌───────┴────────────────────────┐   + additive
      │       │                                │
      │   queue · spot · load · haul · dump · return
      │                    │       │
      │                    │       └── haul_distance ÷ mean_speed
      │                    │
      │                    └── bucket_count × swing_time
      │                                │
      │                                └── diggability ◀── fragmentation
      │
      └── availability ◀── downtime ◀── fault_event
```

Two things worth noticing before the detail.

**Fragmentation appears twice.** A poor blast produces coarse rock, which
both slows digging *and* reduces how much fits in a bucket. It attacks
production from two directions at once, which is why blast quality is one of
the highest-leverage variables in an open pit and one of the most commonly
missed in an investigation.

**Speed sits under two phases.** Anything affecting road speed — rain,
dust, congestion, tyre heat limits — hits both `haul_time` and
`return_time`. Its effect on cycle time is roughly doubled.

---

# decomposes

A whole and its parts. **Two modes**, and the distinction matters — a walk
that treats a product as a sum will draw nonsense conclusions.

```yaml
decomposes:

  # ---- multiplicative -------------------------------------------------

  - whole: production_shift.mass_moved
    mode: multiplicative
    parts:
      - production_shift.cycles_completed
      - production_shift.mean_payload
    basis: definitional
    note: >
      the two levers on output. A shift can lose tonnes by running fewer
      cycles or by carrying less per cycle, and the remedies are unrelated.

  - whole: haul_cycle.load_time
    mode: multiplicative
    parts:
      - loading_event.bucket_count
      - loading_event.swing_time
    basis: definitional

  # ---- additive -------------------------------------------------------

  - whole: haul_cycle.cycle_time
    mode: additive
    parts:
      - haul_cycle.queue_time
      - haul_cycle.spot_time
      - haul_cycle.load_time
      - haul_cycle.haul_time
      - haul_cycle.dump_time
      - haul_cycle.return_time
    basis: definitional

  - whole: equipment_availability.scheduled_time
    mode: additive
    parts:
      - equipment_availability.available_time
      - equipment_availability.downtime
    basis: definitional

  - whole: equipment_availability.available_time
    mode: additive
    parts:
      - equipment_availability.operating_time
      - equipment_availability.standby_time
    basis: definitional

  - whole: equipment_availability.downtime
    mode: additive
    parts:
      - downtime_event.planned_duration
      - downtime_event.unplanned_duration
    basis: definitional
    note: >
      the single most important split in maintenance. Planned downtime is a
      scheduling outcome. Unplanned downtime is a failure. Reporting them
      together hides whether maintenance is working.

  - whole: haul_cycle.haul_time
    mode: additive
    parts:
      - route_segment.transit_time      # summed over the route's segments
    basis: definitional
    note: a haul is the sum of its segments, which is what makes
          per-segment investigation possible
```

## Ratios contribute edges implicitly

A ratio declared in `REGISTRY.md` is already a decomposition. A walk
investigating a drop in `utilization` knows to examine both terms without
these being restated.

```
availability      = available_time / scheduled_time
utilization       = operating_time / available_time
achievement_ratio = mass_moved     / mass_planned
fill_factor       = payload_mass   / rated_capacity
```

---

# partitions

Every attribute is a legal split. This declares the **diagnostically
ordered** ones — where to look first.

Ordering is real operational judgement and is almost entirely `local`. It
encodes where problems usually live at a particular mine.

```yaml
partitions:

  - metric: haul_cycle.cycle_time
    by: [equipment_id, destination, material_type, operator_id,
         origin, shift_id]
    basis: local
    note: >
      trucks first, because a single sick truck is the most common cause and
      the cheapest to confirm. Destination second, because a re-route or a
      tip change moves every cycle at once.

  - metric: haul_cycle.queue_time
    by: [location, destination, shift_id, equipment_id]
    basis: local
    note: >
      location first — queues are a property of a place, not a truck.
      A truck appearing repeatedly is a symptom of where it was sent.

  - metric: haul_cycle.load_time
    by: [loader_id, material_type, origin]
    basis: practice
    note: loader first — load time is the loader's metric, not the hauler's

  - metric: production_shift.mass_moved
    by: [material_type, area, crew_id, shift_id]
    basis: local

  - metric: equipment_availability.availability
    by: [equipment_class, equipment_id, subsystem]
    basis: practice

  - metric: downtime_event.duration
    by: [downtime_type, subsystem, equipment_id, planned]
    basis: practice
    note: >
      always split by planned before anything else, or scheduled servicing
      will masquerade as a reliability problem

  - metric: route_segment.transit_time
    by: [segment_id, direction, equipment_class]
    basis: practice
    note: >
      direction matters more than it looks — loaded and empty runs on the
      same grade are different problems

  - metric: queue_event.wait_time
    by: [queue_type, location, shift_id]
    basis: practice

  - metric: fault_event.occurrence_count
    by: [fault_code, subsystem, equipment_class]
    basis: practice

  - metric: zone_visit.dwell_time
    by: [zone_id, entity_class, operator_id]
    basis: practice
```

---

# confounds

An alternative explanation that must be eliminated before a cause is
asserted. **This is the section that stops confident wrong answers.**

```yaml
confounds:

  - factor: equipment_availability.availability
    affects: production_shift.mass_moved
    basis: practice
    why: >
      fewer machines available moves less material regardless of how well
      the running machines performed. Always check this before investigating
      cycle performance — it is the most common false attribution in
      operational reporting.

  - factor: haul_cycle.haul_distance
    affects: haul_cycle.cycle_time
    conditional: comparing across different origins or destinations
    basis: definitional
    why: >
      a longer route legitimately takes longer. This is a plan change, not a
      performance loss, and treating it as one blames operators for a
      decision made in the planning office.

  - factor: haul_cycle.material_type
    affects: haul_cycle.cycle_time
    basis: practice
    why: >
      different material goes to different destinations over different
      distances at different densities. Comparing ore cycles against waste
      cycles compares two operations.

  - factor: downtime_event.planned
    affects: equipment_availability.availability
    basis: definitional
    why: planned maintenance is a scheduling outcome, not a failure

  - factor: ground_condition.state
    affects: route_segment.mean_speed
    basis: practice
    why: wet or soft ground slows traffic irrespective of operator or machine

  - factor: production_shift.crew_id
    affects: production_shift.mass_moved
    basis: practice
    why: >
      crew rotation changes the population being compared. A "decline"
      across a roster change may be two different teams, not one declining
      team.

  - factor: blast_event.fragmentation
    affects: haul_cycle.load_time
    basis: practice
    why: >
      coarse rock digs slowly. Blaming the loader or its operator for a
      blast design problem is a common and demoralising error.

  - factor: production_shift.mass_planned
    affects: production_shift.achievement_ratio
    basis: definitional
    conditional: comparing achievement across periods
    why: >
      achievement can fall because output fell or because the plan rose.
      These are opposite situations with the same number.

  - factor: equipment_availability.standby_time
    affects: equipment_availability.utilization
    basis: practice
    why: >
      a truck standing by for want of a loader is a flow problem, not an
      equipment problem. Utilization blames the truck for it.

  - factor: weather_observation.rainfall
    affects: production_shift.mass_moved
    conditional: any window containing a rain event or the 6h after one
    basis: local
    why: >
      rain suppresses everything at once — speed, visibility, diggability,
      and sometimes the shift itself. It will mask any other cause present
      in the same window.

  - factor: zone.excluded_from_productivity
    affects: entity_track.time_idle
    basis: local
    why: >
      a machine in the workshop is unavailable, not idle. Counting it as
      idle double-penalises a loss already recorded as downtime.
```

---

# influences

Directional causal links. Walked **upstream for root cause**, downstream for
prediction.

`lag` — how long the effect takes to appear.
`persistence` — how long it outlasts its cause. Both in seconds.

```yaml
influences:

  # ---- weather and ground ---------------------------------------------

  - from: weather_observation.rainfall
    to: ground_condition.state
    lag: 0
    persistence: 21600            # 6h
    basis: local
    why: >
      ground stays soft after rain stops. The persistence figure is the
      single most site-specific number in this graph — free-draining
      laterite recovers in under an hour, clay can hold for a full shift.

  - from: ground_condition.state
    to: route_segment.mean_speed
    lag: 0
    basis: practice

  - from: ground_condition.state
    to: loading_event.swing_time
    lag: 0
    basis: local
    why: wet muck sticks in the bucket and slows the dig cycle

  - from: weather_observation.wind_speed
    to: visibility_condition.state
    lag: 0
    persistence: 1800             # 30m
    basis: local
    why: wind lifts dust on dry haul roads

  - from: visibility_condition.state
    to: route_segment.mean_speed
    lag: 0
    basis: practice

  - from: weather_observation.temperature
    to: route_segment.mean_speed
    lag: 10800                    # 3h — heat accumulates in the tyre
    basis: practice
    why: >
      tyre heat limits force speed restrictions on long hauls in high
      ambient temperature. Frequently missed, because the restriction
      appears hours after the heat.

  # ---- blasting --------------------------------------------------------

  - from: blast_event.powder_factor
    to: blast_event.fragmentation
    lag: 0
    basis: practice

  - from: blast_event.fragmentation
    to: loading_event.swing_time
    lag: 0
    persistence: 172800           # 2 days — until the muckpile is cleared
    basis: practice
    why: >
      the effect persists as long as trucks are still loading from that
      muckpile, which is why a blast two days ago can explain today.

  - from: blast_event.fragmentation
    to: haul_cycle.payload_mass
    lag: 0
    persistence: 172800
    basis: practice
    why: coarse rock packs badly and fills the tray before it fills the mass

  - from: blast_event.occurred
    to: route_segment.transit_time
    lag: 0
    persistence: 3600             # 1h — road closure and clearance
    basis: local
    why: blast exclusion closes roads and forces detours

  # ---- flow and congestion ---------------------------------------------

  - from: loading_event.load_time
    to: haul_cycle.queue_time
    lag: 0
    basis: practice
    why: a slow loader backs up every truck waiting behind it

  - from: equipment_availability.availability
    to: haul_cycle.queue_time
    lag: 0
    basis: practice
    conditional: loader availability specifically
    why: one loader down doubles the queue at the remaining one

  - from: crusher_availability.available_time
    to: haul_cycle.dump_time
    lag: 0
    basis: practice
    why: a crusher stoppage backs trucks up at the tip

  - from: route_segment.mean_speed
    to: haul_cycle.haul_time
    lag: 0
    basis: definitional

  - from: route_segment.mean_speed
    to: haul_cycle.return_time
    lag: 0
    basis: definitional

  # ---- reliability -----------------------------------------------------

  - from: fault_event.occurrence_count
    to: downtime_event.unplanned_duration
    lag: 0
    basis: practice

  - from: downtime_event.unplanned_duration
    to: equipment_availability.availability
    lag: 0
    basis: definitional

  - from: maintenance_action.delay_time
    to: downtime_event.duration
    lag: 0
    basis: practice
    why: >
      waiting for a part or a fitter is a large share of downtime and is a
      supply problem, not a reliability problem

  # ---- output ----------------------------------------------------------

  - from: haul_cycle.cycle_time
    to: production_shift.cycles_completed
    lag: 0
    basis: definitional

  - from: equipment_availability.availability
    to: production_shift.mass_moved
    lag: 0
    basis: definitional

  - from: haul_cycle.payload_mass
    to: production_shift.mean_payload
    lag: 0
    basis: definitional
```

## Cycles are permitted

Congestion feeds back on itself.

```
queue_time ──▶ cycle_time ──▶ trucks accumulate ──▶ queue_time
```

Banning cycles would force the graph to lie about a real phenomenon. Walks
are depth-bounded by `max_relation_depth` instead, and must not revisit a
node within a single walk.

---

# rolls_up

Event-level subjects aggregate into period-level ones. Determines the
granularity a question needs.

```yaml
rolls_up:
  - { from: haul_cycle,      to: production_shift }
  - { from: haul_cycle,      to: material_movement }
  - { from: loading_event,   to: production_shift }
  - { from: route_segment,   to: haul_cycle }
  - { from: downtime_event,  to: equipment_availability }
  - { from: fault_event,     to: equipment_availability }
  - { from: zone_visit,      to: entity_track }
  - { from: blast_event,     to: production_shift }
```

A question about a **trend** reads the period level. A question about a
**cause** descends to the event level. This edge is what lets a strategy
choose without being told.

---

# Three Worked Traces

## "Why did production fall yesterday?"

```
mass_moved
  │
  ├─ decomposes ×  ─▶  cycles_completed · mean_payload
  │                    ── payload flat, cycles down 14% ──
  │
  ├─ confounds     ─▶  availability          [flat — ruled out]
  │                    crew_id               [same crew — ruled out]
  │                    rainfall              [none — ruled out]
  │
  └─ cycles_completed
        │
        └─ influences ◀─ cycle_time
                          │
                          └─ decomposes +
                               queue  ▲ 210s   ◀── the growth
                               spot     flat
                               load     flat
                               haul     flat
                               dump     flat
                               return   flat
                                  │
                                  └─ partitions by location
                                       └─ crusher tip: 190s of the 210
                                            │
                                            └─ influences ◀──
                                                 crusher_availability
```

Four hops. Answer: the crusher was down, trucks queued at the tip. Nothing
about the trucks, the operators, or the loaders — and the graph ruled all
three out explicitly rather than never considering them.

## "Why is loading slow at the north pit?"

```
load_time
  │
  ├─ decomposes ×  ─▶  bucket_count · swing_time
  │                    ── bucket_count up 1.3, swing flat ──
  │
  ├─ confounds     ─▶  fragmentation   ◀── the flag
  │
  └─ influences ◀── blast_event.fragmentation
                      persistence 172800s
                      │
                      └─ widens the window back 2 days
                           └── blast on the 18th, powder factor low
```

The extra bucket per load is the tell. More passes for the same tonnes means
coarse rock. The blast was two days ago and would be invisible to any
investigation that only looked at yesterday.

## "Why were trucks slow on the main ramp?"

```
route_segment.mean_speed  (segment: main_ramp)
  │
  ├─ confounds     ─▶  ground_condition   [dry — ruled out]
  │
  └─ influences ◀── temperature
                      lag 10800s
                      │
                      └─ shifts the window back 3h
                           └── 41°C at midday, TKPH limit hit at 15:00
```

The heat that caused the restriction happened three hours before the
slowdown appeared. Without `lag`, the investigation looks at the same window
and finds a normal afternoon temperature.

---

# What This Graph Requires That REGISTRY.md Does Not Yet Declare

Writing this out surfaced gaps. These subjects and observables are
referenced above and are **not** in the vocabulary yet.

```
blast_event            powder_factor · fragmentation · occurred
                       attributes: bench_id, area, pattern_id

crusher_availability   available_time · downtime · throughput
                       attributes: crusher_id

loading_event          + swing_time
haul_cycle             + rated_capacity, fill_factor
downtime_event         + planned_duration, unplanned_duration
                       (currently one `duration` and a `planned` boolean —
                        the split needs to be explicit for the decomposition
                        edge to work)
production_shift       + mean_payload
route_segment          + surface_type
```

`blast_event` is the significant omission. Fragmentation influences four
separate things and persists for two days — it is one of the most
explanatory variables in the whole model, and nothing can currently ask
about it.

---

# What Is Deliberately Not Here

**Edge weights.** Every influence is equally weighted, which is false — rain
matters far more to speed than haze usually does. Weighting requires
calibration against real outcomes and belongs in part 2.

**Thresholds and benchmarks.** "Availability below 0.85 is abnormal for this
fleet" is a threshold, not an edge. It has no home in the relation model and
needs its own section.

**Cost.** Nothing here knows what anything costs, so nothing can rank
findings by money. That is the first thing an operations manager will ask
for, and it is a deliberate omission rather than an oversight.

**Conditional logic beyond a text note.** `conditional` is free text that a
walk cannot evaluate — it is guidance for the model, not machinery. If
conditional confounders become common, they need a real predicate language.

---

# Extending It

Adding knowledge is adding a line, not writing code.

```
new haul route         → route_segment entry, and a partition if it matters
new equipment class    → an EquipmentClass enum member
new failure mode       → a fault → downtime influence edge
a cause you keep
finding by hand        → the edge that would have found it for you
```

That last one is the maintenance loop that matters. **Every investigation
that a human solved and Samaritan did not is a missing edge.** The graph
should grow from post-mortems, and a missing edge is the only kind of gap
that leaves no trace — the system will not report that it failed to consider
something it was never told about.

---

# Guiding Principle

> The graph is where the mine's knowledge lives.

> An analyzer that walks it looks like an expert.

> An analyzer without it is a query builder.
