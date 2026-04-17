# TEST-SPEC-0368: `Message::RoundStart` (disc 8) + `Message::FinalStateRequest` (disc 10) — small C→W variants

**Task:** TASK-0368
**Spec:** SPEC-19 §3.4 (R31 RoundStart + FinalStateRequest, R34 serde +
  bincode round-trip, R36 compression-skip-below-threshold, R37
  discriminant stability)
**Generated:** 2026-04-17
**Baseline before this task:** 976 lib (default) / 1016 lib
  (`--features zero-copy`) — post TASK-0367 per DC-A1 + DC-B3/B5
  amended trajectory (TASK-0366 now contributes +4 instead of +1).
**Cumulative target after this task:** 981 lib / 1021 lib — **+5** new
  `#[test]` fns (3 original + 2 DC-B3/DC-B5 per SPEC-19 R33 amendment
  2026-04-17: `test_round_start_local_reconnections_populated`,
  `test_round_start_pending_commutations_populated`).

---

## Scope note

This task lands the two **small-payload coordinator → worker**
delta-protocol variants at discriminants 8 and 10:

- `RoundStart { round, border_deltas: Vec<BorderDelta>,
  resolved_borders: Vec<u32>, new_borders: Vec<(u32, PortRef)>,
  local_reconnections: Vec<LocalReconnection>,
  pending_commutations: Vec<PendingCommutation> }` — per-round kickoff.
  The last two fields were appended by the SPEC-19 R33 amendment
  (2026-04-17, DC-B3 + DC-B5); all tests below cover them.
- `FinalStateRequest { round }` — minimal-payload convergence signal.

**Bincode layer only.** R36's "SHOULD skip compression below threshold"
verification lands at the wire layer in TEST-SPEC-0370. R37 byte-level
stability lands in TEST-SPEC-0371.

The `BorderDelta` references in `RoundStart.border_deltas` are carried
via the `crate::protocol::BorderDelta` re-export (TASK-0366). The
`PortRef` imports are promoted from `#[cfg(test)]` scope to production
`use crate::net::PortRef;` at the top of `types.rs` — verified
implicitly by successful compilation of `RoundStart`'s field types.

---

## Test target file paths

- `relativist-core/src/protocol/types.rs` — `#[cfg(test)] mod tests`
  block. All 3 new `#[test]` fns + one blanket-test list extension.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### T1: `round_start_bincode_roundtrip_populated_vecs`

**Purpose:** R31/R34 round-trip identity for `RoundStart` carrying
non-empty values in all FIVE vector fields (3 original + 2 added by
SPEC-19 R33 amendment 2026-04-17). Asserts per-element equality after
decode.

**Target file:** `protocol/types.rs::tests`

**Given:**
- `border_deltas = vec![BorderDelta { border_id: 5, new_target: PortRef::AgentPort(42, 0) }, BorderDelta { border_id: 7, new_target: PortRef::FreePort(9) }]`
- `resolved_borders = vec![1, 3, 8]`
- `new_borders = vec![(20, PortRef::FreePort(20)), (21, PortRef::FreePort(21))]`
- `local_reconnections = vec![LocalReconnection { agent_id: AgentId(7), port: 2, new_target: PortRef::AgentPort(11, 1) }]` *(DC-B3)*
- `pending_commutations = vec![PendingCommutation { request_id: 42, symbol_type: Symbol::Dup, arity: 2 }]` *(DC-B5)*
- `round = 5`

**When:** `bincode_v2::encode` → `bincode_v2::decode_value::<Message>`.

