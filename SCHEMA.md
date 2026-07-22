# SCHEMA.md

> Schemas define the language of Samaritan.
>
> Every stage communicates exclusively through schemas.
>
> Schemas are immutable contracts.
>
> They contain no behaviour, no business logic and no implementation detail.

This document is the single source of truth for every contract in this phase.

Where prose elsewhere disagrees with this file, this file wins.

---

# Philosophy

A schema represents a fact.

It does not represent computation.

It simply describes information.

---

# Pipeline

```
Question
    ↓
ParsedQuestion
    ↓
Intent
    ↓
InvestigationConstraints
    ↓
OperationalDomains
    ↓
InvestigationPlan
    ↓
InformationRequirement[]
    ↓
RequirementSet
```

---

# Common Types

Used throughout. Defined once.

```
Id                opaque string, ULID. Prefixed by type: "req_01J…"
Timestamp         RFC 3339, always UTC, always explicit offset
Confidence        float in [0.0, 1.0]
SchemaVersion     semver string, e.g. "1.0.0"
```

## Confidence bands

Confidence is a single scale with documented meaning.

```
0.90 – 1.00   unambiguous
0.70 – 0.89   confident, minor alternative readings exist
0.50 – 0.69   plausible, a competing reading is close
0.00 – 0.49   low — treat as unresolved
```

In this phase confidence is never combined or aggregated.

No confidence algebra is defined, because nothing in this phase merges
independent confidences. That belongs to Synthesis, which is out of scope.

## Envelope

Every top-level schema carries these fields. They are not repeated below.

```
id              : Id
schema_version  : SchemaVersion
created_at      : Timestamp
```

---

# Question

The raw operator request. The root of all provenance.

```
Question
  id             : Id                 # "q_…"
  text           : string             # verbatim, never modified
  asked_at       : Timestamp          # the resolution clock for relative time
  operator       : Id
  organization   : Id
  site           : Id                 # resolves timezone and shift calendar
  locale         : string             # BCP-47, default "en"
```

`text` is never altered. Normalization produces a new field on a new schema.

`asked_at` is an input, not a wall-clock read. Planning must never call
`now()`. This is what makes replay possible.

`site` is required. "Yesterday" is meaningless without it.

---

# ParsedQuestion

Result of validation and normalization.

```
ParsedQuestion
  question_id         : Id
  status              : ValidationStatus
  normalized_question : string?        # present only when status = Valid
  confidence          : Confidence
  language            : string         # BCP-47, detected
  reason              : string?        # required when status ≠ Valid
  missing             : string[]       # what would make it answerable
```

## ValidationStatus

```
Valid        suitable for investigation
Invalid      not an operational question at all
Ambiguous    operational, but could mean several different investigations
Incomplete   operational and unambiguous, but missing required context
```

Four states, not a boolean.

`Invalid` is terminal. `Ambiguous` and `Incomplete` are re-askable, and
`missing` tells the caller what to supply.

Planning holds no session. It rejects and returns. The caller re-asks.

---

# Intent

The operator's objective.

```
Intent
  type        : IntentType
  confidence  : Confidence
  rationale   : string          # why this classification, for explainability
```

## IntentType

```
Explain      why did something happen
Compare      how do two things differ
Locate       where is something / which thing is it
Recommend    what should be done
Predict      what will happen
Summarize    what is the overall state
```

Six types. Exactly one primary intent per question.

`Investigate` was removed — it was indistinguishable from `Explain`.

Secondary intents are not supported in this phase.

---

# InvestigationConstraints

Execution limits and scope. One vocabulary, used everywhere.

```
InvestigationConstraints
  time          : TimeWindow
  baseline      : TimeWindow?        # reference period, when the intent needs one
  spatial_scope : SpatialScope
  entity_scope  : EntityRef[]        # specific named entities, may be empty
  world_version : WorldVersion
  priority      : Priority
  max_runtime   : duration           # deadline for the whole investigation
  operator      : Id
  organization  : Id
```

## WorldVersion

Which projection of the world the investigation was asked against.

```
WorldVersion
  log_position : integer         # event log sequence number
  as_of        : Timestamp       # the moment that position represents
  snapshot     : Id?             # nearest snapshot, if one was used
```

The engine is an append-only event log projected to a moment. An
investigation that does not pin its position is not repeatable — the same
question re-run next week reads a different world.

The subtler reason matters more. The engine records identity corrections as
events (`EntityMerged`, `EntitySplit`), and the log is reinterpreted when it
does. **What "yesterday" contained can change after yesterday has passed.**

