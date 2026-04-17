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
| **Requirements deferred** | R13 (coordinator dispatches border redex via `interact_*`), R14 (coordinator uses the 6 SPEC-03 rules), R15 parts 1 and 2 (send port-update deltas to workers; update `BorderGraph` after resolution) |
| **Shipped instead** | R8, R9, R10, R11, R12, R15 part 3 (`add_border_states` primitive), R16, R17, R18, R19 — the `BorderGraph` pure data structure itself, in `relativist-core/src/merge/border_graph.rs`. |
| **Unblocker** | SPEC-19 §3.3 (item 2.26) — Delta-Only Protocol with Stateful Workers. The coordinator integration is the same work that stands up `run_grid_delta`, the `InitialPartition`/`RoundStart`/`RoundResult`/`FinalStateRequest`/`FinalStateResult` wire messages (R31-R36), and the `GridConfig.delta_mode` flag (R20). |
| **Why deferred** | R13-R15 parts 1-2 describe *what the coordinator must do when a border redex is detected*. The detection primitive (`BorderGraph::detect_border_redexes`) and the state-mutation primitives (`apply_deltas`, `remove_border`, `add_border_states`) all ship now. The *caller* — the coordinator's BSP loop — does not exist yet because the wire protocol extensions (R31-R36) and the stateful-worker lifecycle (R20-R30) are both item 2.26 scope. Shipping R13-R15 parts 1-2 now would mean writing speculative coordinator glue against a delta loop that has no test harness. |
| **What R13 needs** | Coordinator-side call path: when `border_graph.detect_border_redexes()` yields entries, dispatch each to the appropriate `interact_*` function (from `reduction/`) using agents materialized from the two workers' partitions. |
| **What R14 needs** | Wire up the 6-rule dispatch table (CON-CON, DUP-DUP, ERA-ERA, CON-DUP, CON-ERA, DUP-ERA) on the coordinator side, mirroring `interact_*` used in `reduce_all`. |
| **What R15 parts 1-2 needs** | After `interact_*` returns new connections, (1) package the port reconnections as `BorderDelta`s keyed to the affected workers, and (2) call `border_graph.remove_border(bid)` or `border_graph.apply_deltas(worker_id, ...)` to reflect the resolution. R15 part 3 (`add_border_states` for CON-DUP expansion) already ships — it's the *primitive*; the coordinator just needs to call it. |
| **Estimated effort** | ~200-300 LoC of coordinator glue inside `relativist-core/src/coordinator.rs` (or a new `coordinator/delta_loop.rs` module), plus matching test fixtures. Part of the ~1600 LoC envelope already budgeted for item 2.26. |
| **Acceptance signal** | An integration test in which a 2-worker grid converges on an input that requires at least one border-redex resolution: the coordinator detects the redex via `BorderGraph::detect_border_redexes`, invokes the right `interact_*` rule, sends `BorderDelta` patches to both workers, and the workers' next-round states reflect the resolution. The final `merge()` reconstructs the same output net that the v1 full-partition protocol produces on identical input. |
| **Files to revisit** | `relativist-core/src/coordinator.rs`, `relativist-core/src/protocol/messages.rs` (add `InitialPartition`, `RoundStart`, `RoundResult`, `FinalStateRequest`, `FinalStateResult` per R31-R32), `relativist-core/src/protocol/frame.rs` (discriminant 7-11), `relativist-core/src/merge/grid.rs` (new `run_grid_delta`), `relativist-core/src/config.rs` (`GridConfig.delta_mode: bool` per R20). |
| **Created** | 2026-04-17 (during Stage 6 SHIP of SPEC-19 §3.2) |
| **Status** | OPEN — waiting on item 2.26 |

**Action when item 2.26 starts:**
1. Open `relativist-core/src/merge/border_graph.rs` — all primitives are already defined, tested, and adversarially probed.
2. Create `TASK-04XX` (coordinator border-redex dispatch) under SPEC-19 §3.3 covering R13+R14+R15 parts 1-2 in a single ticket.
3. Add the 5 wire variants (R31-R32) to `Message` with discriminants 7-11; wire framing unchanged (piggybacks on SPEC-18 v2 frame).
4. Stand up `run_grid_delta` in `merge/grid.rs` gated on `GridConfig.delta_mode`.
5. Coordinator loop calls `detect_border_redexes` → `interact_*` → patch via `apply_deltas` / `remove_border` / `add_border_states`.
6. Add integration test demonstrating equivalence with v1 full-partition output on a workload with cross-partition redexes.
7. Close this row after acceptance signal is met.

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
