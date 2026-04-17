# TEST-SPEC-0366: `BorderDelta` re-export + serde derive fix + bincode round-trip

**Task:** TASK-0366
**Spec:** SPEC-19 §3.4 (R33 struct shape, R37 doc-only discriminant stability)
**Generated:** 2026-04-17
**Baseline before this task:** 968 lib (default) / 1008 lib (`--features zero-copy`) — post SPEC-19 §3.2 ship, pipeline-state 2026-04-17.
**Cumulative target after this task:** 972 lib (default) / 1012 lib (`--features zero-copy`) — **+4** new tests (DC-A1: `test_border_delta_bincode_roundtrip`; DC-B3/DC-B5: `test_local_reconnection_bincode_roundtrip`, `test_pending_commutation_bincode_roundtrip`, `test_minted_agent_bincode_roundtrip`, per SPEC-19 R33 amendment 2026-04-17).

---

## Scope note

This task has three distinct contracts under test:

1. **DC-A1 structural defect fix.** The existing `BorderDelta` struct at
   `relativist-core/src/merge/border_graph.rs:126-131` is missing
   `serde::Serialize` and `serde::Deserialize` derives, which R33
   mandates and which downstream TASK-0368/0369 require for the
   `Message` variants that carry `Vec<BorderDelta>`. TASK-0366 adds the
   two derives at the definition site (not by moving the struct —
   SPEC-13 R6-R8 forbids `merge/` depending on `protocol/`).
2. **Re-export visibility.** `protocol/` gains a `pub use
   crate::merge::BorderDelta;` re-export so the new `Message` variants
   can name `crate::protocol::BorderDelta`. Pure layering-hygiene
   contract — the struct stays in `merge/` (pure-core, R19).
3. **Round-trip identity at the struct level.** One new unit test in
   `merge::border_graph::tests` locks R33's round-trip property at the
   struct level. Variant-level round-trips live in TASK-0367/0368/0369
   TEST-SPECs — they are additive evidence, not the primary contract
   for R33.

No wire-layer (`send_frame_*` / CRC / compression) work in this task.
Frame-layer tests for the `Vec<BorderDelta>`-carrying variants live in
TEST-SPEC-0370.

---

## Test target file paths

- `relativist-core/src/merge/border_graph.rs` — inline `#[cfg(test)]
  mod tests` block (existing); one new `#[test]` fn
  (`test_border_delta_bincode_roundtrip`).
- `relativist-core/src/protocol/types.rs` OR `relativist-core/src/protocol/mod.rs` —
  source-grep contract for the `pub use` re-export (enforced at
  compile time by downstream TASKs; UT-0366-03 below exercises the
  import path from a test).
- `relativist-core/src/merge/border_graph.rs` — derive-set contract
  (enforced at compile time by the serde-requiring test body).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### T1: `border_delta_struct_shape_is_two_fields_per_r33`

**Purpose:** Compile-time contract that `BorderDelta` has exactly the
two fields R33 mandates (`border_id: u32`, `new_target: PortRef`), both
`pub`. Breaks compilation if any field is added, renamed, or removed,
or if a visibility modifier is accidentally added.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderDelta`, `PortRef` imported.

**When:** Construct a `BorderDelta` via full struct literal.

**Then:**
```rust
let delta = BorderDelta {
    border_id: 42,
    new_target: PortRef::AgentPort(7, 0),
};
assert_eq!(delta.border_id, 42);
assert_eq!(delta.new_target, PortRef::AgentPort(7, 0));
```

**Assertions:**
- Struct literal compiles (field set is locked to exactly two names).
- Both field reads yield the written values (field types match R33:
  `u32` and `PortRef`).

**SPEC-19 R covered:** R33 (shape).

---

### T2: `border_delta_bincode_roundtrip_agent_port_target`

**Purpose:** Lock R33's round-trip property at the struct level.
Encode a `BorderDelta` carrying a non-trivial `AgentPort` target,
decode, assert equality. This is the DC-A1 amendment test listed in
TASK-0366 acceptance criteria (post 2026-04-17).

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderDelta { border_id: 5, new_target: PortRef::AgentPort(42, 0) }`.

**When:** `bincode_v2::encode(&delta)` → `bincode_v2::decode_value::<BorderDelta>(...)`.

**Then:**
```rust
let original = BorderDelta {
    border_id: 5,
    new_target: PortRef::AgentPort(42, 0),
};
let bytes = bincode_v2::encode(&original).expect("encode must succeed");
let (decoded, _consumed) = bincode_v2::decode_value::<BorderDelta>(&bytes)
    .expect("decode must succeed");
assert_eq!(decoded, original, "bincode round-trip must preserve value");
assert_eq!(decoded.border_id, 5);
assert_eq!(decoded.new_target, PortRef::AgentPort(42, 0));
```

