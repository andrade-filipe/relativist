# TEST-SPEC-0596 — Tests for TASK-0596 — `CompactSubnet` wire format MUST round-trip `free_list`

**Task:** TASK-0596 (Phase B-1, P0 CRITICAL)
**Spec:** SPEC-19 §3.4 R35a (commit `c4c80b8`); SPEC-22 §3.7 R9a, R10b, R12a; SPEC-18 R31, R33; SPEC-04 §A7.
**Origin:** QA-D009-001 (deferred D-009 Stage 6).
**Test floor delta:** **+8 default / +1 zero-copy** (= +9 total).
**Prerequisites:**
- SPEC commit `c4c80b8` already landed (R35a wire suffix + `PROTOCOL_VERSION` bump from `PREVIOUS_LIVE_VERSION` → `PREVIOUS_LIVE_VERSION + 1`).
- No upstream task dependency.

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0596-01 | unit | `relativist-core/src/partition/compact.rs::tests::round_trip_with_empty_free_list` | none | none |
| UT-0596-02 | unit | `relativist-core/src/partition/compact.rs::tests::round_trip_with_populated_free_list` | none | none |
| UT-0596-03 | unit | `relativist-core/src/partition/compact.rs::tests::round_trip_preserves_free_list_order` | none | none |
| UT-0596-04 | unit | `relativist-core/src/partition/compact.rs::tests::round_trip_with_sparse_non_monotonic_free_list` | none | none |
| UT-0596-05 | unit | `relativist-core/src/partition/compact.rs::tests::nets_equivalent_helper_compares_free_list` | none | none |
| UT-0596-06 | unit | `relativist-core/src/partition/compact.rs::tests::archived_round_trip_preserves_free_list` | none | `#[cfg(feature = "zero-copy")]` |
| UT-0596-07 | unit | `relativist-core/src/protocol/coordinator.rs::tests::protocol_version_constant_matches_spec_19_r35a` | none | none |
| IT-0596-08 | integration | `relativist-core/tests/spec19_compactsubnet_free_list_roundtrip.rs::wire_v3_payload_is_rejected_with_unsupported_version` | none | none |
| PT-0596-09 | property | `relativist-core/tests/spec19_compactsubnet_free_list_roundtrip.rs::proptest_round_trip_arbitrary_net_with_free_list` | none | none |
| IT-0596-10 | integration | `relativist-core/tests/spec19_compactsubnet_free_list_roundtrip.rs::tcp_two_worker_partition_transfer_preserves_free_list` | none | none |
| IT-0596-11 | integration | `relativist-core/tests/spec19_compactsubnet_free_list_roundtrip.rs::regression_witness_pre_r35a_bug_repro` | none | none |

Total: **8 default tests + 1 zero-copy test + 2 cross-cutting (UT-0596-07, IT-0596-08 are not gated)**. Net floor delta: 8 default / 1 zero-copy.

---

## Per-test specifications

### UT-0596-01 — `round_trip_with_empty_free_list`

**Purpose.** Validate that the new `free_list: Vec<AgentId>` field of `CompactSubnet` round-trips when it is empty (R35a empty-vector boundary; protects v4 baseline behavior).
**Setup.** Build a `Net` with at least one CON agent so `agents.len() > 0`; explicitly leave `net.free_list` as the default `Vec::new()`.
**Action.** Call `CompactSubnet::from_net(&net)`, then `.into_net()`. Compare via the (extended) `nets_equivalent` helper.
**Assertions.**
- `compact.free_list == Vec::<AgentId>::new()` (intermediate state).
- `back.free_list.is_empty() == true`.
- `back.free_list.len() == net.free_list.len()` (= 0).
- `nets_equivalent(&net, &back) == true` (the helper now compares `free_list`).
**Boundary case coverage.** Empty Vec must serialize as length-prefix `0` and deserialize back to an empty Vec without error. Catches a future regression where the new field is added but treated as `Option<Vec<_>>` and silently defaulted.
**Why it must exist.** SPEC-19 R35a mandates the suffix is always present, including when empty. SPEC-22 R9a requires byte-for-byte preservation of the recycled-id ledger.

---

### UT-0596-02 — `round_trip_with_populated_free_list` (THE BUG-WITNESS TEST)

