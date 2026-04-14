# Relativist

**Distributed Interaction Combinator reducer for Grid Computing**

Relativist is a Rust implementation of [Lafont's Interaction Combinators](https://doi.org/10.1016/S0890-5401(97)90136-X) (1997) with native support for distributed reduction across a grid of machines. It leverages the **strong confluence** property of Interaction Combinators to guarantee that distributed reduction produces the exact same result as sequential reduction, regardless of partitioning strategy or execution order.

## Key Properties

- **Deterministic distributed reduction** — Strong confluence ensures the result is identical whether computed on 1 machine or 8
- **Zero coordination overhead for correctness** — Workers reduce independently; only boundary redexes require cross-node resolution
- **Formally specified** — Every module has a detailed spec with invariants, requirements, and Rust type signatures
- **TDD from specs** — 676+ tests, 11 benchmarks across 3 workload profiles

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

## Quick Start

### Local mode (simulated distribution)

```bash
cargo build --release
./target/release/relativist local --workers 4 --net examples/ep_annihilation.bin
```

### Distributed mode

```bash
# Terminal 1: Start coordinator
./target/release/relativist coordinator --workers 2 --port 9000 --net examples/ep_annihilation.bin

# Terminal 2: Start worker
./target/release/relativist worker --coordinator localhost:9000

# Terminal 3: Start another worker
./target/release/relativist worker --coordinator localhost:9000
```

### Docker

```bash
docker-compose up --scale worker=4
```

> For the full command reference — every subcommand, every flag, end-to-end pipelines, benchmark workflows, known limitations (L1-L7), and troubleshooting — see [**USAGE_GUIDE.md**](USAGE_GUIDE.md).

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

All design decisions are documented in formal specifications:

| Spec | Title | Reqs |
|------|-------|------|
| [SPEC-00](specs/SPEC-00-glossary.md) | Glossary | 35 terms |
| [SPEC-01](specs/SPEC-01-invariantes.md) | Invariants | 24 invariants |
| [SPEC-02](specs/SPEC-02-net-representation.md) | Net Representation | 27 reqs |
| [SPEC-03](specs/SPEC-03-reduction.md) | Reduction Engine | 26 reqs |
| [SPEC-04](specs/SPEC-04-partition.md) | Partitioning | 28 reqs |
| [SPEC-05](specs/SPEC-05-merge.md) | Merge & Grid Cycle | 40 reqs |
| [SPEC-06](specs/SPEC-06-wire-protocol.md) | Wire Protocol | 40 reqs |
| [SPEC-07](specs/SPEC-07-deployment.md) | Deployment | 44 reqs |
| [SPEC-08](specs/SPEC-08-test-strategy.md) | Test Strategy | 44 reqs |
| [SPEC-09](specs/SPEC-09-benchmarks.md) | Benchmarks | 49 reqs |
| [SPEC-10](specs/SPEC-10-security.md) | Security | 37 reqs |
| [SPEC-11](specs/SPEC-11-observability.md) | Observability | 37 reqs |
| [SPEC-12](specs/SPEC-12-user-io.md) | User I/O & Examples | 61 reqs |
| [SPEC-13](specs/SPEC-13-system-architecture.md) | System Architecture | 52 reqs |
| [SPEC-14](specs/SPEC-14-encoding.md) | Arithmetic Encoding | 27 reqs |
| [SPEC-15](specs/SPEC-15-distribution.md) | Distribution & Packaging | 20 reqs |
| [SPEC-16](specs/SPEC-16-worker-daemon.md) | Worker Daemon Mode | 13 reqs |

## Benchmark Results

**Zero correctness failures in 4,200 benchmark executions.**

| Campaign | Reps | Wall Clock | Correctness | Mode |
|----------|------|------------|-------------|------|
| Phase 1 (in-process) | 3,800 | 11 min 39 s | 0 failures | Local shared-memory |
| Phase 2 (Docker/TCP) | 400 | 43 min 42 s | 0 failures | TCP localhost containers |

Every single data point is verified by the fundamental property:

```
reduce_all(net) ≅ run_grid(net, n)
```

where ≅ denotes graph isomorphism (structural equality modulo ID renaming).

**Strict BSP validation** confirms theoretical predictions exactly:
- `cascade_cross(N)` terminates in N rounds (workers ≥ 2)
- `dual_tree(d)` terminates in d rounds (workers ≥ 2)

Full data: [`results/locked/v1_local_baseline/`](results/locked/v1_local_baseline/) — frozen with SHA-256 checksums and provenance manifest.

## Known Limitations

1. **No break-even on local shared memory** — Distribution overhead exceeds parallel gain for all tested configurations in-process. Break-even is expected on network-separated machines (Phase 3 LAN).
2. **Round-robin partitioning only** — No topology-aware partitioning (planned for v2).
3. **Single coordinator, star topology** — Scalability limited by coordinator merge bandwidth.
4. **Terminating nets only** — Non-terminating nets are out of scope (qualified by premise P6).
5. **Exponential readback** — Church exponential results cannot be decoded back to integers (DUP cycle limitation).

## Who Is This For

- **Researchers** studying Interaction Combinators, Interaction Nets, or distributed graph rewriting
- **Grid computing practitioners** exploring deterministic distributed computation models
- **Students** learning Spec-Driven Development, TDD from specs, or distributed systems
- **HVM/Bend community** curious about distributed IC reduction beyond shared memory

## Documentation

- [**USAGE_GUIDE.md**](USAGE_GUIDE.md) — Complete command reference (every subcommand, every flag, end-to-end pipelines, known limitations L1-L7)
- [**CONTRIBUTING.md**](CONTRIBUTING.md) — Development guidelines
- [**docs/INDEX.md**](docs/INDEX.md) — Full documentation index (542 documents organized by topic)
- [**specs/**](specs/) — 17 formal specifications

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