**Assertions:**
- `serde::Serialize` derive is active on `BorderDelta` (compile fails
  otherwise — this is the DC-A1 regression guard).
- `serde::Deserialize` derive is active.
- Round-trip preserves value equality (R33 + SPEC-06 R14 at struct
  level).

**SPEC-19 R covered:** R33 (serde derives + round-trip identity).

---

### T3: `border_delta_bincode_roundtrip_disconnected_sentinel`

**Purpose:** R33 permits the DISCONNECTED sentinel
(`PortRef::FreePort(u32::MAX)` per spec-critic DC-1 of §3.2 verdict and
`crate::net::DISCONNECTED`). Ensure the sentinel survives the round
trip without special-casing collapsing it to `None` or similar.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderDelta { border_id: 9, new_target: PortRef::FreePort(u32::MAX) }`.

**When:** Encode → decode → compare.

**Then:**
```rust
use crate::net::DISCONNECTED;  // PortRef::FreePort(u32::MAX)
let original = BorderDelta {
    border_id: 9,
    new_target: DISCONNECTED,
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<BorderDelta>(&bytes)
    .expect("decode");
assert_eq!(decoded, original);
assert_eq!(decoded.new_target, DISCONNECTED,
    "DISCONNECTED sentinel must round-trip identically");
```

**Assertions:**
- `u32::MAX` is preserved through varint encoding (varints handle the
  full `u32` range; this pins it).
- The DISCONNECTED sentinel is not accidentally normalised to some
  other `PortRef` during serde (e.g. a future custom `impl Serialize`
  that tries to compress `FreePort(u32::MAX)` to a dedicated tag would
  break this test).

**SPEC-19 R covered:** R33 + cross-reference to §3.2 erasure
semantics (DC-1 of §3.2 verdict).

---

### T4: `border_delta_bincode_roundtrip_border_id_boundary`

**Purpose:** Varint edge case — `border_id: u32::MAX` takes the longest
varint encoding path (5 bytes for a `u32` under bincode v2). Ensure
nothing regresses at the high-end boundary.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `BorderDelta { border_id: u32::MAX, new_target: PortRef::FreePort(0) }`.

**When:** Encode → decode → compare.

**Then:**
```rust
let original = BorderDelta {
    border_id: u32::MAX,
    new_target: PortRef::FreePort(0),
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<BorderDelta>(&bytes)
    .expect("decode");
assert_eq!(decoded, original);
assert_eq!(decoded.border_id, u32::MAX);
```

**Assertions:**
- The maximum `u32` border id survives varint encoding + decoding.
- No silent truncation at encode/decode time (would surface if the
  serde impl accidentally narrowed to `u16`).

**SPEC-19 R covered:** R33.

---

### T5: `border_delta_bincode_size_is_varint_not_fixint`

**Purpose:** Regression guard — bincode v2 varint encoding (SPEC-18 R3)
is active on `BorderDelta`. A small `border_id` (0 or 1) encodes in a
single byte under varint but in 4 bytes under fixint. Assert the
smaller shape.

**Target file:** `merge/border_graph.rs::tests`

**Given:** Two `BorderDelta`s, `{border_id: 0, new_target:
PortRef::FreePort(0)}` and `{border_id: 1_000_000, new_target:
PortRef::FreePort(0)}`.

**When:** Encode both.

**Then:**
```rust
let small = BorderDelta {
    border_id: 0,
    new_target: PortRef::FreePort(0),
};
let large = BorderDelta {
    border_id: 1_000_000,
    new_target: PortRef::FreePort(0),
};
let small_bytes = bincode_v2::encode(&small).expect("encode small");
let large_bytes = bincode_v2::encode(&large).expect("encode large");
assert!(
    small_bytes.len() < large_bytes.len(),
    "varint encoding: border_id=0 must encode smaller than \
     border_id=1_000_000 (got small={} bytes, large={} bytes)",
    small_bytes.len(), large_bytes.len()
);
```

**Assertions:**
- Varint encoding is in effect (not `legacy()` / fixint-little-endian
  config from SPEC-06 R4.1 pre-SPEC-18).
- This is a negative guard: if a future refactor reverts the config to
  fixint, the test fires immediately.

**SPEC-19 R covered:** R33 via SPEC-18 R3 (varint encoding carries
the serde-derived types).

---

### T6: `border_delta_is_reachable_at_protocol_path`

**Purpose:** DC-A1 placement contract — the `pub use
crate::merge::BorderDelta;` re-export in `protocol/` works. Any
downstream task (0368/0369/0370) imports `BorderDelta` via either
`crate::protocol::BorderDelta` or `crate::merge::BorderDelta`. This
test pins that both paths resolve to the same type.

**Target file:** `merge/border_graph.rs::tests` OR
`protocol/types.rs::tests` — developer's choice; the test is
path-pinning only.

**Given:** The `pub use` re-export has been added to `protocol/mod.rs`
or `protocol/types.rs`.

**When:** Reference `BorderDelta` via both paths in a single compile
unit.

**Then:**
```rust
// Compile-time identity check — both paths must refer to the same
// type for this function to typecheck.
let _ct: fn(crate::merge::BorderDelta) -> crate::protocol::BorderDelta =
    |d| d;
// Runtime sanity check.
let delta = crate::protocol::BorderDelta {
    border_id: 1,
    new_target: crate::net::PortRef::FreePort(2),
};
assert_eq!(delta.border_id, 1);
```

**Assertions:**
- `crate::protocol::BorderDelta` resolves (re-export is present).
- `crate::merge::BorderDelta` resolves (original path unchanged).
- Both paths point to the same type (compile-time identity via the
  `|d| d` identity function — if they were two distinct types the
  coercion would fail).

**SPEC-19 R covered:** DC-A1 placement ruling (re-export contract);
indirectly R33 (single source of truth for the struct).

---

### T7: `local_reconnection_bincode_roundtrip`  *(DC-B3, SPEC-19 R33 2026-04-17 amendment)*

**Purpose:** Round-trip identity for the new `LocalReconnection` struct.
Locks R33's derives-plus-round-trip property for the DC-B3 wire type.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `LocalReconnection { agent_id: AgentId(7), port: 2,
new_target: PortRef::AgentPort(11, 1) }`.

**When:** `bincode_v2::encode` → `bincode_v2::decode_value`.

**Then:**
```rust
use crate::net::{AgentId, PortRef};
let original = LocalReconnection {
    agent_id: AgentId(7),
    port: 2,
    new_target: PortRef::AgentPort(11, 1),
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<LocalReconnection>(&bytes)
    .expect("decode");
assert_eq!(decoded, original);
assert_eq!(decoded.agent_id, AgentId(7));
assert_eq!(decoded.port, 2);
assert_eq!(decoded.new_target, PortRef::AgentPort(11, 1));
```

**Assertions:**
- All three fields (`agent_id`, `port`, `new_target`) round-trip
  losslessly.
- Struct literal compiles with the exact 3-field shape R33 mandates.
- `u8 port` field is preserved (varint handles u8 as a single byte).

**SPEC-19 R covered:** R33 (DC-B3 struct shape + derives + round-trip).

---

### T8: `pending_commutation_bincode_roundtrip`  *(DC-B5, SPEC-19 R33 + R48 2026-04-17 amendment)*

**Purpose:** Round-trip identity for `PendingCommutation` — the
coordinator → worker AgentId-allocation request type.

**Target file:** `merge/border_graph.rs::tests`

**Given:** `PendingCommutation { request_id: 42, symbol_type:
Symbol::Dup, arity: 2 }`.

**When:** `bincode_v2::encode` → `bincode_v2::decode_value`.

**Then:**
```rust
use crate::net::Symbol;
let original = PendingCommutation {
    request_id: 42,
    symbol_type: Symbol::Dup,
    arity: 2,
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<PendingCommutation>(&bytes)
    .expect("decode");
assert_eq!(decoded, original);
assert_eq!(decoded.request_id, 42);
assert!(matches!(decoded.symbol_type, Symbol::Dup));
assert_eq!(decoded.arity, 2);
```

**Assertions:**
- All three fields round-trip.
- `Symbol` enum variant survives bincode (exhaustive variant coverage
  via `matches!`).
- `u32 request_id` (R48 correlation ID) preserved — varint encoding
  for typical small IDs.

**SPEC-19 R covered:** R33 (DC-B5 struct shape + derives); R48
(correlation-ID field shape).

---

### T9: `minted_agent_bincode_roundtrip`  *(DC-B5 echo side, SPEC-19 R33 + R48 2026-04-17 amendment)*

**Purpose:** Round-trip identity for `MintedAgent` — the worker →
coordinator AgentId echo that matches a prior `PendingCommutation` by
`request_id` (R48).

**Target file:** `merge/border_graph.rs::tests`

**Given:** `MintedAgent { request_id: 42, minted_agent_id: AgentId(103) }`.

**When:** `bincode_v2::encode` → `bincode_v2::decode_value`.

**Then:**
```rust
use crate::net::AgentId;
let original = MintedAgent {
    request_id: 42,
    minted_agent_id: AgentId(103),
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<MintedAgent>(&bytes)
    .expect("decode");
assert_eq!(decoded, original);
assert_eq!(decoded.request_id, 42);
assert_eq!(decoded.minted_agent_id, AgentId(103));
```

**Assertions:**
- `request_id` (R48 correlation) round-trips losslessly.
- `minted_agent_id` (R48 reserved-range constraint is a worker-side
  invariant — this TEST-SPEC does NOT enforce the range constraint;
  that lands in 2.26-C).
- 2-field struct layout compiles with both fields `pub`.

**SPEC-19 R covered:** R33 (DC-B5 echo struct shape + derives); R48
(correlation by request_id at the wire layer).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R33 — struct shape `{border_id: u32, new_target: PortRef}` | T1 |
| R33 — `serde::Serialize + Deserialize` derives | T2, T3, T4, T5 |
| R33 — round-trip identity at struct level | T2, T3, T4 |
| R37 — discriminant stability doc (`protocol/types.rs` `//!` block) | Compile-time via downstream TASK-0371 byte test; no runtime assertion in this TEST-SPEC |
| DC-A1 — `BorderDelta` stays in `merge/`, re-exported to `protocol/` | T6 |
| DC-A1 — derive-set fix (pre-existing defect in TASK-0362) | T2 (would fail to compile without the serde derives — the test is the regression guard) |
| SPEC-18 R3 — varint encoding carries struct | T5 |
| §3.2 DC-1 — DISCONNECTED sentinel (`PortRef::FreePort(u32::MAX)`) | T3 |
| R33 DC-B3 — `LocalReconnection` struct shape + derives + round-trip | T7 |
| R33 DC-B5 — `PendingCommutation` struct shape + derives + round-trip | T8 |
| R33 DC-B5 — `MintedAgent` struct shape + derives + round-trip | T9 |
| R48 — `request_id` correlation field shape (wire-layer only) | T8, T9 |

R37 coverage here is doc-only (the module-level `//!` block update);
the byte-level discriminant test for the `Message` enum lives in
TASK-0371 / TEST-SPEC-0371.

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0366-A | Hand-written `impl Serialize for BorderDelta` that doesn't round-trip | T2 catches value inequality; T5 catches byte-size regression from a fixint drift |
| QA-0366-B | Future refactor renames `new_target` to `target` | T1 struct literal fails to compile (canary) |
| QA-0366-C | Future refactor changes `border_id` to `Option<u32>` | T1 compile failure via exhaustive struct literal |
| QA-0366-D | Hypothetical `PortRef::FreePort(u32::MAX)` reserved as null | T3 fires — DISCONNECTED must round-trip as itself |
| QA-0366-E | SPEC-18 varint config accidentally reverted to `legacy()` (SPEC-06 R4.1) | T5 fires — small ids no longer compact |
| QA-0366-F | `pub use crate::merge::BorderDelta;` deleted in a future cleanup | T6 fails to compile via missing path |
| QA-0366-G | Struct-packed attribute `#[repr(packed)]` applied for memory optimisation | Serde derive breaks for packed reprs; T2/T3/T4/T5 fire at compile time |
| QA-0366-H | `border_id` field moved to end of struct (reordering) | bincode is structural, not name-keyed — round-trip tests still pass, but a cross-version wire peer would see different ordering. Flag as architecture-review concern; no runtime test catches it |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 968 → **972** (+4 new
   `#[test]` fns — `test_border_delta_bincode_roundtrip`,
   `test_local_reconnection_bincode_roundtrip`,
   `test_pending_commutation_bincode_roundtrip`,
   `test_minted_agent_bincode_roundtrip`;
   T1/T3/T4/T5/T6 are test CASES folded into the BorderDelta `#[test]`
   fn per TASK-0366 acceptance; if the developer splits them into
   separate `#[test]` fns, the count floor is still +4 and the gate
   tolerates up to +9).
2. `cargo test --workspace --lib --features zero-copy` count: 1008 →
   **1012** (same logic).
3. `cargo build --workspace` clean (default features).
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean both
   with and without `--features zero-copy`.
6. `cargo fmt --check` clean.
7. `cargo doc --workspace --no-deps` exits 0 with no broken-link
   warnings against the augmented `//!` block in `protocol/types.rs`.

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- `Message::InitialPartition` + `FinalStateResult` bincode round-trips → TEST-SPEC-0367.
- `Message::RoundStart` + `FinalStateRequest` bincode round-trips → TEST-SPEC-0368.
- `Message::RoundResult` bincode round-trip + DC-A2 activity invariant → TEST-SPEC-0369.
- Wire-layer integration (frame round-trip, CRC32, compression) → TEST-SPEC-0370.
- Byte-level discriminant stability for the full 12-variant enum → TEST-SPEC-0371.
