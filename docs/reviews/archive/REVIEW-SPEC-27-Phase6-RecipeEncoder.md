# REVIEW: SPEC-27 Phase 6 mínimo — RecipeEncoder Trait

**Reviewed:** 2026-04-16
**Scope:** TASK-0340 (R24 + R25 only). R26/R27/R28 explicitly deferred to M7 — see `docs/DEFERRED-WORK.md` D-001.
**Verdict:** **APPROVE**

---

## What was reviewed

- `relativist-core/src/encoding/recipe.rs` (~210 LoC inc. 8 inline tests)
- `relativist-core/src/encoding/mod.rs` (added `pub mod recipe;` and 3 re-exports)

## Spec compliance

| Req | Status | Evidence |
|-----|--------|----------|
| R24 (trait shape) | ✅ | `RecipeEncoder: Encoder` with `type Recipe: Serialize + DeserializeOwned + Send + Sync` and the 3 required methods (`is_decomposable`, `make_recipes`, `generate_partition`) |
| R25 (not coupled to Codec) | ✅ | `Codec` supertrait unchanged in `traits.rs`; `EncoderRegistry` continues to store `Box<dyn Codec>` (777 → 797 tests, all 19 registry tests still green); explicit test `codec_does_not_require_recipe_encoder` |

## Code quality

- **Determinism**: `make_recipes` is purely arithmetic over `(input, num_workers)`. No randomness, no hidden state. Round-trip and equality test (`make_recipes_is_deterministic`) confirms.
- **Error handling**: `K=0` returns `EncodeError::InvalidInput`; invalid JSON also returns `InvalidInput`. No `unwrap()` in production paths.
- **Wire-readiness**: `MinimalRecipe` derives `Serialize + Deserialize + PartialEq + Eq + Clone + Debug`. Round-trips through `serde_json` in test `recipes_roundtrip_through_serde`.
- **Object safety not required**: `RecipeEncoder` has an associated type `Recipe`, so `Box<dyn RecipeEncoder>` is intentionally not object-safe. The future SPEC-25 integration (R28) will use a parallel typed registry or downcasting — documented in the module doc-comment.
- **Naming**: `MinimalRecipeEncoder` makes the demo intent explicit; the doc-comment states it is **not** registered in `default_registry()` and explains why.

## Tests added (8 inline)

1. `minimal_encoder_is_decomposable` — R2
2. `make_recipes_returns_k_for_various_k` — R3 + invariant (sum of `pairs` = `size`)
3. `make_recipes_is_deterministic` — R4
4. `make_recipes_rejects_zero_workers` — R5
5. `generate_partition_is_non_empty` — R6 (uses `count_live_agents() > 0`)
6. `recipes_roundtrip_through_serde` — R7
7. `codec_does_not_require_recipe_encoder` — R8 / R25 by construction
8. `make_recipes_rejects_invalid_json` — sanity / fuzz seed

## Test count

- Before: 789
- After: **797** (+8, exceeds the +4 acceptance bar in TASK-0340)

## Build / lint / fmt

- `cargo build --workspace`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo fmt --check`: clean

## Deferred scope (R26/R27/R28) — recorded

The deferral is documented in three independent locations so it cannot be silently lost:
1. `docs/DEFERRED-WORK.md` row D-001 (canonical tracker)
2. `docs/ROADMAP.md` §2.41 status note
3. `docs/V2-FEATURE-MATRIX.md` SPEC-27 row + M10 milestone row
4. `recipe.rs` module-level doc-comment (developer-facing)

Acceptance signal for the deferred scope (when M7 lands): at least one real
`RecipeEncoder` (e.g., `ep_annihilation`) ships a recipe end-to-end through
the registry: coordinator → wire → worker materializes its partition →
reduction proceeds → results decode correctly.

## Findings

**Must-Fix:** None.
**Should-Fix:** None.
**Nice-to-have (future):**
- Once R26 lands, `MinimalRecipeEncoder` may either be retired (replaced by real
  decomposable codecs) or moved into `#[cfg(test)]` to keep the demo as
  test-only scaffolding. Decision deferred to M7.

## Conclusion

Phase 6 mínimo correctly ships the trait definition and the non-coupling
guarantee with the `Codec` registry. Tests prove R24/R25 by construction
and exercise the demo end-to-end. The deferred scope (R26/R27/R28) is
tracked in a way that prevents it from being forgotten when SPEC-25 lands.

**APPROVE.**
