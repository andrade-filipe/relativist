# TEST-SPEC-0369: `Message::RoundResult` (disc 9) — W→C with border deltas + stats + DC-A2 activity invariant

**Task:** TASK-0369
**Spec:** SPEC-19 §3.4 (R32 RoundResult, R34 serde + bincode round-trip,
  R36 compression-skip-below-threshold, R37 discriminant stability)
**Spec-critic verdict:** DC-A2 (Option C — duplicate per R26 literal;
  graph-enforce equality via debug_assert + regression; canonical
  source of truth is `stats.has_border_activity`).
**Generated:** 2026-04-17
**Baseline before this task:** 981 lib (default) / 1021 lib
  (`--features zero-copy`) — post TASK-0368 with DC-B3/DC-B5 amendments
  (+5 tests on TASK-0368 instead of +3; TASK-0366 now +4 instead of +1).
**Cumulative target after this task:** 988 lib / 1028 lib — **+7** tests
  (4 baseline + 1 DC-A2 equality + 1 `#[ignore]` regression stub + 1
  new DC-B5 order-preservation test per SPEC-19 R33/R48 amendment
  2026-04-17: `test_round_result_minted_agents_multi_order_preserved`;
  `#[ignore]` tests count toward test-result totals per cargo
  convention).

---

## Scope note

This task lands the sole **worker → coordinator per-round** variant
`RoundResult` at discriminant 9, carrying:

- `round: u32`
- `border_deltas: Vec<BorderDelta>` — diffs against previous border
  state (R25).
- `stats: WorkerRoundStats` — per-round stats (including
  `stats.has_border_activity` from the §3.1 bundle).
- `has_border_activity: bool` — wire cache of
  `stats.has_border_activity` per R26 literal; graph-enforced to
  equal `stats.has_border_activity` (DC-A2 Option C).
- `minted_agents: Vec<MintedAgent>` — (added by SPEC-19 R33 amendment
  2026-04-17, DC-B5 + R48) the worker's echo of the coordinator's
  `PendingCommutation` requests from the previous round. Each entry
  carries `request_id` (correlation key) + `minted_agent_id`.

**Per DC-A2 of the §3.4 spec-critic verdict:** the top-level
`has_border_activity` field is preserved on the wire (R26 literal) but
the duplication is graph-enforced. The canonical source of truth is
`stats.has_border_activity` — the derivative computed by
`compute_border_activity` (`merge/types.rs` L149, grid.rs convergence
read path). The top-level field is a wire cache for coordinator
pattern-match ergonomics; the two MUST agree at construction (enforced
by debug_assert in the worker-side builder; regression-tested here).

**Bincode layer only.** R36's "SHOULD skip compression below threshold"
verification lands at the wire layer in TEST-SPEC-0370. R37 byte-level
stability lands in TEST-SPEC-0371.

---

## Test target file paths

- `relativist-core/src/protocol/types.rs` — `#[cfg(test)] mod tests`
  block. All 6 new `#[test]` fns (4 baseline + 1 DC-A2 equality +
  1 new DC-B5 order-preservation) + 1 `#[ignore]` regression stub +
  1 blanket-test list extension.

All tests are synchronous `#[test]` units. No `tokio`, no `async`.

---

## Unit Tests

### T1: `round_result_bincode_roundtrip_populated`

**Purpose:** R32/R34 round-trip identity for `RoundResult` with a
non-empty `border_deltas`, the existing `make_test_stats()` fixture,
and `has_border_activity = true`.

**Target file:** `protocol/types.rs::tests`

**Given:**
- `border_deltas = vec![BorderDelta { border_id: 5, new_target: PortRef::AgentPort(42, 0) }, BorderDelta { border_id: 7, new_target: PortRef::FreePort(9) }]`
- `stats = make_test_stats()` (existing helper — includes
  `has_border_activity: true` to match the top-level for T1; adjust
  fixture if needed).
- `has_border_activity = true`
- `minted_agents = vec![MintedAgent { request_id: 42, minted_agent_id: AgentId(103) }]` *(DC-B5)*
- `round = 3`

**When:** Encode → decode → match.

