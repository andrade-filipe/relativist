# TEST-SPEC-0605 — Tests for TASK-0605 — `get_peak_memory_at_construction_complete` probe

**Task:** TASK-0605 (Phase C-5, P0)
**Spec:** SPEC-09 R18a–R18g (commit `82b2d27` — specifically R18d / R18e on construction-phase peak memory).
**Test floor delta:** **+5 default** (3 Linux-gated + 1 non-Linux-gated + 1 cross-platform = 5 total under appropriate cfg gating; effective floor advance is 1 unconditional + 4 gated, but cargo test counts all configured tests on the runner platform — assume Linux CI ⇒ +5 default).
**Prerequisites:** TASK-0602 (RECOMMENDED for the result row field; can run earlier if row struct is extended directly).

---

## Test inventory

| test_id | level | target file | prerequisite TASK | cfg gates |
|---|---|---|---|---|
| UT-0605-01 | unit | `relativist-core/src/bench/memory.rs::tests::probe_returns_nonzero_on_linux` | none | `#[cfg(target_os = "linux")]` |
| UT-0605-02 | unit | `relativist-core/src/bench/memory.rs::tests::probe_returns_zero_on_non_linux` | none | `#[cfg(not(target_os = "linux"))]` |
| UT-0605-03 | unit | `relativist-core/src/bench/memory.rs::tests::probe_does_not_panic_any_os` | none | none |
| IT-0605-04 | integration | `relativist-core/tests/spec09_bench_construction_memory_probe.rs::probe_value_le_end_of_run_peak` | TASK-0602, TASK-0604 (recommended) | `#[cfg(target_os = "linux")]` |
| IT-0605-05 | integration | `relativist-core/tests/spec09_bench_construction_memory_probe.rs::csv_emits_peak_memory_at_construction_complete_column` | TASK-0602, TASK-0604 | `#[cfg(target_os = "linux")]` |

Total: **5 default tests** on Linux CI; **2 default** on non-Linux (UT-0605-02 + UT-0605-03).

---

## Per-test specifications

### UT-0605-01 — `probe_returns_nonzero_on_linux`

**Purpose.** Sanity: on Linux, `get_peak_memory_at_construction_complete()` reads `/proc/self/status` VmHWM and returns a non-zero value.
**Setup.** None — the cargo test process itself has resident memory.
**Action.** `let v = get_peak_memory_at_construction_complete();`
**Assertions.**
- `v > 0`.
- `v >= 1024` (Linux VmHWM reports in kB; the test process is at least a few MiB).
**Boundary case coverage.** Catches a buggy implementation that reads the wrong line (e.g., `VmRSS` instead of `VmHWM`) or fails to parse.
**cfg gate.** `#[cfg(target_os = "linux")]`.
**Why it must exist.** Acceptance criterion #4 of TASK-0605.

---

### UT-0605-02 — `probe_returns_zero_on_non_linux`

**Purpose.** On non-Linux (Windows/macOS), the probe returns 0 (matches existing convention).
**Setup.** None.
**Action.** `let v = get_peak_memory_at_construction_complete();`
**Assertions.**
- `v == 0`.
**Boundary case coverage.** Catches a buggy implementation that returns a fabricated value or panics on missing `/proc`.
**cfg gate.** `#[cfg(not(target_os = "linux"))]`.
**Why it must exist.** Acceptance criterion #5 of TASK-0605.

---

### UT-0605-03 — `probe_does_not_panic_any_os`

**Purpose.** Cross-platform sanity that the function is callable without panic on any supported target.
**Setup.** None.
**Action.** `let _v = get_peak_memory_at_construction_complete();` — discarded.
**Assertions.** No panic (test passes by reaching the next line).
**Boundary case coverage.** Catches a buggy implementation that `unwrap()`s a missing `/proc/self/status` (which on a sandboxed Linux runner could legitimately fail to read).
**cfg gate.** None (runs on every target).
**Why it must exist.** Robustness — mirrors the existing pattern at `bench/memory.rs:36-38` for `get_peak_memory_bytes`.

---

### IT-0605-04 — `probe_value_le_end_of_run_peak`

**Purpose.** SPEC-09 R18d monotonicity: the construction-time peak (snapshot at construction-complete) MUST be `<=` the end-of-run peak (snapshot after `reduce_all`). VmHWM is monotonic non-decreasing, so this is a fundamental property.
**Setup.**
- Run the bench harness in-process for `ep_annihilation`, size=10000, workers=1.
- Capture two values from the result row: `peak_memory_at_construction_complete` (new field), `peak_memory_bytes` (existing field).
**Action.** Read the two values.
**Assertions.**
- `peak_memory_at_construction_complete > 0`.
- `peak_memory_at_construction_complete <= peak_memory_bytes` (monotonicity).
- `peak_memory_at_construction_complete >= 1_000_000` (sanity: a 10000-agent net occupies more than 1 MB).
**Boundary case coverage.** Catches a buggy implementation that snapshots after `reduce_all` (would equal end-of-run peak), or that snapshots before `make_net` (would be near process baseline, possibly < the lower bound).
**cfg gate.** `#[cfg(target_os = "linux")]`.
**Why it must exist.** Acceptance criterion #4 of TASK-0605.

---

### IT-0605-05 — `csv_emits_peak_memory_at_construction_complete_column`

**Purpose.** Acceptance criterion #6: the new CSV column is emitted with valid values, appended to the END of the schema (defends `v1_local_baseline` cross-comparison).
**Setup.**
- Run bench harness for ep_annihilation, size=1000, workers=2, output to a temp CSV path.
**Action.** Parse CSV header + row.
**Assertions.**
- Header contains the column name `peak_memory_at_construction_complete` (or the snake-case rendering chosen by the developer; document the chosen name in the test as a constant).
- The new column appears at the END of the header, NOT in the middle (preserves `v1_local_baseline` schema position parity).
- The value parses as `u64` and is `> 0`.
- The value is `<=` the existing `peak_memory_bytes` column value.
**Boundary case coverage.** Catches a CSV writer bug that swaps column order or emits the new column in the middle.
**cfg gate.** `#[cfg(target_os = "linux")]` (the value-correctness assertion requires Linux; the column-position assertion alone could run cross-platform but bundling them is simpler).
**Why it must exist.** Acceptance criterion #6 of TASK-0605.

---

## Coverage matrix

| test_id | §AC-1 | §AC-2 | §AC-3 | §AC-4 | §AC-5 | §AC-6 | §AC-7 | R18d | R18e |
|---|---|---|---|---|---|---|---|---|---|
| UT-0605-01 | ✅ | | | ✅ | | | | ✅ | |
| UT-0605-02 | ✅ | | | | ✅ | | | | |
| UT-0605-03 | ✅ | | | | | | ✅ | | |
| IT-0605-04 | | ✅ | ✅ | ✅ | | | | ✅ | ✅ |
| IT-0605-05 | | | ✅ | | | ✅ | | ✅ | ✅ |

Every acceptance criterion has ≥1 test.

---

## Out-of-scope tests (deferred to other tasks)

- R37c construction-phase isomorphism (sparse-net specifics) → **TASK-0606** (Phase D).
- Cross-OS (Windows/macOS) memory accounting beyond returning 0 → out of scope per task §Files OUT of scope.
- Streaming-path memory probe correctness specifically → covered indirectly by IT-0605-04 + IT-0604-06 union; explicit coverage is **TASK-0606** + **TASK-0610** (Phase F-2 baseline rodada).
- Construction-memory < total-memory by some margin (stronger gate) → out of scope; belongs to **Phase F-2** baseline analysis per task §Notes.
