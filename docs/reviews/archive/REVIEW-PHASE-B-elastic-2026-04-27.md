# Review — Phase B (Foundations) of SPEC-20 Elastic Grid (TASK-0415..0426)

**Date:** 2026-04-27
**Stage:** 4 (REVIEW) — unified code-quality + architecture review
**Reviewer:** reviewer agent
**Bundle:** D-006 Phase B Waves 1–4 (commits `4fb77bc`, `ba846ad`, `5c6d8b6`, `21cbfb4`)
**Files reviewed (post-commit `21cbfb4`):**
- `relativist-core/src/merge/types.rs` (GridConfig + ExecutionMode amendments — TASK-0415)
- `relativist-core/src/config.rs` (CLI elastic flags — TASK-0416)
- `relativist-core/src/protocol/coordinator.rs` (PROTOCOL_VERSION bump + JoinRequest handler + tokio::select loop + self-worker integration — TASK-0417/0419/0422/0423/0424)
- `relativist-core/src/protocol/types.rs` (Message enum + LeaveKind/JoinNackReason/WorkerCapabilities — TASK-0418)
- `relativist-core/src/protocol/error.rs` (thiserror migration + Coordinator/Fatal variants — TASK-0418 fallout)
- `relativist-core/src/protocol/timers.rs` (TimerKind — TASK-0426)
- `relativist-core/src/protocol/self_worker.rs` (NEW — TASK-0423/0424)
- `relativist-core/src/coordinator.rs` (FSM enums extended; LeaveKind re-exported — TASK-0414 closure + TASK-0420)
- `relativist-core/src/partition/types.rs` (LeaveKind moved here per TASK-0414 MF-002)
- `relativist-core/src/partition/helpers.rs` (`partition_index_of`, `compute_round_id_ranges` — TASK-0420/0421)
- `relativist-core/src/error.rs` (ConfigError, CoordinatorError::WorkerIdSpaceExhausted — TASK-0415/0418)
- `docs/backlog/TASK-04{15..26}*.md` (12 task contracts)
- `docs/tests/TEST-SPEC-04{15,16,18,19,26}*.md` (5 test-spec docs; 7 tasks lack a formal test-spec doc — see §5)

**Cross-refs:**
- `specs/SPEC-20-elastic-grid.md` §3.0 (M0/R0d), §3.1 (R1–R8 hybrid), §3.2 (R9–R17 join), §3.3 (R18–R31 departure), §3.4 (R33/R33a config), §3.5 (R35/R35-cross-spec/R37/R37a wire), §3.8 amendments A1–A8, §4.1.1–4.1.4 (FSM), NF-008 (TimerKind), NF-009 (PVM shape)
- `specs/SPEC-13-system-architecture.md` R28 (dependency direction core ← protocol)
- `specs/SPEC-01-invariantes.md` (T1–T7, D1–D6, G1)
- Phase A precedent: `docs/reviews/REVIEW-TASK-0414-2026-04-25.md`

---

## Verdicts

| Axis | Verdict |
|------|---------|
| **Code-quality** | NEEDS REFACTORING |
| **Architecture** | MINOR DRIFT |
| **Spec compliance** | NON-COMPLIANT on R35 wire shape (3 MUST-violations) and on R26a hybrid path (premature abort) |
| **Wave 2 sanity audit** | PASS — the −1521/+588 LoC delta is mostly verbose-doc stripping + thiserror migration + coordinator hot-spot refactor. No production logic was deleted without a destination. |
| **Discriminant stability (Message enum)** | PASS — variants 12–16 appended at tail; byte-level discriminant test covers all 17 variants |
| **TimerKind NF-008** | PASS — `#[repr(u32)]` with explicit 0–3, pinned by tests in two locations |
| **Overall** | **ACCEPT_WITH_FIXES** — 4 Must-Fix (3 wire-shape spec violations + 1 latent logic bug), 6 Should-Fix |

---

## 1. Summary

The bundle lands the foundational surface of SPEC-20: GridConfig gains 9 elastic fields with normalize/validate/active_mode (TASK-0415), CLI exposes them (TASK-0416), `PROTOCOL_VERSION` is bumped from 3 to 4 (TASK-0417) and pinned by `protocol_version_is_four` plus three `qa_probe_*` paths, the `Message` enum gains five elastic variants at discriminants 12–16 (TASK-0418) with a comprehensive byte-level discriminant-stability test (`test_message_discriminant_stability` covering 17 cases), `TimerKind` is extracted into a dedicated `protocol::timers` module with `#[repr(u32)]`/`#[non_exhaustive]` (TASK-0426 + NF-008 closure), `WorkerId 0` reservation and `partition_index_of` are added (TASK-0420), `compute_round_id_ranges` recomputes ID ranges for `K_eff` membership (TASK-0421), the coordinator handshake distinguishes `Register` from `JoinRequest` (TASK-0419), the main loop integrates `tokio::select!` between accept / reduce / mid-session connect (TASK-0422), and the in-process self-worker is spawned via `ChannelTransport` and joined per round (TASK-0423/0424). The `RetainedStateRegistry` plumbing is partially in place. CoordinatorEvent / CoordinatorAction / CoordinatorState carry `#[non_exhaustive]` (closing TASK-0414 SF-004). MF-001/MF-002/MF-003 from the TASK-0414 review are fully discharged: `StartTimer`/`CancelTimer` use `TimerKind::Collect as TimerId`, `LeaveKind` was moved out of `coordinator.rs`, and `EffectiveWorkerCount` is now a named alias.

