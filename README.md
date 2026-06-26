# Relativist

**Distributed Interaction Combinator reducer for Grid Computing**

Relativist is a Rust implementation of [Lafont's Interaction Combinators](https://doi.org/10.1016/S0890-5401(97)90136-X) (1997) with native support for distributed reduction across a grid of machines. It leverages the **strong confluence** property of Interaction Combinators to guarantee that distributed reduction produces the exact same result as sequential reduction, regardless of partitioning strategy or execution order.

## Key Properties

- **Deterministic distributed reduction** — Strong confluence ensures the result is identical whether computed on 1 machine or 8
- **Zero coordination overhead for correctness** — Workers reduce independently; only boundary redexes require cross-node resolution
- **Formally specified** — Every module has a detailed spec with invariants, requirements, and Rust type signatures
- **TDD from specs** — 1798 default / 1842 zero-copy / 1789 streaming-no-recycle / 1740 release tests on `v2-development` (v1 inviolable floor: 690 on `v1-feature-complete`); 13 benchmarks across 3 workload profiles

## Architecture

```
                    ┌─────────────┐
                    │ Coordinator │
                    │  (merge +   │
                    │  partition) │
                    └──────┬──────┘
                           │ TCP (bincode + CRC32)
              ┌────────────┼────────────┐
              │            │            │
        ┌─────┴─────┐ ┌───┴───┐ ┌─────┴─────┐
        │  Worker 1  │ │  W 2  │ │  Worker N  │
        │ reduce_all │ │  ...  │ │ reduce_all │
        └────────────┘ └───────┘ └────────────┘
```

The coordinator partitions an IC net, distributes partitions to workers via TCP, collects reduced partitions, merges them, resolves boundary redexes, and repeats until the net reaches normal form.

## 3-minute Quick Start

```bash
# 1. Build
cargo build --release

# 2. Generate a small test net
./target/release/relativist generate ep-annihilation -n 20 -o test.bin

# 3. Reduce sequentially
./target/release/relativist reduce -i test.bin -o seq.bin

# 4. Reduce via simulated grid (4 workers, in-process)
./target/release/relativist local -w 4 -i test.bin -o grid.bin

# 5. Confirm identical outputs
./target/release/relativist inspect -i seq.bin
./target/release/relativist inspect -i grid.bin
```

Both `inspect` calls must show identical agent counts and `Normal Form: yes`. That is G1 (fundamental property) in action.

**Full distributed mode (3 terminals):**

```bash
# Terminal 1: Coordinator
./target/release/relativist coordinator --workers 2 --port 9000 -i test.bin -o out.bin

# Terminals 2 and 3: Workers
./target/release/relativist worker --coordinator localhost:9000
```

**Docker:**

```bash
docker compose up --scale worker=4
```

## What's new in v2

- **SPEC-17 transport abstraction** — `Transport` trait with `TcpTransport`, `UnixTransport` (UDS) and `ChannelTransport` (in-process) backends. `--transport tcp|unix` flag plus full TCP knob suite (`--keepalive`, `--send-buffer`, `--recv-buffer`, `--no-tcp-nodelay`).
- **SPEC-18 wire format v2** — bincode v2 (varint), `PortRef` compacto (2-5 bytes), LZ4 compression above `--compression-threshold`, optional rkyv zero-copy archive on hot-path messages (`--features zero-copy` + `--use-zero-copy`).
- **SPEC-19 delta protocol** — stateful workers; only border deltas cross the wire (`--delta-mode`). Coordinator BorderGraph replaces the merged net during convergence; final reconstruct only at Global Normal Form.
- **SPEC-21 streaming generation** — chunked generate -> partition -> dispatch pipeline (`--chunk-size`, default 10000). Coordinator peak memory bounded by O(chunk_size + border state) instead of O(total_agents). Round-robin and FENNEL streaming strategies.
- **SPEC-22 arena management** — free-list recycle of consumed agent slots (`--recycle-policy`, default `disable-under-delta`) plus dense/sparse routing for build_subnet (`--representation` for bench; automatic threshold for runtime).
- **D-011 dense-path bug fix** — `effective_arena_size = max_live_id + 1` replaces the misleading planning-range metric in SPEC-22 R22; closes the +83% wall-clock regression on `ep_annihilation_con 5M w=2`.
- **D-012 instrumentation restored** — `compute_time_secs`, `network_time_secs`, and `mips_mean` are populated for every TCP-mode row in the canonical baseline. Wall-time is now decomposed by component (compute / network / merge), enabling targeted analysis of where each ms is spent.

The five v2 features (delta protocol, zero-copy wire, elastic grid, streaming generation, arena management) are documented together in [docs/guides/v2-features.md](docs/guides/v2-features.md).

## Documentation

The full, keyword-searchable catalog is **[docs/README.md](docs/README.md)** — start there. Quick links:

| Goal                                   | Start at                                                               |
|----------------------------------------|------------------------------------------------------------------------|
| Understand the model                   | [docs/theory/interaction-combinators.md](docs/theory/interaction-combinators.md) |
| Understand the system                  | [docs/architecture/overview.md](docs/architecture/overview.md)         |
| Learn Relativist step by step          | [docs/guides/](docs/guides/README.md) — install → v2 features          |
| Look up a command or flag              | [docs/reference/cli.md](docs/reference/cli.md)                          |
| Use the v2 features                    | [docs/guides/v2-features.md](docs/guides/v2-features.md)                |
| Reproduce or extend benchmarks         | [docs/benchmarks/](docs/benchmarks/README.md)                           |
| Understand the invariants (G1, D3, D6) | [docs/theory/invariants.md](docs/theory/invariants.md)                  |
| Map code to docs                       | [docs/architecture/modules.md](docs/architecture/modules.md)           |
| Debug an issue                         | [docs/reference/troubleshooting.md](docs/reference/troubleshooting.md)  |
| Contribute code                        | [CONTRIBUTING.md](CONTRIBUTING.md)                                      |
| Browse everything                      | [docs/README.md](docs/README.md) (catalog)                              |

## Interaction Combinators

Relativist implements the three fundamental symbols and six interaction rules of Lafont's system:

| Symbol | Name | Ports |
|--------|------|-------|
| γ (gamma) | CON (Constructor) | 1 principal + 2 auxiliary |
| δ (delta) | DUP (Duplicator) | 1 principal + 2 auxiliary |
| ε (epsilon) | ERA (Eraser) | 1 principal + 0 auxiliary |

| Rule | Interaction | Effect |
|------|-------------|--------|
| Annihilation | γ-γ | Both consumed, 4 wires reconnected |
| Annihilation | δ-δ | Both consumed, 4 wires reconnected |
| Commutation | γ-δ | Both consumed, 4 new agents created |
| Erasure | γ-ε | CON consumed, 2 ERAs created |
| Erasure | δ-ε | DUP consumed, 2 ERAs created |
| Erasure | ε-ε | Both consumed |

The **strong confluence** theorem (Lafont 1997) guarantees that any two non-overlapping redexes can be reduced in any order with the same final result. Combined with a distribution protocol that preserves net structure (premises P2-P5), this is what makes distributed reduction of terminating nets deterministic.

## Specs

All design decisions are documented in 28 formal specifications under [`docs/specs/`](docs/specs/README.md). SPEC-00 through SPEC-14 define the shipped v1 surface; SPEC-15 onwards cover v2 work. See the [spec index](docs/specs/README.md) for the full table and the [documentation catalog](docs/README.md) to navigate everything.

## Benchmark Results

**Canonical baseline (v2):** [`reproduce_article/results/locked/v2_post_d012_baseline_2026-05-05/`](reproduce_article/results/locked/v2_post_d012_baseline_2026-05-05/)

| Property | Value |
|----------|-------|
| `all_correct=true` slots | **32 / 32** distributed + 8 / 8 sequential |
| Repetitions per slot | 10 |
| `mips_mean` range (TCP-mode) | 0.002 – 1.261 |
| `network_time_secs` populated | 100% of TCP rows (D-012 RF-04 closed) |
| `compute_time_secs` populated | 100% of TCP rows (D-012 RF-05 closed) |

**Per-component decomposition headline (`ep_annihilation_con 500k w=1`):** wall = 0.460 s = compute 0.10 s + network 0.39 s + merge 0.04 s. The wire dominates ~85% of round time on Docker localhost — Phase 3 LAN will quantify how much further it grows on real cabling.

| Campaign (historical) | Reps | Wall Clock | Correctness | Mode |
|-----------------------|------|------------|-------------|------|
| Phase 1 (in-process)  | 3,800+ | 11 min 39 s | 0 failures | Local shared-memory |
| Phase 2 v1 (Docker/TCP) | 400 | 43 min 42 s | 0 failures | TCP localhost containers (v1) |
| Phase 2 v2 post-D-012 | 400 | 2 h 8 min | 0 failures | TCP localhost containers (v2 canonical) |

Every single data point is verified by the fundamental property:

```
reduce_all(net) ≅ run_grid(net, n)
```

where ≅ denotes graph isomorphism (structural equality modulo ID renaming).

**Strict BSP validation** confirms theoretical predictions exactly:
- `cascade_cross(N)` terminates in N rounds (workers ≥ 2)
- `dual_tree(d)` terminates in d rounds (workers ≥ 2)

Full data and SHA-256 checksums:
- v2 canonical: [`reproduce_article/results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md`](reproduce_article/results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md)
- v1 frozen reference: [`reproduce_article/results/locked/v1_local_baseline/`](reproduce_article/results/locked/v1_local_baseline/) — Reproduction: [docs/benchmarks/campaigns/v1-local-baseline.md](docs/benchmarks/campaigns/v1-local-baseline.md).
- Cold post-mortem analysis: [docs/analysis/D011-final-baseline-analysis-2026-05-04.md](docs/analysis/D011-final-baseline-analysis-2026-05-04.md).

## Known Limitations

Summary only — full list with status and remediation in [docs/benchmarks/limitations.md](docs/benchmarks/limitations.md) (L1-L7).

1. **No break-even on local shared memory** — Distribution overhead exceeds parallel gain in-process. Break-even is expected on network-separated machines (Phase 3 LAN).
2. **Round-robin partitioning only** — No topology-aware partitioning (v2 work).
3. **Single coordinator, star topology** — Scalability limited by coordinator merge bandwidth.
4. **Terminating nets only** — Non-terminating nets are out of scope (qualified by premise P6).
5. **Exponential readback** — Church exponential results cannot be decoded back to integers (DUP cycle limitation).

## Who Is This For

- **Researchers** studying Interaction Combinators, Interaction Nets, or distributed graph rewriting
- **Grid computing practitioners** exploring deterministic distributed computation models
- **Students** learning Spec-Driven Development, TDD from specs, or distributed systems
- **HVM/Bend community** curious about distributed IC reduction beyond shared memory

## Research Context

Relativist is part of a Computer Science thesis (TCC) at Universidade Tiradentes (UNIT), investigating whether Interaction Combinators can serve as a formal model for distributed reduction in Grid Computing.

**Research question:** Do the strong confluence and locality properties of Lafont's Interaction Combinators, combined with a structure-preserving protocol, allow building a distributed reduction model for Grid Computing where the result is deterministic regardless of work order and distribution, for terminating nets?

**Hypothesis:** Yes — strong confluence (P1), combined with protocol correctness (P2-P5: split/merge identity, border completeness, ID consistency, termination), guarantees that distributed reduction of terminating nets produces results structurally identical to sequential reduction.

### References

- Lafont, Y. (1990). *Interaction Nets*. POPL.
- Lafont, Y. (1997). *Interaction Combinators*. Information and Computation.
- Mackie, I. (1997). *Static Analysis of INets for Distributed Implementation*. POPL Workshop.
- Kahl, W. (2015). *Simple Parallel Implementation of INets in Haskell*. IFL.
- Taelin (2024). *HVM2: A Parallel Evaluator for Interaction Combinators*.
- Foster, I., Kesselman, C., Tuecke, S. (2001). *The Anatomy of the Grid*.
- Arrighi, P. et al. (2024). *Space-time Deterministic Graph Rewriting*. LIPIcs.

## Reproducing the paper

All empirical claims in the thesis are backed by frozen, checksummed data and the
exact scripts that produced it, isolated under
[`reproduce_article/`](reproduce_article/) — start at its
[README](reproduce_article/README.md). The application code keeps evolving; that
folder is a snapshot pinned to the paper.

## Contributing

Relativist is developed with the **RPI** workflow (Research → Plan → Implement) —
see [CONTRIBUTING.md](CONTRIBUTING.md), [CODING_STANDARDS.md](CODING_STANDARDS.md),
and the agent definitions in [`.claude/agents/`](.claude/agents/README.md).
Community expectations: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). How decisions are
made: [GOVERNANCE.md](GOVERNANCE.md). Reporting a vulnerability:
[SECURITY.md](SECURITY.md). What the project should do next:
[docs/reference/next-steps.md](docs/reference/next-steps.md).

## License

Licensed under the [Apache License 2.0](LICENSE) — permissive, with an explicit
patent grant. See [NOTICE](NOTICE) for attribution.
