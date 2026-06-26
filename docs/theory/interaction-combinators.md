---
title: Interaction Combinators
summary: Lafont's three symbols, six interaction rules, and the strong-confluence property Relativist relies on.
keywords: [interaction combinators, lafont, CON, DUP, ERA, gamma, delta, epsilon, interaction rule, annihilation, commutation, erasure, redex, active pair, normal form, strong confluence, principal port, auxiliary port]
modules: [net, reduction]
specs: [SPEC-00, SPEC-03, SPEC-01]
audience: [contributor, llm, researcher]
status: reference
updated: 2026-06-26
---

# Interaction Combinators

Relativist implements **Interaction Combinators** (Lafont, 1997) — a Turing-complete
model of computation built from just three symbols and six local rewrite rules. Everything
else in the system (partitioning, the wire protocol, the grid loop) exists to run these
rules across many machines while preserving their result. Read this before the architecture.

Canonical terminology lives in [SPEC-00](../specs/SPEC-00-glossary.md); the rule topologies
in [SPEC-03](../specs/SPEC-03-reduction.md).

## Symbols

An **agent** is one symbol with one **principal port** (port 0) and zero or more **auxiliary
ports**. Relativist uses exactly Lafont's three symbols:

| Symbol | Name | Notation | Ports | Auxiliaries |
|--------|------|----------|-------|-------------|
| CON | Constructor | γ (gamma) | 1 principal + 2 auxiliary | `left` (1), `right` (2) |
| DUP | Duplicator | δ (delta) | 1 principal + 2 auxiliary | `aux0` (1), `aux1` (2) |
| ERA | Eraser | ε (epsilon) | 1 principal only | — |

A **net** is a graph of agents whose ports are joined by **wires**. A port connects to exactly
one other port (**port linearity**, invariant T1). A wire that leaves the net unattached is a
**free port** — the net's interface to the outside.

## Active pairs and reduction

An **active pair** (redex) is two agents joined **principal-port to principal-port**. Reduction
only ever happens at an active pair (T2). To **reduce** a net is to repeatedly rewrite active
pairs until none remain; the result is the **normal form**. Relativist's reduction loop
(`reduce_all`) is in the [`reduction`](../architecture/modules.md#reduction) module.

## The six rules

The three symbols give six possible active pairs. Each rule consumes both agents and rewires
their auxiliaries. Two families:

**Annihilation** — same symbol meets itself; the two agents vanish and their auxiliaries are
cross-connected:

| Rule | Pair | Effect |
|------|------|--------|
| γ–γ | CON–CON | both consumed; the 4 auxiliaries reconnected straight-through |
| δ–δ | DUP–DUP | both consumed; the 4 auxiliaries reconnected straight-through |
| ε–ε | ERA–ERA | both consumed; nothing left |

**Commutation / erasure** — different symbols; the pattern is duplicated or erased:

| Rule | Pair | Effect |
|------|------|--------|
| γ–δ | CON–DUP | both consumed; **4 new agents** created (each passes through the other) |
| γ–ε | CON–ERA | CON consumed; **2 ERAs** created on its auxiliaries |
| δ–ε | DUP–ERA | DUP consumed; **2 ERAs** created on its auxiliaries |

These six rules are the entire computational core. The reducer dispatches on the symbol pair
via a static 3×3 table.

## Strong confluence — why distribution is correct

The property that makes Relativist possible is **strong confluence** (Lafont 1997, invariant
T4, the "diamond property"): *any two non-overlapping active pairs can be reduced in either
order and reach the same net.* Consequences that the whole system depends on:

- The **normal form is unique** for a terminating net (T6), and the total number of
  interactions is invariant (T7) — the result does not depend on reduction order.
- Therefore it does not depend on **who reduces what, or where**. Work can be split across
  machines, reduced independently, and merged, and the answer is identical to reducing
  sequentially on one machine.

This last point is Relativist's central claim, formalized as the **Fundamental Property G1**:

```
reduce_all(net)  ≅  run_grid(net, K)        (≅ = graph isomorphism, modulo agent renaming)
```

for any terminating net and any number of workers K. Strong confluence is the engine; the
distribution protocol ([partition](../specs/SPEC-04-partition.md) +
[merge](../specs/SPEC-05-merge.md)) is engineered to preserve net structure so that G1 holds.
The full invariant ladder (T/D/I/G) is in [invariants.md](invariants.md).

## Scope

Relativist studies **pure Lafont IC** — three symbols, no labels. This is deliberate: the
research question is the correctness of *distributed* reduction, not maximal expressiveness.
Labelled extensions (HVM/Bend-style) are out of scope (see [roadmap](../roadmap.md) §2.42).
The model is defined for **terminating** nets (premise P6); non-terminating nets are out of
scope.

## See also

- [SPEC-00 glossary](../specs/SPEC-00-glossary.md) — normative terminology, notation mappings.
- [SPEC-03 reduction](../specs/SPEC-03-reduction.md) — exact rule topologies, dispatch, loops.
- [invariants.md](invariants.md) — T1–T7, D1–D6, I1–I5, G1.
- [theory-bridge.md](../theory-bridge.md) — links to the thesis arguments (ARG-001 confluence).
