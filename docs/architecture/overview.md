---
title: Architecture overview
summary: BSP model, the pure-core/async-infrastructure layering, the inviolable dependency direction, and the coordinator/worker FSMs.
keywords: [architecture, BSP, bulk synchronous parallel, layer, dependency direction, core, infrastructure, coordinator, worker, FSM, state machine, grid loop, partition, merge, round, pure]
modules: [net, reduction, partition, merge, protocol, coordinator, worker, config, io]
specs: [SPEC-13, SPEC-04, SPEC-05, SPEC-06]
audience: [contributor, llm]
status: reference
updated: 2026-06-26
---

# Architecture overview

Relativist reduces an Interaction Combinator net across a grid of machines and gets the same
answer a single machine would (property [G1](../theory/invariants.md#g1--the-fundamental-property)).
This page is the system map; per-module detail and the code↔doc index are in
[modules.md](modules.md). The formal architecture is [SPEC-13](../specs/SPEC-13-system-architecture.md).

## Execution model: BSP

Distribution uses **Bulk Synchronous Parallel** with barrier synchronization. One **round** is:

```
        ┌──────────────────────────── coordinator ────────────────────────────┐
 net ─▶ │ split into K partitions ─▶ dispatch ─▶ (wait) ─▶ collect ─▶ merge ─▶ │ ─▶ repeat
        └──────────────────────────────────────────────────────────────────────┘
                         │ each partition                ▲ reduced partition
                         ▼                               │
                    worker: reduce_all(partition) ───────┘
```

Workers reduce their partition to a local normal form independently and in parallel. The
coordinator merges the results, resolves any **border redexes** (active pairs that were split
across two partitions), and repeats until the whole net is in normal form. Strong confluence
(T4) is what makes "reduce locally, merge, repeat" produce the sequential answer.

Topology is a single coordinator + K workers (star). The coordinator may also reduce one
partition itself (hybrid mode, SPEC-20), making effective parallelism K rather than K−1.

## Two layers

The codebase is split into a **pure core** and an **async infrastructure** layer.

- **Core** (`net`, `reduction`, `partition`, `merge`, plus `encoding`, `io`, `config`,
  `observability` setup) — pure synchronous Rust. **No async, no tokio, no network, no I/O** in
  the reduction path. This is what makes the core testable in isolation and reusable (e.g. the
  in-process `local` mode and the `ChannelTransport` exercise the exact same code paths as TCP).
- **Infrastructure** (`protocol`, `security`, `coordinator`, `worker`, `main`) — async (tokio),
  networking, framing, the FSMs. Depends on the core; the core never depends on it.

## Dependency direction (inviolable)

```
net  ◀─  reduction  ◀─  partition  ◀─  merge  ◀─  protocol  ◀─  coordinator / worker
```

A lower module must never import a higher one. If `net` seems to need to know about TCP, the
design is wrong — push the dependency upward. This is the single most important structural rule;
it is enforced by review and by the module boundaries.

## Coordinator and worker as state machines

Both the coordinator and worker are **stimulus-response finite state machines**: a pure
`transition(state, event) -> (state, actions)` function, with the async I/O kept at the edges.
This keeps the orchestration logic deterministic and unit-testable without a network.

- **Coordinator** drives the BSP rounds: register workers → partition → dispatch → collect →
  merge → check global normal form → next round or finish. Variants add delta mode (SPEC-19),
  elastic membership (SPEC-20), and pull-dispatch (SPEC-21).
- **Worker** connects, receives a partition, runs `reduce_all`, returns the result; optionally
  retains state across rounds (delta mode) and loops as a daemon (SPEC-16).

## Data flow end to end

```
io::load ─▶ partition::split ─▶ protocol (dispatch) ─▶ worker reduce_all
                                                              │
io::save ◀─ (normal form) ◀─ merge::merge + border resolve ◀─┘  ◀─ collect
```

The same pipeline runs three ways with identical results: `reduce` (pure sequential, no split),
`local` (in-process K-worker simulation), and `coordinator`/`worker` (real TCP grid).

## Performance shape (the headline)

On local/localhost hardware the distribution overhead structurally exceeds the parallel gain
(measured overhead/compute ratio `c_o/c_r ≈ 2.2`, needs `< 0.5` for speedup at K=2). This is a
deliberate, documented **negative result**; the path to break-even (delta protocol, coordinator-
free rounds, delta merge) and the Phase-3-LAN milestone are in
[reference/next-steps.md](../reference/next-steps.md) and [roadmap.md](../roadmap.md) §2.40.

## See also

- [modules.md](modules.md) — every module → its spec → its doc (the code↔doc index).
- [theory/interaction-combinators.md](../theory/interaction-combinators.md) — the model being run.
- [theory/invariants.md](../theory/invariants.md) — what the layers preserve.
- [SPEC-13](../specs/SPEC-13-system-architecture.md) — the formal architecture.
