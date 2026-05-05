# BRIEF: SPEC-21 Streaming Generation — Coherence Brief for spec-critic Round 1

**Generated:** 2026-04-25
**Scope:** Wave 2 second half / v2 Pre-DEV Spec Pipeline — SPEC-21 coherence brief for adversarial Round 1 review
**Downstream consumer:** `spec-critic` (adversarial review agent)

---

## Executive Summary

SPEC-21 specifies a chunked, streaming generation pipeline for Relativist: instead of the coordinator generating a full net in memory before partitioning, it produces agents in bounded batches (`AgentBatch`), partitions each batch via a `StreamingPartitionStrategy`, and accumulates per-worker `PartitionAccumulator` objects incrementally. Three tightly coupled ROADMAP items (2.28 streaming partitioning, 2.27 streaming generation, 2.30 chunked pipeline) are bundled in §§3.1-3.3. §3.6 further extends the spec with a pull-based (lazy) dispatch mode (ROADMAP 2.36), adding two new `Message` variants (`RequestWork`, `NoMoreWork`) to SPEC-06's message catalog.

The spec is well-structured and internally coherent on correctness logic, but it carries **five structural gaps that are hard blocks for the current pipeline stage:**

1. **No `§3.8 Amendments` block.** SPEC-21 amends SPEC-06 (two new Message variants, R31), amends SPEC-07/GridConfig (new fields R24/R25/R34), and amends SPEC-13 (new coordinator FSM state implied by pull model, R30-R32). None of these amendments appear in a formal §3.8 block with old-text/new-text/rationale structure, breaking the v2 pipeline amendment pattern established by SPEC-19, SPEC-20, and SPEC-22.

2. **DISC-009 v2 is not cited anywhere in the spec.** DISC-009 v2 is the primary theoretical source for SPEC-21 per the theory bridge (it defines the generation-protocol streaming level and the five operating modes). Its absence from `Discussions consumed:` frontmatter is a citation gap.

3. **No AC-NNN citations** despite the spec making design claims grounded in AC-007 (HVM2 streaming-style reduction), AC-010 (HVM4 goto-state-machine), and AC-014 (bench methodology). The FENNEL strategy cites Tsourakakis et al. 2014 and Stanton & Kliot 2012 but neither is catalogued as a REF-NNN in the theory bridge.

4. **PROTOCOL_VERSION coordination is unresolved.** R31 adds `RequestWork`/`NoMoreWork` to the `Message` enum (SPEC-06 amendment). This is a wire-format change. The spec does not say whether it requires a PROTOCOL_VERSION bump, and if so, whether this is bump 3→4 (SPEC-21 landing after SPEC-22) or 2→3 (SPEC-21 landing before SPEC-22). This is the same open sequencing question already flagged in SPEC-22's TASK-0476.

5. **I3' interaction not acknowledged.** SPEC-22 (just closed) amended SPEC-01 I3 to I3' (monotonicity relaxed to uniqueness). SPEC-21's R15 requires monotonically increasing AgentIds across batches — this is a stricter constraint than I3'. The spec does not acknowledge the relationship between R15 (generator-level monotonicity) and I3' (arena-level uniqueness). Spec-critic should probe whether R15 is compatible with arena recycling during a streamed pipeline run.

---

## Relevant Context

### 1. Spec Scope Summary

SPEC-21 specifies how Relativist generates and partitions interaction combinator nets incrementally, bounding coordinator peak memory to `O(chunk_size + border_tracking_state)` instead of `O(total_agents)`.

Three features form a single pipeline (§1):
- **2.28 — StreamingPartitionStrategy trait** (§3.1): A per-batch allocation trait. MVP: `RoundRobinStreamingStrategy`. Advanced: `FennelStreamingStrategy` (SHOULD).
- **2.27 — Streaming Net Generation** (§3.2): `Benchmark` trait gains `make_net_stream(&self, size, chunk_size) -> Box<dyn Iterator<Item = AgentBatch>>`. `ep_annihilation` is the MUST generator; others SHOULD.
- **2.30 — Chunked Generation Pipeline** (§3.3): `generate_and_partition_chunked(stream, num_workers, strategy) -> ChunkedPartitionResult`. One-pass processing with pending-connection forward references. Output is structurally identical to SPEC-04's `PartitionPlan`.

