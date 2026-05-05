# Handoff Brief — D-011 Phase F-1: SPEC-09 Tier 3 Measurement Protocol Amendment

**Date:** 2026-04-30
**Target agent:** ESPECIALISTA EM SPECS (camada 1, root do TCC)
**Bundle:** D-011 (Tier 3 hardening + bench enablement)
**Plan:** `codigo/relativist/docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md` Phase F-1
**Independent of:** Phase A (SPEC-19 R35a) — F-1 may run in parallel.

---

## Why this amendment exists (1-paragraph context)

The user wants to re-run the existing 13-benchmark suite (`ep_annihilation`, `dual_tree`, `cascade_cross`, etc.) with Tier 3 optimizations active (SPEC-21 streaming generation + chunked partitioning + SPEC-22 arena recycling + SparseNet) and compare aggregated results against `results/locked/v1_local_baseline/`. The current SPEC-09 (`specs/SPEC-09-benchmarks.md`) does not specify how Tier 3 paths are exercised, measured, or reported. Specifically: (a) `BenchmarkResult.peak_memory_bytes` (line 412) is process-wide peak RSS measured AFTER reduction completes, which captures arena-during-reduction memory but NOT construction-phase memory — the relevant variable for SPEC-21's M5 acceptance gate ("ep_con 100M on 2GB coordinator"); (b) §3.4 Experimental Variables (line 506) does not list `chunk_size`, `recycle_policy`, or `representation` as variables; (c) §3.6 Correctness (line 579) does not state how isomorphism check changes when the net is built via `AgentBatch` stream rather than `make_net()` eager construction; (d) §4 Design (line 682) has no streaming/chunked architecture section. Without these amendments, the bench harness cannot be wired in Phase C without ambiguity, and the results CSVs will lack provenance for the TCC's Resultados section.

## What you MUST do

Author three additions to `specs/SPEC-09-benchmarks.md`:

