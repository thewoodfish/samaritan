# BUILD_PLAN.md

> The order we build part 1 in, and how we know each stage is done.
>
> The sequence front-loads risk. The stages with no open questions come
> first; the stage that tests the whole thesis comes as early as its
> dependencies allow; the model-dependent work comes last, because it is the
> least predictable.
>
> Spec is the authority on *what*. This is the authority on *when*, and on
> *what proves each piece works*.

---

# Principles

**The registry loader is the spec, executable.** Every rule in `SCHEMA.md`
and `REGISTRY.md` becomes a check that fails loudly. If the spec and the code
disagree, the loader is where we find out.

**Nothing merges without its exit criteria met.** Each stage below lists
verifiable exit criteria. They are the definition of done, not a suggestion.

**Reproducibility is a property we test, not assume.** Any stage touching a
model ships with a determinism harness in the same PR, never later.

**Unavailable is never empty.** Every stage that can fail to produce
something must distinguish "nothing needed" from "could not serve." This is
checked at every layer, per `ENGINE.md`.

**We stop at the RequirementSet.** No stage in this plan consumes an answer
from the engine. If a stage needs one, it belongs to part 2.

---

# Stage Map

```
0  Scaffold          repo, crates, CI, no logic
1  Schema            every contract as a Rust type          ← no open questions
2  Registry          load + validate the whole registry     ← no open questions
3  Graph             the relation graph as a walkable model  ← no open questions
4  Resolution        constraints, time, world-version        ← deterministic
5  Graph walk        one analyzer, Explain, end to end       ← THE THESIS TEST
6  Planning/model    validate, normalize, intent, domains    ← reproducibility
7  Dispatch          fan out, dedupe, RequirementSet         ← concurrency
8  Breadth           all analyzers, all six intents          ← coverage
9  Reproducibility   caching, provenance, replay             ← hardening
```

Stages 1–4 have no unresolved decisions and can begin immediately.

Stage 5 is the point of the whole exercise. Everything before it exists to
make it possible; everything after it assumes it succeeded.

---

# Stage 0 — Scaffold

**Goal.** A workspace that builds and tests nothing, correctly.

Build

- Cargo workspace, one crate per boundary:
  `samaritan-schema`, `samaritan-registry`, `samaritan-graph`,
  `samaritan-planning`, `samaritan-dispatch`, and a `samaritan` binary
- CI: `cargo build`, `cargo test`, `cargo clippy -D warnings`, `cargo fmt
  --check`
- The purity discipline from Meredith, adapted: a CI check that no schema or
  registry crate depends on the model or dispatch crates. The boundary is
  enforced by the build, not by review.

Exit criteria

- `cargo test` runs green on an empty suite
- CI passes on a no-op PR
- the dependency-direction check fails when deliberately violated

De-risks. Nothing yet — this is the floor.

---

# Stage 1 — Schema

**Goal.** Every contract in `SCHEMA.md` as a Rust type. No behaviour.

Build

- one type per schema: `Question`, `ParsedQuestion`, `Intent`,
  `InvestigationConstraints`, `TimeWindow`, `WorldVersion`,
  `OperationalDomains`, `InvestigationStrategy`, `InvestigationPlan`,
  `InformationRequirement`, `RequirementSet`, and every enum and sub-struct
- serde on all of them; the wire format round-trips
- the envelope (`id`, `schema_version`, `created_at`) on every top-level type
- ULID id types, prefixed per schema (`req_`, `plan_`, …)
- newtypes for units — `Seconds`, `Kilograms`, `Ratio` — so a duration can
  never be assigned a mass. The unit discipline from `REGISTRY.md` becomes a
  type error, not a convention.

Exit criteria

- every schema in `SCHEMA.md` has a corresponding type, checked off one by one
- a hand-written example of each round-trips through serde unchanged
- `Ratio` rejects values outside 0–1 at construction
- no crate outside `samaritan-schema` defines a schema type

De-risks. Turns the spec from prose into something the compiler enforces.

Open questions blocking this stage: **none.**

---

# Stage 2 — Registry

**Goal.** Load the registry and reject an invalid one, with every check in
`REGISTRY.md`.

Build

- parse the registry from its config file (YAML)
- the full validation suite: **E01–E23 and W01–W09**, each a named check
  with a test that trips it
- the derived reverse index (domain → analyzers)
- the covering-calendar selector, with rejection of a window that spans a
  calendar change
- the model config and threshold blocks

Exit criteria

- the mining registry we wrote loads clean
- **every error code has a fixture that triggers exactly it** — 23 negative
  tests
- every warning code has a fixture — 9 more
- a window spanning a calendar boundary is rejected, naming both versions
- the reverse index matches the one written by hand in `REGISTRY.md`