**Then:**
```rust
let stats = make_test_stats_with_activity(true);  // helper variant
let original = Message::RoundResult {
    round: 3,
    border_deltas: vec![
        BorderDelta { border_id: 5, new_target: PortRef::AgentPort(42, 0) },
        BorderDelta { border_id: 7, new_target: PortRef::FreePort(9) },
    ],
    stats,
    has_border_activity: true,
    minted_agents: vec![
        MintedAgent {
            request_id: 42,
            minted_agent_id: AgentId(103),
        },
    ],
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundResult { round, border_deltas, stats,
                            has_border_activity, minted_agents } => {
        assert_eq!(round, 3);
        assert_eq!(border_deltas.len(), 2);
        assert_eq!(border_deltas[0].border_id, 5);
        assert!(has_border_activity);
        assert!(stats.has_border_activity);
        assert_eq!(minted_agents.len(), 1);
        assert_eq!(minted_agents[0].request_id, 42);
        assert_eq!(minted_agents[0].minted_agent_id, AgentId(103));
    }
    other => panic!("expected RoundResult, got {:?}", other),
}
```

**Assertions:**
- Variant dispatch (disc 9).
- All 5 fields round-trip.
- Top-level and nested `has_border_activity` are both `true` after decode.
- `Vec<MintedAgent>` round-trips (DC-B5 wire path).

**SPEC-19 R covered:** R32, R33 (DC-B5), R34, R48 (correlation ID).

---

### T2: `round_result_bincode_roundtrip_empty_deltas_converged_case`

**Purpose:** The "converged" case — empty `border_deltas`,
`has_border_activity = false`. This is the dominant case at
convergence per SPEC-19 R27 (coordinator detects convergence via
`stats.iter().all(|s| !s.has_border_activity)`).

**Target file:** `protocol/types.rs::tests`

**Given:**
- `border_deltas = vec![]`
- `stats` with `has_border_activity = false`
- `has_border_activity = false`
- `round = 10`

**When:** Encode → decode → match.

**Then:**
```rust
let stats = make_test_stats_with_activity(false);
let original = Message::RoundResult {
    round: 10,
    border_deltas: Vec::new(),
    stats,
    has_border_activity: false,
    minted_agents: Vec::new(),  // DC-B5: empty at convergence
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundResult { round, border_deltas, stats,
                            has_border_activity, minted_agents } => {
        assert_eq!(round, 10);
        assert_eq!(border_deltas.len(), 0);
        assert!(!has_border_activity);
        assert!(!stats.has_border_activity);
        assert_eq!(minted_agents.len(), 0);
    }
    other => panic!("expected RoundResult, got {:?}", other),
}
```

**Assertions:**
- Converged-case fields all round-trip to `false` / empty.
- Empty `Vec<BorderDelta>` survives (bincode varint single-byte zero).
- Empty `Vec<MintedAgent>` also survives (DC-B5 common case: no CON-DUP
  resolutions in flight at convergence).

**SPEC-19 R covered:** R32, R34 (edge case: empty delta set).

---

### T3: `round_result_preserves_interactions_by_rule_array`

**Purpose:** `WorkerRoundStats.interactions_by_rule: [u64; 6]` is a
fixed-size array. Guard against silent serde mishandling (e.g. a
future config flag that encodes fixed-size arrays as length-prefixed
`Vec`).

**Target file:** `protocol/types.rs::tests`

**Given:** `stats.interactions_by_rule = [0, 1, 2, 3, 4, 5]`.

**When:** Encode a `RoundResult` carrying those stats; decode; check
array equality.

**Then:**
```rust
let mut stats = make_test_stats();
stats.interactions_by_rule = [0, 1, 2, 3, 4, 5];
stats.has_border_activity = false;
let original = Message::RoundResult {
    round: 0,
    border_deltas: Vec::new(),
    stats,
    has_border_activity: false,
    minted_agents: Vec::new(),
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundResult { stats, .. } => {
        assert_eq!(stats.interactions_by_rule, [0, 1, 2, 3, 4, 5],
                   "interactions_by_rule array must survive intact");
    }
    other => panic!("expected RoundResult, got {:?}", other),
}
```

**Assertions:**
- All 6 array slots preserved.
- Array remains fixed-size (length access via array indexing).

**SPEC-19 R covered:** R32, R34 (array-in-struct-in-variant).

---

### T4: `round_result_activity_flag_independence_from_stats`

