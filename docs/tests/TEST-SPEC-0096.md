# TEST-SPEC-0096: Add protocol crate dependencies to Cargo.toml

**Task:** TASK-0096
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: cargo check succeeds

**Type:** Build verification
**Input:** Run `cargo check` after adding dependencies
**Expected:** Compilation succeeds with zero errors
**Verifies:** All dependencies resolve correctly

### T2: cargo build succeeds

**Type:** Build verification
**Input:** Run `cargo build` after adding dependencies
**Expected:** Build completes without errors
**Verifies:** Dependencies are compatible and link correctly

### T3: tokio features are complete

**Type:** Source verification
**Input:** Read `Cargo.toml` and check tokio dependency
**Expected:** tokio includes features: `rt-multi-thread`, `net`, `io-util`, `time`, `macros`
**Verifies:** R20 -- async runtime with all needed features

### T4: All required crates present

**Type:** Source verification
**Input:** Read `Cargo.toml` `[dependencies]` section
**Expected:** Contains entries for: `tokio`, `bincode`, `crc32fast`, `serde` (with `derive`), `futures`, `tracing`
**Verifies:** R4, R10, R11, R20, R38 -- all protocol-related dependencies

---

## Edge Cases

### E1: No duplicate dependency entries

**Verify:** Each dependency appears exactly once in `Cargo.toml`. No conflicting version specifications.
**Why:** Duplicate entries cause cargo build warnings or errors.

### E2: serde derive feature is enabled

**Verify:** `serde` dependency includes `features = ["derive"]`.
**How:** Check Cargo.toml for `serde = { version = "1", features = ["derive"] }` or equivalent.
**Why:** Without `derive`, `#[derive(Serialize, Deserialize)]` on Message will fail to compile.
