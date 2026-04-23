# Deferred Work — v2 Tracker

**Purpose:** Single source of truth for spec/feature work that was **partially shipped**
because a hard dependency was not yet implemented. Prevents items from being
silently forgotten when their unblocker lands.

**How to use this file:**
1. When deferring scope from a spec, add a row below with the *unblocking
   milestone/spec/task*. Do not delete the row when shipping the partial scope.
2. When the unblocker ships (e.g., SPEC-25 is implemented), open this file,
   find the row, and create a follow-up task to complete the deferred scope.
3. Only remove a row after the deferred scope is fully implemented and verified.

---

## Active Deferrals

### D-001 — SPEC-27 R26 / R27 / R28 (RecipeEncoder integration with SPEC-25)

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-27 (Encoder/Decoder Trait API) |
| **Requirements deferred** | R26, R27, R28 |
| **Shipped instead** | R24 + R25 only (trait definition + non-coupling guarantee) — TASK-0340 |
| **Unblocker** | SPEC-25 (Recipe-Based Distributed Generation) implementation = ROADMAP item 2.29 = **Milestone M7** |
| **Why deferred** | SPEC-25 is not yet implemented in code (no `GenerationRecipe` type, no `AssignRecipe` wire message, no `make_recipe()` calls in coordinator/worker). R26/R27/R28 are *integrations* with code that does not exist. Shipping them now would mean writing speculative integration code with no observable behavior. |
| **What R26 needs** | Refactor `GenerationRecipe` (introduced by SPEC-25) to implement the new `RecipeEncoder::Recipe` associated type. Mechanical once SPEC-25 lands. |
| **What R27 needs** | Generalize the `AssignRecipe` wire message (introduced by SPEC-25) so it carries any `RecipeEncoder::Recipe`, not just the 9 built-in generators. Touches SPEC-06 wire format. |
| **What R28 needs** | Workers obtain the `EncoderRegistry` and dispatch on the encoder name in the recipe envelope. Trivially compiled in (registry is static), but needs a wire-side encoder identifier and a worker-side lookup. |
| **Estimated effort** | ~150 LoC (R26 + R27) + ~50 LoC (R28) = **~200 LoC**, 2-4 days, after SPEC-25 is in. |
| **Acceptance signal** | At least one real `RecipeEncoder` (e.g., `ep_annihilation`) ships a recipe end-to-end through the registry: coordinator sends recipe over the wire, worker materializes its partition locally, reduction proceeds, results decode correctly. |
| **Files to revisit** | `relativist-core/src/encoding/recipe.rs`, `relativist-core/src/encoding/registry.rs`, `relativist-core/src/protocol/messages.rs`, `relativist-core/src/coordinator.rs`, `relativist-core/src/worker.rs` |
| **Created** | 2026-04-16 (during Phase 6 mínimo of SPEC-27) |
| **Status** | OPEN — waiting on M7 |

**Action when M7 starts:**
1. Open `relativist-core/src/encoding/recipe.rs` — the `RecipeEncoder` trait is already defined and tested.
2. Create `TASK-03XX` (RecipeEncoder integration) under SPEC-27 covering R26+R27+R28 in a single ticket (they are tightly coupled).
3. Refactor SPEC-25's `GenerationRecipe` to implement `RecipeEncoder::Recipe`.
4. Generalize `AssignRecipe` wire message to carry `(encoder_name, recipe_bytes)`.
5. Worker receives `AssignRecipe`, looks up encoder by name in `default_registry()`, calls `generate_partition(&recipe)`.
6. Add an integration test demonstrating the full coordinator→worker recipe path with at least one decomposable codec.
7. Close this row after acceptance signal is met.

---

*(D-002 was resolved on 2026-04-16 — see Resolved Deferrals below.)*

---

