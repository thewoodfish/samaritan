# REQUIREMENTSET.md

> The `RequirementSet` is the terminal artifact of Samaritan's part 1.
>
> It is also, and more importantly, the **specification of the Reality
> Engine's request API**. Whatever a RequirementSet can express, the engine
> must be able to answer. Whatever it cannot express, the engine does not need
> to support.
>
> This document describes its shape, its contracts, and its invariants — so
> Meredith's API can be shaped to fit before part 2 begins.

The shapes here are taken from the running implementation
(`crates/schema`), not from intention. The worked example was emitted by the
pipeline itself — regenerate it any time with:

```
cargo run -p samaritan --example dump_requirementset
```

---

# 1. What it is

A `RequirementSet` is the complete, deduplicated set of questions Samaritan
needs to ask about the world in order to investigate one operator question.
It is produced by Dispatch after the analyzers have run.

It answers, precisely: *given this investigation, what facts must the engine
supply?* Every entry is a self-contained data request. None of them consume
an answer — part 1 stops at the request.

Two consumers read it:

- **The operator / caller** — to see what will be asked, and what could not be.
- **The Reality Engine** — as the batch of requests to serve.

The engine only ever sees the second view. Designing its API is therefore
exactly the exercise of designing a service that consumes a RequirementSet
and returns, per requirement, the facts it names.

---

# 2. Position in the pipeline

```
Question
   │  planning (validate, normalize, intent, domains, constraints, assemble)
   ▼
InvestigationPlan
   │  dispatch (fan out to analyzers, each walks the relation graph)
   ▼
InformationRequirement[]     ← one analyzer's declarations
   │  dispatch (dedup · validate · order)
   ▼
RequirementSet               ← THIS DOCUMENT
   │
   ═══ boundary ═══  the engine begins here (part 2)
```

---

# 3. Top-level shape

```
RequirementSet
  id             Id            "reqset_…"
  schema_version SchemaVersion
  created_at     Timestamp

  plan_id        Id            the InvestigationPlan this came from
  question_id    Id            the original operator question
  world_version  WorldVersion  the projection every requirement reads

  requirements   InformationRequirement[]   deduplicated, normalized order
  unserviceable  Unserviceable[]            well-formed but unanswerable
  execution      ExecutionReport            which analyzers ran, and how
  complete       bool                       true only if nothing failed,
                                            timed out, or was unserviceable
```

The set is the unit the engine receives. `world_version` at the top is the
single projection all its requirements are asked against — the engine pins
its event log to that position for the whole batch (see §7).

`complete` is a headline flag. `false` means the operator is looking at a
partial picture; the engine is not expected to act on it, but it *is* a valid
set (`ENGINE.md`: unavailable is not empty).

---

# 4. `InformationRequirement` — the request unit

**This is the heart of the contract.** One requirement is one question about
one subject. The union of everything a requirement can express is the
engine's request surface.

```
InformationRequirement
  id             Id            unique within the set
  plan_id        Id            provenance → the plan
  requested_by   string[]      the analyzer(s) that asked; >1 after dedup
  purpose        string        why, in prose — for humans, not the engine
  necessity      Necessity     Required | Preferred | Optional

  # WHAT
  subject        string        a registered subject type
  observables    string[]      observables of that subject to return
  filters        Filter[]      attribute/observable predicates (AND)
  spatial        SpatialPredicate[]   geometric predicates (AND)

  # WHEN
  window         TimeWindow    absolute, resolved, UTC
  baseline       TimeWindow?   a reference period, when the intent needs one

  # WHERE / WHICH
  scope          SpatialScope  site-wide, an area, a named zone, or unspecified
  entities       EntityRef[]   specific named entities, may be empty

  # HOW TO SHAPE THE ANSWER
  granularity    Granularity   Raw | PerEvent | Bucketed{s} | Shift | Daily
  group_by       string[]      attribute names to group by (e.g. equipment_id)
  aggregations   Aggregation[] op + field, computed per group
  ordering       Ordering?     rank by a field
  limit          integer?      take the top N (requires ordering)
  expected_shape Shape         Scalar | Series | Set | Table | Histogram

  # RESERVED (part 2)
  round          integer       always 0 in part 1
  depends_on     Id[]          always empty in part 1
```

