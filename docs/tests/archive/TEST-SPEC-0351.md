# TEST-SPEC-0351: Coordinator skip-merge logic + Global Normal Form termination

**Task:** TASK-0351
**Spec:** SPEC-19 §3.1 R3 (track + MAY skip), R4 (Global Normal Form
termination), R5 (no result drift — confluence safety), R6 (SHOULD under
strict BSP), R7 (compatible with both v1 protocol and v2 wire format).
**Generated:** 2026-04-16
**Baseline before this task:** 858+ (post-TASK-0348/0349/0350)
**Cumulative target after this task:** 866+ (≥ +8 new tests)
**Cumulative target for the whole §3.1 bundle:** 850 → 866+ (≥ +16 across
the 4 TEST-SPECs).

---

## Scope note

This is the **payoff** test spec — every behavior delivered by the SPEC-19
§3.1 bundle is observable through these tests:

- **R3:** when ALL workers report `has_border_activity == false`, AND
  `coordinator_free_rounds == true` AND `strict_bsp == true`, the
  coordinator **skips** the merge-redistribute cycle for that round and
  increments `metrics.coordinator_free_rounds`.
- **R4:** when additionally every worker reports `local_redexes == 0`,
  the coordinator declares **Global Normal Form**, breaks the round loop,
  and returns the merged net as the final result.
- **R5:** **no result drift** — toggling `coordinator_free_rounds` MUST
  NOT change the decoded result for any workload. Strong confluence (T4)
  guarantees the same Normal Form regardless of merge skipping.
- **R6:** **strict-BSP gating** — in lenient mode the skip is disabled
  by design (documented choice; lenient mode collapses to one round).
- **R7:** **wire FSM untouched** — `protocol/coordinator.rs` MUST NOT
  gain new variants or branches in this bundle. Verified by code-reading
  test that diffs the file's variant set against a frozen list.

---

## Test target file paths

- `relativist-core/src/merge/grid.rs` — inline `#[cfg(test)] mod tests`
  block for `check_global_normal_form` (helper, exhaustive 2×2 truth
  table) and the `run_grid` integration cases.
- `relativist-core/src/protocol/coordinator.rs` — inline `#[cfg(test)]
  mod tests` block — one structural test (UT-08) verifying no FSM mutation.

All tests are synchronous `#[test]` units; `run_grid` lives in the pure
`merge/` layer and does NOT need a tokio runtime. UT-08 reads its own
file at compile time via `include_str!` for the structural assertion.

---

## Unit Tests

### UT-0351-01..04: `check_global_normal_form_*` — exhaustive 2×2 truth table (R3+R4)

**Purpose:** The pure `check_global_normal_form(stats: &[WorkerRoundStats])
-> (bool, bool)` helper MUST correctly return
`(all_no_border_activity, all_no_local_redexes)`. Exhaustive over the 4
combinations of the 2 booleans across a 2-worker fixture.
**Target file:** `merge/grid.rs::tests`

| # | activity[0] | redexes[0] | activity[1] | redexes[1] | expected (no_border, no_redex) |
|---|-------------|------------|-------------|------------|--------------------------------|
| 01 | false | 0 | false | 0 | (true, true)  — Global Normal Form |
| 02 | false | 5 | false | 3 | (true, false) — skip-merge-eligible |
| 03 | true  | 0 | false | 0 | (false, true) |
| 04 | true  | 5 | true  | 3 | (false, false) — full merge needed |

```rust
#[test]
fn check_global_normal_form_01_all_quiescent_returns_true_true() {
    let stats = vec![
        stats_with(false, 0),
        stats_with(false, 0),
    ];
    assert_eq!(check_global_normal_form(&stats), (true, true));
}
// ... three more parallel cases ...
```

**SPEC-19 R covered:** R3 (skip predicate carrier), R4 (GNF predicate carrier).

---

### UT-0351-05: `run_grid_skips_merge_when_all_no_border_activity_strict_bsp` (R3 end-to-end)

**Purpose:** Under strict BSP with the flag opted in, `run_grid` MUST
skip the merge-redistribute cycle when every worker reports
`has_border_activity == false` AND **at least one** worker still has
local redexes (otherwise R4 termination wins). The metric counter MUST
increment by exactly 1 per skipped round.
**Target file:** `merge/grid.rs::tests` (or `tests/run_grid_coordinator_free.rs`)
**Preconditions:** Construct a small 2-worker net where round 0 produces
`has_border_activity = false` for both workers but worker 0 still has
≥1 local redex queued for round 1.

**Input:**
```rust
let net = build_net_no_borders_with_pending_local_work();
let cfg = GridConfig {
    num_workers: 2,
    max_rounds: Some(3),
    strict_bsp: true,
    coordinator_free_rounds: true,
    ..GridConfig::default()
};
let (_result, metrics) = run_grid(net, cfg);
```

