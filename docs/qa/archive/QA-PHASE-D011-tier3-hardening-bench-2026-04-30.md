# QA Bug Report: D-011 Tier 3 Hardening + Bench Enablement (Phases A–F)

**Date:** 2026-04-30
**Auditor:** QA agent (Stage 5, adversarial bug hunting)
**Branch:** v2-development @ HEAD `69d5d66`
**Commits attacked:** 16 (TASK-0596..TASK-0610)
**Method:** Static analysis + spec cross-reference. Cargo runtime not available in this audit environment; `cargo` repros marked "speculative" where applicable (Stage 6 must verify). Most critical findings reproduce by reading code paths directly.

---

## 1. Executive Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH     | 4 |
| MEDIUM   | 5 |
| LOW      | 4 |
| **Total** | **15** |

Plus 9 edge-cases verified clean and 4 stress scenarios documented.

The reviewer's MF-001..MF-004 are confirmed; MF-001 and MF-002 are **extended** to wider impact than the reviewer flagged. The most consequential new finding is **QA-D011-001 (CRITICAL):** the path-selection precedence in `build_input_net_from_suite` makes `representation=Sparse` a silent no-op when `chunk_size=Some(N)` is also set — and the `make_sparse_net` "non-`dual_tree`" guard is bypassed entirely. This blocks any meaningful Phase F-2 sparse benchmark unless an operator carefully avoids the combination.

---

## 2. Findings Inventory

### QA-D011-001 (CRITICAL) — `representation=Sparse` is silently ignored when `chunk_size=Some(N)`; non-`dual_tree` benchmarks bypass the sparse-not-supported guard

**Severity:** CRITICAL (silent semantic correctness regression — produces rows whose `representation=sparse` is mis-stamped)
**File:** `relativist-core/src/bench/suite.rs:138-291` (`build_input_net_from_suite`)
**Spec violated:** SPEC-09 R18e (representation provenance) + SPEC-22 R12 (sparse path semantics) + Behavior A (D-011 dispatch — non-`dual_tree` MUST error under sparse)

**Reproduction (analytic — not yet executed):**

```bash
cargo run --release --bin relativist -- bench \
  --benchmark ep_annihilation \
  --sizes 1000 \
  --workers 1,2 \
  --representation sparse \
  --chunk-size 100 \
  --csv-detail /tmp/bug.csv
```

**Expected behavior:** Per Behavior A (D-011 plan §D-1), this should fail loudly because `EpAnnihilation::make_sparse_net` returns `Err(...)`. Per SPEC-09 R18e, when the row IS produced, `representation=sparse` must reflect the actual data structure used.

**Actual behavior:** The `match config.chunk_size` outer branch matches `Some(chunk_size)` first (lines 177-291). The inner `match config.representation` is only consulted when `chunk_size == None`. Result:
1. The `make_sparse_net` Err is never raised — `ep_annihilation + sparse` runs without complaint.
2. The streaming path's internal SparseNet assembly is taken regardless of `representation`. The `representation` field is purely cosmetic in this combination.
3. Per Reviewer Deviation 2 already accepted, the streaming path ALWAYS uses internal SparseNet assembly. So `representation=Dense + chunk_size=Some(N)` is ALSO a SparseNet-internal-assembly run — i.e. the `representation` field misrepresents the data structure on every chunked row. The reviewer flagged this as an architectural ambiguity needing a SPEC-09 §4.9 amendment; QA confirms it materially corrupts the eventual R18e column once MF-002 lands the field.

**Root cause:** The match-on-chunk-size-first dispatch loses the `representation` axis. Either the dispatch must be a 2x2 grid (4 explicit cases) OR `chunk_size=Some(_)` AND `representation=Sparse` (without `dual_tree`) must error before constructing the stream.

**Recommended fix (NOT prescriptive code):** At the top of `build_input_net_from_suite`, validate the orthogonal combinations explicitly. For `representation=Sparse`, require that the benchmark supports `make_sparse_net` regardless of `chunk_size`. For `chunk_size=Some(_)`, document (and enforce in row provenance) that the construction path is "streaming-via-SparseNet" — distinct from "sparse-eager" — perhaps via a `ConstructionPath` enum on `BenchmarkResult` instead of (or in addition to) `representation`. Stage 6 should also gate non-`dual_tree+sparse` even under `chunk_size=Some(_)`.

**SPEC violation:** SPEC-09 R18e (representation must record "the construction-phase data structure used"); §4.9 sequencing constraint (non-dual_tree+sparse must Err per Behavior A).

---

### QA-D011-002 (CRITICAL) — `CompactSubnet::into_net` does not validate `free_list` integrity; can produce a `Net` that violates SPEC-22 R4 (recycled-slot freshness invariant) silently in release

**Severity:** CRITICAL (data corruption potential under malicious/corrupted wire payload)
**File:** `relativist-core/src/partition/compact.rs:103-146` (`CompactSubnet::into_net`)
**Spec violated:** SPEC-22 R4(b) (recycled slot must be `None` before pop), R10c (LIFO entries must not duplicate live ids)

**Reproduction (analytic):**

A malicious or corrupted wire payload arrives with:
- `live: [(5u32, era_agent, [DISC,DISC,DISC]), (6u32, era_agent, ...)]` (two live agents)
- `free_list: vec![5u32]` (id 5 appears in BOTH live AND free_list)
- `agent_arena_len: 7`

`CompactSubnet::into_net()`:
- Materializes `agents[5] = Some(era_agent)`, `agents[6] = Some(era_agent)`.
- Sets `free_list = vec![5]` verbatim (no validation).