**Purpose.** This is the headline regression test — the test that *would have caught* QA-D009-001. Validate that a non-empty `free_list` survives `from_net → into_net`.
**Setup.** Construct a `Net` with `next_id = 10`, three live agents at ids `{0, 4, 9}`, and `net.free_list = vec![AgentId(7), AgentId(3), AgentId(1)]` (representing three previously-recycled tombstone slots).
**Action.** `let compact = CompactSubnet::from_net(&net); let back = compact.into_net();`
**Assertions.**
- `compact.free_list == vec![AgentId(7), AgentId(3), AgentId(1)]`.
- `back.free_list == vec![AgentId(7), AgentId(3), AgentId(1)]`.
- `back.free_list.len() == 3`.
- `back.next_id == AgentId(10)` (next_id consistency, SPEC-22 R10b).
- `nets_equivalent(&net, &back) == true`.
**Boundary case coverage.** Catches the actual D-009 bug: `into_net` previously hard-coded `free_list: Vec::new()`. Without R35a, this assertion fires immediately after the field is added but before the wiring is fixed.
**Why it must exist.** Primary regression for SPEC-19 R35a. Single most important test in this task.

---

### UT-0596-03 — `round_trip_preserves_free_list_order`

**Purpose.** `free_list` is a stack (`Vec` LIFO per SPEC-22 R10c); element order matters for pop semantics. Test must use `assert_eq!` on the full Vec, not `HashSet` equality.
**Setup.** `net.free_list = vec![AgentId(5), AgentId(2), AgentId(8)]` (specifically NOT sorted — order-sensitive).
**Action.** Round-trip via `from_net → into_net`.
**Assertions.**
- `back.free_list == vec![AgentId(5), AgentId(2), AgentId(8)]` (exact order, NOT order-agnostic).
- `back.free_list[0] == AgentId(5)` (top of stack — first to be popped on next `create_agent`).
**Boundary case coverage.** Catches a buggy implementation that sorts or dedups the Vec during serialization.
**Why it must exist.** SPEC-22 R10c (LIFO recycle order) — order is observable behavior; preserves coordinator/worker convergence on the next agent id allocated.

---

### UT-0596-04 — `round_trip_with_sparse_non_monotonic_free_list`

**Purpose.** Stress test with non-contiguous, non-monotonic AgentIds drawn from across the arena (typical post-reduction state).
**Setup.** Build a `Net` with `agent_arena_len ≥ 100`; `free_list = vec![AgentId(97), AgentId(2), AgentId(50), AgentId(13)]`.
**Action.** Round-trip.
**Assertions.**
- `back.free_list == vec![AgentId(97), AgentId(2), AgentId(50), AgentId(13)]`.
- All ids in the round-tripped `free_list` are `< back.agents.len() as AgentId` (no out-of-arena ids fabricated).
**Boundary case coverage.** Catches a buggy implementation that uses `free_list.iter().min()` or treats `free_list` as a sorted set.
**Why it must exist.** Realistic post-reduction state (after several CON-CON annihilations) has scattered tombstone ids.

---

### UT-0596-05 — `nets_equivalent_helper_compares_free_list`

**Purpose.** Verify that the test-helper at `compact.rs:152-158` is extended to include `a.free_list == b.free_list` in its conjunction. Without this extension, all subsequent free_list tests would silently pass even with a broken implementation.
**Setup.** Two `Net` instances `a` and `b` identical in every field EXCEPT `free_list`: `a.free_list = vec![AgentId(1)]`, `b.free_list = vec![]`.
**Action.** `let eq = nets_equivalent(&a, &b);`
**Assertions.**
- `eq == false` (the helper must distinguish them).
**Boundary case coverage.** Without this test, the helper could be left untouched and UT-0596-01..04 might still report green. This is the meta-test that guards the test suite itself.
**Why it must exist.** Test-suite integrity gate. The plan explicitly calls out this helper extension as a prerequisite; this test enforces it.

---

### UT-0596-06 — `archived_round_trip_preserves_free_list` (zero-copy)

**Purpose.** Validate the rkyv `Archive`/`Serialize`/`Deserialize` derives on `CompactSubnet` actually carry the new field through the archived form (not just the bincode form).
**Setup.** Same `Net` as UT-0596-02 (free_list = [7, 3, 1]). Build `compact = CompactSubnet::from_net(&net)`.
**Action.** `let bytes = rkyv::to_bytes::<_, 1024>(&compact).unwrap(); let archived = unsafe { rkyv::archived_root::<CompactSubnet>(&bytes) }; let back: CompactSubnet = archived.deserialize(&mut rkyv::Infallible).unwrap();`
**Assertions.**
- `back.free_list == vec![AgentId(7), AgentId(3), AgentId(1)]`.
- `back.into_net().free_list == net.free_list`.
**Boundary case coverage.** rkyv archived layouts can drift from bincode if a field is added without re-deriving. This guards SPEC-18 R33.
**cfg gate.** `#[cfg(feature = "zero-copy")]`. Counts toward zero-copy floor only.
**Why it must exist.** SPEC-18 R31/R33 require both bincode and rkyv round-trips to remain symmetric.

