# TEST-SPEC-0353: rkyv `Archive`/`Serialize`/`Deserialize` derives on 8 types

**Task:** TASK-0353
**Spec:** SPEC-18 §3.5 (R21), §4.6
**Generated:** 2026-04-16
**Baseline before this task:** 887 lib (default) / 890 lib (`--features zero-copy`, post-TASK-0352).

---

## Scope note

R21 enumerates 8 types that MUST gain the rkyv derives so that the
hot-path payload (and every type transitively reachable from it) can be
archived. Each per-type test below produces a representative instance,
calls `rkyv::to_bytes` → `rkyv::access` → `rkyv::deserialize`, and
asserts equality with the original.

The 9th test (UT-09) is the dual-build coexistence guarantee: the
serde-bincode v2 path, which is the only path used by control messages
and by the entire default build, MUST stay green when the new derives
are present. Operationally this means R21 derives MUST NOT shadow or
collide with the existing `Serialize`/`Deserialize` impls — the rkyv
namespace (`rkyv::Serialize`, `rkyv::Deserialize`) is distinct from the
serde namespace (`serde::Serialize`, `serde::Deserialize`).

All 8 round-trip tests below are gated under
`#[cfg(feature = "zero-copy")]`; the coexistence test runs in both builds.

---

## UT-0353-01: `Net` round-trips through rkyv

