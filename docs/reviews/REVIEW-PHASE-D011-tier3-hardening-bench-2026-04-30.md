# Review: D-011 Phase A-F — Tier 3 Hardening + Bench Enablement (Commits abd2976..69d5d66)

**Date:** 2026-04-30
**Reviewer:** REVIEWER agent (Stage 4, unified code quality + architecture)
**Branch:** v2-development
**Verdict:** **ACCEPT_WITH_FIXES**

**Commits reviewed (16):**

| SHA | Task | Phase | Headline |
|-----|------|-------|----------|
| `abd2976` | TASK-0596 | B-1 | CompactSubnet wire `free_list` round-trip + PROTOCOL_VERSION 6→7 |
| `b0c873c` | TASK-0597 | B-2 | Thread `max_pending_lifetime` through legacy callers |
| `5acf9ad` | TASK-0598 | B-3 | debug_assertions ABI drift fix (counter fields always-present) |
| `d455f33` | TASK-0599 | B-4 | Named worker semantics + IT-0591 strengthening |
| `4de2231` | TASK-0600 | B-4 | Collapse `Pull*` / `PullCoordinatorState` parallel types |
| `69d5d66` | TASK-0601 | B-4 | LIFO non-protected stalemate fallback to next_id |
| `d48fefc` | TASK-0602 | C-1 | `BenchmarkSuiteConfig` Tier 3 fields |
| `ad8ff8c` | TASK-0603 | C-3 | `BenchArgs` Tier 3 CLI flags |
| `0cedff1` | TASK-0604 | C-2+C-4 | Bench path selection + `ep_annihilation_stream` wiring |
| `b2c84ea` | TASK-0605 | C-5 | `peak_memory_during_construction` probe + CSV column |
| `a1b65fd` | TASK-0606 | D-1 | Sparse construction path for `dual_tree` |
| `9c98030` | TASK-0607 | D-3 | `sparse_construction_memory.csv` sub-writer |
| `fac1eb6` | TASK-0608 | E-1 | Dockerfile workspace-aware |
| `bd8a047` | TASK-0609 | E-2 | docker-compose `bench-tcp` profile + DOCKER.md |
| `6b182e8` | TASK-0610 | E-4 | TCP smoke CI + QA-D009-001 witness |
| (covered) | TASK-0596 (spec) | A | SPEC-19 R35a amendment (`c4c80b8`) — already closed pre-bundle |
| (covered) | (spec) | F-1 | SPEC-09 R18a-R18g, R37c, §4.9 amendment (`82b2d27`) — already closed pre-bundle |

**Code quality verdict:** **PASS WITH NOTES** (no `unwrap()` in new prod code; no new `unsafe`; one new `tracing::warn!` is correctly placed; legacy `println!` site at `bench/suite.rs:748` was carried forward unchanged — pre-existing CLI surface).

**Architecture verdict:** **MINOR DRIFT** (one architectural deviation in TASK-0604 is well-documented and defensible; one R37c enforcement gap and one R39a schema-completeness gap.).

**Spec compliance:** SPEC-19 R35a fully landed; SPEC-22 R10/R12 hardening landed; SPEC-21 R37b/R37g landed; **SPEC-09 R18b/R18c/R18d/R18e/R18f/R18g not yet emitted on the BenchmarkResult struct or detail.csv schema** — see MF-002.

**Test floor at HEAD:** 1756 default / 1800 zero-copy / 1749 streaming-no-recycle (vs target 1683/1726/1680). +73 default tests above the pre-bundle floor — large margin. v1 floor 690 preserved.

---

## Top-3 Most Concerning Items

1. **MF-001 — SPEC-09 R37c (construction-phase isomorphism gate) is documented in comments but NOT enforced at runtime by the bench harness.** When `representation == Sparse` OR `chunk_size.is_some()`, R37c "MUST additionally assert that the net OBSERVED at the end of construction is graph-isomorphic to the eager-constructed reference net". The harness `run_benchmark_suite` does not perform this check; only the static unit-test suite (`spec09_bench_streaming_path::it_0604_07`, `spec22_sparse_net_dual_tree_bench::isomorphism_at_dual_tree_*`) exercises it. A streaming bug introduced AFTER D-011 closes that produces a structurally divergent but reducible net would silently pass R37 (post-reduction) on every bench rodada. R37c is the only sentinel that catches this class of regression at production scale.