**Top finding (MF-001) is a wire-format spec violation:** `Message::JoinRequest.protocol_version` and `JoinNackReason::ProtocolVersionMismatch` use `u8` rather than the `u32` mandated by SPEC-20 R35 / R35-cross-spec-version-shape (NF-009). This is a Must-Fix because (i) the spec text in §3.5 R35 explicitly types it `u32`; (ii) NF-009 mandates shape alignment with the `RegisterNack` rejection path and ratifies `{ coordinator: u32, worker: u32 }`; (iii) the field-naming choice in the impl (`expected/got`) further diverges from the `{ coordinator, worker }` payload SPEC-19 R37's next revision is required to adopt. **Second-most consequential (MF-002) is a duplicated `LeaveKind` enum:** there is one in `partition::types::LeaveKind` (used by `coordinator.rs` FSM) and a *second* in `protocol::types::LeaveKind` (used by `Message::LeaveRequest`). They are different Rust types with different derive sets; any future code path that needs to convert `Message::LeaveRequest { kind } → CoordinatorEvent::WorkerLeft(_, kind)` will have to write a manual mapping or — more likely — silently store the wrong type. This is exactly the failure mode the TASK-0414 MF-002 fix was meant to prevent. **Third (MF-003) is a latent logic bug at `protocol/coordinator.rs:832`:** the elastic-departure recovery path successfully reconstructs the merged net, logs `"Departure recovery reconstruction succeeded"`, then *immediately returns `ProtocolError::Fatal`* — meaning any departure event in Phase B aborts the run after the recovery succeeds, rendering the partial implementation worse than no-implementation (state mutation before abort risks half-applied side effects in `metrics` if a future refactor moves the metrics push earlier). **Fourth (MF-004): `Message` enum lacks `#[non_exhaustive]`.** All other newly-extended public enums in this bundle (`CoordinatorState`, `CoordinatorEvent`, `CoordinatorAction`, `RetainedSlot`, `DepartureKind`, `TimerKind`, both `LeaveKind`s) are `#[non_exhaustive]`; `Message` — the most consequential public enum, with 5 variants newly appended in this bundle and more guaranteed by SPEC-19 future evolution — is not, even though the user brief explicitly asked to verify this.