Read a requirement as a sentence:

> For **subject** `haul_cycle`, return **observables** `queue_time … return_time`,
> **filtered** by nothing, over **window** yesterday (vs **baseline** last 30
> days), across the **whole site**, **bucketed** hourly, **aggregated** by mean,
> as a **Series**.

That is a complete, executable data request. Nothing about it references a
table, a sensor, or a storage system — only *what is wanted*.

---

# 5. The request vocabulary

Every field the engine must interpret, with its closed value sets.

## Subject and observables

- `subject` — a **registered subject type** (`haul_cycle`,
  `equipment_availability`, `queue_event`, …). The engine stores these as
  opaque named types; their meaning lives in Samaritan's registry. The engine
  never needs to know what a haul cycle *is* — only that it is a type with
  observables and attributes.
- `observables` — the measured or derived fields of the subject to return
  (`cycle_time`, `availability`, …). **Derived observables** (`speed`,
  `time_idle`, `dwell_time`) are requested exactly like measured ones; the
  engine stores them as facts produced upstream, not computed at query time
  (see §9).

## Filter — scalar predicates

```
Filter
  field  string      an observable or attribute of the subject
  op     Eq | Neq | In | NotIn | Gt | Gte | Lt | Lte
  value  scalar | scalar[]     scalar = bool | int | float | string
```

`value` serializes as the bare JSON value — `"haul_truck"`, `5`, `true`, or
`["a","b"]` — not a wrapped variant. Multiple filters combine with **AND**.

## SpatialPredicate — geometric predicates

```
SpatialPredicate
  op      Within | Intersects | Contains | Outside
  region  RegionRef

RegionRef
  kind    Named | Radius | Bounds
  ref     Id?        zone entity id, when kind = Named
  center  Point?     when kind = Radius
  radius  metres?    when kind = Radius
  bounds  Point[]?   closed polygon, when kind = Bounds

Point { lat: float, lon: float }
```

Multiple predicates combine with **AND**. A `Named` region references a **zone
entity in the engine, by id** — Samaritan never carries geometry. Spatial
predicates are only valid on spatial subjects (those carrying position over
time); Dispatch rejects the rest, so the engine never sees them.

## Granularity — the row grain

```
Raw                 every individual record
PerEvent            one row per occurrence of the subject
Bucketed{ bucket_size: seconds }   fixed interval
Shift               one row per shift
Daily               one row per operational day
```

Serialization note: unit variants are bare strings (`"Shift"`); `Bucketed`
is `{ "Bucketed": { "bucket_size": 3600.0 } }`.

## group_by, aggregations, ordering, limit — the reduction

```
group_by      string[]     attribute names; groups rows before aggregating
Aggregation   { op, field?, bins? }
  op     Count | Sum | Mean | Median | Min | Max
       | P90 | P95 | P99 | StdDev | Rate | Histogram
  field  an observable; omitted only for Count
  bins   required when op = Histogram
Ordering      { by: string, direction: Asc | Desc }
limit         integer      take the top N; requires ordering
```

"The five worst trucks by mean cycle time" is: `group_by: [equipment_id]`,
`aggregations: [Mean(cycle_time)]`, `ordering: { by: cycle_time, direction:
Desc }`, `limit: 5`, `expected_shape: Set`.

Empty `aggregations` means return the underlying records at the requested
granularity.

## Shape — the expected answer form

```
Scalar     one value
Series     values over time
Set        a collection of entities
Table      rows with named columns
Histogram  a binned distribution
```

`expected_shape` is a declaration of what the requester expects, so a mismatch
between request and response is caught at the boundary rather than deep in
analysis. It constrains the response contract part 2 will define.

---

# 6. Time and world

## TimeWindow

```
TimeWindow
  start          Timestamp   inclusive, UTC
  end            Timestamp   exclusive, UTC
  resolved_from  string      the verbatim phrase, e.g. "yesterday"
  calendar       Id          "family@version", e.g. "northern_pit@v2"
  timezone       string      IANA, e.g. "Africa/Lagos"
```