**Target file:** `relativist-core/src/net/core.rs` (same `mod tests` as
existing Net tests).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (Net).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_net_minimal() {
    let mut net = Net::new();
    let id = net.add_agent(Symbol::Constructor);
    // Connect the principal port to itself's auxiliary 1 to make it non-trivial.
    net.connect(PortRef::AgentPort(id, 0), PortRef::AgentPort(id, 1));
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&net)
        .expect("Net serializes via rkyv");
    let archived = rkyv::access::<rkyv::Archived<Net>, rkyv::rancor::Error>(
        bytes.as_slice(),
    )
    .expect("rkyv::access on Net");
    let round: Net = rkyv::deserialize::<Net, rkyv::rancor::Error>(archived)
        .expect("rkyv::deserialize on Net");
    assert_eq!(net, round, "Net round-trip identity (R21+R27 precondition)");
}
```

**Asserts:** `original == round`.

---

## UT-0353-02: `Partition` round-trips through rkyv

**Target file:** `relativist-core/src/partition/types.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (Partition).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_partition_with_free_ports() {
    // Build a Partition with at least one IdRange and at least one entry
    // in free_port_index (HashMap<u32, PortRef>).
    let p = sample_partition_with_borders();
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&p)
        .expect("Partition serializes via rkyv");
    let archived = rkyv::access::<rkyv::Archived<Partition>, rkyv::rancor::Error>(
        bytes.as_slice(),
    )
    .expect("rkyv::access on Partition");
    let round: Partition = rkyv::deserialize::<Partition, rkyv::rancor::Error>(
        archived,
    )
    .expect("rkyv::deserialize on Partition");
    assert_eq!(p, round, "Partition round-trip identity (R21+R27)");
    assert_eq!(p.free_port_index, round.free_port_index,
        "free_port_index HashMap preserves entries");
}
```

**Fixture:** `sample_partition_with_borders()` — a `pub(crate)` helper to
be added (or reused if it exists) in `partition/types.rs`'s test module
that builds a Partition with ≥1 IdRange and ≥1 free_port_index entry.

---

## UT-0353-03: `CompactSubnet` round-trips through rkyv

**Target file:** `relativist-core/src/partition/compact.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (CompactSubnet — explicit per R21 even though §4.6
notes the rkyv hot path bypasses it).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_compact_subnet_minimal() {
    let cs = CompactSubnet::from_partition(&sample_partition_with_borders());
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&cs)
        .expect("CompactSubnet serializes via rkyv");
    let archived = rkyv::access::<rkyv::Archived<CompactSubnet>, rkyv::rancor::Error>(
        bytes.as_slice(),
    )
    .expect("rkyv::access on CompactSubnet");
    let round: CompactSubnet = rkyv::deserialize::<CompactSubnet, rkyv::rancor::Error>(
        archived,
    )
    .expect("rkyv::deserialize on CompactSubnet");
    assert_eq!(cs, round, "CompactSubnet round-trip identity (R21)");
}
```

---

## UT-0353-04: `Agent` round-trips through rkyv

**Target file:** `relativist-core/src/net/types.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (Agent).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_agent_each_symbol() {
    for symbol in [Symbol::Constructor, Symbol::Duplicator, Symbol::Eraser] {
        let agent = Agent { id: AgentId(42), symbol, ports: [
            PortRef::Disconnected,
            PortRef::AgentPort(AgentId(1), 0),
            PortRef::FreePort(7),
        ]};
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&agent)
            .expect("Agent serializes via rkyv");
        let archived = rkyv::access::<rkyv::Archived<Agent>, rkyv::rancor::Error>(
            bytes.as_slice(),
        )
        .expect("rkyv::access on Agent");
        let round: Agent = rkyv::deserialize::<Agent, rkyv::rancor::Error>(archived)
            .expect("rkyv::deserialize on Agent");
        assert_eq!(agent, round, "Agent round-trip for symbol {:?}", symbol);
    }
}
```

**Asserts:** all 3 Symbol variants round-trip on the same Agent shell.

---

## UT-0353-05: `Symbol` round-trips through rkyv

**Target file:** `relativist-core/src/net/types.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (Symbol).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_symbol_exhaustive() {
    for s in [Symbol::Constructor, Symbol::Duplicator, Symbol::Eraser] {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&s)
            .expect("Symbol serializes via rkyv");
        let archived = rkyv::access::<rkyv::Archived<Symbol>, rkyv::rancor::Error>(
            bytes.as_slice(),
        )
        .expect("rkyv::access on Symbol");
        let round: Symbol = rkyv::deserialize::<Symbol, rkyv::rancor::Error>(archived)
            .expect("rkyv::deserialize on Symbol");
        assert_eq!(s, round, "Symbol::{:?} round-trip", s);
    }
}
```

---

## UT-0353-06: `PortRef` round-trips through rkyv (all 3 variants)

**Target file:** `relativist-core/src/net/types.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (PortRef — coexistence with manual serde impls).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_portref_all_three_variants() {
    for pr in [
        PortRef::Disconnected,
        PortRef::AgentPort(AgentId(0), 0),
        PortRef::AgentPort(AgentId(u32::MAX), 2),
        PortRef::FreePort(0),
        PortRef::FreePort(u32::MAX),
    ] {
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&pr)
            .expect("PortRef serializes via rkyv");
        let archived = rkyv::access::<rkyv::Archived<PortRef>, rkyv::rancor::Error>(
            bytes.as_slice(),
        )
        .expect("rkyv::access on PortRef");
        let round: PortRef = rkyv::deserialize::<PortRef, rkyv::rancor::Error>(archived)
            .expect("rkyv::deserialize on PortRef");
        assert_eq!(pr, round, "PortRef {:?} round-trip", pr);
    }
}
```

**Asserts:** rkyv encoding is independent of the manual serde
impls (TASK-0344 R5-R8); both encodings coexist for `PortRef`.

---

## UT-0353-07: `IdRange` round-trips through rkyv

**Target file:** `relativist-core/src/partition/types.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (IdRange).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_id_range() {
    let range = IdRange { start: AgentId(0), end: AgentId(1024) };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&range)
        .expect("IdRange serializes via rkyv");
    let archived = rkyv::access::<rkyv::Archived<IdRange>, rkyv::rancor::Error>(
        bytes.as_slice(),
    )
    .expect("rkyv::access on IdRange");
    let round: IdRange = rkyv::deserialize::<IdRange, rkyv::rancor::Error>(archived)
        .expect("rkyv::deserialize on IdRange");
    assert_eq!(range, round, "IdRange round-trip identity (R21)");
}
```

---

## UT-0353-08: `WorkerRoundStats` round-trips through rkyv

**Target file:** `relativist-core/src/merge/types.rs` (test module).
**Feature gate:** `#[cfg(feature = "zero-copy")]`.
**R-mapping:** R21 (WorkerRoundStats).

```rust
#[cfg(feature = "zero-copy")]
#[test]
fn rkyv_round_trip_worker_round_stats() {
    let stats = WorkerRoundStats {
        worker_id: WorkerId(3),
        round: 42,
        local_redexes: 17,
        interactions_by_rule: [10, 7, 0, 0, 0, 0],
        reduce_duration_secs: 0.123_456_789_f64,
        has_border_activity: true,
    };
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&stats)
        .expect("WorkerRoundStats serializes via rkyv");
    let archived = rkyv::access::<rkyv::Archived<WorkerRoundStats>, rkyv::rancor::Error>(
        bytes.as_slice(),
    )
    .expect("rkyv::access on WorkerRoundStats");
    let round: WorkerRoundStats = rkyv::deserialize::<WorkerRoundStats, rkyv::rancor::Error>(
        archived,
    )
    .expect("rkyv::deserialize on WorkerRoundStats");
    assert_eq!(stats, round, "WorkerRoundStats round-trip identity (R21)");
    // f64 cross-endian probe: rend::f64_le must yield bit-identical f64.
    assert_eq!(stats.reduce_duration_secs.to_bits(),
               round.reduce_duration_secs.to_bits(),
               "f64 must round-trip bit-identical");
}
```