1. **§3.3.X "Streaming Representation Metrics"** — new subsection inside §3.3 Mandatory Metrics (after R18's `BenchmarkResult` struct, ~line 415).
2. **§3.6 amendment clause** — extend R36/R37 with a new "Construction-phase isomorphism" paragraph that handles streaming-built nets.
3. **§4.9 "Streaming Architecture"** — new subsection inside §4 Design (after §4.8 Visualizations, ~line 1059).

Plus one minor edit:

4. **§3.4 Experimental Variables** — extend with §3.4.5, §3.4.6, §3.4.7 listing the new bench knobs (`chunk_size`, `recycle_policy`, `representation`).

## §3.3.X — exact authoring guidance

Add new requirements **R18a, R18b, R18c, R18d** to `BenchmarkResult`:

- **R18a — `peak_memory_during_construction: u64`** (bytes). Captured AFTER `make_net(size)` (or after the streaming pipeline finishes producing all `AgentBatch`es and the worker arenas are populated) and BEFORE any reduction call. On Linux: read `/proc/self/status` `VmHWM` at this exact program point. On non-Linux: 0. Rationale: this is the variable that validates SPEC-21 R10/R12 ("memory scales with `chunk_size`, not `total_agents`") and SPEC-22 R32 (free-list memory budget at M5).

- **R18b — `peak_memory_during_reduction: u64`** (bytes). Captured AFTER the final `reduce_all` / `run_grid` call, before merge cleanup. The DIFFERENCE between R18b and R18a is the working-set delta; the CURRENT SPEC-09 R18's `peak_memory_bytes` is renamed/redefined as `max(R18a, R18b)` and kept for backward CSV compat.

- **R18c — `agent_count_at_construction_complete: u64`** and **R18d — `live_agent_count_watermark: u64`**. R18c is the agent count immediately after construction (eager: `net.live_agents()`; streaming: sum of `AgentBatch::agents.len()` across batches). R18d is the maximum live-agent count observed during reduction (sampled at each round boundary; `net.agents.iter().filter(|s| s.is_some()).count()`). For the SparseNet path, R18d is sampled on the dense net (post `to_dense`), and R18c is the SparseNet's `agents.len()` (a `HashMap::len`, not the dense arena length). This closes the "sparse_net_agent_count_watermark" requirement.

- **R18e — `representation: NetRepresentation`** and **R18f — `chunk_size: Option<u32>`** and **R18g — `recycle_policy: RecyclePolicy`**. These three fields MUST be added to `BenchmarkResult` so every CSV row carries provenance of the path taken. They mirror the new `BenchmarkSuiteConfig` fields specified in the D-011 Phase C plan (`relativist-core/src/bench/mod.rs:231-251`).

R18a–R18g all serialize into the existing detail CSV (`csv_detail_path`, R39) as new columns appended to the right of the v1 column set. The summary and rounds CSVs are unchanged. **The amendment MUST explicitly state that the column order before R18e is preserved, so `v1_local_baseline` CSVs remain joinable to the new CSVs by `(benchmark, size, workers, mode, repetition)` keys.**

## §3.6 amendment clause — Construction-phase isomorphism

Insert a new requirement **R37c** between R37b (line 587) and R38 (line 589):

> **R37c.** When `representation == Sparse` OR `chunk_size.is_some()`, the verifier MUST additionally assert that the net OBSERVED at the end of construction (before `reduce_all`/`run_grid`) is graph-isomorphic to the eager-constructed reference net produced by `Benchmark::make_net(size)`. The check uses `nets_isomorphic` (SPEC-08) with the lightweight fast-path of R37a permitted on nets >5000 agents. Rationale: streaming-built nets MAY have different internal `AgentId` assignments or arena layouts than their eager equivalents (SPEC-21 R6, R10) but MUST be reduction-equivalent — without this gate, a streaming bug that produces a structurally-different but reducible net would silently pass R37 (which only checks the post-reduction normal form). For sparse representation, the check is performed AFTER `to_dense(id_range)` but BEFORE `reduce_all`. **(MUST when Tier 3 paths active)**

## §4.9 — Streaming Architecture

Add a new subsection (~line 1059, after §4.8 Visualizations and before §5 Rationale):

```
### 4.9 Streaming Architecture

(Reference: SPEC-21 §3.5 chunked pipeline, SPEC-22 §3.6 free-list lifecycle.)

The bench harness MUST select between two execution paths based on
BenchmarkSuiteConfig.chunk_size:

- chunk_size == None  -> EAGER path: net is materialized in full via
  Benchmark::make_net(size), then run_grid is invoked. Memory peak occurs
  at construction. This path is the v1-equivalent baseline.

- chunk_size == Some(N) -> STREAMING path: net is generated as a sequence
  of AgentBatch chunks of N agents each, partitioned incrementally per
  batch via merge::generate_and_partition_chunked_with_chunk_size_and_lifetime,
  and dispatched per-chunk to workers. Memory peak occurs at the largest
  chunk + retained borders (per SPEC-21 R10).

The recycle_policy field selects the worker-side arena recycling
discipline (SPEC-22 R10b/R10c):

- DisableUnderDelta  -> default; recycling is suppressed when a partition
  is in delta_mode AND the reused id is in the border-referenced set.
- BorderClean        -> recycling proceeds whenever the slot is not on
  the active border (R10c protected tombstone).

The representation field selects the construction-phase data structure:

- Dense  -> standard Net (Vec<Option<Agent>>), v1-equivalent.
- Sparse -> SparseNet (HashMap-backed), converted via to_dense(id_range)
  before reduction. Tier 3 micro-bench scope only (Phase D).

Pending-reference budget across batches MUST be bounded by
max_pending_lifetime (SPEC-21 R37g). Default value is 16 batches.
Exceeding this budget MUST cause the streaming partitioner to return
the error PartitionError::PendingLifetimeExceeded (Phase B-2 fix scope).

Acceptance gates per path:
- EAGER:    no regression vs v1_local_baseline on (wall_clock, MIPS, peak_memory).
- STREAMING: peak_memory_during_construction is bounded by O(chunk_size +
             |retained_borders|), independent of total_agents. Validated
             empirically by observing that doubling input_size at fixed
             chunk_size leaves R18a within 2x.
- SPARSE micro-bench (Phase D): R18a in dual_tree(50000) under sparse
            representation MUST be < 80% of dense R18a at the same size.
```

## §3.4 — new Experimental Variables

Append three subsections to §3.4 (after §3.4.4 Partitioning Strategy, ~line 537):

- **§3.4.5 Chunk size.** Variable `chunk_size: Option<u32>`. Default for v2 bench rodada: `Some(10000)`. Eager baseline: `None`. The chunk size is held constant within a single bench rodada; experiments comparing chunk sizes are out of scope for D-011 (deferred to a future memory-scaling study).

- **§3.4.6 Recycle policy.** Variable `recycle_policy ∈ {DisableUnderDelta, BorderClean}`. Default: `DisableUnderDelta` (matches SPEC-22 R10b conservative choice). The MAY-permit value `BorderClean` is reserved for D-011 Phase C smoke testing; the production rodada uses the default.

- **§3.4.7 Representation.** Variable `representation ∈ {Dense, Sparse}`. Default: `Dense`. `Sparse` is exercised ONLY in the SparseNet micro-bench (D-011 Phase D), limited to `dual_tree` benchmark, not part of the main matrix.

## Cross-reference propagation

After authoring the four edits above, propagate to:

- **`docs/DATA-COLLECTION-PLAN.md`** (if present; otherwise inform user it should be created in Phase F-2 closure) — add a "Tier 3 Measurement Protocol" section pointing to the SPEC-09 amendments.
- **`specs/SPEC-21-streaming-generation.md`** §3.5 — append a one-line cross-reference: "Bench-harness measurement protocol: see SPEC-09 §4.9."
- **`specs/SPEC-22-arena-management.md`** §3.6 — append: "Bench-harness measurement protocol: see SPEC-09 §3.3.X (R18c–R18g)."

## Stages

Spec-only amendment. Same pipeline as Phase A:

1. **Round 1 (you):** author the 4 edits above to SPEC-09 + 2 cross-references.
2. **spec-critic Round 1:** adversarial — does R37c eliminate the streaming-bug-masking risk? Are R18a–R18g sufficient to validate M5 acceptance gate? Are the new CSV columns backward-compatible with `v1_local_baseline` join keys? Output: `codigo/relativist/docs/spec-reviews/SPEC-09-tier3-round1-2026-04-30.md`.
3. **Round 2 (you, defender):** address findings + closure log.
4. **Commit:** `spec(d-011): amend SPEC-09 — Tier 3 measurement protocol (R18a–R18g, R37c, §4.9)` on branch `v2-development`.

**Gate:** spec-critic Round 2 closes. After this commit, Phase C (developer wires bench harness) AND Phase D (developer wires SparseNet micro-bench) are unblocked.

## Files in scope (this dispatch)

- `codigo/relativist/specs/SPEC-09-benchmarks.md` — 4 edits described above
- `codigo/relativist/specs/SPEC-21-streaming-generation.md` — 1-line cross-ref in §3.5
- `codigo/relativist/specs/SPEC-22-arena-management.md` — 1-line cross-ref in §3.6
- `codigo/relativist/docs/spec-reviews/SPEC-09-tier3-closure-2026-04-30.md` — closure log

## Files explicitly OUT of scope

- Any code under `relativist-core/src/`
- `relativist-core/src/bench/mod.rs` (`BenchmarkResult` struct extension is Phase C developer scope)
- `relativist-core/src/bench/memory.rs` (`get_peak_memory_at_construction_complete` is Phase C developer scope)
- `docker-compose.yml` / `Dockerfile` (Phase E)

## Reference reading

1. `codigo/relativist/specs/SPEC-09-benchmarks.md` lines 1-100 (purpose + definitions), 366-505 (§3.3 Metrics), 506-578 (§3.4 Variables), 579-590 (§3.6 Correctness), 940-998 (§4.4 Memory + §4.5 Timing), 1059-1073 (§4.8 Visualizations — insertion point for §4.9)
2. `codigo/relativist/specs/SPEC-21-streaming-generation.md` §3.5 (chunked pipeline), §3.8 A6 (R10b/R10c trigger broadening)
3. `codigo/relativist/specs/SPEC-22-arena-management.md` §3.6 (free-list lifecycle), §3.8 (Amendments / SC closures), R32 (free-list memory budget)
4. `codigo/relativist/docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md` Phases C and D (so you understand what the developer will need from your spec)
5. `codigo/relativist/results/locked/v1_local_baseline/` — observe the existing CSV column order (so R18e–R18g can be appended without breaking joins)

## Output expected back to user

1. Edits applied to the 3 spec files
2. Closure log at `docs/spec-reviews/SPEC-09-tier3-closure-2026-04-30.md`
3. Single commit on `v2-development`: `spec(d-011): amend SPEC-09 — Tier 3 measurement protocol (R18a–R18g, R37c, §4.9)`
4. Brief status report (under 150 words) summarizing: what R18a–R18g + R37c + §4.9 say, what spec-critic Round 1 raised, how Round 2 addressed it, what Phases C and D (developers) need to implement next.

---

**Done. User: copy this entire file content (or the relevant subset) as the prompt to ESPECIALISTA EM SPECS in the TCC root session. May be dispatched in parallel with the SPEC-19 R35a brief.**
