# SPEC-REVIEW-21 — Round 1 Adversarial Review

**Date:** 2026-04-25
**Reviewer:** Spec Critic (adversarial)
**Target:** `specs/SPEC-21-streaming-generation.md` (Status: Draft v1)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-06, SPEC-07, SPEC-09, SPEC-13, SPEC-17, SPEC-18, SPEC-19, SPEC-22
**Briefing:** `docs/briefings/SPEC-21-coherence-brief-2026-04-25.md`
**Bridge:** `docs/theory-bridge.md`

---

## Summary

**Gate decision:** **BLOCK.** SPEC-21 cannot proceed to Round 2 / DEV in its current form.

**Top-3 concerns (one line each):**
1. **No §3.8 Amendments block** — five cross-spec amendments (SPEC-06 R31, SPEC-07 R24/R25/R34, SPEC-09 R10, SPEC-13 R30-R32, SPEC-04 §4.8) are buried in body prose; task-splitter cannot generate Phase A tasks. (SC-001)
2. **PartitionAccumulator (§4.9) recreates the M5 dense-arena pathology that SPEC-22 R22 just fixed** — must adopt SparseNet under FENNEL non-contiguous assignment or M5 milestone is unreachable. (SC-006)
3. **G1 fundamental property threat from SPEC-22 free-list × SPEC-21 streaming interaction** — recycled slot IDs may collide with border-referenced agents during overlapping generation/reduction; protected-tombstone mechanism (SPEC-22 R10b) not invoked. (SC-007)

**Severity counts:**

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH     | 7 |
| MEDIUM   | 10 |
| LOW      | 5 |
| **Total** | **24** |

