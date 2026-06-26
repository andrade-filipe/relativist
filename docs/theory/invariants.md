---
title: Invariants
summary: The correctness contract — theoretical (T), distribution (D), implementation (I), and the global G1 property.
keywords: [invariant, T1, T7, D1, D6, I1, I5, G1, fundamental property, confluence, port linearity, determinism, correctness, reduce_all, run_grid, isomorphism]
modules: [net, reduction, partition, merge]
specs: [SPEC-01, SPEC-00, SPEC-03, SPEC-04, SPEC-05]
audience: [contributor, llm, researcher]
status: reference
updated: 2026-06-26
---

# Invariants

These are the properties Relativist must preserve at every step. They are the contract a change
is checked against: if a change can affect one, the tests that verify it must stay green and the
change should name the invariant it touches. Formal definitions and proofs are in
[SPEC-01](../specs/SPEC-01-invariantes.md); this page is the working summary.

Four layers, bottom to top: **theory → distribution → implementation → the global property**.

## T — Theoretical (from Lafont's IC theory)

Properties inherited from the math; if any breaks, the model is wrong, not just the code.

| ID | Property |
|----|----------|
| T1 | **Port linearity** — every port is connected to exactly one other port. |
| T2 | **Principal-port interaction** — active pairs occur only principal-to-principal (port 0). |
| T3 | **Disjointness** — distinct active pairs share no agent. |
| T4 | **Strong confluence** — non-overlapping redexes commute (the diamond property). |
| T5 | **Rule correctness** — the six rules have exactly Lafont's topologies. |
| T6 | **Unique normal form** — a terminating net has one normal form, independent of order. |
| T7 | **Interaction-count invariance** — the total number of interactions is order-independent. |

T4 is the load-bearing one: it is what lets distribution be correct (see
[interaction-combinators.md](interaction-combinators.md#strong-confluence--why-distribution-is-correct)).

## D — Distribution (preserve T4 across partitioning)

Necessary for distributed reduction to equal sequential reduction.

| ID | Property |
|----|----------|
| D1 | **Split/merge identity** — `merge(split(net)) ≅ net`. |
| D2 | **Isomorphism under partitioning** — structure preserved modulo agent-ID renaming. |
| D3 | **Border-wire preservation** — wires crossing a partition boundary are faithfully tracked. |
| D4 | **ID-space confinement** — each partition owns a disjoint AgentId range (extended for free-list reuse). |
| D5 | **Interface preservation** — free ports / boundary markers survive split and merge. |
| D6 | **Determinism under distribution** — the distributed result does not depend on worker count or order. |

## I — Implementation (Rust-level guarantees)

What the data structures must keep true under CRUD operations.

| ID | Property |
|----|----------|
| I1 | **Bidirectional port array** — every connection is recorded from both ends. |
| I2 | **Reference validity** — every `PortRef` points to a live agent or a valid free port. |
| I3′ | **AgentId uniqueness** — IDs are unique (relaxed to permit free-list slot reuse). |
| I4 | **Stale-redex tolerance** — the redex queue may hold stale entries; reduction prunes them. |
| I5 | **Invariant preservation under CRUD** — create/remove/connect/disconnect keep I1–I4. |

## G1 — The Fundamental Property

The reason the project exists:

```
reduce_all(net)  ≅  extract_result(run_grid(net, K))      for every terminating net, every K
```

`≅` is graph isomorphism (structural equality modulo agent-ID renaming). Sequential reduction
and K-worker distributed reduction produce the same normal form. G1 is the top of the ladder: it
holds because T4 (confluence) holds and the D-layer keeps partition/merge structure-preserving.

Every benchmark data point is a witness to G1 — distributed output is checked isomorphic to
sequential output on each run.

## Amendments

- **Delta mode (SPEC-19 R38):** under the stateful delta protocol the coordinator holds a
  `BorderGraph` rather than a fully merged net between rounds; G1 is restated over the
  reconstructed net at Global Normal Form. The guarantee is unchanged; the intermediate
  representation differs. See [SPEC-19](../specs/SPEC-19-delta-protocol.md).
- **Free-list (SPEC-22):** I3 was relaxed to I3′ to allow recycling freed AgentId slots; D4 was
  extended to confine the free-list per partition. See [SPEC-22](../specs/SPEC-22-arena-management.md).

## See also

- [SPEC-01 invariants](../specs/SPEC-01-invariantes.md) — formal statements + proof obligations.
- [interaction-combinators.md](interaction-combinators.md) — where T4/G1 come from.
- [architecture/overview.md](../architecture/overview.md) — where each layer is enforced.
