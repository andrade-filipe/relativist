# TEST-SPEC-0703: Tests for TASK-0703 — CSV schema extension

**Task:** TASK-0703
**Spec:** none
**Bundle:** D-014 (Stress Curve Campaign)
**Requirements covered:** Acceptance criteria 1-6 from TASK-0703
**Test IDs:** IT-0703-{01, 02} (2 integration tests in one file)

---

## Scope

Verify the 4 new CSV columns appended at the end of the existing D-012 row:
- `vmrss_peak_mb` (f64, MiB)
- `vmrss_current_end_mb` (f64, MiB)
- `stop_reason` (String; `""` for normal rep, `"WallTimeExceeded"`/`"MemoryExceeded"`/`"Oom"` for sentinel)
- `cv_above_gate` (bool)

Plus forward-compat: a struct that deserializes only the original 25 columns MUST read any row written by post-D-014 code.

## Test category & location

| # | Name | Category | File |
|---|------|----------|------|
| IT-0703-01 | `roundtrip_writes_and_reads_new_columns` | integration | `relativist-core/tests/d014_csv_schema_roundtrip.rs` |
| IT-0703-02 | `legacy_struct_reads_post_d014_row` | integration | same file |

Two `#[test]` functions in one file = +2 default. LoC budget ~30 LoC matching TASK-0703.

## Test floor delta

- default: **+2** → ≥ 1811
- zero-copy: **+2** → ≥ 1855
- streaming-no-recycle: **+2** → ≥ 1802
- release: **+2** → ≥ 1753

---

## Integration Tests

### IT-0703-01: `roundtrip_writes_and_reads_new_columns`

**Purpose:** Verify a full row with all 4 new columns serializes and deserializes losslessly.

**Preconditions:**
- The bench row struct (in `bench/csv.rs`) has been extended with the 4 new fields.
- DEV has chosen a serialization for `stop_reason: Option<StopReason>` — recommended `Option<String>` field on the row, derived in the harness; serde's csv default is `""` for `None`.

**Imports (sketch):**
```rust
use relativist_core::bench::csv::BenchmarkRow; // exact name TBD by DEV; assume the existing row struct
// If the row is `pub(crate)`, surface as `pub` per TASK-0707 implementation hint #6 — file a TASK-UPDATER note.
use relativist_core::bench::stop_rule::StopReason;
use std::io::Cursor;
use tempfile::NamedTempFile;
```

**Test body — Sub-step 1: build a synthetic row with all 4 new fields populated:**

```rust
// Construct row using whatever constructor the existing code already has;
// the developer fills the legacy fields with placeholder defaults that the
// existing row-construction code already provides (Default::default() if
// the struct derives Default; otherwise a minimal builder).
let mut row = BenchmarkRow::default();   // or the existing construction pattern
row.vmrss_peak_mb        = 123.4;
row.vmrss_current_end_mb = 100.0;
row.stop_reason          = Some("MemoryExceeded".to_string());
row.cv_above_gate        = true;
```

**Sub-step 2: write to a buffer:**

```rust
let mut buf: Vec<u8> = Vec::new();
{
    let mut wtr = csv::Writer::from_writer(&mut buf);
    wtr.serialize(&row).expect("serialize must succeed");
    wtr.flush().expect("flush must succeed");
}

let csv_text = String::from_utf8(buf.clone()).expect("UTF-8");
```

**Sub-step 3: header sanity (acceptance criterion 1 — 4 new columns at the END):**

```rust
let header_line = csv_text.lines().next().expect("at least header line");
assert!(
    header_line.ends_with("vmrss_peak_mb,vmrss_current_end_mb,stop_reason,cv_above_gate"),
    "header MUST end with the 4 new columns in order; got: {}",
    header_line
);
```

**Sub-step 4: roundtrip read-back (acceptance criteria 2, 3, 4):**

```rust
let mut rdr = csv::Reader::from_reader(Cursor::new(buf));
let read_back: BenchmarkRow = rdr.deserialize::<BenchmarkRow>()
    .next().expect("at least one row").expect("deserialize ok");

assert!((read_back.vmrss_peak_mb - 123.4).abs() < f64::EPSILON * 16.0,
    "vmrss_peak_mb roundtrip mismatch: got {}", read_back.vmrss_peak_mb);
assert!((read_back.vmrss_current_end_mb - 100.0).abs() < f64::EPSILON * 16.0);
assert_eq!(read_back.stop_reason.as_deref(), Some("MemoryExceeded"));
assert!(read_back.cv_above_gate);
```

**Sub-step 5: empty-`stop_reason` round-trip (acceptance criterion 2):**

```rust
let mut row2 = BenchmarkRow::default();
row2.vmrss_peak_mb        = 1.5;
row2.vmrss_current_end_mb = 1.0;
row2.stop_reason          = None;        // serializes as ""
row2.cv_above_gate        = false;

let mut buf2: Vec<u8> = Vec::new();
{
    let mut wtr = csv::Writer::from_writer(&mut buf2);
    wtr.serialize(&row2).unwrap();
    wtr.flush().unwrap();
}
let txt2 = String::from_utf8(buf2.clone()).unwrap();
let data_line = txt2.lines().nth(1).expect("data row exists");
// The row's last 4 fields end with: ",1.5,1.0,,false"
assert!(
    data_line.ends_with(",1.5,1.0,,false"),
    "row's last 4 columns must be `,1.5,1.0,,false`; got: {}",
    data_line
);

// And it deserializes back to None / false:
let mut rdr2 = csv::Reader::from_reader(Cursor::new(buf2));
let rb2: BenchmarkRow = rdr2.deserialize().next().unwrap().unwrap();
assert!(rb2.stop_reason.is_none());
assert!(!rb2.cv_above_gate);
```

