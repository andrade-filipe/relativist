# TEST-SPEC-0471: `Net.free_list` field + constructor initialization

**SPEC-22 §7 ID:** none (plumbing for the field's existence; T1..T18 exercise its behavior).
**Owning task:** TASK-0471.
**Parent spec:** SPEC-22 §3.1 R1, R8; §4.1 (struct definition).
**Type:** unit.
**Theory anchor:** None direct.

---

## Inputs / Fixtures

- Fresh `Net` instances via `Net::new()` and `Net::with_capacity(N)`.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0471-01 | `net_new_initializes_empty_free_list` | `let net = Net::new();` | check `net.free_list` | `net.free_list.is_empty() == true`; `net.free_list.capacity() == 0` (or implementation-defined initial capacity per `Vec::new()`). |
| UT-0471-02 | `net_with_capacity_initializes_empty_free_list` | `let net = Net::with_capacity(100);` | same | `net.free_list.is_empty() == true`. (R8: capacity hint does NOT pre-allocate the free-list — it grows on demand via `remove_agent`.) |
| UT-0471-03 | `net_serde_round_trip_preserves_empty_free_list_field` | `Net::new()` → bincode encode → bincode decode | check `net2.free_list` | empty after round-trip. (Smoke test for the derive propagation; non-empty case in TASK-0475.) |
| UT-0471-04 | `net_clone_preserves_free_list` | `let mut net = Net::new(); net.free_list.push(7);` then clone | check `net2.free_list` | `vec![7]`. (Confirms `Clone` derive flows through.) |
| UT-0471-05 | `net_partial_eq_distinguishes_free_list` | two nets, one with `free_list = [7]` and one with `free_list = []` | `==` | returns `false`. (Confirms `PartialEq` derive includes the field.) |
| UT-0471-06 | `net_field_is_pub_visible` | compile-time accessible | `let _: &Vec<AgentId> = &net.free_list;` | compiles. (Documentation: the field is `pub`; downstream code can read/write directly.) |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | `Net::with_capacity(0)` | empty `free_list`; no pre-alloc. |
| EC-2 | `freeport_redirects` field UNCHANGED by SPEC-22 (no derive change) | Compile-check that `Net.freeport_redirects` is still `HashMap<u32, PortRef>`, still `#[serde(skip)]`. (Regression guard.) |
| EC-3 | `#[cfg(feature = "zero-copy")]` build | rkyv derive includes `free_list` (no rkyv skip on this field). Smoke compile-test. |

## Invariants asserted

- R1 (free-list field exists).
- R8 (constructors initialize empty).
- R28 (always-on default — no feature gate; no `#[cfg(feature = ...)]` around the field).

## ARG/DISC/REF citation

- None direct.

## Determinism notes

Pure synchronous; no tokio.

## Cross-test dependencies

- The field is the substrate for T1..T10 / T8 / T8a / T9 / T9a / T9b. This test is foundational; if it fails, every subsequent free-list test fails.
- TASK-0488 adds the `assert_impl_all!(Net: Send, Sync)` compile-time check; UT-0471-* indirectly verify Send + Sync since `Vec<AgentId>` is `Send + Sync`.