---

### UT-0596-07 — `protocol_version_constant_matches_spec_19_r35a`

**Purpose.** Lock the `PROTOCOL_VERSION` constant in `relativist-core/src/protocol/coordinator.rs` to the spec-mandated value (`PREVIOUS_LIVE_VERSION + 1` per commit `c4c80b8`).
**Setup.** None (constant test).
**Action.** Read the public/`pub(crate)` constant (`coordinator::PROTOCOL_VERSION`).
**Assertions.**
- `PROTOCOL_VERSION` literal equals the value committed in `c4c80b8` (the spec text says `PREVIOUS_LIVE_VERSION + 1`; concrete value per the spec amendment — developer must consult the spec at implementation time and assert the exact integer).
- A doc-comment test (or comment grep, but better: a static assertion in code) cites SPEC-19 R35a + commit `c4c80b8`.
**Boundary case coverage.** Catches a silent revert of the version bump.
**Why it must exist.** Without the version bump, a v3 worker speaking to a v4 coordinator would deserialize wire frames missing `free_list` and silently produce a divergent `next_id`. The version bump is the protective fence.

---

### IT-0596-08 — `wire_v3_payload_is_rejected_with_unsupported_version`

**Purpose.** Verify that a wire payload encoded with the pre-R35a layout (no `free_list` suffix; advertised as `PROTOCOL_VERSION = PREVIOUS_LIVE_VERSION`) is rejected by the v4 deserializer with `ProtocolError::UnsupportedVersion` (or the project's equivalent error variant).
**Setup.** Construct a synthetic byte buffer mimicking the v3 envelope: header with `version = PREVIOUS_LIVE_VERSION`, body = bincode-encoded payload of a `CompactSubnet`-shaped struct without `free_list`.
**Action.** Pass the buffer to the v4 protocol decode entry point (e.g. `decode_message(&bytes)`).
**Assertions.**
- Returns `Err(ProtocolError::UnsupportedVersion { received: PREVIOUS_LIVE_VERSION, expected: PREVIOUS_LIVE_VERSION + 1 })` (or equivalent error variant + payload).
- Does NOT panic.
- Does NOT silently default `free_list = Vec::new()`.
**Boundary case coverage.** Catches a buggy implementation that bumps the version constant but forgets to validate it on the receive path.
**Why it must exist.** SPEC-19 R35a marks the change as backward-INCOMPATIBLE; the receive-path version gate is the contract enforcement.

---

### PT-0596-09 — `proptest_round_trip_arbitrary_net_with_free_list`

**Purpose.** Property-test on randomly-generated nets with mixed empty/populated `free_list` to surface untested combinations.
**Generator strategy.**
```
arb_agent_id() -> AgentId in 0..1024
arb_arena_len() -> u32 in 1..=128
arb_live_count() -> usize in 0..=arena_len
arb_free_list() -> Vec<AgentId> with len in 0..=16, ids drawn from 0..arena_len
                                  AND disjoint from the live-agent id set
arb_net() -> Net with the above, plus arbitrary Symbol per agent and DISCONNECTED ports
```
**Property.** For all generated `net`: `nets_equivalent(&net, &CompactSubnet::from_net(&net).into_net()) == true`.
**Specific sub-assertions.**
- `back.free_list == net.free_list` (Vec equality, order-preserving).
- `back.free_list.len() == net.free_list.len()`.
- `back.next_id == net.next_id`.
**Shrinking note.** Minimal counterexample should narrow to either (a) the smallest Net where free_list is non-empty, or (b) an order-permutation where the original passes element-set but fails Vec equality. Both shrink shapes diagnose distinct bugs.
**Why it must exist.** Catches edge cases not enumerated by UT-0596-01..04 (e.g., free_list with duplicates, free_list pointing at currently-live agents — both should be rejected upstream by `Net` invariants but the round-trip itself must not corrupt input).

---

### IT-0596-10 — `tcp_two_worker_partition_transfer_preserves_free_list`

**Purpose.** End-to-end integration: spawn an in-process coordinator + 2 in-process workers over `tokio::net::TcpStream` (or the project's existing test scaffold for TCP partition transfer), have the coordinator send a `Partition` whose `subnet.free_list = [AgentId(7), AgentId(3), AgentId(1)]`, and assert the worker reconstructs it intact.
**Setup.**
- Coordinator-side: build a `Partition` with `subnet.free_list = vec![AgentId(7), AgentId(3), AgentId(1)]`.
- Use the existing test helpers under `relativist-core/tests/` for in-memory TCP loopback (look for the SPEC-19 / SPEC-04 partition-transfer harness).
**Action.** Coordinator sends; worker receives and decodes the `Message::PartitionAssignment` (or equivalent).
**Assertions.**
- `received_partition.subnet.free_list == vec![AgentId(7), AgentId(3), AgentId(1)]`.
- `received_partition.subnet.next_id == sent_partition.subnet.next_id`.
- Coordinator and worker agree on the next id that `create_agent` would allocate (pop the top of `free_list` first → `AgentId(7)` on both sides).
**Boundary case coverage.** This is the integration validation that `from_net → bincode → TCP → bincode → into_net` is loss-free across the actual protocol stack, not just the in-process serde round-trip.
**Why it must exist.** Acceptance criterion #5 of TASK-0596 ("`next_id` is consistent across coordinator and worker after partition transfer (SPEC-22 R10b/R12a)") — IT-0596-10 is the only test that exercises the full TCP path.

---

### IT-0596-11 — `regression_witness_pre_r35a_bug_repro`

**Purpose.** The "test that would have caught the bug" specified in the D-011 plan. After the fix lands, this test must pass; before the fix, it must fail. It is the historical witness for QA-D009-001.
**Setup.** Same as IT-0596-10 but minimal — a single-hop in-process serde round-trip is sufficient (no TCP harness required). Use `bincode::serialize` + `bincode::deserialize` on a `Partition` whose `subnet.free_list = vec![AgentId(42)]`.
**Action.** Serialize → deserialize via the actual `Partition`'s `serialize_with` / `deserialize_with` adapters in `partition/types.rs`.
**Assertions.**
- `back.subnet.free_list == vec![AgentId(42)]`.
- A comment block in the test cites: "QA-D009-001; SPEC-19 R35a (commit c4c80b8); without the R35a fix this test fails because `into_net` hard-codes `free_list: Vec::new()`."
**Boundary case coverage.** Single-element free_list is the smallest non-empty case; if even this fails, the wire format is broken at the most fundamental level.
**Why it must exist.** Acceptance criterion #4 of TASK-0596 + the plan's explicit instruction to author the witness test. Documents the bug history in the regression suite.

---

## Coverage matrix

| test_id | R35a | R9a | R10b | R12a | R10c | R31 | R33 |
|---|---|---|---|---|---|---|---|
| UT-0596-01 | ✅ | ✅ | | | | | |
| UT-0596-02 | ✅ | ✅ | ✅ | ✅ | | | |
| UT-0596-03 | ✅ | ✅ | | | ✅ | | |
| UT-0596-04 | ✅ | ✅ | | | | | |
| UT-0596-05 | ✅ | | | | | | |
| UT-0596-06 | ✅ | ✅ | | | | ✅ | ✅ |
| UT-0596-07 | ✅ | | | | | | |
| IT-0596-08 | ✅ | | | | | ✅ | |
| PT-0596-09 | ✅ | ✅ | | | | | |
| IT-0596-10 | ✅ | ✅ | ✅ | ✅ | | | |
| IT-0596-11 | ✅ | ✅ | | | | | |

Every R has ≥1 test. R35a has 11/11.

---

## Out-of-scope tests (deferred to other tasks)

- Tests that exercise `max_pending_lifetime` propagation through the legacy generator API → **TASK-0597**.
- Tests that exercise the bench harness reading the `free_list`-aware partition during CSV-emitting runs → **TASK-0604** + smoke under **TASK-0610**.
- Tests of `SparseNet::to_dense` free_list handling → out of scope (sparse path is not the wire path; explicit per task §Files OUT of scope).
- Tests of cross-version downgrade (v4 → v3 worker) → out of scope; SPEC-19 R35a marks the change backward-incompatible — no shim.
