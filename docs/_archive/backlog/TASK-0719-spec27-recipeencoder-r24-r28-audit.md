# TASK-0719 — SPEC-27 v3 R24-R28: RecipeEncoder generalization audit + AssignRecipe encoder-name field

**Spec:** SPEC-27 v3
**Requirements:** R24 (RecipeEncoder trait extension), R25 (fallback to centralized partition for non-decomposable codecs), R26 (refactor SPEC-25 generators), R27 (`AssignRecipe` carries encoder name), R28 (workers share registry, static registration)
**Priority:** P1
**Status:** TODO
**Depends on:** TASK-0716 (default_registry stable; HornerCodec uses R25 fallback)
**Blocked by:** none
**Estimated complexity:** M (~60 LoC production + ~70 LoC tests; mostly audit + small additions)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 R24-R28 generalize the SPEC-25 `GenerationRecipe` infrastructure
behind a `RecipeEncoder` trait so that user-defined encoders can supply their
own recipe types for distributed generation. The trait already exists at
`relativist-core/src/encoding/recipe.rs` (`pub trait RecipeEncoder: Encoder`
with associated type `Recipe`); a `MinimalRecipeEncoder` reference impl is also
shipped. SPEC-25's built-in generators (`compute_recipes()`, etc.) need to
implement `RecipeEncoder` too.

R25 — Codecs that do NOT implement `RecipeEncoder` (e.g., HornerCodec, per Q4)
fall back to centralized generation: the coordinator generates the full net,
partitions via SPEC-04, and ships partitions. This task verifies that the
fallback path exists and is exercised by an integration test (T23).

R27 — The wire-protocol `AssignRecipe` message variant (SPEC-25 R15-R17, in
`relativist-core::protocol`) MUST carry the encoder name so the worker can look
up the correct `RecipeEncoder` implementation in the registry.

R28 — Workers MUST have access to the same `EncoderRegistry` as the
coordinator. Static registration only (no plugin loading; NG4). Verify that
the worker code currently uses a shared `default_registry()` invocation (or
equivalent compile-time-built map).

This task is largely **audit + delta**: the trait exists, `MinimalRecipeEncoder`
exists, SPEC-25 already wires recipes. The v3 deltas are:
1. Verify `AssignRecipe` includes the encoder name field; add it if missing.
2. Verify SPEC-25 built-in generators implement `RecipeEncoder` (or refactor).
3. Add T22 (built-in generators via `RecipeEncoder` produce same result as
   `compute_recipes()`).
4. Add T23 (HornerCodec falls back to centralized partition; distributed
   result matches sequential).

## Acceptance Criteria

- [ ] Audit: `RecipeEncoder` trait at `recipe.rs` matches SPEC-27 v3 R24 signature exactly (`is_decomposable`, `make_recipes`, `generate_partition`).
- [ ] Audit: existing SPEC-25 built-in generators (e.g., `ep_annihilation`, `dual_tree`) implement `RecipeEncoder`. If they only use the legacy `compute_recipes()` API, add an adapter `impl RecipeEncoder for <BuiltinName>` (or a single wrapper struct that delegates to `compute_recipes`).
- [ ] `AssignRecipe` message variant (in `relativist-core::protocol`, SPEC-25 R15-R17) includes a `pub encoder_name: String` field. If absent, add it (PROTOCOL_VERSION bump as needed; coordinate with cicd agent in a follow-up if a bump is required — this task scopes the protocol delta but does NOT bump PROTOCOL_VERSION beyond what is needed).
- [ ] Worker code (in `relativist-core::worker` or equivalent) reads `encoder_name` and looks up the correct `RecipeEncoder` from `default_registry()`. If only one built-in encoder is hardcoded today, add the lookup; otherwise audit-only.
- [ ] HornerCodec is NOT a `RecipeEncoder`; the centralized fallback (R25) is exercised by an integration test (T23).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/encoding/recipe.rs` | modify | Audit; add `RecipeEncoder` impls for SPEC-25 built-in generators if missing. ~30 LoC. |
| `relativist-core/src/protocol/messages.rs` (or wherever `AssignRecipe` lives) | modify | Add `pub encoder_name: String` field to `AssignRecipe`; update wire format if needed (coordinate PROTOCOL_VERSION bump in a follow-up task if necessary — this task scopes the field add). ~10 LoC + serde derive update. |
| `relativist-core/src/worker.rs` (or coordinator-side dispatch) | modify | Read `encoder_name` and dispatch via `default_registry().get(name)`. ~10 LoC if missing. |
| Integration test file (`tests/integration_horner_centralized.rs`) | **CREATE.** | T23: HornerCodec via `run_grid` with `W=2`; assert distributed result matches sequential `reduce_all`. ~70 LoC. |

## Key Types / Signatures

(Existing — verify only.)

```rust
pub trait RecipeEncoder: Encoder {
    type Recipe: Serialize + DeserializeOwned + Send + Sync;
    fn is_decomposable(&self) -> bool;
    fn make_recipes(&self, input: &[u8], num_workers: u32) -> Result<Vec<Self::Recipe>, EncodeError>;
    fn generate_partition(&self, recipe: &Self::Recipe) -> Result<Partition, EncodeError>;
}
```

`AssignRecipe` field add:

```rust
// SPEC-27 v3 R27
pub struct AssignRecipe {
    // ... existing fields ...
    pub encoder_name: String, // NEW: identifies the RecipeEncoder in the registry
    // ... existing recipe payload ...
}
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.6 T22, T23:

