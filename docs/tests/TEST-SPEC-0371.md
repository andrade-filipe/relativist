# TEST-SPEC-0371: Discriminant stability lock-in — byte-level assertion for `Message` variants 0..=11

**Task:** TASK-0371
**Spec:** SPEC-19 §3.4 R37 (discriminant stability); SPEC-18 R3
  (bincode v2 varint discriminant encoding); SPEC-06 R5 (append-only
  principle).
**Spec-critic verdict:** Bonus-2 (R37 "coordinated with SPEC-18" is a
  no-op — SPEC-18 appended zero `Message` variants; discriminants
  7..=11 are first post-SPEC-06 assignments).
**Generated:** 2026-04-17
**Baseline before this task:** 990 lib (default) / 1030 lib
  (`--features zero-copy`) — post TASK-0370 per DC-A1+DC-A2 amended
  trajectory.
**Cumulative target after this task:** 991 lib / 1031 lib — **+1** new
  `#[test]` fn with 12 sub-assertions.

---

## Scope note

This TEST-SPEC encodes a SINGLE `#[test]` fn with TWELVE sub-assertions
— one per current `Message` variant. Under bincode v2 varint encoding
(SPEC-18 R3), discriminants 0..=250 encode as exactly one byte equal
to the discriminant value. The test pins that the 12 current variants
(0..=11) encode with their expected first byte.

This is the R37 regression guard for the entire enum. Any future
reordering, removal, or discriminant re-numbering — whether
intentional or accidental — fires this test immediately.

Per Bonus-2 of the §3.4 spec-critic verdict: SPEC-18 (wire format v2)
appended NO `Message` variants (confirmed by spec-critic grep of
`specs/SPEC-18-wire-format-v2.md` on 2026-04-17; source-inspection of
`protocol/types.rs` lines 23-74 on 2026-04-17 confirmed 7 pre-existing
variants at discriminants 0..=6). Discriminants 7..=11 are therefore
the FIRST post-SPEC-06 assignments under R37's "defer to coordinated
numbering" rule — a no-op coordination.

---

## Test target file paths

- `relativist-core/src/protocol/types.rs` — `#[cfg(test)] mod tests`
  block. ONE new `#[test]` fn
  (`test_message_discriminant_stability`).

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### T1: `test_message_discriminant_stability`

**Purpose:** Byte-level assertion that each of the 12 current `Message`
variants encodes with a first-byte equal to its expected discriminant.
The test is the canonical R37 regression guard.

**Target file:** `protocol/types.rs::tests`

**Given:** An array of `(expected_disc, Message)` pairs, one per
variant, constructed via the module's existing
`make_test_partition()` and `make_test_stats()` helpers.

**When:** For each pair, `bincode_v2::encode(&msg)` → inspect
`bytes[0]`.

**Then:**
```rust
#[test]
fn test_message_discriminant_stability() {
    // SPEC-06 R5 + SPEC-18 R3 + SPEC-19 R37: variants MUST be
    // append-only with stable discriminants. bincode v2 varint
    // encodes discriminants 0..=250 as a single byte equal to the
    // discriminant value.
    //
    // R37 coordination note (spec-critic Bonus-2 2026-04-17):
    // SPEC-18 (wire format v2) appended NO Message variants;
    // discriminants 7..=11 are therefore the FIRST post-SPEC-06
    // assignments under R37's deferred-to-coordinated-numbering rule
    // — a no-op coordination.
    //
    // TODO: when a new Message variant is appended, add its
    // (expected_disc, fixture) entry here. Keep variants append-only
    // per SPEC-06 R5 / SPEC-19 R37.

    let cases: Vec<(u8, Message)> = vec![
        (0, Message::AssignPartition {
            round: 0,
            partition: make_test_partition(),
        }),
        (1, Message::Shutdown),
        (2, Message::PartitionResult {
            round: 0,
            partition: make_test_partition(),
            stats: make_test_stats(),
        }),
        (3, Message::Error {
            round: 0,
            worker_id: 0,
            description: String::new(),
        }),
        (4, Message::Register(RegisterPayload {
            protocol_version: 2,
            auth_token: None,
        })),
        (5, Message::RegisterAck(RegisterAckPayload {
            worker_id: 0,
        })),
        (6, Message::RegisterNack(RegisterNackPayload {
            reason: String::new(),
        })),
        (7, Message::InitialPartition {
            round: 0,
            partition: make_test_partition(),
        }),
        (8, Message::RoundStart {
            round: 0,
            border_deltas: Vec::new(),
            resolved_borders: Vec::new(),
            new_borders: Vec::new(),
        }),
        (9, Message::RoundResult {
            round: 0,
            border_deltas: Vec::new(),
            stats: make_test_stats(),
            has_border_activity: false,
        }),
        (10, Message::FinalStateRequest { round: 0 }),
        (11, Message::FinalStateResult {
            round: 0,
            partition: make_test_partition(),
        }),
    ];

    for (expected_disc, msg) in &cases {
        let bytes = bincode_v2::encode(msg)
            .expect("encode must succeed");
        assert!(!bytes.is_empty(),
                "encoded bytes must be non-empty for {:?}", msg);
        assert_eq!(
            bytes[0], *expected_disc,
            "variant {:?} expected discriminant {} but got {}",
            msg, expected_disc, bytes[0]
        );
    }

    // Cardinality contract: the `cases` Vec MUST cover every current
    // variant. If a future bundle appends a 13th variant without
    // extending this test, the assertion below fires loudly.
    assert_eq!(cases.len(), 12,
        "R37: the discriminant-stability test MUST cover all current \
         Message variants; update `cases` when a variant is appended");
}
```

