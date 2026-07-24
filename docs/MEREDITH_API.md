# MEREDITH_API.md

> The contract between Samaritan and the Reality Engine (Meredith).
>
> Samaritan produces a `RequirementSet` — a batch of precise, pre-validated
> data requests over one pinned projection of the world. This document
> specifies the API Meredith exposes to answer it.
>
> It is written from Samaritan's side: it says exactly what will be asked, how
> often, in what shape, and what must come back. Meredith's job is to answer
> **exactly this — no less, so investigations are not starved; no more, so the
> engine stays a retrieval-and-reduction surface and never a second brain.**

Read [`REQUIREMENTSET.md`](REQUIREMENTSET.md) first — it is the artifact this
API consumes. Read [`ENGINE.md`](ENGINE.md) for the boundary philosophy and
[`WORLD.md`] (in the Meredith repo) for the world model this sits on.

---

# 0. Status and scope

**In scope:** the read/query surface Meredith exposes to Samaritan — the API
that consumes a `RequirementSet` and returns per-requirement results, plus the
supporting endpoints (capabilities, world position) Samaritan needs to form a
request.

**Out of scope:** ingestion, the event log, plugins, rendering, and everything
about how Meredith *builds* the world. Those are Meredith's own concern; this
document only assumes the world exists and is projectable to a pinned
position.

