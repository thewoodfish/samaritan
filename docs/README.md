# Samaritan — Documentation

Samaritan is a mining operational-intelligence pipeline. It turns a business
question ("why did efficiency drop yesterday?") into a **RequirementSet** — a
precise, reproducible specification of what to ask the Reality Engine — and
stops there, at the engine boundary.

## The specification

Read in this order.

| Doc | What it covers |
|---|---|
| [PIPELINE.md](PIPELINE.md) | The whole pipeline: stages, scope boundary, failure model, determinism |
| [SCHEMA.md](SCHEMA.md) | Every contract between stages — the single source of truth |
| [PLANNING.md](PLANNING.md) | Question → InvestigationPlan, the eight planning stages |
| [ANALYZER.md](ANALYZER.md) | The analyzer contract: a plan → the requirements it needs |
| [REGISTRY.md](REGISTRY.md) | Every vocabulary, mapping, threshold, and validation rule |
| [ENGINE.md](ENGINE.md) | The Samaritan ↔ Reality Engine boundary |
| [GRAPH.md](GRAPH.md) | The mining relation graph, worked in full |
| [REQUIREMENTSET.md](REQUIREMENTSET.md) | The terminal artifact — the shape of the engine's request API |
| [MEREDITH_API.md](MEREDITH_API.md) | The Reality Engine's query API — the contract Meredith answers (Part 2 entry point) |

## The build

| Doc | What it covers |
|---|---|
| [BUILD_PLAN.md](BUILD_PLAN.md) | The nine-stage build plan, with exit criteria per stage |

Part 1 (question → RequirementSet) is implemented across the workspace crates:
`schema → graph → registry → planning → analyzer → dispatch`.
