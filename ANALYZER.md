# ANALYZER.md

> An analyzer is a domain expert.
>
> In this phase it has exactly one responsibility.
>
> Given an InvestigationPlan, declare **what must be known** to investigate
> its own domain.
>
> It does not retrieve. It does not reason. It does not conclude.

---

# Scope Of This Phase

An analyzer's eventual lifecycle is

```
determine requirements → retrieve evidence → interpret → hypothesize → recommend
```

**Only the first step is in scope.**

The remaining four are deferred until the Reality Engine exists. The analyzer
contract is written so they can be added without changing this one.

An analyzer in this phase is a pure function.

```
InvestigationPlan → InformationRequirement[]
```

No side effects. No I/O. No world access.

---

# Philosophy

Planning knows business language but not operational reality.

The Reality Engine will know operational reality but not intent.

The analyzer is the only component that knows both: it knows what a question
about *its domain* actually requires in order to be answerable.

That translation is domain expertise, and it is the whole reason analyzers
exist as separate components rather than as a single planner.

---

# The Contract

Every analyzer implements the same interface.

```
Input       InvestigationPlan          (immutable, complete)
Output      InformationRequirement[]   (possibly empty)
Deadline    plan.constraints.max_runtime
```

Every analyzer receives the **full** plan, not a filtered slice. An analyzer
decides for itself what is relevant to its domain.

---

# Declaration

An analyzer declares itself in the registry. It is not discovered.

```yaml
name: efficiency
version: 1.0.0

domains:
  - OperationalPerformance
  - MaterialFlow

intents:
  - Explain
  - Compare
  - Summarize

subjects:
  - haul_cycle
  - loading_event
  - production_shift

description: >
  Investigates operational efficiency: cycle performance, utilization,
  and the gap between planned and achieved throughput.
```

`domains` drives Planning's analyzer selection.

`intents` lets an analyzer decline intents it cannot serve. An analyzer
selected for an intent it does not declare returns empty, not an error.

`subjects` bounds what it may request. A requirement naming a subject outside
this list is a contract violation and Dispatch rejects it.

---

# Requirement Generation

## Rules

**One requirement, one question about the world.**

Do not bundle unrelated observables into one requirement because they happen
to share a time window. Deduplication and necessity both operate per
requirement, and bundling defeats them.

**Every requirement states its purpose.**

`purpose` is prose, addressed to a human reviewing why this was asked. A
requirement whose purpose cannot be stated in one sentence is probably two
requirements.

**Necessity must be honest.**

Marking everything `Required` makes degradation impossible. An analyzer that
cannot distinguish what it truly needs from what would merely help has not
finished thinking.

**Ask for the ranking, not the whole population.**

When the domain question is "which one is worst", say so.

```yaml
aggregations: [{ op: Mean, field: cycle_time }]
ordering: { by: cycle_time, direction: Desc }
limit: 5
expected_shape: Set
```

Requesting every entity in order to sort it locally pulls a large payload to
discard almost all of it.

**Never name a source.**

No tables, sensors, cameras, feeds, or systems. Describe what is wanted.
Where it lives is the engine's problem.

**Use only registry vocabulary.**

`subject`, `observables`, and filter fields come from the controlled
vocabulary. Inventing a term is a violation, not an extension — extend the
registry instead.

**Respect the plan's constraints.**

An analyzer may narrow scope or window when its domain requires it, and must
record why in `purpose`. It may never widen beyond the plan.

Baseline is the exception: an analyzer may request a baseline the plan did
not specify, because whether a comparison is needed is domain knowledge.

## Returning nothing

Returning zero requirements is a legitimate outcome, not a failure.

An analyzer selected by domain overlap may correctly conclude the question
does not concern it.

Dispatch records this as `empty`, distinct from `failed`.

---

# Example

Plan

```
intent:      Explain
question:    Why did efficiency decrease yesterday?
domains:     OperationalPerformance, MaterialFlow, Equipment
window:      2026-07-20T05:00Z → 2026-07-21T05:00Z
baseline:    2026-06-20T05:00Z → 2026-07-20T05:00Z
scope:       entire site
```

The efficiency analyzer reasons, in its own domain terms:

*To explain an efficiency decrease I must first establish that one occurred,
then decompose the cycle to find which component grew.*

Requirements