The resulting `Net` violates SPEC-22 R4: when `create_agent` next pops id=5 from the free_list, the slot is `Some(_)` not `None`. In **debug builds** this fires `debug_assert!(self.agents[id].is_none())` at `core.rs:419-426` (panic). In **release builds** the `debug_assert!` is elided — the slot is silently overwritten and the previously-live agent is destroyed. **Because this is the full TCP wire path, a hostile peer can corrupt the receiving side's net** (mitigated only by the auth-token gate in non-default builds).

**Expected behavior:** `into_net` must verify the post-condition: every `id` in `free_list` MUST have `agents[id].is_none()` AND every `id` MUST be `< agent_arena_len`. Either return Err or sanitise (the latter masks the bug; the former is defensible at this layer).

**Actual behavior:** No validation. Round-trip is symmetric so happy-path tests pass, but adversarial / corrupted inputs are accepted.

**Root cause:** `into_net` is in the deserialization path (no separate validation step); both the bincode_v2 and rkyv paths land here.

**Recommended fix:** Add a post-condition check in `into_net` that:
- Every `id` in `free_list` is `< agent_arena_len`.
- Every `id` in `free_list` has `agents[id].is_none()` after the `live` scatter.
- `free_list` has no duplicates (HashSet check).
- No fabricated `id` ≥ `next_id`.
On violation, return a `Result` (requires changing the API) or sanitise + emit `tracing::error!`. Sanitising is preferred for backward-compatibility but loses the integrity signal; the project should pick one explicitly. UT-0596-04 already asserts arena-bound on round-trip — Stage 6 should add adversarial UTs for the duplicate / overlap cases.

**SPEC violation:** SPEC-22 R4(b), R10c. SPEC-19 R35a is silent on integrity validation — a CLARIFICATION amendment may be warranted.

---

### QA-D011-003 (HIGH) — Auto-pair `dense_cfg` clone in `run_benchmark_suite` does NOT clear `chunk_size` → `ratio_to_dense` is meaningless when both `--csv-sparse` and `--chunk-size` are set

**Severity:** HIGH (wrong scientific result silently produced)
**File:** `relativist-core/src/bench/suite.rs:682-702`
**Spec violated:** SPEC-09 §3.4.7 (`ratio_to_dense` definition implies "sparse vs dense-eager"), TEST-SPEC-0607

**Reproduction:**

```bash
cargo run --release --bin relativist -- bench \
  --benchmark dual_tree --sizes 10 \
  --representation sparse --chunk-size 100 \
  --csv-sparse /tmp/sparse.csv --csv-detail /tmp/detail.csv
```

**Expected behavior:** `ratio_to_dense` compares the sparse-eager construction peak (no streaming) against the dense-eager construction peak (no streaming).

**Actual behavior:** `dense_cfg` at line 683 is `config.clone()` with only `representation` flipped. `chunk_size = Some(100)` is preserved. So the "dense reference" goes through the streaming path (line 177 of `build_input_net_from_suite`), which assembles internally via SparseNet regardless. **Both arms of the ratio are streaming-via-SparseNet** — `ratio_to_dense` reflects ~1.0 (modulo VmHWM monotonicity noise) for any sparse+chunked bench rodada, NOT the sparse-vs-dense compression ratio that SPEC-22 R12 motivates.

Compounding: VmHWM is monotone non-decreasing across the same process. After the sparse main run, `VmHWM` already reflects the larger of (sparse-internal allocations, transient vec/hashmap overhead). The auto-pair dense build at line 686 raises VmHWM only if it exceeds the prior peak — which in the streaming/sparse-internal case it likely will not by much. Result: `dense_peak <= sparse_peak` is observable on small workloads (especially on Linux where VmHWM was already inflated by the test runner), giving `ratio_to_dense > 1.0` — implying sparse used MORE memory than dense. This is the opposite of what the column documents.

**Root cause:** `dense_cfg.chunk_size` is not cleared in the clone before the dense build.

**Recommended fix:** In `dense_cfg`, set `chunk_size = None` AND `representation = NetRepresentation::Dense` to force the eager-dense path. Document in TEST-SPEC-0607 that the auto-pair contract is "sparse-eager vs dense-eager" — both arms exclude the streaming path.

**SPEC violation:** SPEC-09 §3.4.7 (ratio semantics), SPEC-22 R12 (sparse representation contract).

---

### QA-D011-004 (HIGH) — `docker-bench-smoke` CI workflow defaults `--chunk-size=10000` for ALL bench-tcp invocations → CI never exercises the eager path; smoke results are streaming-path values mis-stamped as v1-equivalent

