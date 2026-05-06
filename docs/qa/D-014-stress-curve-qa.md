# QA Review — D-014 Stress Curve Campaign

- **Bundle:** D-014 Stress Curve Campaign (Topic 1)
- **Branch:** `feature/stress-and-encoder` (HEAD `6e31878`)
- **Range under attack:** `bcff245..e066d3c` (9 commits, TASK-0700..0708)
- **Stage:** 5 (QA, post-Stage-4 reviewer review `docs/reviews/D-014-stress-curve-review.md`)
- **Reviewer:** qa agent (Camada 2 Relativist)
- **Date:** 2026-05-06

## Files reviewed

- `relativist-core/src/bench/memory_probe.rs` (357 LoC)
- `relativist-core/src/bench/stop_rule.rs` (347 LoC)
- `relativist-core/src/bench/suite.rs` lines 400-654 (BenchmarkResult sites) + 1060-1212 (StressCurveDescriptor)
- `relativist-core/src/bench/csv.rs` lines 80-142 (write_csv_detail)
- `relativist-core/src/bench/mod.rs` lines 270-297 (D-014 BenchmarkResult fields)
- `relativist-core/src/commands.rs` lines 287-339 (stress-curve dispatch)
- `relativist-core/src/config.rs` lines 660-750 (CLI flags + enums)
- `scripts/stress_curve.sh` (308 LoC)
- `scripts/plot_stress_curve.py` (238 LoC)
- `relativist-core/tests/d014_*.rs` (7 files, ~900 LoC total)
- `docs/benchmarks/campaigns/stress-curve.md` (244 LoC)
- `results/locked/v2_stress_curve_TEMPLATE/SENTINEL.md`
- `relativist-core/Cargo.toml` (deps + dev-deps)

## Bug verdict

**BUGS FOUND — 4 CRITICAL, 5 HIGH, 7 MEDIUM, 4 LOW = 20 total.**

This is unusually high for a Stage 5 QA pass. The single dominant defect (BUG-001 / MF-001 confirmed) cascades into multiple downstream gaps, but several independent defects exist (priority drift on memory + wall, `--n-seq` empty input, OOM-code coverage gaps, smoke-test self-skip masking, sentinel UX trap, Windows path divergence). The infrastructure is well-tested at the unit level — the integration seams between bash/Rust/Python are where the bugs cluster.

## Test coverage

ADEQUATE for individual components (StopRule, MemoryProbe, CSV roundtrip), GAPS at the integration boundary:
- No test exercises the **full** smoke-mode CSV against the **plot script's expected schema** end-to-end on a single host. The plot smoke and the orchestrator smoke each pass with their own synthetic inputs but the wire between them is broken (BUG-001).
- No test pins the `OOM_EXIT_CODES` constant against real systemd-managed OOM-killer behavior (Ubuntu 22+ uses `oom_kill_process` with a different signature).
- No test covers `--n-seq ""` empty CLI input (BUG-008).
- No test covers running the script with `kill -KILL` (vs SIGINT) — the trap is bypassed entirely.
- No test for plot script with locale = `pt_BR.UTF-8` (BUG-014).
- No test against UTF-8 BOM in CSV (the orchestrator has a real risk on Windows when redirecting stdout via PowerShell pipelines used as fallback for WSL-less hosts).

---

## Top 3 CRITICAL bugs

1. **BUG-001 (MF-001 confirmation):** `commands.rs:332-338` `println!` summary redirected by `stress_curve.sh:179-186` into `RAW_CSV` instead of CSV rows. The CSV file post-overnight will contain ~120 lines of `stress-curve outcome: completed_reps=N stop_reason=None last_attempted_n=Some(...)` debug strings, NOT a single CSV row. The plot script expects header `workload,env,workers,n,rep,wall_seconds,...` — none of which exist in the actual stdout. **7-8 hours of overnight wall-clock yields a zero-information CSV.**
2. **BUG-002 (CSV schema name divergence):** `csv.rs:87-94` writes header `benchmark,input_size,mode,workers,repetition,wall_clock_secs,...` but `plot_stress_curve.py:34-46` `REQUIRED_COLUMNS` expects `workload,env,workers,n,rep,wall_seconds,...`. Even if BUG-001 were fixed (Rust emitted CSV rows directly), the plot script would still exit 1 (`required column 'workload' missing`). The two surfaces independently agreed on different schemas. The IT-0703-01 roundtrip test asserts the writer produces `vmrss_peak_mb,vmrss_current_end_mb,stop_reason,cv_above_gate` at end of header, but never asserts the plot script can consume it — it never runs the plot script against a real bench-emitted CSV.
3. **BUG-003 (MemoryProbe per-rep VmHWM contamination — divergence Rust vs bash):** `run_one_sequence` at `suite.rs:1143-1208` uses ONE `MemoryProbe` instance for the entire sequence; `peak_bytes()` returns `VmHWM` which is monotonic non-decreasing across reps. Reps 2..N inherit rep 1's peak. The bash orchestrator works around this by fork-execing a fresh child per rep (line 179 `"$RELATIVIST_BIN" bench ... --n-seq "$N"` in a fresh process), but a Rust user calling `StressCurveDescriptor::run_one_sequence` directly (which `end_to_end_smoke_in_process` does as `IT-0707-06`) sees vmrss_peak monotonically rise — the smoke test asserts `vmrss_peak_bytes > 0` (passes), but the **value is wrong for rep 2 onward**. Documented nowhere in `docs/benchmarks/campaigns/stress-curve.md`. A Rust user assumes the API works; the API silently lies.

---

## Bugs Found (full list)

### BUG-001 — Stress-curve campaign produces garbage CSV (MF-001 EXPLOIT confirmed)

- **Severity:** CRITICAL
- **File:** `relativist-core/src/commands.rs:332-338` + `scripts/stress_curve.sh:171-186`
- **Category:** Logic Error (silent failure under nominal usage)

**Description:**
The orchestrator's per-rep loop captures the Rust binary's stdout into RAW_CSV:
```bash
"$RELATIVIST_BIN" bench --campaign stress-curve --workload "$WL" \
  --env in-process --workers "$WK" --reps "$REP" --n-seq "$N" \
  >>"$RAW_CSV" 2>"$STDERR_LOG"
```
But the Rust dispatch path emits exactly one debug line:
```rust
println!(
    "stress-curve outcome: completed_reps={} stop_reason={:?} last_attempted_n={:?}",
    outcome.completed_reps.len(),
    outcome.stop_reason,
    outcome.last_attempted_n,
);
```
A 1320-rep overnight run yields a RAW_CSV file with 1320 lines like `stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(10000)` — **no header, no per-rep wall, no MIPS, no vmrss, nothing the plot script understands**.

**Reproduction:**
```bash
cargo build --release
./target/release/relativist bench --campaign stress-curve \
  --workload ep_annihilation --env in-process --workers 2 \
  --n-seq 1000 --reps 1
# stdout: stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(1000)
```
Then run `scripts/stress_curve.sh --smoke --no-docker` and inspect `<outdir>/raw/in_process.csv`:
```
stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(1000)
stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(10000)
```
No CSV. The plot script `load_aggregated` reads `df.empty == False`, then iterates `REQUIRED_COLUMNS` — `workload` missing — exits 1. Because the bash orchestrator's plot phase swallows the error (`|| echo "WARN: plot script exited non-zero"`), the script exits 0 anyway. The smoke integration test (IT-0704-01) checks for `figures/*.pdf` existence (line 99) — but the script falls back to a 145-byte PDF stub (line 234-252) when matplotlib fails for ANY reason. **Smoke "passes" with a placeholder PDF and zero real data.**

