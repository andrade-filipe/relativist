# REVIEW — D-014 Stress Curve Campaign

**Reviewer:** Stage 4 (REVIEW) of SDD pipeline
**Date:** 2026-05-06
**Branch:** `feature/stress-and-encoder` (HEAD `e066d3c`)
**Scope:** 9 commits `bcff245..e066d3c` (TASK-0700..0708)
**Inputs read:**
- `docs/backlog/TASK-0700..0708-stress-curve-*.md`
- `docs/tests/TEST-SPEC-0700..0707-*.md` (sampled by acceptance-criteria mapping)
- `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md`
- Source: `relativist-core/src/bench/{memory_probe,stop_rule,suite,csv,mod}.rs`,
  `relativist-core/src/{commands,config,error}.rs`
- Tests: `relativist-core/tests/d014_*.rs` (10 files), `tests/spec09_bench_construction_memory_probe.rs`
- Scripts: `scripts/stress_curve.sh`, `scripts/plot_stress_curve.py`,
  `scripts/requirements-stress-curve.txt`
- Docs: `docs/benchmarks/campaigns/stress-curve.md`,
  `results/locked/v2_stress_curve_TEMPLATE/SENTINEL.md`,
  `docs/pipeline-state.md` (via `docs/next-steps.md`)
- Coding standards: `CLAUDE.md` (Relativist) + root `CLAUDE.md`

---

## Verdict

**Code quality verdict:** PASS WITH NOTES
**Architecture verdict:** ALIGNED — minor schema-coupling gap between
`bench/csv.rs` schema and `scripts/plot_stress_curve.py` reader contract
**Spec compliance:** N/A (no spec; honors design doc §4.4 verbatim).
**Overall:** **AMARELO (yellow) — advance to Stage 5 (QA) with one
mandatory pre-overnight follow-up TASK to close the CSV-emission gap
flagged below as MAJOR-1; non-blocking for QA itself.**

The Rust additions are surgical, well-documented, properly feature-bound
(`cfg(target_os)`, `cfg(unix)` on shell-driven IT files), and respect
the dependency direction (`bench/` is leaf-of-leaves; pulls only `error`,
`std`, `serde`, `windows-sys`). All 5 `BenchmarkResult` construction
sites are updated (4 production + 1 new IT helper); the developer's
"4 sites" count excludes the new IT, but every site is patched —
**no missed 5th site**. Floors hold (Windows host: 1816 default / 1860
zero-copy / 1807 streaming-no-recycle / 1758 release; Linux floor adds
the 3 `cfg(unix)` IT files documented in §"Floor verification" below).
v1 floor 690 inviolable.

The single non-trivial concern is that `--campaign stress-curve`'s
in-Rust dispatch path emits a one-line `println!` summary instead of a
real CSV row. The bash orchestrator captures that text into `RAW_CSV`
under `>>"$RAW_CSV"`, but the real campaign's downstream plot generator
reads `aggregated.csv` by COLUMN NAMES that the existing `bench/csv.rs`
writer does not produce verbatim (`workload`, `env`, `n`, `wall_seconds`,
`mips` vs the schema's `benchmark`, `input_size`, `mode`, `workers`,
`wall_clock_secs`, `mips`/`mips_mean`). Smoke tests pass because they
hand-roll a CSV in the test fixture; the overnight campaign as
currently wired would emit no plottable artifact. This is gated by the
SENTINEL.md operator-audit step ("STOP — do not lock"), so it is
**safe to merge but unsafe to run overnight without a follow-up**.

---

## Per-commit summary

