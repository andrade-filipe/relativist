# TASK-0707 — D-014-ITESTS: 6 integration tests for `stress_curve_*.rs`

**Phase:** D-014 (Stress Curve Campaign) — Stage 3 DEV scope
**Bundle:** D-014 — Stress Curve Campaign
**Status:** TODO
**Priority:** P0 (locks the contract; everything before this is unit-verified, this is the end-to-end check).
**Spec:** none.
**Depends on:** TASK-0700, TASK-0701, TASK-0702, TASK-0703 (production code must exist). NOT dependent on TASK-0704 (script smoke is in TASK-0704 itself); NOT dependent on TASK-0706 (docs only).
**Estimated complexity:** M (~180 LoC across 6 small integration tests + ~30 LoC shared test helpers).

---

## Context

Per design doc §8 testing pyramid, the campaign needs 6 specific integration tests — these are NOT the smoke (TASK-0704) or plot (TASK-0705) tests already covered; they are dedicated checks for the new behavioral surfaces. Listed in design doc §8:

(a) **Memory probe vs oracle 100 MiB** — allocate `Vec<u8>` of 100 MiB, verify probe reports `current_bytes` rises by ≥ 80 MiB and `peak_bytes` ≥ 100 MiB.

(b) **Stop rule wall** — fake a `RepResult` with `wall = 6 min`, `wall_budget = 5 min`; verify `check` returns `Some(WallTimeExceeded)`.

(c) **Stop rule RAM** — fake a `RepResult` with `vmrss_peak_fraction_of_total = 0.85`, threshold `0.80`; verify `check` returns `Some(MemoryExceeded)`.

(d) **Stop rule OOM (SIGKILL)** — spawn a child process that allocates until OOM-killed (or simulate via `child_exit = Killed { signal: 9 }`), feed into `check`; verify `Some(Oom)`.

(e) **`--resume` invariant** — invoke `scripts/stress_curve.sh --smoke --no-docker`, kill it mid-rep, restart with `--resume`, verify final dataset is identical (modulo wall times) to a clean run.

(f) **End-to-end smoke (1 workload, W=2, N=[1k, 10k], 1 rep)** — direct in-process invocation of `StressCurveDescriptor::run_one_sequence` (NOT via the script), verify `SequenceOutcome.completed_reps.len() == 2`.

These 6 tests live in 6 separate files under `relativist-core/tests/`, one per concern, for fast triage on failure.

## Files in scope

| File | Concern | LoC |
|------|---------|-----|
| `relativist-core/tests/d014_memory_probe_oracle.rs` | (a) | ~30 |
| `relativist-core/tests/d014_stop_rule_wall.rs` | (b) | ~25 |
| `relativist-core/tests/d014_stop_rule_ram.rs` | (c) | ~25 |
| `relativist-core/tests/d014_stop_rule_oom.rs` | (d) | ~40 |
| `relativist-core/tests/d014_resume_invariant.rs` | (e) | ~50 |
| `relativist-core/tests/d014_end_to_end_smoke.rs` | (f) | ~30 |
| `relativist-core/tests/common/d014_helpers.rs` | shared `RepResult` factory | ~20 |
| `relativist-core/tests/common/mod.rs` | re-export helper | already exists; add 1 line |

## Files explicitly OUT of scope

- The smoke test from TASK-0704 (`d014_stress_curve_smoke.rs` — script-driven) is separate.
- The plot smoke from TASK-0705 (`d014_plot_smoke.rs`) is separate.
- Property tests (proptest) — design doc lists "~3 properties (proptest opcional)" as optional; defer to a follow-up task if needed.
- macOS — these tests skip with `#[cfg(not(target_os = "macos"))]` because `MemoryProbe` returns errors there (per TASK-0700 contract).

## Acceptance criteria

1. All 6 test files exist and compile.
2. Each test passes on Linux + Windows (where applicable; (e) is `#[cfg(unix)]` only because it shells out to bash).
3. Test (a) on a system with < 200 MiB free RAM is skipped with a clear message (use `MemoryProbe::as_fraction_of_total` to query and skip if `< 0.10` available).
4. Test (d) is `#[cfg(unix)]` — Windows OOM detection is exercised by the unit tests in TASK-0701 + the script integration in TASK-0704, not here.
5. Test (e) is `#[cfg(unix)]` — bash dependency.
6. Tests (b), (c), (f) are cross-platform.
7. Each test has a meaningful failure message (`assert!(..., "...")`) — no silent panics.
8. `cargo test` floor: **+6 default = ≥ 1819** (cumulative TASK-0700..0707; previous was 1813).
9. `cargo test --features zero-copy` floor: **+6 = ≥ 1863**.
10. `cargo test --features streaming-no-recycle` floor: **+6 = ≥ 1810**.
11. `cargo test --release` floor: **+6 = ≥ 1761**.
12. v1 floor (690) inviolable.
13. `cargo clippy --all-features -- -D warnings` clean.
14. `cargo fmt --check` clean.

## Test floor delta

**+6 default** (six new integration test files, one test each). Cumulative after TASK-0700..0707:
- default ≥ 1819
- zero-copy ≥ 1863
- streaming-no-recycle ≥ 1810
- release ≥ 1761

## Implementation hints

1. Test (a) should `std::hint::black_box` the 100 MiB allocation to prevent the optimizer from elide-ing it in release.
2. Test (d) on Linux: `Command::new("sh").arg("-c").arg("python3 -c 'a=[0]*10**11'")` then `wait()`; expect `ExitStatus::signal() == Some(9)`. If `python3` unavailable, skip.
3. Test (e) is the trickiest. Use `tempfile::tempdir()` for the output dir. Spawn `scripts/stress_curve.sh` with `Command::new("bash")`, `kill -INT` after 30 sec, then re-spawn with `--resume`, then compare CSVs sorted by `(workload, env, W, N, rep)`. Allow ± 50% wall-time noise; require exact match on every other column.
4. Test (f) directly imports `StressCurveDescriptor` (TASK-0702) — no shell. Override `n_seq` with `&[1_000, 10_000]` and `wall_budget` with `Duration::from_secs(30)`.
5. Use the `common/d014_helpers.rs` for the `RepResult` factory — DRY across tests (b), (c), (d).
6. **Do NOT** import `relativist-core` internals; use only the `pub` API. If a test needs an internal type that isn't `pub`, file a note in this task's discussion to surface it as `pub(crate)` → `pub` via TASK-UPDATER, do NOT widen visibility silently.

## Estimated LoC

- Test code: ~180 LoC across 6 files.
- Shared helper: ~20 LoC.
- Total: ~200 LoC. Right at the ceiling — split happens naturally because the tests are 6 separate files.

## Cross-references

- Design doc: `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` §8 testing pyramid (lists exactly these 6 tests).
- Consumes: TASK-0700 (`MemoryProbe`), TASK-0701 (`StopRule`), TASK-0702 (`StressCurveDescriptor`), TASK-0703 (CSV columns).
- Peer tests: TASK-0704 (`d014_stress_curve_smoke.rs` — script smoke), TASK-0705 (`d014_plot_smoke.rs` — plot smoke).