§3.6 is a self-declared amendment to SPEC-21 (not a separate spec): pull-based / lazy dispatch. Workers send `RequestWork`; coordinator responds with chunks or `NoMoreWork`. `DispatchMode` enum (`Push` / `Pull` / `Auto`). Default: `Auto`.

In the DISC-009 v2 taxonomy (`docs/theory-bridge.md` L124-127), this spec covers **streaming level 2 (generation-protocol)**, the third of three streaming levels. Level 1 (rule-level, per-fired-rule streaming) is covered by REF-015 / SPEC-14 §8. Level 3 (recipe-based) is SPEC-25 (Wave 3). SPEC-21's framing matches DISC-009's "streaming generation" operating mode.

`(source: specs/SPEC-21-streaming-generation.md, §1; docs/theory-bridge.md, L124-127)`

---

### 2. Predecessor / Amendment Graph

**Frontmatter `Depends on`:**

| Spec | Relation | Key dependency in SPEC-21 |
|------|----------|--------------------------|
| SPEC-01 | Depends on | Invariants T1, I3, D1, C1-C3; R27 maps to these explicitly |
| SPEC-02 | Depends on | `Net`, `AgentId`, `Symbol`, `PortRef`, port array layout; §4.9 PartitionAccumulator wraps `Net` |
| SPEC-04 | Depends on | `PartitionStrategy` R21, `split()`, ID ranges R16-R18, C1-C3 assertions §4.8; R29 re-uses SPEC-04 formula verbatim |
| SPEC-05 | Depends on | `merge()` consumes `ChunkedPartitionResult` (R21 compatibility assertion); `run_grid` integration |
| SPEC-13 | Depends on | Module placement (`src/partition/`), coordinator FSM, dependency-direction rules (Core layer purity, R9/R16) |

**Amendments discovered through requirement-level analysis (NOT in frontmatter):**

| Target Spec | Amendment Type | Triggered by | Severity |
|-------------|---------------|--------------|----------|
| SPEC-06 (Wire Protocol) | Adds `RequestWork { worker_id }` and `NoMoreWork` variants to `Message` enum | R31 | HARD — wire-format change, PROTOCOL_VERSION implication |
| SPEC-07 (CLI/GridConfig) | Adds `chunk_size: usize`, `streaming_strategy: StreamingStrategyConfig`, `dispatch_mode: DispatchMode` to `GridConfig` | R24, R25, R34 | Medium — config struct change |
| SPEC-13 (System Architecture) | Implies new coordinator FSM state(s) for pull dispatch loop (coordinator waits for `RequestWork`, generates chunk on demand) | R30-R32 | Medium — FSM state table must be extended |
| SPEC-03 / Benchmark trait (src/bench/mod.rs) | Adds `make_net_stream()` method to `Benchmark` trait; modifies trait contract | R10 | Medium — trait API change affects all 13 benchmarks |
| SPEC-01 I3 / I3' (SPEC-22 amendment) | R15 requires batch-level monotonicity (max ID in batch k < min ID in batch k+1); needs to be reconciled with I3' from SPEC-22 | R15 | Medium — silent tension |

**Forward references:**
- **SPEC-25 (Recipe Generation, Wave 3):** SPEC-25 §1 cites SPEC-21 explicitly as the complementary approach ("SPEC-21 reduces coordinator memory but does not eliminate it"). SPEC-21 makes no mention of SPEC-25 in return — this is a deliberate one-directional relationship (SPEC-21 is the lower tier, SPEC-25 builds on it).
- **SPEC-22 (Arena Management, just closed):** SPEC-22 has no `Streaming` mention and SPEC-21 has no `SparseNet`/`free_list` mention. The interaction is non-obvious but real (see Section 5 below).

`(source: specs/SPEC-21-streaming-generation.md, frontmatter + §§3.1-3.6; specs/SPEC-22-arena-management.md, frontmatter; specs/SPEC-25-recipe-generation.md, §1)`

---

### 3. Theory Bridge Audit

The theory bridge was last updated 2026-04-24 (`docs/theory-bridge.md` L7).

**IDs cited in SPEC-21 frontmatter:**

