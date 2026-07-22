# ENGINE.md

> Samaritan does not observe the world.
>
> It asks something that does.
>
> That something is the Reality Engine, and it does not know what mining is.

---

# The Two Systems

```
Samaritan            operational intelligence
                     questions · intent · domains · requirements
                            │
                     ═══ boundary ═══
                            │
Reality Engine       spatial intelligence
                     entities · space · time · events · relationships
```

The engine maintains a live model of the physical world. Everything that
happens becomes an event in an append-only log. The world at any moment is
that log projected to that moment.

Samaritan asks it questions. Nothing more.

---

# The Golden Rule

**The engine is domain agnostic.**

It stores reality. It never stores business meaning.

It has never heard of a haul cycle, an efficiency target, a restricted area,
or a shift. It knows entities, positions, zones, events and relationships.

This is not a stylistic preference. It is what allows the same engine to run
a wildlife reserve and a mine without becoming two engines.

Samaritan must never push meaning downward to make a query easier.

---

# World State vs Operational State

The cut that matters most.

```
World State           what objectively exists
                      a vehicle is located inside a polygon

Operational State     what that means here
                      a haul truck breached a blasting zone
```

The engine owns the first. Samaritan owns the second.

The engine can compute containment, because containment is geometry.

It cannot compute *breach*, because breach is a policy.

## Operational state is still real

Samaritan's conclusions are not scratch data. When operational meaning is
recorded, it is recorded as world state — event-sourced, replayable, and
carrying provenance back to the facts that caused it.

An incident replays bit-identically like any observed fact. "Application
computed" does not mean "second class".

---

# The Domain Layer

Samaritan does not talk to the engine in mining terms directly. A domain
layer sits between them.

```
Samaritan            asks about haul_cycle
     │
Domain layer         registers mining types, derives mining facts
     │
Engine               stores and indexes them as opaque named types
```

The domain layer

- registers the component and event types the mining vocabulary needs
- runs the systems that derive mining facts from engine facts
- namespaces everything it registers
- never teaches the engine what any of it means

The engine stores `mining:haul_cycle` the way it stores anything else: as a
registered name it indexes and never interprets.

This is why `REGISTRY.md`'s subject vocabulary is the **domain layer's
contract**, not the engine's data model.

---

# Where Computation Happens

Three layers. Stated fully in `SCHEMA.md`.

```
derivation    upstream, as the world advances     facts → facts
reduction     at query time, in the engine        facts → numbers
reasoning     in Samaritan                        numbers → meaning
```

The engine derives and reduces. It never reasons.

Derivation is the one most easily misplaced. `time_idle` is not computed
when someone asks for it — a domain system derives it as the world moves and
emits it as an event. By query time it is a stored fact with a traceable
origin.

That is what makes it evidence rather than a number that appeared.

---

# Reading The World At A Moment

The engine is a log. The present is a projection. Replay is the same
projection, earlier.

Every investigation therefore pins a `world_version` — a log position taken
once, at plan time, carried through every requirement.

Two reasons.

**Consistency.** Analyzers run in parallel while events keep arriving.
Without a pinned position they would read different worlds and their
findings would be quietly incomparable.

**Honesty.** The engine records identity corrections — two entities proving
to be one, one proving to be two — as events, and reinterprets the log when
it does. **What "yesterday" contained can change after yesterday has
passed.** An investigation that does not say which world it read cannot be
defended later.

---

# Inherited Risk

One property of the engine's extension model matters more to Samaritan than
to any other consumer.

**A missing capability fails silently.**

A domain system that derives from `zone-entered` produces nothing at all if
the application never registered zone containment. No error is raised. The
event simply never arrives, and every downstream derivation is empty.

For most applications that is a bug. For Samaritan it is a correctness
hazard of a different order: **an absence of evidence is indistinguishable
from evidence of absence.**

An investigation would report that no truck entered a restricted zone
yesterday. It would be wrong, confident, and unfalsifiable.

## Required mitigation

Samaritan must never infer "nothing happened" from "nothing returned."

- every requirement's subject and observables are checked against what the
  engine actually has registered, at investigation start, not at query time
- an unavailable subject is reported as **unavailable**, never as empty
- `RequirementSet.complete` is false when any requirement could not be
  served, for any reason
- a `Required` requirement that cannot be served fails the investigation
  rather than degrading it silently

The analyzer capability declarations in `REGISTRY.md` exist partly for this:
an analyzer states the subjects it needs, so the gap is caught at load time
rather than discovered as silence.

---

# What Samaritan Must Never Do

- ask the engine to understand a business rule
- store domain meaning in engine components without namespacing it
- read the world without pinning a version
- treat an empty result as a negative finding
- carry zone geometry, entity truth, or spatial index state of its own
- bypass the domain layer to query engine internals directly

---

# Guiding Principle

> The engine understands reality.

> Samaritan understands what it means here.

> Neither is allowed to do the other's job.