**The engine only ever sees absolute UTC windows.** All relative-time and
shift-calendar reasoning happens in Samaritan; `resolved_from`, `calendar`,
and `timezone` are audit trail, not instructions. The engine does not parse
"yesterday" and does not own a shift calendar.

`baseline` is a second `TimeWindow` of the same shape. When present, the
engine returns the same observables/aggregations for both periods, so the
requester can compare.

## WorldVersion

```
WorldVersion
  log_position   integer     event-log sequence number
  as_of          Timestamp   the instant that position represents
  snapshot       Id?         nearest snapshot, if one was used
```

`world_version` is set **once, on the set**, and every requirement is answered
against it. The engine projects its append-only log to `log_position` and
answers the whole batch there — so late-arriving events and identity
corrections do not change what a replayed investigation sees.

---

# 7. Invariants — what the engine can rely on

Dispatch guarantees all of the following before a RequirementSet leaves
Samaritan. The engine may treat them as preconditions and need not re-check.

1. **`subject` is always a registered subject type.** An unregistered subject
   is recorded `unserviceable`, never placed in `requirements`.
2. **Every `observable` belongs to its `subject`.** Same for every
   `filters[].field`, `aggregations[].field`, `ordering.by`, and `group_by`
   entry — all are drawn from the subject's registered vocabulary.
3. **Every `Named` spatial region references a registered zone entity id.**
4. **Spatial predicates appear only on spatial subjects.**
5. **`limit` implies `ordering`.** A limit without an ordering is invalid and
   is rejected before the boundary.
6. **`window` and `baseline` are absolute, UTC, resolved.** No relative
   expressions, ever.
7. **One requirement names exactly one subject.** There are **no cross-subject
   joins.** If an investigation needs two subjects, it issues two
   requirements. The engine never joins — this is a deliberate simplification
   of its contract.
8. **`round` is 0 and `depends_on` is empty** for every requirement. Part 1 is
   a single, stateless batch; there is no requirement that depends on another's
   answer.
9. **`id` is unique within the set**, and every requirement's `plan_id`
   matches the set's `plan_id`.
10. **The set is order-normalized.** `requirements` are sorted by
    `(subject, first requester, id)`, so two runs of the same investigation
    produce byte-identical requirement lists.

These invariants are what make the engine's job small: it consumes a
pre-validated, pre-normalized batch of single-subject requests over one
pinned world.

---

# 8. Unserviceable — the response model in miniature

```
Unserviceable
  requirement_id  Id
  requested_by    string[]
  necessity       Necessity
  reason          UnavailableSubject | UnavailableObservable
                | UnregisteredTerm | UnknownZone
  detail          string
```

`unserviceable` holds requirements that are **well-formed but cannot be
answered** — a subject the domain layer never registered at this site, an
observable with no source, a zone Samaritan was not taught. They are recorded,
never dropped.

**This directly shapes the engine's response model.** The engine, too, must be
able to answer per-requirement with *"I cannot serve this"* — a subject that is
registered in the vocabulary but has no data at this site, for example, is
something only the engine knows. So the response contract in part 2 is not
"a batch of answers" but **"a per-requirement result, each either data or a
typed unavailability."**

> Unavailable is not empty. An engine that returns zero rows for
> "restricted-zone breaches yesterday" must be distinguishable from one that
> could not look. This is the single most important property the response API
> must preserve (`ENGINE.md`).

`ExecutionReport` (completed / empty / failed / timed_out per analyzer) is
Samaritan-side observability and is **not** part of what the engine consumes.

---

# 9. Where computation happens

The RequirementSet assumes a specific division of labour (`SCHEMA.md`,
Aggregation Placement — settled). The engine's API must match it.

```
derivation   upstream, as the world advances    facts → facts
             (speed, time_idle, zone dwell — produced by domain systems,
              stored as events; NOT computed at query time)

reduction    at query time, in the engine        facts → numbers
             (Mean, P95, Sum, Count, Histogram, spatial containment,
              group_by, ordering, limit)

reasoning    in Samaritan                         numbers → meaning
             (correlation, hypotheses — never asked of the engine)
```

