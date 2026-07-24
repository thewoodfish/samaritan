# PLANNING.md

> Planning is the first stage of Samaritan.
>
> It transforms a business question into a reproducible InvestigationPlan.
>
> Planning understands **business language**, not operational reality.
>
> It decides **who should investigate**, not **what happened**.

---

# Philosophy

Planning is the bridge between operators and investigations.

An operator thinks in business questions.

The investigation system thinks in structured plans.

Planning performs this translation and nothing else.

Planning never

- accesses the world
- retrieves observations
- reasons over evidence
- decides what data exists
- produces explanations

Planning determines **how an investigation should begin.**

---

# Responsibilities

Planning is responsible for

- validating questions
- normalizing language
- extracting intent
- extracting constraints
- resolving relative time and scope
- identifying operational domains
- deriving investigation strategy
- selecting analyzers
- producing an InvestigationPlan

Planning is **not** responsible for

- knowing what data exists
- deciding what to retrieve
- evidence of any kind

Note the boundary: Planning names *who* investigates. Analyzers decide *what
must be known*. Planning never writes a requirement.

---

# Input

```
Question
```

Planning receives the full `Question` schema, not a bare string. It needs
`site` to resolve scope and `asked_at` to resolve relative time.

Planning must never call `now()`. The clock is an input.

---

# Output

Exactly one of

```
InvestigationPlan        when the question is valid
ParsedQuestion           when it is not, carrying status and reason
```

Planning produces no other outputs.

---

# Planning Pipeline

```
Question
    │
    ▼
1. Question Validation
    │
    ▼
2. Question Normalization
    │
    ▼
3. Intent Extraction
    │
    ▼
4. Constraint Extraction
    │
    ▼
5. Constraint Resolution
    │
    ▼
6. Operational Domain Resolution
    │
    ▼
7. Strategy Derivation
    │
    ▼
8. Analyzer Selection
    │
    ▼
InvestigationPlan
```

Stages 1–4 and 6 are inference.

Stages 5, 7 and 8 are deterministic lookups.

Each stage performs exactly one transformation.

---

# Stage 1 — Question Validation

Purpose

Determine whether the question is suitable for operational investigation.

Output

```
ValidationStatus + reason + missing[]
```

## The four states

**Valid** — proceed.

```
Why did production fall yesterday?
```

**Invalid** — not an operational question. Terminal.

```
What is Bitcoin trading at?
```

**Ambiguous** — operational, but several distinct investigations fit.

```
Show me activity.
```

→ `missing: ["what kind of activity", "time range"]`

**Incomplete** — one clear investigation, but required context absent.

```
Why?
```

→ `missing: ["what outcome to explain"]`

## Rules

Planning never infers missing context.

Planning never guesses a time range. An absent time range on an `Explain`
intent is `Incomplete`, not a silent default to "today".

Planning holds no session. It returns the rejection and stops. The caller
re-asks with a new `Question`.

`missing[]` must be specific enough for the caller to act on without a
second round trip.

---

# Stage 2 — Question Normalization

Purpose

Reduce linguistic variation while preserving meaning.

```
Why is production low?
        ↓
Why did production decrease?
```

```
Our trucks seem slow.
        ↓
Investigate haulage efficiency.
```

## Rules

Normalization never adds information not present in the original.

Normalization never resolves scope or time — that is stages 4 and 5.

The original `Question.text` is preserved verbatim. Normalization writes to
a new field on a new schema.

Normalized output is the cache key input. Two phrasings that normalize
identically must produce byte-identical plans.

---

# Stage 3 — Intent Extraction

Purpose

Determine what the operator wants to accomplish.

```
Explain · Compare · Locate · Recommend · Predict · Summarize
```

Exactly one primary intent.

`Investigate` was removed. It had no behavioural difference from `Explain`
and made classification unstable.

Secondary intents are out of scope for this phase.

The classification must carry a `rationale`. An intent without a stated
reason is not explainable and fails review.

---

# Stage 4 — Constraint Extraction