**Expected output:**
```rust
assert!(metrics.coordinator_free_rounds >= 1,
        "at least one round must have skipped merge; got {}",
        metrics.coordinator_free_rounds);
```

**Edge case:** Optionally instrument by counting `merge_calls` in
`GridMetrics` (if such a counter exists) or by a custom test-only spy.
If neither exists, the metric counter increment IS the observable proof.

**SPEC-19 R covered:** R3, R6.

---

### UT-0351-06: `run_grid_terminates_on_global_normal_form` (R4)

**Purpose:** When all workers report BOTH `has_border_activity == false`
AND `local_redexes == 0`, `run_grid` MUST exit the round loop early and
return the current merged net.
**Target file:** `merge/grid.rs::tests`
**Preconditions:** Construct a small net that reaches Normal Form in
round 0 (e.g. a single ERA-ERA annihilation across one partition only,
no borders).

**Input:**
```rust
let net = build_already_quiescent_net();
let cfg = GridConfig {
    num_workers: 2,
    max_rounds: Some(100),   // generous budget
    strict_bsp: true,
    coordinator_free_rounds: true,
    ..GridConfig::default()
};
let (result, metrics) = run_grid(net, cfg);
```

**Expected output:**
```rust
assert!(metrics.rounds <= 1,
        "GNF must terminate ASAP; took {} rounds", metrics.rounds);
assert!(metrics.converged,
        "metrics.converged must be set on GNF exit");
// The result must equal the locally-reduced reference net.
let local_ref = reduce_all_local(initial_net_clone());
assert_eq!(canonical(result), canonical(local_ref));
```

**SPEC-19 R covered:** R4.

---

### UT-0351-07: `run_grid_default_config_unchanged_v1_behavior` (R7 v1 compat)

**Purpose:** With `GridConfig::default()` (i.e. `coordinator_free_rounds
== false`), `run_grid` MUST behave bit-identically to the pre-bundle v1
implementation. Counter MUST stay `0`. No GNF early exit on activity-true
inputs.
**Target file:** `merge/grid.rs::tests`

**Input:**
```rust
let net = build_typical_two_worker_net_with_borders();
let cfg = GridConfig {
    num_workers: 2,
    max_rounds: Some(5),
    strict_bsp: true,
    ..GridConfig::default()         // coordinator_free_rounds = false
};
let (result, metrics) = run_grid(net.clone(), cfg);
```

**Expected output:**
```rust
assert_eq!(metrics.coordinator_free_rounds, 0,
           "default config must never increment the counter");

// Compare against a pre-bundle frozen reference (golden).
// If a golden file is impractical, compare to a fresh local reduction:
let local_ref = reduce_all_local(net);
assert_eq!(canonical(result), canonical(local_ref),
           "default config result must match local reduction");
```

**SPEC-19 R covered:** R7 (v1 protocol path unchanged).

---

### UT-0351-08: `run_grid_lenient_does_not_skip` (R6 SHOULD interpretation)

**Purpose:** `coordinator_free_rounds = true` AND `strict_bsp = false`
MUST NOT skip merge — bundle's documented design choice for R6 SHOULD.
The counter MUST stay `0` even when the activity predicate would otherwise
trigger.
**Target file:** `merge/grid.rs::tests`

**Input:**
```rust
let net = build_net_no_borders_with_pending_local_work();
let cfg = GridConfig {
    num_workers: 2,
    max_rounds: Some(5),
    strict_bsp: false,                  // <- lenient
    coordinator_free_rounds: true,      // <- opted in
    ..GridConfig::default()
};
let (_result, metrics) = run_grid(net, cfg);
```

**Expected output:**
```rust
assert_eq!(metrics.coordinator_free_rounds, 0,
           "lenient mode must not skip even with the flag on (R6 SHOULD)");
```

**SPEC-19 R covered:** R6 (gated SHOULD enforcement).

---

### UT-0351-09: `coordinator_wire_fsm_untouched_by_3_1_bundle` (R7 wire compat)

**Purpose:** R7 forbids new `Message` variants OR new FSM branches in
`protocol/coordinator.rs` for this bundle. Verified by structural
assertion.
**Target file:** `protocol/coordinator.rs::tests`
**Approach:** the test reads the file's source via `include_str!` and
asserts that the set of `Message::*` arms inside the coordinator's main
match has not grown to include any §3.1-specific variant
(e.g. `RoundStart`, `RoundResult`, `FinalStateRequest`, `FinalStateResult`,
`InitialPartition`). Those belong to §3.4 (item 2.26) and MUST NOT
appear here.

