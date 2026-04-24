# TEST-SPEC-0401: Resolver-to-wire transport — populate `PendingCommutation.target_symbols` + `local_wiring` from `CommutationBatch`

**See also:** [docs/backlog/TASK-0401.md](../backlog/TASK-0401.md); [docs/tests/TEST-SPEC-0400.md](./TEST-SPEC-0400.md).

**Task:** TASK-0401
**Parent spec:** SPEC-19 §3.3 R23 + R23a clauses 1-4, §3.4 R33/R34, §3.6 R48b.
**Bundle:** D-005 Option A.
**Date:** 2026-04-23
**Baseline before this task:** 1152 lib default / 1195 lib `--features zero-copy` (post-TASK-0400).
**Cumulative target after this task:** **≥ 1158 / ≥ 1201** (+6 both configs; tests are feature-agnostic).

---

## Scope

### Covers
1. Per-rule `CommutationBatch → PendingCommutation` fidelity for each of the 6 IC rules (CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA) — verify PCs are emitted for asymmetric rules with correct content, and ABSENT for symmetric rules (which use `local_reconnections` not `commutation_batches`).
2. Slot-marker placeholder preservation: `AgentPort(SLOT_MARKER_BASE + k, p)` round-trip from resolver to PC.
3. Concrete-id preservation: `AgentPort(live_id, p)` with `live_id < SLOT_MARKER_BASE` round-trip.
4. Empty `local_wiring` emission is legal (R48b) — ERA fixture with minimal hints.
5. Emission-order preservation: `CommutationBatch.local_wiring` order equals `PendingCommutation.local_wiring` order byte-for-byte.
6. Request_id encoding stability (`encode_request_id(batch.commutation_id, 0)`).
7. Fan-out collapse: 1 `CommutationBatch` → 1 `PendingCommutation` (not N per slot).

### Does NOT cover
- Wire-level bincode/rkyv round-trip (TEST-SPEC-0400).
- Worker-side `R24.1.6b` decoding and R33c rejection (TEST-SPEC-0402).
- End-to-end G1 parity (TEST-SPEC-0403).
- `CommutationBatch` internal construction by the resolver reduction step itself (existing tests in `border_resolver.rs`).

---

## Test target file paths

- `relativist-core/src/merge/border_resolver.rs` — inline `#[cfg(test)] mod tests` block. New `#[test]` fns: UT-0401-01 through UT-0401-06.
- Optional: dedicated `border_resolver_wiring_tests.rs` submodule if inline tests would bloat the resolver file — developer discretion.

All tests synchronous. No tokio, no async.

---

## Unit Tests

### UT-0401-01: `package_resolutions_with_pending_con_dup_populates_target_symbols_and_wiring`

**Purpose:** For a CON-DUP cross-partition redex, `package_resolutions_with_pending` emits exactly ONE `PendingCommutation` whose `target_symbols` carries the two sibling symbols in resolver-emission order, and whose `local_wiring` carries every `(u8, u8, PortRef)` hint the resolver inserted into `batch.local_wiring`.

**Target:** `border_resolver.rs::tests`.

**Given:**
- A minimal 2-partition net with a CON agent in partition 0 and a DUP agent in partition 1, connected principal-to-principal across the border.
- The resolver runs one step, producing a `CommutationBatch` with `target_symbols == vec![Symbol::Dup, Symbol::Con]` (or `[Con, Dup]` — whichever the resolver actually emits; the test captures the reference order from the batch itself).
- `batch.local_wiring` populated with the emitted hints at `border_resolver.rs:820-866`. Expected count depends on fixture: typical CON-DUP with both aux ports routed to sibling placeholders emits 4 hints (2 output agents × 2 aux edges). Fixture MAY produce fewer if some aux connections are border-crossing and go through `PendingNewBorder` instead.

**When:**
```
let dispatches = package_resolutions_with_pending(
    &mut graph,
    &resolutions,
    &mut commutation_counter,
    /* ... */
);
let pcs: Vec<&PendingCommutation> = dispatches
    .iter()
    .flat_map(|d| d.pending_commutations.iter())
    .collect();
```

