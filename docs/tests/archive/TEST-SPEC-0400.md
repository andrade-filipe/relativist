# TEST-SPEC-0400: Wire struct rewrite — `PendingCommutation` Shape A + `LocalWiringHint` + `ProtocolError::MalformedLocalWiring` + `PROTOCOL_VERSION` bump

**See also:** [docs/backlog/TASK-0400.md](../backlog/TASK-0400.md); [specs/SPEC-19-delta-protocol.md](../../specs/SPEC-19-delta-protocol.md) §3.3 R23/R23a, §3.4 R33/R33c/R34, §3.6 R48/R48a/R48b, §3.4 R37, §9 Change Log.

**Task:** TASK-0400
**Parent spec:** SPEC-19 Delta-Only Protocol
**Bundle:** D-005 Option A — worker-side application of `CommutationBatch.local_wiring` for minted agents (production, wire-level).
**Date:** 2026-04-23
**Spec-critic verdicts consumed:**
- SPEC-REVIEW-19-section-3.4-D-005-2026-04-23-REREVIEW-R3 SIGN-OFF (NR3 LOW absorbed: NR3-001 docstring patch, NR3-002 error-surface distinction, NR3-003 DEFER by default).
**Baseline before this task:** 1146 lib default / 1186 lib `--features zero-copy`.
**Cumulative target after this task:** **≥ 1152 / ≥ 1195** (+6 default, +9 zero-copy).

---

## Scope

### Covers
1. Shape A bincode round-trip for `PendingCommutation` (all 3 characteristic shapes: minimal ERA, heterogeneous CON-DUP, max-cap homogeneous).
2. `LocalWiringHint` bincode round-trip (standalone).
3. `MintedAgent` bincode round-trip regression canary (rkyv derives are additive on a pre-existing struct — ensure default path remains correct).
4. Pre-Shape-A blob rejection (R37: bincode intolerance to a new trailing `Vec<LocalWiringHint>` field without a version gate).
5. `MalformedLocalWiringReason` 7-case serde round-trip.
6. `PROTOCOL_VERSION == 3` compile-time sentinel.
7. rkyv zero-copy round-trip for `PendingCommutation` (Shape A) and `LocalWiringHint`.
8. rkyv `bytecheck`/`CheckBytes` corruption rejection (SPEC-18 Q1 contract extension).
9. NR3-001/002/003 absorption — docstring content anchors (one docstring anchor per NR3 finding, asserted via `include_str!` regex or plain string search on the source file — optional, see §OPTIONAL).

### Does NOT cover
- Resolver-side population of `target_symbols`/`local_wiring` (TEST-SPEC-0401).
- Worker-side decoding and R33c case dispatch (TEST-SPEC-0402).
- End-to-end G1 parity on asymmetric rules (TEST-SPEC-0403).
- `PROTOCOL_VERSION` handshake NACK live-session behavior — existing coordinator-handshake tests cover this pathway (Q1/Q2 at `protocol/coordinator.rs` L66-81). TEST-SPEC-0400 adds a version-mismatch round-trip test only if the existing test doesn't already cover v2-vs-v3 explicitly; see UT-0400-05 note.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — inline `#[cfg(test)] mod tests` block. New `#[test]` fns: UT-0400-01, 02, 03, 04, 05, 06.
- `relativist-core/src/protocol/zero_copy_tests.rs` — new `#[test]` fns (gated `#[cfg(feature = "zero-copy")]`): UT-0400-07, 08, 09.

All tests synchronous. No tokio, no async.

---

## Unit Tests

### UT-0400-01: `pending_commutation_bincode_roundtrip_shape_a_heterogeneous_con_dup`

**Purpose:** Round-trip a Shape A `PendingCommutation` representing the heterogeneous CON-DUP case (the exact shape that pre-NF-001 silently corrupted). Verifies that the new `target_symbols: Vec<Symbol>` field preserves per-slot symbols across serialize + deserialize.