```rust
#[test]
fn coordinator_wire_fsm_untouched_by_3_1_bundle() {
    let src = include_str!("coordinator.rs");
    for forbidden in [
        "Message::RoundStart",
        "Message::RoundResult",
        "Message::FinalStateRequest",
        "Message::FinalStateResult",
        "Message::InitialPartition",
    ] {
        assert!(
            !src.contains(forbidden),
            "SPEC-19 §3.1 bundle MUST NOT introduce {forbidden}; that belongs to §3.4 (item 2.26). R7 violation."
        );
    }
}
```

**Limitation:** Source-grep tests are brittle to refactors but cheap and
load-bearing here — they enforce a **scope discipline**, not a runtime
property. If the FSM legitimately needs the variant in a future bundle,
this test is updated together with the spec change.

**SPEC-19 R covered:** R7 (wire FSM compatibility).

---

### UT-0351-10: `run_grid_equivalence_no_border_activity_workload` (R5 confluence — skip path triggered)

**Purpose:** R5 — confluence safety. Run a workload that DOES trigger the
skip path with `coordinator_free_rounds = true`, and the SAME workload with
`coordinator_free_rounds = false`. Decoded results MUST be byte-equal.
**Target file:** `merge/grid.rs::tests`

**Input:**
```rust
let net = build_net_no_borders_with_pending_local_work();   // skip-eligible
let mut cfg_off = GridConfig {
    num_workers: 2,
    max_rounds: Some(10),
    strict_bsp: true,
    ..GridConfig::default()
};
let mut cfg_on = cfg_off.clone();
cfg_on.coordinator_free_rounds = true;

let (result_off, metrics_off) = run_grid(net.clone(), cfg_off);
let (result_on,  metrics_on)  = run_grid(net,         cfg_on);
```

**Expected output:**
```rust
assert_eq!(canonical(result_off), canonical(result_on),
           "R5 confluence: results must be identical with skip on/off");
assert_eq!(metrics_off.coordinator_free_rounds, 0);
assert!(metrics_on.coordinator_free_rounds  >= 1,
        "this workload was chosen to trigger the skip");
```

**SPEC-19 R covered:** R5 (confluence on skip path).

---

### UT-0351-11: `run_grid_equivalence_with_border_activity_workload` (R5 confluence — skip path NOT triggered)

**Purpose:** R5 — same as UT-10 but on a workload where the skip path is
**NOT** triggered (every round has at least one worker with
`has_border_activity == true`). Decoded results MUST still be byte-equal.
This proves the new code path is the identity transformation when its
predicate is false.
**Target file:** `merge/grid.rs::tests`

**Input:**
```rust
let net = build_typical_two_worker_net_with_borders();   // never skip-eligible
let cfg_off = GridConfig {
    num_workers: 2,
    max_rounds: Some(10),
    strict_bsp: true,
    ..GridConfig::default()
};
let mut cfg_on = cfg_off.clone();
cfg_on.coordinator_free_rounds = true;

let (result_off, metrics_off) = run_grid(net.clone(), cfg_off);
let (result_on,  metrics_on)  = run_grid(net,         cfg_on);
```

**Expected output:**
```rust
assert_eq!(canonical(result_off), canonical(result_on),
           "R5 confluence: identity when predicate is always false");
assert_eq!(metrics_off.coordinator_free_rounds, 0);
assert_eq!(metrics_on.coordinator_free_rounds,  0,
           "border-active workload must NEVER trigger the skip");
```

**SPEC-19 R covered:** R5 (identity outside the skip predicate).

---

### UT-0351-12 (G1 spot check): `church_add_2_3_w2_strict_bsp_equivalence` (R5 + bundle acceptance)

**Purpose:** Real workload spot check (G1-style). `church_add(2, 3)` at
`w=2` strict BSP MUST produce the **same decoded `Nat`** with
`coordinator_free_rounds` toggled on vs off.
**Target file:** `merge/grid.rs::tests` (or
`tests/coordinator_free_round_equivalence.rs`)

**Input:**
```rust
let registry = default_registry();
let codec = registry.get("church_add").unwrap();
let net = codec.encode_and_validate(json!({"a": 2, "b": 3})).unwrap();

let cfg_off = GridConfig {
    num_workers: 2,
    max_rounds: Some(20),
    strict_bsp: true,
    ..GridConfig::default()
};
let mut cfg_on = cfg_off.clone();
cfg_on.coordinator_free_rounds = true;

let (net_off, _) = run_grid(net.clone(), cfg_off);
let (net_on,  _) = run_grid(net,         cfg_on);

let dec_off = codec.decode(&net_off).unwrap();
let dec_on  = codec.decode(&net_on).unwrap();
```

**Expected output:**
```rust
assert_eq!(dec_off, dec_on, "G1: church_add(2, 3) result must match");
assert_eq!(dec_off, json!({"result": 5}));
```