**Expected behavior:**
- The Rust dispatch path emits a CSV row per rep (header on first invocation, data on each).
- Or the bash orchestrator generates the CSV itself by reading some structured output from the binary (JSON line, exit code + side-channel file, etc.).
- Or the per-rep child writes its own CSV row to a known path that the orchestrator concatenates.

**Actual behavior:** Silent garbage. Operator wakes up to a CSV full of `stress-curve outcome:` strings and a placeholder PDF. 7-8 hours of compute wasted.

**Fix suggestion (DO NOT IMPLEMENT — assigned to developer Stage 6):**
Add a `--csv-out <path>` flag to the `bench --campaign stress-curve` path; the dispatch code in `run_bench_command` writes a CSV header on first row and a per-rep data row using the `write_csv_detail` writer (or a dedicated stress-curve writer). The orchestrator passes `--csv-out "$RAW_CSV"` once per child invocation, with `--append` on second-onwards. The plot script's required column NAMES must match the writer's column NAMES — this is BUG-002.

---

### BUG-002 — CSV column-name divergence between writer and plot script

- **Severity:** CRITICAL
- **File:** `relativist-core/src/bench/csv.rs:87-94` vs `scripts/plot_stress_curve.py:34-46`
- **Category:** Schema/Contract Mismatch

**Description:**
Plot script REQUIRED_COLUMNS list:
```
workload, env, workers, n, rep, wall_seconds, mips,
vmrss_peak_mb, vmrss_current_end_mb, stop_reason, cv_above_gate
```
CSV writer header:
```
benchmark, input_size, mode, workers, repetition, wall_clock_secs, ...
```
Of 11 required columns, only 4 match by name (`workers`, `vmrss_peak_mb`, `vmrss_current_end_mb`, `cv_above_gate`, `stop_reason`). The other 6 (`workload` vs `benchmark`, `env` vs `mode`, `n` vs `input_size`, `rep` vs `repetition`, `wall_seconds` vs `wall_clock_secs`) are name mismatches. `mips` is in both, but mappable.

**Reproduction:**
Even with BUG-001 fixed (assume Rust emits a real `write_csv_detail` row per invocation):
```bash
python3 scripts/plot_stress_curve.py --input <(bench-output.csv) --output-dir /tmp/figs
# ERROR: required column 'workload' missing from <input>
# exit code 1
```
The IT-0705-01 plot smoke test passes because it FEEDS A SYNTHETIC CSV with the plot-script schema (line 51-65) — it never tries to read what `write_csv_detail` emits.

**Expected behavior:** Single source of truth for column names. Either (a) the writer renames columns to match the plot script, or (b) the plot script reads the canonical 33-column schema.

**Actual behavior:** Two surfaces silently disagree. IT-0703-01 checks the writer; IT-0705-01 checks the reader; nothing checks the joint contract.

**Fix suggestion:** Add an integration test that emits a real `write_csv_detail` row from `BenchmarkResult` and feeds it to the plot script. This test would have caught BUG-001 + BUG-002 simultaneously.

---

### BUG-003 — `MemoryProbe::peak_bytes` is per-process monotonic; reps 2..N see rep 1's VmHWM

- **Severity:** CRITICAL
- **File:** `relativist-core/src/bench/suite.rs:1143-1208`
- **Category:** Cross-platform contract / Documentation gap

**Description:**
`MemoryProbe::peak_bytes()` returns `VmHWM` (Linux) or `PeakWorkingSetSize` (Windows). Both are **monotonic non-decreasing per-process**. In `run_one_sequence`, ONE probe instance is constructed at line 1143; `probe.peak_bytes()` is called at line 1194 after EACH rep. After rep 1 with N=1k allocates ~50 MiB, VmHWM = 50 MiB; rep 2 with N=10k allocates ~500 MiB → VmHWM = 500 MiB (correct). But if rep 2 with smaller N (descending sweep, hypothetical) allocated only 30 MiB, the probe still reports 500 MiB. Worse: for a sweep WITHIN one rep where memory drops between phases, the StopRule will MISDIAGNOSE which N triggered the threshold.

The bash orchestrator (line 179) sidesteps this by fork-execing a FRESH process per (workload, W, N, rep) tuple, so VmHWM resets. The Rust API does NOT.

The methodology page (`docs/benchmarks/campaigns/stress-curve.md`) does NOT document this asymmetry. A Rust user reading `StressCurveDescriptor::run_one_sequence` and assuming "this is what the campaign actually does" will see misleading vmrss values.

**Reproduction:**
```rust
let outcome = StressCurveDescriptor::run_one_sequence(
    StressWorkload::EpAnnihilation, Env::InProcess, 1, 1,
    Some(&[10_000, 1_000]),  // descending — exercises the bug
    None,
)?;
// outcome.completed_reps[1].vmrss_peak_bytes will == reps[0].vmrss_peak_bytes
// — because VmHWM watermark from rep[0] is still in effect.
```

**Expected behavior:** Either:
- Document explicitly in `docs/benchmarks/campaigns/stress-curve.md` §9 limitations: "Rust-API users (run_one_sequence): `vmrss_peak_bytes` is the cumulative process VmHWM, NOT per-rep. Reps after the first see the watermark of the largest preceding allocation. Use the bash orchestrator for per-rep isolation."
- OR: Make `MemoryProbe` track a baseline at start-of-rep and report `peak - baseline` so each rep's peak is delta-from-baseline.

**Actual behavior:** Silent contamination. Tests pass because the smoke uses ascending N (`[1k, 10k]`) where the watermark IS the rep 2 peak by coincidence.

**Severity rationale:** Critical because (a) the Rust API is in a public module (`pub fn run_one_sequence`), (b) the smoke test makes it look correct, (c) the design doc and methodology don't mention the limitation. A future TCC reader who reproduces "the same campaign" via the Rust API gets different results than the bash run.

---

### BUG-004 — Bash trap on SIGINT exits the parent but does NOT kill the in-flight child rep

- **Severity:** CRITICAL
- **File:** `scripts/stress_curve.sh:71-77`
- **Category:** Race condition / Resource leak

**Description:**
The trap handler is:
```bash
ON_INTERRUPT=0
on_interrupt() {
    ON_INTERRUPT=1
    echo "INTERRUPT received; partial output preserved at $OUTPUT_DIR" >&2
    exit 10
}
trap on_interrupt INT TERM
```
When the user hits Ctrl+C in the terminal, the SIGINT goes to the entire foreground process group (bash + relativist child) — both die. **But** if SIGINT is delivered ONLY to the bash PID (e.g., `kill -INT <bash_pid>` from another terminal, or in `IT-0707-05` resume-invariant test which does exactly that at line 82: `unsafe { libc::kill(pid, libc::SIGINT) };`), the bash trap fires and `exit 10` returns immediately — but the in-flight `$RELATIVIST_BIN bench ...` child is **still running**, attached to the parent's now-dead PID. It becomes an orphan reparented to init/systemd, continues consuming CPU + RAM, and may write a truncated row to RAW_CSV after the resume invariant test reads the file at line 96-97.

The IT-0707-05 test catches this only by chance — at line 83 `let _ = child.wait();` waits for the bash to exit (which it does immediately on the trap). But the orphaned `relativist` child may still be running after `wait()` returns. The race is: if the orphan finishes its `>>$RAW_CSV` redirect AFTER the resume invariant test reads the file (line 96-97), the resume CSV will have an extra row vs. the reference. The test currently passes in CI because the smoke's N=[1k, 10k] reps complete in ~0.1s — much faster than the post-SIGINT cleanup window — but on a slower machine or larger N this races.

