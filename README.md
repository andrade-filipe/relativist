# Relativist

**Distributed Interaction Combinator reducer for Grid Computing**

Relativist is a Rust implementation of [Lafont's Interaction Combinators](https://doi.org/10.1016/S0890-5401(97)90136-X) (1997) with native support for distributed reduction across a grid of machines. It leverages the **strong confluence** property of Interaction Combinators to guarantee that distributed reduction produces the exact same result as sequential reduction, regardless of partitioning strategy or execution order.

## Key Properties

- **Deterministic distributed reduction** — Strong confluence ensures the result is identical whether computed on 1 machine or 8
- **Zero coordination overhead for correctness** — Workers reduce independently; only boundary redexes require cross-node resolution
- **Formally specified** — Every module has a detailed spec with invariants, requirements, and Rust type signatures
- **TDD from specs** — 690+ tests, 13 benchmarks across 3 workload profiles

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

## Documentation

Start here based on your goal:

| Goal                                   | Start at                                                               |
|----------------------------------------|------------------------------------------------------------------------|
| Learn Relativist step by step          | [**docs/guides/**](docs/guides/README.md) — 7-step learning path        |
| Look up a command or flag              | [docs/reference/cli.md](docs/reference/cli.md)                          |
| Reproduce or extend benchmarks         | [docs/benchmarks/](docs/benchmarks/README.md)                           |
| Understand the invariants (G1, D3, D6) | [docs/reference/invariants.md](docs/reference/invariants.md)            |
| Debug an issue                         | [docs/reference/troubleshooting.md](docs/reference/troubleshooting.md)  |
| Contribute code                        | [CONTRIBUTING.md](CONTRIBUTING.md)                                      |
| Navigate everything else               | [docs/INDEX.md](docs/INDEX.md)                                          |

v2 features already documented: [delta protocol (SPEC-19)](docs/guides/06-delta-protocol.md), [zero-copy wire format (SPEC-18)](docs/guides/07-zero-copy.md).

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

All design decisions are documented in formal specifications under [`specs/`](specs/). 17 specs (SPEC-00 through SPEC-16) cover the v1 surface; SPEC-17 onwards cover v2 work. See [docs/INDEX.md](docs/INDEX.md) for the full table.

## Benchmark Results

**Zero correctness failures in 4,490 benchmark executions.**

| Campaign | Reps | Wall Clock | Correctness | Mode |
|----------|------|------------|-------------|------|
| Phase 1 (in-process) | 3,800+ | 11 min 39 s | 0 failures | Local shared-memory |
| Phase 2 (Docker/TCP) | 400 | 43 min 42 s | 0 failures | TCP localhost containers |

Every single data point is verified by the fundamental property:

```
reduce_all(net) ≅ run_grid(net, n)
```

where ≅ denotes graph isomorphism (structural equality modulo ID renaming).

**Strict BSP validation** confirms theoretical predictions exactly:
- `cascade_cross(N)` terminates in N rounds (workers ≥ 2)
- `dual_tree(d)` terminates in d rounds (workers ≥ 2)

Full data: [`results/locked/v1_local_baseline/`](results/locked/v1_local_baseline/) — frozen with SHA-256 checksums and provenance manifest. Reproduction: [docs/benchmarks/campaigns/v1-local-baseline.md](docs/benchmarks/campaigns/v1-local-baseline.md).

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

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

[MIT](LICENCE)
