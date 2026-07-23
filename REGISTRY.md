# REGISTRY.md

> The registry is Samaritan's configuration.
>
> It holds every closed vocabulary and every mapping.
>
> Adding a domain, an analyzer, an intent, or a subject is a registry change.
>
> It should never be a code change.

---

# Why This Exists

Mappings described as "configurable" but never written down are invisible,
and an invisible mapping cannot be vetted.

The registry is versioned as one unit. `registry_version` appears in every
`PlanProvenance` and participates in every cache key. Changing the registry
invalidates cached plans, which is the correct behaviour.

---

# Contents

```
Units and types         conventions every vocabulary entry obeys
Intents
Strategies              intent → strategy
Domains
Analyzers               declares domains, intents, subjects
Domain → Analyzer       derived reverse index
Subject vocabulary      subjects, observables, attributes
Enumerations            legal values for enum attributes
Relations               how observables connect — the investigable model
Derived observables     definitions of computed facts
Zones                   operational meaning of engine zone entities
Sites and calendars     timezone + shift calendar, time-versioned
Baseline defaults       intent → default reference period
Window requirements     which intents may omit a time window
Thresholds
Model configuration
Registry validation     every check the loader must perform
Versioning
```

---

# Units And Types

Declared once, obeyed everywhere. Every observable states both.

## Canonical units

```
duration      s      seconds, always — never hours or minutes
mass          kg     kilograms, always — never tonnes
distance      m      metres
speed         m/s
angle         deg    degrees, 0–360, clockwise from true north
ratio         —      0.0 to 1.0, never a percentage
rainfall      mm     conventional exception, declared explicitly
temperature   °C
```

**Ratios are never percentages.** `availability: 0.92`, not `92`. This is the
single most common unit bug in operational reporting and it is closed here by
fiat.

**Durations are always seconds**, including at shift scale. An observable is
named `available_time`, not `available_hours`, so the name cannot contradict
the unit. Presentation converts; the vocabulary does not.

## Types

```
duration · mass · distance · speed · ratio · count · float
boolean · string · enum · id · timestamp
```

`enum` types name their enumeration. Legal values are listed under
Enumerations, and nowhere else.

---

# Intents

Closed set. Six.

```
Explain · Compare · Locate · Recommend · Predict · Summarize
```

---

# Strategies

1:1 with intent. A lookup, never an inference.

```yaml
Explain:
  goal: identify the causes of the observed change
  expected_output: ranked causal hypotheses

Compare:
  goal: identify meaningful differences between the subjects
  expected_output: ranked differences with magnitude

Locate:
  goal: identify which entity or place satisfies the description
  expected_output: ranked candidate entities

Recommend:
  goal: identify actionable improvement opportunities
  expected_output: ranked recommendations with expected impact

Predict:
  goal: estimate a future outcome from observed trend
  expected_output: forecast with an uncertainty range

Summarize:
  goal: produce a concise operational overview
  expected_output: state summary across the requested scope
```

---

# Domains

Closed set. Ten.

```
OperationalPerformance
Production
Equipment
MaterialFlow
Infrastructure
Personnel
Safety
Security
Environment
Logistics
```

---

# Analyzers

Analyzers declare their own coverage. Planning derives the reverse index.

An analyzer knows what it can investigate. Planning should not hold a
hand-maintained list that drifts.

```yaml
analyzers:

  - name: efficiency
    version: 1.0.0
    domains: [OperationalPerformance, Production]
    intents: [Explain, Compare, Summarize]
    subjects: [haul_cycle, loading_event, production_shift,
               equipment_availability, blast_event, entity_track]

  - name: flow
    version: 1.0.0
    domains: [MaterialFlow, Logistics]
    intents: [Explain, Compare, Locate, Summarize]
    subjects: [haul_cycle, queue_event, route_segment, material_movement,
               crusher_availability, entity_track, zone_visit]

  - name: maintenance
    version: 1.0.0
    domains: [Equipment]
    intents: [Explain, Predict, Summarize]
    subjects: [equipment_availability, downtime_event, fault_event,
               maintenance_action]

  - name: environment
    version: 1.0.0
    domains: [Environment]
    intents: [Explain, Compare, Summarize]
    subjects: [weather_observation, ground_condition, visibility_condition]

  - name: safety
    version: 1.0.0
    domains: [Safety]
    intents: [Explain, Locate, Summarize]
    subjects: [incident_event, zone_visit, entity_track]
```

## Derived index

```
OperationalPerformance  →  efficiency
Production              →  efficiency
MaterialFlow            →  flow
Logistics               →  flow
Equipment               →  maintenance
Environment             →  environment
Safety                  →  safety

Infrastructure          →  (none)
Personnel               →  (none)
Security                →  (none)
```