2. **MF-002 — SPEC-09 R18b/R18c/R18d/R18e/R18f/R18g are not implemented on `BenchmarkResult` or the `detail.csv` schema.** TASK-0605 added only `peak_memory_during_construction` (R18a). The §4.9 schema MUST list 7 new columns appended to the v1 22 columns: `peak_memory_during_construction, peak_memory_during_reduction, agent_count_at_construction_complete, live_agent_count_watermark, representation, chunk_size, recycle_policy`. The current `write_csv_detail` emits 23 columns, not 29. The "field-population discipline" clause in SPEC-09 §4.9 line 538 MUST is therefore unsatisfied for v1-equivalent rodadas as well as Tier 3 ones — the rightmost 7 columns MUST NOT be omitted. This blocks the F-2 bench rodada from being R39a-compliant.

3. **MF-003 — TASK-0601 `tracing::warn!` for LIFO stalemate fires on EVERY `create_agent` call hitting the stalemate, not once per stalemate event.** The dispatch brief specified "exactly once per stalemate event (not on every coordinator round)". Current implementation at `relativist-core/src/net/core.rs:387-392` emits a warn each time the protected-only stack is consulted. Under a sustained stalemate (entirely plausible across many redex-creation calls in a single round), this floods the log and could mask other diagnostics. Counter increment is correct (debug-only); the warning is not rate-limited.

---

## Summary

The D-011 bundle is a large and clean delivery. Phase B (hardening) cleanly closes 6 of 7 deferred QA findings from D-009/D-010. Phase C (bench wiring) ships an opt-in Tier 3 path that preserves the eager status quo bit-for-bit (UT-0602-01 locks defaults; IT-0604-12 verifies eager-path stability). Phase E (docker + TCP smoke) is well-scoped and documented. The two SPEC amendments (SPEC-19 R35a, SPEC-09 R18a-g/R37c) landed pre-bundle and the implementation faithfully follows clauses (a)-(g) of R35a.

The findings inventory below is dominated by **integration completeness gaps**, not logic errors. The largest is MF-002 (six R18 columns missing from `BenchmarkResult`/`detail.csv`); the second largest is MF-001 (R37c not enforced at harness level — only at test level). Both are addressable in Phase 6 refactor without disturbing any logic that has already passed CI.

The architectural deviation in TASK-0604 (SparseNet+to_dense streaming-path assembly instead of `merge::merge` chunked partitioning) is **ACCEPTED**: the documented reason (`merge::core::merge` debug-asserts on post-reduction partitions, which streaming construction violates by definition) is sound, and the workaround preserves R37c agent-isomorphism via a different mechanism (sparse incremental assembly with `Pending` directive resolution). The trade-off — that `representation=Dense` is a no-op for the streaming path because internal assembly is sparse — should be documented in SPEC-09 §4.9 in a follow-up amendment, but is not a blocker for D-011 close-out.

---

## Findings Inventory

### 2.1 Spec Compliance

#### MF-001 — SPEC-09 R37c not enforced at harness runtime

**Severity:** MAJOR
**Spec:** SPEC-09 R37c (commit `82b2d27`); SPEC-09 R38 (halt on correctness failure)
**Files:** `relativist-core/src/bench/suite.rs:609-808` (`run_benchmark_suite`)

**Problem:**
R37c reads "When `representation == Sparse` OR `chunk_size.is_some()` ... the verifier MUST additionally assert that the net OBSERVED at the end of construction ... is graph-isomorphic to the eager-constructed reference net produced by `Benchmark::make_net(size)`." This is a runtime obligation on every datapoint, falling under R38's halt-on-failure regime.

The current harness:
- Builds the Tier 3 net (sparse or streaming) via `build_input_net_from_suite`.
- Captures `peak_memory_during_construction` (R18a) immediately after.
- Proceeds directly to `measure_sequential` / `measure_grid`.
- Never builds the reference eager net `bench.make_net(size)` for comparison.
- Never invokes `nets_isomorphic` or the lightweight R37a fast-path against the reference.

The R37c sequencing constraint at SPEC-09 lines 680-689 mandates 6 specific steps including "3. Build the reference eager net via `Benchmark::make_net(size)` ... 4. Run `nets_isomorphic` ... 5. Discard the reference ...". None of these are present in `run_benchmark_suite`.

Test-level coverage exists (`it_0604_07_streaming_vs_eager_merged_net_isomorphism`, `pt_0604_10`, `isomorphism_at_dual_tree_small_size`) but is bounded to 12 proptest cases at 50-400 size and a few hand-picked sizes. The full bench rodada at `--sizes 10000` workers `1,2,4,8` has zero R37c protection.

**Recommended fix:**
Add a runtime R37c gate in `run_benchmark_suite` at the program point where R18a is captured (after `build_input_net_from_suite` returns). When `config.chunk_size.is_some() || config.representation == NetRepresentation::Sparse`, build the eager reference via `bench.make_net(size)` after the R18a sample (per the §4.9 sequencing constraint), invoke `nets_isomorphic` (or R37a fast-path for nets > 5000 agents), and return `Err(...)` with the divergence details on mismatch (per R38). Drop the reference net before proceeding to reduction so it does not poison `peak_memory_bytes` (it cannot poison R18a since R18a is already frozen).