Pinning the position makes an investigation reproducible *and* makes
disagreement legible: two investigations of the same window that reach
different conclusions can be shown to have read different worlds, rather
than appearing to be a reasoning failure.

`world_version` participates in the cache key.

## Priority

```
Low | Normal | High | Critical
```

Priority carries no scheduling semantics in this phase. It is recorded and
propagated only.

## SpatialScope

```
SpatialScope
  kind   : Site | Area | Named | Unspecified
  ref    : Id?           # required unless kind = Site or Unspecified
  label  : string        # human readable, e.g. "Pit 3"
```

`Unspecified` is legal and means the whole site. It must be recorded
explicitly rather than left absent, so that "operator did not narrow scope"
is distinguishable from "scope was never considered."

## EntityRef

```
EntityRef
  kind   : string        # from the subject vocabulary in REGISTRY.md
  ref    : Id?           # null when the operator named it but we cannot resolve it
  label  : string        # as the operator said it, e.g. "truck 14"
```

An unresolvable `EntityRef` is not an error in Planning. Planning does not
know what exists. It passes the label through and lets the engine fail later.

---

# TimeWindow

Relative time is resolved once, in Planning, and never again.

```
TimeWindow
  start          : Timestamp        # inclusive, UTC
  end            : Timestamp        # exclusive, UTC
  resolved_from  : string           # verbatim phrase, e.g. "yesterday"
  calendar       : Id               # which shift calendar resolved it
  timezone       : string           # IANA, e.g. "Africa/Lagos"
```

Nothing downstream ever sees a relative expression.

`resolved_from`, `calendar` and `timezone` exist so a resolution can be
audited and disputed. "Yesterday" at a mine is a shift-calendar question,
not a midnight-to-midnight question, and the two answers differ.

Resolution rules live in `REGISTRY.md`.

---

# OperationalDomains

Business areas involved, ranked.

```
OperationalDomains
  domains : RankedDomain[]        # ordered, at least one
```

```
RankedDomain
  domain     : DomainType
  rank       : integer            # 1 = most relevant, strictly increasing
  confidence : Confidence
  rationale  : string
```

A ranked list replaces the earlier `primary` + `secondary[]` split, which
could not express relevance ordering.

## DomainType

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

Domains are a closed set, versioned with the registry.

---

# InvestigationStrategy

How the investigation should proceed.

```
InvestigationStrategy
  type            : IntentType     # 1:1 with intent
  goal            : string
  expected_output : string
```

Strategy is **derived, not inferred.**

It is a registry lookup keyed by intent, not a separate classification step.
Making it a second inference stage would add non-determinism for no
information gain, since the mapping is 1:1.

Defined in `REGISTRY.md`.

---

# InvestigationPlan

The planning contract. Everything downstream begins here.

```
InvestigationPlan
  id             : Id                        # "plan_…"
  question_id    : Id
  question_text  : string                    # normalized
  intent         : Intent
  domains        : OperationalDomains
  strategy       : InvestigationStrategy
  analyzers      : AnalyzerRef[]
  constraints    : InvestigationConstraints
  provenance     : PlanProvenance
```

```
AnalyzerRef
  name       : string          # registry key
  version    : SchemaVersion
  domains    : DomainType[]    # which matched domains selected it
  rationale  : string
```

```
PlanProvenance
  model_id                : string
  prompt_template_version : string
  registry_version        : SchemaVersion
  cache_hit               : bool
```

The InvestigationPlan contains **no operational evidence** and no claim
about what happened.

`PlanProvenance` exists because Planning is reproducible rather than
deterministic. Without it, an identical-looking plan cannot be shown to have
come from an identical pipeline.

---

# InformationRequirement

**The most important schema in this phase.**

One analyzer's declaration of one thing it needs to know.

The union of what these can express *is* the Reality Engine's request API.

```
InformationRequirement
  id             : Id                  # "req_…"
  plan_id        : Id
  requested_by   : string[]            # analyzer names; >1 after dedup
  purpose        : string              # why this is needed, human readable
  necessity      : Necessity

  subject        : string              # subject vocabulary, REGISTRY.md
  observables    : string[]            # observable vocabulary, REGISTRY.md
  filters        : Filter[]
  spatial        : SpatialPredicate[]

  window         : TimeWindow
  baseline       : TimeWindow?

  scope          : SpatialScope
  entities       : EntityRef[]

  granularity    : Granularity
  aggregations   : Aggregation[]
  ordering       : Ordering?
  limit          : integer?
  expected_shape : Shape

  round          : integer             # always 0 in this phase
  depends_on     : Id[]                # reserved for part 2, always empty
```