So the engine must, at query time: filter, group, aggregate (the reductions
above), evaluate spatial predicates, and window by time — over facts it
already holds. It must **not** be asked to infer, correlate, or derive
`time_idle` on the fly. A requirement for `time_idle` is a request for a
stored fact, not a computation.

---

# 10. Worked example

The real output for *"why did efficiency drop yesterday?"* — three analyzers
(efficiency, flow, maintenance), seven requirements after dedup, all
serviceable. Trimmed here to two representative requirements; the full set is
emitted by the example command above.

```json
{
  "id": "reqset_01J8XQ7K3M",
  "schema_version": "1.0.0",
  "created_at": "2026-07-21T09:14:00Z",
  "plan_id": "plan_01J8XQ7K3M",
  "question_id": "q_01J8XQ7A11",
  "world_version": {
    "log_position": 1284662,
    "as_of": "2026-07-21T09:14:00Z",
    "snapshot": "snap_01J8XP"
  },
  "requirements": [
    {
      "id": "req_01J8XQ7K3M_efficiency_0002",
      "plan_id": "plan_01J8XQ7K3M",
      "requested_by": ["efficiency"],
      "purpose": "Decompose haul_cycle.cycle_time into its parts to locate which grew.",
      "necessity": "Required",
      "subject": "haul_cycle",
      "observables": ["queue_time","spot_time","load_time","haul_time","dump_time","return_time"],
      "filters": [],
      "spatial": [],
      "window": {
        "start": "2026-07-20T05:00:00Z",
        "end": "2026-07-21T05:00:00Z",
        "resolved_from": "yesterday",
        "calendar": "northern_pit@v2",
        "timezone": "Africa/Lagos"
      },
      "baseline": {
        "start": "2026-06-20T05:00:00Z",
        "end": "2026-07-20T05:00:00Z",
        "resolved_from": "default baseline",
        "calendar": "northern_pit@v2",
        "timezone": "Africa/Lagos"
      },
      "scope": { "kind": "Unspecified", "label": "entire site" },
      "entities": [],
      "granularity": { "Bucketed": { "bucket_size": 3600.0 } },
      "aggregations": [
        { "op": "Mean", "field": "queue_time" },
        { "op": "Mean", "field": "spot_time" },
        { "op": "Mean", "field": "load_time" },
        { "op": "Mean", "field": "haul_time" },
        { "op": "Mean", "field": "dump_time" },
        { "op": "Mean", "field": "return_time" }
      ],
      "expected_shape": "Series",
      "round": 0,
      "depends_on": []
    },
    {
      "id": "req_01J8XQ7K3M_efficiency_0003",
      "plan_id": "plan_01J8XQ7K3M",
      "requested_by": ["efficiency"],
      "purpose": "Rule out equipment_availability.availability as an alternative explanation.",
      "necessity": "Preferred",
      "subject": "equipment_availability",
      "observables": ["availability","available_time","scheduled_time"],
      "filters": [],
      "spatial": [],
      "window": { "start": "2026-07-20T05:00:00Z", "end": "2026-07-21T05:00:00Z", "resolved_from": "yesterday", "calendar": "northern_pit@v2", "timezone": "Africa/Lagos" },
      "baseline": { "start": "2026-06-20T05:00:00Z", "end": "2026-07-20T05:00:00Z", "resolved_from": "default baseline", "calendar": "northern_pit@v2", "timezone": "Africa/Lagos" },
      "scope": { "kind": "Unspecified", "label": "entire site" },
      "entities": [],
      "granularity": "Shift",
      "aggregations": [
        { "op": "Mean", "field": "availability" },
        { "op": "Mean", "field": "available_time" },
        { "op": "Mean", "field": "scheduled_time" }
      ],
      "expected_shape": "Table",
      "round": 0,
      "depends_on": []
    }
  ],
  "unserviceable": [],
  "execution": {
    "completed": [
      { "analyzer": "efficiency", "version": "1.0.0", "duration": 0.00009, "requirement_count": 4 },
      { "analyzer": "flow", "version": "1.0.0", "duration": 0.00003, "requirement_count": 1 },
      { "analyzer": "maintenance", "version": "1.0.0", "duration": 0.00002, "requirement_count": 2 }
    ],
    "empty": [], "failed": [], "timed_out": []
  },
  "complete": true
}
```