The test suite already encodes the contract; harness-level enforcement is the missing wiring.

---

#### MF-002 — SPEC-09 §4.9 R39a detail.csv schema is incomplete (6 of 7 Tier 3 columns missing)

**Severity:** MAJOR
**Spec:** SPEC-09 R18b-R18g (commit `82b2d27`); SPEC-09 R39a (lines 528-538, 701-714); SPEC-09 §4.9 field-population discipline (line 538)
**Files:** `relativist-core/src/bench/mod.rs:178-238` (`BenchmarkResult` struct); `relativist-core/src/bench/csv.rs:14-54` (`write_csv_detail`)

**Problem:**
SPEC-09 R39a (line 711) locks the schema:

```
benchmark,...,dup_era,
peak_memory_during_construction,peak_memory_during_reduction,
agent_count_at_construction_complete,live_agent_count_watermark,
representation,chunk_size,recycle_policy
```

That is 22 v1 columns + 7 Tier 3 columns = 29 columns total, MUST. The line 538 clause "the rightmost 7 columns MUST NOT be omitted from a v1-equivalent rodada's CSV" is unambiguous.

Current state:
- `BenchmarkResult` (lines 180-238 of `bench/mod.rs`) has only ONE of the 7 new fields: `peak_memory_during_construction` (R18a). It does NOT carry: `peak_memory_during_reduction` (R18b), `agent_count_at_construction_complete` (R18c), `live_agent_count_watermark` (R18d), `representation` (R18e), `chunk_size` (R18f), `recycle_policy` (R18g).
- `write_csv_detail` writes 23 columns (the v1 22 plus `peak_memory_during_construction`).

A `grep` of `relativist-core/src/` for `peak_memory_during_reduction|agent_count_at_construction_complete|live_agent_count_watermark` returns zero matches.

This blocks Phase F-2 (the bench rodada) from being R39a-compliant. Downstream Python tooling that joins on the documented Tier 3 schema will fail.

**Recommended fix:**
Add the 6 missing fields to `BenchmarkResult`. Wire them through:

- R18b `peak_memory_during_reduction: u64` — sample VmHWM AFTER `reduce_all` / `run_grid` returns (this is essentially what `peak_memory_bytes` already captures end-of-run; R18b can reuse the same probe at the same call site, dual-stored under the new name; clarify the relationship in a comment).
- R18c `agent_count_at_construction_complete: u64` — set per the dispatching discipline (sparse path: `SparseNet::agents.len()`; streaming: sum of `AgentBatch::agents.len()`; eager: `net.live_agents()`). Capture in `build_input_net_from_suite` and forward into rows.
- R18d `live_agent_count_watermark: u64` — peak of `count_live_agents()` observed during reduction. In sequential, sample at end-of-reduction. In grid, requires per-round watermarking; deferring to a SPEC clarification is acceptable IF documented as such. SPEC-09 R18d's exact sampling discipline should be re-read in case the spec already permits an end-of-reduction snapshot.
- R18e/R18f/R18g — copy from `BenchmarkSuiteConfig` directly into every row (already on the config; just thread into the struct).

Then extend `write_csv_detail` to emit all 29 columns in the spec-mandated order. Update `IT-0605-04` (or add a new test) to lock the column count and the column order against R39a verbatim.

---

#### MF-003 — LIFO stalemate `tracing::warn!` is not rate-limited

**Severity:** MAJOR (operational; observability degradation)
**Spec/QA:** QA-D010-016 / dispatch brief 2026-04-30 ("warning fires exactly once per stalemate event")
**File:** `relativist-core/src/net/core.rs:387-392`

**Problem:**
The warn site fires unconditionally inside the `None` (true-stalemate) branch of every `create_agent` call. Under a sustained stalemate (e.g., a coordinator round that creates 1000 fresh agents while every free-list entry is border-protected), the log receives 1000 warnings — one per allocation. This obscures other diagnostics and inflates log volume in production rodadas.

The counter `Net::lifo_stalemate_fallbacks` is debug-only and does increment correctly. The user prompt explicitly flagged this as a verification target.

**Recommended fix:**
Two acceptable options (both preserve `lifo_stalemate_fallbacks` increment):

(a) **Rate-limited warn:** track `last_stalemate_warn_at_count: u64` on `Net` (or in a `tracing` `OnceCell`) and only emit when `lifo_stalemate_fallbacks` crosses a threshold (every 1000 fallbacks, or once per coordinator round if the round counter is accessible).