Three domains have no analyzer. This is legal and must warn at registry
load — a ranked domain that selects nothing is a coverage gap, and silence
would hide it.

---

# Subject Vocabulary

**This is the most consequential section in the registry.**

A subject is a kind of thing or occurrence that can be asked about.

## The engine does not know these words

The Reality Engine is domain agnostic. It stores entities, components,
events, relationships and space. It has never heard of a haul cycle.

Every subject below is a **domain type** — registered with the engine by the
mining domain layer, stored and indexed as an opaque named type, and given
meaning only here.

```
engine knows          entity · component · event · zone · relationship
domain layer declares mining:haul_cycle · mining:downtime_event
this registry says    what those mean and what may be asked of them
```

This list is not the engine's data model. It is **the domain layer's
contract**, and the engine remains reusable for wildlife, security, or
anything else.

Anything not on this list cannot be requested.

## Naming

Subjects and classifications are hierarchical, dot-separated, matching the
engine's classification convention.

```
equipment.haul_truck   matches queries for  equipment
equipment.excavator    matches queries for  equipment
```

A query for a parent matches all descendants. This is what lets an analyzer
ask about `equipment` without enumerating every class of machine.

Registered names are namespaced (`mining:`) when they reach the engine. This
registry omits the prefix for readability; the domain layer adds it.

## Subjects