**Transport:** the contract is transport-agnostic. The primary binding is
**HTTP + JSON** (Samaritan is Rust, Meredith is TypeScript — a network
boundary is the practical default). A gRPC or in-process binding may carry the
same shapes. All types below are given in TypeScript interface notation
(Meredith's language) and map 1:1 to their JSON wire form.

---

# 1. The mental model

Samaritan speaks in **subjects, observables, and attributes**. Meredith speaks
in **entities, components, events, and a projected log**. The API is the
translation, and it is simpler than it sounds.

Every Samaritan **subject** (`haul_cycle`, `equipment_availability`,
`entity_track`, …) is, to the query API, a **stream of timestamped records**.
Each record carries:

- a **time** (an instant, or an interval for period subjects),
- a set of **observable** values — the numeric measures (`cycle_time`,
  `availability`),
- a set of **attribute** values — the categorical dimensions (`equipment_id`,
  `material_type`),
- and, for **spatial** subjects, a **position or geometry** over time.

How a subject is *backed* inside Meredith varies — some are events, some are
periods derived by plugin systems, some are entity component histories — but
the query surface is uniform over all of them. That backing is hidden behind
the **capability catalog** (§4).

So the query API is, in one sentence:

> An OLAP-style query over domain-registered record streams, evaluated against
> a pinned projection of the event log.

`SELECT` aggregations of observables, `GROUP BY` attributes (and time buckets),
`WHERE` filters and spatial predicates and a time window, `FROM` one subject,
`AS OF` one log position. That is the whole of it.

---

# 2. Design principles

Non-negotiable properties the API must preserve. Each is enforced or assumed
elsewhere in Samaritan; the engine must not violate them.

1. **Domain agnostic.** The engine stores and returns registered types by
   name. It never learns what a haul cycle *means*. Meaning lives in
   Samaritan's registry, shared as configuration (§4).

2. **Projection-pinned.** Every request names one `world_version`
   (`log_position`). The engine projects its log to exactly that position and
   answers the whole batch there. Two requests at the same position return the
   same answer, forever — replay is a first-class property, not a feature.

3. **Per-requirement independence.** Requirements in a batch share nothing but
   the world version. There is no ordering, no joining, and no data flow
   between them. The engine may answer them in any order, in parallel.

4. **Single subject per requirement.** One requirement names exactly one
   subject. **The engine never joins across subjects.** If an investigation
   needs two subjects, Samaritan sends two requirements. This is the largest
   simplification of the engine's contract — honour it.

5. **Unavailable is not empty.** The single most important property. The engine
   must distinguish *"I looked and found nothing"* (a valid empty result) from
   *"I cannot look — no source or derivation for this"* (an unavailability).
   Conflating them is a correctness failure, not a cosmetic one (§6, §7).

6. **Computes, does not reason.** The engine filters, windows, groups,
   aggregates, and evaluates spatial predicates over facts it already holds. It
   does **not** infer, correlate, rank across subjects, derive facts at query
   time, or form any opinion (§9).

7. **Stateless batch (v1).** A `RequirementSet` is a single self-contained
   request. No sessions, no rounds, no requirement depends on another's answer
   (`round: 0`, `depends_on: []` always). See §11 for the reserved path to
   iteration.

---

# 3. Endpoints

The primary binding. Four endpoints; `query` is the one that matters.

```
POST /v1/query            answer a RequirementSet batch
GET  /v1/capabilities     what subjects/observables the engine can serve
GET  /v1/world/head       the current log head (to pin a world_version)
POST /v1/world/resolve    resolve a timestamp to a log_position
```

## 3.1 `POST /v1/query`

The heart of the API. Consumes a batch, returns one result per requirement.

```ts
interface QueryRequest {
  world_version: WorldVersion;      // the projection to answer against
  requirements: EngineRequirement[]; // the batch (§5)
}

interface QueryResponse {
  world_version: WorldVersion;      // echoed: the position actually served
  results: RequirementResult[];     // one per requirement, keyed by id
}
```

- The engine projects to `world_version.log_position` **once** and answers
  every requirement there.
- `results` is keyed by `requirement_id`; order is not significant (the caller
  matches by id).
- A malformed batch, or a `log_position` the engine cannot serve (see §8), is a
  transport-level error (HTTP 4xx/5xx). A requirement the engine cannot answer
  is **not** — it is a `200` with an `unavailable` outcome (§6).

## 3.2 `GET /v1/capabilities`

How the engine advertises what it can serve — the ground truth behind every
`unavailable` reason. Samaritan (and operators) use this to know coverage.

```ts
interface Capabilities {
  registry_version: string;        // vocabulary the engine was configured with
  subjects: Record<string, SubjectCapability>;
  zones: ZoneCapability[];         // registered zone entities
  supported_aggregations: AggregationOp[];
  supported_spatial_ops: SpatialOp[];
}

interface SubjectCapability {
  spatial: boolean;                // carries position/geometry over time
  observables: Record<string, { type: string; unit?: string; derived: boolean }>;
  attributes: Record<string, { type: string; enum?: string }>;
  // Whether records actually exist for this subject at this site is a runtime
  // matter; presence here means the engine is *wired to serve* the subject.
}

interface ZoneCapability {
  entity: string;                  // engine entity id, referenced by Named regions
  key: string;
  label: string;
}
```

**Contract:** a subject/observable present in `capabilities` may be requested
and will return data (possibly empty). A subject/observable **absent** here, if
requested, returns `unavailable` — never an empty success. This is the
mechanism that makes principle 5 real.

`registry_version` must match the `registry_version` Samaritan pins in the
plan; a mismatch is a configuration error the caller should surface.

## 3.3 `GET /v1/world/head`

Samaritan pins a `world_version` at plan time. To do so it must know the
current log position.

```ts
interface WorldHead {
  log_position: number;            // the latest committed event
  as_of: string;                   // RFC 3339 UTC, the instant that position represents
  snapshot?: string;               // nearest snapshot id, if any
}
```

## 3.4 `POST /v1/world/resolve`

To pin a **historical** investigation, Samaritan needs the log position that
was current at some past instant.

```ts
interface ResolveRequest { as_of: string; }          // RFC 3339 UTC
interface ResolveResponse { log_position: number; as_of: string; snapshot?: string; }
```

Returns the greatest `log_position` whose event time is `≤ as_of`. This is how
"the world as it stood last Tuesday" becomes a concrete pin.

---

# 4. The capability catalog and the vocabulary

Samaritan and Meredith **share one vocabulary** — the registry
(`REGISTRY.md`). Subjects, observables, and attributes are defined there;
Meredith is configured from the same source (or a projection of it), which is
why `capabilities.registry_version` must agree with the plan's.

This is the seam that keeps the engine domain-agnostic while still being able
to answer domain queries: the *names* are shared configuration; the *meaning*
of the names stays in Samaritan. The engine matches `subject` and `observable`
strings against its catalog and retrieves records — it never interprets them.

**Derived observables** (`speed`, `time_idle`, `dwell_time`) appear in the
catalog with `derived: true`. They are **stored facts**, produced upstream by
Meredith's plugin systems as the log advances (`SCHEMA.md`, Aggregation
Placement). The query layer treats them identically to measured observables —
it never computes `time_idle` on the fly. If a derived observable's producing
system is not installed, the observable is simply absent from the catalog, and
a request for it returns `unavailable`.