**Then:**
- `pcs.len() == 1` (fan-out collapse: ONE PC per batch, not one per slot — R48 / Shape A).
- `pcs[0].target_symbols == batch.target_symbols` (full vector equality, preserving slot-index → symbol binding).
- `pcs[0].local_wiring.len() == batch.local_wiring.len()` (no filtering, no deduplication).
- For each index `i`: `pcs[0].local_wiring[i].src_slot == batch.local_wiring[i].0`, `src_port == .1`, `target == .2`.
- `pcs[0].request_id == encode_request_id(batch.commutation_id, 0)` (slot-0 convention; canonical correlation anchor per R48 / SPEC-19 §3.3 R23a clause 2).

**Edge cases:**
- EC-1: CON-DUP where both aux ports go to sibling-slot placeholders — all 4 `target` fields have `x >= SLOT_MARKER_BASE`.
- EC-2: CON-DUP where one aux port is a concrete border (goes to `PendingNewBorder`) — `local_wiring` has FEWER hints; verify no spurious hints are added.

**SPEC covered:** R23 wire shape for asymmetric rules, R23a clause 2 request_id derivation, R33 struct fidelity.

**Priority:** MANDATORY (P0 — the core bug being fixed).

---

### UT-0401-02: `package_resolutions_with_pending_con_era_emits_two_eras_and_partial_wiring`

**Purpose:** For a CON-ERA cross-partition redex, the resolver emits ONE PC with `target_symbols == vec![Symbol::Era, Symbol::Era]` (two mint instructions, arity 0 each) and `local_wiring` carrying at most a handful of hints — CON's aux ports are routed to the two new ERA principals, typically 2 hints total.

**Target:** `border_resolver.rs::tests`.

**Given:**
- CON agent in partition 0 with aux ports connected locally (principal is the cross-partition side).
- ERA agent in partition 1.
- CON-ERA resolved by the resolver; emits at `border_resolver.rs:1023-1080`.

**When:** Same as UT-0401-01.

**Then:**
- `pcs.len() == 1`.
- `pcs[0].target_symbols == vec![Symbol::Era, Symbol::Era]`.
- `pcs[0].local_wiring.len()` matches the resolver's emission — typically 2 hints mapping CON aux ports → ERA principal ports.
- Each hint's `target` is an `AgentPort` with either slot-marker (sibling ERA) or concrete-id (existing CON aux target).

**Edge cases:**
- EC-1: if CON's both aux ports are ALREADY concrete IDs in the worker's partition, expect 2 hints with `target = AgentPort(concrete_id, port)`.
- EC-2: if CON's aux ports are themselves cross-partition (unusual fixture), the hints route to `FreePort(...)` — which is R33c case 6 territory. The resolver may legitimately emit such hints per clause 6 (ReservedForFuture); the worker rejects them. TEST-SPEC-0401 just verifies the resolver PASSES THEM THROUGH; the worker-side rejection is TEST-SPEC-0402.

**SPEC covered:** R23 + R23a for CON-ERA, R33, R48a concerning stray slot-marker absence.

**Priority:** MANDATORY.

---

### UT-0401-03: `package_resolutions_with_pending_dup_era_symmetric_to_con_era`

**Purpose:** DUP-ERA emits the same SHAPE as CON-ERA (2 ERA mints + local_wiring from DUP aux ports). This test pins the symmetry and guards against accidental rule-specific divergence.

**Target:** `border_resolver.rs::tests`.

**Given:** DUP in partition 0 + ERA in partition 1, principal-connected.

**When:** Same.

**Then:**
- `pcs.len() == 1`.
- `pcs[0].target_symbols == vec![Symbol::Era, Symbol::Era]`.
- `pcs[0].local_wiring` has the same element count as the CON-ERA counterpart (with DUP-specific port-routing).

**Edge cases:**
- EC-1: asymmetric-wiring DUP where aux port 1 is a principal of another live agent (creates a principal-principal redex with the minted ERA). Verify the hint is emitted verbatim; the in-round redex is consumed at R24.2 by the WORKER (TEST-SPEC-0402 territory).

**SPEC covered:** R23 + R23a for DUP-ERA.

**Priority:** MANDATORY.

---

### UT-0401-04: `package_resolutions_with_pending_symmetric_rules_emit_no_pending_commutations`

**Purpose:** Symmetric rules (CON-CON, DUP-DUP, ERA-ERA) are resolved via `local_reconnections` and `resolved_borders`; they DO NOT produce `CommutationBatch` entries and therefore produce ZERO `PendingCommutation`s.