**Purpose:** The top-level `has_border_activity` field MUST actually
be serialised (not optimised away by any future derive macro). Assert
that two `RoundResult`s differing ONLY in `has_border_activity`
produce different encoded byte streams.

**Target file:** `protocol/types.rs::tests`

**Given:** Two `RoundResult` messages identical except for the
top-level flag (using the SAME stats value for both; the test is about
the field being transmitted, not about equality-with-stats).

**When:** Encode both.

**Then:**
```rust
let stats = make_test_stats_with_activity(false);
let msg_true = Message::RoundResult {
    round: 0,
    border_deltas: Vec::new(),
    stats: stats.clone(),
    has_border_activity: true,
    minted_agents: Vec::new(),
};
let msg_false = Message::RoundResult {
    round: 0,
    border_deltas: Vec::new(),
    stats,
    has_border_activity: false,
    minted_agents: Vec::new(),
};
let bytes_true = bincode_v2::encode(&msg_true).expect("encode true");
let bytes_false = bincode_v2::encode(&msg_false).expect("encode false");
assert_ne!(bytes_true, bytes_false,
    "top-level has_border_activity MUST be serialised — \
     different values MUST produce different byte streams");
```

**Assertions:**
- The top-level flag contributes to the wire encoding.
- Regression guard: a future `#[serde(skip)]` on the field would cause
  this test to fail.

**SPEC-19 R covered:** R32 (top-level field literal on the wire).

**NOTE:** This test deliberately uses mismatched top-level and stats
activity. It does NOT assert anything about the invariant; that is the
job of T5 (for the agreement check) and T6 (for the debug_assert
firing). The point here is serialisation presence.

---

### T5: `round_result_activity_matches_stats_activity` (DC-A2)

**Purpose:** DC-A2 amendment: after bincode round-trip, the two
`has_border_activity` fields (top-level and nested) remain equal on
well-formed inputs. This verifies that the serde layer preserves the
invariant on both true/true and false/false pairs.

**Target file:** `protocol/types.rs::tests`

**Given:** Two `RoundResult`s with matched-pairs:
(i) `top_level = true`, `stats.has_border_activity = true`;
(ii) `top_level = false`, `stats.has_border_activity = false`.

**When:** Encode → decode each; check equality.

**Then:**
```rust
for top_level in [true, false] {
    let stats = make_test_stats_with_activity(top_level);
    let original = Message::RoundResult {
        round: 0,
        border_deltas: Vec::new(),
        stats,
        has_border_activity: top_level,
        minted_agents: Vec::new(),
    };
    let bytes = bincode_v2::encode(&original).expect("encode");
    let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
        .expect("decode");
    match decoded {
        Message::RoundResult {
            has_border_activity: msg_flag,
            stats: decoded_stats,
            ..
        } => {
            assert_eq!(
                msg_flag, decoded_stats.has_border_activity,
                "DC-A2: after bincode round-trip, top-level \
                 has_border_activity MUST equal stats.has_border_activity \
                 (top_level input was {})",
                top_level
            );
        }
        other => panic!("expected RoundResult, got {:?}", other),
    }
}
```

**Assertions:**
- Serde preserves agreement on well-formed inputs (true/true and
  false/false).
- **NOTE:** This test does NOT construct a mismatched pair (that would
  fire the DC-A2 debug_assert invariant — job of T6). The test asserts
  that bincode does not silently re-sync the two fields on decode.

**SPEC-19 R covered:** R32; **DC-A2 amendment** (equality preserved
across serde layer).

---

### T6: `round_result_activity_invariant_runtime` (DC-A2, `#[ignore]` stub)

**Purpose:** `#[ignore]`'d placeholder for the worker-side
debug_assert invariant that sub-bundle 2.26-C will enable. When
2.26-C ships the `RoundResult` builder, the `#[ignore]` flips off and
this test verifies the debug_assert fires on mismatched
`top_level != stats.has_border_activity` pairs.

**Target file:** `protocol/types.rs::tests`