```yaml
subjects:

  haul_cycle:
    description: >
      one complete spot-load-haul-dump-return loop by a hauling unit
    observables:
      cycle_time:       { type: duration, unit: s }
      queue_time:       { type: duration, unit: s }
      spot_time:        { type: duration, unit: s }
      load_time:        { type: duration, unit: s }
      haul_time:        { type: duration, unit: s }
      dump_time:        { type: duration, unit: s }
      return_time:      { type: duration, unit: s }
      payload_mass:     { type: mass,     unit: kg }
      rated_capacity:   { type: mass,     unit: kg }
      fill_factor:      { type: ratio, definition: payload_mass / rated_capacity }
      haul_distance:    { type: distance, unit: m }
    attributes:
      equipment_id:     { type: id }
      equipment_class:  { type: enum, of: EquipmentClass }
      operator_id:      { type: id }
      origin:           { type: id }
      destination:      { type: id }
      material_type:    { type: enum, of: MaterialType }
      shift_id:         { type: id }

  loading_event:
    description: one loading interaction between a loading unit and a hauler
    observables:
      load_time:        { type: duration, unit: s }
      spot_time:        { type: duration, unit: s }
      swing_time:       { type: duration, unit: s }
      bucket_count:     { type: count }
      payload_mass:     { type: mass,     unit: kg }
    attributes:
      loader_id:        { type: id }
      hauler_id:        { type: id }
      material_type:    { type: enum, of: MaterialType }
      location:         { type: id }
      shift_id:         { type: id }

  queue_event:
    description: a period an entity spent waiting to be served
    observables:
      wait_time:        { type: duration, unit: s }
      queue_length:     { type: count }
      position_in_queue:{ type: count }
    attributes:
      equipment_id:     { type: id }
      equipment_class:  { type: enum, of: EquipmentClass }
      location:         { type: id }
      queue_type:       { type: enum, of: QueueType }
      shift_id:         { type: id }

  equipment_availability:
    description: >
      availability and utilization of one machine over one period.
      Availability is mechanical readiness. Utilization is the share of
      available time actually spent producing. The gap between them is
      where operational loss hides.
    observables:
      scheduled_time:   { type: duration, unit: s }
      available_time:   { type: duration, unit: s }
      operating_time:   { type: duration, unit: s }
      downtime:         { type: duration, unit: s }
      standby_time:     { type: duration, unit: s }
      availability:     { type: ratio, definition: available_time / scheduled_time }
      utilization:      { type: ratio, definition: operating_time / available_time }
    attributes:
      equipment_id:     { type: id }
      equipment_class:  { type: enum, of: EquipmentClass }
      shift_id:         { type: id }

  downtime_event:
    description: a period a machine was not available for production
    observables:
      duration:            { type: duration, unit: s }
      planned_duration:    { type: duration, unit: s }
      unplanned_duration:  { type: duration, unit: s }
      response_time:       { type: duration, unit: s }
      repair_time:         { type: duration, unit: s }
    attributes:
      equipment_id:     { type: id }
      equipment_class:  { type: enum, of: EquipmentClass }
      downtime_type:    { type: enum, of: DowntimeType }
      planned:          { type: boolean }
      fault_code:       { type: string }
      shift_id:         { type: id }

  fault_event:
    description: a fault or alarm reported by a machine's onboard systems
    observables:
      time_to_acknowledge: { type: duration, unit: s }
      occurrence_count:    { type: count }
    attributes:
      equipment_id:     { type: id }
      equipment_class:  { type: enum, of: EquipmentClass }
      fault_code:       { type: string }
      subsystem:        { type: enum, of: Subsystem }
      severity:         { type: enum, of: FaultSeverity }
      shift_id:         { type: id }

  maintenance_action:
    description: a maintenance task performed on a machine
    observables:
      labour_time:      { type: duration, unit: s }
      delay_time:       { type: duration, unit: s }
      parts_count:      { type: count }
    attributes:
      equipment_id:     { type: id }
      maintenance_type: { type: enum, of: MaintenanceType }
      subsystem:        { type: enum, of: Subsystem }
      work_order_id:    { type: id }
      shift_id:         { type: id }

  production_shift:
    description: aggregate production outcome for one shift
    observables:
      mass_moved:       { type: mass, unit: kg }
      mass_planned:     { type: mass, unit: kg }
      mean_payload:     { type: mass, unit: kg }
      cycles_completed: { type: count }
      achievement_ratio:{ type: ratio, definition: mass_moved / mass_planned }
    attributes:
      shift_id:         { type: id }
      crew_id:          { type: id }
      material_type:    { type: enum, of: MaterialType }
      area:             { type: id }

  material_movement:
    description: mass moved from one location to another over a period
    observables:
      mass_moved:       { type: mass, unit: kg }
      load_count:       { type: count }
      mean_payload:     { type: mass, unit: kg }
    attributes:
      origin:           { type: id }
      destination:      { type: id }
      material_type:    { type: enum, of: MaterialType }
      shift_id:         { type: id }

  route_segment:
    description: >
      traversal of one defined stretch of haul road by one entity
    observables:
      transit_time:     { type: duration, unit: s }
      mean_speed:       { type: speed,    unit: m/s }
      segment_length:   { type: distance, unit: m }
      gradient:         { type: ratio }
    attributes:
      segment_id:       { type: id }
      equipment_id:     { type: id }
      equipment_class:  { type: enum, of: EquipmentClass }
      direction:        { type: enum, of: HaulDirection }
      surface_type:     { type: enum, of: SurfaceType }
      shift_id:         { type: id }

  blast_event:
    description: >
      one blast fired against one pattern. The muckpile it produces drives
      loading and payload for as long as trucks work from it — days, not
      minutes — which is why it is a subject, not a passing event.
    observables:
      powder_factor:    { type: float,    unit: "kg/m3" }
      fragmentation:    { type: distance, unit: m }        # mean fragment size
      volume:           { type: float,    unit: "m3" }
    attributes:
      bench_id:         { type: id }
      area:             { type: id }
      pattern_id:       { type: id }
      material_type:    { type: enum, of: MaterialType }
      shift_id:         { type: id }

  crusher_availability:
    description: availability and throughput of a crusher over a period
    observables:
      scheduled_time:   { type: duration, unit: s }
      available_time:   { type: duration, unit: s }
      downtime:         { type: duration, unit: s }
      throughput:       { type: mass,     unit: kg }
      availability:     { type: ratio, definition: available_time / scheduled_time }
    attributes:
      crusher_id:       { type: id }
      shift_id:         { type: id }

  entity_track:
    description: >
      the movement of one object through space over time — the subject
      spatial predicates and path-derived observables operate on
    spatial: true
    observables:
      speed:              { type: speed,    unit: m/s }
      heading:            { type: angle,    unit: deg }
      distance_travelled: { type: distance, unit: m }
      time_idle:          { type: duration, unit: s }
    attributes:
      entity_id:        { type: id }
      entity_class:     { type: enum, of: EquipmentClass }
      operator_id:      { type: id }
      shift_id:         { type: id }

  zone_visit:
    description: >
      one continuous presence of an entity inside one zone, from entry
      to exit
    spatial: true
    observables:
      dwell_time:       { type: duration,  unit: s }
      entry_time:       { type: timestamp }
      exit_time:        { type: timestamp }
    attributes:
      entity_id:        { type: id }
      entity_class:     { type: enum, of: EquipmentClass }
      zone_id:          { type: id }
      operator_id:      { type: id }
      shift_id:         { type: id }

  incident_event:
    description: >
      an operational incident derived by the domain layer from world facts
      — the engine records the fact, the domain layer records the meaning
    observables:
      time_to_resolve:  { type: duration, unit: s }
      duration:         { type: duration, unit: s }
    attributes:
      incident_type:    { type: enum, of: IncidentType }
      severity:         { type: enum, of: IncidentSeverity }
      entity_id:        { type: id }
      zone_id:          { type: id }
      shift_id:         { type: id }

  weather_observation:
    description: environmental conditions at a point in time
    observables:
      rainfall:         { type: float, unit: mm }
      temperature:      { type: float, unit: "°C" }
      wind_speed:       { type: speed, unit: m/s }
    attributes:
      station_id:       { type: id }
      area:             { type: id }

  ground_condition:
    description: assessed trafficability of a surface over a period
    observables:
      assessed_at:      { type: timestamp }
    attributes:
      state:            { type: enum, of: GroundState }
      area:             { type: id }
      segment_id:       { type: id }

  visibility_condition:
    description: assessed visibility over a period
    observables:
      visibility_range: { type: distance, unit: m }
      assessed_at:      { type: timestamp }
    attributes:
      state:            { type: enum, of: VisibilityState }
      area:             { type: id }
```

