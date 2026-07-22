# PIPELINE.md

> Samaritan is an operational intelligence pipeline.
>
> Intelligence is not one system.
>
> It is a sequence of transformations.
>
> Every stage has one responsibility.
>
> Every stage accepts one bounded schema.
>
> Every stage produces one bounded schema.

---

# Scope Of This Phase

Samaritan's full lifecycle is

```
Question → Requirements → Reality Engine → Evidence → Analysis → Understanding
```

**This phase builds only the left half.**

We stop at the boundary of the Reality Engine.

We build everything that decides **what we need to know**.

We build nothing that consumes an answer.

The reason is deliberate: the request surface Samaritan needs should be
derived from what it actually asks, not guessed in advance.

The engine is domain agnostic and stays that way. It stores reality; it
never learns what mining means. That boundary — and the risks it carries —
is specified in `ENGINE.md`.

## In Scope

- Planning
- Dispatch
- Requirement Generation
- The vocabulary of what can be asked

## Out Of Scope

Deferred until the request side is settled.

- Evidence
- Hypotheses
- AnalyzerResult
- Correlation and confidence merging
- InvestigationResult
- OperationalStory
- Recommendation
- VisualizationRequest
- Synthesis in its entirety

These are not cancelled. They are sequenced.

---

# Philosophy

Samaritan transforms uncertainty into operational understanding.

It does this through a series of independent stages.

Each stage transforms one representation into another.

No stage performs work belonging to another.

---

# Architecture

```
                    PLANNING
────────────────────────────────────────

Business Question
        │
        ▼
Question Validation
        │
        ▼
Question Normalization
        │
        ▼
Intent Extraction
        │
        ▼
Constraint Extraction
        │
        ▼
Constraint Resolution
        │
        ▼
Operational Domain Resolution
        │
        ▼
Strategy Derivation
        │
        ▼
Analyzer Selection
        │
        ▼
InvestigationPlan

────────────────────────────────────────
                 DISPATCH
────────────────────────────────────────

InvestigationPlan
        │
        ▼
Instantiate selected analyzers
        │
        ▼
Fan out (parallel, deadline-bounded)

────────────────────────────────────────
          REQUIREMENT GENERATION
────────────────────────────────────────

Efficiency      Flow       Maintenance
 Analyzer     Analyzer      Analyzer

     │            │              │
     ▼            ▼              ▼

InformationRequirement[]  per analyzer

     └────────────┬──────────────┘
                  ▼
────────────────────────────────────────
                 COLLECT
────────────────────────────────────────

Deduplicate · Merge · Record provenance
                  │
                  ▼
            RequirementSet
                  │
                  ▼
        ═══ BOUNDARY ═══
         Reality Engine
       (domain agnostic)
```

---

# Stage 1 — Planning

Purpose

Transform business language into a reproducible InvestigationPlan.

Planning understands

- business language
- intents
- operational domains
- analyzers
- constraints

Planning never understands

- what data exists
- what happened
- evidence

Output

```
InvestigationPlan
```

Specified in `PLANNING.md`.

---

# Stage 2 — Dispatch

Purpose

Fan out the InvestigationPlan and collect what comes back.

Dispatch is orchestration only.

Responsibilities

- instantiate the analyzers named in the plan
- provide each analyzer the full InvestigationPlan
- execute analyzers in parallel
- enforce the plan's `max_runtime` deadline
- collect InformationRequirement lists
- deduplicate and merge into a RequirementSet
- record which analyzers failed or timed out

Dispatch performs no reasoning.

Dispatch never modifies the semantic content of a requirement.

Dispatch may merge two identical requirements into one, but must preserve
every requesting analyzer in `requested_by`.

Output

```
RequirementSet
```

---

# Stage 3 — Requirement Generation

Purpose

Let each domain expert declare what it needs to know.

Every analyzer receives the same InvestigationPlan.

Every analyzer operates independently.

Analyzers never communicate directly.

Analyzers never depend on one another.

In this phase an analyzer has exactly one responsibility:

**translate an InvestigationPlan into the information it would need to
investigate its own domain.**

Every analyzer returns the same schema.

```
InformationRequirement[]
```

Specified in `ANALYZER.md`.

---

# The Terminal Artifact

`RequirementSet` is the output of this phase.

It is also the specification of the Reality Engine's request API.

Whatever a RequirementSet can express, the engine must be able to answer.

Whatever it cannot express, the engine does not need to support.

This is the whole point of stopping here.

---

# Pipeline Objects

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

Every stage accepts exactly one schema.

Every stage produces exactly one schema.

All defined in `SCHEMA.md`.

---

# Concurrency Model

Planning

Single threaded.

Stages execute in order.

---

Dispatch

Single threaded coordinator.

Spawns one worker per selected analyzer.

