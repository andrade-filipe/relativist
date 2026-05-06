# TEST-SPEC-TASK-0716: Tests for TASK-0716 — default_registry — drop `lambda`, add `horner`

**Task:** TASK-0716
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R19 (default_registry contents — 5 v1 codecs, no `"lambda"`), R20 (duplicate-name rejection)
**Test IDs (from SPEC-27 v3 §7.4):** T14 (default_registry contents), T15 (duplicate registration fails), T16 (unknown encoder returns None — including `lambda`).

---

## Scope

This task swaps the `default_registry()` registration: removes `LambdaCodec::new()` registration, adds `HornerCodec::new()` registration. `LambdaCodec` itself stays in the codebase (per §5.1 Future Work) but is NOT in the default registry.

R20 (duplicate-name rejection) is already shipped; this task ensures the post-v3 layout is properly tested.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0716-01 | unit (in-module) | none | `relativist-core/src/encoding/registry.rs` | ~25 |
| UT-0716-02 | unit (in-module) | none | same | ~15 |
| UT-0716-03 | unit (in-module) | none | same | ~15 |
| UT-0716-04 | unit (in-module) | none | same | ~20 |
| UT-0716-05 | unit (in-module) | none | same | ~20 |

## Test floor delta (from TASK-0716 acceptance criteria)

- default: **+5** → ≥ 1872
- zero-copy: **+5** → ≥ 1916
- streaming-no-recycle: **+5** → ≥ 1863
- release: **+5** → ≥ 1814

(Note: pre-v3 tests that asserted `lambda` was present in `default_registry` MUST be **inverted** — those tests are NOT counted as new since they replace existing lines. The +5 above is net new tests; if any existing test was inverted in place, count it as 0 net delta. Stage 4 reviewer confirms the precise delta.)

---

## Unit Tests

### UT-0716-01: `default_registry_contains_5_v3_codecs` (T14)

**Purpose:** R19 — `default_registry()` contains exactly 5 codecs in canonical order: `church_add`, `church_mul`, `church_exp`, `church_sum_of_squares`, `horner`.

**Input:**
```rust
let r = default_registry();
let entries: Vec<(&str, &str)> = r.list();
let names: Vec<&str> = entries.iter().map(|(n, _)| *n).collect();
```

**Expected output:**
```rust
assert_eq!(entries.len(), 5);
assert_eq!(
    names,
    vec!["church_add", "church_mul", "church_exp", "church_sum_of_squares", "horner"]
);
```