De-risks. The registry is where drift between spec and reality surfaces
first. If GRAPH.md and REGISTRY.md have diverged, these tests find it.

Open questions blocking this stage: **none** — but see the note on enum
members below; the loader works regardless of whether the members are the
*right* ones.

---

# Stage 3 — Graph

**Goal.** The relation graph as a structure a walk can traverse.

Build

- load `decomposes` (with `mode`), `partitions`, `confounds`, `influences`,
  `rolls_up` into a typed graph
- validation specific to the graph: E18–E23, W07–W09
- traversal primitives: neighbours of a node by edge kind, depth-bounded
  walk that never revisits a node within one walk
- the window-widening rule: following an `influences` edge backwards widens
  the window by `lag + persistence`

Exit criteria

- the mining graph loads and every qualified reference resolves (the check
  the validation script already runs)
- a multiplicative decomposition is distinguishable from an additive one in
  the type, not a string
- a depth-bounded walk from `cycle_time` terminates and visits the expected
  nodes
- following rainfall → ground → speed backwards widens the window by exactly
  the persistence sum

De-risks. Proves the graph is walkable *before* an analyzer depends on it.

Open questions blocking this stage: **none.**

---

# Stage 4 — Resolution

**Goal.** The deterministic half of Planning. No model yet.

Build

- constraint extraction's output types (the model fills them in stage 6; here
  we accept them pre-filled)
- time resolution: relative expression + `asked_at` + calendar → absolute
  `TimeWindow`, against the covering calendar version
- baseline defaults per intent, truncated at calendar boundaries
- spatial resolution: zone key → entity id
- world-version pinning
- strategy derivation (lookup) and analyzer selection (reverse index)
- the threshold precedence order for domain and analyzer culling

Exit criteria

- "yesterday" at the sample site resolves to the exact shift-aligned window
  in `PLANNING.md`, **not** midnight
- a window resolved against a historical date picks the calendar version in
  force *then*, not now
- an unresolvable expression returns `Incomplete` with the phrase, never a
  guess
- given a filled-in intent and domains, a complete `InvestigationPlan` is
  assembled deterministically — same input, byte-identical plan

De-risks. Everything here is pure and testable without a model. Getting it
solid means stage 6 only has to add the model, not the plumbing.

Open questions blocking this stage: **`time_idle`'s definition** is not
needed to *resolve* constraints, but is needed before any efficiency number
is trusted. Resolve before stage 5's results are read as real.

---

# Stage 5 — Graph Walk · THE THESIS TEST

**Goal.** One analyzer, one intent, walking the graph, producing real
requirements. This is the stage the whole architecture is betting on.

Scope, deliberately narrow

- the `efficiency` analyzer only
- the `Explain` intent only
- the `InvestigationPlan` fed in hand-built, so nothing depends on the model
  yet

Build

- the generic `Explain` strategy as a graph walk:
  establish → decompose → partition → confound
- the analyzer as a *view over the graph* — subjects and domains, not mining
  logic
- emission of `InformationRequirement`s from the walk, with `ordering` and
  `limit` for the partition step

Exit criteria — **this is the go/no-go for the generic-analyzer thesis**

- the walk, given the "why did efficiency drop yesterday" plan, produces
  requirements that match the ones hand-written in `ANALYZER.md` — same
  subjects, observables, windows, aggregations
- the confounder step emits the availability requirement `ANALYZER.md`
  predicted, marked `Preferred`
- every emitted requirement validates against the registry
- no mining-specific string appears in the strategy code; all of it comes
  from the graph

If the walk reproduces the hand-written requirements, the thesis holds and
stages 6–8 proceed. **If it does not, we stop and revise the graph model
before building anything on top of it** — cheaply, against one analyzer.

Open questions blocking this stage: this stage *resolves* the biggest one.

---

# Stage 6 — Planning · Model

**Goal.** The four inference stages, with reproducibility measured, not
assumed.

Build

- validate, normalize, intent-extract, domain-resolve — each a model call at
  temperature 0 against a pinned model and versioned prompt template
- the four-state validation outcome and the `Incomplete`/`Ambiguous` return
  path
- `PlanProvenance` populated with the actual pins
- a **determinism harness**: the same question run N times must produce the
  same plan, and the harness reports when it does not

Exit criteria

- the four states each have a question that reliably produces them
- a rejected question returns a reason and `missing[]`, never a partial plan
- the determinism harness passes at a stated threshold across a fixed
  question set — and the threshold is written down, because "reproducible in
  practice" is a measured claim
- a full plan for the sample question is byte-identical to stage 4's
  hand-assembled one