| ID | Present in Bridge? | Bridge Summary |
|----|--------------------|---------------|
| REF-001 (Lafont 1990) | YES (L159) | Locality principle, p.96; cited for "reduction depends on immediate neighborhoods" (§5.2) |
| REF-002 (Lafont 1997) | YES (L160) | Net structure, strong confluence; cited in §5.1 rationale |
| REF-005 (Mackie & Pinto 2002) | YES (L161) | Cut elimination, multiplexing nets; cited in frontmatter but role in SPEC-21 is thin (no body reference found) |
| REF-015 (Mackie & Sato 2015) | YES (L170) | "Parallel Evaluation of INets: Case Studies — rule-level streaming reference." Bridge says: Cited by DISC-009 v2 (streaming level 1), SPEC-14 §8. But the bridge also tags REF-015 as a rule-level streaming reference (streaming level 1), whereas SPEC-21 is generation-protocol streaming (streaming level 3 in DISC-009). SPEC-21's use of REF-015 "batch vs streaming" is plausible but the mismatch in streaming level should be flagged by spec-critic. |
| ARG-001 (P1-P6) | YES (L18) | Central argument; cited correctly in §5.1 and §5.2 rationale |
| ARG-002 (C1-C3) | YES (L27) | Partitioning structure; cited correctly in §5.1 |

**IDs cited in SPEC-21 body (not in frontmatter):**

| ID | Present in Bridge? | Note |
|----|--------------------|------|
| DISC-004 v2 | YES (L96) | Cited in §4.2 trait doc comment ("DISC-004 v2, Section 1.6") and §5.1. NOT in `Discussions consumed:` frontmatter — missing citation. |
| DISC-009 v2 | YES (L124) | NOT cited in SPEC-21 anywhere — the primary source for SPEC-21's level-3 streaming taxonomy is entirely absent. This is a significant gap. |

**AC-NNN identifiers expected but absent from SPEC-21:**

| AC | Bridge Entry | SPEC-21 should cite because |
|----|--------------|---------------------------|
| AC-007 (HVM2 Reduction Engine) | YES (L204) | 8×8 dispatch table + on-the-fly redex detection is the closest prior art for streaming the reduction trace. Absent from SPEC-21. |
| AC-010 (HVM4 WNF Evaluation) | YES (L209) | Goto state machine, frame reuse, 1-file-per-interaction — the closest "streaming generation" reference for the pipeline architecture. Absent from SPEC-21. |
| AC-014 (Bench Methodology) | YES (L218) | If SPEC-21 streaming benchmarks are spec'd (T10 measures peak memory), this is the relevant methodology reference. Absent. |

**Non-catalogued references cited in SPEC-21 body:**

| Citation | Bridge status | Note |
|----------|--------------|-------|
| Tsourakakis et al., KDD 2014 (FENNEL) | NOT in bridge | Cited in §4.4 FennelStreamingStrategy. Not catalogued as REF-NNN. |
| Stanton & Kliot, KDD 2012 (LDG) | NOT in bridge | Cited in §4.4. Not catalogued as REF-NNN. |

**Bridge audit verdict:** PARTIAL FAIL. REF-001, REF-002, REF-005, REF-015, ARG-001, ARG-002, DISC-004 v2 all resolve (though DISC-004 is not in frontmatter). DISC-009 v2 is absent despite being the primary taxonomy source. Two non-catalogued references (FENNEL/LDG papers) are used in the body without REF-NNN registration. Three expected AC citations (AC-007, AC-010, AC-014) are absent.

`(source: docs/theory-bridge.md; specs/SPEC-21-streaming-generation.md, frontmatter + §4.2 + §4.4 + §5.1 + §5.2)`

---

### 4. Reference Implementation Mapping

**AC-007 — HVM2 Reduction Engine** (`docs/theory-bridge.md` L204):
- 8×8 dispatch table + atomic link with ownership + on-the-fly redex detection.
- Relevant to SPEC-21's streaming loop: the pipeline processes agents in batches and detects border wires (cross-partition "redexes") on-the-fly during `install_connection`. This mirrors AC-007's on-the-fly redex detection pattern.
- SPEC-21's §4.6 pseudocode is structurally analogous to AC-007's reduction loop: per-item dispatch → install → detect active pair (here: detect border).
- SPEC-21 does NOT cite AC-007.

