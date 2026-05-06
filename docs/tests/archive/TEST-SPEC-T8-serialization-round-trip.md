# TEST-SPEC-T8: Serialization round-trip with non-empty free-list (closes SC-014)

**SPEC-22 §7.1 ID:** T8.
**Owning task:** TASK-0475 (free-list serde participation), TASK-0491 (`is_behaviorally_equal` helper).
**Parent spec:** SPEC-22 §3.1 R9; §3.2 R21; §6.1 step 6 (serde round-trip).
**Type:** integration.
**Theory anchor:** AC-006 (HVM2 arena serialization rationale); REF-002 (IC structural identity is wire-faithful).

---

## Inputs / Fixtures

- A `Net` built and partially reduced:
  1. Build a CON-DUP active pair as in T6.
  2. Run `reduce_step()` once (fires CON-DUP, leaves 4 live agents and 2 freed/recycled slots — but per T6, free-list ends empty).
  3. Add 2 explicit `remove_agent` calls on 2 of the 4 resulting agents to force a non-empty free-list.
  - Post-state: `agents.len() ~ 6`, `free_list.len() == 2`, `next_id` per the §3.8 A3 accounting.
- bincode encoder/decoder using the existing SPEC-18 `Net` serde path.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-T8-01 | `bincode_round_trip_preserves_free_list_set` | `let bytes = bincode::serialize(&net).unwrap(); let net2 = bincode::deserialize::<Net>(&bytes).unwrap();` | compare `net.free_list.iter().copied().collect::<HashSet<_>>()` with `net2.free_list.iter().copied().collect::<HashSet<_>>()` | sets equal. (LIFO order is preserved by `Vec` serde — full-vec equality is also acceptable.) |
| UT-T8-02 | `bincode_round_trip_is_behaviorally_equal` | same | `net.is_behaviorally_equal(&net2)` (helper from TASK-0491) | returns `true`. |
| UT-T8-03 | `bincode_round_trip_continuing_reduction_converges_identically` | clone the net pre-deserialize; reduce both `net` (original) and `net2` (deserialized) to normal form independently | `net_final.is_behaviorally_equal(&net2_final)` returns `true`. |
| UT-T8-04 | `validate_free_list_passes_post_deserialize` | post-deserialize | `net2.validate_free_list()` (helper from TASK-0475) | `Ok(())` — every ID in `free_list` corresponds to a `None` slot. |
| UT-T8-05 | `freeport_redirects_skipped_in_serde_unaffected` | net with 1 entry in `freeport_redirects` (skipped via `#[serde(skip)]`) | round-trip | `net2.freeport_redirects.is_empty()` (existing SPEC-02 contract; `is_behaviorally_equal` accommodates this since the helper compares freeport_redirects directly per R21 — both sides are empty post-skip). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Net with empty free-list | Round-trip succeeds; `net2.free_list.is_empty()`. (Confirms backward-compat with v2-baseline serialized nets.) |
| EC-2 | Net with free-list containing 1000 entries (stress the Vec serde path) | Round-trip succeeds; `net.free_list == net2.free_list` byte-for-byte (Vec serde preserves order). |
| EC-3 | Net deserialized from a bytestring that has been bit-flipped in the free-list region | bincode returns `Err(_)` — corrupted deserialization. (Negative test; protects against silent corruption.) |
| EC-4 | rkyv zero-copy round-trip (under `--features zero-copy`) | Same assertions hold via `rkyv::from_bytes`/`rkyv::to_bytes`. (Joint coverage with TEST-SPEC-0475.) |

## Invariants asserted

- R9 (free-list participates in serde with valid post-deserialize state).
- R21 (round-trip behavioral equality — closes SC-014).
- I3' (uniqueness preserved across serde).

## ARG/DISC/REF citation

- AC-006 (HVM2 arena serialization).
- REF-002 (IC structural identity).

## Determinism notes

bincode is deterministic (fixed encoding for a given `Net`); rkyv is also deterministic. Reduction strategy in UT-T8-03 may differ between `net` and `net2` (different runtime instances), but `is_behaviorally_equal` is confluence-safe per R21 (compares up to redex-queue ordering and trailing-slot trim).

The test does **NOT** rely on `==` on `Net` for the post-deserialize comparison — `==` would trigger the trailing-slot ambiguity that R21 explicitly resolved (SC-008 / SC-014). UT-T8-02 uses `is_behaviorally_equal` exclusively.

## Cross-test dependencies

- Hard-depends on TASK-0491 (`is_behaviorally_equal`); if T8 lands before TASK-0491, UT-T8-02 falls back to byte-equality with explicit normalization (trim trailing `None`/`DISCONNECTED`) and migrates to `is_behaviorally_equal` once the helper lands.
- T8a (TASK-0476) tests the version-mismatch path; T8 tests the same-version round-trip.
- TEST-SPEC-0475 plumbing test covers the Vec serde primitive.