`spatial: true` marks a subject carrying position over time. Spatial
predicates may only be applied to such subjects.

`definition:` on a ratio states how it is computed from sibling observables,
so two analyzers can never disagree about what `utilization` means.

## Rules

Every `observables` entry in a requirement must belong to the requested
subject.

Every filter `field` must be an observable or attribute of that subject.

Every `aggregations[].field` and `ordering.by` must be a numeric observable
of that subject.

Cross-subject joins are not expressible. If an analyzer needs two subjects,
it issues two requirements. Whether the engine should support joins is an
open question deliberately left closed for now.

---

# Enumerations

Legal values live here and nowhere else. An attribute typed `enum` names one
of these. A filter comparing against a value outside its enumeration is
invalid.

```yaml
EquipmentClass:
  - equipment.haul_truck
  - equipment.excavator
  - equipment.loader
  - equipment.dozer
  - equipment.grader
  - equipment.drill
  - equipment.water_cart
  - equipment.light_vehicle

MaterialType:
  - ore
  - waste
  - overburden
  - topsoil

QueueType:
  - loading
  - dumping
  - crusher
  - weighbridge
  - refuelling

DowntimeType:
  - planned_maintenance
  - unplanned_breakdown
  - standby
  - operational_delay
  - shift_change
  - refuelling
  - weather

MaintenanceType:
  - preventive
  - corrective
  - inspection
  - service

Subsystem:
  - engine
  - transmission
  - hydraulics
  - brakes
  - tyres
  - electrical
  - body
  - cooling

FaultSeverity:
  - informational
  - warning
  - critical

IncidentType:
  - restricted_zone_breach
  - overspeed
  - proximity
  - unplanned_stop
  - overload

IncidentSeverity:
  - low
  - medium
  - high
  - critical

HaulDirection:
  - loaded
  - empty

GroundState:
  - dry
  - damp
  - wet
  - muddy
  - icy

VisibilityState:
  - clear
  - haze
  - dust
  - fog
  - smoke
  - dark

SurfaceType:
  - sealed
  - gravel
  - dirt
  - rock

OperationalRole:
  - extraction
  - processing
  - dumping
  - stockpile
  - maintenance
  - fuelling
  - haul_route
  - restricted
  - office
```

---

# Relations

The subject vocabulary is a list of nouns. Relations are what make it a
**model** — and what makes generic investigation possible at all.

Everything above says what exists. This section says how things connect.

## Why this exists

An analyzer investigating "why did efficiency drop" uses exactly two pieces
of domain knowledge:

```
cycle_time is made of queue + spot + load + haul + dump + return
fleet availability is an alternative explanation that must be ruled out
```

Both are relationships between terms already declared. Relationships are
data. Written down, an investigation strategy becomes a walk over this graph
rather than code that knows about mining.

**The graph says what is legal and true. A model still decides what is
relevant** — a naive walk requests everything reachable, which is useless.
The graph constrains; it does not choose.

## Five kinds of edge

```
decomposes    a whole into additive parts        which part grew?
partitions    a metric by an attribute           which one is responsible?
confounds     an alternative explanation         am I blaming the wrong thing?
influences    a directional causal link          what upstream caused this?
rolls_up      event-level into period-level      what granularity answers this?
```

## decomposes

A whole and its parts, in one of two modes.

```
additive        the whole is the sum of its parts, same unit throughout
multiplicative  the whole is the product of its parts, units differ
```

**The mode is mandatory and load-bearing.** A walk that treats a product as
a sum draws nonsense conclusions — `mass_moved` is `cycles × payload`, and
attributing a tonnage drop to "the sum of cycles and payload" is meaningless.
Additive parts share the whole's unit; multiplicative parts do not, and are
exempt from the unit check.

