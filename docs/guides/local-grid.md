---
title: Local grid (in-process BSP)
summary: Run the full BSP cycle across N simulated workers in one process, read round metrics, and confirm G1 (local output == sequential output).
keywords: [local, grid, bsp, in-process, workers, round-robin, strict-bsp, metrics, rounds, border-interactions, G1, partition, merge]
modules: [partition, merge]
specs: [SPEC-04, SPEC-05, SPEC-01]
audience: [user, llm]
status: guide
updated: 2026-06-26
---

# Local grid (in-process BSP)

`relativist local` runs the complete BSP cycle (partition -> reduce -> merge ->
resolve border -> repeat) across N simulated workers in a single process, with
no TCP. It is the fastest way to confirm that distribution preserves the result
(invariant G1) and to measure the protocol's algorithmic overhead in isolation
from network cost.

For the BSP execution model itself, see
[../architecture/overview.md](../architecture/overview.md) — this guide does
not restate it.

Prerequisite: you can already generate and inspect nets
([first-reduction.md](first-reduction.md)).

## usage

```bash
relativist local -w <WORKERS> -i <INPUT> [-o <OUTPUT>] [-m <METRICS>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --workers <N>` | required (`>=1`) | Simulated workers. |
| `-i, --input <PATH>` | required | Input net (`.bin` or `.ic`). |
| `-o, --output <PATH>` | none | Write the reduced net. |
| `-m, --metrics <PATH>` | none | Write per-round metrics (`.json`/`.csv`). |
| `--strategy <NAME>` | `round-robin` | Partitioning strategy (only `round-robin` in v1). |
| `--max-rounds <N>` | unlimited | Cap BSP rounds. |
| `--strict-bsp` | `false` | One genuine BSP round per queue drain (see below). |
| `--log-format <text\|json>` | TTY auto | Log output format. |

`--delta-mode` is rejected on `local` (it needs a coordinator runtime). Full
flag list: [../reference/cli.md](../reference/cli.md).

## smoke-test-four-workers

```bash
relativist generate ep-annihilation -n 500 -o ep500.bin
relativist local -w 4 -i ep500.bin
```

Expected output:

```
=== Relativist Execution Summary ===
Converged:          yes
Rounds:             1
Total interactions: 500
Total time:         0.000s
Final agents:       0
Avg round time:     0.000s
Local interactions: 500
Border interactions:0
```

How to read it:

- **Converged: yes** — the net reached normal form (SPEC-01 G1).
- **Rounds: 1** — no cross-partition cascade (Profile A workload).
- **Border interactions: 0** — no reaction needed a merge; round-robin spread
  independent redexes cleanly.

## save-result-and-metrics

```bash
relativist generate mixed-rules -n 5 -o mixed5.bin
relativist local -w 2 -i mixed5.bin -o mixed5_grid.bin -m metrics.json
```

`metrics.json` carries a `rounds` array. Per round:

- `partition_time_secs`, `compute_time_secs`, `merge_time_secs`, `network_time_secs`
- `border_redexes`, `border_ratio`, `agents_at_start`
- `bytes_sent`, `bytes_received` (zero in-process)

Use it to plot per-phase overhead across rounds.

## verify-g1

G1 (SPEC-01): the local grid output equals the sequential `reduce` output for
the same input. Confirm it directly:

```bash
relativist generate mixed-rules -n 50 -o m50.bin
relativist reduce -i m50.bin -o m50_seq.bin
relativist local -w 4 -i m50.bin -o m50_grid.bin
relativist inspect -i m50_seq.bin
relativist inspect -i m50_grid.bin   # same agent/symbol counts, both normal form
```

Both nets reach normal form with identical live-agent and per-symbol counts:
worker count does not change the result, only the path to it. The `bench`
suite checks this automatically (graph isomorphism, or `--skip-g1` for the
weak symbol-count check).

## strict-bsp

By default (`--strict-bsp=false`) Relativist runs in *lenient* mode: after the
merge, `reduce_all` drains the entire queue — including cross-partition
cascades produced by border resolution — so most benchmarks converge in a
**single round**.

To observe the real per-round cost (and, in TCP mode, real RTT):

```bash
relativist generate cascade-cross -n 100 -o cc100.bin
relativist local -w 2 -i cc100.bin --strict-bsp -m strict_metrics.json
```

In strict mode the queue is processed exactly **once** per round; newly created
redexes wait for the next round. For `cascade-cross(N)` with `workers >= 2` the
net finishes in **exactly N rounds**, matching the theoretical prediction.

## round-limit-and-json-log

```bash
# Stop after 5 rounds
relativist generate dual-tree -n 10 -o dual10.bin
relativist local -w 4 -i dual10.bin --max-rounds 5

# Structured log (data pipelines)
relativist generate con-dup-expansion -n 50 -o condup50.bin
relativist local -w 2 -i condup50.bin --log-format json
```

## when-not-to-use-local

- Real network cost -> `coordinator` + `worker`
  ([distributed-tcp.md](distributed-tcp.md)).
- Pure sequential algorithm cost -> `reduce`
  ([first-reduction.md](first-reduction.md)).
- High-level arithmetic -> `compute`
  ([church-arithmetic.md](church-arithmetic.md)).

---

**Next ->** [distributed-tcp.md](distributed-tcp.md): the same BSP cycle over
real TCP.