Purpose

Pull execution constraints out of the question text.

**This stage was missing from the original design.** Without it, nothing
explained how "yesterday" or "Pit 3" reached the plan.

Extracted from text

- time expression
- baseline expression, if the operator named one
- spatial scope
- named entities
- urgency language

Taken from the Question envelope, not the text

- operator
- organization
- site

Supplied by configuration

- max_runtime
- default priority

## Rules

Extraction produces **unresolved** expressions — the verbatim phrase.

`"yesterday"`, `"Pit 3"`, `"truck 14"`

Resolution is stage 5. Keeping them separate means a bad resolution can be
audited against what the operator actually said.

An entity the operator names but Planning cannot identify is passed through
as a label with a null ref. Planning does not know what exists.

---

# Stage 5 — Constraint Resolution

Purpose

Convert every unresolved expression into an absolute, auditable value.

Deterministic. No inference. Pure function of

```
(expression, asked_at, site calendar, site timezone)
```

## Time resolution

```
"yesterday"
    ↓
site → Africa/Lagos, shift calendar site_shift_v1
asked_at → 2026-07-21T09:14:00Z
    ↓
start: 2026-07-20T05:00:00Z
end:   2026-07-21T05:00:00Z
resolved_from: "yesterday"
calendar: site_shift_v1
timezone: Africa/Lagos
```

**"Yesterday" is a shift-calendar question, not a midnight question.** A site
running 06:00–18:00 shifts does not mean midnight-to-midnight by "yesterday",
and getting this wrong silently corrupts every downstream comparison.

Resolution rules live in `REGISTRY.md`.

## Baseline resolution

Some intents need a reference period to be meaningful.

`Explain` a decrease requires something to have decreased *from*.

When the operator names no baseline, the registry supplies the default for
that intent — typically a trailing window of equal or greater length.

The default is recorded in `resolved_from` as `"default baseline"` so it is
never mistaken for something the operator asked for.

## Spatial resolution

`"Pit 3"` resolves to a zone entity id, via the registry's zone table.

A place the operator names that is not in the registry does not resolve.
Samaritan may only reason about places it has been taught the meaning of.

## World version pinning

Planning records **which projection of the world** the investigation reads.

```
log_position: 1_284_662
as_of:        2026-07-21T09:14:00Z
```

Taken once, at plan time, and carried in the constraints from then on.

Every analyzer and every requirement reads the same world. Without this,
two analyzers running in parallel could observe different worlds as events
continue to arrive, and their findings would be quietly incomparable.

It also makes an investigation re-runnable months later against the world as
it stood — not as it has since been corrected.

## Failure

An expression that cannot be resolved does not become a guess.

It returns the question to `Incomplete` with the offending phrase in
`missing[]`.

---

# Stage 6 — Operational Domain Resolution

Purpose

Determine which business domains are affected, in order of relevance.

```
Why did efficiency decrease?
        ↓
1. OperationalPerformance   (0.94)
2. MaterialFlow             (0.81)
3. Equipment                (0.77)
4. Environment              (0.52)
```

Output is a ranked list. Every entry carries a confidence and a rationale.

Rank must be strictly increasing, starting at 1.

Domains below the registry's relevance floor are dropped, not ranked last.
Carrying a domain at 0.11 confidence into analyzer selection wastes an entire
parallel worker.

The domain set is closed and versioned. See `REGISTRY.md`.

---

# Stage 7 — Strategy Derivation

Purpose

Attach the investigation strategy.

**This is a lookup, not an inference.**

```
intent → strategy
```

The mapping is 1:1 and lives in `REGISTRY.md`.

The original design treated strategy as a separate reasoning step, but its
possible outputs were exactly the intent names. A second inference stage
would add non-determinism and produce no new information.

---

# Stage 8 — Analyzer Selection

Purpose

Determine which analyzers participate.

**This is a lookup, not an inference.**

Planning does not know how analyzers work.

Planning knows only which analyzers declare coverage of which domains.