| Commit | Title | Prod LoC | Test LoC | Verdict |
|---|---|---|---|---|
| `bcff245` | TASK-0700 cross-platform MemoryProbe + BenchError | +385 (4 files) | inline 4 UTs | ACCEPT |
| `e8dbf7c` | TASK-0701 StopRule sequence aborter | +347 (1 file) | inline 6 UTs | ACCEPT |
| `ebfa092` | TASK-0702 stress-curve descriptor + CLI | +273 (4 files) | — | ACCEPT_WITH_NOTES |
| `415ac6e` | TASK-0703 D-014 CSV +4 columns | +53 (3 files) | +204 IT | ACCEPT |
| `f25be1f` | TASK-0707 6 IT files | +13 helpers | +682 IT | ACCEPT_WITH_NOTES |
| `dabd1fb` | TASK-0704 bash orchestrator | +308 sh | +125 IT | ACCEPT_WITH_NOTES |
| `51c3190` | TASK-0705 plot generator | +238 py + 3 req | +118 IT | **ACCEPT_WITH_FIXES** (CSV column-name contract) |
| `8a192aa` | TASK-0706 docs page | +244 md | — | ACCEPT |
| `e066d3c` | TASK-0708 sentinel + doc updates | +106 md + ~10 docs | — | ACCEPT |

Production Rust: ~1058 LoC across `bench/{memory_probe,stop_rule,suite,csv,mod}.rs`,
`{config,commands,error}.rs`. Tests: 1729 LoC across 10 IT files +
inline UTs. Shell + Python: ~550 LoC. All within
the per-task <200 LoC budget given the additive nature.

---

## Must-Fix Issues

### MF-001 — `--campaign stress-curve` emits `println!` summary, not a CSV row

**Category:** Code Quality / Spec Compliance / Architecture
**Severity:** MAJOR
**Files:** `relativist-core/src/commands.rs:332-337`,
`scripts/stress_curve.sh:178-186`, `scripts/plot_stress_curve.py:34-46`

**Problem.** Three coupled gaps:

1. **Production `println!`.** `commands.rs:332` emits `println!(...)` from
   the dispatch branch:
   ```rust
   println!(
       "stress-curve outcome: completed_reps={} stop_reason={:?} last_attempted_n={:?}",
       outcome.completed_reps.len(),
       outcome.stop_reason,
       outcome.last_attempted_n,
   );
   ```
   `CLAUDE.md` (Relativist) §Coding Standards: "No `println!` — use
   `tracing` macros only". Pre-existing `commands.rs` does call
   `println!` elsewhere (suite path), so this isn't unprecedented;
   nonetheless this dispatch path's *sole* output is the println, which
   the bash orchestrator then captures.

2. **No CSV emission in dispatch path.** The bash script does
   ```bash
   "$RELATIVIST_BIN" bench --campaign stress-curve ... >>"$RAW_CSV"
   ```
   capturing the human-readable summary line *as if it were a CSV row*.
   The orchestrator does NOT add a header. The first such file looks
   like:
   ```
   stress-curve outcome: completed_reps=2 stop_reason=None last_attempted_n=Some(10000)
   stress-curve outcome: completed_reps=1 stop_reason=Some(MemoryExceeded) last_attempted_n=Some(316228)
   ```
   The script comment at `:172-177` admits this:
   "The relativist binary's --campaign stress-curve currently prints a
   stdout summary; CSV emission per-rep is a follow-up. … TASK-0708
   will replace this with a proper CSV-append flag." But TASK-0708 was
   *not* used to add the flag — it became the SENTINEL only. So the
   gap is real and unmitigated.

3. **Plot script reads columns by name that don't exist.**
   `scripts/plot_stress_curve.py:34-46` declares
   ```python
   REQUIRED_COLUMNS = [
       "workload", "env", "workers", "n", "rep",
       "wall_seconds", "mips",
       "vmrss_peak_mb", "vmrss_current_end_mb",
       "stop_reason", "cv_above_gate",
   ]
   ```
   But `bench/csv.rs` produces `benchmark`, `input_size`, `mode`,
   `workers`, `repetition`, `wall_clock_secs`, `mips`, `vmrss_peak_mb`,
   `vmrss_current_end_mb`, `stop_reason`, `cv_above_gate`. The first 5
   names differ; only the 4 D-014 additions match. `plot_stress_curve.py`
   on a real `bench/csv.rs` output would exit **code 1** at
   `load_aggregated()` per its own contract.