**Worse:** the trap does NOT remove a lockfile or partial-row marker. There is no lockfile mechanism at all. Two concurrent `--resume` invocations on the same `--output-dir` would race at `RAW_CSV` append.

**Reproduction:**
```bash
# Terminal 1
scripts/stress_curve.sh --output-dir /tmp/d014run

# Terminal 2 (during a rep)
kill -INT $(pgrep -f "stress_curve.sh")
# bash exits with code 10; relativist child reparents to init.
ps aux | grep relativist  # still running for tens of seconds.

# Inspect /tmp/d014run/raw/in_process.csv
# may or may not have a partial trailing row
```

**Expected behavior:**
- Trap should `kill $CHILD_PID` first (track the most recent child PID), wait for it (with a 5s timeout), then exit.
- Acquire a flock on `<outdir>/.lock` at start; release on exit. Refuse `--resume` if the lock exists (operator must verify no orphans exist).

**Actual behavior:** Trap exits parent; orphan relativist may corrupt CSV; no lock; concurrent `--resume` would race.

**Fix suggestion:**
```bash
on_interrupt() {
    ON_INTERRUPT=1
    if [[ -n "${CHILD_PID:-}" ]]; then
        kill "$CHILD_PID" 2>/dev/null || true
        wait "$CHILD_PID" 2>/dev/null || true
    fi
    echo "INTERRUPT received; partial output preserved at $OUTPUT_DIR" >&2
    exit 10
}
# In the rep loop:
"$RELATIVIST_BIN" bench ... >>"$RAW_CSV" 2>"$STDERR_LOG" &
CHILD_PID=$!
wait "$CHILD_PID"
EC=$?
```

---

### BUG-005 — `OOM_EXIT_CODES` does NOT cover modern Linux OOM-killer signatures

- **Severity:** HIGH
- **File:** `relativist-core/src/bench/stop_rule.rs:74-83`
- **Category:** Cross-platform coverage gap

**Description:**
```rust
pub const OOM_EXIT_CODES: &[i32] = &[137, -1073741801];
```
Coverage analysis vs. real-world OOM behaviors:

| Platform / mechanism | Real exit signature | Covered? |
|---|---|---|
| Linux kernel OOM-killer (default) | `signal: 9` (SIGKILL) | YES via SIGKILL_SIGNUM check |
| Linux bash subshell wrapping the killed proc | exit 137 (= 128 + 9) | YES |
| Linux systemd-managed unit with `OOMScoreAdjust=1000` + `OOMPolicy=kill` | exit 137 OR `SIGTERM` (signal 15) per `man systemd-oomd` | **PARTIAL** (137 yes, SIGTERM no) |
| Linux cgroup v2 `memory.events.oom_kill` triggered | `signal: 9` typically, but can also reap with `OOMSCORE_ADJ` → `SIGKILL → ECHILD` | YES |
| Windows STATUS_NO_MEMORY (0xC0000017) | i32 = -1073741801 | YES |
| Windows job-object with JOB_OBJECT_LIMIT_PROCESS_MEMORY exceeded | exit `STATUS_QUOTA_EXCEEDED` (0xC0000044) = -1073741756 | **NO** |
| Windows job-object terminated by parent (kill_on_close) | exit code 1 (ERROR_INVALID_FUNCTION-like) — ambiguous | **NO** |
| WSL2 OOM-killer | depends on Win+Linux interaction; sometimes SIGKILL, sometimes exit 35 (ENOMEM via panic) | **PARTIAL** |
| Rust `alloc::handle_alloc_error` default abort | `SIGABRT` (signal 6) | **NO** (false negative — looks like generic crash) |
| Linux kernel with `vm.oom_kill_allocating_task=1` + `panic_on_oom=1` | kernel panic; child sees nothing | NA |

The constant is described as covering "Linux + macOS bash-mediated 137" and "Windows STATUS_NO_MEMORY". macOS is out of scope per memory_probe.rs. So coverage is Linux + Windows. But:
- **Windows job objects**: STATUS_QUOTA_EXCEEDED = `-1073741756` is missing. If the campaign ever runs in a constrained job (Hyper-V container, `ProcessThreadAttribute`, LXSS), OOM looks like a generic exit.
- **Rust panic on alloc**: `alloc::handle_alloc_error` defaults to `process::abort()` → SIGABRT (signal 6). The current code only treats SIGKILL as OOM (`signal == SIGKILL_SIGNUM` at line 97). A panic-aborted child with SIGABRT is misclassified as a generic crash, the StopRule does NOT trip OOM, and the sequence continues to the next N — which then ALSO panics — until wall budget catches it (5 min later).

**Reproduction:**
On a Linux host with `ulimit -v 100000` (100 MiB virtual address):
```bash
ulimit -v 100000
target/release/relativist bench --campaign stress-curve \
  --workload ep_annihilation --env in-process --workers 1 --n-seq 1000000
# alloc may fail; depending on the workload either SIGSEGV or SIGABRT.
# Neither is in OOM_EXIT_CODES → StopRule misses → next N attempted.
```

**Expected behavior:** Add to `OOM_EXIT_CODES`: `-1073741756` (Windows STATUS_QUOTA_EXCEEDED), and add SIGABRT (6) to a separate `MEMORY_RELATED_SIGNALS` const (alongside SIGKILL=9). Document the chosen coverage explicitly.

**Actual behavior:** Linux kernel OOM-killer + bash-mediated 137 covered. Windows job-object OOM, Rust alloc-abort, and systemd-oomd SIGTERM are missed.

**Fix suggestion:**
```rust
pub const OOM_EXIT_CODES: &[i32] = &[
    137,            // bash-mediated SIGKILL
    -1073741801,    // STATUS_NO_MEMORY (0xC0000017)
    -1073741756,    // STATUS_QUOTA_EXCEEDED (0xC0000044) — Windows job-object
];
pub const OOM_SIGNALS: &[i32] = &[9, 6];  // SIGKILL, SIGABRT (Rust alloc abort)

// In check():
ChildExit::Killed { signal } if OOM_SIGNALS.contains(&signal) => {
    return Some(StopReason::Oom);
}
```

---

### BUG-006 — Smoke "self-skip" mechanism masks BUG-001 by accepting placeholder PDFs

- **Severity:** HIGH
- **File:** `scripts/stress_curve.sh:231-253` + `relativist-core/tests/d014_stress_curve_smoke.rs:108-111`
- **Category:** Test-coverage gap (anti-pattern)

**Description:**
When `python3` lacks matplotlib, the orchestrator (line 215-229) skips the real plot phase. In smoke mode, line 231-253 then **fabricates** a 145-byte stub PDF:
```
%PDF-1.4
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Count 1/Kids[3 0 R]>>endobj
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]>>endobj
xref
...
```
The smoke integration test (IT-0704-01) line 108-111 asserts:
```rust
assert!(!pdfs.is_empty(),
    "figures/ must contain at least 1 PDF (smoke placeholder is acceptable when matplotlib is unavailable)");
```
So the smoke passes even when (a) the CSV is garbage, (b) the plot script never ran, and (c) the only PDF is a 1-page blank stub. Combined with BUG-001 + BUG-002, `scripts/stress_curve.sh --smoke` exits 0 on a system that produces ZERO real data.

**Reproduction:**
```bash
# Run smoke on a host without matplotlib (or with it, but where BUG-002 makes the plot fail anyway)
scripts/stress_curve.sh --smoke --no-docker
echo $?  # 0
ls /tmp/.../figures/  # smoke_placeholder.pdf (145 bytes, 1 page, blank)
cat /tmp/.../raw/in_process.csv  # garbage from BUG-001
```