(b) **One-shot per `Net` instance:** track `stalemate_warned: bool` on `Net` (debug-only field per TASK-0598's strategy-(b) ABI parity), set to `true` after the first warn, suppress subsequent warns. Reset on `Net::new()`.

Option (a) is more informative for diagnosing real stalemate storms; option (b) is simpler and matches the brief's text more literally. Either satisfies the dispatch brief.

---

#### SF-001 — SPEC-09 R18d `live_agent_count_watermark` sampling discipline is undefined for grid mode

**Severity:** MINOR
**Spec:** SPEC-09 R18d (commit `82b2d27`)
**Files:** SPEC and harness both unclear; flagged for SPEC clarification.

**Problem:**
SPEC-09 R18d at lines 489-491 (truncated by tool but referenced elsewhere) describes a "live_agent_count_watermark" without specifying whether the watermark is sampled per-round or end-of-reduction in grid mode. Sequential mode is unambiguous (one final sample). The implementation in MF-002 should pick one discipline; at the same time, request a SPEC clarification via the spec-critic / especialista-em-specs path.

**Recommended fix:**
Pick end-of-reduction snapshot as the default operational discipline (cheapest to wire). Surface the choice as a comment in `BenchmarkResult`. Track a follow-up SPEC item to either ratify this or force per-round sampling.

---

#### SF-002 — `peak_memory_during_construction` is `u64` not `Option<u64>` end-to-end; non-Linux blank-rendering relies on a sentinel value 0

**Severity:** MINOR
**Spec:** SPEC-09 §4.9 (non-Linux convention); user-flagged deviation #3
**Files:** `relativist-core/src/bench/mod.rs:216` (field type); `relativist-core/src/bench/suite.rs:310-316` (`peak_for_sparse_row` converts u64→Option<u64> at sub-CSV emit time only); `relativist-core/src/bench/csv.rs:50` (main detail.csv prints `r.peak_memory_during_construction` raw — emits literal `0` on non-Linux, NOT blank).

**Problem:**
The sub-CSV (`sparse_construction_memory.csv`) correctly converts 0→blank via `peak_for_sparse_row` (UT-0607 locks this). The main `detail.csv` does NOT — it writes the raw `u64`, so a non-Linux developer's bench rodada will produce `peak_memory_during_construction=0` rows that are indistinguishable from a hypothetical "construction used 0 bytes" outcome. SPEC-09 §4.9 (per the user's prompt) prefers blank string for "not measured".

**Recommended fix:**
Either:
(a) Change the field type on `BenchmarkResult` to `Option<u64>` and update all writers to render `None` as blank.
(b) Keep the raw `u64` field and treat 0 as the sentinel at the writer level (mirror `peak_for_sparse_row` in `write_csv_detail`).

Option (a) is type-safe but breaks the existing `BenchmarkResult` Serialize derive. Option (b) is local. Pick one and apply consistently across all 7 R18 columns when MF-002 is addressed.

---

### 2.2 Module Boundaries / Dependency Direction

#### PASS — All B/C/D phase changes respect SPEC-13 dependency direction

`net <- reduction <- partition <- merge <- protocol`. The new code:

- `bench/suite.rs` builds `SparseNet` (in `net::sparse`) and consumes `partition::streaming::ConnectionDirective` — both are below `bench` in the layering, and `bench` is leaf.
- `partition/compact.rs::CompactSubnet` gains a `free_list` field that mirrors `Net::free_list` (in `net`) — same layer (`partition`) consuming a type from a strictly lower layer (`net`). Direction-respecting.
- `coordinator.rs` imports `PullCoordinatorState` (defined in `coordinator.rs` itself); no cross-layer leak.
- TASK-0598 ABI parity moves counter fields from cfg-gated to always-present. The fields live on `Net` (lowest layer) and are consumed by `partition/compact.rs`, `partition/helpers.rs`, `partition/remap.rs`, `net/sparse.rs` — all valid up-direction reads.

No circular dependencies. No new feature-gated module crossing into core.

---

### 2.3 Code Quality

#### MF-004 — `bench::memory::get_peak_memory_bytes` and `get_peak_memory_during_construction` are byte-identical functions

**Severity:** MAJOR (DRY / Clean Code)
**File:** `relativist-core/src/bench/memory.rs:8-50`

**Problem:**
Both functions read `/proc/self/status` VmHWM on Linux, return 0 elsewhere, share the helper `read_vmhwm_bytes`. The doc-comments document a different SEMANTIC contract (call site discipline) but the bodies are identical. A reader has to read the doc-comments to understand the difference; the type system does not encode it.

**Recommended fix:**
Two options:

(a) **Single function + ZST marker:** keep one function, document the call-site discipline in its doc-comment, and add `#[deprecated]` on the duplicate name.