**Given:** (placeholder — builder doesn't exist yet).

**When:** (deferred).

**Then:**
```rust
#[test]
#[ignore = "TODO(2.26-C): enable once worker_emit_round_result builder lands"]
#[should_panic(expected = "RoundResult invariant")]
fn test_round_result_activity_invariant_runtime() {
    // TODO(2.26-C): enable once the worker-side RoundResult builder
    // exists (sub-bundle 2.26-C). When it ships, invoke the builder
    // with a mismatched pair:
    //     top_level = true, stats.has_border_activity = false
    // and expect the debug_assert! in the builder to fire with the
    // panic message containing "RoundResult invariant".
    //
    // The `#[should_panic(expected = "RoundResult invariant")]`
    // attribute matches the debug_assert message pattern from
    // docs/spec-reviews/SPEC-19-section-3.4-design-choices-2026-04-17.md
    // DC-A2 (4-line panic message starts with
    // "RoundResult invariant: top-level has_border_activity MUST ...").
    //
    // Until 2.26-C: this test is `#[ignore]`'d and counts toward the
    // lib test total but does NOT execute under `cargo test`.
    panic!("stub — enable in 2.26-C");
}
```

**Assertions:**
- Test body currently just `panic!`s; the `#[ignore]` attribute
  suppresses execution.
- Counts toward `cargo test` lib total per cargo convention
  (listed in `N ignored`).

**SPEC-19 R covered:** DC-A2 regression stub — enforces the invariant
once the worker builder exists.

---

### T7 (list-extension, not a new `#[test]`): extend `test_all_variants_serde_roundtrip`

**Purpose:** Add `RoundResult` to the blanket round-trip list.

**Target file:** `protocol/types.rs::tests`
(inside `test_all_variants_serde_roundtrip`).

**Change:** Append:
```rust
Message::RoundResult {
    round: 0,
    border_deltas: Vec::new(),
    stats: make_test_stats(),
    has_border_activity: false,
    minted_agents: Vec::new(),  // DC-B5
},
```

**Assertions:** Blanket test continues passing for all variants.

**SPEC-19 R covered:** R34 (blanket).

---

### T8: `round_result_minted_agents_multi_order_preserved`  *(DC-B5, SPEC-19 R33 + R48 2026-04-17 amendment)*

**Purpose:** Pins R48's wire-layer guarantee that `MintedAgent` entries
survive round-trip with BOTH their `request_id` correlation keys AND
their list order intact. The coordinator uses `request_id` as the
correlation ID to match each `MintedAgent` against an outstanding
`PendingCommutation`; linear-scan matching on the reply depends on
order stability (R48).

**Target file:** `protocol/types.rs::tests`

**Given:** `RoundResult` with `minted_agents` carrying three entries
with distinct `request_id` values (100, 101, 102) and distinct
`AgentId`s (1001, 1002, 1003). All other Vecs empty; `round = 4`.

**When:** Encode → decode → match.

**Then:**
```rust
let stats = make_test_stats_with_activity(false);
let original = Message::RoundResult {
    round: 4,
    border_deltas: Vec::new(),
    stats,
    has_border_activity: false,
    minted_agents: vec![
        MintedAgent { request_id: 100, minted_agent_id: AgentId(1001) },
        MintedAgent { request_id: 101, minted_agent_id: AgentId(1002) },
        MintedAgent { request_id: 102, minted_agent_id: AgentId(1003) },
    ],
};
let bytes = bincode_v2::encode(&original).expect("encode");
let (decoded, _) = bincode_v2::decode_value::<Message>(&bytes)
    .expect("decode");
match decoded {
    Message::RoundResult { minted_agents, .. } => {
        assert_eq!(minted_agents.len(), 3);
        assert_eq!(minted_agents[0].request_id, 100);
        assert_eq!(minted_agents[0].minted_agent_id, AgentId(1001));
        assert_eq!(minted_agents[1].request_id, 101);
        assert_eq!(minted_agents[1].minted_agent_id, AgentId(1002));
        assert_eq!(minted_agents[2].request_id, 102);
        assert_eq!(minted_agents[2].minted_agent_id, AgentId(1003));
    }
    other => panic!("expected RoundResult, got {:?}", other),
}
```

**Assertions:**
- Each `request_id` survives round-trip (R48 correlation integrity).
- Element order is preserved in the `Vec` (R48 linear-scan matching
  property on the coordinator).
- Each `minted_agent_id` is preserved losslessly.
- Distinct (request_id, minted_agent_id) tuples don't cross-contaminate.

**SPEC-19 R covered:** R32, R33 (DC-B5 wire path), R34, R48 (correlation
ID + order preservation on the reply side).