**Severity:** HIGH (CI doesn't validate what it claims; downstream regressions in eager path can land undetected)
**File:** `docker-compose.yml:60` (`--chunk-size=${CHUNK_SIZE:-10000}`); `.github/workflows/docker-bench-smoke.yml:61-65`, `109-114`
**Spec violated:** N/A (operational gap, not a spec violation per se; affects every PR going forward)

**Reproduction:** Trace the docker-compose-bench-tcp.yml: `BENCH_SIZES=1000` + default `CHUNK_SIZE=10000` ⇒ `chunk_size=Some(10000)` in `BenchmarkSuiteConfig`. `build_input_net_from_suite` takes the streaming branch (lines 177+). The CSV row written is a streaming-path row.

**Expected behavior:** The smoke should validate at minimum:
1. The eager path (chunk_size unset) — this is the v1-equivalent baseline.
2. The streaming path (chunk_size=Some) — separately, with the row's `chunk_size` column populated (after MF-002 lands).

**Actual behavior:** Only the streaming path is exercised. Worse, the IT-0610-02 grep is `grep -q "ep_annihilation"` — a row from EITHER path matches.

**Root cause:** The compose service was authored to validate the Tier 3 path; the eager-path coverage is implicit and was lost in the default.

**Recommended fix:** Either (a) Run two compose invocations in CI — one without `CHUNK_SIZE` (eager) and one with `CHUNK_SIZE=100` (streaming). (b) Set `CHUNK_SIZE=` (empty / unset) as the docker-bench-smoke default and override per-run in the CI step. Update IT-0610-02 to assert the eager-path row's `chunk_size` field (post-MF-002) is empty/None.

**SPEC violation:** None directly; but combined with MF-002 (no `chunk_size` column emitted) the smoke bench rodada produces rows whose data structure is unverifiable from the CSV.

---

### QA-D011-005 (HIGH) — `build_input_net_from_suite` streaming branch does NOT enforce SPEC-21 R37c construction-isomorphism vs. eager reference (extends MF-001)

**Severity:** HIGH — reviewer flagged as MAJOR (MF-001); QA extends.
**File:** `relativist-core/src/bench/suite.rs:177-291` + `run_benchmark_suite:609-808`
**Spec violated:** SPEC-09 R37c, R38 (halt on correctness failure), §4.9 sequencing constraint steps 3–5

**Reproduction:** N/A (the absence is the bug). To witness the gap concretely: write an adversarial benchmark that overrides `make_net_stream` to emit a stream that builds a structurally divergent net (e.g. miss one wire). Currently the harness will accept it without error.

**Confirmation of the reviewer's MF-001:** verified by source inspection — there is no `nets_isomorphic` call in `run_benchmark_suite` at all (grep returns 0). `bench.make_net(size)` is built only as `seq_reference_net` in the SEQUENTIAL baseline path (line 632, 766), and only the warmup uses it; it is NOT compared structurally against the streaming-built `input_net`.

**Extension:** the reviewer noted PT-0604-10 has 12 random cases at 50–400 size. QA observes additionally:
- `dual_tree` size IS a depth (per `dual_tree.rs:21-26`); IT-0606-04 caps at depth 12 (~16k agents). The full bench rodada at size 14 (per `default_sizes`) is NOT exercised by ANY R37c isomorphism test (PT-0604-10 stops at 400; IT-0604-07 uses `ep_annihilation`).
- The harness's only correctness gate at production scale is the post-reduction G1 check (sequential vs. distributed). G1 is a **post-reduction** isomorphism — it can pass even if the **pre-reduction** input net differed from the eager reference, provided the reductions are confluent over the difference. SPEC-09 R37c exists precisely because confluence-equivalent does NOT imply construction-isomorphic, and the difference materially affects bench measurements.

**Recommended fix:** As reviewer prescribed — add an R37c gate in `run_benchmark_suite` between line 647 (`build_input_net_from_suite`) and line 656 (R18a sample). Build `bench.make_net(size)` as a reference, run `nets_isomorphic` (or R37a fast-path for ≥5000 agents), drop the reference, then sample R18a. Currently every Phase F-2 sparse / streaming row is produced without this gate.

**SPEC violation:** SPEC-09 R37c (must-clause), SPEC-09 R38 (halt-on-failure regime), §4.9 sequencing constraint.

---

### QA-D011-006 (HIGH) — SPEC-09 R39a detail.csv schema is incomplete; 6 of 7 mandated Tier 3 columns missing (extends MF-002)

**Severity:** HIGH — reviewer flagged as MAJOR (MF-002); QA confirms and extends impact analysis.
**File:** `relativist-core/src/bench/csv.rs:14-54` (`write_csv_detail`); `relativist-core/src/bench/mod.rs:178-238` (`BenchmarkResult`)
**Spec violated:** SPEC-09 R39a (exact 29-column schema), R18b/c/d/e/f/g (mandatory fields), §4.9 line 538 ("the rightmost 7 columns MUST NOT be omitted from a v1-equivalent rodada's CSV")

**Reproduction:** Count the `writeln!` arguments in `write_csv_detail`:
- Header: 23 columns (counted: `benchmark, input_size, mode, workers, repetition, correct, wall_clock_secs, total_interactions, mips, rounds, speedup, efficiency, overhead_ratio, peak_memory_bytes, bytes_sent, bytes_received, con_con, dup_dup, era_era, con_dup, con_era, dup_era, peak_memory_during_construction`).
- SPEC-09 R39a mandates 29: the 6 missing are `peak_memory_during_reduction, agent_count_at_construction_complete, live_agent_count_watermark, representation, chunk_size, recycle_policy`.

**Impact analysis (which of the 6 are Phase F-2-blocking for the TCC?):**

For the bench rodada to produce data that is DEFENSIBLE in the TCC artigo (specifically the break-even analysis at `c_o/c_r = 2.2` per `docs/ROADMAP.md` §2.40), the harness must record:

| Column | F-2 Necessity | Reasoning |
|--------|---------------|-----------|
| `peak_memory_during_reduction` (R18b) | **NEEDED** | Already captured by `peak_memory_bytes` (legacy). MUST be exposed under the new name AND populated identically; otherwise a Phase F-2 Python tool joining on R39a's schema breaks. Cheap (rename / dual-store). |
| `agent_count_at_construction_complete` (R18c) | **NEEDED** | The "input-size invariant" against which R18a is normalised in §4.9 acceptance gates. Without this column, the Phase F-2 sparse-vs-dense comparison cannot normalise memory by agent count. |
| `live_agent_count_watermark` (R18d) | **NEEDED FOR PHASE F-2** but discipline ambiguous (SF-001) — pick end-of-reduction snapshot for now (SPEC-09 R18d permits this for sequential; grid mode is unclear). |
| `representation` (R18e) | **CRITICAL** | Without this column, sparse and dense rows are indistinguishable in the same CSV. Pollutes any downstream pandas join. |
| `chunk_size` (R18f) | **CRITICAL** | Same as R18e — eager and streaming rows are indistinguishable. |
| `recycle_policy` (R18g) | **NEEDED** but lower urgency for D-011 close-out — only mattered if the rodada mixes policies (forbidden by §3.4.6). Still required by R39a. |

**TCC-bottom-line:** All 6 are MUST under R39a, but **R18e and R18f are existential** for any Phase F-2 sparse / streaming rodada. R18b and R18c are needed for the analytical workflow. R18d and R18g are completeness-of-schema.

**Recommended fix:** As reviewer prescribed (MF-002 §recommended fix). Stage 6 must add all 6 fields and emit all 29 columns — partial fixes leave the schema inconsistent.

**SPEC violation:** SPEC-09 R39a (must-clause), R18b–g.

---

### QA-D011-007 (HIGH) — `tracing::warn!` in LIFO non-protected stalemate fallback fires per `create_agent` call, not per stalemate event (confirms MF-003)

**Severity:** HIGH — reviewer flagged as MAJOR (MF-003); QA confirms via source inspection.
**File:** `relativist-core/src/net/core.rs:387-392`
**Spec violated:** D-011 dispatch brief 2026-04-30 ("warning fires exactly once per stalemate event")

**Reproduction:** Construct an adversarial workload where every free-list entry is border-protected AND the round is delta-active (`is_in_delta_round = true` + `recycle_policy = BorderClean`). Call `create_agent` 1000 times. The warn emits 1000 times.

A practical scenario from real workloads: a `dup_era` chain reducing a heavily border-protected partition, where the deletion cascade pushes ids onto `free_list` faster than `create_agent` can pop non-protected ones. The dispatch brief explicitly requires once-per-event semantics; the implementation diverges.

**Expected behavior:** Per dispatch brief — at most one warn per stalemate event (a contiguous run of stalemate fallbacks).

**Actual behavior:** A warn per call. Counter increment is correct (`lifo_stalemate_fallbacks` is monotonic counter — fires on every event, intentional debug-only).

**Recommended fix:** Per reviewer's MF-003 §recommended fix — track `stalemate_warned: bool` on `Net` (debug-only field; reset on `Net::new()`) OR add a rate-limit threshold.

**SPEC violation:** Dispatch brief; not a SPEC clause.

---

### QA-D011-008 (MEDIUM) — `ep_annihilation_stream` does not error on `chunk_size=0`; silently coerces to `pairs_per_batch=1`

**Severity:** MEDIUM (UX bug; latent semantic ambiguity)
**File:** `relativist-core/src/bench/streaming.rs:144-157`
**Spec violated:** None directly; SPEC-21 R24 silent on zero.

**Reproduction:**

```rust
let stream = ep_annihilation_stream(100, 0); // chunk_size = 0
// pairs_per_batch = (0 / 2).max(1) = 1
// produces 100 batches of 2 agents each — same as chunk_size=1 or 2
```

**Expected behavior:** Either reject `chunk_size=0` at the CLI / `BenchArgs` layer (`#[arg(value_parser = ...)]`), OR document the coercion semantics in SPEC-21 R24.

**Actual behavior:** `chunk_size=0` is silently equivalent to `chunk_size=1` for `ep_annihilation`. Bench rows show `chunk_size=0` (post-MF-002 fix) but the actual chunk size used was 1 — provenance is corrupted.

**Root cause:** Defensive `.max(1)` at line 149 absorbs the user's bad input without telling them.

**Recommended fix:** Add a CLI-layer validation rejecting `chunk_size==0`. OR change the dispatch to error on `Some(0)` in `build_input_net_from_suite`.

---

### QA-D011-009 (MEDIUM) — `peak_memory_during_construction: u64` field on `BenchmarkResult` distinguishes "0 bytes peak" from "non-Linux unmeasured" only by sentinel value 0; sub-CSV writer uses `Option<u64>` correctly but main `detail.csv` does not (extends SF-002)

**Severity:** MEDIUM
**File:** `relativist-core/src/bench/csv.rs:50` (writes raw `u64` not `Option<u64>`)
**Spec violated:** SPEC-09 §4.9 (non-Linux convention prefers blank string over literal 0)

**Reproduction:** Run any bench on Windows / macOS:
```bash
cargo run --bin relativist -- bench --benchmark ep_annihilation --sizes 100 \
  --csv-detail /tmp/detail.csv
```
The `peak_memory_during_construction` column shows `0` on every row. Indistinguishable from a Linux capture where construction genuinely allocated 0 bytes (vacuous, but theoretically possible for empty nets).

**Recommended fix:** Either change the field to `Option<u64>` (breaks Serialize derive; deeper refactor) OR replicate `peak_for_sparse_row`'s sentinel handling in `write_csv_detail`. Apply consistently to all 7 R18 columns when MF-002 lands.

---

### QA-D011-010 (MEDIUM) — `bench::memory::get_peak_memory_bytes` and `get_peak_memory_during_construction` are byte-identical (confirms MF-004)

**Severity:** MEDIUM (DRY violation, not a runtime bug)
**File:** `relativist-core/src/bench/memory.rs:8-50`
**Reproduction:** `diff` the two function bodies — identical (both call `read_vmhwm_bytes()` on Linux, return 0 elsewhere). Only doc-comments differ.
**Recommended fix:** Per reviewer MF-004 — collapse to one function (option (a)) OR newtype phantom marker (option (b)).

---

### QA-D011-011 (MEDIUM) — `CompactSubnet::into_net` silently drops live agents whose `id >= agent_arena_len`

**Severity:** MEDIUM (defensive; corrupted-input handling)
**File:** `relativist-core/src/partition/compact.rs:108-118`
**Spec:** SPEC-19 §3.4 (wire integrity)

**Reproduction:** Construct a CompactSubnet with `agent_arena_len = 5` and a `live` entry for `(10, agent, ports)`. The `if idx < agents.len()` guard on line 110 silently drops the entry. No error, no log.

**Expected behavior:** A corrupted wire payload should be surfaced — return `Err` from the deserialize adapter (requires API change) OR log via `tracing::error!`.

**Actual behavior:** Silent data loss. The receiver gets a Net that's missing live agents the sender claimed to have. Downstream `nets_isomorphic` would catch this in tests but only if the test compares against the sender's view; production cannot.

**Recommended fix:** Add `tracing::error!` with the dropped count, OR consider this an unrecoverable wire corruption and panic (defensible in a trusted-peer model).

---

### QA-D011-012 (MEDIUM) — `chunk_size: u32` upper bound: `chunk_size = u32::MAX` is the sentinel for "disable streaming, fall back to split()" (per `config.rs:289-292`) but the bench harness at `suite.rs:181` casts `chunk_size as usize` and uses it verbatim — no sentinel handling

**Severity:** MEDIUM (potential panic in 32-bit / surprising memory behavior in 64-bit)
**File:** `relativist-core/src/bench/suite.rs:181`; `relativist-core/src/config.rs:289-292`

**Reproduction (analytic):** 32-bit target: `u32::MAX as usize` = 4294967295 ≤ usize::MAX (32-bit usize=u32::MAX); `(chunk_size / 2).max(1)` in `ep_annihilation_stream` = 2147483647 pairs per batch. Then `Vec::with_capacity(2 * pairs_per_batch)` — allocation of ~16 GiB virtual on a 32-bit target panics OOM.

64-bit target: same path, ~16 GiB Vec allocation; on most dev boxes succeeds and produces a single batch (degenerate).

The CLI doc-comment at `config.rs:289-291` says `chunk_size=u32::MAX` should "fall back to SPEC-04 split() directly (R26 short-circuit)" — but the BENCH harness has no such short-circuit; it goes straight to streaming. Inconsistency between the *daemon* CLI semantics (where the short-circuit lives) and the *bench* CLI semantics.

**Recommended fix:** Either honor the same sentinel in the bench harness (large `chunk_size` ⇒ eager) OR document that the bench's `--chunk-size u32::MAX` is the streaming-path stress test (operator-facing inconsistency).

---

### QA-D011-013 (MEDIUM) — `assert!` (NOT `debug_assert!`) on `next_id < u32::MAX` in `streaming-no-recycle` feature path will panic in production on AgentId exhaustion; the non-feature path uses different exhaustion semantics

**Severity:** MEDIUM (panic-vs-error inconsistency)
**File:** `relativist-core/src/net/core.rs:316-320`

**Reproduction:** With `--features streaming-no-recycle`, run a workload that creates ~4 billion agents under streaming/delta. The `assert!` at line 316 panics with `"AgentId space exhausted: ..."`. The non-feature path takes the free-list pop path (which also panics? — needs checking).

**Expected behavior:** Either both paths panic identically, OR both return an Err. SPEC-22 doesn't mandate one; the implementation should be consistent.

**Recommended fix:** Stage 6 to confirm parity. If both panic, downgrade to `debug_assert!` and emit a `tracing::error!` + exit cleanly in production. If returning Err, this is a wider refactor (creates Err variant on `create_agent`).

---

### QA-D011-014 (LOW) — Doc-comment for `Net::create_agent` claims O(1) amortized; TASK-0601 deeper-scan path is O(`free_list.len()`)

**Severity:** LOW (doc rot)
**File:** `relativist-core/src/net/core.rs:303` (doc-comment)

**Reproduction:** Read line 303: `/// Complexity: O(1) amortized (may trigger Vec reallocation on the fresh path).` The new TASK-0601 deeper-scan path scans the entire `free_list` (line 365 `for ... in self.free_list.iter().enumerate().rev()`) when LIFO top is border-protected and recycle_policy=BorderClean. Worst case O(`free_list.len()`).

**Recommended fix:** Update doc-comment to "O(1) amortized in the no-stalemate case; O(free_list.len()) per call under sustained Strategy B border-protection (TASK-0601)".

---

### QA-D011-015 (LOW) — Doc-comment for protocol version rejection at `coordinator.rs:187, 197` says "rejects `Register.protocol_version < PROTOCOL_VERSION`"; implementation rejects on `!=` (also future versions)

**Severity:** LOW (doc rot)
**File:** `relativist-core/src/protocol/coordinator.rs:187, 197, 449`

**Reproduction:** The doc-comment uses `<`; the runtime check uses `!=`. A v8 worker connecting to a v7 coordinator is also rejected (correct behavior; doc is wrong about the relation).

**Recommended fix:** Update doc-comment.

---

## 3. Edge Case Catalog

### Attack 1 — Wire format & PROTOCOL_VERSION (TASK-0596, TASK-0610)

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-1.1 | `free_list = vec![]` round-trip | CLEAN — UT-0596-01 covers |
| EC-1.2 | `free_list = vec![u32::MAX]` (sentinel? near-overflow) | SUSPICIOUS — `into_net` does not validate `id < arena_len` semantically; if `u32::MAX < arena_len` (only possible on absurd nets), accepted; otherwise the `if idx < agents.len()` silently drops on the live side but `free_list` is preserved verbatim. Add adversarial UT. |
| EC-1.3 | `free_list` with duplicates `vec![1, 1]` | DANGEROUS (QA-D011-002) |
| EC-1.4 | `free_list` overlaps live agent ids | DANGEROUS (QA-D011-002) — release-build silent corruption |
| EC-1.5 | `free_list` id ≥ `next_id` | NOT VALIDATED — should be rejected |
| EC-1.6 | rkyv 4-byte misaligned buffer | OUT OF SCOPE for this audit (rkyv handles alignment via its own framing) |
| EC-1.7 | v6 client → v7 coordinator | CLEAN — `coordinator.rs:449` rejects with NACK |
| EC-1.8 | v8 (future) → v7 coordinator | CLEAN per impl, doc-rot per QA-D011-015 |

### Attack 2 — Bench harness path selection (TASK-0604)

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-2.1 | `chunk_size=Some(0)` | SUSPICIOUS (QA-D011-008) — silent coercion |
| EC-2.2 | `chunk_size=Some(1)` | MARGINAL — works, every pair its own batch |
| EC-2.3 | `chunk_size=Some(N) > 2*size` | CLEAN — single batch produced; degenerate but correct |
| EC-2.4 | `chunk_size=Some(_)` AND `representation=Sparse` | **CRITICAL BUG** (QA-D011-001) |
| EC-2.5 | `representation=Sparse` for non-`dual_tree` (no chunk) | CLEAN — `make_sparse_net` Err path fires (suite.rs:158-165) |
| EC-2.6 | `representation=Sparse` for non-`dual_tree` (with chunk) | **CRITICAL BUG** (QA-D011-001) — guard bypassed |
| EC-2.7 | `max_pending_lifetime=0` | NOT TESTED in the bench path; for `ep_annihilation_stream` no Pending directives are emitted (purely Resolved), so the lifetime check at `suite.rs:251-263` is vacuous. For `dual_tree_stream` with forward-references and `max_lifetime=0`, every Pending directive expires on the same chunk it was recorded — the harness errors immediately. Should add a UT. |
| EC-2.8 | `max_pending_lifetime=u32::MAX` | CLEAN — `if max_lifetime != u32::MAX` short-circuits at line 251 (sentinel handling explicit) |
| EC-2.9 | `chunk_size=u32::MAX` | SUSPICIOUS (QA-D011-012) |

### Attack 3 — Memory probe (TASK-0605)

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-3.1 | `/proc/self/status` unreadable (sandbox) | CLEAN — `read_vmhwm_bytes` returns 0 silently (the file_op already returns Err and the function returns 0 — line 54) |
| EC-3.2 | VmHWM line missing (kernel variant) | CLEAN — function returns 0 (correct fall-back) |
| EC-3.3 | Non-Linux returns 0 | CLEAN as design intent; PROBLEMATIC at CSV writer (QA-D011-009) |
| EC-3.4 | Concurrent allocation between probe and reduce_all | NOT MITIGATED — but VmHWM is monotone non-decreasing so the value is a valid lower bound; documented in `memory.rs:35` |

### Attack 4 — Sparse path (TASK-0606, 0607)

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-4.1 | `dual_tree(depth=0)` | NOT TESTED — IT-0606-04 starts at depth 5; behavior undefined for depth=0 (single root agent? empty?). Add UT. |
| EC-4.2 | `dual_tree(depth=20)` | CACHED OUT — depth 14 is the practical ceiling (~16k agents, ~3 min in debug per IT-0606-04 doc-comment) |
| EC-4.3 | `--csv-sparse` AND `--csv-detail` set together | NOT TESTED for sparse-row leakage into detail.csv — Reviewer Deviation 5 asks QA to verify; QA observes no `representation=Dense` row from the auto-pair leaks into `all_results` (line 686 builds dense net but doesn't push a row). CLEAN. |
| EC-4.4 | Concurrent `--csv-sparse <samepath>` | NOT TESTED — file-system race; out of scope for v2 |
| EC-4.5 | `--csv-sparse /readonly/foo.csv` | NOT TESTED — likely a `?` propagation through `io::Result` produces a clean error |
| EC-4.6 | Auto-pair `dense_cfg` does not clear `chunk_size` | **HIGH BUG** (QA-D011-003) |

### Attack 5 — TCP smoke (TASK-0610)

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-5.1 | `ep_annihilation_produces_non_empty_free_list_after_reduction` under `streaming-no-recycle` | CLEAN — the feature only bypasses recycling when `is_in_delta_round || streaming_active`; sequential `reduce_all` has neither flag set. Witness IS robust. |
| EC-5.2 | `--mode tcp --workers 0` | OUT OF SCOPE — `workers` is `u32` but `0` workers is a degenerate config; no validation observed. Add UT. |
| EC-5.3 | Worker disconnect mid-CompactSubnet transfer | OUT OF SCOPE for this bundle (existing v1 protocol behavior, not changed by D-011) |
| EC-5.4 | docker-compose default `CHUNK_SIZE=10000` | **HIGH BUG** (QA-D011-004) |

### Attack 6 — Phase B medium/low fixes (TASK-0598, 0599, 0600, 0601)

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-6.1 | TASK-0598: counter fields always-present, struct size growth | CLEAN — net struct grows by ~56-64 bytes; not a regression at any reasonable scale |
| EC-6.2 | TASK-0598 + rkyv archived form | CLEAN — `#[cfg_attr(feature="zero-copy", rkyv(with=rkyv::with::Skip))]` correctly elides counters from archived form (test floor 1800 zero-copy proves) |
| EC-6.3 | TASK-0600 `From<PullCoordinatorState>` half-initialized | NOT TESTED — projection is total; UT-0600-02 covers exhaustiveness. Stage 6 may add an adversarial UT but unlikely to surface a real bug. |
| EC-6.4 | TASK-0601 worst-case O(N) under sustained stalemate | DOC ROT (QA-D011-014); not a runtime bug |
| EC-6.5 | TASK-0601 warn fires per-call | **HIGH BUG** (QA-D011-007) |
| EC-6.6 | TASK-0599 named worker assertions | CLEAN — search for `worker_a` / `worker_b` in tests returns 0 hits (verified via grep). |

### Attack 7 — Cross-task interactions

| EC | Scenario | Verdict |
|---|----------|---------|
| EC-7.1 | TASK-0596 + TASK-0604: chunked CompactSubnet preserves free_list | NOT EXERCISED — the chunked path in `build_input_net_from_suite` does not produce CompactSubnet payloads at all (it builds a Net via SparseNet+to_dense, then hands to `run_grid` which partitions internally). The wire-form free_list test is on the run_grid output, which IS exercised by IT-0610-05. CLEAN. |
| EC-7.2 | TASK-0606 + TASK-0610: docker bench-tcp `--representation sparse --benchmark dual_tree` | NOT TESTED — would currently work for `dual_tree` but invokes QA-D011-001 if `--chunk-size` is also set (which is the docker default per QA-D011-004) |
| EC-7.3 | TASK-0598 + zero-copy ABI parity | CLEAN — test floor 1800 zero-copy is held |

---

## 4. Reviewer findings cross-reference

| Reviewer ID | Reviewer severity | QA verdict |
|-------------|-------------------|------------|
| **MF-001** (R37c not enforced at runtime) | MAJOR | **CONFIRMED + EXTENDED** (QA-D011-005). The reviewer noted PT-0604-10's 12 random cases at 50–400 size; QA additionally observes the production bench rodada at default depth 14 has zero R37c protection AT ANY scale. **Severity confirmed at HIGH** for production rodadas. |
| **MF-002** (R39a schema incomplete; 6 of 7 columns missing) | MAJOR | **CONFIRMED + EXTENDED** (QA-D011-006). QA additionally analyses which columns are F-2-blocking for the TCC: R18e (representation) and R18f (chunk_size) are existential — without them, sparse and streaming rows cannot be distinguished from eager rows in the same CSV. R18b/R18c are needed for the sparse-vs-dense memory normalisation. R18d/R18g are completeness. **All 6 are MUST per R39a; 4 are F-2-blocking for TCC analysis.** |
| **MF-003** (warn rate not limited) | MAJOR | **CONFIRMED** (QA-D011-007) by source inspection. Per-call fire confirmed at `core.rs:387-392`. |
| **MF-004** (memory.rs DRY) | MAJOR | **CONFIRMED** (QA-D011-010) by source inspection — bytes identical. |
| SF-001 (R18d sampling discipline undefined for grid) | MINOR | CONFIRMED — out of scope until MF-002 lands |
| SF-002 (peak_memory blank-rendering) | MINOR | CONFIRMED + EXTENDED (QA-D011-009) |
| SF-003 (function decomposition) | MINOR | NOT FURTHER ATTACKED — code-smell, not a bug |
| SF-004 (god struct trend) | MINOR | NOT FURTHER ATTACKED |
| SF-005 (peak_for_sparse_row co-location) | MINOR | NOT FURTHER ATTACKED |
| SF-006 (println!) | MINOR | NOT FURTHER ATTACKED — pre-existing |
| SF-007 (80% memory gate looseness) | MINOR | NOT FURTHER ATTACKED — explicitly informative gate per IT-0606-04 doc |
| SF-008 (cfg matrix discoverability) | MINOR | CONFIRMED — pattern is correct, no fix |

**New findings beyond reviewer:** QA-D011-001 (CRITICAL — sparse+chunk dispatch), QA-D011-002 (CRITICAL — wire integrity), QA-D011-003 (HIGH — auto-pair clone preserves chunk_size), QA-D011-004 (HIGH — docker-bench-smoke streaming-default), QA-D011-008 (MEDIUM — chunk_size=0 silent coercion), QA-D011-011 (MEDIUM — silent live-agent drop), QA-D011-012 (MEDIUM — chunk_size sentinel mismatch with daemon CLI), QA-D011-013 (MEDIUM — assert vs debug_assert exhaustion semantics), QA-D011-014 (LOW — doc rot O(1)), QA-D011-015 (LOW — doc rot < vs !=).

---

## 5. Recommended Priority for Stage 6 Refactor

### MUST-FIX before D-011 close-out (blocking):

1. **QA-D011-001 (CRITICAL):** Fix the sparse+chunk dispatch. Either reject the combination explicitly OR enforce `make_sparse_net` Err propagation regardless of `chunk_size`. **Highest priority — produces wrong scientific data otherwise.**
2. **QA-D011-005 / MF-001 (HIGH):** Add R37c gate in `run_benchmark_suite`. Spec-mandated for every Tier 3 datapoint.
3. **QA-D011-006 / MF-002 (HIGH):** Land the 6 missing R18 columns on `BenchmarkResult` and `write_csv_detail`. R18e/R18f minimum (existential); R18b/R18c next (analytical); R18d/R18g for completeness.
4. **QA-D011-003 (HIGH):** Clear `chunk_size` (and confirm `representation=Dense`) in the `dense_cfg` clone for auto-pair.
5. **QA-D011-004 (HIGH):** Update `docker-compose.yml` and `docker-bench-smoke.yml` to exercise BOTH eager and streaming paths in CI.
6. **QA-D011-007 / MF-003 (HIGH):** Rate-limit the LIFO stalemate `tracing::warn!`.
7. **QA-D011-002 (CRITICAL):** Add wire-integrity post-condition validation in `CompactSubnet::into_net`. Defensible to land alongside MF fixes.
8. **QA-D011-010 / MF-004 (MEDIUM):** Collapse the memory.rs duplication.

### SHOULD-FIX in same PR (cheap, related):

9. **QA-D011-008 (MEDIUM):** Reject `chunk_size=0` at the CLI layer.
10. **QA-D011-009 (MEDIUM):** Sentinel-handle `peak_memory_during_construction` in `write_csv_detail`.
11. **QA-D011-011 (MEDIUM):** Surface dropped-agent count in `CompactSubnet::into_net`.

### DEFER to D-012 / follow-up:

12. **QA-D011-012 (MEDIUM):** Reconcile `chunk_size=u32::MAX` semantics between daemon CLI and bench CLI.
13. **QA-D011-013 (MEDIUM):** Confirm exhaustion-handling parity between feature/non-feature `create_agent` paths.
14. **QA-D011-014 (LOW):** Update `create_agent` complexity doc-comment.
15. **QA-D011-015 (LOW):** Update protocol version rejection doc-comment.

### Architectural follow-ups (not blocking D-011):

- Open a SPEC-09 §4.9 amendment to clarify TASK-0604's SparseNet-internal-assembly choice (per Reviewer Deviation 2). The "representation=Dense + chunk_size=Some(N) ⇒ internally sparse" semantic is undocumented.
- Open a SPEC-09 R18d clarification (per SF-001) — sequential vs grid sampling discipline.
- Open a SPEC-19 R35a clarification on wire-integrity validation (per QA-D011-002) — does `into_net` MUST validate, or is that the upstream's responsibility?

---

## 6. Stress Scenarios (informational; not blocking)

### SS-001 — Stalemate storm (production simulation)
Sustained `is_in_delta_round = true` workload with full-border-protected free_list at depth 1000. Current behavior: 1000 warn lines (QA-D011-007); deeper-scan O(N) per call (QA-D011-014). Recommend Stage 6 add a stress test capping at 100 events to validate the rate-limiter.

### SS-002 — Wire-tampering simulation
Adversarial peer crafts a CompactSubnet with overlapping live + free_list ids (QA-D011-002). Currently: panic in debug, silent corruption in release. Recommend Stage 6 add an adversarial test in `tests/spec19_wire_integrity.rs` covering the 4 corruption modes from Attack 1.

### SS-003 — Sparse + Chunk + Dense rodada in same CSV (post-MF-002)
Once R18e and R18f land, an operator could emit a mixed CSV where sparse and dense rows coexist. Per SPEC-09 §3.4.7 this is forbidden. Recommend adding a runtime guard in `write_csv_detail` (or its caller) that errors when the result set contains heterogeneous representations.

### SS-004 — Docker smoke regression detection
Today the smoke gate would NOT catch a regression in the eager-path detail.csv emission (QA-D011-004). Once fixed, the smoke must run BOTH paths AND assert the row count is 2 (one per path).

---

## 7. Verification Plan for Stage 6

After Stage 6 implements the fixes, the following gates must pass before close-out:

1. `cargo test --workspace` ≥ 1756 default tests (current floor); adversarial UTs from this report add ≥ 6 tests.
2. `cargo test --features zero-copy` ≥ 1800 tests.
3. `cargo test --features streaming-no-recycle` ≥ 1749 tests.
4. `cargo clippy --workspace -- -D warnings` clean.
5. `docker compose --profile bench-tcp run --rm bench-tcp` (without CHUNK_SIZE) AND (with CHUNK_SIZE=100) BOTH succeed and produce CSV rows distinguishable by the new `chunk_size` column.
6. R37c gate: an adversarial benchmark with a deliberately-buggy `make_net_stream` MUST fail under `run_benchmark_suite` (post-fix).
7. Wire-integrity: a tampered CompactSubnet (free_list with duplicate or live-overlap) MUST fail at deserialization (post-fix QA-D011-002).
8. Sparse + chunk_size combo: `bench --representation sparse --chunk-size 100 --benchmark ep_annihilation` MUST error explicitly (post-fix QA-D011-001).

If all 8 pass, D-011 verdict moves from ACCEPT_WITH_FIXES to CLOSE_OUT-ready.