**Then:**
```rust
let original = Message::RoundStart {
    round: 5,
    border_deltas: vec![
        BorderDelta { border_id: 5, new_target: PortRef::AgentPort(42, 0) },
        BorderDelta { border_id: 7, new_target: PortRef::FreePort(9) },
    ],
    resolved_borders: vec![1, 3, 8],
    new_borders: vec![
        (20, PortRef::FreePort(20)),
        (21, PortRef::FreePort(21)),
    ],
    local_reconnections: vec![
        LocalReconnection {
            agent_id: AgentId(7),
            port: 2,
            new_target: PortRef::AgentPort(11, 1),
        },
    ],
    pending_commutations: vec![
        PendingCommutation {
            request_id: 42,
            symbol_type: Symbol::Dup,
            arity: 2,
        },
    ],
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundStart { round, border_deltas, resolved_borders, new_borders,
                           local_reconnections, pending_commutations } => {
        assert_eq!(round, 5);
        assert_eq!(border_deltas.len(), 2);
        assert_eq!(border_deltas[0].border_id, 5);
        assert_eq!(border_deltas[0].new_target, PortRef::AgentPort(42, 0));
        assert_eq!(border_deltas[1].border_id, 7);
        assert_eq!(border_deltas[1].new_target, PortRef::FreePort(9));
        assert_eq!(resolved_borders, vec![1, 3, 8]);
        assert_eq!(new_borders.len(), 2);
        assert_eq!(new_borders[0], (20, PortRef::FreePort(20)));
        assert_eq!(new_borders[1], (21, PortRef::FreePort(21)));
        assert_eq!(local_reconnections.len(), 1);
        assert_eq!(local_reconnections[0].agent_id, AgentId(7));
        assert_eq!(local_reconnections[0].port, 2);
        assert_eq!(local_reconnections[0].new_target, PortRef::AgentPort(11, 1));
        assert_eq!(pending_commutations.len(), 1);
        assert_eq!(pending_commutations[0].request_id, 42);
        assert!(matches!(pending_commutations[0].symbol_type, Symbol::Dup));
        assert_eq!(pending_commutations[0].arity, 2);
    }
    other => panic!("expected RoundStart, got {:?}", other),
}
```

**Assertions:**
- Variant dispatch (disc 8).
- `Vec<BorderDelta>` round-trips with correct length and element
  values.
- `Vec<u32>` round-trips.
- `Vec<(u32, PortRef)>` tuple round-trips — no adapter needed under
  bincode v2 (product-type check).
- `Vec<LocalReconnection>` round-trips (DC-B3 wire path).
- `Vec<PendingCommutation>` round-trips (DC-B5 wire path).

**SPEC-19 R covered:** R31 (variant shape + disc 8), R33 (DC-B3, DC-B5
struct shapes on the wire), R34 (round-trip), R48 (correlation ID
preservation).

---

### T2: `round_start_bincode_roundtrip_empty_vecs`

**Purpose:** Empty-Vec regression guard — bincode v2 varint encodes a
zero-length `Vec` as a single zero byte. If a future macro / attribute
silently switches to `Option<Vec<...>>` or applies a length-prefix
shift, the empty case breaks first. Post-amendment: covers ALL FIVE
vector fields (3 original + 2 DC-B3/DC-B5).

**Target file:** `protocol/types.rs::tests`

**Given:** `RoundStart` with `round: 0` and all five `Vec` fields
empty.

**When:** Encode, decode.

**Then:**
```rust
let original = Message::RoundStart {
    round: 0,
    border_deltas: Vec::new(),
    resolved_borders: Vec::new(),
    new_borders: Vec::new(),
    local_reconnections: Vec::new(),   // DC-B3
    pending_commutations: Vec::new(),  // DC-B5
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundStart { round, border_deltas, resolved_borders, new_borders,
                           local_reconnections, pending_commutations } => {
        assert_eq!(round, 0);
        assert_eq!(border_deltas.len(), 0);
        assert_eq!(resolved_borders.len(), 0);
        assert_eq!(new_borders.len(), 0);
        assert_eq!(local_reconnections.len(), 0);
        assert_eq!(pending_commutations.len(), 0);
    }
    other => panic!("expected RoundStart, got {:?}", other),
}
```

**Assertions:**
- Empty `Vec` stays empty through round-trip (not silently turned
  into `None` or `Option::None`).
- All FIVE collections are accessed via `.len()` — so a type drift
  to e.g. `Option<Vec<...>>` for any field would compile-fail this
  test.
- Regression guard on the "idle worker" common case (all five vecs
  empty = zero border activity for this round).

**SPEC-19 R covered:** R31, R33, R34 (edge case).

---

### T3: `final_state_request_bincode_roundtrip_minimal`

**Purpose:** R31 round-trip for the smallest-payload variant (one
`u32` field). Exercises disc 10.

**Target file:** `protocol/types.rs::tests`

**Given:** `Message::FinalStateRequest { round: 42 }`.

**When:** Encode → decode → match.

**Then:**
```rust
let original = Message::FinalStateRequest { round: 42 };
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::FinalStateRequest { round } => {
        assert_eq!(round, 42);
    }
    other => panic!("expected FinalStateRequest, got {:?}", other),
}
// Size sanity: FinalStateRequest is the smallest variant.
// 1 byte disc + 1-5 bytes varint for round = 2..=6 bytes total.
assert!(bytes.len() <= 6,
        "FinalStateRequest must encode in <= 6 bytes; got {}", bytes.len());
```