**AC-010 — HVM4 WNF Evaluation + Interactions** (`docs/theory-bridge.md` L209):
- Goto state-machine architecture, frame reuse, 1-file-per-interaction organization.
- Most relevant to SPEC-21: the "streaming generation" concept maps to HVM4's incremental evaluation model where partial results are produced and consumed without materializing the full computation graph. The `PartitionAccumulator` pattern (§4.9) parallels HVM4's frame-reuse concept.
- SPEC-21 does NOT cite AC-010.

**AC-014 — Benchmark Methodology** (`docs/theory-bridge.md` L218):
- Relevant to SPEC-21's T10 performance test (§7.4) which specifies peak memory measurement. AC-014's `hrtime` wall-clock + discover-configure-execute-display runner is the canonical benchmark pattern.
- SPEC-21 does NOT cite AC-014.

`(source: docs/theory-bridge.md, L204-L218; specs/SPEC-21-streaming-generation.md, §4.6, §4.9, §7.4)`

---

### 5. Cross-Spec Consistency Hot-Spots

#### 5.1 SPEC-22 / Arena interaction

SPEC-22 (just closed, `Reviewed v2`) amends SPEC-01 I3 to I3' (uniqueness, not monotonicity). SPEC-21 R15 requires:

> "The generator MUST assign AgentId values to agents in a globally unique, monotonically increasing sequence across all batches ... the maximum AgentId in batch k MUST be less than the minimum AgentId in batch k+1."

This is **stricter than I3'** — it requires batch-level monotonicity in addition to uniqueness. The tension:
- R15 is a generator-level contract (generators must produce monotonically increasing IDs across batches).
- I3' is an arena-level contract (arena may reuse IDs via free-list).
- In the streaming pipeline, the generator produces IDs; the arena receives them via `PartitionAccumulator.add_agent()`. If R15 is satisfied, I3' is automatically satisfied for the generation phase. But what happens when workers reduce their accumulator-produced partitions and fire reduction rules (SPEC-03 creates new agents via `create_agent`, which may pull recycled IDs from the free-list under SPEC-22)?

SPEC-21 R23 specifies that accumulators must size to `max_agent_id_in_this_worker + 1`, not global max. Under SPEC-22's free-list, `create_agent` during reduction may return a recycled ID smaller than previously assigned IDs. SPEC-21 does not acknowledge this. Spec-critic should ask: does the streaming pipeline need to disable the free-list during accumulation (analogous to SPEC-22's RecyclePolicy::DisableUnderDelta)?

**SparseNet / PartitionAccumulator synergy (not explored):** SPEC-22 R22 mandates `SparseNet` for `build_subnet()` when `id_range > 4 × live_agent_count`. SPEC-21's §4.9 `PartitionAccumulator` wraps a `Net` (dense). For large nets with scattered ID assignments (e.g., FENNEL assigns non-contiguous IDs to workers), each accumulator could inflate exactly as `build_subnet()` did — the M5 pathology SPEC-22 R22 was designed to fix. SPEC-21 does not cross-reference SPEC-22's SparseNet for accumulator construction. This is a significant missed optimization and a potential invariant gap.

**Slot-id stability during streaming:** SPEC-22 R10b defines the "protected tombstone" mechanism for border-referenced IDs under delta mode. In the streaming pipeline, border wires are discovered incrementally as `install_connection` is called (§4.6 pseudocode). An agent with a border wire is inserted into the `border_map`. If that agent is later consumed by a reduction rule (after dispatch, during the BSP round), its slot should not be recycled. SPEC-21 does not reference SPEC-22's protected-tombstone mechanism. This is a gap if both SPEC-21 (streaming generation) and SPEC-22 (arena recycling) are active simultaneously in a streaming+delta pipeline.

`(source: specs/SPEC-21-streaming-generation.md, R15, R23, §4.9; specs/SPEC-22-arena-management.md, R10b/R10c, R22, R24)`

#### 5.2 SPEC-19 / Delta Protocol

SPEC-21 R36 (SHOULD) states:

> "The pull model MUST be compatible with the delta protocol (SPEC-19): if delta mode is active, workers accumulate chunks into their persistent partition state."

This SHOULD-grade requirement is the only point of contact between SPEC-21 and SPEC-19. It acknowledges the interaction but does not formalize it. Key unresolved questions:
- In delta mode, the coordinator holds a `BorderGraph`. When a new streaming chunk is dispatched to a worker (via pull model), the coordinator must update the `BorderGraph` with any new border wires in the chunk. SPEC-21 makes no mention of `BorderGraph` updates during chunked dispatch.
- If borders from chunk N are not registered in the BorderGraph before chunk N+1 arrives, the coordinator's border-redex detection may miss cross-chunk border redexes.
- SPEC-19 is NOT listed in SPEC-21's `Depends on` frontmatter despite R36 explicitly referencing it.

`(source: specs/SPEC-21-streaming-generation.md, R36; specs/SPEC-19-delta-protocol.md, §3.2)`

#### 5.3 SPEC-06 / SPEC-17 / SPEC-18 — Wire Format

SPEC-21 R31 adds `RequestWork` and `NoMoreWork` to the `Message` enum (SPEC-06). This is a wire-format change. The spec does NOT:
- Specify a PROTOCOL_VERSION bump.
- Reference SPEC-17 (Transport Abstraction / `ChannelTransport`) for in-memory testing of the pull protocol.
- Reference SPEC-18 (Wire Format v2) for serialization of the new variants.

The PROTOCOL_VERSION sequencing issue (also open in SPEC-22 TASK-0476): SPEC-22 lands as version 2→3. SPEC-20 lands as version 3→4. If SPEC-21 also requires a bump, the landing order determines the version number. This must be resolved by `especialista-em-specs` in the §3.8 amendments block.

`(source: specs/SPEC-21-streaming-generation.md, R31; specs/SPEC-06-wire-protocol.md, §3.3; specs/SPEC-22-arena-management.md, R9a / A9)`

#### 5.4 SPEC-13 / Coordinator FSM