---

# 5. The request: `EngineRequirement`

The engine receives each `InformationRequirement` from the set. It reads the
fields below and **ignores the Samaritan-facing ones** (`requested_by`,
`purpose`, `plan_id`, `round`, `depends_on`) — with one optional exception:
`necessity` may be used for load-shedding (serve `Required` first under
pressure).

```ts
interface EngineRequirement {
  id: string;                      // echo back on the result

  // WHAT
  subject: string;                 // one registered subject
  observables: string[];           // measures to return
  filters: Filter[];               // attribute/observable predicates (AND)
  spatial: SpatialPredicate[];     // geometric predicates (AND)

  // WHEN
  window: TimeWindow;              // absolute, UTC, half-open [start, end)
  baseline?: TimeWindow;           // a second window; same query, returned alongside

  // WHERE / WHICH
  scope: SpatialScope;             // site | area | named zone | unspecified
  entities: EntityRef[];           // specific named entities (may be empty)

  // HOW TO SHAPE THE ANSWER
  granularity: Granularity;        // Raw | PerEvent | Bucketed{s} | Shift | Daily
  group_by: string[];              // attribute names to group by
  aggregations: Aggregation[];     // op + field, computed per group
  ordering?: Ordering;             // rank by a field
  limit?: number;                  // top N (implies ordering)
  expected_shape: Shape;           // Scalar | Series | Set | Table | Histogram

  // OPTIONAL (load-shedding only)
  necessity?: "Required" | "Preferred" | "Optional";
}
```

The supporting types, exactly as they arrive on the wire:

```ts
interface TimeWindow {
  start: string;                   // RFC 3339 UTC, inclusive
  end: string;                     // RFC 3339 UTC, exclusive
  resolved_from: string;           // audit only — the engine ignores it
  calendar: string;                // audit only
  timezone: string;                // audit only
}

interface WorldVersion {
  log_position: number;
  as_of: string;                   // RFC 3339 UTC
  snapshot?: string;
}

type Filter = {
  field: string;                   // observable or attribute of the subject
  op: "Eq" | "Neq" | "In" | "NotIn" | "Gt" | "Gte" | "Lt" | "Lte";
  value: Scalar | Scalar[];        // bare JSON value; array for In/NotIn
};
type Scalar = boolean | number | string;

interface SpatialPredicate {
  op: "Within" | "Intersects" | "Contains" | "Outside";
  region: RegionRef;
}
interface RegionRef {
  kind: "Named" | "Radius" | "Bounds";
  ref?: string;                    // zone entity id, when kind = Named
  center?: { lat: number; lon: number };  // when kind = Radius
  radius?: number;                 // metres, when kind = Radius
  bounds?: { lat: number; lon: number }[]; // closed polygon, when kind = Bounds
}

interface SpatialScope {
  kind: "Site" | "Area" | "Named" | "Unspecified";
  ref?: string;                    // zone entity id, when kind = Named/Area
  label: string;                   // human readable
}

interface EntityRef {
  kind: string;
  ref?: string;                    // null if Samaritan could not resolve it
  label: string;                   // as the operator said it
}

type Granularity =
  | "Raw" | "PerEvent" | "Shift" | "Daily"
  | { Bucketed: { bucket_size: number } };  // seconds

interface Aggregation {
  op: "Count" | "Sum" | "Mean" | "Median" | "Min" | "Max"
    | "P90" | "P95" | "P99" | "StdDev" | "Rate" | "Histogram";
  field?: string;                  // observable; omitted only for Count
  bins?: number;                   // required when op = Histogram
}

interface Ordering { by: string; direction: "Asc" | "Desc"; }

type Shape = "Scalar" | "Series" | "Set" | "Table" | "Histogram";
```

