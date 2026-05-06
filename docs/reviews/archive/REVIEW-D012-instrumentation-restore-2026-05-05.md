# REVIEW — D-012 Instrumentation Restore

**Reviewer:** Stage 4 (REVIEW) of SDD pipeline
**Date:** 2026-05-05
**Scope:** Commits `360d6ea`, `fd2cafe`, `ca3634c`, `ac828f9` on `v2-development`
**Inputs read:**
- `docs/handoffs/2026-05-05-D012-instrumentation-restore-handoff.md`
- `docs/backlog/TASK-0615..0618-*.md`
- `docs/tests/TASK-0615..0618-tests.md`
- `docs/analysis/D011-final-baseline-analysis-2026-05-04.md` (RF-04, RF-05, RF-07 mechanism context)
- Source: `relativist-core/src/protocol/coordinator.rs`, `merge/grid.rs`, `protocol/channel.rs`, `bench/suite.rs`, `bench/benchmarks/ep_annihilation.rs`
- Tests: `relativist-core/tests/d012_*.rs`

---

## Verdict

**ACCEPT_WITH_FIXES** — 2 Major findings, 6 Significant findings, 4 Minor nits.

The bundle's production deltas are surgical, idiomatic, and respect the dependency direction. The instrumentation correctly closes RF-04 (network time) and RF-05 (compute time) on the distributed code path. TASK-0617 is mechanically clean, with a documented and defensible deviation (the developer found 2 pre-existing release-mode compile errors beyond the handoff's stated 2). The 2 Major findings concern (a) parity of the new per-round Vecs with the existing `push_partial_round_metrics` helper on early-return paths, and (b) the IT-0618-A1 witness that does NOT actually exercise the literal acceptance criterion (`summary.csv::mips_mean > 0`) — it pins the metric struct only, not the CSV writer path that the analysis flagged as "structurally zero across every row."

The most important finding is **MF-001** (per-round Vec parity): pre-existing pattern flaw applies symmetrically to the new metrics, but the bundle introduces three new round-keyed Vecs that compound the inconsistency. Stage 6 REFACTOR should either extend `push_partial_round_metrics` to cover them or document the chosen invariant explicitly.

---

## Per-commit summary

| Commit | Title | Production LoC | Test LoC | Verdict |
|---|---|---|---|---|
| `360d6ea` | TASK-0617 — make `cargo test --release` compile | +11 / -1 (4 files) | 0 | ACCEPT |
| `fd2cafe` | TASK-0615 — restore per-round network time instrumentation | +38 / -8 (1 file) | +361 (1 new file) | ACCEPT_WITH_FIXES |
| `ca3634c` | TASK-0616 — aggregate per-worker compute time on TCP path | +18 (1 file) | +417 (1 new file) | ACCEPT_WITH_FIXES |
| `ac828f9` | TASK-0618 — pin `total_interactions` / mips end-to-end (path a) | 0 prod | +184 (1 new file) | ACCEPT_WITH_FIXES |

**Total:** ~67 LoC production + ~962 LoC tests across 4 commits. Within the ~170/~210 estimate for production but the test side is ~4.5× the estimate (largely OneShotTransport infrastructure + the rich documentation comments — both defensible).

---

## Major Findings (Stage 6 REFACTOR must address)

### MF-001 — New per-round metric Vecs are not patched by `push_partial_round_metrics`

**Category:** Code Quality / Correctness (latent)
**Files:** `relativist-core/src/protocol/coordinator.rs:137-165` (helper), `:851-852` (init), `:912/929/986/1111/1148/1161/1192/1246/1323` (early-return sites), `:1373-1374` (push site), `:1397-1400` (TASK-0616 push site).

**Problem.** The helper `push_partial_round_metrics(&mut metrics)` was introduced (per QA-003 inline test at `:1951-1968`) precisely to keep all per-round Vecs at the same length as `effective_slots_per_round` when the coordinator returns early via `?`. After this bundle, three new per-round Vecs are populated only at end-of-round (after all early-return sites):
- `metrics.network_send_time_per_round` (TASK-0615, line 1373)
- `metrics.network_recv_time_per_round` (TASK-0615, line 1374)
- `metrics.compute_time_per_round` (TASK-0616, line 1399)