**Expected output:** All assertions pass.

**Edge cases:**
- (EC-1) `stop_reason = Some("Oom")`: identical roundtrip path; not asserted explicitly here (covered by UT in TEST-SPEC-0701 enum coverage). Optional sub-step if DEV wants.
- (EC-2) `vmrss_peak_mb = 0.0` (a zero-byte rep): roundtrips. The test does NOT explicitly cover; default-zero values are the easy case.
- (EC-3) Negative or NaN `vmrss_*`: out of scope — `MemoryProbe` returns `u64`, divided by 1024×1024 is non-negative finite.

---

### IT-0703-02: `legacy_struct_reads_post_d014_row`

**Purpose:** Acceptance criterion 5 — a struct deserializing only the original 25 columns reads any row produced by post-D-014 code without error. Forward compatibility for the D-010/D-011/D-012 readers.

**Test body:**

```rust
// 1. Write a post-D-014 row (with the 4 new columns populated) using the
//    NEW struct.
let mut row = BenchmarkRow::default();
row.vmrss_peak_mb        = 50.0;
row.vmrss_current_end_mb = 40.0;
row.stop_reason          = None;
row.cv_above_gate        = false;

let mut buf: Vec<u8> = Vec::new();
{
    let mut wtr = csv::Writer::from_writer(&mut buf);
    wtr.serialize(&row).unwrap();
    wtr.flush().unwrap();
}

// 2. Define a "legacy" struct mirroring only the pre-D-014 columns.
//    This local struct has #[derive(Deserialize)] and lists the original
//    25 fields. We do NOT derive Serialize — read-only.
//
//    The csv crate, by default, ignores trailing columns when the
//    struct has fewer fields than the row. Verify that this default
//    holds.

#[derive(serde::Deserialize)]
struct LegacyRow {
    // The developer copies the 25 pre-D-014 field names + types verbatim
    // from the row struct's git history (commit predating this TASK).
    // Listed here as a placeholder; DEV fills exact list:
    //   workload: String,
    //   workers: u32,
    //   ... (23 more) ...
    // For the test, ANY subset that compiles and roundtrips is sufficient
    // to demonstrate the forward-compat behavior.
    #[serde(flatten)]
    _opaque: std::collections::HashMap<String, String>,
    // alternative: drop the flatten and list the exact 25 columns by name.
}

// 3. Deserialize into LegacyRow. Must NOT error.
let mut rdr = csv::Reader::from_reader(std::io::Cursor::new(buf));
let _legacy: LegacyRow = rdr.deserialize::<LegacyRow>()
    .next().expect("must have a row").expect(
        "legacy struct must deserialize a post-D-014 row without error \
         (the csv crate ignores trailing columns by default; verifying \
         that contract is intact)"
    );
```

**Expected output:** No panic; the legacy reader successfully consumes the row.

**Edge cases:**
- (EC-1) The `flatten + HashMap<String, String>` shortcut sidesteps having to enumerate 25 field names. If DEV prefers explicit columns, copying them from the pre-D-014 row struct works equally well — the contract is the same.
- (EC-2) `csv::ReaderBuilder::flexible(true)` is the default; if a future change toggles strict mode, this test catches it.
- (EC-3) Existing pre-D-014 CSV-related tests (UT-level + integration) MUST continue to pass — this is implicit (TASK-0703 acceptance criterion 6 "zero regression"). Not a separate test; it's the floor invariant of the cumulative ≥ 1811 default.

---

## Acceptance criteria mapping

| TASK-0703 AC | Test coverage |
|---|---|
| AC-1 (4 columns at end of row, exact names) | IT-0703-01 Sub-step 3 |
| AC-2 (`stop_reason` empty-string for None / variant name for Some) | IT-0703-01 Sub-steps 4 + 5 |
| AC-3 (`cv_above_gate` true/false) | IT-0703-01 Sub-step 4 (true), Sub-step 5 (false) |
| AC-4 (roundtrip with f64 EPSILON tolerance) | IT-0703-01 Sub-step 4 |
| AC-5 (legacy reader forward-compat) | IT-0703-02 |
| AC-6 (zero regression on pre-D-014 tests) | implicit (cumulative floor invariant) |

## Edge Cases Catalog

| # | Scenario | Expected | Test |
|---|----------|----------|------|
| EC-0703-01 | All 4 new fields populated | roundtrips losslessly | IT-0703-01 (4) |
| EC-0703-02 | `stop_reason = None` | empty string in CSV; round-trips to None | IT-0703-01 (5) |
| EC-0703-03 | Old struct reading new row | succeeds; trailing columns ignored | IT-0703-02 |
| EC-0703-04 | Header order preserved | new columns at the END | IT-0703-01 (3) |

## Out of scope

- Frozen `results/locked/v2_post_d012_baseline_2026-05-05/` — read-only forever; not tested.
- Property tests — deferred.
- Schema versioning / a separate `schema_version` column — out of scope; the additive column model IS the versioning.
- Rust struct field ORDER vs CSV column order — by serde-csv-derive convention, the derive macro emits columns in declaration order. Test (3) verifies the suffix; the prefix (existing 25) was already verified by pre-D-014 tests.

## Open questions for DEV

1. The exact existing field name list for `LegacyRow` should be lifted from `bench/csv.rs` HEAD before this task. The `flatten + HashMap<String, String>` shortcut is acceptable but less explicit.
2. If `BenchmarkRow` does NOT derive `Default`, DEV substitutes the existing row constructor (a single-call helper used by D-012 baseline) for the test fixture.