---

Requirement Generation

Fully parallel.

No analyzer blocks another.

No analyzer depends on another.

An analyzer that exceeds the deadline is abandoned, not awaited.

---

Collection

Single threaded.

Merges completed requirement lists into one RequirementSet.

Ordering is normalized before output — see Determinism.

---

# Ownership

Planning owns

- language
- intent
- constraints
- operational domains
- analyzer selection

Dispatch owns

- orchestration
- lifecycle
- deadlines
- deduplication

Analyzers own

- domain expertise
- knowing what their domain needs to be understood

No responsibility overlaps.

---

# Failure Model

Failure is expected and must be representable.

## Planning failure

Planning rejects the question.

It produces a ParsedQuestion with a non-valid status and a reason.

No InvestigationPlan is produced.

The caller re-asks. Planning holds no session.

## Analyzer failure

An analyzer may fail, time out, or legitimately return zero requirements.

These are three different outcomes and are recorded separately.

The RequirementSet always reports

- which analyzers completed
- which produced nothing
- which failed, and why
- which exceeded the deadline

## Unserviceable requirements

A requirement may be well-formed and still impossible to answer — an
observable the engine has no source for, a subject the domain layer never
registered.

Dispatch validates every requirement before the set is emitted, and records
the ones that cannot be served.

**Unavailable is not empty.** This distinction is load-bearing. Reporting
"no restricted-zone breaches occurred" when the truth is "nothing was
watching for them" is the worst failure this system can produce — confident,
wrong, and unfalsifiable.

A `Required` requirement that cannot be served fails the investigation. It
never degrades into a quiet negative finding.

See `ENGINE.md`.

## Partial success

A RequirementSet with some analyzers failed is still a valid RequirementSet.

It is never silently presented as complete.

---

# Pipeline Rules

## Every stage has one responsibility

Never mix planning with requirement generation.

Never let an analyzer re-interpret the question.

---

## Schemas are immutable

Every stage receives immutable input.

Every stage produces a new output schema.

Stages never mutate the output of a previous stage.

---

## Parallel by design

Adding another analyzer must not require architectural change.

Only a registry entry.

---

## No hidden communication

Analyzers never communicate directly.

All communication flows through the pipeline.

---

## Everything is explainable

Every requirement must be traceable.

```
InformationRequirement
        ↓
   requested_by → Analyzer
        ↓
   plan_id → InvestigationPlan
        ↓
   domain + intent + constraint that motivated it
        ↓
   question_id → Question
```

Every object carries an id.

Every derived object names its parent.

Nothing appears without provenance.

---

# Determinism

Planning uses a language model. Planning is therefore **reproducible**, not
deterministic in the strict sense.

The distinction is recorded honestly rather than claimed away.

Reproducibility is guaranteed when all of the following are pinned

- model id
- temperature 0
- prompt template version
- registry version
- the resolution clock (`asked_at`)

Given identical values for all five, Planning must return an identical
InvestigationPlan, served from cache.

Cache key

```
sha256(
  model_id ‖ prompt_template_version ‖ registry_version ‖
  normalized_question ‖ resolved_constraints
)
```

Ordering is normalized before any output is emitted.

- domains sorted by rank, then name
- analyzers sorted by name
- requirements sorted by (subject, analyzer, id)

Parallel execution must never change output ordering.

---

# Open Decisions

Recorded for vetting, not yet settled.

## 1. One-shot vs. iterative requirements — DEFERRED TO PART 2

Real investigation often narrows: "which truck was slowest?" then "what
happened to that truck?" The second requirement cannot be written without
the first answer.

Whether Samaritan supports that shapes the engine profoundly — a stateless
batch API versus a session API with rounds.

**It does not affect this phase.**

Iteration begins only when an answer returns, and no answer returns in part
1. The first round of requirements is identical under both designs.

This phase therefore produces round 0 and stops. The schema reserves `round`
and `depends_on` so rounds can be added later without a breaking change.

Revisit when analysis is designed.

## 2. The `Investigate` intent

Seven intents were listed originally, but `Investigate` and `Explain` have
no clear behavioural difference and will be confused by any classifier.

This spec drops `Investigate` and keeps six.

## 3. Aggregation placement — SETTLED

The Reality Engine computes.

Requirements may ask for statistical reductions, derived durations and
distances, spatial containment, and binning. The engine returns answers, not
raw records to be reduced locally.

The line is drawn at inference: the engine computes over what it holds,
Samaritan reasons about what it means.

Recorded in `SCHEMA.md`.

---

# Guiding Principle

Planning determines **who should investigate.**

Dispatch determines **who runs.**

Analyzers determine **what must be known.**

The Reality Engine — later — determines **what actually happened.**

This phase ends the moment we know what to ask.