### D-003 — SPEC-19 R13 / R14 / R15 parts 1-2 (coordinator-side border-redex resolution)

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-19 §3.2 (item 2.35) |
| **Requirements deferred** | R13, R14, R15 parts 1-2 — originally the full set; **partially closed as of 2026-04-23**. |
| **Shipped (Stage 1)** | R8, R9, R10, R11, R12, R15 part 3, R16, R17, R18, R19 — `BorderGraph` pure data structure (2026-04-17). |
| **Shipped (Stage 2, bundle 2.26 A/B/C/D)** | R20-R36 wire extensions + resolver + BSP loop + stateful-worker lifecycle (2026-04-18). |
| **Shipped (Stage 3, refactor 2026-04-23)** | **MF-001 closure** (TASK-0394): worker-side R23/R26 completion — `local_reconnections` + `pending_commutations → minted_agents` 2-phase echo. **MF-002 closure** (TASK-0395): G1 parity integration tests `grid_delta_integration_tests::ut_0385_06/07/08`. **Symmetric rules (CON-CON, DUP-DUP, ERA-ERA) empirically verified** via `LocalDeltaDispatch`-driven round-trip, asserting `canonicalize(out_delta) == canonicalize(out_v1)` and `metrics.total_interactions == metrics_v1.total_interactions` in both `strict_bsp = true` and `strict_bsp = false` matrices. |
| **Still deferred** | **Asymmetric rules (CON-DUP, CON-ERA, DUP-ERA)** cross-partition G1 parity — D-004 plumbing shipped 2026-04-23 (coordinator-side round-N+2 finalizer) but flip of `SKIP_ASYMMETRIC` still blocked by **D-005** (CommutationBatch.local_wiring does not cross the wire, so minted agents land fully disconnected on the worker). In `tests/grid_delta_integration_tests.rs` these fixtures continue to run under `const SKIP_ASYMMETRIC: bool = true;` which asserts convergence only (not net equivalence). |
| **Unblocker** | **D-005** — propagate `CommutationBatch.local_wiring` across the `PendingCommutation` wire contract (or equivalent LocalDeltaDispatch workaround) so worker-side `Net::wire_agents` materialises the agent-internal edges for minted agents. D-004's coordinator-side plumbing (2026-04-23) is already in place waiting. |
| **Acceptance signal — partial** | Symmetric-rule parity: 3 of 6 IC rules verified (UT-0385-06/07/08 symmetric branches, both strict modes). |
| **Acceptance signal — full** | All 6 rules verified; `SKIP_ASYMMETRIC = false`. |
| **Created** | 2026-04-17 |
| **Status** | **PARTIALLY CLOSED** 2026-04-23 — symmetric-rule branch shipped; D-004 coordinator plumbing shipped 2026-04-23; asymmetric flip now waiting on D-005. |

**Action when D-004 ships:**
1. Set `SKIP_ASYMMETRIC = false` in `relativist-core/src/merge/grid_delta_integration_tests.rs`.
2. Re-run `cargo test --workspace --lib` and verify UT-0385-08 passes on all 6 fixtures under both strict modes.
3. Move this row to "Resolved Deferrals" with the commit hash.

---

