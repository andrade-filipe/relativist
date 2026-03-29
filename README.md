# Relativist

**Distributed Interaction Combinator reducer for Grid Computing**

Relativist is a Rust implementation of [Lafont's Interaction Combinators](https://doi.org/10.1016/S0890-5401(97)90136-X) (1997) with native support for distributed reduction across a grid of machines. It leverages the **strong confluence** property of Interaction Combinators to guarantee that distributed reduction produces the exact same result as sequential reduction, regardless of partitioning strategy or execution order.

## Key Properties

- **Deterministic distributed reduction** — Strong confluence ensures the result is identical whether computed on 1 machine or 8
- **Zero coordination overhead for correctness** — Workers reduce independently; only boundary redexes require cross-node resolution
- **Formally specified** — Every module has a detailed spec with invariants, requirements, and Rust type signatures
- **TDD from specs** — 103 tests specified before a single line of implementation code

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
| [SPEC-00](specs/SPEC-00-glossario.md) | Glossary | 35 terms |
| [SPEC-01](specs/SPEC-01-invariantes.md) | Invariants | 16 invariants |
| [SPEC-02](specs/SPEC-02-net-representation.md) | Net Representation | 25 reqs |
| [SPEC-03](specs/SPEC-03-reduction.md) | Reduction Engine | 19 reqs |
| [SPEC-04](specs/SPEC-04-partition.md) | Partitioning | 24 reqs |
| [SPEC-05](specs/SPEC-05-merge.md) | Merge & Grid Cycle | 33 reqs |
| [SPEC-06](specs/SPEC-06-wire-protocol.md) | Wire Protocol | 40 reqs |
| [SPEC-07](specs/SPEC-07-deployment.md) | Deployment | 39 reqs |
| [SPEC-08](specs/SPEC-08-test-strategy.md) | Test Strategy | 103 tests |
| [SPEC-09](specs/SPEC-09-benchmarks.md) | Benchmarks | 44 reqs |

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
