# TEST-SPEC-0475: Free-list serde + bincode round-trip (R9)

**SPEC-22 §7 ID:** T8 (spec-catalog) plus this plumbing file.
**Owning task:** TASK-0475.
**Parent spec:** SPEC-22 §3.1 R9; §6.1 step 6.
**Type:** unit (serde primitive level).

---

## Inputs / Fixtures

- A `Net` with non-empty `free_list`:
  - Create 5 agents (IDs 0-4).
  - Remove IDs 1 and 3 ⇒ `free_list = [1, 3]`.
- bincode encoder/decoder (existing serde path).

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0475-01 | `free_list_field_in_serde_output` | net with `free_list = [1, 3]` | `let bytes = bincode::serialize(&net).unwrap();` | bincode bytes contain 2 u32 values for `free_list`; the bytestring length is consistent with a `Vec<u32>` of length 2 (verifiable via `bincode::serialize_size`). |
| UT-0475-02 | `serde_round_trip_preserves_free_list_order` | `free_list = [1, 3]` (push order) | round-trip | deserialized net has `free_list == vec![1, 3]` (Vec serde preserves order). |
| UT-0475-03 | `validate_free_list_returns_ok_on_valid_state` | net with `free_list = [1, 3]`, `agents[1].is_none()`, `agents[3].is_none()` | `net.validate_free_list()` | `Ok(())`. |
| UT-0475-04 | `validate_free_list_rejects_some_slot` | net with `free_list = [1]` BUT `agents[1] == Some(CON)` (synthetic invalid state) | `net.validate_free_list()` | `Err(NetError::FreeListInvalid { id: 1, reason: "slot is Some" })`. |
| UT-0475-05 | `serde_round_trip_then_validate_passes` | UT-0475-02's net | round-trip then `net2.validate_free_list()` | `Ok(())`. (R9 post-condition verified end-to-end.) |
| UT-0475-06 | `rkyv_round_trip_preserves_free_list` (under `--features zero-copy`) | same fixture | rkyv `to_bytes` / `from_bytes` round-trip | deserialized `free_list == vec![1, 3]`. |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | Empty free-list round-trip | Round-trip succeeds; deserialized net has `free_list.is_empty()`. (Backward compat with v2-baseline serialized nets — pre-SPEC-22 nets that have empty free-list semantically.) |
| EC-2 | Free-list with 1M entries | Round-trip succeeds; bincode handles `Vec` of any length up to memory budget. |
| EC-3 | Net deserialized into a structurally-incompatible binary (different `PROTOCOL_VERSION`) | Wire-version rejection per R9a / TASK-0476; coverage in T8a. (Out of T8 / TEST-SPEC-0475 scope.) |
| EC-4 | Validate after a synthetic violation: free-list contains an ID > arena_len | `validate_free_list` returns `Err`. |

## Invariants asserted

- R9 (serde participation + post-deserialize validity check).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

bincode and rkyv are deterministic. Pure synchronous; no tokio.

## Cross-test dependencies

- T8 (spec-catalog) is the integration mirror, joint with TASK-0491's `is_behaviorally_equal`.
- TEST-SPEC-0476 covers the wire-version rejection (T8a).
- TEST-SPEC-0471 covers the field's empty-case round-trip; TEST-SPEC-0475 covers the non-empty case.