## Necessity

```
Required     the analyzer cannot proceed without it
Preferred    materially improves the answer
Optional     nice to have
```

Necessity lets the engine degrade under load and lets a partial answer be
interpreted rather than discarded.

## Filter

```
Filter
  field : string        # observable or attribute vocabulary
  op    : Eq | Neq | In | NotIn | Gt | Gte | Lt | Lte
  value : scalar | scalar[]
```

Filters are declarative predicates, not a query language.

## SpatialPredicate

Geometric narrowing. Separate from `Filter` because the operand is geometry,
not a scalar.

```
SpatialPredicate
  op     : Within | Intersects | Contains | Outside
  region : RegionRef
```

```
RegionRef
  kind   : Named | Radius | Bounds
  ref    : Id?            # required when kind = Named — a zone entity id
  center : Point?         # required when kind = Radius
  radius : distance?      # required when kind = Radius
  bounds : Point[]?       # required when kind = Bounds — closed polygon
```

A `Named` region references a **zone entity in the engine**, by id.

Samaritan never carries zone geometry. The engine owns where a zone is;
Samaritan's registry owns only what a zone means operationally.

```
Point
  lat : float
  lon : float
```

Multiple predicates combine with AND.

`Within` and `Outside` test containment of the subject's position.
`Intersects` tests overlap of its extent or path with the region.

A requirement using a `Named` region not present in the registry is invalid
and Dispatch rejects it, exactly as with an unregistered observable.

## Granularity

```
Raw          every individual record or event
PerEvent     one row per occurrence of the subject
Bucketed     fixed interval — requires bucket_size
Shift        one row per shift
Daily        one row per operational day
```

```
Bucketed additionally carries
  bucket_size : duration
```

## Aggregation

An aggregation binds an operation to a specific field.

```
Aggregation
  op    : AggregationOp
  field : string           # an observable of the subject; omitted for Count
  bins  : integer?         # required when op = Histogram
```

```
AggregationOp
  Count | Sum | Mean | Median | Min | Max
  P90 | P95 | P99 | StdDev | Rate | Histogram
```

Binding the field to the operation lets one requirement ask for
`Mean(speed)` and `Max(queue_length)` together, which a bare list of
operations could not express.

Empty means no aggregation — return the underlying records.

The engine is expected to compute these. See Aggregation Placement below.

## Ordering

Rank the result and optionally take only the top of it.

```
Ordering
  by        : string              # an observable, or an aggregation of one
  direction : Asc | Desc
```

Paired with `limit`, this expresses "the five worst trucks by mean cycle
time" as a single request.

Without it, an analyzer wanting the worst performer must ask for every
entity and sort the result itself — pulling a large payload to discard
almost all of it.

`ordering` without `limit` is legal and means "return everything, ranked."

`limit` without `ordering` is invalid — an arbitrary subset is never a
meaningful answer.

## Shape

What the analyzer expects back.

```
Scalar     one value
Series     values over time
Set        a collection of entities
Table      rows with named columns
Histogram  binned distribution
```

`Shape` is a declaration of expectation, not a rendering instruction. It
exists so a mismatch between what was asked and what the engine can return
is caught at the boundary rather than deep in analysis.

## Rules

A requirement never names a data source, table, sensor, camera, or database.

A requirement describes **what is wanted**, never **where it lives**.

`subject`, `observables`, `filters[].field`, `aggregations[].field`,
`ordering.by` and named regions must all draw from the registry vocabulary.
A requirement using an unregistered term is invalid and must be rejected by
Dispatch, not passed on.

## Observed and derived observables

An observable may be directly observed or derived from other facts. The
requirement does not distinguish them, and must not.

```
speed              derived from position over time
distance_travelled derived from a path
dwell_time         derived from paired zone entry and exit
time_idle          derived from a state definition
```

**Derivation does not happen at query time.**

The engine is event-sourced. Derived facts are produced upstream by domain
systems as the world advances, emitted as events, and stored as world state
carrying `derived_from` provenance back to the facts that caused them.

By the time a requirement asks for `time_idle`, it is a stored fact with a
traceable origin — not a computation performed on the way out.

Three consequences.

**Derived facts replay.** Rebuilding the world reproduces them identically.
A derived fact is exactly as durable and exactly as auditable as an observed
one.