`push_partial_round_metrics` does NOT touch them. If the coordinator hits any of the 9 early-return sites, these three Vecs end up shorter than `effective_slots_per_round.len()`. A bench harness that zips them by index (e.g., `bench/suite.rs:621-625`) would silently drop the last partial round, or worse, mis-align rounds across columns.

**Caveat.** The bundle is consistent with the EXISTING flawed pattern: `bytes_sent_per_round` / `bytes_received_per_round` (added pre-D-012 at `:1366-1367`) are also not in the helper. So the bundle does not REGRESS the helper's coverage — it merely propagates the gap. But the QA-003 helper is an "all per-round Vecs" promise (per inline doc-comment "restores per-round Vec parity"), and the bundle is the right moment to either honor that promise or amend it.

**Suggested fix (Stage 6).** One of:
1. Extend `push_partial_round_metrics` to include the 5 missing Vecs (`bytes_sent_per_round`, `bytes_received_per_round`, `network_send_time_per_round`, `network_recv_time_per_round`, `compute_time_per_round`). Update QA-003 test to assert on them. ~10 LoC.
2. Document explicitly (in the helper's doc-comment) that these 5 Vecs are NOT patched and the rationale (e.g., "downstream consumers tolerate a length-mismatch on the last round only"). The bench harness should then be audited to confirm tolerance. Worse than option 1.
3. Add a `debug_assert_eq!(network_send_time_per_round.len(), effective_slots_per_round.len())` at end of `run_coordinator` so any future regression fires loudly in debug builds.

Recommend option 1 + option 3 together.

**Why this is Major.** It's a correctness regression-vector that would land silently. A future task that adds early-return logic in the dispatch/collect phase would compound the asymmetry without warning. This bundle introduces the new Vecs and is the appropriate place to fix or formalize the contract.

---

### MF-002 — IT-0618-A1 does NOT witness the literal TASK-0618 / TEST-SPEC-0618 acceptance criterion

**Category:** Test Contract Fidelity / Spec Compliance
**Files:** `relativist-core/tests/d012_mips_witness.rs:55-130` (IT-0618-A1).

**Problem.** TASK-0618 path-(a) acceptance criterion (taskfile §"Acceptance for (a):") states:

> `summary.csv::mips_mean > 0` for any non-trivial bench.

TEST-SPEC-0618 IT-0618-A1 (lines 56-69) mandates:

> 1. Run the bench end-to-end.
> 2. Read the produced `summary.csv` and `detail.csv` from the bench's output directory.
> 3. Parse the rows...
> Assertions: `summary.csv::mips_mean > 0.0` ... `detail.csv::total_interactions > 0` ...

The implemented IT-0618-A1 does NEITHER. It calls `run_grid` directly, captures the in-memory `GridMetrics`, and computes `mips` itself from `metrics.total_interactions / elapsed * 1e-6`. It bypasses `bench/suite.rs::run_one`, the CSV writers in `bench/csv.rs`, and the `summary.csv`/`detail.csv` artifacts entirely.

This matters because TASK-0618's commit body (ac828f9) explicitly admits:

> "The v2-baseline showing total_interactions = 0 in all 40 rows is a separate latent issue (likely a CSV-writer-path artifact in a specific bench mode); the contract this task pins is 'a non-trivial bench through `run_grid` populates `total_interactions > 0`,' which is now test-asserted."

The latent issue the developer flags is precisely what RF-07 reports. The witness as implemented does NOT close RF-07 in the failure mode the v2-baseline exhibited; it pins a stricter, narrower invariant. RF-07 is *the* red flag this task was meant to close.

**Suggested fix (Stage 6).** Either:
1. Replace IT-0618-A1's body with a real bench-harness invocation (e.g., `bench::suite::run_one` with a small workload + tempdir output), then read the produced `summary.csv` and assert `mips_mean > 0`. ~30 LoC of CSV-reader test setup. Closes the literal acceptance criterion.
2. Keep the current IT-0618-A1 (rename to `metrics_struct_records_nonzero_total_interactions_and_mips`) AND add a SECOND test `summary_csv_records_nonzero_mips_after_full_bench_run` that does the end-to-end CSV assertion.
3. Investigate the "separate latent issue" the developer flagged. If reproduced, file as TASK-0619 explicitly and document in this bundle's commit history that RF-07 closure is conditional on TASK-0619. Update TASK-0618 status to "partial closure of RF-07" rather than "closes RF-07."

Recommend option 2 (keep the metric-struct unit-style test, add a CSV-end-to-end integration test).

**Why this is Major.** The commit message claim "Closes: TASK-0618 (D-011-FU-MIPS), RF-07" is not supported by the test contract as implemented. Either the test is short of the spec, or the spec is short of the witness — and the developer's own commit body admits the gap. RF-07 closure should be honest: either the witness exercises the failure mode, or the closure is downgraded to "partial."

---

## Significant Findings (polish / cleanup; not strict blockers)

### SF-001 — IT-0615-04 silently passes when no rounds executed

**Category:** Test Contract Fidelity
**Files:** `relativist-core/tests/d012_network_time_witness.rs:298-343` (IT-0615-04).

**Problem.** The TEST-SPEC-0615 IT-0615-04 contract says: "even for a no-redex round, the protocol exchange happened" and "any round that completes a wire round-trip records measurable send/recv." The test as implemented (lines 312-326) accepts an alternative outcome: if `network_send_time_per_round.is_empty()` AND `metrics.rounds == 0`, the test passes silently with `return`. That branch is NOT in the spec — the spec mandates "the protocol exchange happened, even for 0-redex" and pins the wall-time-on-the-wire semantics regardless of byte count.

Concretely, if a future change makes the coordinator short-circuit a 0-redex net before any dispatch (the `metrics.rounds == 0` early-exit at `merge/grid.rs:60-63`), this test would pass without ever exercising "heartbeat-only round" instrumentation. The test is currently witnessing two different code paths (round-happened or round-didn't) and rubber-stamps both.

**Suggested fix (Stage 6).** Choose one:
1. Force the bench to actually emit a heartbeat round. Use a workload that doesn't short-circuit (e.g., a 1-redex net that converges in 1 round → exactly 1 dispatch+collect → asserts `len() >= 1` strictly). The existing `build_simple_redex_net` is identical to IT-0615-01, so the heartbeat-only edge case is genuinely subsumed by IT-0615-01 and IT-0615-04 becomes redundant.
2. Use the alternative path explicitly: configure the coordinator to send a heartbeat even when `redexes == 0`, then assert send/recv > 0. Today there's no such config knob (only `max_rounds`).
3. Tighten the assertion: if Vecs are empty, the test FAILS rather than silently passing. Then document in TASK-0619 that "heartbeat-only round" needs a coordinator-side knob to force a no-work round, and remove the test until that knob exists.

The easiest path is option 1 (recognize IT-0615-04 is duplicated by IT-0615-01 and remove it, with a comment block citing why).

---

### SF-002 — IT-0616-04 `#[ignore]` body is a non-functional placeholder

**Category:** Test Contract Fidelity
**Files:** `relativist-core/tests/d012_compute_time_witness.rs:356-366`.

**Problem.** TEST-SPEC-0616 says:

> The unused test (03 or 04) MUST be present in the file with a `#[ignore = "..."]` attribute and a comment block citing the rationale. This preserves the spec's full coverage on git history; **a future migration between paths re-enables the dormant test by deleting the `#[ignore]` line**.

The implemented IT-0616-04 body is `panic!("path (b) not in effect — see #[ignore] reason")`. If a future maintainer deletes the `#[ignore]` line per the TEST-SPEC's documented re-activation procedure, the test will immediately panic. There is NO actual test logic preserved — the documented spec contract for IT-0616-04 (residual computation, 95% component coverage, etc.) is not carried in code, only in the file's header comment.

**Suggested fix (Stage 6).** Replace the body with a stub that compiles but performs no real-clock measurement (e.g., reads `metrics.compute_time_per_round[0]`, `metrics.merge_time_per_round[0]`, etc., and asserts the residual relationship). On path (a) the values pushed by the SUM aggregation will trivially satisfy whatever the residual formula computes (or trivially fail — the point is that re-enabling exposes the path-mismatch, which the developer/migrator wants to see). Better than a panic.

Alternative: delete the test entirely and rely on the TEST-SPEC + commit-history archaeology if path (b) is ever needed.

The current state is the worst of both options: the test exists but is non-functional.

---

### SF-003 — IT-0618-A1 test name `tcp_round_records_nonzero_total_interactions_and_mips` is misleading

**Category:** Code Quality / Naming
**Files:** `relativist-core/tests/d012_mips_witness.rs:55-130`.

**Problem.** The function name claims a "tcp_round" but the body uses `run_grid` (in-process path) — there is no TCP / `run_coordinator` invocation. The TEST-SPEC IT-0618-A1 calls for "Coordinator + 1 worker over localhost TCP." This is the same TCP-vs-ChannelTransport substitution flagged in TASK-0615's commit body, but in TASK-0618 the substitution goes further: not even a Channel — straight `run_grid`. A reader grepping for "tcp" and finding this test would be misled.

**Suggested fix (Stage 6).** Rename to one of:
- `nontrivial_grid_run_records_nonzero_total_interactions_and_mips`
- `in_process_grid_records_nonzero_total_interactions_and_mips`
- `metrics_total_interactions_and_mips_are_nonzero_after_grid_run`

This is a 1-line change, harmless, and aligns the function name with what the body actually does.

---

### SF-004 — IT-0618-A2 only covers `num_workers=2` (multi-worker path); `run_single_worker` (num_workers=1) is unwitnessed

**Category:** Test Contract Fidelity / Coverage
**Files:** `relativist-core/tests/d012_mips_witness.rs:141-184` (IT-0618-A2), `relativist-core/src/merge/grid.rs:1480` (`run_single_worker` aggregation).

**Problem.** The aggregation invariant `metrics.total_interactions == sum(local_interactions_per_round) + sum(border_interactions_per_round)` is asserted only on the multi-worker code path (`num_workers=2`, which invokes the loop at `merge/grid.rs:70+`). The single-worker path at `run_single_worker` (line 1466+) has DIFFERENT aggregation logic:

```rust
metrics.total_interactions = stats.total_interactions;  // line 1480: ASSIGNMENT, not +=
metrics.local_interactions_per_round.push(stats.total_interactions);  // line 1479
metrics.border_interactions_per_round.push(0);  // line 1484
```

This is structurally consistent (single round, no border), but the assertion-style is different. If a future change introduces multi-round support to `run_single_worker` and forgets to switch from `=` to `+=`, the bug would not be caught by IT-0618-A2.

Additionally, `bench/suite.rs::run_one` calls `run_grid` with whatever `num_workers` the bench config specifies. Many bench profiles use `workers=1` for the seq-baseline lane. If `total_interactions = 0` on those rows of the v2-baseline (per RF-07's "all 40 rows of v2-post and all 40 rows of v1"), and `run_single_worker` is the path used, then the bug is on the path the witness does NOT cover.

**Suggested fix (Stage 6).** Add `IT-0618-A3` (or extend A2) with `num_workers=1` and the same invariant. ~15 LoC.

---

### SF-005 — `bytes_sent_per_round` / `bytes_received_per_round` had this same parity gap pre-D-012; the bundle missed an opportunity to fix the existing pattern

**Category:** Architecture / Pre-existing tech debt
**Files:** Same as MF-001.

**Problem.** Pre-existing flaw, called out so Stage 6 / a future bundle can address. `bytes_sent_per_round` and `bytes_received_per_round` were added before D-012 and live at `:1366-1367` (push site) but not in the helper. The bundle introduces 3 more vecs with the same gap. If MF-001 is fixed by extending the helper, fix bytes_sent/received at the same time (they are exactly the same shape: pushed once at end-of-round, on the happy path only).

This is pre-existing debt; flagging it under Significant rather than Major because the bundle did not introduce it.

---

### SF-006 — `_compile_time_imports_used` in `d012_network_time_witness.rs` is a code smell

**Category:** Code Quality / Test Hygiene
**Files:** `relativist-core/tests/d012_network_time_witness.rs:349-361`.

**Problem.** A `#[allow(dead_code)] fn _compile_time_imports_used()` exists at the bottom of the file to silence dead-import warnings on `HashMap`, `IdRange`, `Partition`. This is a smell — if these types are not used in any test, they should be removed from the imports. The function body constructs a `Partition` struct literal that has no semantic relationship to the test contracts.

A grep of the file confirms: `IdRange` and `Partition` are not referenced anywhere else in the file. `HashMap` is constructed only in `Partition.free_port_index` initialization in this dead-code stub. They came in via copy-paste from another test file.

**Suggested fix (Stage 6).** Delete the function and the unused imports. ~10 LoC reduction.

---

## Minor / Nits (informational only)

### NTH-001 — TASK-0617 deviation from TEST-SPEC-0617 expected band ([10,16]) is justifiably documented but warrants a CLAUDE.md update

**File:** `relativist-core/CLAUDE.md` (Build & Test section).

The handoff and TEST-SPEC-0617 expected `count(debug) - count(release) ∈ [10, 16]`. The actual delta after the fix is **58** (1784 debug → 1726 release). The commit body explains why correctly (80 pre-existing `cfg(debug_assertions)` occurrences across 11 files). However, this expanded delta becomes the new release-mode floor, and `CLAUDE.md` already documents test floors. Stage 6 should add `cargo test --release: 1736` (post-D-012) to `CLAUDE.md`'s "Build & Test" section so future maintainers see it.

The same applies to `docs/next-steps.md` (sdd-pipeline territory at bundle close).

### NTH-002 — TASK-0616 OneShotTransport adapter would benefit from a shared test fixture if more multi-worker channel tests appear

**File:** `relativist-core/tests/d012_compute_time_witness.rs:76-99`.

The `OneShotTransport` adapter is fine where it is. If future tests need the same multi-worker channel pattern, consider promoting it to `relativist-core/tests/common/mod.rs` (or similar) so it isn't re-defined. Not a blocker — at 24 LoC it's cheap to inline. Just a note for future bundles.

### NTH-003 — TASK-0617's `panic!` arm in `coordinator.rs:1871` could use `unreachable!` semantics for the FSM intent

**File:** `relativist-core/src/coordinator.rs:1872-1877`.

The handoff explicitly recommends `panic!` over `unreachable!` for FSM diagnostic clarity. The dev followed the recommendation. Implementation hint #2 in TASK-0617 is consistent. Just noting that some style guides would prefer `unreachable!("...")` for "this case is logically impossible in this test context" — both are functionally equivalent under `-D warnings`. Defensible either way.

### NTH-004 — The TASK-0615 commit body says "Closes RF-04" without preserving the historical pre-fix witness in the test file

**File:** `relativist-core/tests/d012_network_time_witness.rs`.

A common pattern in this codebase (per D-011 history) is to land a TDD-RED commit FIRST that fails on HEAD, then a GREEN commit that fixes. D-012 lands the test and fix together, which is a defensible choice for instrumentation-only work, but it means the "FAILS pre-fix" claim in TEST-SPEC-0615's per-test specifications is not historically witnessed. If the operator wants the audit trail, future bundles could split per the D-011 RED→GREEN pattern. Not in this bundle's scope.

---

## Out-of-scope items observed but not flagged

These were noticed during review but reasonably deferred or not in the bundle's scope:

1. **`condup_expansion` setup-time asymmetry (RF-02 / D-011-FU-CONDUP).** Explicitly out of D-012 scope per handoff §6. Pre-existing.
2. **CI release-test lane (`.github/workflows/release-test.yml`).** Per TEST-SPEC-0617 §"CI lane (recommended follow-up)" — explicitly OUT of D-012 scope, routes to a future CICD task.
3. **`bench/suite.rs:506` formula derivation site.** Reads `grid_metrics.total_interactions` correctly. The "v2-baseline showing 0 in all 40 rows" claim in the TASK-0618 commit body could be RF-07's actual root cause living in a bench-mode-specific path that none of the new tests cover (e.g., streaming-no-recycle, or zero-copy with a specific feature flag combination). Not in D-012 scope; flagged as a candidate for TASK-0619.
4. **`cfg(debug_assertions)` density across the codebase (80 occurrences in 11 files).** TASK-0617's deviation report surfaces this as pre-existing. Audit-style cleanup is out of scope.
5. **`run_distributed_one_worker` helper duplication between `d012_network_time_witness.rs` and `d012_compute_time_witness.rs`.** Two near-identical helpers, slightly different shapes (single-worker vs N-worker). Could share. Not introduced by this bundle (each file builds its own); minor.
6. **`bench/csv.rs:159` write site.** Not modified by this bundle. Per MF-002 above, the path from `metrics.total_interactions` to CSV column `total_interactions` is currently un-witnessed end-to-end. Worth a follow-up.

---

## Counts & summary

- **Major (MF):** 2
- **Significant (SF):** 6
- **Minor (NTH):** 4
- **Total findings:** 12

**Path decisions (TASK-0616 path (a) vs (b); TASK-0618 path (a) vs (b)):** Both decisions are sound and well-rationalized. TASK-0616's path (a) avoids the wire-format extension correctly (the `WorkerRoundStats` payload already carries `reduce_duration_secs` — verified at `protocol/worker.rs:255-256` and `merge/grid.rs:147`). TASK-0618's path (a) "0 production change" claim is verified — `git diff fd2cafe ac828f9 -- relativist-core/src/` shows no src/ changes between TASK-0615 and TASK-0618. The aggregation sites at `merge/grid.rs:208,240,300` and `protocol/coordinator.rs:1449` already populate `total_interactions`. The only concern with path (a) is MF-002: the "CSV-writer-path latent issue" the commit flags is not closed by the witness.

**ChannelTransport vs TCP-localhost:** Documented deviation, defensible. Both transports exercise the same wire-facing instrumentation sites in `protocol/coordinator.rs`. Reviewer agrees with the developer's rationale.

**OneShotTransport adapter:** Test-only, reasonable workaround for the `&mut dyn Transport` ownership constraint when spawning N workers from a single `ChannelTransport::pair`. Not a structural protocol issue.

**Production code quality:**
- No `unwrap()` introduced (all `.expect()` are test-side).
- No `unsafe` introduced.
- No `println!` introduced (`tracing` not used either, but the new code is purely measurement — no log emission needed).
- `thiserror` discipline preserved.
- `pub(crate)` discipline preserved (no new public API on the production side).
- `Instant::now()` straddling pattern is idiomatic Rust.
- Dependency direction respected: all production changes live in `protocol/coordinator.rs` (the protocol layer, which depends on `merge` and `net`). Core-layer purity preserved (`merge/grid.rs` was only touched in test-only `mod tests` for TASK-0617's release-mode fix; no production logic added).

---

## Recommendation for Stage 5 (QA)

QA should focus on:
1. **MF-001:** craft an adversarial test that triggers an early-return path mid-round (e.g., simulate a `distribute_partitions` timeout or a `recv_frame` `ProtocolError`) and assert that the new per-round Vecs end up out of parity with `effective_slots_per_round.len()`. This exposes the latent bug.
2. **MF-002:** craft a real bench-harness invocation (with `bench::suite::run_one`) and verify `summary.csv::mips_mean > 0` end-to-end. If it fails, RF-07 is not actually closed and the bundle's commit message claim is overstated.
3. **SF-004:** run IT-0618-A2's invariant against `num_workers=1` and assert it holds.

If QA finds nothing else, Stage 6 REFACTOR scope is: extend `push_partial_round_metrics` (MF-001 + SF-005), promote IT-0618-A1 to a CSV-end-to-end test or add a sibling test (MF-002), tighten or remove IT-0615-04 (SF-001), give IT-0616-04 a real body or remove it (SF-002), rename `tcp_round_*` IT-0618-A1 (SF-003), add `num_workers=1` coverage (SF-004), and delete the dead-import stub (SF-006). Estimated total: ~80 LoC across 4 files. Well under the bundle's 200-LoC-per-task ceiling.