**Mandatory (must fix before Round 2):**
- SC-001 (Missing §3.8)
- SC-002 (Predecessors absent from Depends on)
- SC-003 (DISC-009 v2 absent)
- SC-004 (REF-015 streaming-level mismatch)
- SC-005 (PROTOCOL_VERSION undecided)
- SC-006 (PartitionAccumulator dense-net M5 pathology)
- SC-007 (G1 threat: free-list × streaming)
- SC-008 (Benchmark trait default-impl decision)
- SC-009 (I3'/R15 reconciliation)

**Recommended (should fix):**
- SC-010 through SC-019 (MEDIUM)

**Note:** The brief's five predicted findings are all confirmed and sharpened. Two CRITICAL, four HIGH, plus three additional HIGH adversarial findings beyond the brief (SC-006 PartitionAccumulator dense-net, SC-007 G1 free-list interaction, SC-008 Benchmark trait default-impl) emerged from cross-checking against SPEC-22's just-closed amendments.

---

## Findings — CRITICAL

### SC-001 — Missing §3.8 Amendments block

**Severity:** CRITICAL
**Axis:** Completeness / Pipeline integrity
**Section:** All amendment-bearing requirements (R10, R24, R25, R30-R34, R31)
**Problem:** SPEC-21 amends at minimum five other artifacts — SPEC-06 (`Message` enum gains `RequestWork`/`NoMoreWork`, R31), SPEC-07 (`GridConfig` gains `chunk_size`, streaming-strategy selector, `DispatchMode`, R24/R25/R34), SPEC-13 (coordinator/worker FSMs gain pull-dispatch states, R30-R32), SPEC-09's `Benchmark` trait (`make_net_stream` method, R10), and implicitly SPEC-04 (border-id allocation lifted out of `split()` into the streaming pipeline, §4.8). None of these appear in a structured §3.8 Amendments block with old-text/new-text/rationale entries. This is the same pattern that produced the top-ranked finding in SPEC-22 Round 1 and was a hard block until §3.8 was authored.
**Impact if unresolved:** The task-splitter cannot generate Phase A amendment tasks. Every amendment must be reverse-engineered from prose scattered across §3.1-§3.6, with no canonical old-text/new-text reference for the developer to apply. Each downstream spec (SPEC-06/07/09/13) cannot be patched in lockstep. PROTOCOL_VERSION sequencing (see SC-005) cannot be reasoned about without a single block enumerating wire-format changes.
**Suggested resolution:** Author a §3.8 Amendments block with at minimum these entries, mirroring SPEC-22 §3.8 structure (A1..A10):
- A1: SPEC-06 R-NN — add `Message::RequestWork { worker_id }` and `Message::NoMoreWork` variants.
- A2: SPEC-06 — PROTOCOL_VERSION disposition (see SC-005).
- A3: SPEC-07 GridConfig — add `chunk_size: u32`, `streaming_strategy: StreamingStrategyConfig`, `dispatch_mode: DispatchMode`.
- A4: SPEC-09 `Benchmark` trait — add `make_net_stream(&self, size, chunk_size) -> Box<dyn Iterator<Item = AgentBatch>>` with explicit default-impl decision (SC-008).
- A5: SPEC-13 — coordinator FSM state additions for pull dispatch; worker FSM `RequestWork` emission state.
- A6: SPEC-04 §4.5 — clarify that `split()` remains unchanged; chunked pipeline is additive, not a replacement.

### SC-002 — Predecessors missing from `Depends on:` despite explicit amendments

**Severity:** CRITICAL
**Axis:** Consistency
**Section:** Frontmatter `Depends on:` line
**Problem:** SPEC-21 declares dependence only on SPEC-01, SPEC-02, SPEC-04, SPEC-05, SPEC-13. The body and §3.6 amendment requirements demonstrate dependence on at least six additional specs that are not declared:
- **SPEC-06** — R31 modifies the `Message` enum.
- **SPEC-07** — R24, R25, R34 add three `GridConfig` fields.
- **SPEC-09** — R10 modifies the `Benchmark` trait housed in SPEC-09 / `src/bench/`.
- **SPEC-17** — R31's new variants must serialize over the transport-abstraction layer; in-memory `ChannelTransport` testing of pull protocol requires SPEC-17 contract.
- **SPEC-18** — Wire format v2 serde must accept the new variants.
- **SPEC-19** — R36 explicitly references the delta protocol's `BorderGraph`.
- **SPEC-22** — §4.9 `PartitionAccumulator` interacts with arena recycling, free-list, and SparseNet (see SC-006, SC-007).
**Impact if unresolved:** Spec-graph integrity is broken. Reviewers and task-splitter cannot identify which predecessors must be re-read before implementation. CI tooling that validates the spec graph (if any) emits no warnings. Cross-spec amendments to SPEC-06/07/09/13/17/18/19/22 are invisible to the dependency planner.
**Suggested resolution:** Update the frontmatter `Depends on:` line to include SPEC-06, SPEC-07, SPEC-09, SPEC-17, SPEC-18, SPEC-19, and SPEC-22. For each addition, the §3.8 Amendments block (SC-001) should justify the dependency with the specific R-number that triggers it.

---

## Findings — HIGH

### SC-003 — DISC-009 v2 absent from `Discussions consumed:`

**Severity:** HIGH
**Axis:** Consistency / Theory bridge integrity
**Section:** Frontmatter; §1
**Problem:** Per `docs/theory-bridge.md` L124-127, DISC-009 v2 is the canonical taxonomy source for the three streaming levels (rule-level / state-protocol / generation-protocol) and the five operating modes. SPEC-21 covers level-3 streaming-generation, the third tier of DISC-009's taxonomy, yet DISC-009 v2 is not cited anywhere in the spec — neither in `Discussions consumed:` frontmatter nor in any body section. The theory bridge usage policy treats unverified ID claims as hard blocks; here the inverse problem applies — the spec is silent on its primary theoretical anchor.
**Impact if unresolved:** SPEC-21's positioning relative to SPEC-25 (recipe generation, level-3+) and SPEC-14 §8 (rule-level streaming, level-1) cannot be reconstructed by a reader without the bridge. The justification for bundling §3.1-§3.3 (streaming generation) with §3.6 (pull dispatch) into one spec lacks its taxonomic citation — DISC-009 v2 explicitly catalogs these as distinct operating modes.
**Suggested resolution:** Add `DISC-009 v2` to `Discussions consumed:` in frontmatter. Add a single-paragraph cross-reference in §1 stating "SPEC-21 covers DISC-009 v2's generation-protocol streaming level (level 3); the pull-based dispatch in §3.6 corresponds to the on-demand operating mode in the same taxonomy." Update §3.6 prose to cite DISC-009 v2 when justifying the bundling decision.

### SC-004 — REF-015 streaming-level mismatch

**Severity:** HIGH
**Axis:** Consistency / Citation correctness
**Section:** Frontmatter; §1 (implicit)
**Problem:** SPEC-21 frontmatter cites REF-015 (Mackie & Sato 2015) tagged as "batch vs streaming" — a phrasing that suggests generation-protocol streaming. However, `docs/theory-bridge.md` L170 catalogs REF-015 as a **rule-level** streaming reference (DISC-009 streaming level 1, the per-fired-rule trace level), explicitly tied to SPEC-14 §8. Level 1 and level 3 are at opposite ends of DISC-009's hierarchy; REF-015 does not establish anything about generation-protocol streaming, only about reduction-trace streaming.
**Impact if unresolved:** Either (a) the citation is borrowed from DISC-009's taxonomy section without verifying it supports SPEC-21's level-3 claims, or (b) REF-015 is being repurposed without a justifying note. In either case, a reader following the citation will not find support for SPEC-21's pipeline architecture in REF-015. Round 2/3 reviewers and the redator (when this spec ends up in the artigo) will be unable to use REF-015 as evidence for streaming generation.
**Suggested resolution:** Either (a) drop REF-015 from frontmatter and replace with DISC-009 v2 (the actual primary source — see SC-003), or (b) keep REF-015 but add a justifying sentence in §5 Rationale: "REF-015 establishes streaming at the rule-level (level 1); SPEC-21 generalizes the streaming concept to the generation-protocol level (level 3) per DISC-009 v2."

### SC-005 — PROTOCOL_VERSION disposition for R31 unspecified

**Severity:** HIGH
**Axis:** Consistency / Cross-spec coordination
**Section:** R31; missing §3.8
**Problem:** R31 adds two new variants (`RequestWork`, `NoMoreWork`) to the `Message` enum owned by SPEC-06. Every Message-catalog addition is a wire-format change. SPEC-22 R9a/A9 already plans a PROTOCOL_VERSION bump (v2 → v3); SPEC-20 plans an independent bump (v3 → v4). SPEC-21 is silent on whether R31 also requires a bump and, if so, what version number the post-merge state should be. This is the third spec in the pre-DEV wave to touch PROTOCOL_VERSION without a coordinated sequencing decision; the brief escalates this from "open question" to "architectural decision required."
**Impact if unresolved:** If all three specs land independently and each bumps the constant, the result is non-deterministic depending on merge order. Workers running mismatched binaries will silently misinterpret messages. Backward-compatibility tests cannot be authored because the target version is undefined.
**Suggested resolution:** Add a §3.8 amendment entry (under SC-001) explicitly declaring SPEC-21's PROTOCOL_VERSION position. Coordinate with `especialista-em-specs` to settle the wave-2 sequencing: e.g., "SPEC-21 bumps from whatever-SPEC-22-and-SPEC-20-leave (v4) to v5" or "SPEC-21's R31 wire additions piggyback on the SPEC-22 v2→v3 bump because both land in the same release; no further bump required." Either decision is acceptable; ambiguity is not.

### SC-006 — PartitionAccumulator wraps a dense `Net`, recreating the M5 pathology SPEC-22 R22 just fixed

**Severity:** HIGH
**Axis:** Invariant Preservation / Cross-spec consistency
**Section:** §4.9 (PartitionAccumulator); R23
**Problem:** §4.9 specifies `PartitionAccumulator { subnet: Net, ... }` where `Net` is the dense SPEC-02 layout: `agents: Vec<Option<Agent>>` and a parallel `ports: Vec<PortRef>` whose length is `agents.len() * PORTS_PER_SLOT`. R23 mandates that the accumulator's Vecs be sized to `max_agent_id_in_this_worker + 1`. Under the FENNEL strategy (R5), agents are assigned to workers by topological similarity, not by ID range — a single worker may receive agent 0 and agent 5_000_000 while owning only ~1000 agents total. The accumulator's port array then inflates to 5_000_001 × 3 = 15M `PortRef` entries to hold ~3000 live ports. This is precisely the M5 pathology SPEC-22 R22 fixed in `build_subnet()` via `SparseNet` under the threshold `id_range > 4 × live_agent_count`. SPEC-21 was authored in parallel with SPEC-22 and contains zero references to `SparseNet`, `free_list`, `RecyclePolicy`, or `id_range × live_agent_count` thresholds.
**Impact if unresolved:** The M5 milestone target ("ep_con 100M runs on 2GB coordinator") is unreachable when SPEC-21 streaming and SPEC-22 SparseNet must coexist — the streaming pipeline reintroduces the dense-arena inflation that SparseNet eliminates. The spec violates SPEC-22's R22 contract by constructing partitions whose internal `Net` does not pass the SparseNet threshold check. T10 (peak memory measurement) will fail on FENNEL+large nets.
**Suggested resolution:** Amend §4.9 to construct the accumulator using `SparseNet` (SPEC-22 R22) when `id_range > 4 × live_agent_count`, with a `to_dense()` finalization step matching SPEC-22's `build_subnet()` discipline. Add a cross-reference: "PartitionAccumulator MUST follow SPEC-22 R22 SparseNet selection; R23's id_range sizing applies to the dense-finalized form, not the in-progress accumulator." Add the corresponding entry to §3.8 (SC-001).

### SC-007 — G1 threat: slot-id stability across stream chunks vs SPEC-22 free-list recycling

**Severity:** HIGH
**Axis:** Invariant Preservation
**Section:** R15, R23, §4.6 install_connection; cross-cuts SPEC-22 R10b
**Problem:** R15 requires generators to produce monotonically increasing `AgentId`s across batches. SPEC-22 amended I3 to I3' (uniqueness, not monotonicity) and added a free-list-based slot recycler with `RecyclePolicy` and protected tombstones (R10b). During a streaming pipeline run, two phases may overlap once early dispatch (Q1, ROADMAP 2.19) is integrated:
1. The coordinator continues feeding chunks into accumulators.
2. Workers begin reducing previously dispatched partitions, firing reduction rules that consume agents and (under SPEC-22) feed slot IDs into the free-list.
If a recycled slot ID is later assigned by a worker's `create_agent` call to a NEW agent, while a still-pending border wire from the streaming side references the OLD agent at that slot, G1 violates: the border-target identity is ambiguous. SPEC-22 R10b ("protected tombstones for border-referenced IDs under delta mode") is the precise mechanism to prevent this — but SPEC-21 does not invoke it.
**Impact if unresolved:** Streaming + delta mode (the M5 target combination) can produce non-deterministic results under concurrent reduction and chunk dispatch. The G1 fundamental property — sequential-baseline equivalence — fails silently.
**Suggested resolution:** Add an explicit requirement (e.g., R37b): "During streaming pipeline execution, the worker arena MUST disable free-list recycling for any slot whose ID appears in `border_map` or in the pending connection store; equivalently, MUST use SPEC-22 R10b protected-tombstone discipline." Cross-reference SPEC-22 R10b. Add to §3.8 amendment block as a SPEC-22 interaction note. Resolve OQ-D (brief) explicitly.

### SC-008 — `Benchmark` trait amendment (R10) breaks 13 existing benchmark implementations unless a default impl is provided

**Severity:** HIGH
**Axis:** Completeness / Cross-spec consistency
**Section:** R10; missing §3.8
**Problem:** R10 adds `make_net_stream(&self, size: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>>` to the `Benchmark` trait. R11 retains `make_net` "as a convenience wrapper that collects the stream into a full net," which is the inverse of what R10 says — R10 describes adding a new method, not changing the existing one. Per the brief (Section "Non-Obvious Connections" #4), the codebase has 13 `Benchmark` implementations in SPEC-09. The spec does not state whether R10 supplies a default impl (e.g., `fn make_net_stream(&self, size, chunk_size) -> ... { collect_make_net_into_batches(self.make_net(size), chunk_size) }`) or whether all 13 implementations must be hand-updated. R12 says only `ep_annihilation` MUST support streaming for MVP; the other 12 are SHOULD. Without a default impl, the SHOULD becomes effectively MUST — code does not compile until every implementor exists.
**Impact if unresolved:** task-splitter cannot estimate Phase B effort: either ~30 LoC (one default impl) or ~13 × ~40 LoC ≈ 520 LoC of trait-implementation work. Either reading is consistent with R10/R11/R12 prose. The brief flags this as "the task-splitter will need this decision to estimate Phase effort."
**Suggested resolution:** Make R10 explicit: "R10. The `Benchmark` trait gains `make_net_stream` with a default implementation `{ Box::new(default_chunked_iter(self.make_net(size), chunk_size)) }` so that existing implementations remain valid. Generators that benefit from native streaming (e.g., `ep_annihilation`) MUST override the default per R12." Add corresponding entry to §3.8 (SC-001).

### SC-009 — I3'/R15 contract reconciliation unstated

**Severity:** HIGH
**Axis:** Invariant Preservation
**Section:** R15; cross-cuts SPEC-22 §3.8 A1 (I3 → I3')
**Problem:** SPEC-22 relaxed I3 to I3' (uniqueness, not monotonicity) to permit free-list recycling. SPEC-21 R15 imposes batch-level monotonicity on generators ("the maximum AgentId in batch k MUST be less than the minimum AgentId in batch k+1"). These are formally compatible — R15 is strictly stronger than I3', so satisfying R15 satisfies I3'. But the spec does not acknowledge the relationship, leaving an implementer to discover the subtle interaction independently. The relevant subtlety: R15 applies only to the **generation** phase; once chunks are dispatched and workers reduce them, the arena MAY recycle IDs (I3'), and SPEC-21 must then no longer hold any monotonicity assumption.
**Impact if unresolved:** Implementers reading R15 in isolation may assume monotonicity is a global property and write assertions in `reduce_all` or `merge` that fail once recycling kicks in. Conversely, implementers reading SPEC-22 I3' in isolation may write generators that produce non-monotonic IDs and violate R15's stricter contract. Round 2 review will surface this; flagging now is cheaper.
**Suggested resolution:** Add a one-paragraph note at the end of §3.5 (Invariant Preservation): "R15 is a generator-phase contract: it is strictly stronger than SPEC-01 I3' (post-amendment by SPEC-22 §3.8 A1). Once a chunk has been dispatched and a worker fires reduction rules, the worker's arena MAY recycle slot IDs per I3'. Code in `src/partition/streaming.rs` MUST NOT assume monotonicity on agents created post-dispatch."

---

## Findings — MEDIUM

### SC-010 — DISC-004 v2 cited in body only, not in `Discussions consumed:`

**Severity:** MEDIUM
**Axis:** Consistency / Citation hygiene
**Section:** Frontmatter; §4.2 (trait doc-comment); §5.1
**Problem:** §4.2 doc comment cites "DISC-004 v2, Section 1.6" for the partition-quality independence argument; §5.1 also references DISC-004 v2 implicitly via ARG-002 Passo 10. The `Discussions consumed:` frontmatter line is absent entirely (unlike SPEC-22 which has the line populated). DISC-004 v2 is a primary source for §4.2's correctness rationale; its omission from frontmatter breaks the project's citation-integrity convention.
**Impact if unresolved:** Round 2 reviewers and the redator-when-it-reaches-the-artigo cannot mechanically discover DISC-004 v2 as a source. The brief's citation audit (Section 3) flags this as a soft fail.
**Suggested resolution:** Add a `Discussions consumed:` line to frontmatter listing both `DISC-004 v2` and `DISC-009 v2` (per SC-003).

### SC-011 — AC-007, AC-010, AC-014 absent despite natural anchors

**Severity:** MEDIUM
**Axis:** Theory bridge integrity
**Section:** Frontmatter; §4.6 (pipeline architecture); §7.4 (T10 peak memory)
**Problem:** Per the brief Section 3 / Section 4:
- AC-007 (HVM2 Reduction Engine, on-the-fly redex detection) maps closely to §4.6's `install_connection`-style border detection during the streaming loop.
- AC-010 (HVM4 WNF Evaluation, frame reuse and goto state machine) maps to the `PartitionAccumulator` (§4.9) frame-reuse pattern.
- AC-014 (Bench Methodology) is the canonical methodology reference for T10 peak-memory measurement (§7.4).
None of these are cited in SPEC-21.
**Impact if unresolved:** SPEC-21 lacks reference-implementation grounding. Other Wave 2 specs (SPEC-19/20/22) carry AC citations where applicable; SPEC-21's absence is anomalous.
**Suggested resolution:** Add to frontmatter `References consumed:`: AC-007, AC-010, AC-014. Add a one-sentence cross-reference in §4.6 (AC-007), §4.9 (AC-010), and §7.4 (AC-014).

### SC-012 — Backpressure and flow control between R16 and R32 unreconciled

**Severity:** MEDIUM
**Axis:** Completeness
**Section:** R16 vs R32; OQ-B in brief
**Problem:** R16 declares the iterator synchronous and that "Integration with async channels (e.g., `tokio::sync::mpsc`) for backpressure is an Infrastructure-layer concern handled by the coordinator, not by the generator." R32 step 4 specifies that the coordinator "generates the next chunk (via `make_net_stream`), partitions it (via `StreamingPartitionStrategy`), and sends `AssignPartition` with the new chunk to the requesting worker" — that IS a backpressure mechanism: the rate of generation is gated by the rate of `RequestWork` arrivals. The two requirements are not formally inconsistent (R16 is about the iterator's synchronous nature; R32 is about coordinator orchestration), but the spec does not explain the relationship and Q (OQ-B in brief) flags this as confusing.
**Impact if unresolved:** Implementers reading R16 may assume no flow control exists; implementers reading R32 may assume the iterator must be lazy and re-entrant. The pull dispatch mode requires the iterator to be paused between chunks — does R16's synchronous contract permit this (it does, since `Iterator::next` is pull-based by definition)?
**Suggested resolution:** Add a clarifying sentence to R16: "The iterator's pull-based `next()` interface naturally supports the pull dispatch model (R32) without async coordination — the coordinator drives the iterator one `next()` call per `RequestWork` message. Async channels (`tokio::sync::mpsc`) are required only for the push dispatch model with overlapping generation and dispatch."

### SC-013 — Termination signaling for push mode unspecified

**Severity:** MEDIUM
**Axis:** Completeness
**Section:** R31 (NoMoreWork is pull-only); §6.2 push-mode bridge; OQ-C in brief
**Problem:** R31 introduces `NoMoreWork` as the termination signal for the pull model. Section §6.2 shows the push-mode bridge collecting all chunks into a `PartitionPlan` before dispatch, so workers see only one `AssignPartition` followed by the standard merge protocol. But the spec never explicitly states: "In push mode, no termination message is required because workers receive the complete partition in a single `AssignPartition`." For an implementer toggling between modes, the absence is a guess-the-protocol problem.
**Impact if unresolved:** Worker implementations may add defensive `NoMoreWork` handling to push mode unnecessarily, or omit it from pull mode by accident. Test T13 (short stream) is designed for pull only; equivalent push-mode behavior is undocumented.
**Suggested resolution:** Add to §3.6 or §6.2: "In push mode (the default for `num_workers ≤ 2`), no `NoMoreWork` message is sent; the worker receives a single `AssignPartition` with the complete partition and proceeds to the standard merge protocol. `NoMoreWork` is meaningful only in pull mode (R31)."

### SC-014 — v1 backward compatibility scope of R26 imprecise

**Severity:** MEDIUM
**Axis:** Consistency / Completeness
**Section:** R26; OQ-D in brief
**Problem:** R26 specifies that `chunk_size = u32::MAX` degenerates to v1 behavior. After SPEC-22's `Net` amendments (free-list, RecyclePolicy, SparseNet), the v1 code-path no longer exists in pristine form — even the "degenerate" path uses a v2 `Net`. R26 does not clarify whether the backward-compatibility guarantee is (a) bit-identical output to v1 (impossible — `Net` layout differs), (b) merge-result isomorphism with v1 (likely intended), or (c) simply "use SPEC-04 `split()` rather than `generate_and_partition_chunked`."
**Impact if unresolved:** Test T6 (streaming vs batch equivalence) measures isomorphism, which works regardless of layout differences. But "v1 behavior" elsewhere in the codebase may mean different things to different reviewers.
**Suggested resolution:** Reword R26 to: "When `chunk_size = u32::MAX`, the pipeline MUST short-circuit to SPEC-04 `split()` after collecting the full stream into a single Net. The merge result MUST be isomorphic (SPEC-00 §6.12) to the v1 `split()`-produced result; bit-identical layout is NOT guaranteed because of SPEC-22 arena-management amendments."

### SC-015 — Pull-model FSM not specified; only prose narrative

**Severity:** MEDIUM
**Axis:** Completeness / Testability
**Section:** R30-R32; cross-cuts SPEC-13
**Problem:** R30-R32 describe pull dispatch as a 7-step protocol in numbered prose with no FSM transition table or state diagram. SPEC-13 owns coordinator and worker FSMs as formal state machines. The pull model implies new states (e.g., coordinator: `AwaitingRequest`; worker: `AwaitingChunkAfterResult`) and new transitions, but their formal description is absent.
**Impact if unresolved:** The task-splitter cannot decompose pull-dispatch implementation into FSM-state tasks vs orchestration tasks. The reviewer cannot verify that all transition edges are exhaustive (e.g., what happens when `RequestWork` arrives at a coordinator that has not yet finished generating chunk N+1?). SPEC-13 has no patch authored.
**Suggested resolution:** Add an FSM transition table to §3.6 listing (Coordinator state, Event, Action, Next state) tuples for at minimum: Init → DispatchingFirst, DispatchingFirst → AwaitingResults, AwaitingResults+RequestWork → GeneratingNext, GeneratingNext+stream-done → SendingNoMoreWork, SendingNoMoreWork → AwaitingFinalResults. Same for worker. Reference SPEC-13's existing FSM structure.

### SC-016 — Pending-store memory bound unaddressed

**Severity:** MEDIUM
**Axis:** Completeness / Testability
**Section:** §4.7 ("at most O(forward_refs_in_flight)"); §8 Q1; OQ-E in brief
**Problem:** §4.7 claims the pending store is bounded by "O(forward_refs_in_flight)" and that for `dual_tree` this is O(width_of_current_layer). For pathological generators that emit all forward references in batch 1 and resolve them all in the final batch (a legitimate generation pattern), the store grows to O(total_agents). Q1 (§8) bounds accumulator memory but explicitly does not address the pending store; OQ-E in the brief identifies this as unaddressed.
**Impact if unresolved:** T10 (peak memory measurement) can pass on `ep_annihilation` (no forward refs) but fail on a future generator that defers forward-reference resolution. The M5 milestone's memory bound is undefined for such generators.
**Suggested resolution:** Add a requirement (e.g., R28b): "Generator implementations MUST resolve any forward reference within at most `MAX_PENDING_LIFETIME` chunks (default: 16). The pipeline MAY enforce this as a debug assertion. Generators that violate the bound MUST be refactored or excluded from streaming mode." Alternatively: "The pending store has no MUST-bound; SPEC-21 documents that streaming-mode peak memory may include O(total forward references) for adversarial generators." Either decision is acceptable; silence is not.

### SC-017 — SPEC-19 BorderGraph update protocol under chunked dispatch unspecified

**Severity:** MEDIUM
**Axis:** Consistency / Completeness
**Section:** R36 (SHOULD); cross-cuts SPEC-19 §3.2
**Problem:** R36 is the only point of contact with SPEC-19 and is only SHOULD-grade. It says workers "accumulate chunks into their persistent partition state" but does not say how the coordinator's `BorderGraph` (SPEC-19 §3.2) gets updated when each new chunk introduces border wires. If the BorderGraph is not updated before chunk N+1's reductions begin, the coordinator's border-redex detection misses cross-chunk active pairs. Per the brief (Section 5.2 / Non-Obvious Connection #2), the M5 target ("ep_con 100M on 2GB coordinator") is only achievable with simultaneous SPEC-21 and SPEC-19 — making this SHOULD effectively MUST.
**Impact if unresolved:** Streaming + delta mode is unimplementable from SPEC-21 alone. Either SPEC-21 or SPEC-19 must own the BorderGraph-update-on-chunk-arrival contract; currently neither does.
**Suggested resolution:** Either (a) elevate R36 to MUST and specify that the coordinator MUST call `BorderGraph::extend_with_chunk_borders(&new_borders)` after each `install_connection` call that yields a border wire under delta mode; or (b) explicitly defer streaming+delta to a future spec and tighten R36 to "SPEC-21 streaming and SPEC-19 delta protocol are NOT compatible in v2; combined operation is deferred to SPEC-NN." Coordinate with the SPEC-19 owner.

### SC-018 — Border-id allocation §4.8 implicitly amends SPEC-04 R12 without §3.8 entry

**Severity:** MEDIUM
**Axis:** Consistency
**Section:** §4.8; cross-cuts SPEC-04 R12
**Problem:** §4.8 specifies a new border-id allocation policy: "border IDs start at 0 and increment monotonically" for fresh-generated nets. SPEC-04 R12 specifies "border IDs start at `max_existing_freeport_id(net) + 1`." The two are reconciled by §4.8 step 2 (scan first batch for max FreePort), but this is a substantive change to the border-id allocation contract and merits a §3.8 amendment entry against SPEC-04. As authored, the change is buried in a Design section.
**Impact if unresolved:** SPEC-04's R12 contract is silently weakened; readers expecting a single border-id-allocation policy across the codebase find two. Tests that exercise both `split()` and `generate_and_partition_chunked` may produce non-overlapping but inconsistent border-id ranges.
**Suggested resolution:** Promote §4.8's allocation policy to a numbered requirement (e.g., R29b) and add the corresponding §3.8 amendment entry referencing SPEC-04 R12.

### SC-019 — G1 termination semantics under pull dispatch unestablished

**Severity:** MEDIUM
**Axis:** Invariant Preservation
**Section:** R35; cross-cuts SPEC-01 G1; brief Attack Surface item 8
**Problem:** R35 covers the "fewer chunks than workers" edge case but does not analyze G1's BSP-barrier guarantee under pull dispatch. SPEC-01 G1 requires sequential-baseline equivalence; the BSP synchronization barrier (SPEC-05 R-NN) requires all workers to complete a round before merge. Under pull dispatch, workers complete chunks at different wall-clock times — fast workers receive more chunks, slow workers receive fewer or `NoMoreWork` early. The merge step (SPEC-05) is invoked only after all `NoMoreWork` signals are acknowledged, but the relationship between BSP rounds and pull dispatch is not formalized.
**Impact if unresolved:** A reader cannot determine whether a "round" in pull dispatch corresponds to (a) one chunk per worker, (b) all chunks until a global barrier, or (c) something else. T11 (pull-based dispatch protocol) tests result correctness but not BSP-barrier semantics.
**Suggested resolution:** Add a paragraph to §3.6 stating: "The BSP barrier under pull dispatch is the moment all workers acknowledge `NoMoreWork`; before this moment, individual workers may complete reductions on their accumulated chunks but MUST NOT begin the merge phase. This preserves G1 by reducing pull dispatch to a single 'logical BSP round' regardless of wall-clock interleaving."

---

## Findings — LOW

### SC-020 — FENNEL (Tsourakakis 2014) and LDG (Stanton & Kliot 2012) papers cited without REF-NNN registration

**Severity:** LOW (TCC-root cleanup territory, analog of SPEC-22 SC-013)
**Axis:** Theory bridge integrity
**Section:** §4.4 (FennelStreamingStrategy doc-comment)
**Problem:** §4.4 cites "Tsourakakis et al., KDD 2014 (FENNEL)" and "Stanton & Kliot, KDD 2012 (LDG)" inline. Neither paper appears in `biblioteca/referencias.bib` nor in `docs/theory-bridge.md`. Per the project convention, every external reference cited in a spec must have a REF-NNN entry in the bridge.
**Impact if unresolved:** Bridge audit fails for these citations. The redator cannot generate a bibliography entry when these end up in the artigo.
**Suggested resolution:** TCC-root cleanup task (out of scope for spec-critic): the BIBLIOTECARIO agent should catalog these as REF-NNN and update the bridge. SPEC-21 should then cite the assigned REF-NNN identifiers in §4.4 instead of bare paper names. Same scope-handling pattern as SPEC-22 SC-013.

### SC-021 — `chunks_processed` field tracked outside finalize() in RoundRobinStreamingStrategy

**Severity:** LOW
**Axis:** Completeness / Implementability
**Section:** §4.3 finalize() returns `chunks_processed: 0` with comment "tracked externally by pipeline"
**Problem:** The `finalize()` method on the strategy returns `StreamingPartitionStats` including `chunks_processed`, but the round-robin implementation in §4.3 sets the field to 0 with the comment "tracked externally by pipeline." The pipeline itself has no documented protocol for splicing chunks_processed back into the stats before returning to the caller. R20 specifies `ChunkedPartitionResult.stats: StreamingPartitionStats` but does not say who sets the count.
**Impact if unresolved:** Either the value is always 0 in `ChunkedPartitionResult`, or there is an undocumented stitching step. T1 ("verify finalize() statistics match expectations") cannot test the field.
**Suggested resolution:** Either (a) move `chunks_processed` out of `StreamingPartitionStats` into a separate `PipelineStats` set by the pipeline, or (b) require strategies to track chunks via a `record_chunk_complete()` callback. Document the chosen approach in §4.6 pseudocode.

### SC-022 — `WorkerId` type origin not specified

**Severity:** LOW
**Axis:** Consistency / Completeness
**Section:** §4.1, §4.2, R3, R7
**Problem:** `WorkerId` is used throughout (`Vec<(AgentId, WorkerId)>`) but never imported, declared, or cross-referenced to a predecessor spec. The type might be `u32` (per SPEC-04) or a newtype `struct WorkerId(u32)` (per CLAUDE.md "Newtype pattern for IDs"). The spec is silent.
**Impact if unresolved:** Implementer guesses. Trivial to fix.
**Suggested resolution:** Add to §2 Definitions or §4.1: "`WorkerId` is the worker identifier type defined in SPEC-04 §X.Y (or, if not defined there, introduced here as `pub struct WorkerId(pub u32);`)."

### SC-023 — `PortId` arity not bounded

**Severity:** LOW
**Axis:** Consistency
**Section:** §4.1 ConnectionDirective
**Problem:** `ConnectionDirective::Resolved { source: (AgentId, PortId), ... }` uses `PortId` (presumably `u8` or `u32` per SPEC-02). SPEC-02's symbols have arity ∈ {0, 1, 2}, so PortId values are in 0..=2. The spec does not state the bound.
**Impact if unresolved:** Implementer must look up SPEC-02 to know the valid range; trivial.
**Suggested resolution:** One-line note in §4.1: "`PortId` follows SPEC-02 §X with values in 0..=2 (auxiliary ports per the agent's symbol arity)."

### SC-024 — Default chunk_size = 10,000 noted as guess (Q2) — surface in R24

**Severity:** LOW
**Axis:** Testability
**Section:** R24; §8 Q2
**Problem:** R24 specifies the default value as "SHOULD be 10,000 agents" but Q2 acknowledges this is a guess pending benchmarking. The brief notes this is acceptable for Draft. The requirement should be tagged as benchmark-TBD to avoid the default surviving into v2 release without empirical justification.
**Impact if unresolved:** A "default = guess" requirement may persist post-DEV without revisit.
**Suggested resolution:** Reword R24's default clause to: "The default value SHOULD be 10,000 agents pending benchmark calibration (Q2). The default MUST be re-evaluated before v2 release."

---

## Cross-spec consistency audit

The following table enumerates every requirement in SPEC-21 that names another spec's R-number (or refers to another spec by section) and reports the audit verdict.

| SPEC-21 R | References | Verdict | Notes |
|-----------|------------|---------|-------|
| R1 | SPEC-04 R21 (`PartitionStrategy`) | OK | Trait declared as streaming counterpart; consistent. |
| R7 | SPEC-04 R6 (C1) | OK | C1 invocation is precise. |
| R10 | SPEC-09 / `src/bench/mod.rs` `Benchmark` trait | GAP | Trait amendment not in §3.8; SC-008. |
| R15 | SPEC-01 I3 | STALE | I3 was relaxed to I3' by SPEC-22 §3.8 A1; R15 must reference I3'. SC-009. |
| R17 | implies SPEC-04 split() unchanged | OK | Additive design honored. |
| R21 | SPEC-04 `Partition` (R28 split outputs) | OK | Structural compatibility asserted. |
| R23 | SPEC-04 §4.5 Step 5 sparse layout | PARTIAL | References sparse layout but uses dense `Net` — see SC-006 (M5 pathology). |
| R26 | SPEC-04 split() | AMBIGUOUS | Backward-compat scope unclear; SC-014. |
| R27 | SPEC-01 T1, I3, D1, C1, C2, C3 | STALE on I3 | Same as R15; SC-009. |
| R28 | SPEC-04 §4.8 (assert_coverage_and_disjunction, assert_border_consistency) | OK | Assertion delegation is consistent. |
| R29 | SPEC-04 R16-R18 (id ranges) | OK | Verbatim re-use. |
| R30 | implies SPEC-13 coordinator FSM amendment | GAP | FSM not specified; SC-015. |
| R31 | SPEC-06 `Message` enum | GAP | Wire amendment not in §3.8; PROTOCOL_VERSION undecided; SC-001, SC-005. |
| R32 | SPEC-13 coordinator/worker FSM | GAP | FSM not specified; SC-015. |
| R33 | SPEC-01 G1, D1, D5 | PARTIAL | D5 invocation OK; G1 termination semantics under pull dispatch unestablished — SC-019. |
| R33 | SPEC-20 dynamic departure | OK | Re-dispatch pattern reference is valid. |
| R34 | SPEC-07 `GridConfig` | GAP | Config amendment not in §3.8; SC-001. |
| R36 | SPEC-19 delta protocol, BorderGraph | GAP | BorderGraph update protocol unspecified; SC-017. |
| R36 | SPEC-19 not in `Depends on:` frontmatter | GAP | SC-002. |
| §4.8 | SPEC-04 R12 (border-id allocation) | GAP | Implicit amendment; SC-018. |
| §4.9 | SPEC-22 SparseNet (R22) | GAP (silent) | Should adopt SparseNet under threshold; SC-006. |
| §5.1 | DISC-004 v2 §1.6 | OK on content | Citation absent from frontmatter; SC-010. |
| §5.2 | REF-001, REF-002 | OK | Both verified in bridge. |
| §6.2 | SPEC-04 `split()` and `PartitionPlan` | OK | Compatibility bridge code is precise. |

---

## Theory bridge audit

| ID claimed | Bridge entry | Resolves? | Verdict |
|------------|--------------|-----------|---------|
| REF-001 (Lafont 1990) | L159 | YES | OK; supports §5.2 locality. |
| REF-002 (Lafont 1997) | L160 | YES | OK; supports §5.1 confluence. |
| REF-005 (Mackie & Pinto 2002) | L161 | YES | Listed in frontmatter but body usage is thin — body has no explicit REF-005 cite. Soft find: trim or use. (Not severe enough to file separately.) |
| REF-015 (Mackie & Sato 2015) | L170 | PARTIAL | Bridge tags as level-1 (rule-level streaming); SPEC-21 uses for level-3 framing. SC-004. |
| ARG-001 P1-P6 | L18 | YES | OK; supports §5.1 / §5.2. |
| ARG-002 C1-C3 | L27 | YES | OK; supports §5.1. |
| DISC-004 v2 | L96 | YES on content; absent from frontmatter | SC-010. |
| DISC-009 v2 | L124 — primary anchor | NOT cited | SC-003 (HIGH). |
| AC-007 | L204 | NOT cited | SC-011. |
| AC-010 | L209 | NOT cited | SC-011. |
| AC-014 | L218 | NOT cited | SC-011. |
| Tsourakakis 2014 (FENNEL) | NOT in bridge | n/a | SC-020 (LOW; TCC-root cleanup). |
| Stanton & Kliot 2012 (LDG) | NOT in bridge | n/a | SC-020 (LOW; TCC-root cleanup). |

**Bridge audit verdict:** PARTIAL FAIL. Two HIGH (SC-003 missing DISC-009 v2, SC-004 REF-015 mismatch) plus one MEDIUM (SC-011 absent AC anchors) and one LOW (SC-020 unregistered FENNEL/LDG papers).

---

## Invariant audit

| Invariant (SPEC-01) | Threat | Section | Severity |
|---------------------|--------|---------|----------|
| T1 (Port Linearity) | Maintained at every step in §4.6 pseudocode (`install_connection` updates both endpoints). OK. | R27 | none |
| T4 (Strong Confluence) | Untouched — streaming generation does not alter reduction rules. OK. | §5.1 | none |
| T7 (Interaction Counts) | Tested by T7 (end-to-end equivalence). OK. | §7.2 | none |
| I3 → I3' (Monotonicity → Uniqueness) | R15 imposes generator-level batch monotonicity stricter than I3'; reconciliation unstated. | R15, R27 | HIGH (SC-009) |
| D1 (Split/Merge Identity) | Extended to streaming via R27; testable via T6/T7. OK on content. | R27, §7.2 | none |
| D5 (Exclusive Ownership) | R33 invokes for re-dispatch; OK given §6 guidance. | R33 | none |
| C1 (Complete Agent Coverage) | Pipeline tracks via `agent_owner`; assertion in R28 OK. | R7, R27, §5.1 | none |
| C2 (Complete Wire Coverage) | Pending-store empty assertion (R19) is the linchpin; bound on pending store unaddressed (SC-016). | R19, §5.1 | MEDIUM |
| C3 (FreePort Bijectivity) | Border map records both endpoints; OK given §4.6 pseudocode. | R27, §5.1 | none |
| G1 (Sequential Baseline Equivalence) | Streaming + delta + free-list interaction can violate G1 if recycled IDs collide with border-referenced agents. | R15, R23, R36, §4.6 | HIGH (SC-007) |
| G1 under pull dispatch | BSP-barrier semantics under pull mode unestablished. | R32-R35 | MEDIUM (SC-019) |
| M5 memory bound | Dense `Net` accumulator inflates under FENNEL non-contiguous assignment. | §4.9 | HIGH (SC-006) |

---

## Untestability catalog

Requirements that cannot be mechanically verified against the spec as written:

| R | Untestable claim | Why | Linked finding |
|---|------------------|-----|----------------|
| R5 | "FennelStreamingStrategy SHOULD be provided" with `alpha` "SHOULD be configurable" | Two SHOULDs nested; no test covers "must alpha be configurable" vs "alpha exists with any value." | SC-016 / Q3 |
| R6 | "8x memory reduction compared to holding the full net" | Compared against what baseline? No test infrastructure measures the comparison. | (informal — not filed separately) |
| R8 | "deterministic across invocations" | Determinism is testable, but R8 does not specify whether iteration order, hash-map iteration, or thread scheduling is in scope. T1 tests round-robin determinism only; FENNEL uses `HashMap`-based assignment cache whose iteration order is non-deterministic in standard `std::collections::HashMap`. | (related to OQ-A in brief) |
| R10 | `make_net_stream(...) -> Box<dyn Iterator<Item = AgentBatch>>` | Default-impl decision missing — cannot test that "all 13 benchmarks have valid streaming." | SC-008 |
| R22 | "MUST NOT buffer the full stream before partitioning" | "Full stream" is undefined when stream is unbounded. Test T10 measures peak memory but does not directly measure buffering. | (informal) |
| R26 | `chunk_size = u32::MAX` "degenerates to v1 behavior" | "v1 behavior" undefined post-SPEC-22 (see SC-014). | SC-014 |
| R27 (I3 clause) | "I3: AgentId values MUST be monotonically increasing within the stream and within each partition accumulator" | I3 is no longer an invariant; I3' is. Test fails to compile against current SPEC-01. | SC-009 |
| R32 | 7-step pull dispatch protocol | No FSM table — test cannot enumerate all transitions. | SC-015 |
| R36 | "MUST be compatible with the delta protocol" | Compatibility undefined; no `BorderGraph` update protocol. SHOULD-grade. | SC-017 |
| R37 | "SHOULD reduce idle time for heterogeneous workers" | No measurement methodology specified; "higher throughput" undefined. | SC-011 (AC-014 absence) |

---

## Specialist self-flagged zones (RZ-N)

SPEC-21 §8 lists six Open Questions (Q1-Q6). The brief identifies five additional OQs (OQ-A through OQ-E). Verdicts:

| Zone | Source | Disposition |
|------|--------|-------------|
| Q1 — Accumulator memory for large nets | §8 | Cross-cuts SC-006. Q1 should explicitly reference SPEC-22 SparseNet as the in-scope mitigation. |
| Q2 — Optimal chunk_size | §8 | LOW. SC-024 surfaces the requirement to revisit. |
| Q3 — FennelStreamingStrategy alpha tuning | §8 | Acceptable as Open Question for Draft; the contradiction noted in Q3 ("adaptive alpha requires knowing total edges/vertices upfront, which contradicts the streaming model") suggests dropping FENNEL to FUTURE scope or fixing alpha at a benchmarked default. Decision required before DEV begins. |
| Q4 — Interaction with delta protocol | §8 | HIGH. SC-017 escalates. |
| Q5 — Root port handling in streaming | §8 | MEDIUM. R28 debug assertions may fire prematurely if root agent appears mid-stream. Not separately filed; folds into SC-015 / FSM specification. |
| Q6 — Port array sizing in accumulators | §8 | HIGH. Same root cause as SC-006. Q6's resolution should reference SPEC-22 SparseNet. |
| OQ-A (brief) — Determinism of streaming order | brief §6 | Folded into SC-016 / R8 testability. |
| OQ-B (brief) — Backpressure / flow control | brief §6 | SC-012. |
| OQ-C (brief) — Termination signaling in push mode | brief §6 | SC-013. |
| OQ-D (brief) — v1 backward compatibility | brief §6 | SC-014. |
| OQ-E (brief) — Memory bounds on pending store | brief §6 | SC-016. |

---

## Sign-off

**Verdict:** BLOCK — MAJOR REVISION REQUIRED.

**Rationale:** Two CRITICAL findings (SC-001 missing §3.8, SC-002 missing predecessors) make the spec unactionable for the task-splitter. Three independent HIGH findings tied to the just-closed SPEC-22 (SC-006 SparseNet, SC-007 free-list G1 threat, SC-009 I3' reconciliation) demonstrate that SPEC-21 was authored without awareness of SPEC-22's amendments and must be reconciled. SC-005 (PROTOCOL_VERSION) cannot be deferred — it is the third spec in the wave to touch the constant without coordinated sequencing.

**Round 2 entry conditions:**
1. §3.8 Amendments block authored with at minimum 6 entries (per SC-001 suggested resolution).
2. Frontmatter `Depends on:` includes SPEC-06, 07, 09, 17, 18, 19, 22.
3. Frontmatter `Discussions consumed:` includes DISC-009 v2 and DISC-004 v2.
4. PartitionAccumulator (§4.9) cross-references SPEC-22 SparseNet selection.
5. G1 free-list interaction explicitly addressed (new R or §3.5 amendment).
6. PROTOCOL_VERSION disposition declared with sequencing decision coordinated with SPEC-22 / SPEC-20.
7. R10 Benchmark trait amendment specifies default-impl decision.

**Checklist:**

### Consistency
- [ ] All terms match SPEC-00 definitions
- [x] Type signatures compatible with predecessor specs (mostly; `WorkerId` origin SC-022)
- [ ] No contradictions with predecessor requirements (R15 stricter than I3'; SC-009)
- [x] Data flow assumptions match predecessor outputs (SPEC-04 PartitionPlan compatibility OK)

### Testability
- [ ] Every MUST requirement has a testable criterion (R10, R22, R26, R27 untestable as authored)
- [x] Boundary conditions defined (R35 short-stream edge case)
- [x] Error conditions specified (R19 empty pending store)

### Completeness
- [ ] Pseudocode provided for non-trivial operations (§4.6 OK; §3.6 pull dispatch lacks FSM — SC-015)
- [ ] All edge cases documented (Q5 root port, Q6 sparse arena unaddressed)
- [ ] Rust type signatures for all public types/functions (`PortId` origin SC-023, `WorkerId` SC-022)
- [ ] No undefined terms or dangling references (DISC-009 v2 absent, FENNEL/LDG papers unregistered)

### Invariant Preservation
- [x] T1-T7 maintained by all operations (modulo I3 → I3' reconciliation SC-009)
- [ ] D1-D6 maintained by all operations (D1 OK; D5 dispatch under pull mode partial — SC-007)
- [x] I1-I4 maintained (subject to SC-009 / I3 → I3')
- [ ] G1 not violatable by any valid operation sequence (free-list × streaming threat — SC-007; pull-mode BSP barrier — SC-019)

---

**Reviewer:** Spec Critic (adversarial), Round 1
**Date:** 2026-04-25
**Spec status before review:** Draft v1
**Recommended status after revision:** Reviewed v1 (post-revision) → Round 2 by especialista-em-specs.
