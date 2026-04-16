# TEST-SPEC-0340: RecipeEncoder trait + MinimalRecipeEncoder demo

**Task:** TASK-0340
**Spec:** SPEC-27 R24, R25 (Phase 6 mínimo)
**Generated:** 2026-04-16
**Deferred:** R26, R27, R28 → see `docs/DEFERRED-WORK.md` D-001

---

## Scope note

Phase 6 mínimo ships only the trait definition + a `MinimalRecipeEncoder`
demo to exercise the trait. There is no integration with any wire message,
coordinator, or worker yet — those land when SPEC-25 ships (M7).

Tests below verify R24 (trait shape) and R25 (non-coupling with `Codec`
registry) by construction. The demo encoder is deliberately trivial: it
exists only to prove the trait can be implemented and called end-to-end.

---

## R1: trait compiles with required associated type and methods

Compile-only check — if the file builds, the trait shape is correct.
Implicit in `cargo build --workspace`. No explicit test needed beyond
the demo implementation existing.

## R2: `MinimalRecipeEncoder::is_decomposable()` returns true

```rust
let enc = MinimalRecipeEncoder::new();
assert!(enc.is_decomposable());
```

## R3: `make_recipes(input, K)` returns exactly K recipes

```rust
let enc = MinimalRecipeEncoder::new();
let input = br#"{"size":12}"#;
for k in [1u32, 2, 4, 8] {
    let recipes = enc.make_recipes(input, k).expect("make_recipes");
    assert_eq!(recipes.len() as u32, k, "expected {} recipes", k);
}
```

## R4: `make_recipes` is deterministic for the same (input, K)

```rust
let enc = MinimalRecipeEncoder::new();
let input = br#"{"size":12}"#;
let a = enc.make_recipes(input, 4).unwrap();
let b = enc.make_recipes(input, 4).unwrap();
assert_eq!(a, b, "recipes must be deterministic");
```

## R5: `make_recipes` rejects `num_workers == 0`

```rust
let enc = MinimalRecipeEncoder::new();
let res = enc.make_recipes(br#"{"size":12}"#, 0);
assert!(res.is_err(), "K=0 must be rejected as a logic error");
```

## R6: `generate_partition(&recipe)` returns a non-empty partition

```rust
let enc = MinimalRecipeEncoder::new();
let recipes = enc.make_recipes(br#"{"size":12}"#, 3).unwrap();
for r in &recipes {
    let p = enc.generate_partition(r).expect("generate_partition");
    assert!(!p.is_empty(), "each recipe must produce a non-empty Partition");
}
```

(`Partition::is_empty()` is the existing helper on `crate::partition::Partition`;
if not present, substitute with checking `agents.len() > 0` or similar.)

## R7: recipes round-trip through serde

This proves R24's wire-readiness requirement (recipes must be `Serialize +
DeserializeOwned`).

```rust
let enc = MinimalRecipeEncoder::new();
let recipes = enc.make_recipes(br#"{"size":12}"#, 2).unwrap();
let bytes = serde_json::to_vec(&recipes[0]).unwrap();
let back: <MinimalRecipeEncoder as RecipeEncoder>::Recipe =
    serde_json::from_slice(&bytes).unwrap();
assert_eq!(back, recipes[0]);
```

## R8: `RecipeEncoder` is NOT a supertrait of `Codec` (R25 by construction)

This is a compile-time check, not a runtime test. It is verified by the
fact that `EncoderRegistry` stores `Box<dyn Codec>` (no `RecipeEncoder`
bound) and the `default_registry()` continues to work unchanged. No new
test is required — the existing 19 registry tests (TEST-SPEC-0336 +
TEST-SPEC-0337) already exercise this. We add one explicit assertion:

```rust
// R8: trait separation — codec registry does not require RecipeEncoder.
// If this file ever changed Codec to require RecipeEncoder, the registry
// tests in encoding/registry.rs would fail to compile.
let _: Box<dyn crate::encoding::traits::Codec> =
    Box::new(crate::encoding::codec_church::ChurchArithmeticCodec::add());
```

## Acceptance Criteria

1. `cargo test --workspace` count: 789 → **793+** (≥ +4 new tests covering R2/R3/R6/R7 at minimum; R4/R5/R8 add coverage but may be folded).
2. `cargo build --workspace` succeeds (proves R1).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
5. No change to existing 19 registry tests (proves R8 / R25 by construction).

## Out of Scope (deferred to M7)

- No wire message tests (R27 needs `AssignRecipe` from SPEC-25).
- No coordinator/worker integration tests (R28 needs SPEC-25 to be implemented).
- No production-quality recipe encoders — `MinimalRecipeEncoder` is a stub.
- See `docs/DEFERRED-WORK.md` D-001 for the full unblock checklist.