(b) **Newtype phantom:** introduce `pub struct ConstructionPhase; pub struct ReductionPhase;` ZSTs and a generic `pub fn sample_vmhwm<P>() -> u64`. The two call sites become `sample_vmhwm::<ConstructionPhase>()` / `sample_vmhwm::<ReductionPhase>()` — the generic parameter encodes the call-site discipline at the type level.

Option (a) is enough; the cost saving is minor. Option (b) is overkill but is the type-theoretic fix.

---

#### SF-003 — `build_input_net_from_suite` mixes 4 unrelated concerns in one function (~165 lines)

**Severity:** MINOR (Single Responsibility / Function size)
**File:** `relativist-core/src/bench/suite.rs:126-292`

**Problem:**
The function:
1. Branches on `chunk_size`.
2. Branches on `representation`.
3. Drives the eager dense path (1 line).
4. Drives the sparse-eager path (~20 lines, including `make_sparse_net` + `to_dense`).
5. Drives the streaming-sparse-assembly path (~115 lines, including `Pending` directive resolution + lifetime check + post-stream completeness check + `to_dense`).

Function is 165 lines (target < 20). Cyclomatic complexity > 5. Each branch could be its own helper:

- `build_input_net_eager_dense(bench, size) -> Net`
- `build_input_net_eager_sparse(bench, size) -> Result<Net, String>`
- `build_input_net_streaming(bench, size, chunk_size, max_pending_lifetime) -> Result<Net, String>`

Then `build_input_net_from_suite` becomes a 5-line dispatcher.

**Recommended fix:**
Extract the three branch bodies into private helpers in `bench/suite.rs`. The current function survives as the 5-line public dispatcher. Tests are unchanged.

---

#### SF-004 — `BenchmarkSuiteConfig` is now 17 fields (god struct risk)

**Severity:** MINOR (god struct trend)
**File:** `relativist-core/src/bench/mod.rs:350-395`

**Problem:**
17 public fields on a config struct, no builder. Adding a new option (e.g., `verbose_breakdown` per SPEC-09 R48 note) is a breaking API change (or requires updating every test fixture). Several existing tests construct the struct via `..Default::default()` — that's because a `Default` impl exists, but the struct-literal style at 17 fields is unwieldy.

**Recommended fix:**
Add a `BenchmarkSuiteConfigBuilder` (idiomatic Rust builder pattern). Existing test sites and CLI surface remain unchanged; new tests prefer the builder. Not blocking for D-011.

Alternatively: split the 4 Tier 3 fields into a `Tier3Config` sub-struct and embed it. Smaller change, same readability win.

---

#### SF-005 — `peak_for_sparse_row` is a private function used only by the sub-CSV writer; should be co-located with `SparseConstructionRow`

**Severity:** MINOR (cohesion)
**Files:** `relativist-core/src/bench/suite.rs:310-316` (function); `relativist-core/src/bench/csv.rs:163-213` (consumer)