**Derived facts are evidence.** Because they carry `derived_from`, an
analyzer can trace `time_idle` back to the position fixes that produced it.
A query-time computation would arrive with no history and could not be
questioned.

**The definition is Samaritan's, not the engine's.** The engine does not
know what idle means. The domain layer defines it, and that definition
belongs in `REGISTRY.md` — written once, so two analyzers can never mean
different things by the same word.

---

# RequirementSet

The terminal artifact of this phase.

```
RequirementSet
  id            : Id                        # "reqset_…"
  plan_id       : Id
  question_id   : Id
  world_version : WorldVersion              # every requirement reads this world
  requirements  : InformationRequirement[]  # deduplicated, normalized order
  unserviceable : Unserviceable[]
  execution     : ExecutionReport
  complete      : bool
```

```
Unserviceable
  requirement_id : Id
  requested_by   : string[]
  necessity      : Necessity
  reason         : UnavailableSubject | UnavailableObservable
                 | UnregisteredTerm | UnknownZone
  detail         : string
```

`complete` is true only when every analyzer completed **and** every
requirement is serviceable.

An unserviceable `Required` requirement fails the investigation. It must
never be dropped and left to look like a question nobody needed to ask.

**Unavailable is not empty.** See `ENGINE.md`.

```
ExecutionReport
  completed : AnalyzerOutcome[]
  empty     : AnalyzerOutcome[]     # ran, legitimately needed nothing
  failed    : AnalyzerOutcome[]
  timed_out : AnalyzerOutcome[]
```

```
AnalyzerOutcome
  analyzer      : string
  version       : SchemaVersion
  duration      : duration
  requirement_count : integer
  error         : string?          # required when failed
```

`empty`, `failed` and `timed_out` are three distinct outcomes and are never
collapsed into one.

`complete = false` must be surfaced to the operator. A partial RequirementSet
is valid, but never presented as whole.

## Deduplication

Two requirements are duplicates when all of the following match exactly

```
subject · observables · filters · window · baseline ·
scope · entities · granularity · aggregations · expected_shape
```

On merge

- keep the earliest `id`
- union `requested_by`
- concatenate `purpose` from each requester
- take the strongest `necessity` (Required > Preferred > Optional)

Deduplication never alters semantic content. Near-matches are not merged.

---

# Schema Ownership

Planning produces

- ParsedQuestion
- Intent
- OperationalDomains
- InvestigationConstraints
- InvestigationStrategy
- InvestigationPlan

Analyzers produce

- InformationRequirement

Dispatch produces

- RequirementSet

---

# Design Principles

## Immutable

Schemas are never modified. Each stage creates a new schema.

## Reproducible

Given identical input and identical pinned configuration, a stage produces
identical output. See Determinism in `PIPELINE.md`.

## Minimal

Schemas carry only what the next stage needs — plus provenance, which is
never optional.

## Implementation independent

Schemas never reference databases, ECS, cameras, rendering, networking or
storage.

Schemas describe meaning, never implementation.

## Versioned

Every top-level schema carries `schema_version`.

Additive change increments minor. Field removal or semantic change
increments major and requires a migration note.

---

# Aggregation Placement — SETTLED

Computation happens in three places, and they are not the same place.

## 1. Derivation — upstream, as the world advances

Domain systems turn facts into other facts.

```
position fixes        →  speed, distance_travelled
position + zone       →  zone entered, zone dwell
speed + threshold     →  idle period
```

Event-sourced, replayable, provenance-carrying. Runs on the world clock,
before any question is asked.

## 2. Reduction — at query time, in the engine

The engine reduces stored facts to answers.

```
Mean · Median · Min · Max · Sum · Count · StdDev · P90/95/99 · Rate
Histogram · ordering · limit
spatial containment and intersection
```

Cheap, mechanical, no domain knowledge required. Samaritan asks for
`Mean(speed)` and receives a number rather than pulling every fix.

## 3. Reasoning — in Samaritan

Correlation across subjects, hypothesis formation, confidence, causal
explanation. Anything requiring a model or an opinion.

Out of scope for this phase, but the boundary is drawn now.

---

The dividing lines

**The engine derives facts from facts, and reduces facts to numbers.**

**Samaritan decides which facts are worth deriving, and what the numbers
mean.**

The engine never learns what "idle" or "restricted" means. It is told which
derivation to run, by a domain layer that knows.

---

# Guiding Principle

Schemas are the shared language of Samaritan.

Changing a schema changes the language of the entire platform.

They should evolve deliberately and remain simple, explicit and stable.