- `recipe_encoder_builtin_ep_annihilation_matches_compute_recipes` — `RecipeEncoder::make_recipes` for `ep_annihilation` produces the same `Vec<Recipe>` as the legacy `compute_recipes("ep_annihilation", ...)` call (T22).
- `horner_codec_centralized_fallback_matches_sequential` — encode `[3,2,5,1] @ 2` via `HornerCodec`; reduce sequentially; reduce via `run_grid(W=2)` with SPEC-04 default partition; assert the decoded values are equal (T23).
- `assign_recipe_carries_encoder_name_through_wire_round_trip` — serialize `AssignRecipe { encoder_name: "ep_annihilation", ... }`, deserialize, assert field round-trips.
- `worker_dispatches_recipe_via_registry_lookup` — unit test that simulates an `AssignRecipe` arriving at the worker; assert the registry is queried with the carried encoder name.

## Dependencies Context

- `RecipeEncoder` trait at `relativist-core/src/encoding/recipe.rs` (existing).
- `MinimalRecipeEncoder` reference impl (existing).
- SPEC-25 `compute_recipes` and built-in generators (existing).
- `AssignRecipe` message variant in `relativist-core::protocol` (existing; field add may require new protocol version coordination).
- `default_registry()` from TASK-0716.
- `run_grid(net, num_workers, partition_strategy)` from `relativist-core::merge`.

## Notes

- This task is the most audit-heavy of the bundle. If the existing
  `MinimalRecipeEncoder` already covers SPEC-25's built-in generators
  semantically (e.g., one `MinimalRecipeEncoder` per workload name), the trait
  audit is satisfied and the only delta is the `encoder_name` field add. If a
  Stage 4 review uncovers a structural mismatch (e.g., the existing impl is too
  narrow), file a follow-up task and split this work into two TASKs.
- PROTOCOL_VERSION bump: SPEC-25 R15-R17 may already have field-level optionality
  via serde defaults; if so, adding `encoder_name: String` with
  `#[serde(default)]` is non-breaking. If serialization is positional (rkyv
  zero-copy or bincode strict), a version bump is required — coordinate with
  the cicd agent in a separate task. This task ships the field; the version
  bump scope is documented but NOT executed.
- T23 integration test belongs to `relativist-core/tests/` (or
  `relativist-cli/tests/` if the test exercises the CLI surface). The
  test-generator agent (Stage 2) decides the location.
- This task closes Phase 6 of SPEC-27 v3 §6 ("RecipeEncoder"). It is the LAST
  TASK in the bundle; after it lands, the SDD pipeline advances to Stage 2
  (test-generator) for the entire bundle.