**Assertions:**
- Variant dispatch (disc 10).
- `round` is preserved.
- Encoded size is under 6 bytes (sanity; catches fixint config drift).

**SPEC-19 R covered:** R31 (variant shape + disc 10), R34.

---

### T4 (list-extension, not a new `#[test]`): extend `test_all_variants_serde_roundtrip`

**Purpose:** Add `RoundStart` and `FinalStateRequest` to the blanket
round-trip list. Coordinates with TASK-0367 + TASK-0369.

**Target file:** `protocol/types.rs::tests`
(inside `test_all_variants_serde_roundtrip`).

**Change:** Append (with all 6 `RoundStart` fields, 5 of them empty
Vecs):
```rust
Message::RoundStart {
    round: 0,
    border_deltas: Vec::new(),
    resolved_borders: Vec::new(),
    new_borders: Vec::new(),
    local_reconnections: Vec::new(),   // DC-B3
    pending_commutations: Vec::new(),  // DC-B5
},
Message::FinalStateRequest { round: 0 },
```

**Assertions:** Blanket test continues passing for all variants.

**SPEC-19 R covered:** R34 (blanket).

---

### T5: `round_start_local_reconnections_populated_only`  *(DC-B3, SPEC-19 R33 2026-04-17 amendment)*

**Purpose:** Targeted coverage of the DC-B3 wire path. Isolates
`local_reconnections` so a regression in its serde layer surfaces
independently of the broader T1 aggregate test.

**Target file:** `protocol/types.rs::tests`

**Given:** `RoundStart` with `local_reconnections` non-empty (2 entries,
varied symbols on `new_target`), all other Vecs empty, `round = 1`.

**When:** Encode → decode → match.

**Then:**
```rust
let original = Message::RoundStart {
    round: 1,
    border_deltas: Vec::new(),
    resolved_borders: Vec::new(),
    new_borders: Vec::new(),
    local_reconnections: vec![
        LocalReconnection {
            agent_id: AgentId(5),
            port: 0,
            new_target: PortRef::AgentPort(6, 1),
        },
        LocalReconnection {
            agent_id: AgentId(6),
            port: 1,
            new_target: PortRef::FreePort(42),
        },
    ],
    pending_commutations: Vec::new(),
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundStart { round, local_reconnections, .. } => {
        assert_eq!(round, 1);
        assert_eq!(local_reconnections.len(), 2);
        assert_eq!(local_reconnections[0].agent_id, AgentId(5));
        assert_eq!(local_reconnections[0].port, 0);
        assert_eq!(local_reconnections[0].new_target, PortRef::AgentPort(6, 1));
        assert_eq!(local_reconnections[1].agent_id, AgentId(6));
        assert_eq!(local_reconnections[1].port, 1);
        assert_eq!(local_reconnections[1].new_target, PortRef::FreePort(42));
    }
    other => panic!("expected RoundStart, got {:?}", other),
}
```

**Assertions:**
- Element order preserved (DC-B3 list semantics rely on position for
  sequential application by the worker).
- Heterogeneous `new_target` variants (`AgentPort` vs `FreePort`) both
  round-trip inside the same Vec.
- Principal port (`port: 0`) vs aux (`port: 1`) both round-trip via
  `u8`.

**SPEC-19 R covered:** R31, R33 (DC-B3 wire path), R34.

---

### T6: `round_start_pending_commutations_multi_request_id_order_preserved`  *(DC-B5, SPEC-19 R33 + R48 2026-04-17 amendment)*

**Purpose:** Targeted coverage of the DC-B5 wire path. Pins the R48
invariant that `request_id` is the correlation key: two distinct
`PendingCommutation` entries must survive round-trip with both their
identifiers and their positional order intact.

**Target file:** `protocol/types.rs::tests`

**Given:** `RoundStart` with `pending_commutations` non-empty (2 entries,
distinct request_ids 100 and 101, distinct symbol types Con and Era),
all other Vecs empty, `round = 2`.

**When:** Encode → decode → match.