**Asserts:** all 6 fields, including the `[u64; 6]` rule histogram, the
`f64`, and the `has_border_activity` bool from SPEC-19 §3.1.

---

## UT-0353-09: serde bincode v2 path stays green with rkyv derives present

**Target file:** `relativist-core/src/protocol/frame.rs` (test module).
**Feature gate:** none — runs in both `default` and
`--features zero-copy` builds.
**R-mapping:** R21 (coexistence — derives MUST NOT shadow serde impls).

```rust
#[test]
fn serde_bincode_v2_path_unaffected_by_rkyv_derives() {
    // Round-trip every hot-path Message variant through bincode v2 to
    // prove the new rkyv derives don't collide with the serde impls.
    // This test runs in BOTH builds (it does not require the rkyv
    // dependency, only that the existing serde derives still produce
    // the same bytes).
    let messages = vec![
        Message::AssignPartition {
            round: 1,
            partition: sample_partition_with_borders(),
        },
        Message::PartitionResult {
            round: 1,
            partition: sample_partition_with_borders(),
            stats: WorkerRoundStats {
                worker_id: WorkerId(0),
                round: 1,
                local_redexes: 0,
                interactions_by_rule: [0; 6],
                reduce_duration_secs: 0.0,
                has_border_activity: false,
            },
        },
        Message::Shutdown,
    ];
    for msg in &messages {
        let bytes = crate::protocol::bincode_v2::encode(msg)
            .expect("bincode v2 encode");
        let round: Message = crate::protocol::bincode_v2::decode_value(&bytes)
            .expect("bincode v2 decode");
        assert_eq!(msg, &round, "bincode round-trip for {:?}", msg);
    }
}
```

**Asserts:** the bincode v2 encoding for every hot-path AND the
representative control variant `Shutdown` round-trips. Test is
unconditional so a regression in either build is caught.

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0353-01..08 | ⏭ skipped (cfg-gated) | ✅ runs (8 tests) |
| UT-0353-09 | ✅ runs | ✅ runs |
| **Total new tests** | **+1** | **+9** |

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0353-A | Empty `Net` (no agents) → rkyv round-trip | Empty Vec arena edge case | QA |
| QA-0353-B | `Partition` with 1000-entry `free_port_index` | Stress HashMap rkyv archiving | QA |
| QA-0353-C | `Agent::ports` containing 3 × `PortRef::Disconnected` | All-Disconnected variant array | QA |
| QA-0353-D | `WorkerRoundStats` with `f64::NAN` for `reduce_duration_secs` | NaN bit pattern preservation through rend::f64_le | QA |
| QA-0353-E | `Symbol` deserialized from a buffer where the discriminant byte is invalid | Validating API must reject (R26 precondition) | QA |
| QA-0353-F | `IdRange { start: AgentId(u32::MAX), end: AgentId(0) }` (inverted, semantically invalid) | rkyv round-trips bytes; semantic check is elsewhere — confirm rkyv does not over-validate | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- None. All 9 tests above are deterministic CPU-bound serialization
  round-trips with no I/O, no timing, no platform-specific behavior.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 889 → **890** (+1: UT-09).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   890 → **899** (+9: UT-01..09).
3. Existing serde round-trip tests for `Partition`, `Net`, `PortRef`,
   `WorkerRoundStats` etc. all stay green in BOTH builds.
4. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
5. `cargo fmt --check` clean.
6. Bundle floor: 887 lib tests in default build (CLAUDE.md hard floor).

---

## Out of scope

- `ProtocolError::ArchiveValidationFailed` variant → TEST-SPEC-0354.
- Aligned receive buffer → TEST-SPEC-0355.
- `send_frame` archive path → TEST-SPEC-0356.
- `recv_frame` archive path → TEST-SPEC-0357.
- CLI `--use-zero-copy` flag → TEST-SPEC-0358.
- T11-T14 end-to-end suite → TEST-SPEC-0359.
