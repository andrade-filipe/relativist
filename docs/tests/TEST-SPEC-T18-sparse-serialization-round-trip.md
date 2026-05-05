# TEST-SPEC-T18: SparseNet serialization round-trip

**SPEC-22 §7.2 ID:** T18.
**Owning task:** TASK-0486 (SparseNet derives Serialize/Deserialize per R18).
**Parent spec:** SPEC-22 §3.2 R18 (Debug, Clone, PartialEq, Eq, Serialize, Deserialize derives).
**Type:** unit.
**Theory anchor:** None direct.

---

## Inputs / Fixtures

- A `SparseNet` constructed identically to T15's fixture: 10 agents, non-trivial wiring, populated `freeport_redirects` (1 entry).
- bincode encoder/decoder.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T18-01 | `bincode_round_trip_structural_equal` | original `sparse` | `let bytes = bincode::serialize(&sparse).unwrap(); let sparse2 = bincode::deserialize::<SparseNet>(&bytes).unwrap();` | `sparse == sparse2` (full `==` per R21 round-trip 2 logic — SparseNet has no trailing-slot ambiguity). |
| UT-T18-02 | `agents_hashmap_round_trip` | same | `sparse.agents == sparse2.agents` | `true`. |
| UT-T18-03 | `ports_hashmap_round_trip` | same | `sparse.ports == sparse2.ports` | `true`. |
| UT-T18-04 | `redex_queue_round_trip_byte_equal` | same | `sparse.redex_queue == sparse2.redex_queue` | `true`. |
| UT-T18-05 | `freeport_redirects_skipped_in_serde` | original has 1 entry in freeport_redirects (`#[serde(skip)]` per TASK-0486) | round-trip | `sparse2.freeport_redirects.is_empty()`. (R18 requires the derive; the `#[serde(skip)]` attribute on the field — mirroring Net — means the field is NOT on the wire. Documented in TASK-0486.) |
| UT-T18-06 | `next_id_root_round_trip` | same | `sparse.next_id == sparse2.next_id && sparse.root == sparse2.root` | `true`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty SparseNet | Round-trip succeeds; both sides empty. |
| EC-2 | Large sparse (10000 agents) | Round-trip succeeds; bincode handles arbitrary HashMap sizes. |
| EC-3 | Sparse with redex queue containing 100 entries | All queue entries preserved in order. |
| EC-4 | Bit-flipped bytestring in the agents region | bincode returns `Err(_)` — deterministic corruption detection. |

## Invariants asserted

- R18 (SparseNet derives Serialize, Deserialize).
- R21 round-trip 2 (structural equality on SparseNet).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

bincode is deterministic. `HashMap::==` is order-independent. Pure synchronous; no tokio.

UT-T18-05 documents that `freeport_redirects` is `#[serde(skip)]` on SparseNet — same as on Net. This is a deliberate design choice (the field is partition-state, not on the wire). When deserializing, the field defaults to empty. This means a round-trip discards `freeport_redirects` content. Code paths that need to preserve the field across a serde boundary must save/restore it explicitly OR use `to_dense`/`to_sparse` conversions (which DO copy the field per TASK-0489 / TASK-0490).

## Cross-test dependencies

- T15 covers the conversion round-trip (Sparse → Dense → Sparse) where `freeport_redirects` IS preserved.
- T8 covers Net serde round-trip (free-list IS serialized; freeport_redirects is NOT).
- TEST-SPEC-0486 covers the SparseNet derive primitive.