```yaml
decomposes:

  - whole: production_shift.mass_moved
    mode: multiplicative
    parts:
      - production_shift.cycles_completed
      - production_shift.mean_payload

  - whole: haul_cycle.load_time
    mode: multiplicative
    parts:
      - loading_event.bucket_count
      - loading_event.swing_time

  - whole: haul_cycle.cycle_time
    mode: additive
    parts:
      - haul_cycle.queue_time
      - haul_cycle.spot_time
      - haul_cycle.load_time
      - haul_cycle.haul_time
      - haul_cycle.dump_time
      - haul_cycle.return_time

  - whole: equipment_availability.scheduled_time
    mode: additive
    parts:
      - equipment_availability.available_time
      - equipment_availability.downtime

  - whole: equipment_availability.available_time
    mode: additive
    parts:
      - equipment_availability.operating_time
      - equipment_availability.standby_time

  - whole: equipment_availability.downtime
    mode: additive
    parts:
      - downtime_event.planned_duration
      - downtime_event.unplanned_duration
```

Ratios contribute decomposition implicitly. `utilization` is declared as
`operating_time / available_time`, so a walk investigating a utilization
drop already knows to examine both terms. Those edges are not repeated here.

The full worked graph, with basis annotations and causal traces, is in
`GRAPH.md`. This section is the machine-checkable subset.

## partitions

Every attribute is a legal partition key. This section declares the
**diagnostically ordered** ones — where to look first when localizing a
problem.

```yaml
partitions:
  - metric: haul_cycle.cycle_time
    by: [equipment_id, destination, material_type, operator_id, shift_id]

  - metric: haul_cycle.queue_time
    by: [location, destination, shift_id]

  - metric: production_shift.mass_moved
    by: [material_type, area, crew_id]

  - metric: downtime_event.duration
    by: [downtime_type, subsystem, equipment_id]

  - metric: queue_event.wait_time
    by: [queue_type, location, shift_id]

  - metric: route_segment.transit_time
    by: [segment_id, direction, equipment_class]
```

Ordering matters. It is the difference between "which truck" and "which
route" being the first question asked, and it encodes real operational
judgement about where problems usually live.

Partitioning is what `ordering` and `limit` on a requirement exist to serve:
split by `equipment_id`, rank by `cycle_time`, take the worst five.

## confounds

An alternative explanation that must be eliminated before a cause is
asserted.

```yaml
confounds:
  - factor: equipment_availability.availability
    affects: production_shift.mass_moved
    why: >
      fewer machines available moves less material regardless of how well
      the running machines performed

  - factor: haul_cycle.haul_distance
    affects: haul_cycle.cycle_time
    why: >
      a longer route legitimately takes longer — this is a plan change,
      not a performance loss
    conditional: comparing across different origins or destinations

  - factor: haul_cycle.material_type
    affects: haul_cycle.cycle_time
    why: different material goes to different destinations at different rates

  - factor: downtime_event.planned
    affects: equipment_availability.availability
    why: planned maintenance is a scheduling outcome, not a failure

  - factor: ground_condition.state
    affects: route_segment.mean_speed
    why: wet ground slows traffic irrespective of operator or machine

  - factor: production_shift.crew_id
    affects: production_shift.mass_moved
    why: crew rotation changes the comparison population
```

`conditional` marks a confounder that only applies in some framings. Haul
distance does not confound a comparison of the same route against itself; it
confounds a comparison across routes. A walk that ignores this either
over-fetches or draws a false conclusion.

## influences

A directional causal link. Walked upstream for root cause, downstream for
prediction.

`lag` is how long the effect takes to appear. `persistence` is how long it
outlasts its cause.

```yaml
influences:
  - from: weather_observation.rainfall
    to: ground_condition.state
    lag: 0
    persistence: 21600          # 6h — ground stays wet after rain stops

  - from: ground_condition.state
    to: route_segment.mean_speed
    lag: 0

  - from: visibility_condition.state
    to: route_segment.mean_speed
    lag: 0

  - from: route_segment.mean_speed
    to: haul_cycle.haul_time
    lag: 0

  - from: haul_cycle.cycle_time
    to: production_shift.mass_moved
    lag: 0

  - from: fault_event.occurrence_count
    to: downtime_event.duration
    lag: 0

  - from: downtime_event.duration
    to: equipment_availability.availability
    lag: 0

  - from: equipment_availability.availability
    to: production_shift.mass_moved
    lag: 0

  - from: loading_event.load_time
    to: haul_cycle.queue_time
    lag: 0
    why: a slow loader backs up the trucks waiting behind it
```

