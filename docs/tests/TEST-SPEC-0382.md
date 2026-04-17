# TEST-SPEC-0382: `compute_outgoing_deltas` — diff helper for R25 with DC-C6 sentinel encoding

**Task:** TASK-0382
**Spec:** SPEC-19 §3.3 R25 (worker maintains `previous_border_state`;
  emit only changed entries each round)
**Spec-critic amendments incorporated:**
- DC-C6 (locked in, option C) — disconnection emitted as
  `BorderDelta { border_id, new_target: crate::net::DISCONNECTED }`
  where `DISCONNECTED = PortRef::FreePort(u32::MAX)`. NEVER emit
  `PortRef::FreePort(K)` (border-id-indexed) — that is a spec drift
  bug. NEVER use the literal `u32::MAX` — use the named constant.
**Provenance:** `docs/spec-reviews/SPEC-19-section-3.3-2.26C-design-choices-2026-04-17.md` §DC-C6
**Generated:** 2026-04-17

---

## Scope note

TASK-0382 ships the pure diff helper in `merge/helpers.rs`:

```rust
pub(crate) fn compute_outgoing_deltas(
    previous: &HashMap<u32, PortRef>,
    current: &HashMap<u32, PortRef>,
) -> Vec<BorderDelta>
```

Three semantic cases (per R25 + DC-C6):
1. Key in BOTH, values differ → `BorderDelta { border_id: K, new_target: current[K] }`.
2. Key only in `current` → `BorderDelta { border_id: K, new_target: current[K] }`.
3. Key only in `previous` → `BorderDelta { border_id: K, new_target: DISCONNECTED }`.

Returns `Vec<BorderDelta>` with no ordering guarantee. Tests that need
deterministic comparison sort by `border_id` first.

**DC-C6 firewall:** the disconnection sentinel MUST be the named
constant `crate::net::DISCONNECTED`. Two regression-guard tests
(UT-0382-08 and UT-0382-09) lock this convention.

---

## Test target file paths

- `relativist-core/src/merge/helpers.rs` — inline `#[cfg(test)] mod tests`.
  Nine new `#[test]` fns.

All tests are synchronous. No `tokio`, no `async`.

---

## Unit Tests

### UT-0382-01: `compute_outgoing_deltas_empty_both_returns_empty`

**Purpose:** Both maps empty → `Vec::new()`. Edge-case identity.

**Target:** `merge/helpers.rs::tests`

**Given:** `previous = HashMap::new()`, `current = HashMap::new()`.

**When:** `let out = compute_outgoing_deltas(&previous, &current);`

**Then:** `out.is_empty() == true`.

**Assertions:** Total over the empty input.

**SPEC-19 R covered:** R25 (boundary).

---

### UT-0382-02: `compute_outgoing_deltas_unchanged_entry_emits_nothing`

**Purpose:** Identical entry on both sides emits zero deltas (the
quiet-round invariant — coordinator-free round case from R3).

**Target:** `merge/helpers.rs::tests`

**Given:** Both maps = `{5 → AgentPort(AgentId(7), 0)}`.

**When:** Call helper.

**Then:** `out.is_empty() == true`.

**Assertions:** Equality short-circuits emission. Critical for
DC-C5's "all quiet" convergence path.

**SPEC-19 R covered:** R25.

---

### UT-0382-03: `compute_outgoing_deltas_changed_entry_emits_delta_with_new_target`

**Purpose:** R25 case 1 — key present in both, values differ →
emit one delta with `new_target = current[K]`.

**Target:** `merge/helpers.rs::tests`

**Given:**
- `previous = {5 → AgentPort(AgentId(1), 0)}`
- `current  = {5 → AgentPort(AgentId(2), 1)}`

**When:** Call helper.

**Then:**
- `out.len() == 1`
- `out[0].border_id == 5`
- `out[0].new_target == PortRef::AgentPort(AgentId(2), 1)`.

**Assertions:** New value is emitted, not the old one (R25 wording:
"only changed entries are emitted").

**SPEC-19 R covered:** R25 case 1.

---

### UT-0382-04: `compute_outgoing_deltas_new_entry_emits_delta_with_current_value`

**Purpose:** R25 case 2 — key only in `current` (newly added border
from coordinator-side CON-DUP) → emit one delta.

**Target:** `merge/helpers.rs::tests`