**Target:** `border_graph.rs::tests`.

**Given:** Shape A PC carrying two sibling symbols (Dup, Con), one slot-marker placeholder hint, one concrete-id hint.
```
let slot_marker_base: u32 = SLOT_MARKER_BASE; // import from border_resolver
let pc_in = PendingCommutation {
    request_id: 0x1234_5678,
    target_symbols: vec![Symbol::Dup, Symbol::Con],
    local_wiring: vec![
        // Slot-marker placeholder: slot 0 aux-1 → slot 1 aux-2
        LocalWiringHint {
            src_slot: 0,
            src_port: 1,
            target: PortRef::AgentPort(slot_marker_base + 1, 2),
        },
        // Concrete id: slot 1 aux-1 → concrete id 17 port 0
        LocalWiringHint {
            src_slot: 1,
            src_port: 1,
            target: PortRef::AgentPort(17, 0),
        },
    ],
};
```

**When:** `let bytes = bincode::serialize(&pc_in)?; let pc_out: PendingCommutation = bincode::deserialize(&bytes)?;`.

**Then:**
- `pc_out == pc_in` (derived `PartialEq`).
- `pc_out.target_symbols.len() == 2` and `pc_out.target_symbols[0] == Symbol::Dup` and `pc_out.target_symbols[1] == Symbol::Con` (assert order preservation explicitly — the NF-001 anchor).
- `pc_out.local_wiring.len() == 2`, with element-wise equality on `(src_slot, src_port, target)`.
- The first hint's target decodes as `AgentPort(x, _)` with `x >= SLOT_MARKER_BASE` (placeholder pass-through; no coercion).
- The second hint's target decodes as `AgentPort(17, 0)` (concrete-id pass-through).

**Edge cases:**
- EC-1: `request_id = 0` — minimum value, valid correlation anchor.
- EC-2: `request_id = u32::MAX` — boundary value; verify no overflow in bincode variant discriminant.

**SPEC covered:** R23, R33, R34.

**Priority:** MANDATORY (P0 for Shape A fidelity).

---

### UT-0400-02: `pending_commutation_bincode_roundtrip_minimal_era`

**Purpose:** Round-trip a minimal-arity PC (`target_symbols = [Era]`, empty `local_wiring`) — the R48b "empty legal" canonical case. Also stresses the lower bound of the Shape A discriminator (single-symbol vector).

**Target:** `border_graph.rs::tests`.

**Given:**
```
let pc_in = PendingCommutation {
    request_id: 42,
    target_symbols: vec![Symbol::Era],
    local_wiring: vec![],  // R48b empty legal
};
```

**When:** bincode serialize → deserialize.

**Then:**
- `pc_out == pc_in`.
- `pc_out.target_symbols == vec![Symbol::Era]`.
- `pc_out.local_wiring.is_empty()`.

**Edge cases:**
- EC-1: confirm `Vec<Symbol>` with single element uses bincode length prefix 1 (implementation-consistency test; check `bytes.len()` upper bound: `4 (request_id) + 8 (target_symbols length u64) + 1 (Symbol::Era u8) + 8 (local_wiring length u64) = 21`; relax upper bound to `<= 24` to tolerate bincode option-discriminant variations).
- EC-2: re-deserialize a second time into a fresh buffer — determinism of bincode encoding.

**SPEC covered:** R33 shape, R48b empty-legal.

**Priority:** MANDATORY.

---

### UT-0400-03: `pending_commutation_bincode_roundtrip_max_cap_homogeneous_16`

**Purpose:** Stress the resolver-side 16-symbol cap (`border_resolver.rs:318-322`) at the wire layer. Ensures the upper boundary of the Shape A vector is accepted by bincode and does not collide with any size-bound bincode heuristic.

**Target:** `border_graph.rs::tests`.

**Given:**
```
let pc_in = PendingCommutation {
    request_id: 100,
    target_symbols: vec![Symbol::Con; 16],  // exactly 16, the cap
    local_wiring: vec![],  // keep local_wiring out of this test (tested elsewhere)
};
```