**Expected behavior:**
- The smoke test should assert at least one REAL plot PDF (size > 5 KB indicating actual matplotlib output) when matplotlib is available.
- The placeholder PDF mechanism should be explicitly opt-in via a `--allow-placeholder` flag, NOT silently fall back. A campaign whose smoke produces a blank PDF is broken.
- The orchestrator should propagate the plot script's exit code; line 222-225 currently swallows it (`|| echo "WARN: plot script exited non-zero"`).

**Actual behavior:** The smoke test passes regardless of the actual plot or CSV correctness, providing false confidence to anyone running it.

---

### BUG-007 — `as_fraction_of_total(u64::MAX)` returns finite f64 > 1.0; `StopRule::check` accepts it as memory-trip but operator may interpret as bug

- **Severity:** MEDIUM
- **File:** `relativist-core/src/bench/memory_probe.rs:116-124`
- **Category:** Edge case / Documentation gap

**Description:**
```rust
pub fn as_fraction_of_total(&self, bytes: u64) -> f64 {
    if self.total_ram_bytes == 0 { return 0.0; }
    bytes as f64 / self.total_ram_bytes as f64
}
```
The defensive `total_ram_bytes == 0` branch DOES protect against NaN. So the original adversarial test from the reviewer (BUG: NaN bypass) is **REFUTED** — line 117-121 makes NaN unreachable.