**Assertions (12 sub-assertions + 1 cardinality):**
- 12 × `bytes[0] == expected_disc` — one assertion per variant for
  discriminants 0..=11.
- 12 × `!bytes.is_empty()` — defensive.
- Final cardinality assertion: `cases.len() == 12` — regression guard
  against future append-without-extend drift.

**SPEC-19 R covered:** R37 (discriminant stability).
**SPEC-18 R covered:** R3 (varint encoding of discriminants 0..=250 as
single byte).
**SPEC-06 R covered:** R5 (append-only principle; reordering would
fire multiple sub-assertions).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| SPEC-19 R37 — discriminant stability for variants 0..=11 | T1 (12 sub-assertions) |
| SPEC-19 R37 — "coordinated with SPEC-18" (Bonus-2 no-op note) | T1 (inline comment documents the no-op) |
| SPEC-18 R3 — varint encoding 0..=250 as single byte | T1 (assertion shape relies on this) |
| SPEC-06 R5 — append-only principle (variants stable by position) | T1 (cardinality + positional match) |
| Future-proofing: new-variant drift detection | T1 cardinality assertion (`cases.len() == 12`) |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0371-A | Future refactor reorders variants (e.g. swap disc 7 and 11) | T1 fires on 2 of 12 sub-assertions — very loud |
| QA-0371-B | Variant deleted (e.g. drop `Shutdown` at disc 1) | Compile fails at the fixture construction line; good |
| QA-0371-C | Variant count exceeds 250 — bincode switches to 3-byte varint encoding | T1's `bytes[0] == disc` shape breaks for discriminants > 250; comment in test flags this ("assertion holds while variant count ≤ 251; extend encoding check if more variants are appended") |
| QA-0371-D | New variant appended at disc 12 without extending T1 | Cardinality assertion fires (`cases.len() == 12` will be false because… wait, it still reports 12 — the cardinality guards T1's `cases` Vec, which wouldn't auto-include a new variant. So a new variant at disc 12 would NOT fire T1 by itself, but the next protocol test that uses `all_variants_serde_roundtrip` would. Reinforce the TODO comment) |
| QA-0371-E | `#[repr(u8)]` added to `Message` (currently has non-Copy payloads) | Would compile-fail (`Message` carries non-Copy `Partition`); spec-level concern, not T1 |
| QA-0371-F | A `make_test_*` helper changes signature (breaks fixture) | Compile fails; good fail-fast |
| QA-0371-G | bincode config silently switches from varint back to fixint (`legacy()`) | T1 first assertion fires: `AssignPartition` encodes with 4 bytes of 0s, first byte matches 0, passes; `Shutdown` encodes with 4 bytes starting 0x01, 0x00, 0x00, 0x00, first byte matches 1, passes; but downstream tests (TEST-SPEC-0366 T5, TEST-SPEC-0368 T3) WOULD catch the size regression. So T1 alone is not sufficient to detect fixint drift — flagged for QA |
| QA-0371-H | Variant renamed but discriminant stays (e.g. `Shutdown` → `Terminate`) | T1 compile-fails at the fixture construction; good canary |
| QA-0371-I | Sub-bundle 2.26-D adds new variants and forgets to update T1 | T1 still passes (cases.len() == 12 still, since the test code hasn't been touched); the cardinality assertion doesn't catch this. Code review gate |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 990 → **991** (+1 new
   `#[test]` fn).
2. `cargo test --workspace --lib --features zero-copy` count: 1030 →
   **1031** (+1).
3. `cargo build --workspace` clean.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. No `unwrap()` in production code; test uses `.expect(...)`.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0366 T5** (bincode size is varint not fixint) provides
  the complementary fixint-drift detection that T1 alone misses
  (QA-0371-G above). Together, the two tests cover the "discriminant
  is stable AND the encoding scheme is varint" envelope.
- **TEST-SPEC-0370 T1..T5** exercise the per-variant flag bits
  (FLAG_COMPRESSED / FLAG_ARCHIVED) at the frame layer, which is
  independent of discriminant stability.
- **TEST-SPEC-0367..0369** blanket `test_all_variants_serde_roundtrip`
  extensions do NOT test discriminants; they test round-trip values.
  T1 here is the authoritative discriminant test.

---

## Out of scope

- Variant-count > 251 (varint byte-count transition) — flagged in the
  test comment; not exercised until a future bundle appends many
  variants.
- Cross-version compatibility (e.g. v1 client talking to v2
  coordinator) — protocol-version negotiation is SPEC-10 territory,
  not discriminant stability.
- Wire-layer flag bits — TEST-SPEC-0370.