De-risks. This is where "reproducible not deterministic" gets tested against
a real model. If stability is worse than the cache needs, we learn it here,
isolated, before breadth multiplies the surface.

Open questions blocking this stage: **enum members and prompt templates.**
The model classifies into whatever the registry declares — if the enums are
wrong, the model learns the wrong vocabulary. Reconcile enum members against
real site reporting before this stage hardens.

---

# Stage 7 — Dispatch

**Goal.** Fan out, run analyzers in parallel, collect one RequirementSet.

Build

- instantiate the analyzers a plan names
- parallel execution, each analyzer on the same pinned `world_version`
- the `max_runtime` deadline: an analyzer over deadline is abandoned, its
  partial output discarded
- deduplication: identical requirements merge, `requested_by` unions,
  strongest necessity wins
- normalized output ordering, so parallelism never changes the result
- the `ExecutionReport` — completed / empty / failed / timed_out, kept
  distinct
- registry validation of every requirement; unserviceable ones recorded, a
  `Required` unserviceable one fails the investigation

Exit criteria

- two analyzers emitting the identical availability requirement produce one
  merged requirement with `requested_by: [efficiency, maintenance]`
- a deliberately slow analyzer is abandoned at the deadline and reported
  `timed_out`, and `complete` is false
- an analyzer returning zero requirements is reported `empty`, never `failed`
- output ordering is identical across repeated runs of the same plan
- a requirement naming an unregistered term is rejected here, not passed on

De-risks. Concurrency and the failure taxonomy — the parts most likely to
hide nondeterminism and silent-empty bugs.

Open questions blocking this stage: **none.**

---

# Stage 8 — Breadth

**Goal.** All five analyzers, all six intents.

Build

- `flow`, `maintenance`, `environment`, `safety` as graph views
- the remaining strategies: `Compare`, `Locate`, `Recommend`, `Predict`,
  `Summarize`, each a distinct walk shape
- the environment analyzer's backward influence walk with window widening —
  the rain-persistence case from `GRAPH.md` end to end

Exit criteria

- each intent has a strategy and a test question whose requirements match a
  hand-written expectation
- the "why did efficiency drop" question, run whole, produces a RequirementSet
  matching the eleven-requirement example traced in the diagram
- the environment analyzer, on a post-rain window, widens its window back by
  the persistence figure and asks about rainfall before the window began
- `Locate` produces a valid plan with no time window, defaulting to
  `world_version`

De-risks. Confirms the generic strategies generalise past the one proven in
stage 5. Any strategy that needs bespoke logic surfaces here.

Open questions blocking this stage: **the `conditional` confounder field.**
If conditional confounders turn out to be common across intents, the free-text
field needs to become an evaluable predicate — a schema change decided here,
with real cases in hand.

---

# Stage 9 — Reproducibility

**Goal.** The plan is cacheable and replayable, end to end.

Build

- the cache key from `PIPELINE.md`: model, prompt version, registry version,
  normalized question, resolved constraints, world-version
- plan and RequirementSet caching keyed on it
- replay: the same question at the same world-version returns the cached
  artifact
- provenance traceable end to end — every requirement back to the analyzer,
  domain, and original sentence

Exit criteria

- a repeated question served from cache, proven by `cache_hit: true`
- changing any pinned input misses the cache; changing nothing hits it
- a stored RequirementSet from a past world-version replays identically today
- the full provenance chain resolves for every requirement in a set

De-risks. The reproducibility promise, made real and measured rather than
asserted.

Open questions blocking this stage: **none** — but this stage is only
honest if stage 6's determinism harness passed.

---

# Open Decisions, By The Stage That Needs Them

Not blockers to *starting* — blockers to *trusting* a stage's output.

```
before stage 5 output is real   time_idle definition
before stage 6 hardens          enum members vs. site reporting
before stage 6 hardens          prompt template versions
during stage 8                  conditional confounders → predicate language?
ongoing                         GRAPH.md ↔ REGISTRY.md drift
                                (stage 2/3 tests catch the machine-checkable part)
```

None of these block stages 0–4. Start there.

---

# What This Plan Does Not Cover

Everything past the RequirementSet, all of it part 2:

- the Reality Engine request/response contract
- iterative requirement rounds (`round`, `depends_on` reserved, unused)
- evidence, hypotheses, analyzer results
- synthesis, confidence algebra, stories, recommendations
- influence-edge weighting and cost modelling

The RequirementSet is the seam. When it is solid, its shape tells us what the
engine must answer — and part 2 begins there.

---

# The One-Line Version

Build the parts with no open questions first (0–4), prove the graph-walk
thesis on a single analyzer (5), then add the model, breadth, and caching
(6–9) — and let stage 5 be the gate that decides whether the architecture is
right before anything depends on it.