**Then:**
```rust
let original = Message::RoundStart {
    round: 2,
    border_deltas: Vec::new(),
    resolved_borders: Vec::new(),
    new_borders: Vec::new(),
    local_reconnections: Vec::new(),
    pending_commutations: vec![
        PendingCommutation {
            request_id: 100,
            symbol_type: Symbol::Con,
            arity: 1,
        },
        PendingCommutation {
            request_id: 101,
            symbol_type: Symbol::Era,
            arity: 0,
        },
    ],
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundStart { round, pending_commutations, .. } => {
        assert_eq!(round, 2);
        assert_eq!(pending_commutations.len(), 2);
        assert_eq!(pending_commutations[0].request_id, 100);
        assert!(matches!(pending_commutations[0].symbol_type, Symbol::Con));
        assert_eq!(pending_commutations[0].arity, 1);
        assert_eq!(pending_commutations[1].request_id, 101);
        assert!(matches!(pending_commutations[1].symbol_type, Symbol::Era));
        assert_eq!(pending_commutations[1].arity, 0);
    }
    other => panic!("expected RoundStart, got {:?}", other),
}
```

**Assertions:**
- Each `request_id` survives round-trip (R48 correlation integrity).
- Element order is preserved in the `Vec` (R48 linear-scan matching
  property on the receiving worker).
- Distinct `Symbol` variants (`Con`, `Era`) both round-trip inside the
  same Vec without cross-contamination.
- Distinct `arity` values (0, 1) both round-trip — regression guard
  against any `u8` narrowing.

**SPEC-19 R covered:** R31, R33 (DC-B5 wire path), R34, R48
(correlation-ID preservation + order preservation).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R31 — `RoundStart` variant at disc 8 | T1, T2, T5, T6 (variant dispatch) |
| R31 — `FinalStateRequest` variant at disc 10 | T3 |
| R31 — `RoundStart` field shape (6 fields post-amendment, incl. tuple Vec) | T1 (populated), T2 (empty), T5 (DC-B3), T6 (DC-B5) |
| R33 — `LocalReconnection` inside `RoundStart` | T1, T5 |
| R33 — `PendingCommutation` inside `RoundStart` | T1, T6 |
| R34 — serde + bincode v2 round-trip identity | T1, T2, T3, T4 (blanket), T5, T6 |
| R48 — correlation ID + order preservation on `pending_commutations` | T6 |
| R34 — CRC32C integrity | DEFERRED to TEST-SPEC-0370 |
| R36 — compression SHOULD skip below threshold | DEFERRED to TEST-SPEC-0370 (frame-layer) |
| R37 — discriminant stability | DEFERRED to TEST-SPEC-0371 (byte-level test) |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0368-A | `border_deltas` of size 10_000 | Does varint length prefix overflow? (SPEC-18 R3 confirms no; sanity probe) |
| QA-0368-B | `resolved_borders` with `u32::MAX` element | Varint edge case on the element; T1 fixture uses small values, adversarial extension |
| QA-0368-C | `new_borders` with duplicated `(u32, PortRef)` entries | Coordinator-side dedup is out of scope; serialiser must be indifferent (no dedup in serde path) |
| QA-0368-D | Non-empty `border_deltas` but empty `resolved_borders` and `new_borders` | Asymmetric Vec sizes — regression guard on any zipper-style serde impl |
| QA-0368-E | DISCONNECTED sentinel inside `new_borders` (`PortRef::FreePort(u32::MAX)`) | Does the tuple's `PortRef` round-trip? Already T3 of TEST-SPEC-0366 for the bare struct; here it's inside a tuple inside a Vec |
| QA-0368-F | `FinalStateRequest { round: u32::MAX }` | Varint 5-byte encoding — exercise at the top-end |
| QA-0368-G | `RoundStart` payload size crosses `DEFAULT_COMPRESSION_THRESHOLD` (1024 bytes) | TEST-SPEC-0370's territory — but here a pathological `border_deltas` Vec of 200 elements would force it above. Out of scope for bincode layer |
| QA-0368-H | Variant source order drifts (RoundStart placed at disc 10, FinalStateRequest at disc 8) | Byte-level test in TEST-SPEC-0371 catches this |
| QA-0368-I | `PortRef` variant `AgentPort` port index widened beyond 1 (valid 0..=1) | Panics somewhere downstream during consumption; serde doesn't catch it — spec-level concern, not test here |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 976 → **981** (+5 new
   `#[test]` fns: T1, T2, T3, T5 DC-B3, T6 DC-B5).
2. `cargo test --workspace --lib --features zero-copy` count: 1016 →
   **1021** (+5).
3. `cargo build --workspace` clean, default features.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. `cargo doc --workspace --no-deps` clean.

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- `RoundResult` bincode round-trip + DC-A2 activity invariant → TEST-SPEC-0369.
- Wire-layer integration (frame round-trip, CRC32, R36 compression skip) → TEST-SPEC-0370.
- Byte-level discriminant stability → TEST-SPEC-0371.