### D-004 — Coordinator-side round-N+2 finalizer for DC-B5 2-phase commutation flow

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-19 §3.3 R26, R48, DC-B5 (2026-04-17 spec-critic) |
| **Requirements deferred** | Coordinator consumption of `Message::RoundResult.minted_agents` — resolution of `PendingPortRef::Pending { request_id, agent_slot, port_slot }` tokens emitted by the resolver's CON-DUP / CON-ERA / DUP-ERA branches into concrete `PortRef::AgentPort(minted_id, port)` assignments; subsequent call to `BorderGraph::add_border_states` for the `pending_new_borders` packaged with the commutation. |
| **Shipped instead** | Worker-side half (TASK-0394): the worker fulfils `PendingCommutation`s by minting agents from `partition.id_range`, populates `Message::RoundResult.minted_agents` with the correlated `request_id` / `minted_agent_id` pairs. The wire protocol carries the echo (shipped 2.26-A). The *coordinator-side consumption* of that echo never reached DEV because 2.26-C scoped `RoundResultPayload` (the pure-core dispatch type used by `WorkerDispatch`) without the `minted_agents` field, so the coordinator's BSP loop discards it silently. |
| **Unblocker** | None — this is a direct implementation task, not a wait on another spec. Can be scheduled whenever the user prioritises MF-003 closure. |
| **Why deferred** | Discovered during DEV of TASK-0395 (MF-002). The review of 2026-04-23 flagged the wire/worker gap (MF-001, closed) but did not audit the pure-core `RoundResultPayload` shape, so the coordinator-side half was invisible at review time. Scope ~200-400 LoC touching `RoundResultPayload` + `BorderGraph` + `run_grid_delta_inner` + LocalDeltaDispatch update + 3 integration tests (`SKIP_ASYMMETRIC` flip). Non-blocking for the current TCC scope: the v1 protocol (run_grid) is what Phase 3 LAN benchmarks exercise; `delta_mode=true` is v2 future work. |
| **What the fix needs** | (1) Extend `pub struct RoundResultPayload` in `relativist-core/src/merge/types.rs` with `pub minted_agents: Vec<MintedAgent>`. (2) Add `BorderGraph::register_minted_agents(&mut self, worker_id: WorkerId, mints: &[MintedAgent])` in `relativist-core/src/merge/border_graph.rs` that resolves `PendingPortRef::Pending` tokens against the received mints and promotes `pending_new_borders` via `add_border_states`. (3) Wire a call to `register_minted_agents` in `run_grid_delta_inner`'s result-apply loop (`relativist-core/src/merge/grid.rs`), immediately after `apply_deltas`. (4) Update `LocalDeltaDispatch` in `relativist-core/src/merge/grid_delta_integration_tests.rs` to forward `minted_agents` from `Message::RoundResult` into the returned `RoundResultPayload`. (5) Flip `SKIP_ASYMMETRIC = false` and verify UT-0385-08 passes on all 6 fixtures under both strict modes (closes D-003 in full). |
| **Estimated effort** | ~200-400 LoC production + ~50 LoC test updates; 2-4 days of focused work. TDD-friendly: each of steps (1)-(4) is independently testable, and (5) is the acceptance gate. |
| **Acceptance signal** | `cargo test --workspace --lib` retains ≥ 1138 passing (post-refactor-2026-04-23 baseline), UT-0385-08 runs all 6 fixtures × 2 strict modes under `SKIP_ASYMMETRIC = false`, canonicalized net equivalence AND `metrics.total_interactions` parity hold on every case. After this: D-003 closes fully. |
| **Files to revisit** | `relativist-core/src/merge/types.rs` (`RoundResultPayload` struct), `relativist-core/src/merge/border_graph.rs` (`register_minted_agents`), `relativist-core/src/merge/grid.rs` (`run_grid_delta_inner` wire), `relativist-core/src/merge/grid_delta_integration_tests.rs` (`LocalDeltaDispatch` forwarding + `SKIP_ASYMMETRIC` flip). |
| **Created** | 2026-04-23 (during DEV of TASK-0395, post-REVIEW-2026-04-23 bundle close) |
| **Status** | **PARTIALLY SHIPPED** 2026-04-23 via TASK-0398 + TASK-0399. Steps (1)-(4) done: `RoundResultPayload.minted_agents` extended, `BorderGraph::register_minted_agents` + `enqueue_pending_borders` implemented with R48 validation and DC-B6 preserve-existing-border path, `encode_request_id`/`decode_request_id` codec shared between resolver and LocalDeltaDispatch, `run_grid_delta_inner` wired, `package_resolutions_with_pending` exposes pending borders to coordinator. 8 UT-0398-01..08 green. Step (5) `SKIP_ASYMMETRIC` flip **blocked by newly-discovered D-005** (CommutationBatch.local_wiring does not cross the wire; minted agents land disconnected). |

**Action when D-005 ships:**
1. Flip `SKIP_ASYMMETRIC = false` in `relativist-core/src/merge/grid_delta_integration_tests.rs`.
2. Re-run `cargo test --workspace --lib` and verify UT-0385-08 passes on all 6 fixtures under both strict modes.
3. Move D-003 AND this row to "Resolved Deferrals" with the commit hash.
4. (Optional) Address the two SHOULD-FIX follow-ups noted in 2026-04-23 review: (a) tighten `register_minted_agents` docstring re: unchanged state on `Err` **[done inline 2026-04-23]**; (b) consider release-mode `GridError::ProtocolViolation` instead of `debug_assert!` in `encode_request_id` under the unlikely >2^28 commutations per run regime.

---