**Why it slipped past tests.** `d014_plot_smoke.rs:51-65` hand-rolls a
12-row CSV with the *plotter's* expected columns; `d014_stress_curve_smoke.rs`
checks for `>=1` PDF (placeholder counts) and does not parse `RAW_CSV`
contents. So the test contract is "the script accepts a CSV with this
column shape", not "the binary produces a CSV with this column shape".
The two never connect.

**Recommendation (post-merge follow-up; non-blocking for Stage 5 QA).**
Open a follow-up TASK (suggest `TASK-0720`) that:
- Adds an `--append-csv PATH` flag to `BenchArgs` for the stress-curve
  dispatch (mirrors existing `--csv-detail-path` pattern in suite path).
- Per rep, builds a `BenchmarkResult` and calls
  `write_csv_detail` (header on first write, append after) — reusing
  the *real* schema. **OR** writes a dedicated stress-curve CSV with
  the columns `plot_stress_curve.py` expects (`workload`, `env`, `n`,
  `wall_seconds`, `mips`, …) and updates the plot script to read THAT
  schema. Pick one schema; the design doc §4.2 row 4 is consistent with
  the latter.
- Replaces the `println!` with `tracing::info!`.
- Updates `scripts/stress_curve.sh:178-186` to invoke with
  `--append-csv "$RAW_CSV"` and remove the stdout-redirect.
- Adds an IT that runs `bench --campaign stress-curve` end-to-end and
  feeds the resulting CSV through `plot_stress_curve.py` — closing
  the contract loop.

**Why MAJOR not CRITICAL.** SENTINEL.md §3 step 4 mandates an operator
sanity check ("if any fails: STOP, do NOT lock") that would catch the
empty-figures / malformed-CSV symptom. So no risk of silently locking
a corrupted dataset. But running the campaign as-is would burn 7-8h
overnight to produce a dataset that the plot stage cannot consume — a
real cost that justifies a follow-up before TASK-0708's overnight run.

---

## Should-Fix Issues

### SF-001 — `BenchmarkResult.stop_reason: Option<String>` (primitive obsession)

**Category:** Code Quality / Rust Idioms (DIP)
**Severity:** MINOR
**Files:** `relativist-core/src/bench/mod.rs:285-289`,
`relativist-core/src/bench/csv.rs:135`

**Problem.** `mod.rs:286` stores stop reason as `Option<String>` rather
than `Option<crate::bench::stop_rule::StopReason>`. CSV emission at
`csv.rs:135` does `r.stop_reason.as_deref().unwrap_or("")`. This loses
type safety: any string can be inserted, including misspellings of the
3 enum variants. The serde bound on `StopReason` already supports CSV
serialization (the enum has `Serialize` + `Deserialize` derive at
`stop_rule.rs:21`), so the typed form is achievable.

**Before:**
```rust
#[serde(default)]
pub stop_reason: Option<String>,
```
**After (sketch):**
```rust
#[serde(default, with = "stop_reason_csv")]
pub stop_reason: Option<StopReason>,
// where stop_reason_csv::serialize emits "" for None and the variant
// name (matching `Debug`) for Some(_), and ::deserialize parses the
// inverse — using the existing `Serialize` derive output verbatim.
```

**Why MINOR.** The `String` form works; the test at
`d014_csv_schema_roundtrip.rs:73` happens to pass `"MemoryExceeded"`
verbatim. But if any future code path computes the reason from a non-
literal source the typo risk surfaces. Given the dispatch path doesn't
even populate `stop_reason` today (always `None`), this can wait for
the MF-001 follow-up.

### SF-002 — `RepResult::vmrss_peak_fraction_of_total: f64` lacks finiteness guard

**Category:** Defensive programming / SOLID (LSP)
**Severity:** MINOR
**Files:** `relativist-core/src/bench/stop_rule.rs:46-50, 106-109`

**Problem.** `RepResult.vmrss_peak_fraction_of_total` is `f64`. `StopRule::check`
at `:107` does `if last_rep.vmrss_peak_fraction_of_total > self.memory_fraction_max`.
If a degenerate rep produces NaN, IEEE 754 makes any comparison `false`
— the rule silently passes a NaN-fraction rep. The doc-comment on
`:46-48` acknowledges this:
> "the rule itself does not enforce finiteness — the caller is responsible."