**Given:**
- `previous = HashMap::new()`
- `current  = {9 → PortRef::AgentPort(AgentId(3), 0)}`

**When:** Call helper.

**Then:**
- `out.len() == 1`
- `out[0].border_id == 9`
- `out[0].new_target == PortRef::AgentPort(AgentId(3), 0)`.

**Assertions:** First-report semantics for newly-created borders.

**SPEC-19 R covered:** R25 case 2.

---

### UT-0382-05: `compute_outgoing_deltas_removed_entry_emits_disconnect_sentinel`  (DC-C6)

**Purpose:** R25 case 3 — key only in `previous` → emit a
disconnection delta with `new_target == crate::net::DISCONNECTED`.

**Target:** `merge/helpers.rs::tests`

**Given:**
- `previous = {3 → PortRef::AgentPort(AgentId(7), 0)}`
- `current  = HashMap::new()`

**When:** Call helper.

**Then:**
- `out.len() == 1`
- `out[0].border_id == 3`
- `out[0].new_target == crate::net::DISCONNECTED`.

**Assertions:** The sentinel IS the named constant; assertion uses
`crate::net::DISCONNECTED` and not `PortRef::FreePort(u32::MAX)` or
`PortRef::FreePort(3)`. If the helper accidentally emits `FreePort(K)`
(border-id-indexed) instead, this test fires.

**SPEC-19 R covered:** R25 case 3 + DC-C6 (locked in, option C).

---

### UT-0382-06: `compute_outgoing_deltas_mixed_emits_three_deltas`

**Purpose:** Combined — 2 unchanged, 1 changed, 1 new, 1 removed →
exactly 3 deltas. Asserts membership by sorting on `border_id`.

**Target:** `merge/helpers.rs::tests`