### D-005 — Worker-side application of `CommutationBatch.local_wiring` for minted agents

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-19 §3.3 R23 / DC-B5 / DC-B6 (2026-04-23 DEV discovery of TASK-0399) |
| **Requirements deferred** | Worker-side application of the agent-internal wiring emitted by the resolver alongside the minted `AgentId`s. Specifically, when a worker receives a `Message::PendingCommutation` and mints agents from its `partition.id_range`, it must also apply each `(slot_a, port_a, slot_b, port_b)` tuple from `CommutationBatch.local_wiring` via `partition.subnet.wire_agents(...)` so that the minted agents' internal edges are materialised before `Message::RoundResult` is returned. Currently the wire protocol does not carry `local_wiring`; only `mint_count` and a single `border_info` reach the worker. |
| **Shipped instead** | D-004 plumbing (coordinator-side round-N+2 finalizer, TASK-0398+TASK-0399) is fully in place: the coordinator correctly resolves `PendingPortRef::Pending` tokens against echoed `MintedAgent`s and promotes fully-resolved `PendingNewBorder`s to `AddBorderEntry`s via `BorderGraph::add_border_states`. However, the minted agents arrive DISCONNECTED (no internal edges) because the worker never receives the wiring instructions. Net result: `SKIP_ASYMMETRIC = true` remains in the integration test file. |
| **Unblocker** | None — direct implementation task. Option A (preferred, wire-level fix): extend `Message::PendingCommutation` and/or introduce a new `AssignCommutation` wire message carrying `local_wiring: Vec<(u8, u8, u8, u8)>`; worker applies via a new `BorderResolver::apply_local_wiring` equivalent method post-mint. Option B (test-only workaround): have `LocalDeltaDispatch` stash `CommutationBatch.local_wiring` keyed by `commutation_id` at resolver-output time and apply it during `dispatch_round_start` before composing the mint echo; production workers remain unchanged and the wire contract does not grow. Option B unblocks the integration tests immediately; Option A is required for real distributed runs. |
| **Why deferred** | Discovered during DEV of TASK-0399 (post-wire of `register_minted_agents`). The initial test run with `SKIP_ASYMMETRIC = false` reached `register_minted_agents` successfully but produced empty/disconnected output nets for CON-DUP / CON-ERA / DUP-ERA because `local_wiring` was dropped at resolver-to-wire serialisation. The gap is pre-existing in SPEC-19 §3.3 (R23 defines `local_wiring` as part of `CommutationBatch` but §3.4 wire encoding for `PendingCommutation` omits the field). D-004 plumbing is correct and isolated; fixing D-005 is orthogonal. |
| **What the fix needs (Option A, production-grade)** | (1) Spec amendment in `specs/SPEC-19.md` §3.4 to add `local_wiring` to `PendingCommutation` wire encoding. (2) Wire type in `relativist-core/src/protocol/types.rs` extended. (3) `relativist-core/src/worker.rs` — consume `local_wiring`, call `partition.subnet.wire_agents(PortRef::AgentPort(minted_id_a, port_a), PortRef::AgentPort(minted_id_b, port_b))` for each tuple. (4) LocalDeltaDispatch forwarding. (5) Flip `SKIP_ASYMMETRIC = false`. |
| **What the fix needs (Option B, test-only)** | (1) `LocalDeltaDispatch` stores `local_wiring` keyed by `commutation_id` when building `PendingCommutation` from `CommutationBatch`. (2) In `dispatch_round_start` mint loop, after `alloc_agent`, apply wiring on the subnet. (3) Flip `SKIP_ASYMMETRIC = false`. Production `worker.rs` untouched. |
| **Estimated effort** | Option A: ~80-150 LoC + spec amendment, 1-2 days. Option B: ~50 LoC test-only, half a day. Recommended path: ship Option B first to unblock full G1 parity proof, then Option A when real LAN distribution benchmarks demand it. |
| **Acceptance signal** | UT-0385-08 passes on all 6 fixtures × 2 strict modes with `SKIP_ASYMMETRIC = false`, canonicalized net equivalence AND `metrics.total_interactions` parity hold on every case. After this: D-003 closes fully AND D-004 closes fully. |
| **Files to revisit** | `relativist-core/src/merge/border_resolver.rs` (`CommutationBatch.local_wiring` source), `relativist-core/src/merge/grid_delta_integration_tests.rs` (`LocalDeltaDispatch` + `SKIP_ASYMMETRIC`), `relativist-core/src/protocol/types.rs` + `relativist-core/src/worker.rs` if Option A. |
| **Created** | 2026-04-23 (during DEV of TASK-0399, post-TASK-0398 plumbing) |
| **Status** | OPEN — independent of other deferrals; scheduled at user discretion. Recommended path: Option B next, Option A before Phase 3 LAN benchmarks in delta mode. |