SPEC-21 §3.6 (R30-R32) specifies a pull-based dispatch loop. R32 describes a 7-step protocol that replaces the standard `AssignPartition` → await `PartitionResult` loop with a more complex bidirectional loop. This requires:
- A new coordinator FSM state (e.g., `Streaming` or `AwaitingRequest` between the `Dispatching` and `WaitingResults` states in SPEC-13's FSM).
- New worker FSM state: worker sends `RequestWork` after sending `PartitionResult`, which is outside the current `Idle` → `Reducing` → `Sending` → `Idle` cycle.

SPEC-21 does not include a FSM diagram or transition table amendment. For task-splitter, this is a gap: without a formal FSM amendment, there is no clear insertion point in `src/coordinator.rs` or `src/worker.rs`.

`(source: specs/SPEC-21-streaming-generation.md, R30-R32; specs/SPEC-13-system-architecture.md, R19-R25)`

#### 5.5 SPEC-03 / Reduction

SPEC-21's pipeline terminates after `generate_and_partition_chunked()` and hands off to `run_grid` via a `PartitionPlan` (§6.2 compatibility bridge). The streaming pipeline itself does not interrupt `reduce_all` mid-loop; each worker receives a complete `Partition` (or in pull mode, accumulates multiple chunks). No `reduce_all` modification is required for the MVP.

However, the FENNEL strategy's `assignment_cache` grows to O(total_agents). For a 100M-agent net, this is ~800 MB for 8-byte entries. The spec acknowledges this as Q6 (§8) but does not bound it as a hard constraint. `spec-critic` should ask for a MUST-bound or a SHOULD-bound on cache size with a graceful degradation path.

`(source: specs/SPEC-21-streaming-generation.md, R5-R6, §8 Q6)`

#### 5.6 SPEC-25 Forward Reference

SPEC-25 (Recipe Generation, Wave 3) is the natural successor in the DISC-009 streaming hierarchy (generation-protocol level). SPEC-25 explicitly cites SPEC-21 as the fallback for non-decomposable benchmarks. SPEC-21 makes no mention of SPEC-25.

A forward-reference note in SPEC-21's §8 Open Questions would make the relationship explicit and avoid the task-splitter discovering the dependency only at SPEC-25 time.

`(source: specs/SPEC-25-recipe-generation.md, §1 + R18 + §5 table)`

---

### 6. Known Open Questions

**From SPEC-21 §8:**

| OQ | Question | Assessment |
|----|----------|-----------|
| Q1 | Accumulator memory for large nets (O(N) total). Addressed by ROADMAP 2.19 (early dispatch) or SPEC-25. | Not a spec-blocker, but Q1 should cross-reference SPEC-22 SparseNet as a mitigation path within SPEC-21's own scope. |
| Q2 | Optimal chunk_size (10,000 is a guess). | Acceptable for Draft; spec-critic should verify default is documented as "benchmark-TBD". |
| Q3 | FennelStreamingStrategy alpha parameter tuning. Adaptive alpha noted as future work. | The problem identified ("adaptive alpha requires knowing total edges/vertices upfront") contradicts the streaming model. Spec-critic should push for a clear resolution: either (a) fixed default with a note that per-benchmark calibration is a separate task, or (b) drop FENNEL to FUTURE scope. |
| Q4 | Interaction with delta protocol (SPEC-19). Deferred. | This is SPEC-21's biggest coherence gap. The "SHOULD" in R36 is insufficient given that the coordinator's BorderGraph is a MUST-maintain invariant under delta mode. |
| Q5 | Root port handling in streaming. Deferred. | R28 requires debug assertions from SPEC-04 §4.8 to pass; if root agent appears mid-stream, those assertions may fire prematurely. Spec-critic should probe this edge case. |
| Q6 | Port array sizing for FENNEL (sparse assignment → inflated dense accumulator). | This is the exact scenario SPEC-22 R22 was designed to address. Q6's resolution should reference SPEC-22 SparseNet. |

**Additional OQs from this brief:**

| OQ | Question |
|----|----------|
| OQ-A | **Determinism of streaming order:** R8 guarantees determinism given the same batch sequence. But `make_net_stream` is a `Box<dyn Iterator>`. Is iterator determinism a required property of generators? The spec does not say. If a generator uses non-deterministic ID assignment (e.g., drawing from a concurrent pool), R8 is vacuously satisfied but the results would be non-deterministic across runs. |
| OQ-B | **Backpressure / flow control:** R16 states the iterator is synchronous and that async backpressure is an Infrastructure-layer concern. But R32 (pull model) specifies a protocol where the coordinator generates the next chunk only when a worker requests it — this IS a backpressure mechanism. The spec needs to reconcile R16's "async backpressure is Infrastructure" with R32's "coordinator generates on demand". |
| OQ-C | **Termination signaling in push mode:** R31 defines `NoMoreWork` for the pull model. The push model has no termination signal — the coordinator sends all chunks upfront. How does the worker know all chunks have been processed? The current design (§6.2) has the coordinator return a single `PartitionPlan` equivalent after all chunks are processed, so the worker never sees individual chunks. This may be fine, but the spec should say so explicitly. |
| OQ-D | **v1 backward compatibility:** R26 specifies that `chunk_size = u32::MAX` degenerates to v1 behavior. But it also requires a SPEC-22 free-list to be present in the `Net` struct (since SPEC-22 is always-on per R28). This means the "v1 behavior" path uses a v2 `Net` type. The backward-compatibility guarantee is with respect to output correctness, not binary compatibility. The spec should clarify this distinction. |
| OQ-E | **Memory bounds on pending store:** Q1 bounds accumulator memory but the pending store (`HashMap<AgentId, Vec<PendingConnection>>`) is not bounded. For a generator that emits all forward references in batch 1 and resolves them all in batch N, the pending store holds O(total_agents) entries. This is unacknowledged. |

---

### 7. Round 1 Attack Surface for `spec-critic`

The following angles are predicted to yield the highest-severity findings in Round 1:

**1. Missing §3.8 Amendments block (CRITICAL — pipeline integrity)**
SPEC-21 amends SPEC-06 (R31), SPEC-07 (R24/R25/R34), SPEC-13 (R30-R32 coordinator FSM), and the `Benchmark` trait in `src/bench/mod.rs` (R10). None appear in a formal §3.8 block. Compare SPEC-19, SPEC-20, SPEC-22 — all have §3.8. The task-splitter cannot produce amendment tasks (Phase A) without explicit old-text/new-text entries. This is a hard block.

**2. DISC-009 v2 absent from `Discussions consumed:` (HIGH — theory bridge integrity)**
DISC-009 v2 is the canonical source for the generation-protocol streaming level (levels: rule-level / state-protocol / generation-protocol). The theory bridge explicitly marks DISC-009 v2 as informing "SPEC-21 Stage 0 (streaming generation)". The spec cites DISC-004 v2 in the body but not in frontmatter; DISC-009 v2 is not cited at all. Per the theory bridge usage policy: "An ID absent from this bridge is a hard block." In this case the ID IS in the bridge but is absent from the spec.

**3. REF-015 streaming-level mismatch (HIGH — citation correctness)**
REF-015 is cited in the frontmatter as "batch vs streaming" but the theory bridge categorizes it as "rule-level streaming reference" (streaming level 1) — the opposite end of the streaming hierarchy from generation-protocol (level 3). Spec-critic should probe whether REF-015 actually supports SPEC-21's claims or whether the citation is borrowed from DISC-009's taxonomy section.

**4. SPEC-22 × SPEC-21 PartitionAccumulator uses dense Net, not SparseNet (HIGH — M5 correctness)**
SPEC-21 §4.9 defines `PartitionAccumulator` wrapping a dense `Net`. For FENNEL strategy with non-contiguous ID assignments, each accumulator's dense arena inflates to `max_agent_id + 1` slots even when few agents are assigned to that worker. This is exactly the M5 pathology SPEC-22 R22 fixed in `build_subnet()`. SPEC-21 independently recreates the same problem in its accumulator design, without referencing SPEC-22's solution. Spec-critic should flag this as a potential invariant gap (memory bound violation at M5 target).

**5. PROTOCOL_VERSION bump not specified for R31 wire-format addition (HIGH — coordination with SPEC-22)**
R31 adds `RequestWork` and `NoMoreWork` to the `Message` enum. Every Message-catalog addition is a wire-format change. SPEC-22 A9 already plans a PROTOCOL_VERSION bump. If SPEC-21 also requires a bump, the two specs must coordinate their version numbers. The spec is silent on this.

**6. I3' vs R15 monotonicity tension (MEDIUM — invariant coherence)**
SPEC-22 relaxed I3 to I3' (uniqueness, not monotonicity). SPEC-21 R15 imposes batch-level monotonicity on generators. These are compatible but the spec does not acknowledge the relationship, leaving the implementation to discover the tension independently. The spec should explicitly state: "R15 is stricter than I3'; generators MUST produce monotonically increasing IDs to satisfy R15, but the arena MAY reuse IDs (I3') after dispatch."

**7. Pull model FSM not specified (MEDIUM — implementability)**
R30-R32 describe a pull protocol in prose and numbered steps but provide no FSM transition table or state diagram. SPEC-13 owns the coordinator/worker FSMs. Without explicit FSM states, the task-splitter cannot determine which tasks to assign to Phase B (coordinator) vs Phase C (worker). Spec-critic should demand a FSM amendment in §3.8.

**8. Termination invariant under pull model (MEDIUM — G1 threat)**
In the pull model, R35 covers the "fewer chunks than workers" edge case. But there is no analysis of the termination guarantee: if a worker that received `NoMoreWork` reduces its chunks to normal form and the result has border wires connecting to workers that are still receiving chunks, is the round termination condition still well-defined? The BSP barrier (SPEC-01 G1) requires all workers to complete a round before the coordinator merges. In the pull model, workers complete at different times. G1 applicability under pull dispatch is not established.

---

## Primary Sources

| # | File | Relevance |
|---|------|-----------|
| 1 | `specs/SPEC-21-streaming-generation.md` | Subject spec — read in full before Round 1 |
| 2 | `specs/SPEC-22-arena-management.md` | Just-closed sibling; §3.8 A1-A10 define I3', SparseNet, protected tombstones, PROTOCOL_VERSION v2→v3 — all cross-interact with SPEC-21 |
| 3 | `docs/theory-bridge.md` | Theory bridge — authoritative source for all ARG/DISC/REF/AC ID resolution |
| 4 | `specs/SPEC-06-wire-protocol.md` | Owns `Message` enum; R31 amendments land here |
| 5 | `specs/SPEC-13-system-architecture.md` | Owns coordinator/worker FSMs; pull model requires FSM amendments |
| 6 | `specs/SPEC-19-delta-protocol.md` | BorderGraph, delta mode, R10b protected tombstones — relevant to R36 |
| 7 | `specs/SPEC-25-recipe-generation.md` | Forward-reference sibling; clarifies SPEC-21's scope (streaming vs recipe) |
| 8 | `docs/briefings/SPEC-22-coherence-brief-2026-04-24.md` | Pattern reference for this brief; SPEC-22 Round 1 findings confirm §3.8 absence is a CRITICAL finding |

---

## Non-Obvious Connections

1. **SPEC-22 SparseNet is the correct accumulator implementation for SPEC-21's FENNEL strategy.** SPEC-21 §4.9 independently defines a dense `PartitionAccumulator` that suffers the same M5 inflation SPEC-22 R22 was designed to prevent. The fix is already in the codebase post-SPEC-22 (`SparseNet` + `to_dense` with id_range bound). SPEC-21 should adopt SPEC-22's solution rather than re-inventing an inferior one.

2. **R36's SHOULD-grade delta+streaming interaction is de facto MUST under M5 target.** The M5 milestone ("ep_con 100M runs on 2GB coordinator") is only achievable if both streaming (SPEC-21) and delta protocol (SPEC-19) are active simultaneously. A SHOULD-grade requirement for their interaction means the milestone condition is optional. This is a scope-definition problem that spec-critic should surface.

3. **DISC-009's 5-operating-mode taxonomy classifies pull-based dispatch as a distinct operating mode from streaming generation.** SPEC-21 bundles both under one spec (§§3.1-3.3 = streaming generation, §3.6 = pull-based). DISC-009 v2 treats these as separate entries in its 5-mode taxonomy. The bundling is defensible (they share types and config), but the spec should cite DISC-009 to justify this decision.

4. **The `Benchmark` trait amendment (R10) affects all 13 benchmarks in SPEC-09.** Adding `make_net_stream()` to the `Benchmark` trait breaks all existing implementations unless a default implementation is provided (e.g., a blanket `fn make_net_stream(&self, size, chunk_size) -> Box<dyn Iterator<...>> { ... collect make_net(size) into batches ... }`). SPEC-21 does not specify whether the default implementation is provided or whether all 13 benchmarks must be manually updated. The task-splitter will need this decision to estimate Phase effort.

5. **PROTOCOL_VERSION triple-collision risk.** SPEC-20 bumps v → v+1; SPEC-22 bumps v+1 → v+2; SPEC-21 (if it requires a bump for R31) bumps v+2 → v+3. The landing order must be fixed before DEV begins, or all three specs will collide on PROTOCOL_VERSION. This is the third occurrence of this sequencing issue and escalates it from "open question" to "architectural decision required."

---

## Identified Gaps

The following were searched for and NOT found:

1. **§3.8 Amendments block in SPEC-21** — does not exist. The spec self-declares as "an amendment to SPEC-21" in §3.6 header but no structured amendment section exists.
2. **DISC-009 v2 in SPEC-21 `Discussions consumed:`** — absent.
3. **Any mention of SPEC-22/SparseNet/free_list in SPEC-21** — zero matches. The two specs were likely authored in parallel and did not cross-reference each other.
4. **Any mention of SPEC-17 (Transport Abstraction) or SPEC-18 (Wire Format v2) in SPEC-21** — zero matches despite R31 requiring a new wire message.
5. **PROTOCOL_VERSION disposition for R31** — not specified anywhere in the spec.
6. **FSM transition table for pull dispatch mode** — absent; only prose description in R30-R32.
7. **REF-NNN entries for FENNEL (Tsourakakis 2014) and LDG (Stanton & Kliot 2012)** — not in `biblioteca/referencias.bib` or theory bridge. These are cited in §4.4 body text without registration.
8. **Memory bound on pending store** — explicitly unaddressed in §8 Open Questions despite being O(total_agents) in worst case.
