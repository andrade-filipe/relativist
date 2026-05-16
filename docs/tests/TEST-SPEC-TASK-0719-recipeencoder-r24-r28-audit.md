# TEST-SPEC-TASK-0719: Tests for TASK-0719 — RecipeEncoder generalization audit + AssignRecipe encoder-name field

**Task:** TASK-0719
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R24 (RecipeEncoder trait extension), R25 (fallback to centralized partition for non-decomposable codecs), R26 (refactor SPEC-25 generators), R27 (`AssignRecipe` carries encoder name), R28 (workers share registry, static registration)
**Test IDs (from SPEC-27 v3 §7.6):** T22 (built-in generators via RecipeEncoder match `compute_recipes()`), T23 (HornerCodec falls back to centralized partition; distributed result matches sequential).

---

## Scope

This task closes Phase 6 of SPEC-27 v3 §6. Work is mostly **audit + delta**: the trait already exists at `relativist-core/src/encoding/recipe.rs`. Deltas:
1. SPEC-25 built-in generators implement `RecipeEncoder` (or via adapter).
2. `AssignRecipe` carries `encoder_name: String`.
3. Worker dispatches recipes via registry lookup keyed on `encoder_name`.
4. T23: HornerCodec (NOT a RecipeEncoder) falls back to centralized partition (R25).

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0719-01 | unit (in-module) | none | `relativist-core/src/encoding/recipe.rs` | ~30 |
| UT-0719-02 | unit (in-module) | none | `relativist-core/src/protocol/messages.rs` (or wherever `AssignRecipe` lives) | ~25 |
| UT-0719-03 | unit (in-module) | none | `relativist-core/src/worker.rs` (or coordinator dispatch site) | ~30 |
| IT-0719-04 | integration | none | `relativist-core/tests/integration_horner_centralized.rs` | ~70 |

## Test floor delta (from TASK-0719 acceptance criteria)

- default: **+4** → ≥ 1886
- zero-copy: **+4** → ≥ 1930
- streaming-no-recycle: **+4** → ≥ 1877
- release: **+4** → ≥ 1828

---

## Unit Tests

### UT-0719-01: `recipe_encoder_builtin_ep_annihilation_matches_compute_recipes` (T22)

**Purpose:** R26 — `RecipeEncoder::make_recipes` for `ep_annihilation` produces the same `Vec<Recipe>` as the legacy `compute_recipes("ep_annihilation", ...)` call.

**Input:**
```rust
let workload_name = "ep_annihilation";
let input_json = br#"{"size": 100}"#;  // canonical SPEC-25 input shape
let num_workers = 4u32;

// Legacy path.
let legacy = compute_recipes(workload_name, input_json, num_workers).unwrap();

// New trait-based path.
let encoder = MinimalRecipeEncoder::for_workload(workload_name);  // or analogous constructor
let new = encoder.make_recipes(input_json, num_workers).unwrap();
```

**Expected output:**
```rust
assert_eq!(legacy.len(), new.len());
assert_eq!(legacy.len(), num_workers as usize);
// Recipe payloads MUST be element-wise equal (deterministic generation).
for (l, n) in legacy.iter().zip(new.iter()) {
    assert_eq!(l, n, "recipe content MUST match between legacy and trait-based paths");
}
```

**Edge cases:**
- (EC-1) `num_workers = 1` (degenerate single-worker grid) — both paths return one recipe spanning the whole problem.
- (EC-2) `num_workers > problem_size`: implementation-defined; both paths MUST return the same number of recipes (whatever the convention is).
- (EC-3) Other built-in generators (e.g., `dual_tree`, `con_dup`) — at least 2 additional workloads tested via the same pattern, parameterized.

---

### UT-0719-02: `assign_recipe_carries_encoder_name_through_wire_round_trip` (R27)

**Purpose:** R27 — `AssignRecipe` MUST include `encoder_name: String`. Wire round-trip MUST preserve the field.