**Target:** `border_resolver.rs::tests`.

**Given:** 3 separate sub-fixtures — CON-CON, DUP-DUP, ERA-ERA.

**When:** Run resolver on each, then call `package_resolutions_with_pending`.

**Then (per sub-fixture):**
- `dispatches.iter().flat_map(|d| d.pending_commutations.iter()).count() == 0`.
- `local_reconnections` is non-empty (for CON-CON and DUP-DUP) or both sides produce `resolved_borders` (for ERA-ERA).

**Edge cases:**
- EC-1: CON-CON that would theoretically "look asymmetric" via mismatched auxiliary wiring — still symmetric in IC terms; verify no PC emitted.
- EC-2: ERA-ERA produces zero new agents and zero border activity; `dispatches` may be entirely empty.

**SPEC covered:** R23 wire path for symmetric rules (negative space — asserts ABSENCE).

**Priority:** MANDATORY (regression canary — ensures TASK-0401 doesn't accidentally emit PCs for the symmetric path).

---

### UT-0401-05: `commutation_batch_to_pending_preserves_local_wiring_order_verbatim`

**Purpose:** R23a determinism guarantee — the resolver emits `local_wiring` entries in a deterministic order; the wire transport MUST preserve that order byte-for-byte. No sorting, no re-ordering, no deduplication.

**Target:** `border_resolver.rs::tests`.

**Given:** Synthetic `CommutationBatch`:
```
let batch = CommutationBatch {
    commutation_id: 7,
    target_symbols: vec![Symbol::Con, Symbol::Con, Symbol::Con],  // 3-slot case
    local_wiring: vec![
        (2, 1, PortRef::AgentPort(SLOT_MARKER_BASE + 0, 2)),  // emission 1
        (0, 2, PortRef::AgentPort(99, 0)),                     // emission 2 — concrete id
        (1, 1, PortRef::AgentPort(SLOT_MARKER_BASE + 2, 1)),  // emission 3
    ],
    // ... other CommutationBatch fields as required by the struct's current shape
};
let pc = commutation_batch_to_pending(&batch, encode_request_id(7, 0));
```

**When:** Invoke the conversion helper directly (or exercise via `package_resolutions_with_pending` with a carefully-crafted fixture).

**Then:**
- `pc.local_wiring.len() == 3`.
- `pc.local_wiring[0] == LocalWiringHint { src_slot: 2, src_port: 1, target: AgentPort(SLOT_MARKER_BASE + 0, 2) }`.
- `pc.local_wiring[1] == LocalWiringHint { src_slot: 0, src_port: 2, target: AgentPort(99, 0) }`.
- `pc.local_wiring[2] == LocalWiringHint { src_slot: 1, src_port: 1, target: AgentPort(SLOT_MARKER_BASE + 2, 1) }`.
- `pc.target_symbols == vec![Symbol::Con, Symbol::Con, Symbol::Con]`.
- `pc.request_id == encode_request_id(7, 0)`.

**Edge cases:**
- EC-1: batch with duplicate `(src_slot, src_port)` keys (legal at resolver level per TASK-0401 §3 explicit non-guarantee) — verify the duplicates SURVIVE transport (the worker rejects them at R33c case 5).
- EC-2: empty `local_wiring` → `pc.local_wiring == vec![]` (R48b).

**SPEC covered:** R23a determinism; R48b empty-legal on wire.

**Priority:** MANDATORY.

---

### UT-0401-06: `commutation_batch_to_pending_request_id_matches_encode_slot_zero`

**Purpose:** Fan-out collapse invariant — after Shape A migration, exactly one PC is emitted per batch with `request_id = encode_request_id(commutation_id, 0)` (slot 0 is the canonical correlation anchor). The coordinator's D-004 `decode_request_id` expects slot 0 and would misroute on any other slot.

**Target:** `border_resolver.rs::tests`.

**Given:**
```
let batch = CommutationBatch {
    commutation_id: 0x0BCD_ABCD,  // fits within low-28-bit
    target_symbols: vec![Symbol::Dup, Symbol::Con],
    local_wiring: vec![/* any content */],
    // ...
};
```

**When:** `commutation_batch_to_pending(&batch, encode_request_id(batch.commutation_id, 0))`.

**Then:**
- `pc.request_id == encode_request_id(0x0BCD_ABCD, 0)`.
- Inverse: `decode_request_id(pc.request_id) == (0x0BCD_ABCD, 0)` (round-trip via the helper pair shipped in TASK-0398).

**Edge cases:**
- EC-1: `commutation_id = 0` → `pc.request_id == encode(0, 0)` — minimum correlation value.
- EC-2: `commutation_id = 0x0FFF_FFFF` → still fits in the low-28-bit; encoding survives.
- EC-3: if any code path ACCIDENTALLY calls `encode_request_id(commutation_id, k)` for `k > 0` per-slot within a PC, the test catches it: assert `decode_request_id(pc.request_id).1 == 0`, not `k`.

**SPEC covered:** R48 correlation key, DC-0398-A encoding contract, Shape A fan-out collapse.

**Priority:** MANDATORY.

---

## Property Tests

### PT-0401-01 (optional): `prop_commutation_batch_to_pending_is_structural_identity_on_shared_fields`

**Property:** For any arbitrary `CommutationBatch` with shape-valid fields, `commutation_batch_to_pending(&batch, r)` produces a PC where `target_symbols` equals `batch.target_symbols` (clone) and `local_wiring` is `.iter().map(cast)` of `batch.local_wiring`.

**Generator strategy:**
- `arb_commutation_id()` → `u32` in `0..0x0FFF_FFFF`.
- `arb_target_symbols()` → `Vec<Symbol>` with length `1..=16`.
- `arb_local_wiring(target_symbols_len)` → `Vec<(u8, u8, PortRef)>` with `src_slot` valid and mixed targets.

**Assertion:**
```
prop_assert_eq!(pc.target_symbols, batch.target_symbols);
prop_assert_eq!(pc.local_wiring.len(), batch.local_wiring.len());
for i in 0..pc.local_wiring.len() {
    prop_assert_eq!(pc.local_wiring[i].src_slot, batch.local_wiring[i].0);
    prop_assert_eq!(pc.local_wiring[i].src_port, batch.local_wiring[i].1);
    prop_assert_eq!(pc.local_wiring[i].target, batch.local_wiring[i].2);
}
prop_assert_eq!(pc.request_id, encode_request_id(batch.commutation_id & 0x0FFF_FFFF, 0));
```

**Priority:** OPTIONAL. The 6 unit tests already cover the categorical cases; proptest is belt-and-braces. Include if proptest is active in the crate test plan.

---

## Integration Tests

None at this task level. Integration is TEST-SPEC-0403's territory (LocalDeltaDispatch end-to-end). TEST-SPEC-0401's highest abstraction is `package_resolutions_with_pending` — an intra-module API.

---

## Negative Tests

None at this task level. The resolver is a PRODUCER of PCs; the receiver (worker, TEST-SPEC-0402) owns rejection. Any malformed input the resolver produces is by construction accepted or caught by `debug_assert!` (NF-004 `!target_symbols.is_empty()`).

The single assertion boundary at this task's scope:
- **debug_assert NF-004:** if TASK-0401 added `debug_assert!(!batch.target_symbols.is_empty())` at the conversion site, add UT-0401-NF to assert a debug panic on an empty batch. Given `debug_assert!` is stripped in release builds, this test is `#[cfg(debug_assertions)]`-gated and uses `#[should_panic(expected = "NF-004")]`.

**Priority:** OPTIONAL (defensive; the resolver is believed to never emit empty batches under G1-correct execution).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-0401-01 | CON-DUP with all-placeholder aux routing | 4 hints, all with `target = AgentPort(SLOT_MARKER_BASE + k, p)` | UT-0401-01 EC-1 |
| EC-0401-02 | CON-DUP with partial concrete-id aux routing | Mixed hints; count `<= 4` | UT-0401-01 EC-2 |
| EC-0401-03 | CON-ERA with both CON aux ports concrete | 2 hints, both `target = AgentPort(concrete, _)` | UT-0401-02 EC-1 |
| EC-0401-04 | DUP-ERA with one aux port principal-principal | Hint is passed through; in-round redex is WORKER territory | UT-0401-03 EC-1 |
| EC-0401-05 | Symmetric rule CON-CON on border | 0 PCs emitted | UT-0401-04 |
| EC-0401-06 | Synthetic batch with duplicate (src_slot, src_port) | Duplicates SURVIVE transport | UT-0401-05 EC-1 |
| EC-0401-07 | Synthetic batch with empty local_wiring | `pc.local_wiring == vec![]` | UT-0401-05 EC-2 |
| EC-0401-08 | `commutation_id == 0x0FFF_FFFF` boundary | round-trip encode/decode stable | UT-0401-06 EC-2 |

---

## Coverage mapping

| Requirement / contract | Covered by |
|------------------------|-----------|
| SPEC-19 R23 wire payload content (asymmetric rules) | UT-0401-01, 02, 03 |
| SPEC-19 R23 (absence for symmetric rules) | UT-0401-04 |
| SPEC-19 R23a clause 1 (slot namespace within PC) | UT-0401-01 structural (per-slot symbol binding) |
| SPEC-19 R23a clause 2 (request_id derivation) | UT-0401-06 |
| SPEC-19 R23a clause 3 (slot-marker placeholder emission) | UT-0401-01 EC-1, UT-0401-05 |
| SPEC-19 R23a clause 4 (concrete-id pass-through) | UT-0401-05 (middle entry), UT-0401-01 EC-2 |
| SPEC-19 R23a determinism guarantee (order) | UT-0401-05 |
| SPEC-19 R33 struct fidelity | UT-0401-01 through 05 (all) |
| SPEC-19 R34 wire gate (bincode format) | UT-0401-01 through 06 implicitly (the PCs emitted are bincode-compatible per TEST-SPEC-0400) |
| SPEC-19 R48 correlation key | UT-0401-06 |
| SPEC-19 R48b empty-legal on wire (resolver side) | UT-0401-05 EC-2 |
| Fan-out collapse (Shape A design invariant) | UT-0401-06 EC-3 (slot-0 anchor) + UT-0401-01 (`pcs.len() == 1`) |
| NF-004 resolver-side `!target_symbols.is_empty()` guard | UT-0401-NF (OPTIONAL, debug-assertions-only) |

---

## Test count estimate

| Config | Baseline | UTs added | Target |
|--------|----------|-----------|--------|
| `cargo test --workspace --lib` | 1152 | +6 (UT-0401-01..06) | **≥ 1158** |
| `cargo test --workspace --lib --features zero-copy` | 1195 | +6 | **≥ 1201** |

PT-0401-01 is OPTIONAL; if included, both targets +1. UT-0401-NF is OPTIONAL; if included, default +1 under debug assertions only (not counted in release-mode runs).

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1152 → ≥ 1158.
2. `cargo test --workspace --lib --features zero-copy` count: 1195 → ≥ 1201.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
4. `cargo fmt --check` clean.
5. UT-0385-06/07 (symmetric-rule G1 parity) still green — regression canary.
6. `SKIP_ASYMMETRIC` remains `true` — flip is TASK-0403.
7. No regression on existing tests; baseline 1152 / 1195 lower bound holds.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0400:** wire types (`PendingCommutation`, `LocalWiringHint`). TEST-SPEC-0401 depends on UT-0400-01 correctness; if UT-0400-01 fails, all of TEST-SPEC-0401's UTs fail sympathetically.
- **TEST-SPEC-0402:** consumes PCs produced by the resolver. TEST-SPEC-0401 guarantees the PCs are well-formed; TEST-SPEC-0402 verifies worker-side decoding.
- **TEST-SPEC-0403:** bundle acceptance — exercises TEST-SPEC-0401 end-to-end via LocalDeltaDispatch.
- **TEST-SPEC-0398 UT-0398-01:** `encode_request_id`/`decode_request_id` round-trip at BorderGraph level. UT-0401-06 relies on the same helpers at resolver level.

---

## Out of scope

- **Resolver internal reduction logic** — the production rules at `border_resolver.rs:820-866` (CON-DUP) and `:1023-1080` (CON-ERA / DUP-ERA) are tested by preexisting UTs. TEST-SPEC-0401 tests the TRANSPORT, not the production.
- **Fan-out collapse observability via counts** — if the pre-Shape-A resolver emitted `N` PCs per batch (one per slot), and post-Shape-A emits 1, a "before/after count" test is NOT in this TEST-SPEC. UT-0401-01's explicit `pcs.len() == 1` assertion is the equivalent.
- **Performance of `.clone()` on `Vec<Symbol>`** — trivial; not tested.
- **Concurrent resolver invocation** — BSP is sequential at the coordinator level.