However, if `bytes == u64::MAX` (e.g., from an arithmetic-overflow upstream), the result is `~1.84e10` — a finite f64 > 1.0. `StopRule::check` at line 107 sees `1.84e10 > 0.80`, returns `Some(MemoryExceeded)`. **Functionally fine.** But the CSV row will have `vmrss_peak_mb = u64::MAX / (1024*1024) = ~17_592_186_044_416.0` — a meaningless number that the plot script's log-scale axis will draw at the top of the figure and the `cv_above_gate` flag won't catch (it's about std/mean of wall_clock, not vmrss).

**Reproduction:**
```rust
let probe = MemoryProbe::new()?;
probe.as_fraction_of_total(u64::MAX); // ~1.84e10
```
Verified by UT-0700-04 line 351-355 (test asserts `is_finite()` but doesn't assert `<= 1.0`).

**Expected behavior:** `as_fraction_of_total` should saturate at some sane value (1.0 + epsilon, or u64-aware clamp). Even if the StopRule still trips, a CSV cell value of `17592186044416` is misleading. The contract docstring on line 109-115 says "Passing values larger than `total_ram_bytes` returns a finite f64 > 1.0 (no saturation; saturation is the caller's contract)" — the caller (`run_one_sequence:1200`) does NOT saturate. **Documentation matches code; both are wrong.**

**Fix suggestion:** Saturate inside `as_fraction_of_total` to `min(2.0, bytes/total)` and document the cap. Alternatively, make `peak_bytes()` return `Result<u64, BenchError>` for over-u32 values on Windows where `pmc.PeakWorkingSetSize` is `usize` (not u64) and could be wrapped on 32-bit Windows targets. But Windows 32-bit is out of scope per the Cargo.toml target gate.

---

### BUG-008 — `--n-seq ""` empty input → trivial run with success exit code

- **Severity:** MEDIUM
- **File:** `relativist-core/src/config.rs:697-698` + `commands.rs:316-323`
- **Category:** UX / Edge case

**Description:**
The CLI flag `--n-seq` uses `value_delimiter = ','`. If the user passes `--n-seq ""`, clap parses it to `Some(vec![])` (empty vector). The dispatch at `commands.rs:316`:
```rust
let n_seq_owned = args.n_seq.clone();
// ...
StressCurveDescriptor::run_one_sequence(..., n_seq_owned.as_deref(), None)
```
`as_deref()` on `Some(vec![])` is `Some(&[])`. Then in `run_one_sequence:1145`:
```rust
let n_seq: &[usize] = n_seq_override.unwrap_or(STRESS_CURVE_N_SEQ);
```
`unwrap_or(STRESS_CURVE_N_SEQ)` on `Some(&[])` returns `&[]` — NOT the canonical 11-point sweep. So `--n-seq ""` produces an empty `n_seq`, `run_sequence` returns `SequenceOutcome { completed_reps: vec![], stop_reason: None, last_attempted_n: None }`, the binary prints `stress-curve outcome: completed_reps=0 stop_reason=None last_attempted_n=None`, and exits 0.

The bash orchestrator does not pass `--n-seq ""` directly, but if a user mis-edits the script (or types the command interactively with autocomplete that strips a value), the smoke "succeeds" with zero reps.

**Reproduction:**
```bash
target/release/relativist bench --campaign stress-curve \
  --workload ep_annihilation --env in-process --workers 1 --n-seq ""
echo $?  # 0
# stdout: stress-curve outcome: completed_reps=0 stop_reason=None last_attempted_n=None
```

**Expected behavior:** Empty `n_seq` should be a CLI validation error: "stress-curve --n-seq must be non-empty (or omit the flag for the canonical 11-point sweep)".

**Actual behavior:** Trivial silent-success.

**Fix suggestion:**
```rust
// in commands.rs after parsing
if let Some(ref n) = args.n_seq {
    if n.is_empty() {
        return Err(RelativistError::Config(
            "--n-seq cannot be empty (omit the flag for canonical sweep)".into()
        ));
    }
}
```

---

### BUG-009 — `--n-seq` accepts arbitrarily large `usize` and silently truncates to `u32::MAX` in `run_one_sequence`

- **Severity:** MEDIUM
- **File:** `relativist-core/src/bench/suite.rs:1162`
- **Category:** Silent truncation

**Description:**
```rust
let size: u32 = n.try_into().unwrap_or(u32::MAX);
```
`STRESS_CURVE_N_SEQ` includes `1_000_000_000` (10⁹) which fits in u32 (~4.29e9 max). But a user passing `--n-seq 5000000000` (5e9 > u32::MAX) gets silent truncation to u32::MAX = 4_294_967_295. The CSV row says `n=5000000000` (the original `usize`) but the suite actually ran with size=4_294_967_295 — silent disagreement.

**Reproduction:**
```bash
# 64-bit host
target/release/relativist bench --campaign stress-curve \
  --workload ep_annihilation --env in-process --workers 1 --n-seq 5000000000
# Internally truncates to u32::MAX; outcome.completed_reps[0].n == 5_000_000_000.
# Wall time + vmrss correspond to N=4_294_967_295 not N=5e9.
```

**Expected behavior:** `try_into()` failure should return a CLI error, not silently truncate.

**Actual behavior:** Silent truncation; CSV n column reports the wrong N.

**Fix suggestion:** `let size: u32 = n.try_into().map_err(|_| BenchError::...)?;` and propagate.

---

### BUG-010 — Sentinel directory `_TEMPLATE` will coexist with the real `<DATE>` directory; checksums won't drop the template

- **Severity:** MEDIUM
- **File:** `results/locked/v2_stress_curve_TEMPLATE/SENTINEL.md`
- **Category:** UX trap / repo hygiene

**Description:**
The sentinel directory is committed at `results/locked/v2_stress_curve_TEMPLATE/`. After the operator runs the campaign per SENTINEL.md §"How to invoke", they create `results/locked/v2_stress_curve_2026-05-12/` (or whatever date) and `git add` it. The instructions in SENTINEL.md line 91-93:
> This sentinel directory itself can be removed in the same commit
> that lands the locked directory (it serves no purpose afterwards).
**say the operator MAY remove it but does not enforce.** A fatigued operator running an overnight at 6am will likely forget; the result is BOTH `_TEMPLATE/SENTINEL.md` and `2026-05-12/MANIFEST.md` in `results/locked/`. `docs/INDEX.md` will need both entries; reviewers don't know which is "real"; `git log results/locked/v2_stress_curve*/` shows two unrelated commits.

Also: the SENTINEL.md instructions §3 use `$(date -I)` which on a host with locale `LC_TIME=pt_BR.UTF-8` may return `2026-05-12` (works) but on a host with LANG=C and `date -I` unsupported (BusyBox) may print `--help` to stderr — the command `mkdir -p results/locked/v2_stress_curve_$(date -I)` then creates the literal directory `v2_stress_curve_` with empty date. `find raw figures` checksums silently produce nothing.

**Reproduction:**
Run the script per SENTINEL.md instructions. Forget step 91-93. Examine repo state:
```
results/locked/v2_stress_curve_TEMPLATE/
results/locked/v2_stress_curve_2026-05-12/
```
Both are tracked. Future readers searching for "the locked dataset" find two; the README/INDEX has to disambiguate.

**Expected behavior:** Either:
- The sentinel directory should self-delete on first successful campaign run (the orchestrator unlinks `results/locked/v2_stress_curve_TEMPLATE/` if it exists and a fresh dataset just landed).
- Or the orchestrator should refuse to start if `results/locked/v2_stress_curve_TEMPLATE/` exists at the same level as the new output dir (forcing the operator to address it first).
- Or the sentinel should live elsewhere (e.g., `docs/benchmarks/SENTINEL-v2-stress-curve.md`) and not pollute the `results/locked/` tree where readers expect ONLY locked datasets.

**Actual behavior:** UX hazard with no enforcement.

---

### BUG-011 — `--resume` parses CSV via positional `cut -d,` but the actual CSV is the BUG-001 stdout summary, not a CSV at all

- **Severity:** MEDIUM (becomes HIGH if BUG-001 fixed but resume parsing not updated)
- **File:** `scripts/stress_curve.sh:138-148`
- **Category:** Logic Error / Coupling to broken format

**Description:**
```bash
while IFS=, read -r line; do
    wl=$(echo "$line" | cut -d, -f1)
    sz=$(echo "$line" | cut -d, -f2)
    wk=$(echo "$line" | cut -d, -f4)
    rp=$(echo "$line" | cut -d, -f5)
    if [[ -n "$wl" && "$wl" != "benchmark" ]]; then
        DONE["${wl}|in_process|${wk}|${sz}|${rp}"]=1
    fi
done <"$RAW_CSV"
```
- Today (BUG-001 unfixed): RAW_CSV contains lines like `stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(1000)`. `cut -d, -f1` → `stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(1000)` (no comma); `cut -f4` is empty. The DONE map fills with one entry: `stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(1000)|in_process||...`. The skip predicate `${DONE[$key]:-}` will never match because the real keys are `ep_annihilation|in_process|2|1000|1`. **Resume always re-runs everything.** OK, so today the bug is benign in effect (re-run is the right behavior given garbage input). But:
- After BUG-001 is fixed and the CSV becomes a real CSV with header `benchmark,input_size,mode,workers,repetition,...`: `f1=benchmark`, `f2=input_size`, `f4=workers`, `f5=repetition`. The DONE-key build expects `wl|in_process|wk|sz|rp`, but `wl` is now `ep_annihilation` (good) and the other fields ARE correct — so it would work — IF the column positions didn't change. But `csv.rs:87-94` puts `mode` at f3 and `workers` at f4; the script reads `wk=$(cut -f4)` which is `workers` — correct (f4 in the CSV is `workers`, line 4 of the comma-list). But the DONE-key uses `in_process` as a hardcoded literal — assuming docker is f4 `mode`. So if a future `--env docker_tcp` rep is added, the resume key build does NOT distinguish it (the literal `in_process` is wrong for docker rows).

**Reproduction:** After BUG-001 fix, run script with both `--workloads ep_annihilation` and Docker phase enabled, kill mid-run, re-resume — Docker reps NOT skipped (they have `mode=tcp_localhost` not `in_process`).

**Expected behavior:** Read the CSV with `csv` crate semantics (handles quoting, escapes), or at minimum read the actual `mode` column from `cut -f3` and use it in the key.

**Actual behavior:** Hardcoded `in_process` substring breaks when phases are mixed.

---

### BUG-012 — `--resume` does not detect a truncated mid-row CSV (despite docs claiming it does)

- **Severity:** MEDIUM
- **File:** `scripts/stress_curve.sh:135-149` + `docs/benchmarks/campaigns/stress-curve.md:181-183`
- **Category:** Documentation drift / silent corruption

**Description:**
The methodology doc claims:
> **A truncated mid-row CSV is detected and the script refuses with exit 1** — the operator must `tail` the malformed CSV, manually remove the partial row, and re-invoke `--resume`.

The script does NOT do this. The `--resume` parser at line 138-148 just reads each line via `IFS=, read -r line` and extracts cells with `cut`. A truncated last row (e.g., `ep_annihilation,1000,sequential,1` cut off mid-write because the Rust child was killed) produces `wl=ep_annihilation`, `sz=1000`, `wk=` (empty cut -f4 because field 4 doesn't exist on the truncated line), `rp=` (empty). The DONE entry is `ep_annihilation|in_process||1000|` — which won't match any real run, so the rep gets re-run (acceptable), BUT the truncated line stays in RAW_CSV. After resume, the CSV contains:
```
ep_annihilation,1000,sequential,1                <- TRUNCATED
ep_annihilation,1000,sequential,1,0,true,0.5,...  <- redo
```
Plot script reads BOTH rows. Truncated row has missing values for many columns; pandas may interpret `0.5` as `wall_seconds` for the first row when the column position is off — corrupted analysis.

The IT-0707-05 resume invariant test does NOT exercise truncated rows (kill SIGINT happens between reps, so writes are atomic-line — the rep completes a write, then the next iter starts and gets killed before its write begins). The actual mid-row truncation case is NOT tested.

**Reproduction:**
```bash
# Manually truncate a CSV
printf "benchmark,input_size,mode,workers,repetition\nep_annihilation,1000,sequential,1" > /tmp/d014/raw/in_process.csv  # NO trailing newline
scripts/stress_curve.sh --smoke --no-docker --resume --output-dir /tmp/d014
# Script does NOT exit 1; appends new rows; truncated row stays.
```

**Expected behavior:** Either implement the documented behavior (detect truncated row, exit 1) or remove the lie from the doc.

**Actual behavior:** Doc lies; script proceeds with corrupted CSV.

---

### BUG-013 — `set -euo pipefail` + `set +e` toggle around the rep call leaves `pipefail` unchanged, so a midpipe SIGPIPE gets silently absorbed

- **Severity:** LOW
- **File:** `scripts/stress_curve.sh:178-188`
- **Category:** Bash trap subtlety

**Description:**
```bash
set +e
"$RELATIVIST_BIN" bench ... >>"$RAW_CSV" 2>"$STDERR_LOG"
EC=$?
set -e
```
`set +e` disables errexit but does NOT touch `pipefail`. There's no pipe here so it's moot. But the docstring "/the relativist binary's --campaign stress-curve currently prints a stdout summary" (BUG-001) is in the script as a comment — when BUG-001 is fixed, the call site may grow a pipe (`| awk`, `| tee`, etc.) and `pipefail` will turn the absorbed SIGPIPE into an exit-1 → script abort. Document the intent.

**Severity:** LOW because the current site has no pipe. Flagging for forward-compat.

---

### BUG-014 — Plot script `pandas.read_csv` is locale-sensitive for decimal point

- **Severity:** LOW (medium on pt-BR/de-DE hosts)
- **File:** `scripts/plot_stress_curve.py:74`
- **Category:** Locale / Reproducibility

**Description:**
`pd.read_csv(path)` defaults to `decimal='.'` and `thousands=None`. The Rust writer at `csv.rs:103` uses `{:.6}` formatting which always emits `.` regardless of locale (good). BUT: if the operator runs the campaign on a host with `LC_NUMERIC=pt_BR.UTF-8` and re-saves the CSV through Excel or a locale-aware tool before plotting, the wall_clock_secs cell becomes `0,123456` (comma decimal). Plot script then reads `0,123456` as a string, `.errorbar` gets a non-numeric series, raises a TypeError. The error message is cryptic ("could not convert string to float").

The methodology doc does NOT warn about this. The orchestrator does not pin `LC_NUMERIC=C` for the python invocation.

**Fix suggestion:** Add `LC_NUMERIC=C python3 ...` to the orchestrator at line 222.

---

### BUG-015 — `Stop rule timing race`: when wall AND memory both trip, priority order is `Memory > Wall` — but a wall-time-driven OOM (wall blew up because allocator is thrashing) gets misclassified

- **Severity:** LOW
- **File:** `relativist-core/src/bench/stop_rule.rs:94-117`
- **Category:** Diagnostic accuracy

**Description:**
The priority `Oom > Memory > Wall` (line 7-8) is documented and tested (UT-0701-04). However: a rep that genuinely went OOM but the kernel killed the wrong process (e.g., a sibling Docker container) leaves the relativist child still running, RSS plateaued near the threshold, wall budget exceeded due to thrashing. The rule reports `WallTimeExceeded`. Operator looks at the row, says "increase the wall budget", reruns; rep OOMs again. Loop. The rule should also surface "memory near the gate AND wall exceeded" as a third diagnostic state, e.g., `MemoryThrash`. But adding a fourth StopReason invalidates the IT-0703 schema test (it counts variants implicitly).

**Severity:** LOW — operator can read the vmrss column to disambiguate. Flagging as feature gap not bug.

---

### BUG-016 — `cv_above_gate` is hardcoded `false` at all writer sites; never set to `true` anywhere

- **Severity:** MEDIUM
- **File:** `relativist-core/src/bench/suite.rs:450, 653` + `mod.rs:867`
- **Category:** Dead column / Documentation drift

**Description:**
`docs/benchmarks/campaigns/stress-curve.md:65-68` advertises:
> `cv_above_gate` | bool | `(stddev/mean) > 0.05` flag from the post-rep aggregator

Grepping the codebase, `cv_above_gate` is set at:
- `bench/suite.rs:450` → `false`
- `bench/suite.rs:653` → `false`
- `bench/mod.rs:867` (test helper) → `false`
- `bench/csv.rs:359` (test helper) → `false`
The aggregation step in `aggregate()` at suite.rs:658-686 computes `cv = stats::coeff_of_variation(&times)` and stores it in `AggregatedStats` — but never propagates a per-row `cv_above_gate` boolean back into individual `BenchmarkResult`s. **The column is always `false`.** Plot script's `cv_flagged` annotation (`'*'` suffix on labels) NEVER fires.

**Reproduction:** Run any campaign, grep `cv_above_gate=true` in the resulting CSV — zero matches.

**Expected behavior:** After aggregation, set `cv_above_gate` on each row that contributed to a CV > 0.05 group; OR remove the column from the schema and the docs.

**Fix suggestion:** Add a post-aggregate pass that mutates `BenchmarkResult.cv_above_gate` based on the corresponding `AggregatedStats.cv > 0.05` decision.

---

### BUG-017 — `MemoryProbe::new()` panics-equivalent on macOS (returns Err); but unit test `fraction_of_total_in_unit_interval` line 333 returns silently — masking macOS coverage gap

- **Severity:** LOW
- **File:** `relativist-core/src/bench/memory_probe.rs:329-356`
- **Category:** Test discipline / coverage gap

**Description:**
```rust
let probe = match MemoryProbe::new() {
    Ok(p) => p,
    Err(_) => return, // macOS / unsupported — nothing to assert
};
```
On macOS this silently passes by returning. The CI matrix doesn't include macOS hosts (per Cargo.toml + methodology), so this is fine in practice. But the test is named `fraction_of_total_in_unit_interval` — its body asserts nothing on macOS. A future contributor adding macOS support might assume the test covers macOS too.

**Fix suggestion:** Wrap the test body in `#[cfg(not(target_os = "macos"))]` so macOS literally doesn't compile this test, signaling the coverage gap explicitly.

---

### BUG-018 — `MemoryProbe::current_bytes` is NOT thread-safe-documented; concurrent calls from multiple threads each `read_to_string("/proc/self/status")` independently

- **Severity:** LOW
- **File:** `relativist-core/src/bench/memory_probe.rs:62-83, 134-150`
- **Category:** Concurrency / documentation

**Description:**
`/proc/self/status` is a per-fd kernel-managed pseudo-file. The kernel snapshots the values on `open()`; concurrent reads from different threads each get THEIR OWN snapshot — **safe** but slightly different timepoints (kernel renders the file lazily on read, not at open). On Windows, `GetProcessMemoryInfo` is documented thread-safe.

So MemoryProbe IS thread-safe. But `Clone` derives a fresh `MemoryProbe { total_ram_bytes }` cache without re-reading meminfo — a parent thread's probe shared via Clone with a child thread is fine. No bug, but the docstring on line 22-26 doesn't say so. A reader would have to dig.

**Fix suggestion:** Add `// thread-safety: All methods are thread-safe; the kernel snapshots /proc/self/status per-read.` to the struct docstring.

---

### BUG-019 — `IT-0707-04` synthetic SIGKILL fallback masks the real OOM-killer test on hosts without python3

- **Severity:** LOW
- **File:** `relativist-core/tests/d014_stop_rule_oom.rs:71-75`
- **Category:** Test integrity

**Description:**
```rust
} else {
    eprintln!("INFO IT-0707-04: python3 not available; using synthetic SIGKILL");
    ChildExit::Killed { signal: 9 }
};
```
On a host without `python3`, the test hardcodes `ChildExit::Killed { signal: 9 }` and asserts `rule.check(&r) == Some(Oom)` — which is **the same test as `UT-0701-03`** at unit level. The IT layer doesn't add coverage; it's a tautology. The CI only sees the unit-test guarantee, not real OOM behavior.

**Fix suggestion:** When python3 is unavailable, mark the test as `#[ignore]` rather than running a tautology — or use a Rust-only OOM trigger like `Vec::with_capacity(usize::MAX)` (panics on alloc; SIGABRT, not SIGKILL — also exposes BUG-005).

---

### BUG-020 — `commands.rs:333-337` `println!` uses `Debug` formatter on `Option<StopReason>`, so the CSV (per BUG-001) sees `Some(MemoryExceeded)` not `MemoryExceeded`

- **Severity:** LOW (subset of BUG-001)
- **File:** `relativist-core/src/commands.rs:333-337`
- **Category:** Format/parser drift

Even if BUG-001 is partially fixed (e.g., orchestrator parses stdout instead of writing CSV), the Debug-formatted `Some(MemoryExceeded)` differs from the plot script's expected `STOP_REASON_COLOR` keys (`"WallTimeExceeded"`, `"MemoryExceeded"`, `"Oom"`, `""`). The plot script `STOP_REASON_COLOR.get(sr, "#888888")` would always return the gray fallback — every bar in `summary_walls.pdf` is unlabeled-gray.

---

## Edge Cases Not Covered

### EC-001 — `--workers 0`

The CLI accepts `--workers 0` (clap default `Vec<u32>`); `commands.rs:314`: `let workers = args.workers.first().copied().unwrap_or(1) as usize;` returns 0; `run_one_sequence:1148`: `let workers_u32 = workers.max(1) as u32;` saturates to 1. So 0 workers silently becomes 1. Operator may not realize.

### EC-002 — Concurrent `MemoryProbe::new()` calls

Each call independently re-opens `/proc/meminfo` on Linux. If the system is hot-pluggable RAM (rare on workstations, common on cloud VMs with ballooning), two probes constructed one minute apart might cache DIFFERENT `total_ram_bytes`. The methodology says nothing.

### EC-003 — `--n-seq 0`

`run_one_sequence` invokes `run_benchmark_suite` with `sizes: Some(vec![0u32])` → benchmark generators may panic on `n=0` (e.g., `dual_tree(0)` constructs a 0-depth tree which has 0 agents). Tests don't cover this.

### EC-004 — CSV with UTF-8 BOM

If the operator opens the CSV in Notepad on Windows and saves, a BOM gets prepended (`EF BB BF` bytes). `pandas.read_csv` with default `encoding='utf-8'` chokes on the first row's first cell which becomes `﻿benchmark`. No test covers this.

### EC-005 — Output directory on a non-POSIX filesystem

The orchestrator `mkdir -p "$OUTPUT_DIR/raw"` and `find raw figures -type f | sort | xargs sha256sum` — if `OUTPUT_DIR` is on Windows NTFS via WSL with case-insensitive paths, `find` returns lowercase, `sha256sum` reads lowercase, but a file system with case-folded files might double-count. Untested.

### EC-006 — `git rev-parse HEAD` in the manifest when checkout is detached HEAD

The MANIFEST.md uses `git -C "$REPO_DIR" rev-parse HEAD` which works on detached HEAD. Fine. But branch info is missing — the methodology page doesn't mention what branch/tag the campaign ran from. The operator inspecting the locked dataset 6 months later won't know whether it was `feature/stress-and-encoder` HEAD or a tag.

---

## Test Coverage Gaps

### TG-001 — No test asserts that `write_csv_detail` output is consumable by `plot_stress_curve.py`

This is the gap that MASKS BUG-001 + BUG-002. Add an integration test:
```rust
#[test]
fn write_csv_detail_output_is_plot_script_consumable() {
    let result = sample_row(...);
    let mut buf = Vec::new();
    write_csv_detail(&mut buf, &[result])?;
    let csv_path = tempdir().join("out.csv");
    std::fs::write(&csv_path, buf)?;
    let status = Command::new("python3")
        .arg("scripts/plot_stress_curve.py")
        .arg("--input").arg(&csv_path)
        .arg("--output-dir").arg(tempdir())
        .status()?;
    assert!(status.success(), "plot script must consume real Rust output");
}
```

### TG-002 — No test covers `--resume` with the SCRIPT'S OWN OUTPUT after BUG-001 fix

The current resume invariant test (`d014_resume_invariant.rs`) writes RAW_CSV via the broken path. It cannot fail on a real schema. Add a test that round-trips a known-correct CSV through `--resume`.

### TG-003 — No test covers Windows-job-object OOM signature (-1073741756)

If the campaign ever runs in a constrained job, `STATUS_QUOTA_EXCEEDED` is missed.

### TG-004 — No test covers SIGABRT (Rust alloc panic-abort) as OOM

A workload that exhausts RAM via Rust allocator panic produces SIGABRT, not SIGKILL. Currently classified as generic crash.

### TG-005 — No test exercises `cv_above_gate=true` codepath

Per BUG-016, the column is always false. Add a test that aggregates a high-CV result set and asserts `cv_above_gate=true` propagates to the row.

### TG-006 — No test for plot script with empty CSV (header only)

`plot_stress_curve.py:75-77` handles the empty case (`exit 2`), but no integration test exercises this with a real Rust-emitted empty CSV.

### TG-007 — No test for plot script with single row (degenerate stats)

Geometric mean of one value is itself; `np.std` of one-element array is 0. The errorbar plot gets a 0-length whisker — visually acceptable but untested.

### TG-008 — No test verifies the orchestrator's smoke completes without matplotlib

The smoke test `IT-0704-01` skips when matplotlib is missing. There is no separate test that asserts the orchestrator produces the placeholder PDF correctly. The placeholder writer at `stress_curve.sh:236-251` could regress (line ending wrong, PDF magic shifted) and no test catches it.

---

## Stress Scenarios

### SS-001 — Overnight run interrupted by power loss / hard reset

No fsync on RAW_CSV writes (bash `>>` uses default OS buffering). Hard reboot mid-write loses the most recent N reps' data. `--resume` then misses them.

**Recommendation:** Document that overnight runs require UPS-backed hosts, OR add `sync` after each rep in the orchestrator.

### SS-002 — Multiple parallel campaigns on the same host

No advisory lock. Two operators simultaneously run `scripts/stress_curve.sh --output-dir <different>` will share the same `MemoryProbe` denominator (host RAM total) but compete for it. RAM-fraction gates trip earlier than expected for both. No mention in methodology.

### SS-003 — Disk full mid-rep

`>>"$RAW_CSV"` returns ENOSPC; `set +e` swallows; `EC=$?` captures the error; line 190-192 emits a WARN but the loop continues. Eventually all subsequent reps also fail. The campaign exits 0 (because the loop's body suppresses errors) with a half-written CSV. No disk-space pre-flight check despite SENTINEL.md §5.1 #10 mentioning ≥ 10 GiB free.

### SS-004 — Clock skew / NTP correction during run

`Instant::now()` is monotonic, so wall_clock_secs is safe. But `date -I` in the manifest could shift if the system clock corrects. The 2026-05-12 in the directory name vs. the actual MANIFEST.md timestamp could diverge by hours.

### SS-005 — Workload generator runs out of u32 ID space at N=10⁹

The `STRESS_CURVE_N_SEQ` reaches 10⁹ (1e9). Each interaction-net agent has a u32 id (`AgentId(u32)`); workloads like `condup_expansion` may double the agent count in the first round. At N=1e9, the second round would need 2e9 u32 ids — fits, but close to u32::MAX = 4.29e9. At N=2e9 (above u32::MAX/2), expansion overflows. The campaign caps at 1e9 so this is "safe by ~2x", but not documented as a deliberate safety margin.

### SS-006 — `condup_expansion` workload at large N is intentionally unbounded growth

The methodology says workloads include `condup_expansion`. Per ARG-001 / discussions, this is a Profile B workload (expansion with collapse). At N=1e9 with 4 workers, the cross-partition CON-DUP expansion may flood `redex_queue`s beyond the cap and trigger SPEC-21 backpressure. This is the expected stress-curve behavior — but if the StopRule trips on memory before expansion completes, the CSV row's `total_interactions` is partial. The plot may look like a sublinear scaling curve when the true behavior is cut off. Methodology mentions this in §7 but the plot script doesn't visually distinguish "completed" from "stopped" rows on the metric plots (only on `summary_walls.pdf`).

---

## Verdict on MF-001

**CONFIRMED. Severity: CRITICAL.** The reviewer's adversarial scenario reproduces verbatim:
- Operator runs `cargo run --release --bin relativist -- bench --campaign stress-curve --workload ep_annihilation --env in-process --workers 2 --n-seq 1000`
- Stdout: `stress-curve outcome: completed_reps=1 stop_reason=None last_attempted_n=Some(1000)`
- Bash orchestrator at line 179-186 redirects this stdout to `RAW_CSV`
- After 7-8h overnight, `RAW_CSV` has ~1320 lines of `stress-curve outcome:...` and **zero CSV rows**
- Plot script at line 78-84 exits 1 with `required column 'workload' missing`
- Orchestrator at line 222-225 swallows the error
- Smoke fallback at line 234-251 generates a 145-byte blank PDF
- Operator wakes up to a blank PDF, an unparseable CSV, and a `MANIFEST.md` claiming success.

**Additional details beyond the reviewer's framing:**
- BUG-002 makes the gap STRUCTURAL (column NAMES diverge), not just FORMAT (rows missing). Even if BUG-001 is patched to emit `write_csv_detail` rows, BUG-002 still kills the plot.
- BUG-006 (placeholder PDF) makes the smoke pass green even when both BUG-001 and BUG-002 are unfixed, providing FALSE confidence to the operator.
- BUG-016 (cv_above_gate dead column) means even a perfectly-fixed campaign produces incomplete metadata.
- BUG-003 (Rust API VmHWM contamination) means the `IT-0707-06 end_to_end_smoke_in_process` test "passes" with subtly wrong vmrss values that no test asserts the correctness of.

**The scenario is not theoretical. The exact reproduction the reviewer described is the default behavior on the current branch.**

---

## Recommendation

**D-014 is NOT safe-to-merge** in its current state. Stage 6 REFACTOR by the developer is REQUIRED to address at least the 4 CRITICAL bugs (BUG-001 through BUG-004) before any merge can be considered. The HIGH bugs (BUG-005, BUG-006) should be addressed in the same Stage 6 because they directly amplify the criticals (BUG-006 masks BUG-001/002 in CI; BUG-005 means OOM detection misses common patterns).

The proper resolution requires a follow-up TASK-0720 with the following acceptance criteria:

### TASK-0720 (mandatory before merge)
1. **Fix BUG-001:** `bench --campaign stress-curve` emits a real CSV row (or accepts a `--csv-out` path that the orchestrator passes per-rep). The dispatch path must call `write_csv_detail` (or equivalent) to produce structured output.
2. **Fix BUG-002:** Single-source-of-truth for column names. Either rename the writer's columns to match the plot script's `REQUIRED_COLUMNS`, OR rename the plot script's expectations to match the writer. Add an integration test (TG-001) that pipes the writer's output to the plot script and asserts exit 0.
3. **Fix BUG-003:** Document the Rust API's per-rep VmHWM-contamination caveat in `docs/benchmarks/campaigns/stress-curve.md` §9, OR refactor `MemoryProbe` to support per-rep delta-from-baseline. (Documenting is acceptable; refactoring is preferred.)
4. **Fix BUG-004:** Add child-PID tracking + cleanup to the bash trap; add a flock on `<outdir>/.lock`.
5. **Fix BUG-005:** Add `STATUS_QUOTA_EXCEEDED` (-1073741756) to `OOM_EXIT_CODES`; add `OOM_SIGNALS` (SIGKILL=9, SIGABRT=6); update `StopRule::check` to use both. Update tests.
6. **Fix BUG-006:** Make the placeholder-PDF fallback opt-in via `--allow-placeholder`. Smoke test asserts a real PDF (size > 5 KB) when matplotlib is available.

### Optional follow-ups (post-merge, lower priority)
- BUG-007 through BUG-020 are MEDIUM/LOW; none are merge blockers individually.
- BUG-016 (`cv_above_gate` dead column) should be fixed or the column removed before TASK-0708 (the lock task) — otherwise the locked dataset has a known-dead column that future readers will distrust.
- TG-001 through TG-008 cover gaps; at minimum TG-001 (writer→plot integration) MUST be added in TASK-0720.

### Why this is not "fix in TASK-0708 (lock)"
TASK-0708 per the SENTINEL.md is the **execution task** ("operator presses go"). It assumes the infrastructure is correct. With the current bugs, the overnight run produces unusable output. Asking the operator to discover this at 6am after waking up from an 8-hour run would be unprofessional. TASK-0720 must land BEFORE TASK-0708 is dispatched.

### Bisection-friendliness
The 9 commits in `bcff245..e066d3c` are well-structured (one TASK per commit). TASK-0720 can target the specific commits:
- BUG-001 → fix in `commands.rs` (touches `415ac6e` + `dabd1fb` boundary)
- BUG-002 → fix in either `csv.rs` (touches `415ac6e`) or `plot_stress_curve.py` (touches `51c3190`)
- BUG-003 → fix in `memory_probe.rs` (touches `bcff245`) + doc update (touches `8a192aa`)
- BUG-004 → fix in `stress_curve.sh` (touches `dabd1fb`)
- BUG-005 → fix in `stop_rule.rs` (touches `e8dbf7c`) + tests (touches `f25be1f`)
- BUG-006 → fix in `stress_curve.sh` (touches `dabd1fb`) + integration test (touches `f25be1f`)

A single TASK-0720 cleanup commit can land all six in one revision.

---

## Refutations of pre-flagged adversarial points

For completeness, points the reviewer suggested that I investigated and **refuted**:

- **`StopRule::check` NaN/INFINITY bypass via `as_fraction_of_total`:** REFUTED. `memory_probe.rs:117-121` short-circuits to `0.0` when `total_ram_bytes == 0`, making NaN unreachable. `bytes as f64 / nonzero u64 as f64` is always finite (u64 cannot be NaN, division of finite-finite is finite-or-Inf). To produce NaN, the caller would have to construct `RepResult` manually with a NaN literal in `vmrss_peak_fraction_of_total` — which the production codepath at `suite.rs:1200` does not do. The original adversarial framing assumed `MemTotal` failure produces NaN; verified it does not.
- **`measure_sequential` (suite.rs:444) and `measure_grid` (suite.rs:642) BenchmarkResult ripple — 5th site:** REFUTED. Grep `BenchmarkResult\s*\{` across the workspace returns only 4 sites: `csv.rs:358` (test), `mod.rs:827` (test), `suite.rs:410` (measure_sequential), `suite.rs:589` (measure_grid). Plus the d014 schema test at `tests/d014_csv_schema_roundtrip.rs:26`. All 5 sites set the 4 D-014 fields. No 5th production site exists.
- **`tempfile` + `libc` runtime-vs-dev-deps regression:** REFUTED. Inspected `relativist-core/Cargo.toml`. `tempfile` and `csv` are correctly under `[dev-dependencies]` (lines 117-118); `libc` is correctly under `[target.'cfg(unix)'.dev-dependencies]` (line 121). `windows-sys` is under `[target.'cfg(target_os = "windows")'.dependencies]` (line 94) which is correct (it's a runtime dep on Windows only). On Linux CI, `windows-sys` is automatically excluded by Cargo target gating.

---

## Compliance with QA agent boundaries (per CLAUDE.md)

- Did NOT touch `src/` or `tests/` (verified by file ops).
- Did NOT touch `specs/` or `progress.md` (verified).
- Did NOT make commits (verified — only `Write` tool used on the QA report).
- Wrote ONLY in `docs/qa/D-014-stress-curve-qa.md` per the user's instruction.
- Read OBJETIVO_TCC.md first. Read enough of progress.md to ground the report.