**When:** bincode serialize → deserialize.

**Then:**
- `pc_out == pc_in`.
- `pc_out.target_symbols.len() == 16`.
- All 16 elements equal `Symbol::Con`.

**Edge cases:**
- EC-1: `target_symbols = vec![Symbol::Con; 17]` → round-trip still succeeds (wire layer does NOT enforce the cap; only the resolver does). Confirms R33c case 8 `TargetSymbolsTooLong` is NOT wire-enforced if NR3-003 was DEFERRED. If NR3-003 was shipped at TASK-0400, this EC flips to assert a worker-side `MalformedLocalWiringReason::TargetSymbolsTooLong` rejection — ONLY relevant at TEST-SPEC-0402.
- EC-2: `target_symbols = vec![Symbol::Era; 16]` (all ERA, max cap) — symmetry check.

**SPEC covered:** R33 shape cap (informational, enforced at resolver not wire).

**Priority:** OPTIONAL (R2 defensive — the resolver-side cap is already asserted; this is belt-and-braces). Developer MAY skip if time-pressured; recommend INCLUDE.

---

### UT-0400-04: `malformed_local_wiring_reason_all_seven_cases_serde_roundtrip`

**Purpose:** Every `MalformedLocalWiringReason` variant (7 cases, optionally 8 if NR3-003 shipped) survives a serde round-trip wrapped in the `ProtocolError::MalformedLocalWiring { request_id, reason }` variant. Pins the error-surface contract for R33c dispatch.

**Target:** `border_graph.rs::tests` (or `protocol/error.rs::tests`).

**Given:** 7 `MalformedLocalWiringReason` instances, one per case:
```
let cases = vec![
    MalformedLocalWiringReason::SrcSlotOutOfRange   { src_slot: 5, symbol_count: 2 },
    MalformedLocalWiringReason::SrcPortOutOfRange   { src_slot: 0, src_port: 7 },
    MalformedLocalWiringReason::TargetSiblingOutOfRange { sibling_slot: 99, symbol_count: 2 },
    MalformedLocalWiringReason::DanglingConcreteAgent  { agent_id: 123_456, port: 1 },
    MalformedLocalWiringReason::DuplicateSourcePort    { src_slot: 0, src_port: 1 },
    MalformedLocalWiringReason::ReservedForFuture      { border_id: 42 },
    MalformedLocalWiringReason::ZeroArity,
    // If NR3-003 shipped:
    // MalformedLocalWiringReason::TargetSymbolsTooLong { len: 20 },
];
```

**When:** For each `reason`:
```
let err_in  = ProtocolError::MalformedLocalWiring { request_id: 42, reason };
let bytes   = bincode::serialize(&err_in)?;
let err_out: ProtocolError = bincode::deserialize(&bytes)?;
```