**Input:**
```rust
let original = AssignRecipe {
    // ... existing fields populated from a known-good recipe ...
    encoder_name: "ep_annihilation".to_string(),
    // ... payload ...
};

// Serialize + deserialize via the existing serialization mechanism (rkyv / bincode / serde — match existing protocol).
let bytes = serialize(&original).unwrap();
let recovered: AssignRecipe = deserialize(&bytes).unwrap();
```

**Expected output:**
```rust
assert_eq!(recovered.encoder_name, "ep_annihilation");
assert_eq!(recovered, original);  // full equality
```

**Edge cases:**
- (EC-1) Empty encoder name (`""`) — serialize succeeds; semantic check (rejecting empty names at the registry level) lives elsewhere.
- (EC-2) Non-ASCII encoder name (`"hörner"`) — serialize succeeds; UTF-8 round-trips.
- (EC-3) Long name (>255 chars) — round-trips correctly (no truncation).
- (EC-4) Field MUST be deserializable from a wire payload that includes it AND from a payload that does NOT (if `#[serde(default)]` is used for backward compatibility — see TASK-0719 NOTE on PROTOCOL_VERSION bump).

---

### UT-0719-03: `worker_dispatches_recipe_via_registry_lookup` (R28)

**Purpose:** R28 — workers use `default_registry()` (or shared static registry) to look up the encoder by name.

**Input:** Simulate an `AssignRecipe` arriving at the worker; assert the worker code queries the registry with the carried `encoder_name`.

**Approach:** Mock or instrument the registry to record lookups, then dispatch a recipe.
```rust
let registry = default_registry();
let recipe = AssignRecipe {
    encoder_name: "ep_annihilation".to_string(),
    // ... payload ...
};

// Simulate worker-side dispatch.
let lookup_result = registry.get(&recipe.encoder_name);
assert!(lookup_result.is_some(), "registry MUST contain ep_annihilation");

// Dispatch produces a Partition (the worker side of generate_partition).
let partition = dispatch_recipe_to_worker(&registry, &recipe).unwrap();
assert!(partition.agents.len() > 0);
```

**Expected output:** Worker successfully dispatches via registry lookup; produces a non-empty partition.

**Edge cases:**
- (EC-1) Unknown encoder name (`"nonexistent"`) — worker returns a clean error (not panic).
- (EC-2) Wrong encoder name for the recipe payload (e.g., `dual_tree` payload sent to `ep_annihilation` encoder) — generation fails with `EncodeError::InvalidInput` (or analogous).
- (EC-3) Registry is `Send + Sync` (workers may be on threads): not asserted here, but TASK-0719 NOTE mentions static registration; verified by trait bounds at compile time.

---

## Integration Tests

### IT-0719-04: `horner_codec_centralized_fallback_matches_sequential` (T23)

**Purpose:** R25 — HornerCodec (NOT a RecipeEncoder) falls back to centralized partition (coordinator generates full net + partitions via SPEC-04, ships partitions to workers). Distributed result MUST match sequential `reduce_all` (G1 again, this time via the centralized-fallback path).

**Note:** This test partially overlaps IT-0715-08 (T13 in-process G1) but specifically asserts the **centralized fallback path** is exercised (no RecipeEncoder dispatch).

**Input:**
```rust
let codec = HornerCodec::new();
let input = br#"{"coeffs":[3,2,5,1],"x":2}"#;

// Sequential baseline.
let mut net_seq = codec.encode(input).unwrap();
reduce_all(&mut net_seq);
let seq_value = codec.decode(&net_seq).unwrap();

// Distributed via run_grid with W=2; HornerCodec is NOT a RecipeEncoder, so the
// coordinator MUST fall back to centralized partition (SPEC-04 R25 fallback).
let net_for_grid = codec.encode(input).unwrap();
let merged = run_grid(net_for_grid, 2, PartitionStrategy::RoundRobin);
let inproc_value = codec.decode(&merged).unwrap();
```

**Expected output:**
```rust
assert_eq!(seq_value, inproc_value);
// Decoded value: 35 (T7 canonical).
assert_eq!(seq_value["value"].as_str().unwrap(), "35");
```