```
OperationalPerformance
        ↓
efficiency · flow · maintenance
```

The mapping is many-to-many and inverted: **analyzers declare the domains
they cover**, in the registry. Planning resolves the reverse index.

This means adding an analyzer requires a registry entry and no change to
Planning.

## Rules

The union of analyzers across all ranked domains is selected, deduplicated.

Each `AnalyzerRef` records which domains selected it, and why.

Selecting zero analyzers is a planning failure, not an empty plan.

---

# Investigation Plan

The contract between Planning and Dispatch.

```yaml
id: plan_01J8XQ7K3M
question_id: q_01J8XQ7A11
question_text: Why did efficiency decrease yesterday?

intent:
  type: Explain
  confidence: 0.96
  rationale: >
    Question asks for the cause of an observed decrease.

domains:
  - domain: OperationalPerformance
    rank: 1
    confidence: 0.94
    rationale: efficiency is the direct subject
  - domain: MaterialFlow
    rank: 2
    confidence: 0.81
    rationale: efficiency losses commonly originate in haul and queue flow
  - domain: Equipment
    rank: 3
    confidence: 0.77
    rationale: availability affects effective efficiency

strategy:
  type: Explain
  goal: identify the causes of the observed change
  expected_output: ranked causal hypotheses

analyzers:
  - name: efficiency
    version: 1.0.0
    domains: [OperationalPerformance]
    rationale: declares coverage of OperationalPerformance
  - name: flow
    version: 1.0.0
    domains: [OperationalPerformance, MaterialFlow]
    rationale: declares coverage of both ranked domains
  - name: maintenance
    version: 1.0.0
    domains: [Equipment]
    rationale: declares coverage of Equipment

constraints:
  time:
    start: 2026-07-20T05:00:00Z
    end: 2026-07-21T05:00:00Z
    resolved_from: yesterday
    calendar: site_shift_v1
    timezone: Africa/Lagos
  baseline:
    start: 2026-06-20T05:00:00Z
    end: 2026-07-20T05:00:00Z
    resolved_from: default baseline
    calendar: site_shift_v1
    timezone: Africa/Lagos
  spatial_scope:
    kind: Unspecified
    label: entire site
  entity_scope: []
  world_version:
    log_position: 1284662
    as_of: 2026-07-21T09:14:00Z
    snapshot: snap_01J8XP
  priority: Normal
  max_runtime: 30s
  operator: op_01J8X0
  organization: org_01J8X0

provenance:
  model_id: claude-opus-4-8
  prompt_template_version: planning/2026-07-01
  registry_version: 1.0.0
  cache_hit: false
```

The InvestigationPlan contains no operational evidence and asserts nothing
about what happened.

---

# Planning Principles

## Business first

Planning understands business language, not implementation.

## World agnostic

Planning has no knowledge of observations, entities, sensors, drones,
cameras or databases.

It names entities the operator mentioned. It does not know whether they
exist.

## Reproducible

Planning uses a language model at temperature 0 with a response cache.

It is reproducible, not deterministic. The distinction is recorded rather
than claimed away.

Pinned: model id, prompt template version, registry version, `asked_at`.

Every plan carries `PlanProvenance` proving which pipeline produced it.

## Explainable

Every planning decision carries a rationale.

The operator can ask why an intent was chosen, why a domain was ranked
where it was, and why an analyzer was selected — and get an answer from the
plan itself, not from logs.

## Stateless

Planning depends on no previous investigation.

Rejection is a return value, never a conversation.

## Extensible

Adding a domain, an analyzer, or an intent is a registry change.

Planning code should not change.

---

# What Planning Never Knows

- what data exists
- what actually happened
- whether an entity is real
- what evidence will be found

These belong to later stages, or to the Reality Engine.

---

# Guiding Principle

Planning transforms business intent into operational intent.

It does not investigate.

It does not reason about the world.

Its sole responsibility is producing a reproducible InvestigationPlan that
defines **who should investigate, and under what constraints**.