**Edge cases:**
- (EC-1) Order MAY be implementation-defined; if so, weaken to `HashSet` comparison. But TASK-0716 acceptance lists explicit order — keep `assert_eq!` on the `Vec`.
- (EC-2) Each entry has a non-empty description (descriptions verified in TASK-0718's `encoders list` output test).

---

### UT-0716-02: `default_registry_excludes_lambda` (T16 — lambda case)

**Purpose:** R19 — `"lambda"` MUST NOT be in `default_registry()`. Closes the v3 swap.

**Input:**
```rust
let r = default_registry();
let lambda = r.get("lambda");
```

**Expected output:**
```rust
assert!(lambda.is_none());
```

**Edge cases:**
- (EC-1) `LambdaCodec` is still constructable via `relativist_core::encoding::codec_lambda::LambdaCodec` (user-side opt-in registration); only the **default** registry excludes it.
- (EC-2) Calling `r.list()` MUST NOT include "lambda" anywhere in the list.

---

### UT-0716-03: `default_registry_unknown_returns_none` (T16 — generic case)

**Purpose:** R20 / R17 — unknown name returns `None`.

**Input:**
```rust
let r = default_registry();
let unknown = r.get("nonexistent");
let typo = r.get("HORNER");  // case-sensitivity
let blank = r.get("");
```

**Expected output:**
```rust
assert!(unknown.is_none());
assert!(typo.is_none());  // assumes case-sensitive lookup; verify against R17 contract
assert!(blank.is_none());
```

**Edge cases:**
- (EC-1) Case-sensitivity: codec names are case-sensitive per R17 (no normalization); `HORNER` MUST NOT match `horner`.
- (EC-2) Unicode / whitespace / non-ASCII names MUST also return None; the registry only knows the registered names.

---

### UT-0716-04: `register_horner_twice_returns_duplicate_name_error` (T15)

**Purpose:** R20 — re-registering an existing name returns `RegistryError::DuplicateName`.

**Input:**
```rust
let mut r = default_registry();
let result = r.register(Box::new(HornerCodec::new()));
```

**Expected output:**
```rust
match result {
    Err(RegistryError::DuplicateName(name)) => assert_eq!(name, "horner"),
    other => panic!("expected DuplicateName(\"horner\"), got {:?}", other),
}
```

**Edge cases:**
- (EC-1) Pre-existing `register_and_get_round_trip` test (which uses `LambdaCodec` for registry mechanics testing) MUST still pass — TASK-0716 keeps the LambdaCodec module available for opt-in testing.
- (EC-2) Re-registering `church_add` (or any other v3 codec) MUST also return `DuplicateName`.

---

### UT-0716-05: `default_registry_horner_description_matches_r22`

**Purpose:** R22 — listed description for `"horner"` matches the spec example output.

**Input:**
```rust
let r = default_registry();
let entry = r.get("horner").expect("horner must be registered post-v3");
let desc = entry.description();
```

**Expected output:** `desc == "Polynomial evaluation via Horner's method"` (or close paraphrase per TASK-0715 acceptance — the exact wording is editorial; this test asserts the canonical R22 phrasing).

**Edge cases:**
- (EC-1) Description is non-empty.
- (EC-2) Description does NOT mention "lambda" or "Mackie/Pinto" (those belong to LambdaCodec; cross-codec contamination is a regression).
- (EC-3) Description fits on one terminal line (no embedded newlines) — required for `encoders list` formatting (TASK-0718).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `default_registry()` has exactly 5 entries | `list().len() == 5` | UT-0716-01 |
| EC-002 | Names in canonical R19 order | matches `["church_add", ..., "horner"]` | UT-0716-01 |
| EC-003 | `get("lambda")` returns None | post-v3 swap | UT-0716-02 |
| EC-004 | `LambdaCodec` still constructable user-side | type still accessible | UT-0716-02 EC-1 |
| EC-005 | `get("nonexistent")` returns None | generic unknown | UT-0716-03 |
| EC-006 | Case-sensitive lookup | `get("HORNER")` is None | UT-0716-03 |
| EC-007 | Re-register `horner` returns DuplicateName | R20 | UT-0716-04 |
| EC-008 | Re-register `church_add` returns DuplicateName | R20 | UT-0716-04 EC-2 |
| EC-009 | `horner` description matches R22 example | exact or paraphrase | UT-0716-05 |
| EC-010 | Description has no newlines | terminal-line formatting | UT-0716-05 EC-3 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T14 (default_registry contains the 5 v1 codecs) | UT-0716-01 + UT-0716-05 |
| T15 (duplicate registration fails) | UT-0716-04 |
| T16 (unknown encoder returns None — `lambda`, `nonexistent`) | UT-0716-02 + UT-0716-03 |

## Dependencies Context

- `HornerCodec` from TASK-0715 (must implement `Codec`).
- `ChurchArithmeticCodec`, `ChurchOp`, `EncoderRegistry`, `RegistryError` (existing).
- `LambdaCodec` retained in codebase (`encoding::codec_lambda::LambdaCodec`).

## Notes

- This task is small (~5 unit tests); the bulk of TASK-0716's work is the production-side code change (`r.register(LambdaCodec::new())` → `r.register(HornerCodec::new())`).
- Pre-existing tests that previously asserted `default_registry().get("lambda").is_some()` MUST be **inverted in place** (now assert `is_none()`); those inversions do NOT count toward the +5 new test delta. Stage 4 reviewer agent confirms the precise delta.
- The order of registration in `default_registry()` is editorial but SHOULD match R19 bullet order to keep `encoders list` (TASK-0718) output stable for documentation/screenshot purposes.
- Test floor delta: **+5** (after subtracting any in-place inversions from the count).