**SPEC-19 R covered:** R5 + G1 acceptance gate from the bundle index.

---

## Adversarial probes (QA candidates for Stage 5 — referenced, not implemented as tests here)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0351-A | `num_workers = 0` with `coordinator_free_rounds = true` | Empty stats slice — `iter().all()` returns `true` vacuously, would falsely trigger GNF on round 0 with no work done | QA |
| QA-0351-B | `num_workers = 1` (no borders by definition) | Activity always `false`; the skip path triggers every round it has local work — verify the counter accrues correctly and termination is on R4 (zero redexes), not infinite-skip | QA |
| QA-0351-C | Worker that oscillates `has_border_activity` round-to-round (true → false → true) | Verify the skip is **per-round**, not sticky — round N may skip, round N+1 may merge fully | QA |
| QA-0351-D | `coordinator_free_rounds = true` AND `strict_bsp = false` (also covered by UT-08) — adversarial twist: ensure no metric increment, no GNF early exit, AND no panic on the SHOULD-violating combination | UT-08 covers the metric and result; QA should run with a high `max_rounds` to confirm no regression in lenient termination | QA |
| QA-0351-E | `max_rounds = Some(0)` with `coordinator_free_rounds = true` | Exit immediately; counter MUST be `0`; result MUST equal the input net | QA |
| QA-0351-F | Large workload (`ep_annihilation_con(100)` at w=2) — verify spec T11 `metrics.coordinator_free_rounds >= 1` | Real benchmark workload from SPEC-19 T11; confirms the optimization actually fires on the canonical bench | QA / Bench |
| QA-0351-G | Race where `has_border_activity` differs between local-reduction stats and the carrier seen by the coordinator | Local-simulation has no race window (sequential `run_grid`); flagged for the wire FSM bundle (item 2.26) when async timing matters | QA / future |

---

## Hard-to-write-deterministically tests (FLAGGED)

- **UT-0351-09 (FSM-untouched grep)** is **brittle to source refactors**:
  if `protocol/coordinator.rs` legitimately renames a string match arm
  for an unrelated reason, the test fails on a non-issue. Mitigation:
  the assertion is keyed on `Message::*` variant names, which are
  stable identifiers under SPEC-18; renames trigger a compile error in
  consumers first. Test is acceptable but flagged for future migration
  to a parsed-AST check if `syn` becomes a dev-dep.

- **QA-0351-G (race)** is **non-deterministic** and only relevant to the
  wire FSM bundle (item 2.26). NOT implemented in this bundle. The
  local-simulation path is sequential — no race window exists in
  `run_grid`.

- **UT-0351-12 (G1 spot check)** depends on `default_registry()`,
  `church_add` codec, and a working `decode` path — all in scope from
  SPEC-27 Phase 5. No flakiness expected; deterministic by construction.

---

## Acceptance Gate

1. `cargo test --workspace` count: 858 → **866+** (≥ +8: UT-01..04 truth
   table + UT-05 skip + UT-06 GNF + UT-07 default + UT-08 lenient +
   UT-09 FSM untouched + UT-10/11 equivalences + UT-12 G1).
2. All previously passing tests still pass (no regression on the 850
   baseline; CLAUDE.md hard floor).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. Release smoke `compute add 3 5 → 8` works.
6. Bundle goal verified: with `coordinator_free_rounds = true` on a
   skip-eligible workload, the metric counter is non-zero AND the result
   is byte-equal to the flag-off run.

## Bundle-wide cumulative test count

| TEST-SPEC | New tests | Cumulative lib tests |
|-----------|-----------|----------------------|
| Baseline (post-SPEC-18) | — | 850 |
| TEST-SPEC-0348 | +6  | 856 |
| TEST-SPEC-0349 | +4  | 860 |
| TEST-SPEC-0350 | +2  | 862 |
| TEST-SPEC-0351 | +8  | 870 |
| **Bundle total** | **+20** | **870** (target: 866+) |

`+20` lands inside the orchestrator's `+12 to +18` hint; the conservative
post-bundle floor is **866+** (which absorbs any reduction during
implementation negotiation), with the headline target of **870**.

## Out of Scope

- §3.2 BorderGraph (item 2.35).
- §3.3 full Delta-Only Protocol (item 2.26).
- §3.4 new `Message` variants — explicitly forbidden by R7 in this bundle.
- §3.5 invariant amendments — formal proof work item OQ-1 / ARG-005.
- §3.6 `delta_mode` config field.
- §3.7 R45 per-round delta byte / time vectors.
- Real-wire coordinator FSM in `protocol/coordinator.rs` — the bundle
  changes only `merge/grid.rs` (local-simulation path); the wire FSM
  inherits the type plumbing from TASK-0348/0349 for free when item
  2.26 ships.