```yaml
- id: req_01J8XR0001
  plan_id: plan_01J8XQ7K3M
  requested_by: [efficiency]
  purpose: >
    Establish whether haul cycle time actually degraded in the window
    relative to the trailing baseline.
  necessity: Required

  subject: haul_cycle
  observables: [cycle_time, payload_mass]
  filters:
    - field: equipment_class
      op: Eq
      value: haul_truck

  window:   { start: 2026-07-20T05:00:00Z, end: 2026-07-21T05:00:00Z, … }
  baseline: { start: 2026-06-20T05:00:00Z, end: 2026-07-20T05:00:00Z, … }

  scope: { kind: Unspecified, label: entire site }
  entities: []

  granularity: PerEvent
  aggregations:
    - { op: Mean,  field: cycle_time }
    - { op: P95,   field: cycle_time }
    - { op: Count }
  expected_shape: Series

  round: 0
  depends_on: []

- id: req_01J8XR0002
  plan_id: plan_01J8XQ7K3M
  requested_by: [efficiency]
  purpose: >
    Decompose cycle time into its phases to locate which phase grew.
  necessity: Required

  subject: haul_cycle
  observables: [queue_time, spot_time, load_time, haul_time, dump_time,
                return_time]
  filters: []

  window:   { … }
  baseline: { … }

  scope: { kind: Unspecified, label: entire site }
  entities: []

  granularity: Bucketed
  bucket_size: 1h
  aggregations:
    - { op: Mean, field: queue_time }
    - { op: Mean, field: spot_time }
    - { op: Mean, field: load_time }
    - { op: Mean, field: haul_time }
    - { op: Mean, field: dump_time }
    - { op: Mean, field: return_time }
  expected_shape: Series

  round: 0
  depends_on: []

- id: req_01J8XR0003
  plan_id: plan_01J8XQ7K3M
  requested_by: [efficiency]
  purpose: >
    Rule out reduced fleet availability as the explanation before
    attributing the loss to cycle performance.
  necessity: Preferred

  subject: equipment_availability
  observables: [available_time, scheduled_time, operating_time, utilization]
  filters:
    - field: equipment_class
      op: In
      value: [equipment.haul_truck, equipment.excavator]

  window:   { … }
  baseline: { … }

  scope: { kind: Unspecified, label: entire site }
  entities: []

  granularity: Shift
  aggregations:
    - { op: Sum,  field: available_time }
    - { op: Sum,  field: scheduled_time }
    - { op: Mean, field: utilization }
  expected_shape: Table

  round: 0
  depends_on: []
```

Note that `req_01J8XR0003` is likely also requested by the maintenance
analyzer. Dispatch will merge them and record `requested_by: [efficiency,
maintenance]`.

That overlap is expected and healthy — it is evidence that two independent
domain experts agree the fact matters.

---

# What An Analyzer Must Not Do

- retrieve data
- call any external system
- assume an entity exists
- assume an observable is available
- communicate with another analyzer
- re-interpret the question
- override the plan's intent or domains
- widen the plan's scope or window
- produce hypotheses, conclusions, or recommendations

The last is worth stating plainly: **an analyzer in this phase forms no
opinion about what happened.** It has no evidence to form one from.

---

# Isolation

Analyzers run in parallel and never interact.

Overlapping requirements are resolved by Dispatch's deduplication, not by
analyzers coordinating.

This is intentional. Coordination would make analyzer output depend on
execution order, which would break reproducibility.

The cost is redundant thinking. The benefit is that any analyzer can be
added, removed, or reordered with no effect on any other.

---

# Failure And Deadlines

An analyzer that exceeds `max_runtime` is abandoned, not awaited.

Its partial output is discarded — a half-formed requirement list is worse
than none, because it looks complete.

Dispatch records the outcome as `timed_out` and sets
`RequirementSet.complete = false`.

An analyzer that throws is recorded as `failed` with its error message.

Neither failure prevents the RequirementSet from being produced.

---

# Implementation Note

An analyzer's requirement generation may be rule-based, model-based, or a
mix. The contract does not care.

If model-based, the same reproducibility rules apply as for Planning:
temperature 0, pinned model, pinned prompt template, response cache keyed on
the plan's content hash.

An analyzer's version must change whenever its requirement-generation
behaviour changes, so that a cached RequirementSet can be invalidated.

---

# Guiding Principle

Planning decides **who investigates.**

The analyzer decides **what must be known.**

Nothing in this phase decides what is true.