**`persistence` is load-bearing.** Rain that stopped before the window began
can still explain a slow shift. Without it, an investigation asking only
about the window would find clear skies and miss the cause entirely. A walk
following an influence edge backwards must widen its window by
`lag + persistence`.

That chain — rain to ground to speed to haul time to throughput — is how
"why did efficiency drop yesterday" reaches the answer "it rained the night
before" without anyone writing that path in code.

## rolls_up

Event-level subjects aggregate into period-level ones. Determines what
granularity a question needs.

```yaml
rolls_up:
  - from: haul_cycle
    to: production_shift
  - from: loading_event
    to: production_shift
  - from: haul_cycle
    to: material_movement
  - from: downtime_event
    to: equipment_availability
  - from: zone_visit
    to: entity_track
```

A question about a trend reads the period level. A question about a cause
descends to the event level. The edge is what lets a strategy choose.

## Cycles are permitted

Congestion genuinely feeds back: long queues slow cycles, slow cycles
lengthen queues. Banning cycles would force the graph to lie.

Instead, every walk is depth-bounded by `max_relation_depth` and must not
revisit a node within one walk.

## What did not fit cleanly

Recorded honestly, because it bounds how generic analyzers can be.

**Confounders are context-dependent.** Only `haul_distance` needed
`conditional` here, but the pattern will recur, and `conditional` is free
text a walk cannot evaluate. Today it is guidance for the model, not
machinery.

**Influence strength is not captured.** Every edge is equally weighted. Rain
matters more to speed than visibility usually does, and nothing says so.
Weights would need calibration against real outcomes, which is a part 2
concern.

**Some knowledge is not relational at all.** "Availability below 0.85 is
abnormal for this fleet" is a threshold, not an edge. Benchmarks like these
have no home in this section yet.

These are the seams where a generic strategy will still need judgement — and
the reason the model prunes the walk rather than the walk deciding alone.

---

# Derived Observables

A derived observable is a fact computed from other facts.

**It is produced upstream, not at query time.** A domain system watches the
world advance, derives the fact, and emits it as an event. The engine stores
it like any other fact, with provenance back to what caused it.

By the time an analyzer asks for `time_idle`, it is a stored, replayable,
traceable fact.

The engine runs the derivation. It does not know what the derivation means.
That is written here, once — and every analyzer inherits it.

Each definition below implies a domain system.

```yaml
derived_observables:

  speed:
    derived_from: [position fixes]
    definition: displacement over elapsed time between consecutive fixes
    unit: m/s

  distance_travelled:
    derived_from: [speed, elapsed time]
    definition: summed displacement along the path
    unit: m

  time_idle:
    derived_from: [speed, shift state, zone occupancy]
    emits: idle-period-ended
    definition: >
      cumulative time with speed below idle_speed_threshold for at least
      idle_min_duration, while the entity is on shift and not inside a zone
      marked excluded_from_productivity
    unit: s
    parameters:
      idle_speed_threshold: 0.5      # m/s
      idle_min_duration: 120         # s

  dwell_time:
    derived_from: [zone-entered, zone-exited]
    emits: zone-dwell
    definition: >
      duration of a single continuous presence within a zone, one value
      per entry
    unit: s
```

`zone-entered` and `zone-exited` are engine facts — the engine computes
containment because containment is geometry, not meaning.

`idle-period-ended` and `zone-dwell` are domain facts. The engine emits them
only because a domain system told it to.

## time_idle is the one to argue about

Idle is a business definition, not a physical one. A truck stopped in a
queue, stopped at shift change, and stopped with a fault are all "not
moving" and none of them mean the same thing operationally.

The definition above makes three choices, each of which is arguable:

**Queue time counts as idle.** A truck waiting for a loader is not producing.
The counter-argument is that queueing is a flow problem, not an equipment
problem, and blaming the truck hides the real cause.

**Workshop time does not count**, via `excluded_from_productivity`. A machine
under maintenance is unavailable, not idle — counting it twice would double
-penalise the same loss.

**Off-shift time does not count.** A parked truck at 2am is not idle, it is
off duty.

Whatever is agreed goes here, once. Every analyzer inherits it, and the
number becomes arguable in one place rather than five.

---

# Zones

**Zones are entities in the engine. Their geometry is not stored here.**

The engine owns where a zone is. This registry owns what a zone means.

```yaml
zones:
  - entity: ent_01J8XZ01          # the zone entity in the engine
    key: pit_3
    label: Pit 3
    operational_role: extraction

  - entity: ent_01J8XZ02
    key: crusher_primary
    label: Primary Crusher
    operational_role: processing

  - entity: ent_01J8XZ03
    key: workshop
    label: Workshop
    operational_role: maintenance
    excluded_from_productivity: true

  - entity: ent_01J8XZ04
    key: blast_area_c
    label: Blast Area C
    operational_role: restricted
    restricted: true
    restricted_to: [equipment.light_vehicle]
```