## 5.1 Field → engine action

The complete mapping. Every field the engine acts on, and how.

| Field | Engine action |
|---|---|
| `subject` | Resolve the record stream in the catalog. Absent → `unavailable(UnavailableSubject)`. |
| `observables` | The measures to return/aggregate. Any absent from the subject → `unavailable(UnavailableObservable)`. |
| `filters` | Row predicates on observables/attributes, combined with AND. |
| `spatial` | Geometric predicates on the record's position/extent, AND. Only valid on spatial subjects. |
| `window` | Select records in `[start, end)`. Absolute UTC — never parsed. |
| `baseline` | Run the identical query over this second window; return alongside. |
| `scope` | Narrow to a zone (§7.3). `Unspecified`/`Site` = whole site. |
| `entities` | Narrow to specific entities (by `ref`; unresolved `ref` = a label the engine may match or ignore). |
| `granularity` | The row grain: raw/per-event, or time-bucketed (Bucketed/Shift/Daily). |
| `group_by` | Partition rows by these attributes before aggregating. |
| `aggregations` | Compute per group over the observables. |
| `ordering` + `limit` | Sort groups by `ordering.by`, take top N. |
| `expected_shape` | Format the result (§6.2). A shape the data cannot fill is a mismatch (§6.3). |

---

# 6. The response: `RequirementResult`

Each requirement gets exactly one result: **data or a typed unavailability.**

```ts
type RequirementResult = {
  requirement_id: string;
} & (
  | { status: "data"; data: DataPayload }
  | { status: "unavailable"; reason: UnavailableReason; detail: string }
);

type UnavailableReason =
  | "UnavailableSubject"       // subject not in the catalog
  | "UnavailableObservable"    // observable not on the subject
  | "UnregisteredTerm"         // a filter/agg/ordering/group_by field is unknown
  | "UnknownZone"              // a Named region references no registered zone
  | "ProjectionUnavailable";   // cannot reconstruct the requested position (§8)
```

`UnavailableReason` deliberately mirrors Samaritan's own `Unserviceable`
reasons — Samaritan pre-validates against the vocabulary, so most of these are
caught before the boundary; the engine raises them for gaps only it knows
(a registered subject whose producing system is not installed at this site,
for example). The two unavailability models compose into one honest picture.

## 6.1 `DataPayload` — the universal row form

Every answer is a set of **rows over named columns**. `shape` tells the
consumer how to read them; the row form itself is uniform.

```ts
interface DataPayload {
  shape: Shape;
  columns: Column[];
  rows: CellValue[][];             // each row aligned to `columns`
  baseline?: { rows: CellValue[][] };  // same columns, for the baseline window
  row_count: number;               // convenience; == rows.length
}

interface Column {
  name: string;                    // deterministic (see below)
  kind: "time" | "dimension" | "measure" | "bin";
  unit?: string;                   // for measures, from the observable
}

type CellValue = number | string | boolean | null | Bin;
interface Bin { lower: number; upper: number; count: number; }  // Histogram only
```

**Column set and order** (deterministic, so responses are comparable):

1. A **time** column first, iff granularity is time-bucketed
   (`Bucketed`/`Shift`/`Daily`) — a bucket-start `Timestamp`.
2. One **dimension** column per `group_by` attribute, in the given order.
3. One **measure** column per `aggregation`, in the given order.

**Measure column naming** is deterministic: `{op}_{field}` lowercased, e.g.
`mean_cycle_time`, `p95_cycle_time`, `sum_available_time`. `Count` with no
field is `count`. This lets the consumer address columns without guessing.

A `null` cell is a legitimate value: the group existed but the measure was
undefined for it (e.g. a mean over zero records). It is **not** the same as an
absent row.

## 6.2 Shapes

`expected_shape` constrains how the rows are produced and read:

- **Series** — rows ordered by the time column ascending; measures per bucket.
  The default for "establish" and "decompose" requests. Requires a
  time-bucketed granularity.
- **Table** — rows with dimension + measure columns; no ordering implied. The
  general case.