**Action when D-005 starts:**
1. Decide Option A vs B (Option B is the cheap test-only unblocker; Option A is the production fix).
2. If Option A, amend SPEC-19 §3.4 first (spec-critic round optional given surgical scope).
3. Implement in a single ticket following the 6-stage SDD pipeline.
4. Step (3)/(Option B step 3) flips the canary; it doubles as the D-003 AND D-004 full-closure gate.
5. Close D-003, D-004, AND this row after the canary is green.

---

## Resolved Deferrals (archive)

### D-002 — SPEC-18 R20-R27 (rkyv zero-copy archive path) — SHIPPED 2026-04-16

| Field | Value |
|-------|-------|
| **Source spec** | SPEC-18 (Wire Format v2) §3.5 |
| **Requirements shipped** | R20, R21, R22, R23, R24, R25, R26, R27 + tests T11-T14 |
| **Bundle** | TASK-0352..0359 (~600 LoC across 8 atomic tasks) |
| **Shipped via** | Stage 3 DEV of SPEC-18 §3.5 bundle (item 2.24). Cargo feature `zero-copy = ["dep:rkyv"]` (default OFF). Hot-path messages `AssignPartition` / `PartitionResult` carry rkyv archives with FLAG_ARCHIVED set; receivers use the validating `rkyv::access` API (NEVER `access_unchecked`); R12 ordering preserved (decompress → CRC → rkyv access); R25 alignment honored via `AlignedVec` copy in `decode_archive_payload`; R22 hot-path enforcement via try-then-try Assign-first discrimination (DC-3) with mandated source comment; R26 non-hot-path archives rejected with literal phrase `"non-hot-path archive payload (matched neither AssignPartition nor PartitionResult)"`; DC-4 send-side errors carry the mandated `"serialize: "` prefix. |
| **Acceptance signal — verified** | Round-trip identity `recv_frame(send_frame_v2(p)) == p` holds for both `AssignPartition` and `PartitionResult` across the full size battery (TASK-0359 T11/T13 cross-cut matrix, 8 round-trips per run). FLAG_ARCHIVED is emitted only with the feature ON AND the message on the hot path; default builds reject FLAG_ARCHIVED frames cleanly via the bincode decoder (UT-0357-09). |
| **Final test counts** | 887 lib baseline → **903 lib default** (+16) / **937 lib `--features zero-copy`** (+50). Both feature configs: clippy `--workspace --all-targets -- -D warnings` clean, `cargo fmt --check` clean. Release smoke `compute add 3 5 → 8` passes. |
| **Files touched** | `relativist-core/Cargo.toml`, `relativist-core/src/lib.rs`, `relativist-core/src/protocol/{config,error,frame,mod,zero_copy_tests}.rs`, `relativist-core/src/config.rs`, `relativist-core/src/net/{core,types}.rs`, `relativist-core/src/partition/{types,compact}.rs`, `relativist-core/src/merge/types.rs`. |
| **Resolved** | 2026-04-16 |
| **Status** | SHIPPED — all 7 action steps from the original D-002 plan executed and verified; bundle entered Stage 4 REVIEW. |

---

## Conventions

- **D-NNN** is the deferral ID. Use a fresh integer per row.
- Always link back to the spec, the original task, and the unblocking milestone/spec/feature ID from `V2-FEATURE-MATRIX.md`.
- Always state the *acceptance signal*: the observable behavior that proves the deferred scope is now complete.
- Do not let deferral rows accumulate indefinitely. If an item has been deferred more than 6 months past its unblocker shipping, escalate or formally drop it (move to Resolved with a rationale).