`restricted` is the clearest example of the boundary. The engine can compute
that a truck is inside a polygon. It cannot know that being inside that
polygon is a safety breach. That word lives here and nowhere else.

`excluded_from_productivity` is the same idea: time parked in the workshop
should not count against haul efficiency, and only the domain knows that.

Zones do double duty — they are also how `SpatialScope` resolves an operator
saying "Pit 3" into an entity id a requirement can carry.

A `SpatialPredicate` naming a zone absent from this registry is invalid,
even if the entity exists in the engine. Samaritan may only reason about
places it has been taught the meaning of.

---

# Sites And Calendars

Resolves timezone and shift calendar. Required for constraint resolution.

```yaml
sites:
  - id: site_01J8X0
    name: Northern Pit Operation
    organization: org_01J8X0
    timezone: Africa/Lagos
    calendar_family: northern_pit
```

## Calendars are time-versioned

A calendar has a validity range. Resolution selects the version covering the
window being resolved — **not** the version in force today.

```yaml
calendars:
  northern_pit:
    - version: v1
      effective_from: 2024-01-01
      effective_until: 2026-03-31
      operational_day_starts: "07:00"
      shifts:
        - { name: day,   start: "07:00", duration: 43200 }   # 12h
        - { name: night, start: "19:00", duration: 43200 }

    - version: v2
      effective_from: 2026-04-01
      effective_until: null
      operational_day_starts: "06:00"
      shifts:
        - { name: day,   start: "06:00", duration: 43200 }
        - { name: night, start: "18:00", duration: 43200 }
```

Without versioning, a site that changed shift patterns would silently
mis-resolve every historical window — applying today's shifts to last year's
events. That defeats the whole point of pinning `world_version` so old
investigations can be re-run faithfully.

**A window spanning a calendar change is rejected**, not silently split. It
returns `Incomplete`, naming both versions, and the operator narrows the
question. A comparison across a shift-pattern change is not a comparison.

## Time resolution rules

Relative expressions resolve against `asked_at` in the site's timezone, using
the covering calendar's `operational_day_starts` — **not** midnight.

```yaml
resolution:
  "now":              instantaneous, at the pinned world_version
  "today":            current operational day, from its start to asked_at
  "yesterday":        previous full operational day
  "this shift":       current shift, from its start to asked_at
  "last shift":       most recently completed shift
  "this week":        last 7 operational days
  "last N days":      N previous full operational days
  "last N shifts":    N most recently completed shifts
  "last N hours":     N × 3600 seconds ending at asked_at
  "<weekday>":        most recent completed operational day with that name
  "<date>":           the operational day beginning on that date
  "<date> to <date>": inclusive span of operational days
```

An expression not in this table does not get guessed. It returns
`Incomplete` with the phrase in `missing[]`.

---

# Baseline Defaults

When an intent needs a reference period and the operator named none.

```yaml
baseline_defaults:
  Explain:    trailing 30 operational days, ending at window start
  Predict:    trailing 90 operational days, ending at window start
  Recommend:  trailing 30 operational days, ending at window start
  Compare:    none — the operator must name both sides
  Locate:     none
  Summarize:  none
```

A default baseline is always recorded as `resolved_from: "default baseline"`
so it is never mistaken for an operator's request.

A default baseline is truncated at the covering calendar's
`effective_from`. A default must never silently reach across a calendar
change.

`Compare` deliberately has no default. A comparison with an invented second
side is worse than a rejection.

---

# Window Requirements

Which intents may omit a time window.

```yaml
window_required:
  Explain:    true
  Compare:    true
  Predict:    true
  Summarize:  true
  Recommend:  true
  Locate:     false      # defaults to the pinned world_version
```

`Locate` is the exception. "Where is truck 14?" is a complete question with
no time expression in it — it means *now*, and *now* is exactly what
`world_version` already pins.

Every other intent asks about a period. Absent a window, they return
`Incomplete` rather than defaulting, because a silently assumed window
produces a confident answer to a question nobody asked.

---

# Thresholds

```yaml
thresholds:
  intent_confidence_floor: 0.60    # below this, return Ambiguous
  domain_relevance_floor: 0.50     # below this, drop the domain
  max_domains: 5
  max_analyzers: 8
  max_relation_depth: 3            # bounds every graph walk
  default_max_runtime: 30          # s
  default_priority: Normal
```

`max_relation_depth` is what keeps a walk from requesting the world. The
influence chain rain → ground → speed → haul_time → mass_moved is four hops;
a depth of 3 from either end reaches most of it, and the model prunes the
rest.

