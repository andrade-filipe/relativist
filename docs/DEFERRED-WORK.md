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