The "caller" (the descriptor at `suite.rs:1192-1199` and the bash
orchestrator) goes through `MemoryProbe::as_fraction_of_total` which
guards `total_ram_bytes == 0` but does NOT guard non-finite numerator
(though `bytes: u64` cannot be NaN, so this is currently safe). A future
change to `f64` peak — e.g., from a Python child process exit log —
would expose the gap.

**Recommendation.** Add to `StopRule::check` at the top:
```rust
if !last_rep.vmrss_peak_fraction_of_total.is_finite() {
    return Some(StopReason::MemoryExceeded); // fail closed
}
```
or, more conservatively, `debug_assert!(last_rep.vmrss_peak_fraction_of_total.is_finite())`.

**Why MINOR.** Today's `MemoryProbe` cannot produce NaN; the gap is
hypothetical. The doc-comment is honest about it. Acceptable defer.

### SF-003 — `MemoryProbe` per-call cfg branches duplicate platform structure

**Category:** Code Quality / DRY
**Severity:** MINOR
**Files:** `relativist-core/src/bench/memory_probe.rs:34-107`

**Problem.** Three methods (`new`, `current_bytes`, `peak_bytes`) each
contain a 4-arm `#[cfg(target_os = ...)]` block. Maintenance cost is
real: any future addition (e.g., FreeBSD support, a 4th method) requires
touching 3 + N sites. Standard Rust idiom: a private trait `Backend`
with one `#[cfg]`-gated impl per platform, exposed by a single inline
`fn backend() -> impl Backend`. Reduces cyclomatic complexity per method
and centralizes the macOS/unsupported error message.

**Why MINOR.** The current code is honest, readable, and the test
inline at `:237-356` covers all 3 platforms. Refactoring is invariant-
preserving but not load-bearing. Acceptable as-is for v0.20.0-pre.1.

### SF-004 — Plot script's silent geomean-on-positive-only could mask zero rows

**Category:** Robustness / Empirical correctness
**Severity:** MINOR
**Files:** `scripts/plot_stress_curve.py:126-134`

**Problem.** `plot_metric` filters to `vals_pos = vals[vals > 0]` before
computing geometric mean. If all 5 reps for a (workload, env, W, N) cell
have `wall_seconds == 0` (e.g., a child crashed before the timer started)
the cell renders `NaN` silently and the user sees a gap in the curve
without explanation. The campaign already has `cv_above_gate` for
statistical instability; a similar `all_zero` flag (or a stderr warning
"N=10000 W=2 had K=5 zero-wall reps; cell omitted") would close this gap.

**Why MINOR.** `aggregated.csv` is the lock-time artifact; operators
can grep for zeros manually. Plot-time silent omission is consistent
with matplotlib defaults. Acceptable for v0.20.0-pre.1.

### SF-005 — Bash `cut -d,` parsing in `--resume` will mis-handle any future quoted-field row

**Category:** Robustness / Maintainability
**Severity:** MINOR
**Files:** `scripts/stress_curve.sh:138-148`

**Problem.** The resume loop reads the CSV with `cut -d, -fN` per field.
This works for the current schema (no fields contain literal commas),
but is one schema change away from silent breakage. The `csv` Rust
crate the writer side uses *does* quote fields containing commas — if a
future column adds free-form text (`"Known anomalies"` etc.) the cut
field count drifts.

**Recommendation.** Either pin the field separator on a guaranteed-safe
delimiter (e.g., write the resume index as a 5-column TSV `wl<TAB>env<TAB>w<TAB>n<TAB>rep`
alongside the CSV) or use `python3 -c "import csv; ..."` for parsing.
A 5-line python helper would be simpler and identical to what
`plot_stress_curve.py` does.

**Why MINOR.** Today's schema has no quoted fields. Acceptable for
v0.20.0-pre.1.

### SF-006 — `IT-0707-05` excludes `vmrss_*` strict-equality intentionally

