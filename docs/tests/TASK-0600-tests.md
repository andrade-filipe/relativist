# TEST-SPEC-0600 — Tests for TASK-0600 — Collapse parallel `Pull*` / `PullCoordinatorState` types

**Task:** TASK-0600 (Phase B-4b, P2)
**Spec:** SPEC-13 §x (FSM state catalog); SPEC-21 §3.8 A5 (pull-only FSM extensions — 5 coordinator + 2 worker states).
**Origin:** QA-D010-013 — parallel state representations `Pull*` (per-FSM enum) and `PullCoordinatorState` (aggregate) overlap and risk drift.
**Test floor delta:** **+2 default** (1 type-collapse witness + 1 exhaustiveness sanity test).
**Prerequisites:** None (parallel-OK with B-4a/B-4c).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0600-01 | unit | `relativist-core/src/protocol/coordinator.rs::tests::canonical_pull_state_type_is_unique` | none | none |
| UT-0600-02 | unit | `relativist-core/src/protocol/coordinator.rs::tests::canonical_pull_state_enum_is_exhaustive_per_spec_21_a5` | none | none |
| IT-0600-03 | integration | `relativist-core/tests/spec13_pull_state_collapse.rs::no_control_flow_reads_both_types` | none | none |

Total: **3 default tests** (2 UT + 1 IT). Conservative floor delta: **+2 default** (the IT is structural and may be folded into a UT depending on Stage 3 developer choice).

---

## Per-test specifications

### UT-0600-01 — `canonical_pull_state_type_is_unique`

**Purpose.** Verify exactly **one** pull-state type exists in the protocol module after the collapse. The redundant type is removed (or kept as a `pub use` alias for downstream compat — but the underlying struct/enum definition must be unique).
**Setup.** None — type-system check.
**Action.** Construct a value of the canonical type (e.g. `let s = PullState::Idle;`); construct via the alias (if kept); compare via `TypeId::of`.
**Assertions.**
- `std::any::TypeId::of::<PullState>() == std::any::TypeId::of::<PullCoordinatorState>()` (if the latter is kept as `pub use PullState as PullCoordinatorState;`) OR `PullCoordinatorState` does not exist as a path (compile-time check via a UI-test-style negative assertion or comment-driven note).
- The canonical type's `Debug` impl emits the variant names from SPEC-21 §3.8 A5 (e.g. `Idle`, `Dispatching`, `Awaiting`, ... — the exact 5 coordinator names).
- A doc-comment cites QA-D010-013 + the chosen direction (which type was kept).
**Boundary case coverage.** Catches a future re-introduction of a parallel type (e.g. someone adds `PullCoordinatorState` again as a real `enum` rather than an alias) — `TypeId` equality breaks.
**Why it must exist.** Acceptance criterion #1 (exactly one type encodes the per-FSM pull state).

---

### UT-0600-02 — `canonical_pull_state_enum_is_exhaustive_per_spec_21_a5`

**Purpose.** SPEC-21 §3.8 A5 enumerates 5 coordinator pull states + 2 worker pull states. The collapsed canonical type MUST contain all 7 (or contain the 5 coordinator states, if the worker has its own type — Stage 3 developer's call but documented).
**Setup.** None — exhaustive-match check.
**Action.** Write a `match` on the canonical type that handles every variant explicitly (no `_ =>` wildcard). The compiler enforces exhaustiveness.
**Assertions.**
- The match compiles (compile-time guarantee).
- Each spec-mandated variant is hit at least once at runtime (loop over `[Variant::A, Variant::B, ...]` and count matches; assert count == spec count).
- Variant count == 5 for `PullCoordinatorState` (or unified type's coordinator subset) per SPEC-21 §3.8 A5.
- Variant count == 2 for the worker subset per SPEC-21 §3.8 A5.
**Boundary case coverage.** Catches a buggy collapse that drops a state variant during the refactor (e.g. removing `PullCoordinatorState::Awaiting` because it "looked redundant").
**Why it must exist.** Acceptance criterion #3 (SPEC-13 / SPEC-21 §3.8 A5 state-set is preserved — no states added or removed).

---

### IT-0600-03 — `no_control_flow_reads_both_types`

**Purpose.** Structural test: scan the production source under `relativist-core/src/protocol/{coordinator,worker}.rs` and assert that no single function body references **both** types in the same control-flow path. This is enforced by source-text inspection at test-build time (a `build.rs`-style or static-text grep test).
**Setup.** None — operates on the source tree.
**Action.** Read each `.rs` file in `relativist-core/src/protocol/`; for every function that contains the substring `PullState` (or whatever the canonical name is) AND also contains the original alias name, fail.
**Assertions.**
- Zero functions reference both names within their body. (If the alias is kept as `pub use`, only the type *definitions* contain both names; *use sites* must reference only one.)
- The test fails with a clear error message naming the offending function(s).
**Boundary case coverage.** Catches a partial refactor that introduces a function which reads both representations — the exact drift QA-D010-013 flagged.
**Why it must exist.** Acceptance criterion #2 (no place in `protocol/{coordinator,worker}.rs` reads BOTH types in the same control-flow path).

**Implementation guidance.** This test can be a `#[test]` that does string parsing (acceptable since the source files are small and stable). Alternatively, fold the check into clippy via a custom lint — but a test is simpler and adequate.

---

## Coverage matrix

| test_id | AC-1 (one type) | AC-2 (no dual reads) | AC-3 (state set preserved) | AC-4 (tests pass) | AC-5 (lint clean) |
|---|---|---|---|---|---|
| UT-0600-01 | ✅ | | | ✅ | |
| UT-0600-02 | | | ✅ | ✅ | |
| IT-0600-03 | | ✅ | | ✅ | |

Every acceptance criterion 1-3 has ≥1 test. AC-4 is a "no regression" guarantee — verified by full `cargo test` passing. AC-5 (lint) is a `cargo clippy -- -D warnings` gate.

---

## Out-of-scope tests (deferred to other tasks)

- Tests on the protocol wire format (it does not change) → already covered by SPEC-19 / SPEC-06 existing tests.
- Tests on the FSM transitions (semantic correctness of pull-mode dispatch) → SPEC-21 existing test surface; this task is type-level only.

---

## Known spec ambiguity (adversarial flag)

- The task description leaves the **direction** of the collapse to Stage 3 (developer): is the canonical type `PullState` (per-FSM enum) or `PullCoordinatorState` (aggregate)? UT-0600-01 is direction-agnostic — Stage 3 picks the name and the test must be regenerated to match. **Flag for SDD-pipeline:** the test-generator output assumes `PullState` is the chosen canonical name; if the developer chooses otherwise, regenerate the test names accordingly.
- Whether the worker has its own type or shares the coordinator's: SPEC-21 §3.8 A5 lists 5 coordinator + 2 worker states, which suggests two types or one type with a discriminant. UT-0600-02 is written to handle both shapes by counting variants in two separate matches if needed. The Stage 3 developer should document the chosen shape in the test's doc-comment.
- The IT-0600-03 source-text grep approach is fragile to identifier renaming — a more robust alternative is to grep for `TypeId::of::<>` or use the `proc-macro2` parser. Stage 3 developer may pick either; the test contract is "no function reads both types," not "use this specific tooling."