**Edge cases:**
- (EC-1) `W = 4`, `W = 8` — same equality. Aligns with IT-0715-08 / T13 specifically for the centralized fallback path.
- (EC-2) Verify that the coordinator did NOT call `RecipeEncoder::make_recipes` for HornerCodec (which is not a RecipeEncoder). Implementation-dependent; assert via instrumentation if available, otherwise rely on the type system (HornerCodec doesn't implement RecipeEncoder, so the call wouldn't compile).
- (EC-3) Static check: `assert_not_recipe_encoder::<HornerCodec>()` — use a trait-bound helper that fails to compile if HornerCodec implements RecipeEncoder. Implemented via `static_assertions` crate or a hand-rolled negative-impl check.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `ep_annihilation` legacy vs trait-based recipes match | element-wise equal | UT-0719-01 |
| EC-002 | `num_workers = 1` (degenerate) | both paths return 1 recipe | UT-0719-01 EC-1 |
| EC-003 | Multiple workloads (`dual_tree`, `con_dup`) | parameterized matching | UT-0719-01 EC-3 |
| EC-004 | `AssignRecipe.encoder_name` round-trips | `recovered == original` | UT-0719-02 |
| EC-005 | Empty / unicode / long encoder names | round-trip cleanly | UT-0719-02 EC-1..3 |
| EC-006 | Worker registry lookup with valid name | returns Some(codec) | UT-0719-03 |
| EC-007 | Worker registry lookup with invalid name | clean error, no panic | UT-0719-03 EC-1 |
| EC-008 | HornerCodec via run_grid W=2 | matches sequential value | IT-0719-04 |
| EC-009 | HornerCodec via run_grid W=4, W=8 | matches sequential value | IT-0719-04 EC-1 |
| EC-010 | HornerCodec NOT a RecipeEncoder (compile-time check) | type-system enforces | IT-0719-04 EC-3 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T22 (built-in generators via RecipeEncoder match `compute_recipes()`) | UT-0719-01 |
| T23 (HornerCodec centralized fallback matches sequential) | IT-0719-04 |

(R27 wire-protocol field add and R28 worker dispatch are not directly numbered T-IDs in §7 but are covered by UT-0719-02 and UT-0719-03 respectively.)

## Dependencies Context

- `RecipeEncoder` trait at `recipe.rs` (existing).
- `MinimalRecipeEncoder` reference impl (existing).
- SPEC-25 `compute_recipes()` and built-in generators (existing).
- `AssignRecipe` message variant (existing; field add in this task).
- `default_registry()` from TASK-0716.
- `run_grid(net, num_workers, partition_strategy)` from `merge`.
- `HornerCodec` from TASK-0714/TASK-0715.

## Notes

- This task is the most audit-heavy of the bundle. If existing implementations already satisfy R24-R28 semantically, the task delivers tests + a single new `encoder_name` field on `AssignRecipe`.
- **PROTOCOL_VERSION bump:** if `AssignRecipe` serialization is positional (rkyv zero-copy or strict bincode), adding `encoder_name` requires a version bump. Coordinate with cicd agent in a separate task; this task ships the field. If serialization uses `#[serde(default)]`, the field add is non-breaking and no bump is required.
- The compile-time check that HornerCodec is NOT a RecipeEncoder (IT-0719-04 EC-3) can use `static_assertions::assert_not_impl_all!(HornerCodec: RecipeEncoder)` if `static_assertions` is acceptable as a dev-dependency. Otherwise, omit and rely on the runtime check that `dispatch_recipe_to_worker` returns an appropriate error path.
- IT-0719-04 may live in `relativist-core/tests/` OR `relativist-cli/tests/` depending on which crate exposes `run_grid` to integration-test consumers; per TASK-0719, the test-generator agent decides location — recommend `relativist-core/tests/integration_horner_centralized.rs`.
- This is the LAST TEST-SPEC of the D-015 bundle. After it lands and Stage 3+ (DEV/REVIEW/QA/REFACTOR) close, the bundle is shipped.
- Test floor delta: **+4** total (3 unit + 1 integration).