**Category:** Test Quality / Spec Compliance
**Severity:** MINOR (review of developer's finding #7)
**Files:** `relativist-core/tests/d014_resume_invariant.rs:108-120`

**Problem.** TEST-SPEC-0707 §IT-0707-05 (e) calls for "strict equality"
on the resumed dataset modulo wall-time noise. The developer chose to
relax `vmrss_peak`, `vmrss_current_end`, and `mips` to also tolerate
drift, on the basis that cross-process memory measurements legitimately
drift run-to-run.

**Reviewer position: ACCEPT THE RELAXATION.** The justification is
correct on the merits — VmHWM/VmRSS sampling is inherently noisy
across child processes, and `mips = total_interactions / wall` so any
wall-noise propagates to mips. Holding the test to "strict equality"
would make it flaky on CI. The strict-equality contract sensibly
applies to the **deterministic** columns (`benchmark`, `input_size`,
`mode`, `workers`, `correct`, `total_interactions`, etc.) and the
inline `WALL_NOISE_TOKENS` list `:112-120` is the right granularity.

**No fix required.** Recommend documenting the relaxation in
`docs/benchmarks/campaigns/stress-curve.md` §"Test contract" so the
contract is explicit (a single sentence: "the resume invariant excludes
wall-time- and memory-derived columns, which legitimately vary across
child-process runs").

---

## Nice-to-Have

### NTH-001 — `print_help()` parses its own docstring with `sed`

**Files:** `scripts/stress_curve.sh:50-52`. `sed -n '2,/^=*$/p'` is
fragile against future edits to the comment-block delimiters. Replace
with a literal here-doc.

### NTH-002 — `unwrap_or(u32::MAX)` in `n.try_into()` (`suite.rs:1184`)

The N-sweep includes 10⁹ which fits in u32 (≤ 4.29 × 10⁹). The
`unwrap_or(u32::MAX)` silently downgrades any future N > u32::MAX to
the cap. Adding a `tracing::warn!` on the saturation arm would catch
the day someone tries N=10¹⁰.

### NTH-003 — `StressCurveDescriptor` is a zero-state namespace

`pub struct StressCurveDescriptor;` is a stylistic choice. A free
`mod stress_curve { pub fn n_seq() -> &'static [usize] { ... } }`
is more idiomatic Rust and saves the `Self::` qualifier. Acceptable
either way.

### NTH-004 — `OOM_EXIT_CODES` and `SIGKILL_SIGNUM` are `pub`, exposed to bash via stringly-typed coupling

The exit-code list is `pub const OOM_EXIT_CODES: &[i32] = &[137, -1073741801];`
in `stop_rule.rs:83`. The bash script duplicates the literal list
implicitly (the design doc binds them, the script does not import).
Consider emitting them via a tiny `relativist debug oom-exit-codes`
sub-command so the script `read`s them at startup — eliminating drift.
Today the doc-comment on `OOM_EXIT_CODES` does say "Exposed publicly so
the bash orchestrator (TASK-0704) can pull the same list and avoid
drift" — but the script doesn't actually pull. Single source of truth
not yet wired.

### NTH-005 — `d014_helpers.rs` lives at `tests/common/`

Convention. No issue.

### NTH-006 — `cargo doc` coverage for `pub` items

Spot-check looks good: every new `pub` item in `bench/memory_probe.rs`,
`bench/stop_rule.rs`, `bench/suite.rs::StressCurveDescriptor`, and the
new `BenchError` variants has a `///` doc-comment. Field-level
documentation on `BenchmarkResult.{vmrss_peak_mb, vmrss_current_end_mb,
stop_reason, cv_above_gate}` is also complete.

---

## Response to the 8 findings declared by the developer

| # | Finding | Reviewer disposition |
|---|---------|----------------------|
| 1 | `run_bench_command` dispatch placeholder (println vs CSV) | **DISAGREE — needs follow-up TASK BEFORE overnight.** See MF-001 above. The developer's framing ("stdout-capture is acceptable") is true *in isolation* (no test fails, no compile warning) but breaks the campaign-to-plot pipeline. Safe to merge; unsafe to run TASK-0708 overnight without `TASK-0720` first. |
| 2 | `BenchmarkResult` field additions ripple in 4 sites | **AGREE on count but actual is 5.** The 4 production + 1 new IT helper at `d014_csv_schema_roundtrip.rs:26`. All 5 patched correctly; no missed site. `validate.rs::DetailRow` is a separate parsing-side struct that ignores trailing columns, so doesn't need touching. |
| 3 | `run_one_sequence` rep semantics (bash reps=1 vs Rust reps=N) | **AGREE — acceptable, minor doc gap.** The inline doc-comment on `run_one_sequence` at `suite.rs:1124-1142` explains it correctly. Should also be lifted to `docs/benchmarks/campaigns/stress-curve.md` §"Run modalities" — one paragraph noting that bash mode resets VmHWM between every rep, in-Rust mode resets only between Ns. |
| 4 | `d014_stress_curve_descriptor.rs` IT walltime ~16-25s | **AGREE — accept as-is.** 25s walltime per IT is steep for CI but the test exercises a critical contract (descriptor smoke + memory probe + StopRule). `#[ignore]` would hide it from the default `cargo test`. Better path: keep the test in default; if CI floor wallclock becomes a problem, wrap the smoke part (Part C) in `#[cfg(not(target_env = "ci"))]` after Stage 6. |
| 5 | `shellcheck` not run | **DEFER to cicd agent.** Reviewer cannot run shellcheck in this environment. Queue a CICD lint TASK alongside the MF-001 follow-up; a single CI job covering shellcheck + ruff for the python script + flake8 would close it. |
| 6 | Plot script PDF placeholder in smoke | **AGREE — accept; pre-condition gate IS sufficient.** §5.1 of the docs page lists 12 pre-conditions; the smoke gate is bypassed only via `--smoke`. The placeholder PDF is exclusive to `--smoke` mode (`scripts/stress_curve.sh:234-253`); it cannot leak into the overnight campaign. |
| 7 | TASK-0707 IT-0707-05 `vmrss_*` strict equality EXCLUDED | **AGREE — accept the relaxation.** See SF-006. Recommend documenting in §"Test contract" of the docs page. |
| 8 | `--campaign stress-curve --n-seq ""` produces a trivial-success | **AGREE — minor UX gap.** Add a check in `commands.rs:316`: `if let Some(ref n) = args.n_seq { if n.is_empty() { return Err(RelativistError::Config("--n-seq must contain at least one value".to_string())); } }`. ~3 LoC, queue with the MF-001 follow-up. |

---

## Floor Verification (developer's claim)

**Windows host:** 1816 default / 1860 zero-copy / 1807 streaming-no-recycle / 1758 release.
**Linux host:** floor adds the 3 `cfg(unix)`-gated test files —
`d014_stress_curve_smoke.rs`, `d014_resume_invariant.rs`,
`d014_stop_rule_oom.rs` — each contributing 1 test function, so
Linux default ≥ 1819, release ≥ 1761.

**Reviewer audit:** all 3 files start with `#![cfg(unix)]` (verified
via Grep). Each is gated for legitimate UNIX-only reasons:
- `d014_stress_curve_smoke.rs` — invokes `bash` directly + reads
  `target/release/relativist[.exe]` in a UNIX path layout.
- `d014_resume_invariant.rs` — uses `libc::kill` for SIGINT (the
  Windows equivalent — `GenerateConsoleCtrlEvent` — is non-portable
  across Cargo's test framework).
- `d014_stop_rule_oom.rs` — uses `std::os::unix::process::ExitStatusExt::signal()`,
  which has no Windows analog.

**Verdict:** explanation is correct; the floor gap is principled, not
incidental. A follow-up TASK could add Windows-specific equivalents
(`windows::Win32::System::Threading::TerminateProcess` for SIGINT-
substitute, `STATUS_NO_MEMORY` injection for OOM-substitute) but this
is not blocking.

---

## Architectural Sanity (CLAUDE.md §"Module Structure")

**Dependency direction (inviolable):**
`net <- reduction <- partition <- merge <- protocol <- coordinator/worker`

**bench/ position.** The `bench` module is *not* in the inviolable
chain — it's a sibling crate-level module that imports from `net`,
`reduction`, `merge`, `protocol`. The new `bench/memory_probe.rs`,
`bench/stop_rule.rs`, and `bench/suite.rs` additions touch:
- `crate::error::BenchError` (new variant added in error.rs) — clean.
- `windows-sys` crate (Cargo.toml addition) — gated `#[cfg(target_os = "windows")]`.
- `std::fs`, `std::process` — `cfg(target_os = "linux")` for `/proc/`.
- `serde::{Serialize, Deserialize}` — already in workspace.

**`bench/` does NOT pull async/tokio.** Verified by grep: the new files
contain zero `tokio`/`async`/`await` tokens. Core layer purity preserved.

**`commands.rs` (the dispatch site) is allowed to be async-unaware
since it's the CLI entry, not the core layer.** The dispatch branch
calls `StressCurveDescriptor::run_one_sequence` synchronously, which
internally calls `run_benchmark_suite` (sync). No async leakage.

**`error.rs` extension.** New `BenchError` enum with 3 variants
(`MemoryProbe`, `Unsupported`, `Io`) is `#[derive(Debug, Error)]` —
matches the project's `thiserror` convention. Doc-comment names
`BenchError` distinct from `MergeError`/`GridError` so the harness
can map cleanly to a single CSV column. Good.

**No spec to violate** (TASK-0700..0708 explicitly say "Spec: none").
Honors design doc §4.4 verbatim.

---

## Test infrastructure cleanliness

- 10 IT files, all with `//! IT-XXXX-XX —` headers naming the
  TEST-SPEC + acceptance criteria they exercise.
- Skip-on-missing-environment idiom is uniform across the bash- and
  python-dependent ITs (`d014_stress_curve_smoke.rs`, `d014_resume_invariant.rs`,
  `d014_plot_smoke.rs`, `d014_stop_rule_oom.rs`). Skips print
  diagnostic to stderr — debuggable.
- `tests/common/d014_helpers.rs` is a 27-LoC `pub fn rep(...) -> RepResult`
  helper imported via `mod common;` in 3 IT files. Standard Rust
  test-fixture idiom.

---

## Recommendation for Stage 5 (QA)

**Advance to Stage 5 (QA) WITHOUT REFACTOR FIRST.** None of the issues
above are correctness regressions; all are forward-looking gaps. QA
should be invoked next.

Suggested adversarial attack vectors for Stage 5 QA:
1. **MF-001 surface area.** Run the ACTUAL `bench --campaign stress-curve`
   command end-to-end and feed its captured output through
   `plot_stress_curve.py`. Confirm or refute the schema-mismatch claim
   on a non-smoke invocation.
2. **`StopRule::check` priority on simultaneous trips.** Inject
   pathological inputs: `f64::NAN`, `f64::INFINITY`, `Duration::MAX`,
   negative wall (impossible but check), `Killed { signal: i32::MIN }`.
3. **`MemoryProbe` arithmetic edges.** What does `as_fraction_of_total`
   return for `bytes == u64::MAX` on a host with 8GB RAM? Reviewer
   already confirmed `f64` is finite; QA should verify the result is
   sensible (>1.0, finite, not NaN).
4. **Bash orchestrator under partial CSV.** Truncate `RAW_CSV` mid-row
   and confirm `--resume` either errors or recovers. The docs page
   §7 claims "the script refuses with exit 1" — verify.
5. **Bash interrupt handling.** §`d014_resume_invariant.rs` sends
   SIGINT mid-rep at 30s. QA: send SIGTERM, SIGKILL, SIGHUP. Does the
   trap fire correctly? Does the partial CSV remain re-resumable?
6. **`OOM_EXIT_CODES` drift.** The Rust constant `[137, -1073741801]`
   and the bash script's implicit knowledge of those codes are not
   structurally bound. Add a third OOM signature to the Rust constant
   and confirm the script does NOT pick it up — proving the drift
   risk.
7. **`StressCurveDescriptor::run_one_sequence` with `Env::DockerTcp`.**
   Confirm the `BenchError::Unsupported` is surfaced as
   `RelativistError::Config` at `commands.rs:326` with the right
   user-visible message (the test file `d014_stress_curve_descriptor.rs`
   does not exercise this branch).
8. **`stop_reason: Option<String>`.** Insert the literal string
   `"NotARealVariant"` and confirm the CSV writer accepts it without
   error — proving the type-safety gap of SF-001.

---

## Recommendation for Stage 6 (REFACTOR) — priorities, if/when invoked

If Stage 5 QA returns clean, REFACTOR can be skipped or used to apply
the small cleanups below. If QA finds correctness issues, REFACTOR
is needed regardless.

### High-value (do in same PR or follow-up)

1. **MF-001 follow-up TASK** (open as `TASK-0720`):
   - Add `--append-csv PATH` to `BenchArgs`.
   - Replace `println!` with `tracing::info!` and write a real
     stress-curve CSV per rep.
   - Reconcile the schema with `plot_stress_curve.py` — pick one
     of the two paths described above.
   - Add an end-to-end IT (binary → CSV → plot script).

2. **Pre-overnight smoke amendment** (1 commit):
   - Add the `--n-seq ""` empty-input guard (finding #8).
   - Add the `tracing::warn!` saturation log on `unwrap_or(u32::MAX)`
     (NTH-002).

### Medium-value (defer to v0.20.0-pre.2)

3. **SF-001 typed `stop_reason`.** ~30 LoC + serialize_with helper.
4. **SF-002 NaN guard in `StopRule::check`.** ~3 LoC + 1 unit test.
5. **NTH-001 here-doc for `print_help()`.** Trivial sh edit.
6. **NTH-004 single-source-of-truth for OOM exit codes.** A tiny
   `relativist debug oom-codes` subcommand the script consumes via
   `read`. ~20 LoC.

### Low-value (v2.1+)

7. **SF-003 `MemoryProbe` Backend trait refactor.** ~80 LoC churn for
   a maintainability gain that pays off only if a 3rd platform lands.
8. **Windows OOM IT equivalent** (close the 3 `cfg(unix)` floor gap).

---

## Passed Checks

- [x] No `unwrap()` in production code (the 2 found are pre-existing in
  `suite.rs:947`, untouched by this bundle).
- [x] No `unsafe` without `// SAFETY:` comment (3 unsafe blocks; all
  have SAFETY comments — `memory_probe.rs:194, 217`,
  `d014_resume_invariant.rs:80`).
- [x] No `unsafe` in production paths beyond required Win32 FFI in
  `memory_probe.rs`.
- [x] `thiserror` used for `BenchError` (no `anyhow`).
- [x] `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`
  on public types where appropriate (`MemoryProbe`, `StopReason`,
  `ChildExit`, `RepResult`, `SequenceOutcome`, `StressWorkload`,
  `Env`, `CampaignKind`, `StressWorkloadArg`, `StressEnvArg`).
- [x] `pub(crate)` minimization respected — most new public surface
  has reason to be reachable from binary path or shell-invoked tests.
- [x] All `pub` items have `///` doc-comments.
- [x] Module boundaries match SPEC-13 (no async leakage into core).
- [x] Core layer (`net/`, `reduction/`, `partition/`, `merge/`)
  untouched.
- [x] All 5 `BenchmarkResult` construction sites updated.
- [x] CSV schema additive (header sanity test
  `csv.rs:401-477` + `tests/spec09_bench_construction_memory_probe.rs:99-141`
  bumped from 29→33).
- [x] Floor verification cross-checked: developer's claim of 3
  `cfg(unix)`-gated tests is structurally correct.
- [x] v1 floor 690 inviolable.
- [ ] `cargo clippy --all-features -- -D warnings` clean (reviewer
  did not run; developer reports clean — accept on trust pending QA).
- [ ] `cargo fmt --check` clean (reviewer did not run; same).
- [ ] `shellcheck` clean (not run by developer; CICD agent follow-up
  per finding #5).

---

**End of review.**