**Problem:**
The sentinel-conversion logic is in `suite.rs` but only `csv.rs` consumes its output. Move it to `csv.rs` as a private helper (or replace it inline; it's 6 lines).

**Recommended fix:**
Move `peak_for_sparse_row` to `bench/csv.rs` as a private fn or inline at the call site. Update IT-0607 reference if it cited the path.

---

#### SF-006 — `bench/suite.rs:748` uses `println!` (operational, but inconsistent with project standard)

**Severity:** MINOR (pre-existing pattern; carried forward unchanged by D-011)
**File:** `relativist-core/src/bench/suite.rs:748`

**Problem:**
Project coding standard says "No `println!` — use `tracing` macros only". The CLI bench output is one of 4 surviving sites (3 are in `commands.rs` / `bench/validate.rs`, all of which are CLI surfaces where `println!` is operationally justified). D-011 did not introduce this; not a regression.

**Recommended fix (defer):**
Replace with `tracing::info!` once the CLI's tracing subscriber is wired to print to stdout. Not blocking; track in a follow-up bundle.

---

### 2.4 Test Discipline

#### PASS — Test specs honored; gates clean

The 15 test specs `TASK-0596-tests.md`..`TASK-0610-tests.md` map cleanly to delivered tests. All commit messages cite the spec section. Test floor delta (+73 default) exceeds the +50 estimate from the consolidated plan.

#### SF-007 — IT-0606-04 80% memory gate is loose at depth 12

**Severity:** MINOR (acknowledged limitation)
**File:** `relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs:174-260`

**Problem:**
The test sets a relaxed `<80%` ratio gate per the D-011 plan §D-2, but at depth 12 (~16k agents) the dense-arena allocation may not dominate process memory enough for the gate to be discriminating. The test doc-comment acknowledges this. The original SPEC-22 acceptance ("<30% dense in ep_construct(5M)") would require larger sizes than `dual_tree`'s tractable depth range supports (depth 14 takes ~3 min in debug per the comment).

**Recommended action:**
ACCEPT the gate as a Phase D-1 informative micro-bench; flag for v2.1 to expand sparse benchmarks to a workload that hits the SPEC-22 acceptance threshold.

#### SF-008 — IT-0599-02/03/04 share workload helper but feature-cfg coverage is split

**Severity:** MINOR (test discoverability)
**File:** `relativist-core/tests/spec22_streaming_no_recycle.rs`

**Problem:**
The four-test bundle has clean intent but a slightly tangled cfg matrix:

- IT-0599-02 — `cfg(not(feature = "streaming-no-recycle"))`
- IT-0599-03 — `cfg(feature = "streaming-no-recycle")`
- IT-0599-04 — unconditional, runtime-cfg-branched

This is correct — both feature settings get coverage AND the unconditional runtime check guards against macro-expansion accidents. Documentation (in commit message) is good. No fix needed; flag as a pattern other reviewers should look out for in future cross-feature tests.

---

### 2.5 Architectural Deviations

See Section 3 (per-deviation verdict) below.

---

## 3. Per-Deviation Verdict

### Deviation 1 (TASK-0596): LIFO direction, error variant, T8 isolation, double PROTOCOL_VERSION bump

**Verdict:** ACCEPT (all four sub-points)

The `.last()` semantics is correct per `Net::create_agent`'s actual SPEC-22 R5/R10c LIFO discipline (top of stack is the most recently pushed entry, retrieved by `Vec::pop`). The user prompt's "spec said `[0]` is first to pop" is itself a misreading of R10c — the spec text describes "first popped after the most recent push", which IS `.last()`. The code is right; the spec citation in TASK-0596's brief was loose.

`Message::RegisterNack` carrying a string reason is the project's current `UnsupportedVersion`-class error pathway. Adding a structured `ProtocolError::UnsupportedVersion` variant is a wider refactor and not in scope for D-011. IT-0596-08 verifies the rejection fires; IT-0596-11 is the bug-witness sentinel — both are sufficient.

The `test_compact_smaller_for_sparse` modification (drain `free_list` before encoding) is correct and necessary: the post-R35a wire form carries `free_list` as a `Vec<AgentId>` whose serialized cost scales with tombstone count, so the original "compact*3 < dense" inequality no longer holds when many tombstones have parked their ids in `free_list`. The test's intent (validate agents-arena compression) is preserved by the drain.

Both `PROTOCOL_VERSION` (6→7) AND `PREVIOUS_LIVE_VERSION` (5→6) bumped is correct per the const_assert `PROTOCOL_VERSION == PREVIOUS_LIVE_VERSION + 1` invariant. UT-0596-07 locks the literal 7. IT-0596-08 locks the rejection of v6.

### Deviation 2 (TASK-0604): SparseNet+to_dense streaming-path assembly instead of `merge::merge`

**Verdict:** ACCEPT — but document in SPEC-09 §4.9 in a follow-up F-1 amendment

This is the highest-priority architectural decision in the bundle. The dev's justification is sound: `merge::core::merge` enforces D3 invariants (post-reduction partitions have no live redexes) via debug-asserts, and pre-reduction reconstruction violates this by definition. The chosen workaround — incremental SparseNet assembly with `Pending` directive resolution + `to_dense` at the end — preserves the construction-phase memory benefit (agents are emitted in chunks) while sidestepping the merge-path debug-asserts.

The trade-offs:
- The `representation=Dense` flag is effectively a no-op for the streaming path (internal assembly is sparse, regardless of the user's flag). This should be documented: streaming construction always passes through `SparseNet` internally, and `representation` only affects the eager path.
- R37c agent-isomorphism is preserved via a different mechanism: `Pending` directive resolution guarantees that the assembled `SparseNet` has every connection the eager `Net` has, by construction. PT-0604-10 (12 random cases) exercises this — coverage could be tighter, but the contract holds.
- The `chunk_size: Option<u32>` semantics still match SPEC-09 R29a (None=eager, Some(N)=streaming), which is the operator-facing contract.

The deviation does NOT violate R37c (isomorphism is preserved); it does NOT violate SPEC-21 R10/R12 (memory still scales with `chunk_size` because the SparseNet only holds the agents from chunks not yet `to_dense`-converted). It DOES leave R18a's "AFTER `merge::generate_and_partition_chunked_with_chunk_size_and_lifetime` returns" wording in SPEC-09 R18a's bullet list slightly stale — the actual return point is `SparseNet::to_dense`.

**Recommendation:** open a follow-up SPEC-09 amendment to clarify that the streaming path's R18a sample point is "AFTER the SparseNet's `to_dense(None)` returns" (functionally equivalent to "AFTER chunked partition pipeline returns" since both are the same instant in the dispatcher), and note the SparseNet-internal-assembly choice.

### Deviation 3 (TASK-0605): `peak_memory_during_construction: u64` not `Option<u64>`; non-Linux blank rendering

**Verdict:** FLAG_FOR_QA (covered by SF-002)

The sub-CSV writer correctly emits blank for 0 (UT-0607-01b). The main detail.csv writer does NOT. SF-002 prescribes the fix.

### Deviation 4 (TASK-0606): `dual_tree` size = depth, IT sizes 5/12 (not spec's 500/5000); error on non-dual_tree+sparse

**Verdict:** ACCEPT (size adaptation) + ACCEPT (error message clarity)

The depth-vs-agent-count mismatch is correctly documented in the test files' doc-comments. Depth 12 = ~16k agents per tree, ~32k total — large enough to exercise dense arena allocation and id-renumbering pressure during reduction.

The "Behavior A" hard-error for non-`dual_tree` benchmarks with `representation=sparse` is correctly implemented (`Benchmark::make_sparse_net` default impl returns `Err(...)` with a descriptive message at `bench/mod.rs:159-166`). The error text is clear: "benchmark X does not support representation=sparse (D-011 Phase D-1 scope: dual_tree only); use representation=dense". IT-0606-06 in `spec22_sparse_net_dual_tree_bench.rs:355-366` verifies the error propagates through `run_benchmark_suite`.

### Deviation 5 (TASK-0607): Auto-pair behavior — when `--csv-sparse` set + sparse representation, ALSO build dense

**Verdict:** ACCEPT — but verify the dense build is NOT added to detail/rounds/summary CSVs

The auto-pair (suite.rs:682-702) builds the dense reference for the `ratio_to_dense` computation but does NOT add a row to `all_results` / `all_summaries`. The dense net is built then dropped; only the VmHWM probe value is captured for `sparse_construction_rows`. This is the right call: adding a dense row to detail.csv would pollute the operator's intended `representation=sparse` output and break a hypothetical pandas join on `(benchmark, size, representation)`.

**Verification ask for QA (Stage 5):** confirm no row whose `representation=Dense` appears in `--csv-detail` output when the user passes `--representation sparse --csv-sparse`. IT-0607-04 may already cover this; if not, QA should add an assertion.

### Deviation 6 (TASK-0598): Counter fields always-present, writes debug-only (Strategy B)

**Verdict:** ACCEPT

Strategy (b) is documented in the dispatch brief and is the right call: same struct layout across debug/release means a debug-built coordinator can serialize/deserialize a `Net` against a release-built worker without ABI drift. Read paths in tests are correctly gated `#[cfg(debug_assertions)]` for assertion-style checks (UT-0598-02 in `net/core.rs::tests`). IT-0598-05 in `tests/spec22_debug_release_abi_parity.rs` is the compile-time fence. No regression.

### Deviation 7 (TASK-0600): Canonical `PullCoordinatorState`; `CoordinatorState::Pull*` variants kept for ABI

**Verdict:** ACCEPT

The `From<PullCoordinatorState> for CoordinatorState` projection at `coordinator.rs:893-908` is the only producer of the legacy variants. IT-0600-03 in `tests/spec13_pull_state_collapse.rs` is a text-grep regression fence that scans the production sources for `= CoordinatorState::Pull*` outside the canonical From impl. Two parallel state representations do exist in the type system, but only one is the source of truth (the canonical `PullCoordinatorState`); the legacy enum is an output projection. UT-0600-01/02 cover the projection and exhaustiveness.

### Deviation 8 (TASK-0601): LIFO non-protected stalemate fallback; warn fire frequency

**Verdict:** REJECT (the warn-frequency point) — see MF-003

The deeper-scan + fall-through to `next_id` strategy is correct (UT-0601-01 / IT-0601-02 / IT-0601-02b cover the four cases). The performance posture (O(free_list.len()) only under Strategy B + delta gate) is acceptable.

The warn-fires-once semantics in the dispatch brief is NOT honored (warn fires per-call, not per-event). MF-003 covers the fix.

---

## 4. Summary Metrics

**Total findings:** 4 MAJOR (MF-001..MF-004) + 8 MINOR (SF-001..SF-008) + 0 SUGGEST = 12 findings

**Files Stage 5 (QA) should hammer:**

1. **`relativist-core/src/bench/suite.rs:609-808`** (`run_benchmark_suite`) — for MF-001 (R37c gate absent), MF-002 (R18b-g missing), and the auto-pair behavior verification (Deviation 5). Adversarial inputs: streaming path with deliberately mis-built `make_net_stream` override that produces a non-isomorphic net.
2. **`relativist-core/src/bench/csv.rs:14-54`** (`write_csv_detail`) — for MF-002. Verify column count, column order, and v1-baseline joinability after the schema fix.
3. **`relativist-core/src/net/core.rs:355-395`** (LIFO stalemate) — for MF-003. Construct an adversarial workload that triggers a sustained stalemate over ~1000 allocations and observe log volume.
4. **`relativist-core/src/partition/compact.rs::tests::nets_equivalent`** — for the SPEC-19 R35a regression sentinel; ensure no future field added to `CompactSubnet` slips past `nets_equivalent` (the helper compares 6 fields explicitly; a 7th field added without updating the helper would silently green-light a regression).
5. **`relativist-core/tests/spec22_sparse_net_dual_tree_bench.rs:197-260`** (IT-0606-04) — for SF-007. On a Linux CI agent with a known-fixed memory profile (e.g., the `docker-bench-smoke` runner from TASK-0610), capture the actual sparse/dense ratio at depth 12 and document whether the 80% gate is meaningful or vacuous.

**Commits Stage 5 should re-bisect under fault injection:**

- `0cedff1` (TASK-0604, the largest LoC commit and the streaming-assembly architectural deviation) — fault inject by writing a deliberately-buggy `make_net_stream` override on a forked benchmark and confirming R37c-violation is caught (currently it would NOT be at the harness level — see MF-001).
- `abd2976` (TASK-0596, the wire-format change) — adversarial test: a worker speaking v6 sending a Partition payload; verify rejection.

---

## 5. Recommendation

### For Stage 5 (qa):

Focus on the three MAJOR findings. Specifically:

1. **MF-001 fault injection.** Write an adversarial test that produces a streaming-path `Net` that is reduction-equivalent but NOT agent-isomorphic to the eager reference (e.g., missing one wire that the reduction can recover). Confirm it currently passes `run_benchmark_suite` (MF-001 is real). After Stage 6 fixes MF-001, the same test must FAIL with R37c divergence error.

2. **MF-002 schema completeness.** Pull the v1_local_baseline `phase2_detail.csv` header and assert that the column count + order delta against the post-D-011 detail.csv matches the SPEC-09 R39a-mandated 7 new columns. This will currently FAIL (only 1 of 7 columns is emitted). After Stage 6 fixes MF-002, assert all 29 columns present with correct values for a v1-equivalent rodada.

3. **MF-003 stalemate log volume.** Run a workload that triggers ~1000 stalemate fallbacks under `tracing` capture; assert the warn count is 1 (after fix) or O(1) per event boundary, NOT 1000 (current behavior).

4. **Bench rodada smoke.** Run `cargo run --release --bin relativist -- bench --benchmark ep_annihilation --sizes 1000 --workers 1,2 --chunk-size 100 --max-pending-lifetime 16` end-to-end. Verify (a) it completes, (b) CSVs are produced, (c) the eager-path equivalent (`--chunk-size <unset>`) produces detail.csv rows that bit-stably match `v1_local_baseline/phase2_detail.csv` for the v1 22 columns at the matched (benchmark, size, workers, mode) rows.

### For Stage 6 (developer refactor):

**Must address before close-out (blocking):**
- MF-001 (R37c gate)
- MF-002 (R18b-g schema completion)
- MF-003 (warn rate-limit)
- MF-004 (memory.rs DRY)

**Should address (non-blocking but in same PR if cheap):**
- SF-002 (peak_memory_during_construction blank rendering in main detail.csv)
- SF-003 (function decomposition of `build_input_net_from_suite`)
- SF-005 (move `peak_for_sparse_row` to csv.rs)

**Defer to follow-up (track in next-steps.md):**
- SF-001 (R18d sampling discipline SPEC clarification)
- SF-004 (BenchmarkSuiteConfig builder)
- SF-006 (println! → tracing::info!)
- SF-007 (sparse memory gate at larger sizes — v2.1)
- SF-008 (no action — pattern documentation only)

**Architectural follow-up (not blocking D-011 close):**
- File a SPEC-09 §4.9 amendment to clarify TASK-0604's SparseNet-internal-assembly choice and the resulting "no-op `representation=Dense` for streaming path" semantics. The SPEC currently implies merge-pipeline assembly; the implementation is sparse-pipeline. Realign.

After the four MF fixes land, this bundle is **CLOSE_OUT-ready**. The D-011 verdict moves from ACCEPT_WITH_FIXES to ACCEPT once MF-001..MF-004 are green and a re-review confirms no logic regression.