The architecture verdict is MINOR DRIFT (not NEEDS RESTRUCTURING): the LeaveKind duplication is a localized bug, the dependency direction `net ← reduction ← partition ← merge ← protocol` is otherwise respected, the core layer stays pure (no tokio in `merge/`, `partition/`, `reduction/`, `net/`), and the FSM-vs-runtime split is preserved (the FSM transition table doesn't yet handle the new SPEC-20 events — that's TASK-0436's job — but the wildcard absorber is documented and tested).

Wave 2 sanity audit is PASS. The flagged −1521/+588 LoC delta breaks down as: (a) `protocol/coordinator.rs` lost ≈1187 LoC of which ≈800 LoC was verbose multi-paragraph doc comments + the v1 `run_coordinator` body whose elastic replacement is now ≈360 LoC of new logic; (b) `protocol/error.rs` lost 363 LoC almost entirely from replacing a manual `Display` impl with `thiserror::Error` derive macros (≈150 LoC) plus collapsing per-variant docstrings to single-line attributes (≈200 LoC); the new variants `Coordinator(Box<...>)` and `Fatal(String)` are additive. No requirement was deleted; the only semantic change is that `AuthFailed` gained a `reason: String` field (compatible — every callsite was updated). The +59 LoC in `partition/helpers.rs` is `partition_index_of` plus its tests; +142 LoC in `protocol/types.rs` is the 5 new Message variants + 3 supporting types + their bincode round-trip tests — all accounted for.

**Cleared for Stage 5 QA after MF-001..MF-004 are fixed.**

---

## 2. Findings

### Must-Fix

#### MF-001 — `JoinRequest.protocol_version` and `JoinNackReason::ProtocolVersionMismatch` use `u8` instead of `u32` (SPEC-20 R35 / R35-cross-spec-version-shape / NF-009)

**Category:** Spec Violation (wire shape) / Code Quality (primitive type drift)
**Principle/Spec:** SPEC-20 §3.5 R35 (typed schema), R35-cross-spec-version-shape (NF-009), TASK-0418 acceptance criterion 3
**File:** `relativist-core/src/protocol/types.rs:198`, `:269-276`, `:439`, `:1244`
**Problem:** SPEC-20 §3.5 R35 explicitly types `JoinRequest.protocol_version: u32` and `JoinNackReason::ProtocolVersionMismatch { coordinator: u32, worker: u32 }`. The implementation uses `u8` for both, plus renames the field pair to `{ expected, got }`. NF-009's whole point is bit-exact shape alignment between the `Register` and `JoinRequest` rejection paths; a v3 worker hitting a v4 coordinator must observe identical payload shapes regardless of which window it connected during. Diverging the type (u8 vs u32) and the field names (`expected/got` vs `coordinator/worker`) makes that impossible.

**Before:**
```rust
// types.rs:196-203
JoinRequest {
    /// Protocol version for fast rejection (MUST match `Register.protocol_version`).
    protocol_version: u8,
    auth_token: Option<[u8; 32]>,
    capabilities: WorkerCapabilities,
},

// types.rs:269-276
pub enum JoinNackReason {
    ProtocolVersionMismatch {
        expected: u8,
        got: u8,
    },
    // ...
}
```

**After:**
```rust
JoinRequest {
    /// Protocol version for fast rejection (R35: u32 per NF-009 shape alignment
    /// with SPEC-19 R37 RegisterNack version-mismatch rejection).
    protocol_version: u32,
    auth_token: Option<[u8; 32]>,
    capabilities: WorkerCapabilities,
},

pub enum JoinNackReason {
    /// SPEC-20 R35 / NF-009: payload shape MUST be { coordinator: u32, worker: u32 }
    /// to match the SPEC-19 R37 RegisterNack ProtocolVersionMismatch path.
    ProtocolVersionMismatch {
        coordinator: u32,
        worker: u32,
    },
    // ...
}
```
And update all callsites in `protocol/coordinator.rs:131-141, 245-254, 297-313` plus the byte-level test fixture at `types.rs:1244` and the smoke fixture at `:439`.

**Why:** This is the entire point of NF-009 closure — the spec ratified `{ coordinator: u32, worker: u32 }` *specifically* so that whichever side rejects (initial `Register` or mid-session `JoinRequest`), the wire shape is identical. Locking this in u8 wastes the closure work and creates a guaranteed migration cost the moment SPEC-19 R37's revision lands its mirroring `RegisterNackReason::ProtocolVersionMismatch { coordinator: u32, worker: u32 }`. Note: `RegisterPayload.protocol_version` is *also* `u8` today — that is a pre-existing type carried over from SPEC-18 and changing it is its own wire break that this Must-Fix does NOT mandate; the asymmetry is acceptable until SPEC-19 R37's next revision lands. The MF here is exclusively about the SPEC-20-introduced surfaces (JoinRequest + JoinNackReason).

---

#### MF-002 — `LeaveKind` is defined in TWO places (`partition::types` AND `protocol::types`); they will silently diverge

**Category:** Architecture (duplicate type with different derive sets — exactly the failure mode TASK-0414 MF-002 sought to prevent)
**Principle/Spec:** SPEC-13 R28 (single shared types module below the protocol layer); SPEC-20 R21 (single canonical `LeaveKind`)
**File:** `relativist-core/src/partition/types.rs:25-32`, `relativist-core/src/protocol/types.rs:241-251`
**Problem:** TASK-0414 closure correctly relocated `LeaveKind` to `partition::types::LeaveKind` (TASK-0414 MF-002 fix). However, when TASK-0418 added `Message::LeaveRequest { kind: LeaveKind }`, instead of importing `crate::partition::LeaveKind`, the developer **defined a second `LeaveKind`** at `protocol::types::LeaveKind` (with a different derive set: protocol's has `Serialize, Deserialize`; partition's does not — note the partition-side docstring even says "TASK-0418 will add Serialize/Deserialize here", but that never happened — the second definition was added instead).

```rust
// partition/types.rs:25-32 (used by coordinator.rs FSM)
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeaveKind { AfterResult, Urgent }

// protocol/types.rs:241-251 (used by Message::LeaveRequest)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub enum LeaveKind { AfterResult, Urgent }
```

These are distinct Rust types. The coordinator's FSM consumes `partition::LeaveKind` via `pub use crate::partition::LeaveKind;` at `coordinator.rs:34`. The wire layer's `Message::LeaveRequest.kind` is `protocol::LeaveKind`. Today they happen to have identical variant lists, so the bug is dormant — but the moment any future task adds a third variant (e.g., `Drain`, `Migration`), the developer will add it to one and forget the other, and the wire-to-FSM bridge will silently drop or misinterpret the new variant. This is precisely the "silent divergence" risk MF-002 from the TASK-0414 review warned against in writing.

**Before:**
Two enum definitions, in `partition/types.rs:25-32` and `protocol/types.rs:241-251`.

**After:**
Promote `partition::types::LeaveKind` to be the canonical definition, add `Serialize/Deserialize/rkyv::*` derives there, and replace `protocol::types::LeaveKind` with a re-export:
```rust
// partition/types.rs (canonical site)
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub enum LeaveKind { AfterResult, Urgent }

// protocol/types.rs
pub use crate::partition::LeaveKind;
```
Then replace the inline definition at `protocol/types.rs:241-251` with the re-export and delete the duplicate `derive` block. Update `Message::LeaveRequest.kind` import path if the impl pattern requires it (compiler will guide; should be a no-op once the re-export is in place).

**Why:** SPEC-13 R28 mandates that types shared between the protocol layer and the FSM layer must live in a layer below both. The duplication today is a compiled-but-architecturally-wrong solution — it works only because the variant lists are identical, and only until they aren't. The fix is a 4-line attribute addition + a 1-line re-export. Note: adding `Serialize/Deserialize` to `partition::types::LeaveKind` is safe because the type is currently not on any wire path; the protocol layer's wire-format usage is the *only* serialization site, and that becomes the same site after the re-export.

---

#### MF-003 — Departure recovery in `run_coordinator` reconstructs successfully then aborts; partial implementation is worse than no implementation

**Category:** Latent Bug (production logic) / Spec Violation (R26a, R27)
**Principle/Spec:** SPEC-20 R26a (D == K_eff edge case), R27 (fall-back rules), R29a (at-least-once); also SPEC-13 R21 baseline
**File:** `relativist-core/src/protocol/coordinator.rs:766-833`
**Problem:** Wave 3 introduced a partial elastic-departure path: when one or more workers depart in a round, the coordinator calls `materialize_reclaimed_partitions` + `BorderGraph::from_partition_plan` + `reconstruct(...)` and assigns the result to `current_net`. It then logs `"Departure recovery reconstruction succeeded."` at INFO level, mutates `_round_reclaimed_initial`, and **immediately returns `ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up")`** at line 832.

```rust
// line 821-832
let merged_net = reconstruct(&border_graph, surviving_partitions, reclaimed_partitions);
current_net = merged_net;
tracing::info!(agent_count = current_net.count_live_agents(), "Departure recovery reconstruction succeeded.");

_round_reclaimed_initial += departing_worker_ids.len() as u32;
let _ = _round_reclaimed_initial;

return Err(ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into()));
```

This is functionally worse than the v1 fatal-on-disconnect baseline (which Phase B is supposed to preserve under `elastic_departure = false`). Three problems:

1. **The path runs even when `elastic_departure = false` is the only valid Phase B state** (Phase B does not own departure recovery — that's Phase D, TASK-0438+). The fact that any of `departing_worker_ids.push(...)` paths fire under Phase B at all (`handle_connection_loss` returns `RecoveryTriggered` whenever `grid_config.elastic_departure = true`) means a Phase B run with `elastic_departure = true` does heavy work then aborts; a Phase B run with `elastic_departure = false` correctly aborts via `ConnectionLossOutcome::Abort` at the same call site. The partial path is dead-code under the only correct Phase B configuration, but it pollutes the function with mutable state changes (`current_net = merged_net`, `metrics` mutations) that occur *before* the eventual abort.

2. **The metrics push for `workers_departed_per_round`, `retained_initial_reclaims_per_round`, etc., is unreachable** (it sits at line 836+, *after* the `return Err(...)`). Therefore even if a reader of `GridMetrics` post-mortem expected to see departure stats on the round that aborted, they will not be populated.

3. **`metrics.merge_time_per_round.push(...)` on the join-window path at line 959** is the *wrong* metric — it records join-window duration but pushes into the merge_time field. This is independent of the abort issue but is co-located.

Phase B's contract (per task scope) is to land the *foundations* needed for Phase C/D, NOT to ship a half-working departure path. The cleanest fix is to gate the entire departure block behind a feature flag or behind a debug assertion, and revert to the v1 fatal-on-departure semantics for Phase B.

**Before:**
```rust
if !departing_worker_ids.is_empty() {
    // ... 70 LoC of partial implementation that ends with:
    return Err(ProtocolError::Fatal("Departure recovery reconstruction succeeded but stream management is TASK-0443 follow-up".into()));
}
```

**After (Phase B scope):**
```rust
if !departing_worker_ids.is_empty() {
    // SPEC-20 §3.3 elastic departure recovery is a Phase D deliverable
    // (TASK-0438..0443). For Phase B, fall back to the v1 fatal-on-departure
    // baseline so partial state is never observed.
    return Err(ProtocolError::Fatal(format!(
        "Departure of workers {:?} detected; recovery is Phase D (TASK-0438..0443)",
        departing_worker_ids
    )));
}
```
And separately fix the metric on line 959:
```rust
// Correct: push to a join-window-specific metric, NOT merge_time_per_round.
metrics.join_round_overhead_ms_per_round.push(t_window_start.elapsed().as_millis() as u64);
```
(Currently `metrics.merge_time_per_round.push(t_window_start.elapsed())` is wrong.)

**Why:** A "succeeded then aborted" path is the most confusing failure mode for a downstream operator: the INFO-level `"Departure recovery reconstruction succeeded"` log line precedes the FATAL error, so post-mortem readers will conclude the recovery worked, then be surprised by the abort. Either the recovery is wired end-to-end (Phase D job — out of scope for Phase B) or it is omitted entirely; the middle ground is a hazard. As a side benefit, removing the partial block also un-buries the metric mis-assignment at line 959.

---

#### MF-004 — `Message` enum lacks `#[non_exhaustive]` (asymmetric with all other extended public enums; explicit user-brief check)

**Category:** Code Quality / Architecture (future-proofing)
**Principle/Spec:** SPEC-06 R5 (append-only); user brief item #2 ("Verify `#[non_exhaustive]` on enums that may grow (Message, CoordinatorEvent, CoordinatorAction)")
**File:** `relativist-core/src/protocol/types.rs:42-43`
**Problem:** All other public enums extended in this bundle carry `#[non_exhaustive]`:
- `CoordinatorState` (`coordinator.rs:109`) ✓
- `CoordinatorEvent` (`coordinator.rs:172`) ✓
- `CoordinatorAction` (`coordinator.rs:284`) ✓
- `RetainedSlot` (`coordinator.rs:53`) ✓
- `DepartureKind` (`coordinator.rs:71`) ✓
- `TimerKind` (`protocol/timers.rs:24`) ✓
- `partition::LeaveKind` (`partition/types.rs:25`) ✓
- `protocol::LeaveKind` (`protocol/types.rs:240`) — *no derive*, but no `non_exhaustive` either

But the most-consumed public enum — `Message` itself, the enum that gained 5 new variants in this very bundle — has no `#[non_exhaustive]`. The R37 contract is "append-only forever"; future variants 17, 18, ... are guaranteed by SPEC-21+. External consumers (test fixtures, alternate transports, observability decoders) writing exhaustive matches on `Message` will silently break the moment a new variant is added.

**Before:**
```rust
// types.rs:42-43
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
```

**After:**
```rust
/// IMPORTANT: New variants MUST be appended at the end of this enum to
/// preserve bincode discriminant stability (SPEC-06 R5, SPEC-19 R37, SPEC-20 R37).
/// `#[non_exhaustive]` forces external matchers to include `_ =>` arms; new
/// variants MAY be added in any future spec without breaking downstream builds.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
```

**Why:** This is a one-attribute change. Internal exhaustiveness is preserved (the crate's own `match` blocks already use `_ => return Err(...)` arms; e.g., `protocol/coordinator.rs:316`, `:419`, `:651`); adding `#[non_exhaustive]` does NOT break the existing in-crate matches because `#[non_exhaustive]` only forces wildcard arms in *external* crates. Today the only external match site is in test fixtures and they already use wildcard guards. The user brief flagged this as a mandatory check; landing it now is the cheapest possible fix for a known forward-compat hazard.

---

### Should-Fix

#### SF-001 — Spec field-name divergence in `Message::JoinAck` (`worker_id` vs `assigned_worker_id`)

**Category:** Spec Compliance (informative — wire shape unchanged)
**Spec:** SPEC-20 §3.5 R35 schema column for discriminant 13
**File:** `relativist-core/src/protocol/types.rs:208-215`
**Problem:** The spec schema says `JoinAck { assigned_worker_id: WorkerId, partition_index: u32, next_round_number: u32 }`. The impl uses `worker_id` instead of `assigned_worker_id`. Since serde derives use *field names* (not positions) for struct variants, this is technically a wire shape difference for any externally-tooled bincode decoder that hand-writes the schema. In practice, all serialization is symmetric (encode/decode both use the same struct, so round-trip works), but a future tool generating a JSON/Protobuf cross-mapping from the spec text would mis-align.

The cost-benefit is: rename costs ~5 callsites; benefit is exact spec adherence. Recommend rename to `assigned_worker_id`.

```rust
// Before
JoinAck {
    worker_id: WorkerId,
    partition_index: u32,
    next_round_number: u32,
},
// After
JoinAck {
    assigned_worker_id: WorkerId,
    partition_index: u32,
    next_round_number: u32,
},
```

---

#### SF-002 — CLI flags do not provide the `--no-*` form mandated by SPEC-20 R34

**Category:** Spec Compliance (UX surface)
**Spec:** SPEC-20 §3.4 R34 (`--hybrid` / `--no-hybrid`, `--elastic-departure` / `--no-elastic-departure`, etc.)
**File:** `relativist-core/src/config.rs:206-225`
**Problem:** R34 specifies a paired flag form for every elastic boolean: `--hybrid` / `--no-hybrid`, `--elastic-departure` / `--no-elastic-departure`, `--elastic-join` / `--no-elastic-join`, `--retain-partitions` / `--no-retain-partitions`, `--checkpoint-partitions` / `--no-checkpoint-partitions`. The implementation uses `clap::ArgAction::SetTrue` only — there is no `--no-*` companion flag. Today this works because every boolean defaults to `false`, so a user who omits `--hybrid` correctly gets `hybrid=false`; but R34's intent is to allow operators to explicitly disable a derived/auto-enabled value (e.g., disable `elastic_join` even though `hybrid_coordinator` is set, which auto-enables it via `normalize`).

Recommend adopting clap's `Arg::overrides_with` pattern or `ArgAction::SetFalse` with negated companions:
```rust
#[arg(long, action = ArgAction::SetTrue, overrides_with = "no_hybrid")]
pub hybrid: bool,
#[arg(long = "no-hybrid", action = ArgAction::SetTrue, overrides_with = "hybrid")]
pub no_hybrid: bool,
// in build_grid_config: hybrid_coordinator: args.hybrid && !args.no_hybrid
```

If the user community is fine with a single-flag UX, document the deviation from R34 explicitly in `GridConfig::active_mode` Rustdoc and make `validate()` reject contradictory derived values.

---

#### SF-003 — 7 of 12 Phase B tasks lack a formal `docs/tests/TEST-SPEC-04*.md` doc

**Category:** Test Coverage / Process
**File:** `docs/tests/`
**Problem:** Test-spec docs exist for `0415, 0416, 0418, 0419, 0426`. They do NOT exist for:

| Task | Topic | Where tests live (inline) |
|------|-------|---------------------------|
| 0417 | PROTOCOL_VERSION bump | `protocol/coordinator.rs:1092-1095, 1097-1127, 1129-1159, 1161-1189` (4 tests) |
| 0420 | WorkerId reservation + partition_index | `partition/helpers.rs:809-841` (2 tests) |
| 0421 | ID range K_eff recomputation | `partition/helpers.rs` (test exists in `mod tests`; not enumerated by name in the diff) |
| 0422 | tokio::select event loop | none directly — exercised through `accept_workers` tests + `process_join_request` tests |
| 0423 | Self-worker spawn | `protocol/self_worker.rs` has the spawn function but **no inline tests** in the file |
| 0424 | Strict-BSP self-uniformity | none — task is described as "verification" only |
| 0425 | SoloReducing state batch loop | exercised indirectly through the `solo_budget` config test in `merge/types.rs` |

This is a process gap, not a code defect. The Stage 5 QA agent will need to either (a) accept the inline tests as the contract OR (b) flag the missing test-spec docs. Recommend creating short stub test-spec docs for the 7 tasks pointing at the inline tests.

---

#### SF-004 — `protocol/self_worker.rs` lacks any unit tests

**Category:** Test Coverage
**File:** `relativist-core/src/protocol/self_worker.rs:1-109`
**Problem:** The new file exposes two public symbols (`spawn_self_partition`, `reduce_solo_batch`) and one struct (`SelfWorkerHandle`) with three fields, plus uses three `unwrap()`-equivalents on the `tokio::task::Handle::current()` and `ChannelTransport::accept` paths. There are no tests in the file. The `expect("ChannelTransport accept must succeed")` at line 36 is the closest the file comes to defensive coding; it is the only `expect` and the message is generic.

The function is invoked by the coordinator at `protocol/coordinator.rs:581` only when `hybrid_coordinator = true && self_partition.is_some()`. Existing coordinator-integration tests do not exercise hybrid mode (the `test_accept_workers_*` family all pass `hybrid_mode = false`).

Recommend adding at minimum:
- `spawn_self_partition_round_trips_a_partition_result` — spawn, send `AssignPartition`, receive `PartitionResult`, assert `is_coordinator_self == true` in stats.
- `spawn_self_partition_panic_propagates_via_oneshot` — instrument the spawned task to panic, assert `panic_rx.try_recv()` carries the message.

The `expect("ChannelTransport accept must succeed")` should be replaced with a `?` or `unwrap_or_else(|e| ...)` that propagates a `ProtocolError::Fatal` through the public API; today a `ChannelTransport::accept` failure on a fresh in-process channel is virtually impossible, but the API contract treats the function as infallible-by-panic, which is inconsistent with the rest of `protocol/`.

---

#### SF-005 — Wave 2 helper enums (`DepartureEventKind`, `ConnectionLossOutcome`) are `#[allow(dead_code)]` without the conventional "wired by TASK-NNNN" comment

**Category:** Code Quality (developer-discipline)
**File:** `relativist-core/src/protocol/coordinator.rs:39, 47`
**Problem:** Both enums lost their TASK-NNNN-specific comments during the Wave 2 doc-stripping refactor. Wave 1 (commit `4fb77bc`) had:
```rust
#[allow(dead_code)]
// production call site lands when run_coordinator is wired (TASK-0438); GridConfig field lands separately (TASK-0415).
pub(crate) enum DepartureEventKind { ... }
```
Wave 2 (commit `ba846ad`) collapsed this to:
```rust
#[allow(dead_code)]
pub(crate) enum DepartureEventKind { ... }
```
The user brief's check #10 explicitly allows `#[allow(dead_code)]` *with* a "wired by TASK-NNNN" comment but flags bare suppression as not acceptable. Wave 2 stripped the comment.

In practice both enums are now USED (called from line 689 and 723 of the same file), so the `#[allow(dead_code)]` itself is over-conservative; remove it. If it must stay (e.g., for a `--features delta-only` configuration), restore the explanatory comment.

---

#### SF-006 — `merge_time_per_round` is being incorrectly populated by the join-window duration

**Category:** Latent Bug (metrics misclassification) — co-located with MF-003 but independent
**File:** `relativist-core/src/protocol/coordinator.rs:959`
**Problem:**
```rust
metrics.merge_time_per_round.push(t_window_start.elapsed());
```
`t_window_start` is the join-window start time (line 904). Pushing it into `merge_time_per_round` corrupts the merge-time metric: any post-run analyzer keyed on `merge_time_per_round` will conflate merge-phase time with join-window-drain time. The correct field is `metrics.join_round_overhead_ms_per_round`, which is even initialized as a placeholder elsewhere in the same loop (line 846: `metrics.join_round_overhead_ms_per_round.push(0)`). One value is being pushed twice into different fields per round.

This is a Should-Fix because it does not affect correctness of the BSP loop, only the semantics of metrics output. Strictly it is a Phase B regression of the SPEC-19 R45 metric contract.

```rust
// Before
metrics.merge_time_per_round.push(t_window_start.elapsed());
// After
metrics.join_round_overhead_ms_per_round.push(t_window_start.elapsed().as_millis() as u64);
// And remove the placeholder push at line 846 to avoid double-counting.
```

---

## 3. Passed Checks

- [x] **No `unwrap()` in production code** — `protocol/self_worker.rs:611` uses `.unwrap()` on `self_partition.as_ref()` with a guard one line earlier; `protocol/coordinator.rs:581-617` is structurally guarded by `if let Some(ref _p) = self_partition`. Test-only `unwrap`s are fine. (One `expect("ChannelTransport accept must succeed")` in `self_worker.rs:36` — flagged as SF-004.)
- [x] **No `unsafe` blocks**
- [x] **No `println!`** — tracing macros only (verified by inspection across all 9 files)
- [x] **`thiserror` errors** — `ProtocolError`, `ConfigError`, all `*Error` types use `#[derive(Error)]`. Wave 2 explicitly migrated `ProtocolError`'s manual `Display` impl to `thiserror`.
- [x] **Module boundaries respected** — core layer (`net/`, `reduction/`, `partition/`, `merge/`) imports no tokio, no async, no `protocol::` types. Spot checks: `merge/types.rs` only imports `crate::partition::*`; `partition/helpers.rs` only imports `crate::net::*` and `crate::merge::GridConfig` (this is the one direction core-into-merge that's permitted by SPEC-13's `partition <- merge` arrow being a *type-only* dependency — no logic). `partition::types::LeaveKind` is in the correct shared layer.
- [x] **TimerKind discriminants pinned** — two test sites: `protocol/timers.rs:46-51` and `coordinator.rs:1344-1366`; both assert `0/1/2/3` for `InitialWait/JoinWindowMin/JoinWindowMax/Collect`.
- [x] **Message enum discriminants 0..=16 are stable** — `protocol/types.rs:1164-1294` covers all 17 variants byte-by-byte; cardinality assertion at the tail forces future maintainers to extend the test.
- [x] **PROTOCOL_VERSION pinned at 4** — `protocol/coordinator.rs:1092-1095` plus three `qa_probe_*` tests covering v0/v1 rejection paths.
- [x] **Discriminant-stability for `LeaveKind` / `JoinNackReason`** — round-trip in `test_all_variants_serde_roundtrip` (`types.rs:386-464`).
- [x] **TASK-0414 carryover Must-Fix items closed:**
  - MF-001 (raw `0` literals for collect timer): now `TimerKind::Collect as TimerId` at `coordinator.rs:494, 519`; new tests `test_all_dispatched_starts_collect_timer_with_correct_id` and `test_all_partitions_returned_cancels_collect_timer_with_correct_id` pin the id.
  - MF-002 (`LeaveKind` location): moved to `partition::types::LeaveKind` ✓ — but see MF-002 above for the *new* divergent duplicate in `protocol::types`.
  - MF-003 (`InvokeSplitAndDispatch(u32)` → named type): now `EffectiveWorkerCount` alias at `coordinator.rs:267, 343`.
  - SF-001 (wildcard arm comment): comment at `coordinator.rs:588-597` references TASK-0436.
  - SF-002 (CoordinatorContext NOTE): comment at `coordinator.rs:368-381`.
  - SF-003 (UT-0414-09 inclusion of new variants): the systematic loop at `coordinator.rs:1164-1180` now includes all 11 variants.
  - SF-004 (`#[non_exhaustive]` on FSM enums): present on `CoordinatorState`, `CoordinatorEvent`, `CoordinatorAction`, `RetainedSlot`, `DepartureKind`, `TimerKind`. (Note: NOT present on `Message` — see MF-004.)
- [x] **`is_coordinator_self` field plumbed through `WorkerRoundStats` and the self-worker** — verified at `merge/types.rs:199-200` (field default false) and `protocol/self_worker.rs:75` (`is_coordinator_self: true` in the spawned worker's stats).
- [x] **Newtype IDs preserved** — `WorkerId` (alias `u32`), `AgentId` (alias `u32`), `BorderId` (alias `u32`), `TimerId` (alias `u32` derived from `TimerKind as u32`).
- [x] **Wave 2 sanity audit** — confirmed (see §1 paragraph 3); the −1521/+588 LoC delta is dominated by doc-stripping (+thiserror migration + V1 `run_coordinator` body replacement). No production behavior was deleted without a destination. The +59 LoC in `partition/helpers.rs`, +142 LoC in `protocol/types.rs`, and +5 LoC in `merge/grid.rs` cumulatively account for the additive scope of TASK-0418/0420.
- [x] **G1 confluence preserved by self-worker** — self-worker uses the same `reduce_all` primitive (`self_worker.rs:62`) and emits a `WorkerRoundStats` carrying its `is_coordinator_self` flag. The merge path at `protocol/coordinator.rs:861-867` does NOT special-case `is_coordinator_self == true`; the K_eff partitions flow through `merge::merge` uniformly. R3c uniformity holds by construction.
- [x] **R25 conditional preserved (delta-mode-only invariant)** — verified via `protocol/delta_wire_tests.rs` (untouched by Wave 2 except `+1` line) and the `coordinator_free_rounds` field test at `merge/types.rs`. Wave 2 did not regress R25.

---

## 4. Wire-Serde Regression Assessment

The byte-level discriminant test at `protocol/types.rs:1164-1294` covers all 17 `Message` variants and forces the cardinality at the tail (`assert_eq!(cases.len(), 17, ...)`). New variants 12–16 carry their own per-variant round-trip in `test_all_variants_serde_roundtrip`. The `#[cfg_attr(feature = "zero-copy", derive(rkyv::*))]` is present on `LeaveKind`, `WorkerCapabilities`, `JoinNackReason`, and `TimerKind`. `RegisterPayload` retains its existing `protocol_version: u8` (pre-existing wire shape; not changed by this bundle).

Two divergences from spec wire shape (MF-001 above) are NOT covered by the existing tests — they are spec-text violations, not test failures. `JoinRequest.protocol_version` round-trips fine as `u8` because both encode and decode see `u8`; the test is internally consistent but externally non-compliant.

---

## 5. Test Coverage Assessment vs `docs/tests/TEST-SPEC-04*`

| Task | Test-Spec Doc | Inline Tests | Coverage |
|------|---------------|--------------|----------|
| 0415 GridConfig elastic fields | TEST-SPEC-0415 | `merge/types.rs:934-1083` (8 tests covering defaults / normalize / validate / active_mode / wire round-trip) | ✓ FULL |
| 0416 CLI elastic flags | TEST-SPEC-0416 | `config.rs:1207-1246` (visible spot-checks) | ✓ FULL |
| 0417 PROTOCOL_VERSION bump | *missing* | `protocol/coordinator.rs:1092-1189` (4 tests) | ✓ adequate |
| 0418 Message variants | TEST-SPEC-0418 | `protocol/types.rs:386-1294` (extensive bincode round-trip + discriminant stability) | ✓ FULL |
| 0419 handshake Register vs JoinRequest | TEST-SPEC-0419 | `protocol/coordinator.rs:1129-1189` (qa_probe_5 / qa_probe_9 / smoke) | ✓ adequate |
| 0420 WorkerId reservation + partition_index | *missing* | `partition/helpers.rs:809-841` (2 tests) | ⚠ partial — no test for `WorkerId 0` reservation in non-hybrid mode |
| 0421 ID range K_eff recomputation | *missing* | `partition/helpers.rs` (in-mod tests, names not visible from diff) | ⚠ assumed adequate (cargo test count: +12 in Wave 1) |
| 0422 tokio::select event loop | *missing* | none directly | ✗ no targeted test; integration coverage only |
| 0423 self-worker spawn | *missing* | none in `protocol/self_worker.rs` | ✗ NO TESTS — see SF-004 |
| 0424 strict-BSP self-uniformity | *missing* | none — task is verification-only | ✗ no test; rely on integration-level EG-U17 (Phase D) |
| 0425 SoloReducing state | *missing* | indirect via config test only | ✗ no targeted test |
| 0426 TimerKind | TEST-SPEC-0426 | `protocol/timers.rs:46-51` + `coordinator.rs:1344-1366` (two redundant pinning tests) | ✓ FULL |

**Summary:** 4 of 12 tasks have FULL coverage; 4 have adequate coverage; 2 are partial; 2 (TASK-0423, TASK-0424, TASK-0425, TASK-0422) are essentially uncovered at the unit level. Phase D will integration-test these via EG-U1, EG-U16, EG-U17, but Stage 5 QA should treat 0423/0425 as priority adversarial probes.

---

## 6. Recommendation for Stage 5 (QA)

**ACCEPT_WITH_FIXES.** Stage 5 may proceed in parallel with the Must-Fix patch as long as the patch is in flight. QA priorities for adversarial follow-up, in descending order:

1. **MF-001 wire-shape probe.** Decode a `JoinRequest`/`JoinNack` byte stream produced by a hand-written u32 encoder; assert today's `u8` impl rejects it cleanly (with `Deserialize` error). After MF-001 is fixed, the same probe must succeed. Both directions matter for cross-spec NF-009.

2. **MF-002 LeaveKind divergence trap.** Add a test that constructs a `Message::LeaveRequest { kind: protocol::LeaveKind::Urgent }` and tries to feed its `kind` field directly into a `CoordinatorEvent::WorkerLeft(..., partition::LeaveKind)` constructor. The current code WILL fail to compile (different types) — that's the bug. After the MF-002 fix (single canonical type), it MUST compile and round-trip.

3. **MF-003 partial-recovery probe.** With `elastic_departure = true`, simulate a single worker connection-loss in a 2-worker run. Assert the run aborts with a `ProtocolError::Fatal` whose message identifies the departed worker AND assert that no `metrics.workers_departed_per_round` entry is recorded for that round (because the metric push is unreachable). After MF-003 is fixed, the failure mode must be a clean abort with NO `"reconstruction succeeded"` log line preceding it.

4. **TASK-0423 self-worker stress.** Spawn 100 self-partition tasks back-to-back with synthetic panics injected randomly; assert `panic_rx` carries the panic for every panicked task and `join_handle.await` returns successfully (no panic propagation). This catches the SF-004 missing-tests gap.

5. **TASK-0419 mid-session-with-Register hardening.** Connect via TCP, send `Register` instead of `JoinRequest` while the coordinator's FSM is in `WaitingForResults`; assert the connection is buffered and `Register` is rejected with a `RegisterNack` whose reason describes the violation (TASK-0419 acceptance criteria item 2 — "Register received mid-session → rejected as protocol violation"). The current impl at `protocol/coordinator.rs:316-323` issues a generic "protocol error" reason; verify that's QA-acceptable.

6. **R26a hybrid edge-case (deferred to Phase D, but spot-check now).** With `hybrid_coordinator = true`, force the only remote worker to disconnect during a round. Today this hits MF-003's premature-abort path. Post-fix, it must abort cleanly per the Phase B baseline. Phase D will replace the abort with a fall-back to `SoloReducing` per R27.

7. **Metric mis-classification probe (SF-006).** Run a 5-round elastic grid; verify `merge_time_per_round` does NOT spike on rounds where the join-window-min timer fired but no join occurred (today it does because of the line-959 mis-push).

8. **Visual sanity check on `protocol/coordinator.rs:557-650` early-allocation logic.** Specifically, the `partitions_iter.next()` self-partition extraction at line 573-577 happens BEFORE retained-state is consulted at line 588-600; if a future task adds delta-mode self-partition retention, the iterator state will be consumed twice. Probe with a small input net to confirm no panic / partition mis-assignment at K_eff = 1 hybrid mode.

The bundle is structurally sound; the four Must-Fix items are mechanical (renaming + attribute + type unification + dead-code removal), totaling under ~30 LoC in production. Land them in Wave 5 of Phase B (or as a fast-follow patch to Phase B Wave 4) before Phase C dispatches anything that touches `Message::JoinRequest` or `Message::LeaveRequest`.

Phase B: 4 Must-Fix, 6 Should-Fix, verdict: ACCEPT_WITH_FIXES
