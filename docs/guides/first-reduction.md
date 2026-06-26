---
title: First reduction
summary: Generate an IC net, inspect its statistics, and reduce it sequentially to normal form — the generate/inspect/reduce workflow.
keywords: [first reduction, generate, inspect, reduce, reduce_all, sequential, baseline, normal form, workload, ic text format, bincode, smoke test]
modules: [io, reduction]
specs: [SPEC-07, SPEC-12]
audience: [user, llm]
status: guide
updated: 2026-06-26
---

# First reduction

Generate an IC net, inspect its contents, and run **sequential** reduction (the
baseline) to normal form. Three subcommands: `generate`, `inspect`, `reduce`.

> Prerequisite: `relativist` installed — see
> [getting-started.md](getting-started.md).

## generate

`generate <EXAMPLE> -n <SIZE> -o <FILE>` writes a parametric workload net to disk.
Output format is inferred from the extension (`.bin` bincode, `.ic` text).

```bash
relativist generate <EXAMPLE> -n <SIZE> -o <FILE>
```

### workload types

| `<EXAMPLE>`            | Profile | Description                              |
|-----------------------|---------|------------------------------------------|
| `ep-annihilation`     | A       | N ERA-ERA pairs (trivial annihilation)   |
| `ep-annihilation-con` | A       | N CON-CON pairs (cross annihilation)     |
| `ep-annihilation-dup` | A       | N DUP-DUP pairs (parallel annihilation)  |
| `con-dup-expansion`   | B       | N CON-DUP pairs (expansion + collapse)   |
| `dual-tree`           | B/C     | Two trees of depth N                     |
| `mixed-rules`         | C       | ERA-ERA + CON-CON + CON-DUP in thirds    |
| `erasure-propagation` | C       | Chain of N CON agents, ERA at the head   |
| `tree-sum`            | A/B     | Sum of N ones via Church add             |
| `sum-of-squares`      | A/B     | 1² + 2² + … + N² via Church add chain    |

```bash
relativist generate ep-annihilation -n 100 -o ep100.bin     # 100 ERA-ERA pairs (.bin)
relativist generate dual-tree -n 6 -o dual6.ic              # dual tree, depth 6 (.ic text)
relativist generate mixed-rules -n 10 -o mixed10.bin        # 10 of each rule
relativist generate erasure-propagation -n 50 -o erasure50.bin
relativist generate con-dup-expansion -n 100 -o condup100.bin
```

### text format (.ic)

`.ic` is human-readable — useful for debugging. Three ERA-ERA pairs:

```bash
relativist generate ep-annihilation -n 3 -o ep3.ic
cat ep3.ic
```

```
agent a0 ERA
agent a1 ERA
agent a2 ERA
agent a3 ERA
agent a4 ERA
agent a5 ERA
wire a0.principal a1.principal
wire a2.principal a3.principal
wire a4.principal a5.principal
```

Full `.bin` / `.ic` details: [../reference/file-formats.md](../reference/file-formats.md).

## inspect

`inspect -i <FILE>` prints statistics — live agent count, per-symbol counts,
redex count, normal-form flag — without modifying anything.

```bash
relativist generate ep-annihilation -n 100 -o ep100.bin
relativist inspect -i ep100.bin
```

```
=== Relativist Inspect ===
Agents:  200
  CON: 0
  DUP: 0
  ERA: 200
Redexes: 100
Normal Form: no
```

After reduction the same net is empty and in normal form:

```bash
relativist reduce -i ep100.bin -o ep100_reduced.bin
relativist inspect -i ep100_reduced.bin
```

```
=== Relativist Inspect ===
Agents:  0
  CON: 0
  DUP: 0
  ERA: 0
Redexes: 0
Normal Form: yes
```

## reduce

`reduce -i <INPUT> [-o <OUTPUT>]` calls `reduce_all` directly until no redex
remains — no partitioning, no parallelism. This is the **baseline** the grid, TCP,
and delta modalities are compared against. It prints the interaction count and
final agent count.

```bash
# Basic reduction, no persisted output
relativist generate ep-annihilation -n 1000 -o ep1000.bin
relativist reduce -i ep1000.bin
```

```
=== Relativist Reduce Summary ===
Interactions: 1000
Final agents: 0
```

```bash
# Reduce and persist the result
relativist generate dual-tree -n 8 -o dual8.bin
relativist reduce -i dual8.bin -o dual8_reduced.bin

# Reduce, persist, then inspect
relativist generate erasure-propagation -n 20 -o erasure20.bin
relativist reduce -i erasure20.bin -o erasure20_reduced.bin
relativist inspect -i erasure20_reduced.bin
```

## end-to-end smoke test

Confirms the install works from generate through inspect:

```bash
relativist generate mixed-rules -n 20 -o mixed20.bin
relativist inspect  -i mixed20.bin            # Agents: 240, Redexes: 120
relativist reduce   -i mixed20.bin -o mixed20_seq.bin
relativist inspect  -i mixed20_seq.bin        # Agents: 80 (all ERA), Redexes: 0, Normal Form: yes
```

---

**Next →** [local-grid.md](local-grid.md): run the same reduction across N
simulated workers with `local -w N`.
