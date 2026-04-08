# TEST-SPEC-0161: Define FileIoError, NetFormat, and InspectOutputFormat types

**Task:** TASK-0161
**Spec:** SPEC-12 R48, R49, R50, R52
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: detect_format recognizes .bin extension

**Type:** Unit
**Input:** `detect_format(Path::new("net.bin"))`
**Expected:** Returns `Some(NetFormat::Bin)`
**Verifies:** R48 -- .bin maps to binary format

### T2: detect_format recognizes .ic extension

**Type:** Unit
**Input:** `detect_format(Path::new("net.ic"))`
**Expected:** Returns `Some(NetFormat::Ic)`
**Verifies:** R48 -- .ic maps to text DSL format

### T3: detect_format recognizes .json extension

**Type:** Unit
**Input:** `detect_format(Path::new("net.json"))`
**Expected:** Returns `Some(NetFormat::Json)`
**Verifies:** R48 -- .json maps to JSON format

### T4: detect_format returns None for unknown extension

**Type:** Unit
**Input:** `detect_format(Path::new("net.xyz"))`
**Expected:** Returns `None`
**Verifies:** R49 -- unrecognized extension yields None (or error at caller)

### T5: detect_format returns None for no extension

**Type:** Unit
**Input:** `detect_format(Path::new("net"))`
**Expected:** Returns `None`
**Verifies:** R49 -- missing extension yields None

### T6: detect_format with nested path extracts correct extension

**Type:** Unit
**Input:** `detect_format(Path::new("/some/nested/path/network.ic"))`
**Expected:** Returns `Some(NetFormat::Ic)`
**Verifies:** Extension detection works regardless of directory depth

### T7: NetFormat derives Debug, Clone, Copy

**Type:** Compile-time verification
**Input:**
```rust
let f = NetFormat::Bin;
let f2 = f;          // Copy
let f3 = f.clone();  // Clone
let _ = format!("{:?}", f); // Debug
```
**Expected:** Compiles without error
**Verifies:** TASK-0161 AC -- required derive traits

### T8: NetFormat derives clap::ValueEnum

**Type:** Compile-time verification
**Input:** Use `NetFormat::value_variants()` from the `clap::ValueEnum` trait
**Expected:** Returns a slice with 3 variants: `[Bin, Ic, Json]`
**Verifies:** TASK-0161 AC -- clap integration for CLI argument parsing

### T9: NetSummary derives Serialize

**Type:** Unit
**Input:**
```rust
let summary = NetSummary {
    agents: 10, wires: 15, redexes: 5,
    con: 4, dup: 3, era: 3,
    free_ports: 2, normal_form: false,
};
let json = serde_json::to_string(&summary).unwrap();
```
**Expected:** `json` contains `"agents":10`, `"normal_form":false`
**Verifies:** TASK-0161 AC / TASK-0162 AC -- NetSummary is serializable

### T10: ReductionSummary derives Serialize

**Type:** Unit
**Input:**
```rust
let summary = ReductionSummary {
    agents_before: 100, agents_after: 10,
    redexes_before: 50, redexes_after: 0,
    normal_form: true,
    total_interactions: 90,
    duration_secs: 1.234,
    mips: 0.073,
};
let json = serde_json::to_string(&summary).unwrap();
```
**Expected:** `json` is valid JSON containing `"normal_form":true` and `"mips":0.073`
**Verifies:** ReductionSummary is serializable

---

## Edge Cases

### E1: detect_format is case-sensitive

**Verify:** `detect_format(Path::new("net.BIN"))` returns `None` (not `Some(Bin)`).
**Why:** File extensions on case-sensitive file systems are exact matches. The implementation uses `and_then(|e| e.to_str())` which preserves case.

### E2: detect_format with double extension

**Verify:** `detect_format(Path::new("net.tar.bin"))` returns `Some(NetFormat::Bin)` because `Path::extension()` returns only the last extension.
**Why:** Documents that the parser handles only the final extension segment.

### E3: NetFormat PartialEq works correctly

**Verify:** `NetFormat::Bin == NetFormat::Bin` is `true`, `NetFormat::Bin == NetFormat::Ic` is `false`.
**Why:** PartialEq is derived and used by test assertions and format matching.