---

## Coverage mapping

| Requirement | Covered by |
|---|---|
| R32 — `RoundResult` variant at disc 9 | T1, T2, T3, T4, T5, T7 (blanket), T8 |
| R32 — all 5 fields (round, border_deltas, stats, has_border_activity, minted_agents) present on wire | T1 (populated), T2 (empty/false converged), T4 (flag independence), T8 (DC-B5) |
| R33 — `MintedAgent` inside `RoundResult.minted_agents` | T1 (populated case), T2 (empty case), T8 (multi-entry order) |
| R34 — serde + bincode v2 round-trip identity | T1, T2, T3, T5, T7 (blanket), T8 |
| R34 — `WorkerRoundStats` array `[u64; 6]` preserved | T3 |
| R48 — `request_id` correlation + order preservation on `minted_agents` reply | T8 |
| R34 — CRC32C integrity | DEFERRED to TEST-SPEC-0370 |
| R36 — compression SHOULD skip | DEFERRED to TEST-SPEC-0370 |
| R37 — discriminant stability | DEFERRED to TEST-SPEC-0371 |
| DC-A2 — equality of top-level and nested `has_border_activity` preserved by serde | T5 |
| DC-A2 — graph-enforced invariant at worker builder (debug_assert) | T6 (ignored stub for 2.26-C) |
| DC-A2 — canonical source of truth is `stats.has_border_activity` | Doc-comment + T5 (assertion reads stats as source-of-truth) |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|----------|---------------|
| QA-0369-A | Mismatched `top_level = true` / `stats.has_border_activity = false` via direct struct literal (not the builder) | Serde round-trips the lie; coordinator convergence detector sees both values and acts on the lie. DC-A2 debug_assert in the builder is the firewall — but direct struct construction bypasses it. QA should scan for direct literal construction in `src/`; code review gate |
| QA-0369-B | `border_deltas` with 10_000 entries | Payload size crosses compression threshold; verified in TEST-SPEC-0370 forced-compression test |
| QA-0369-C | `WorkerRoundStats` gains a 7th `interactions_by_rule` slot | T3 compile-time check on `[u64; 6]` fires |
| QA-0369-D | `has_border_activity` field reordered to first position in variant | bincode is positional; byte stream shifts; T5's equality check still passes, but T4's ne-bytes probably still holds. Byte-level diff from TEST-SPEC-0371 also won't catch it (TEST-SPEC-0371 tests disc byte only). Architecture-review concern |
| QA-0369-E | Future refactor: `has_border_activity` renamed to `had_border_activity` | All tests compile-fail; canary |
| QA-0369-F | `stats` field reordered after `has_border_activity` | Bincode is positional; all round-trip tests still pass, but cross-version peers would see different encoding. Architecture-review concern |
| QA-0369-G | Future `#[serde(skip)]` on `has_border_activity` | T4 fires (bytes_true == bytes_false) |
| QA-0369-H | 2.26-C lands builder but doesn't flip T6's `#[ignore]` | Invariant isn't exercised; spec-critic Stage 5 review should grep for `TODO(2.26-C)` + `#[ignore]` on this test |

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 981 → **988** (+4 baseline
   `#[test]` fns + 1 DC-A2 equality test + 1 `#[ignore]` stub +
   1 new DC-B5 order-preservation test = +7 total; cargo counts
   `#[ignore]` tests in the `N ignored` line which contributes to the
   reported total count).
2. `cargo test --workspace --lib --features zero-copy` count: 1021 →
   **1028** (+7).
3. `cargo build --workspace` clean, default features.
4. `cargo build --workspace --features zero-copy` clean.
5. `cargo clippy --workspace --all-targets -- -D warnings` clean.
6. `cargo fmt --check` clean.
7. `cargo doc --workspace --no-deps` clean (new `///` comment on
   `has_border_activity` renders canonical-vs-cache wording).

---

## Out of scope (deferred to later TEST-SPECs in the bundle)

- Wire-layer integration (frame round-trip, CRC32, R36 compression skip) → TEST-SPEC-0370.
- Byte-level discriminant stability → TEST-SPEC-0371.
- Worker-side builder debug_assert enablement → sub-bundle 2.26-C (T6 stub enabled there).