- **Set** — rows are entities (one dimension column, an entity id), ordered by
  `ordering` and truncated to `limit`. "The five worst trucks."
- **Scalar** — exactly one row, one measure column. A single number. If the
  query would produce more than one row, that is a mismatch (§6.3).
- **Histogram** — a single `aggregation` with `op: "Histogram"`; the result is
  a `bin` column of `Bin` values. `bins` sets the count.

## 6.3 Shape mismatch

If the query cannot be formed into `expected_shape` — e.g. `Scalar` requested
but the grouping yields many rows, or `Series` requested with a non-bucketed
granularity — the engine returns `data` with the **actual** shape it produced
and sets a `shape_mismatch: true` marker, rather than failing. Samaritan
caught most of these at plan time; the marker lets it notice any that slipped
through without discarding a usable answer.

```ts
interface DataPayload {
  // ... as above, plus:
  shape_mismatch?: boolean;
}
```

---

# 7. Query semantics — precise

The exact evaluation, in order. This is where the nuance lives.

## 7.1 Projection

Reconstruct the world (and the subject's record stream) **as of
`world_version.log_position`**: take the nearest snapshot at or before the
position, replay the log forward to the position, and query the result. No
event after `log_position` is visible.

Because identity corrections (`EntityMerged`, `EntitySplit`) are themselves
events, projecting to position P includes exactly the corrections committed by
P — so a query at P is stable forever, even as later corrections arrive. This
is what makes a pinned investigation defensible months later.

## 7.2 Time selection

`window` is **half-open, `[start, end)`**, both UTC.

- **Instant records** (events like `haul_cycle`, `incident_event`): included
  iff their timestamp ∈ `[start, end)`.
- **Interval records** (period subjects like `equipment_availability`,
  `production_shift`): included iff the interval **starts** within
  `[start, end)`. (Overlap-and-apportion is deliberately *not* the default; see
  §11 open decisions.)

`baseline`, when present, selects a second record set the same way. The engine
runs the identical grouping/aggregation over it and returns it under
`data.baseline.rows`.

## 7.3 Spatial scope and predicates

- `scope.kind = "Unspecified"` or `"Site"`: no spatial narrowing — the whole
  site.
- `scope.kind = "Named"`/`"Area"` with `ref` = a zone entity id: narrow to
  records associated with that zone. For a **spatial** subject, this is
  geometric containment of the record's position in the zone at the record's
  time. For a **non-spatial** subject that carries a location/zone attribute,
  it is an equality filter on that attribute. A `Named` scope on a non-spatial
  subject with no location attribute is a no-op with a `shape_mismatch`-style
  note (or `UnknownZone` if the ref is unregistered).
- `spatial` **predicates** apply only to spatial subjects (Samaritan guarantees
  this — invariant 4). Each is evaluated against the record's position/extent
  at its time; multiple combine with AND. `Named` regions reference a zone
  entity by id; unknown → `unavailable(UnknownZone)`.

## 7.4 Filters

Each `Filter` is a predicate on an observable or attribute of the record.
`In`/`NotIn` take an array `value`; the rest take a scalar. All filters combine
with **AND**. A filter on an unknown field → `unavailable(UnregisteredTerm)`.

## 7.5 Grouping

Rows are partitioned by the tuple of (**time bucket**, if the granularity is
bucketed) × (**each `group_by` attribute value**). `PerEvent`/`Raw` with empty
`group_by` means one row per record (no grouping).

Time bucketing:
- `Bucketed{bucket_size}`: fixed-width buckets of `bucket_size` seconds,
  aligned to `window.start`. A record falls in the bucket containing its time.
- `Shift` / `Daily`: buckets are the site's shifts / operational days. **The
  engine does not own the shift calendar** — bucket boundaries for `Shift`/
  `Daily` are supplied by Samaritan out of band (a calendar the engine is
  configured with, or — cleaner — Samaritan pre-buckets by sending explicit
  `Bucketed` windows). See §11.

## 7.6 Aggregation

For each group, compute each `Aggregation` over the named observable across the
group's records:

- `Count` (no field) — number of records in the group.
- `Sum`, `Mean`, `Median`, `Min`, `Max`, `StdDev` — the obvious reductions over
  the observable's values; `null` values are skipped; a reduction over zero
  values is `null`.
- `P90` / `P95` / `P99` — percentiles; interpolation method is the engine's
  choice but must be **fixed and documented** (determinism).
- `Rate` — records (or summed observable) per unit time over the group's window
  span; the engine must document the unit (per second) and Samaritan scales.
- `Histogram` — a distribution of the observable into `bins` equal-width bins
  spanning the observed range; returned as `Bin[]`.

Empty aggregations list ⇒ return the raw observable values per record (no
reduction), at the requested granularity.

## 7.7 Ordering and limit

Sort groups by `ordering.by` (a column name — a measure like `mean_cycle_time`
or a dimension) in `direction`, then take the first `limit` rows. `limit`
without `ordering` never reaches the engine (Samaritan invariant 5). Ties break
by the dimension columns then time, so ordering is total and stable.

## 7.8 Determinism

Given the same `world_version` and the same requirement, the engine must return
byte-identical `rows` (modulo the documented percentile/rate conventions).
Column order is fixed (§6.1); row order is fixed by ordering or, absent
ordering, by (time, dimensions) ascending. Replay is not optional.

---

# 8. Errors vs unavailability

A sharp line, because it encodes principle 5.

**Transport errors (HTTP 4xx/5xx, whole request fails):**
- Malformed request body.
- `world_version.log_position` **beyond the log head** — the engine cannot
  answer the future. `409 Conflict`.
- `world_version.log_position` **behind the earliest snapshot with no replay
  path** — cannot reconstruct. `410 Gone`, or a per-requirement
  `ProjectionUnavailable` if the batch is otherwise serviceable.
- `registry_version` mismatch severe enough that names cannot be resolved.
  `409`.

**Per-requirement unavailability (HTTP 200, `status: "unavailable"`):**
- The subject/observable/term/zone is not in the catalog (the engine is not
  wired to serve it). This is **data about coverage**, not a failure.

**Never an error, never unavailability — a valid empty answer (HTTP 200,
`status: "data"`, `rows: []`):**
- The subject is served, the query is valid, and **no records matched**. Zero
  rows is a finding. It must be distinguishable from unavailability, and it is:
  `status` differs.

> The whole point: an operator asking "were there restricted-zone breaches
> yesterday?" must be able to tell "no, none" (empty data) from "we weren't
> recording zone containment" (unavailable). The API makes that a type-level
> distinction, not a judgement call.

---

# 9. Non-goals — what Meredith must never do

Stated explicitly so the contract cannot drift into a second brain.

- **Parse relative time.** Windows arrive absolute and UTC. The engine never
  sees "yesterday".
- **Own a shift calendar** (beyond bucket boundaries handed to it, §7.5).
- **Join across subjects.** One requirement, one subject, always.
- **Derive facts at query time.** `time_idle`, `speed`, and every derived
  observable are stored facts from upstream systems; the query path reads them,
  never computes them.
- **Rank across subjects, correlate, or hypothesize.** The engine returns
  numbers; Samaritan decides what they mean.
- **Interpret domain meaning.** `restricted`, `excluded_from_productivity`, and
  every operational word live in Samaritan. The engine knows a zone by its id
  and geometry, never by its policy.

If a proposed engine feature requires any of these, it belongs in Samaritan.

---

# 10. Worked example

The real `RequirementSet` for *"why did efficiency drop yesterday?"* (see
`REQUIREMENTSET.md` §10) becomes this exchange. Two of the seven requirements
shown.

## Request

```json
POST /v1/query
{
  "world_version": { "log_position": 1284662, "as_of": "2026-07-21T09:14:00Z", "snapshot": "snap_01J8XP" },
  "requirements": [
    {
      "id": "req_01J8XQ7K3M_efficiency_0002",
      "subject": "haul_cycle",
      "observables": ["queue_time","spot_time","load_time","haul_time","dump_time","return_time"],
      "filters": [], "spatial": [],
      "window":   { "start": "2026-07-20T05:00:00Z", "end": "2026-07-21T05:00:00Z", "resolved_from": "yesterday", "calendar": "northern_pit@v2", "timezone": "Africa/Lagos" },
      "baseline": { "start": "2026-06-20T05:00:00Z", "end": "2026-07-20T05:00:00Z", "resolved_from": "default baseline", "calendar": "northern_pit@v2", "timezone": "Africa/Lagos" },
      "scope": { "kind": "Unspecified", "label": "entire site" },
      "entities": [],
      "granularity": { "Bucketed": { "bucket_size": 3600.0 } },
      "group_by": [],
      "aggregations": [
        { "op": "Mean", "field": "queue_time" }, { "op": "Mean", "field": "spot_time" },
        { "op": "Mean", "field": "load_time" },  { "op": "Mean", "field": "haul_time" },
        { "op": "Mean", "field": "dump_time" },  { "op": "Mean", "field": "return_time" }
      ],
      "expected_shape": "Series",
      "necessity": "Required"
    },
    {
      "id": "req_01J8XQ7K3M_efficiency_0004",
      "subject": "haul_cycle",
      "observables": ["haul_distance"],
      "filters": [], "spatial": [],
      "window":   { "start": "2026-07-20T05:00:00Z", "end": "2026-07-21T05:00:00Z", "resolved_from": "yesterday", "calendar": "northern_pit@v2", "timezone": "Africa/Lagos" },
      "baseline": { "start": "2026-06-20T05:00:00Z", "end": "2026-07-20T05:00:00Z", "resolved_from": "default baseline", "calendar": "northern_pit@v2", "timezone": "Africa/Lagos" },
      "scope": { "kind": "Unspecified", "label": "entire site" },
      "entities": [],
      "granularity": "Shift",
      "group_by": [],
      "aggregations": [ { "op": "Mean", "field": "haul_distance" } ],
      "expected_shape": "Table",
      "necessity": "Preferred"
    }
  ]
}
```

## Response

```json
{
  "world_version": { "log_position": 1284662, "as_of": "2026-07-21T09:14:00Z", "snapshot": "snap_01J8XP" },
  "results": [
    {
      "requirement_id": "req_01J8XQ7K3M_efficiency_0002",
      "status": "data",
      "data": {
        "shape": "Series",
        "columns": [
          { "name": "bucket", "kind": "time" },
          { "name": "mean_queue_time",  "kind": "measure", "unit": "s" },
          { "name": "mean_spot_time",   "kind": "measure", "unit": "s" },
          { "name": "mean_load_time",   "kind": "measure", "unit": "s" },
          { "name": "mean_haul_time",   "kind": "measure", "unit": "s" },
          { "name": "mean_dump_time",   "kind": "measure", "unit": "s" },
          { "name": "mean_return_time", "kind": "measure", "unit": "s" }
        ],
        "rows": [
          ["2026-07-20T05:00:00Z", 210.4, 33.1, 96.7, 402.0, 51.2, 388.5],
          ["2026-07-20T06:00:00Z", 265.9, 34.0, 95.9, 401.3, 52.0, 390.1]
        ],
        "baseline": {
          "rows": [
            ["2026-06-20T05:00:00Z", 141.2, 32.8, 95.1, 399.8, 50.9, 386.0]
          ]
        },
        "row_count": 2
      }
    },
    {
      "requirement_id": "req_01J8XQ7K3M_efficiency_0004",
      "status": "data",
      "data": {
        "shape": "Table",
        "columns": [ { "name": "mean_haul_distance", "kind": "measure", "unit": "m" } ],
        "rows": [ [3120.0] ],
        "baseline": { "rows": [ [3105.0] ] },
        "row_count": 1
      }
    }
  ]
}
```

Reading it: queue time nearly doubled across the window versus the baseline
(210→266 vs 141) while every other phase held, and haul distance barely moved
— so the loss is queueing, not routing. **Samaritan draws that conclusion; the
engine only supplied the numbers.** That is the boundary working exactly as
intended.

---

# 11. Open decisions — settle before building

Called out so they are chosen deliberately.

1. **Batch vs session (iteration).** v1 is a stateless batch — every
   requirement is `round: 0`, `depends_on: []`. But some investigations need a
   second round keyed on the first answer ("which truck was slowest?" → "what
   happened to *that* truck?"). Samaritan's schema reserves `round` and
   `depends_on` so a session endpoint (`POST /v1/query` returning a session id,
   then follow-up rounds) can be added **without breaking v1**. **Decide now
   whether the engine is batch-only for v1** — it is a much simpler engine if
   so, and the reserved fields mean you are not painted into a corner.

2. **Shift/Daily bucketing ownership.** The engine does not own the shift
   calendar. Two clean options: (a) the engine is configured with the site's
   calendar and buckets `Shift`/`Daily` itself; (b) Samaritan never sends
   `Shift`/`Daily` and instead pre-buckets by issuing explicit `Bucketed`
   windows. Option (b) keeps the engine calendar-free — **recommended.** If
   chosen, `Shift`/`Daily` become Samaritan-internal only and the engine may
   reject them.

3. **Interval-record windowing.** §7.2 defaults to "interval included if it
   starts within the window." Overlap-and-apportion (a period straddling the
   window edge contributes proportionally) is more accurate but much more
   complex. Decide per subject class, or standardise on start-within.

4. **Percentile and rate conventions.** Fix and document the percentile
   interpolation method and the `Rate` time unit. Determinism depends on it.

5. **Response serialization details.** The row form here (§6.1) is proposed,
   not yet ratified with the engine. Column naming, `null` semantics, and the
   `baseline` placement should be confirmed against Meredith's data layer
   before it hardens.

6. **Transport.** HTTP/JSON is the assumed binding. If Samaritan and Meredith
   end up colocated, an in-process binding carrying the same shapes avoids
   serialization entirely. gRPC is the middle option. Choose based on
   deployment, not the contract — the shapes are identical.

7. **Capability freshness.** `capabilities` can change as plugins/systems are
   installed. Decide whether Samaritan fetches it per session, caches it with a
   TTL, or is pushed invalidations. `registry_version` is the coarse guard.

---

# 12. Implementation notes for Meredith

Grounding the contract in Meredith's world model, without over-specifying it.

- **A subject is a record stream over the projected world.** `haul_cycle` and
  the other event subjects are events in the log; `equipment_availability` and
  the period subjects are interval facts produced by plugin systems;
  `entity_track` is an entity's `Transform` history (spatial); `zone_visit` is
  the paired `zone-entered`/`zone-exited` facts. The query layer sees all of
  them as "timestamped records with observables + attributes (+ geometry)."

- **The catalog is the plugin registration, projected.** Meredith's mining
  plugin registers these types (`PLUGINS.md`); `capabilities` is that
  registration exposed to Samaritan. A subject is "wired to serve" iff its
  producing system is installed — which is exactly what makes the
  unavailable/empty distinction real.

- **Derivation happens on the world clock, not the query clock.** `time_idle`,
  `speed`, and zone dwell are emitted as events by plugin systems as the log
  advances, carrying `derived_from` provenance. The query path never derives —
  it reads stored facts. This keeps replay honest: a derived fact reconstructs
  identically at a pinned position.

- **Projection is snapshot + replay.** `world_version.log_position` selects the
  moment; a snapshot at or before it plus forward replay reconstructs the
  facts. This is Meredith's existing model (`WORLD.md`); the query API is a read
  over it, nothing more.

- **Operational meaning stays out.** The engine returns `haul_cycle` records
  and zone geometries. It never returns "efficiency dropped" or "restricted
  zone breached" — those are Samaritan's to compute from the numbers. The
  engine that answers this API is a retrieval-and-reduction surface over a
  domain-agnostic world. That is the whole design.

---

# Guiding principle

> Samaritan asks precisely, over one pinned world, one subject at a time, and
> can tell silence from absence.
>
> Meredith answers precisely, computes only what it holds, and never guesses
> what the answer means.
>
> The `RequirementSet` is the whole of the question. This API is the whole of
> the answer. Everything else — language, intent, causality, meaning — stayed
> on Samaritan's side of the boundary, on purpose.