**Then:**
- `err_out == err_in` via derived `PartialEq`.
- `matches!(err_out, ProtocolError::MalformedLocalWiring { request_id: 42, .. })`.
- Each specific `reason` variant pattern-matches identically on both sides.
- `err_in.to_string()` (via `thiserror`'s `#[error]`) contains both "malformed local_wiring" and the embedded `request_id=42` (per TASK-0400 §4 format string).

**Edge cases:**
- EC-1: `request_id = 0` — minimum value.
- EC-2: `request_id = u32::MAX` — boundary.
- EC-3 (NR3-003 gated): if `TargetSymbolsTooLong { len: 0 }` is semantically contradictory (len 0 is ZeroArity's territory), assert the variant is constructible but logically nonsensical and document the choice; the developer MAY simply not test this collision point.

**SPEC covered:** R33c cases 1-7 (optionally 8), NR3-002 absorption.

**Priority:** MANDATORY (every R33c case must have a round-trip probe).

---

### UT-0400-05: `protocol_version_equals_three_sentinel`

**Purpose:** Compile-time sentinel against accidental re-bump or revert of `PROTOCOL_VERSION`. Pins the v2→v3 change for the duration of the D-005 bundle window and downstream.

**Target:** `border_graph.rs::tests` (or `protocol/coordinator.rs::tests`).

**Given:** The `const PROTOCOL_VERSION: u32` at `protocol/coordinator.rs:570`.

**When:**
```
use crate::protocol::coordinator::PROTOCOL_VERSION;
```

**Then:**
- `assert_eq!(PROTOCOL_VERSION, 3);`.
- The const is visible (re-exported or `pub(crate)` as needed for tests).

**Edge cases:**
- EC-1 (NF-002 bidirectionality): verify an existing handshake-mismatch test at `protocol/coordinator.rs:66-81` already covers v2-worker-vs-v3-coordinator NACK. If missing, add a dedicated UT-0400-05b `handshake_v2_worker_rejected_by_v3_coordinator` exercising the `HandshakeAck` path. **Default plan: rely on existing path; INCLUDE UT-0400-05b only if grep confirms no such test.**

**SPEC covered:** R37 version bump, NF-002 bidirectionality.

**Priority:** MANDATORY (trivial sentinel; one line of protection against accidental rebump).

---

### UT-0400-06: `minted_agent_bincode_roundtrip_regression_canary`

**Purpose:** `MintedAgent` is amended with rkyv derives in TASK-0400 §3. The derive macro may theoretically reorder attributes; this test pins the bincode round-trip is byte-compatible with pre-amendment behavior.

**Target:** `border_graph.rs::tests`.

**Given:**
```
let ma_in = MintedAgent {
    request_id: 0xABCD_1234,
    minted_agent_id: 42u32,  // AgentId = u32
};
```

**When:** bincode serialize → deserialize.

**Then:**
- `ma_out == ma_in`.
- `bytes.len() == 8` (4 bytes `request_id` + 4 bytes `minted_agent_id`; bincode default `fixint` mode).

**Edge cases:**
- EC-1: `minted_agent_id = 0` (worker-side range lower bound — irrelevant here, ensures no special-case encoding).
- EC-2: `minted_agent_id = u32::MAX - 10_000` (coordinator-reserved range lower boundary — wire-level round-trip is indifferent; range semantics are enforced elsewhere).

**SPEC covered:** R34 (rkyv derive propagation is additive; default bincode path unchanged).

**Priority:** MANDATORY (regression canary for the rkyv derive propagation; cheap insurance).

---

### UT-0400-07: `pending_commutation_rkyv_access_roundtrip_shape_a` `#[cfg(feature = "zero-copy")]`

**Purpose:** `rkyv::access::<ArchivedPendingCommutation, _>(&serialized)` returns `Ok` and the archived view exposes `target_symbols` and `local_wiring` in byte-identical form.

**Target:** `protocol/zero_copy_tests.rs` (follow the Q1-validated pattern used for other zero-copy structs).

**Given:** Same heterogeneous CON-DUP PC as UT-0400-01.

**When:**
```
let serialized = rkyv::to_bytes::<_, 256>(&pc_in).expect("rkyv serialize");
let archived   = rkyv::access::<ArchivedPendingCommutation, rkyv::rancor::Error>(&serialized)?;
```

**Then:**
- `archived.request_id == pc_in.request_id` (primitive).
- `archived.target_symbols.len() == pc_in.target_symbols.len()` and element-wise equal via `ArchivedSymbol` discriminator.
- `archived.local_wiring.len() == pc_in.local_wiring.len()` and element-wise equal via `ArchivedLocalWiringHint`.
- Deserialize back to owned: `let pc_out: PendingCommutation = rkyv::deserialize::<_, rkyv::rancor::Error>(archived)?;` and `pc_out == pc_in`.
- Byte count comparison: `serialized.len()` within ± 16 bytes of baseline pre-D-005 size adjusted for new `Vec<Symbol>` field — document the anchor (Q5 alignment invariant from `zero_copy_tests.rs:656`).

**Edge cases:**
- EC-1: minimum PC (UT-0400-02 shape) — verify the archive accommodates an empty `local_wiring` with a 0-length prefix, not a null Vec.
- EC-2: round-trip under `--features zero-copy` AND default build: the same test body must compile under both feature flags (the outer `#[cfg(feature = "zero-copy")]` gates the entire test; there is no cross-feature compatibility claim here).

**SPEC covered:** R34 rkyv gate.

**Priority:** MANDATORY (zero-copy is a feature of the project; must validate).

---

### UT-0400-08: `local_wiring_hint_rkyv_access_roundtrip` `#[cfg(feature = "zero-copy")]`

**Purpose:** Standalone rkyv round-trip for `LocalWiringHint` (the struct is embedded in PC via `Vec`; this isolates the struct for simpler diagnostic).

**Target:** `protocol/zero_copy_tests.rs`.

**Given:** 3 hints with the three target categories:
```
let hints = vec![
    LocalWiringHint { src_slot: 0, src_port: 1, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 0) }, // placeholder
    LocalWiringHint { src_slot: 1, src_port: 2, target: PortRef::AgentPort(123, 0) },                 // concrete
    // FreePort target is rejected by the worker (R33c case 6); wire round-trip is still valid:
    LocalWiringHint { src_slot: 0, src_port: 2, target: PortRef::FreePort(999) },
];
```

**When:** rkyv serialize → access → assert → deserialize.

**Then:**
- For each hint `h_in` in `hints`:
  - `archived_h.src_slot == h_in.src_slot`.
  - `archived_h.src_port == h_in.src_port`.
  - `archived_h.target` deserializes to the same `PortRef` variant (discriminator + payload).
- Owned round-trip equality: `h_out == h_in` for all three.

**Edge cases:**
- EC-1: `target = PortRef::AgentPort(u32::MAX - 10_000, 0)` (coordinator-reserved range boundary; wire path indifferent).
- EC-2: alignment — `Vec<LocalWiringHint>` embedded in PC must preserve the rkyv 4-byte alignment for `PortRef` (Q5 cross-check; assert `serialized.as_ptr() as usize % 4 == 0` is NOT required, but the archived struct must dereference cleanly).

**SPEC covered:** R34 rkyv gate.

**Priority:** MANDATORY.

---

### UT-0400-09: `pending_commutation_rkyv_bytecheck_rejects_corrupted_length_byte` `#[cfg(feature = "zero-copy")]`

**Purpose:** Corruption resilience gate per SPEC-18 Q1 / R34 `bytecheck` requirement. A 1-byte flip in the `target_symbols` Vec length prefix MUST cause `rkyv::access` to return `Err(_)`, not silent success.

**Target:** `protocol/zero_copy_tests.rs` (pattern from existing zero-copy corruption tests).

**Given:**
1. Same heterogeneous CON-DUP PC as UT-0400-01, serialized via rkyv.
2. Locate the byte offset of the `target_symbols` Vec length prefix inside the archive. rkyv archives store Vecs via a relative pointer + length; the length is usually a `u32` or `usize`-sized field at a deterministic offset once `request_id` (4 bytes) is consumed.
3. Flip one bit in that length byte, e.g., `serialized[length_offset] ^= 0xFF;`.

**When:**
```
let result = rkyv::access::<ArchivedPendingCommutation, rkyv::rancor::Error>(&corrupted);
```

**Then:**
- `result.is_err()`.
- The error is a validation/archive-check variant (not a panic, not a silent malformed access).
- A parallel test on the NON-corrupted bytes returns `Ok(_)` — verifies the corruption, not the baseline, is the cause.

**Edge cases:**
- EC-1: flip a byte in the `local_wiring` Vec length prefix (distinct field, also must be bytecheck-protected).
- EC-2: flip a byte in the `Symbol` enum discriminator at position `[k]` in `target_symbols` — expected `Err` because `Symbol`'s rkyv derive includes bytecheck (verify by reading `net/types.rs:34-38`).

**SPEC covered:** R34 bytecheck, SPEC-18 Q1 contract extension to Shape A.

**Priority:** MANDATORY (security-posture test; protects against malicious or accidentally-corrupted wire bytes).

---

## Property Tests

### PT-0400-01 (optional): `prop_pending_commutation_roundtrip_stable_under_bincode`

**Property:** For all arbitrary PCs within the supported shape envelope (`target_symbols.len() ∈ [1, 16]`, `local_wiring.len() ∈ [0, 32]`, `src_slot < target_symbols.len()`, `src_port ∈ [0, 2]`, any `PortRef` target), `bincode::deserialize(bincode::serialize(pc)?)? == pc`.

**Generator strategy:**
- `arb_symbol()` → one of `{Con, Dup, Era}`.
- `arb_target_symbols()` → `Vec<Symbol>` with length in `1..=16`.
- `arb_portref()` → weighted: 40% `AgentPort(x, p)` with `x < SLOT_MARKER_BASE`, 40% `AgentPort(SLOT_MARKER_BASE + s, p)` with `s ∈ [0, target_symbols.len())`, 20% `FreePort(bid)`.
- `arb_local_wiring(target_symbols)` → `Vec<LocalWiringHint>` respecting the generator invariants above.

**Assertion:** `prop_assert_eq!(pc_out, pc_in);`.

**Shrinking note:** Minimal counterexample should isolate the field that fails (`target_symbols` vs `local_wiring` vs `PortRef` variant).

**Priority:** OPTIONAL (Stage 5 QA is the natural home for property coverage). Developer MAY defer to TEST-SPEC-0402 where the envelope is already exercised via R33c case tests. RECOMMEND INCLUDE if proptest dependency is already active in the crate.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-0400-01 | Empty `target_symbols` on wire | Round-trip OK (wire is shape-agnostic); semantic rejection at R24.1.6a (TEST-SPEC-0402) | UT-0400-04 (ZeroArity variant) |
| EC-0400-02 | `target_symbols.len() == 17` on wire | Round-trip OK (wire cap is 16 only at resolver) | UT-0400-03 EC-1 |
| EC-0400-03 | Slot-marker `x = SLOT_MARKER_BASE` with `s = 0` | Wire pass-through; decode at TEST-SPEC-0402 | UT-0400-08 |
| EC-0400-04 | Concrete-id target with `id = 0` | Wire pass-through (id 0 is legal for worker's first-minted agent) | UT-0400-01 implicit |
| EC-0400-05 | Serialize empty `local_wiring` | Round-trip OK; Vec length prefix = 0 | UT-0400-02 |
| EC-0400-06 | `request_id = 0` + `request_id = u32::MAX` | Round-trip OK at both boundaries | UT-0400-01 EC-1/2 |
| EC-0400-07 | `MintedAgent { minted_agent_id: u32::MAX - 10_000 }` | Round-trip OK; range semantics NOT enforced at wire | UT-0400-06 EC-2 |
| EC-0400-08 | Corrupted length byte in rkyv archive | `rkyv::access` returns `Err(_)` | UT-0400-09 |
| EC-0400-09 | Pre-Shape-A bincode blob fed to post-R37 decoder | `Err(DeserializationFailed)` (NOT silent success) | (covered by existing handshake-version test OR UT-0400-05 EC-1) |

---

## Coverage mapping

| Requirement / contract | Covered by |
|------------------------|-----------|
| SPEC-19 R23 wire payload shape (heterogeneous mints) | UT-0400-01 |
| SPEC-19 R33 `LocalWiringHint` struct surface | UT-0400-01, 02, 08 |
| SPEC-19 R33c case 1 (SrcSlotOutOfRange) serialization | UT-0400-04 |
| SPEC-19 R33c case 2 (SrcPortOutOfRange) serialization | UT-0400-04 |
| SPEC-19 R33c case 3 (TargetSiblingOutOfRange) serialization | UT-0400-04 |
| SPEC-19 R33c case 4 (DanglingConcreteAgent) serialization | UT-0400-04 |
| SPEC-19 R33c case 5 (DuplicateSourcePort) serialization | UT-0400-04 |
| SPEC-19 R33c case 6 (ReservedForFuture) serialization | UT-0400-04 |
| SPEC-19 R33c case 7 (ZeroArity) serialization | UT-0400-04 |
| SPEC-19 R33c case 8 (TargetSymbolsTooLong, NR3-003 gated) | UT-0400-04 EC-3 (if shipped) |
| SPEC-19 R34 bincode round-trip | UT-0400-01, 02, 03, 06 |
| SPEC-19 R34 rkyv round-trip | UT-0400-07, 08 |
| SPEC-19 R34 rkyv bytecheck rejects corruption | UT-0400-09 |
| SPEC-19 R37 PROTOCOL_VERSION bump | UT-0400-05 |
| SPEC-19 R48b empty-local_wiring legal (on wire) | UT-0400-02 |
| NF-001 Shape A heterogeneous fidelity | UT-0400-01 |
| NF-002 bidirectional version rejection | UT-0400-05 (EC-1 if existing test missing) |
| NR3-001 absorption (docstring anchor) | OPTIONAL inline string search |
| NR3-002 absorption (error-surface distinction) | UT-0400-04 (format string assertion) |
| NR3-003 deferral (8th case optional) | UT-0400-04 EC-3 sentinel if shipped |

---

## Test count estimate

| Config | Baseline | UTs added | Target |
|--------|----------|-----------|--------|
| `cargo test --workspace --lib` | 1146 | +6 (UT-0400-01..06) | **≥ 1152** |
| `cargo test --workspace --lib --features zero-copy` | 1186 | +6 default + 3 zero-copy (UT-0400-07..09) | **≥ 1195** |

Note: PT-0400-01 is OPTIONAL; if included, default count rises by 1 (a single proptest `!{ }` macro call counts as one `#[test]` from cargo's perspective).

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1146 → ≥ 1152.
2. `cargo test --workspace --lib --features zero-copy` count: 1186 → ≥ 1195.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
4. `cargo fmt --check` clean.
5. No regression on any existing test (baseline 1146 / 1186 remains a lower bound).
6. Grep verification: no remaining references to `pc.symbol_type` or `pc.arity` outside the git history / `docs/` directory.
7. `PROTOCOL_VERSION == 3` at `protocol/coordinator.rs:570`.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0398 / TEST-SPEC-0399:** D-004 plumbing and integration — consume `MintedAgent`, not `PendingCommutation.target_symbols`. Those tests remain green after TASK-0400 because the `MintedAgent` shape is amendment-only (rkyv derives added).
- **TEST-SPEC-0401:** consumes the Shape A PC via `commutation_batch_to_pending` — depends on UT-0400-01 shape definition being correct.
- **TEST-SPEC-0402:** consumes `LocalWiringHint` + `MalformedLocalWiringReason` — depends on UT-0400-04 round-trip guarantee so worker-side rejects can be carried over the wire end-to-end.
- **TEST-SPEC-0403:** bundle acceptance gate; no direct UT coupling; relies on full stack TASK-0400..0402.

---

## Out of scope

- **Migration or decode-retry paths** for pre-R37 blobs — R37 explicitly prohibits any such path.
- **rkyv forward-compatibility** (reading a v4 archive with a v3 decoder) — unconstrained by this bundle.
- **Fuzz-level property testing** of arbitrary byte corruption — handled by Stage 5 QA adversarial tests; UT-0400-09 covers the minimal case.
- **Multi-thread contention** around the `PROTOCOL_VERSION` const — stateless `const`, irrelevant.