Note the two requirements share nothing but shape: different subjects,
observables, granularities, aggregations, and shapes. Each is independently
answerable. That independence is the property the engine's API can exploit —
requests fan out with no ordering or joining between them.

---

# 11. What this means for Meredith's API

Reading the invariants and the response model backward gives a concrete
starting shape for the engine.

## Request

A single endpoint that takes a **batch of independent, single-subject
requests over one pinned world version**:

```
answer(world_version, requirement[]) -> result[]
```

Each `requirement` is exactly the `WHAT / WHEN / WHERE / HOW` fields above
(id, subject, observables, filters, spatial, window, baseline, scope,
entities, granularity, group_by, aggregations, ordering, limit,
expected_shape). The engine can ignore the Samaritan-facing fields
(`requested_by`, `purpose`, `necessity`, `plan_id`, `round`, `depends_on`) —
though `necessity` is useful if the engine sheds load (serve `Required` first).

The engine needs, per subject type, to know its observables and attributes —
this is the same vocabulary Samaritan holds, so the registry (or a projection
of it) is shared configuration, not something the engine invents.

## Response

**Per-requirement, keyed by `requirement.id`**, each result is one of:

```
result
  requirement_id  Id
  outcome         Data(payload, shape) | Unavailable(reason, detail)
```

- `Data` carries the answer in the requirement's `expected_shape`
  (scalar / series / set / table / histogram), plus the same shape for
  `baseline` when one was requested.
- `Unavailable` mirrors Samaritan's own `Unserviceable` reasons — the engine
  reports "registered but no data at this site," and the two unavailability
  models compose into one honest picture.

## Engine responsibilities, bounded

The engine must, at query time: **window, filter, group, aggregate, and
evaluate spatial predicates over stored facts.** It must resolve zone entity
ids. It must project its log to `log_position`. It must distinguish
*no data* from *empty result*.

The engine must **not**: parse relative time, own a shift calendar, join
across subjects, derive `time_idle`/`speed` at query time, or reason about
what any of it means.

## What the batch shape buys the engine

Because requirements are independent (invariant 7), single-subject
(invariant 7), stateless (invariant 8), and pre-validated (invariants 1–5),
the engine's executor is a fan-out of simple per-subject queries with no
planner, no join engine, and no session state. The hard parts — language,
intent, shift calendars, causal structure — all stayed in Samaritan.

---

# 12. Open decisions that shape the engine

Called out so they are settled deliberately, not by accident, before part 2.

1. **Aggregation placement — settled.** The engine computes reductions. The
   API accepts `aggregations`, `group_by`, `ordering`, `limit`. (§9)

2. **Iteration — deferred.** Part 1 is one stateless batch (`round: 0`,
   `depends_on: []`). Some investigations genuinely need a second round that
   depends on the first answer ("which truck was slowest?" → "what happened to
   *that* truck?"). The schema reserves `round` and `depends_on` so a session
   API can be added without a breaking change — but the v1 engine can be a
   pure request/response batch. **Decide before hardening the engine whether
   v1 is batch-only.** (`PIPELINE.md`, Open Decisions)

3. **Response shape contract — undefined.** `expected_shape` says what
   Samaritan expects; the exact serialization of a returned series / set /
   table / histogram is part 2's to define. It should be defined against these
   shapes, not invented.

4. **Grouping and windowing semantics** need pinning: half-open windows
   (`start` inclusive, `end` exclusive) are assumed here; bucket alignment and
   partial-bucket handling at window edges are engine decisions.

5. **Cross-subject joins — deliberately absent.** If part 2 finds this too
   limiting, adding joins is a real expansion of the engine's contract; the
   current design assumes it stays out.

---

# Guiding principle

> The RequirementSet is the whole of what Samaritan asks.
>
> Design the engine to answer exactly this — no less, so investigations are
> not starved; no more, so the engine stays a retrieval-and-reduction surface
> and never a second brain.