**Given:**
- `previous = {1 → A, 2 → B, 3 → C, 4 → D}`           (4 entries)
- `current  = {1 → A, 2 → B, 3 → C', 5 → E}`          (4 entries: 3 unchanged, C→C', new key 5, removed key 4)

  where `A = AgentPort(AgentId(1), 0)`, `B = AgentPort(AgentId(2), 0)`,
  `C = AgentPort(AgentId(3), 0)`, `C' = AgentPort(AgentId(3), 1)`,
  `D = AgentPort(AgentId(4), 0)`, `E = AgentPort(AgentId(5), 0)`.

**When:** Call helper, sort `out` by `border_id`.

**Then:**
- `out.len() == 3`
- After sort: `out[0] = BorderDelta { border_id: 3, new_target: C' }`
- `out[1] = BorderDelta { border_id: 4, new_target: crate::net::DISCONNECTED }`
- `out[2] = BorderDelta { border_id: 5, new_target: E }`.

**Assertions:** Exactly 3 deltas (1 changed, 1 removed-as-DISCONNECTED,
1 new). Mixed input does not double-emit unchanged entries.

**SPEC-19 R covered:** R25 (composite).

---

### UT-0382-07: `compute_outgoing_deltas_large_unchanged_emits_zero`

**Purpose:** Hot-path perf sanity. 1000 identical entries → zero
deltas (proves `O(|previous| + |current|)` per the doc-comment, but
more importantly proves the function does NOT emit one delta per key
unconditionally).

**Target:** `merge/helpers.rs::tests`

**Given:** Both maps populated with 1000 identical
`(K, AgentPort(AgentId(K), 0))` entries for `K in 0..1000`.

**When:** Call helper.

**Then:** `out.is_empty() == true`.

**Assertions:** No delta emitted on the all-stable case. (Performance
sanity; absence of any silent regression that emits one delta per
key.)

**SPEC-19 R covered:** R25 (scale).

---

### UT-0382-08: `compute_outgoing_deltas_disconnect_uses_named_constant`  (DC-C6 firewall)

**Purpose:** Lock the convention that `crate::net::DISCONNECTED` is
used by name (not the underlying `PortRef::FreePort(u32::MAX)` literal,
which would still type-check but would silently break if the sentinel
is ever rebased to a different variant). Pure regression guard.

**Target:** `merge/helpers.rs::tests`

**Given:** `previous = {99 → AgentPort(AgentId(1), 0)}`, `current = HashMap::new()`.

**When:** Call helper.

**Then:**
- `out[0].new_target == crate::net::DISCONNECTED` (uses the named const)
- AND ALSO `out[0].new_target == PortRef::FreePort(u32::MAX)` (current
  identity definition; if DISCONNECTED is ever rebased, the named
  constant assertion above passes but THIS asserts the structural
  equality holds at the time of writing).
- The `border_id` field is `99`, NOT collapsed/aliased to `u32::MAX`
  (regression guard against a hypothetical bug that confuses the
  border id with the sentinel marker).

**Assertions:** Two equivalent equality assertions ensure both the
name binding and the structural value are pinned.

**SPEC-19 R covered:** DC-C6 (named-constant firewall).

---

### UT-0382-09: `compute_outgoing_deltas_disconnect_does_not_use_freeport_border_id`  (DC-C6 firewall)

**Purpose:** Negative guard against a regression to the original draft
(pre-spec-critic) where the helper emitted `PortRef::FreePort(K)`
(border-id-indexed) for disconnection. That encoding type-checks but
is a spec deviation. This test fires immediately if the draft
semantics return.

**Target:** `merge/helpers.rs::tests`

**Given:** `previous = {7 → AgentPort(AgentId(2), 0)}`, `current = HashMap::new()`.

**When:** Call helper.

**Then:**
- `out[0].new_target != PortRef::FreePort(7)` (would be the buggy
  border-id-indexed encoding)
- `out[0].new_target == crate::net::DISCONNECTED`.

**Assertions:** Negative-form guard. Encoding MUST NOT collapse to
`FreePort(border_id)`.

**SPEC-19 R covered:** DC-C6 (negative guard).

---

## Coverage mapping

| Requirement / DC | Covered by |
|---|---|
| R25 — empty input identity | UT-0382-01 |
| R25 — unchanged entry emits nothing | UT-0382-02, UT-0382-07 |
| R25 case 1 (changed) | UT-0382-03 |
| R25 case 2 (newly added in current) | UT-0382-04 |
| R25 case 3 (removed from current) | UT-0382-05 |
| R25 composite (mixed) | UT-0382-06 |
| DC-C6 (option C) — DISCONNECTED encoding | UT-0382-05, UT-0382-08 |
| DC-C6 negative guard — NOT `FreePort(border_id)` | UT-0382-09 |
| Hot-path perf sanity (no per-key emission) | UT-0382-07 |

---

## Adversarial angles (QA candidates for Stage 5)

| # | Scenario | Why dangerous |
|---|---|---|
| QA-0382-A | Helper returns one delta per key in `current` unconditionally | UT-0382-02, UT-0382-07 fire |
| QA-0382-B | Helper emits `FreePort(border_id)` for disconnect (pre-DC-C6 draft) | UT-0382-05, UT-0382-09 fire |
| QA-0382-C | DISCONNECTED constant rebased to a different variant (e.g. `PortRef::Disconnected`) | UT-0382-08 still passes the named-const assertion; UT-0382-08's structural assertion fires; flag at Stage 5 to update if the rebase is intentional |
| QA-0382-D | Helper signature changes to `Option<Vec<u32>> out-param` (DC-C6 option B fallback) | UT-0382-09 won't compile against the new signature; canary |
| QA-0382-E | Hash iteration nondeterminism causes flaky ordering assertions | UT-0382-06 sorts before asserting; QA: scan for any test that asserts `out[i] == ...` without first sorting |
| QA-0382-F | A future refactor sorts `out` by `border_id` automatically | All tests pass; behavior change. Spec-critic to rule on whether ordering is part of the public contract |
| QA-0382-G | `border_id == u32::MAX` collides with DISCONNECTED encoding under a different sentinel scheme | Edge case; not exercised here. Open question for spec-critic |

---

## Acceptance gate

- `cargo test --workspace --lib` floor: +9 new `#[test]` fns. Gate
  tolerates +9 to +11.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo fmt --check` clean.
- No regression on v1 690-test baseline.

---

## Out of scope (deferred)

- Helper integration with `handle_round_start` (calls this from R24
  step 4) → TEST-SPEC-0381.
- Coordinator-side `BorderGraph::apply_deltas` interpretation of the
  DISCONNECTED sentinel → 2.26-B / TEST-SPEC for the resolver.
- Deterministic-ordering policy on `Vec<BorderDelta>` output → spec-critic
  open question.
