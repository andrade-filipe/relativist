# Formal specifications — index

The 28 formal specifications that define Relativist. Under the RPI workflow they are **reference**
(the source of truth for *why* the system is shaped as it is), not a per-change gate. Code comments
cite these by bare ID (`SPEC-03`); this index and [`../architecture/modules.md`](../architecture/modules.md)
resolve IDs to modules and prose docs. Specs are English.

**Status:** *shipped* = defines current behavior (SPEC-00..14 + the implemented parts of 15–22, 27);
*draft* = forward-looking / partially implemented.

## Core theory & algorithm (shipped)

| ID | Title | Module(s) | Owns | Status |
|----|-------|-----------|------|--------|
| [SPEC-00](SPEC-00-glossary.md) | Glossary | all | normative terminology, notation mappings | shipped |
| [SPEC-01](SPEC-01-invariantes.md) | System invariants | net, reduction, partition, merge | T1–T7, D1–D6, I1–I5, G1 | shipped |
| [SPEC-02](SPEC-02-net-representation.md) | Net representation | net | `Symbol`, `Agent`, `PortRef`, `Net`, CRUD, serde | shipped |
| [SPEC-03](SPEC-03-reduction.md) | Reduction engine | reduction | the six rules, dispatch, `reduce_all/n` | shipped |
| [SPEC-04](SPEC-04-partition.md) | Net partitioning | partition | `split`, border classification, ID ranges | shipped |
| [SPEC-05](SPEC-05-merge.md) | Merge & grid cycle | merge | `merge`, `run_grid`, border-redex resolution, BSP | shipped |

## Infrastructure (shipped)

| ID | Title | Module(s) | Owns | Status |
|----|-------|-----------|------|--------|
| [SPEC-06](SPEC-06-wire-protocol.md) | Wire protocol | protocol | messages, framing (len+CRC32), bincode | shipped |
| [SPEC-07](SPEC-07-deployment.md) | Deployment & CLI | config, commands | subcommands, GridConfig, single binary | shipped |
| [SPEC-08](SPEC-08-test-strategy.md) | Test strategy | all (tests) | TDD taxonomy, proptest, isomorphism checks | shipped |
| [SPEC-09](SPEC-09-benchmarks.md) | Benchmark suite | bench | the `Benchmark` trait, profiles, metrics | shipped |
| [SPEC-10](SPEC-10-security.md) | Security | security | 3-tier model, token auth, TLS 1.3 | shipped |
| [SPEC-11](SPEC-11-observability.md) | Observability | observability | tracing, metrics, health endpoints | shipped |
| [SPEC-12](SPEC-12-user-io.md) | User I/O & examples | io | `.bin`/`.ic`/`.json`, generators | shipped |
| [SPEC-13](SPEC-13-system-architecture.md) | System architecture | all | BSP, module structure, FSMs, feature flags | shipped |
| [SPEC-14](SPEC-14-encoding.md) | Arithmetic encoding | encoding | Church numerals, `compute` pipeline | shipped |

## Operations & v2 evolution (draft / partial)

| ID | Title | Module(s) | Status |
|----|-------|-----------|--------|
| [SPEC-15](SPEC-15-distribution.md) | Binary distribution & install | CI, commands | draft (partial) |
| [SPEC-16](SPEC-16-worker-daemon.md) | Worker daemon mode | worker | draft |
| [SPEC-17](SPEC-17-transport-abstraction.md) | Transport abstraction & tuning | protocol | draft (Transport trait shipped) |
| [SPEC-18](SPEC-18-wire-format-v2.md) | Wire format v2 | protocol, net | draft (bincode v2 shipped; LZ4/rkyv opt-in) |
| [SPEC-19](SPEC-19-delta-protocol.md) | Delta protocol & stateful workers | merge, coordinator, worker | draft (opt-in `--delta-mode`) |
| [SPEC-20](SPEC-20-elastic-grid.md) | Elastic grid | coordinator, worker | draft (hybrid shipped; recovery deferred) |
| [SPEC-21](SPEC-21-streaming-generation.md) | Streaming generation & partitioning | partition, io, bench | draft (default-on) |
| [SPEC-22](SPEC-22-arena-management.md) | Arena management & memory | net, merge | draft (free-list + sparse, default-on) |
| [SPEC-23](SPEC-23-compact-memory.md) | Compact memory representation | net | draft |
| [SPEC-24](SPEC-24-wan-deployment.md) | WAN deployment & security | security, protocol | draft |
| [SPEC-25](SPEC-25-recipe-generation.md) | Recipe-based distributed generation | io, coordinator, worker | draft |
| [SPEC-26](SPEC-26-gui-application.md) | GUI desktop application | (new crate) | draft |
| [SPEC-27](SPEC-27-encoder-decoder-api.md) | Encoder/Decoder API & registry | encoding | draft (HornerCodec + registry shipped) |

## See also

- [`../architecture/modules.md`](../architecture/modules.md) — module → spec → doc map.
- [`../theory/invariants.md`](../theory/invariants.md) — the SPEC-01 invariant ladder in prose.
- [`../README.md`](../README.md) — the full documentation catalog.
