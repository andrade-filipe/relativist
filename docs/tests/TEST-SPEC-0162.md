# TEST-SPEC-0162: Define NetSummary and ReductionSummary structs

**Task:** TASK-0162
**Spec:** SPEC-12 R29, R30, R44, R44a, R44b, R45, R46, R47
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: NetSummary can be constructed with all fields

**Type:** Unit
**Input:**
```rust
let summary = NetSummary {
    agents: 1000, wires: 1500, redexes: 500,
    con: 400, dup: 350, era: 250,
    free_ports: 6, normal_form: false,
};
```
**Expected:** All fields are accessible and have the assigned values
**Verifies:** TASK-0162 AC -- struct definition with all 8 fields

### T2: NetSummary serializes to JSON

**Type:** Unit
**Input:**
```rust
let summary = NetSummary {
    agents: 0, wires: 0, redexes: 0,
    con: 0, dup: 0, era: 0,
    free_ports: 0, normal_form: true,
};
let json = serde_json::to_string(&summary).unwrap();
```
**Expected:** `json` is valid JSON containing `"agents":0`, `"normal_form":true`, `"free_ports":0`
**Verifies:** TASK-0162 AC -- NetSummary derives `serde::Serialize`

### T3: NetSummary derives Debug and Clone

**Type:** Compile-time verification
**Input:**
```rust
let s = NetSummary { agents: 1, wires: 0, redexes: 0, con: 1, dup: 0, era: 0, free_ports: 0, normal_form: true };
let s2 = s.clone();
let _ = format!("{:?}", s2);
```
**Expected:** Compiles without error
**Verifies:** TASK-0162 AC -- required derive traits

### T4: ReductionSummary with all grid fields as None serializes without them

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
**Expected:** JSON contains `"total_interactions":90`, `"duration_secs":1.234`. If grid fields are `Option` with `skip_serializing_if`, they are absent from the JSON.
**Verifies:** TASK-0162 AC -- local reduction summary without grid fields

### T5: ReductionSummary normal_form field reflects reduction outcome

**Type:** Unit
**Input:** Create a `ReductionSummary` with `normal_form: false` and `redexes_after: 5`
**Expected:** `summary.normal_form` is `false`
**Verifies:** R47 -- summary reports whether output net reached Normal Form

### T6: ReductionSummary MIPS field is correctly typed as f64

**Type:** Unit
**Input:**
```rust
let summary = ReductionSummary {
    ..default_fields(),
    mips: 1_000_000.0 / 1.0 / 1_000_000.0, // 1.0 MIPS
};
```
**Expected:** `summary.mips` is approximately `1.0`
**Verifies:** MIPS = total_interactions / duration_secs / 1_000_000 as f64

---

## Edge Cases

### E1: NetSummary normal_form is true when redexes is 0

**Verify:** Constructing `NetSummary { redexes: 0, normal_form: true, .. }` is consistent. The computation function (TASK-0169) sets `normal_form = (redexes == 0)`, but the struct itself does not enforce this invariant.
**Why:** The struct is a plain data holder; the invariant is enforced by the `net_summary` computation.

### E2: ReductionSummary duration_secs can be zero

**Verify:** A `ReductionSummary` with `duration_secs: 0.0` and `mips: f64::INFINITY` (or `f64::NAN`) does not cause serialization errors.
**Why:** An instant reduction (zero duration) is theoretically possible for an empty net. The formatter must handle this gracefully.

### E3: ReductionSummary serialization produces valid JSON for all Option states

**Verify:** Construct a `ReductionSummary` with all `Option` fields set to `Some(...)`, serialize, and verify valid JSON. Then construct with all `None`, serialize, and verify valid JSON.
**Why:** Both configurations must produce parseable output.