## Precedence

Culling applies in a fixed order, and the order is load-bearing.

```
1. drop every domain scoring below domain_relevance_floor
2. rank the survivors
3. truncate to max_domains
4. resolve analyzers from the surviving domains
5. if analyzers exceed max_analyzers, drop from the lowest-ranked domain
   upward, never partially covering a domain
```

Step 5 never leaves a domain half-served. A domain is either investigated or
it is not — a partial answer attributed to a fully-ranked domain is worse
than an acknowledged omission.

`intent_confidence_floor` is the line between classifying and guessing.

---

# Model Configuration

Pinned for reproducibility. Participates in every cache key.

```yaml
models:
  planning:
    model_id: claude-opus-4-8
    temperature: 0
    prompt_template_version: planning/2026-07-01

  analyzers:
    default:
      model_id: claude-opus-4-8
      temperature: 0
      prompt_template_version: analyzer/2026-07-01
    overrides: {}        # per-analyzer, keyed by analyzer name
```

An analyzer's effective prompt version is folded into its own `version`, so
a prompt change invalidates that analyzer's cached output without
invalidating every other analyzer's.

Changing any value here changes Samaritan's behaviour and must bump
`registry_version`.

---

# Registry Validation

Every check the loader performs. **The registry does not load if any fails.**

## Errors

```
E01  an analyzer declares a subject not in the subject vocabulary
E02  an analyzer declares a domain not in the domain set
E03  an analyzer declares an intent not in the intent set
E04  two analyzers share a name
E05  an enum-typed attribute names an enumeration that does not exist
E06  a ratio's definition references an observable not on its own subject
E07  a derived observable is not an observable of any subject
E08  a spatial predicate is permitted on a subject without spatial: true
E09  a zone entry omits its engine entity id
E10  a zone's operational_role is not in OperationalRole
E11  a calendar family has overlapping or gapped validity ranges
E12  a site names a calendar family that does not exist
E13  a strategy is missing for a declared intent
E14  a baseline default names an intent not in the intent set
E15  an observable declares a unit inconsistent with its type
E16  a duration observable is named *_hours or *_minutes
E17  a ratio observable declares a percentage unit
E18  a relation references an observable or attribute not in the vocabulary
E19  a decomposition declares no mode, or a mode outside {additive,
     multiplicative}
E20  an additive decomposition's parts do not all share the whole's unit
     (multiplicative decompositions are exempt — their parts differ by unit
     by definition)
E21  a partition names an attribute the metric's subject does not have
E22  a confounder or influence names a subject no analyzer can reach
E23  a rolls_up edge names a subject that does not exist
```

`E16` and `E17` exist because unit bugs hide in names. An observable called
`available_hours` measured in seconds will be misread by a human eventually,
whatever the schema says.

## Warnings

```
W01  a domain has no analyzer covering it
W02  a subject is in no analyzer's subject list — it is unreachable
W03  an observable is on no subject reachable by any analyzer
W04  an enumeration is referenced by no attribute and no zone field
W05  max_analyzers exceeds the number of registered analyzers
W06  a zone is marked restricted with no restricted_to list
W07  a numeric observable participates in no relation — no walk can reach it
W08  an influence edge declares persistence without lag, or neither
W09  a subject has no partitions declared — problems in it cannot be localized
```

`W07` is the graph's equivalent of `W02`. An observable in the vocabulary
that no relation touches can be requested by name but will never be reached
by an investigation, which usually means a relation is missing rather than
the observable being useless.

`W02` is the check that would have caught `entity_track` sitting in the
vocabulary with no analyzer able to request it — a whole capability present
and unreachable.

---

# Versioning

The registry is versioned as a single unit.

```
registry_version: 1.0.0
```

Additive change — new analyzer, new subject, new observable, new enum member
— increments minor.

Removing or redefining a term increments major, because existing cached
plans and requirement sets may no longer be interpretable.

## Registry version vs analyzer version

Both exist and they answer different questions.

```
registry_version   what vocabulary and mappings were in force
analyzer version   how one analyzer turns a plan into requirements
```

A change to an analyzer's declared domains, intents or subjects bumps
**both** — the registry because the mapping changed, the analyzer because
its behaviour did.

A change to an analyzer's prompt or logic bumps **only** its own version, so
one analyzer's cache invalidates without discarding every other's.

`registry_version` is recorded in every `PlanProvenance` and participates in
every cache key.

---

# Guiding Principle

The registry is where Samaritan is configured, and therefore where it is
argued about.

If a behaviour is worth debating, it belongs here — not buried in code.
