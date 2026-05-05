# SPEC-REVIEW-21 — Round 2: Streaming Generation — Closure Pass

**Date:** 2026-04-25
**Author:** especialista-specs (Round 2 closure / defender)
**Target:** `specs/SPEC-21-streaming-generation.md` (Status transitions Draft → Reviewed v2)
**Round 1 baseline:** `docs/spec-reviews/SPEC-REVIEW-21-round-1-2026-04-25.md` — 24 findings (2 C / 7 H / 10 M / 5 L), gate BLOCK.
**Round 1 verdict:** BLOCK — MAJOR REVISION REQUIRED. Closure pass therefore had to be SUBSTANTIAL: every CRITICAL and HIGH addressed inline; MEDIUMs/LOWs deferred only with explicit in-spec gating.
**Predecessors re-consulted:** SPEC-01 (G1, I3'), SPEC-02 (PortRef, AgentId, PortId), SPEC-04 (R12, R16-R18, R21, R22, §4.5 split, §4.8 assertions), SPEC-05 (merge), SPEC-06 (Message enum, PROTOCOL_VERSION), SPEC-07 (GridConfig), SPEC-09 (Benchmark trait, 13 implementations), SPEC-13 (coordinator/worker FSMs), SPEC-17 (transport abstraction), SPEC-18 (wire format v2), SPEC-19 (BorderGraph §3.2), SPEC-22 (SparseNet R22, R10b/R10c protected tombstones, free-list, I3' amendment, PROTOCOL_VERSION 2→3).
**Format reference:** `docs/spec-reviews/SPEC-REVIEW-22-round-2-2026-04-25.md` (per-finding verdict / evidence / diff pointer / gate decision).

---

## Summary

**Gate decision: CONDITIONAL_PASS pending Round 3 spec-critic confirmation.**

| Metric | Value |
|--------|------:|
| Round 1 findings CLOSED inline       | 23 / 24 |
| Round 1 findings DEFERRED with gating | 1 / 24 |
| Round 1 findings NOT_CLOSED          | 0 / 24 |
| NF-NNN (Round 2) CRITICAL            | 0       |
| NF-NNN (Round 2) HIGH                | 0       |
| NF-NNN (Round 2) MEDIUM              | 0       |
| NF-NNN (Round 2) LOW                 | 0       |
| **Total fresh findings**             | **0**   |

The Round 2 closure is comprehensive. Both CRITICAL findings are CLOSED with structural changes:
- §3.8 Amendments to Predecessor Specs section authored, populated with A1-A8 (8 amendments) following the canonical SPEC-22 §3.8 / SPEC-20 §3.8 four-field schema (target / R-number / Old text / New text / Rationale).
- Frontmatter `Depends on:` extended from 5 specs (SPEC-01/02/04/05/13) to 12 (added SPEC-06, SPEC-07, SPEC-09, SPEC-17, SPEC-18, SPEC-19, SPEC-22).

All 7 HIGH findings are CLOSED:
- DISC-009 v2 added to frontmatter `Discussions consumed:` (SC-003); §1 carries the level-3 generation-protocol positioning paragraph.
- REF-015 streaming-level reconciliation in §5.2 (SC-004).
- Predecessors-missing reconciled at the same surface as SC-002 (SC-005).
- §4.9 PartitionAccumulator redesigned around SPEC-22 SparseNet (SC-006).
- §3.7 R37b authored with two strategies plus optional `streaming-no-recycle` cargo gate (SC-007).
- R10 Benchmark trait default-impl decision specified; §3.8 A4 records the SPEC-09 amendment (SC-008).
- §3.7 R37c authored with PROTOCOL_VERSION sequencing decision via defensive `PREVIOUS_LIVE_VERSION + 1` language; §3.8 A2 records the SPEC-06 amendment (SC-009 reading of "PROTOCOL_VERSION undecided", which the Round 1 review refers to as SC-005-style).

10 of 10 MEDIUM findings are CLOSED inline (no MEDIUM deferrals). 4 of 5 LOW findings are CLOSED inline; 1 LOW (SC-020, FENNEL/LDG REF-NNN registration) is DEFERRED with explicit gating because the Round 2 prompt explicitly identifies SC-020 as TCC-root territory and forbids editing `docs/theory-bridge.md` or `biblioteca/referencias.bib` from this Round 2 pass.

No fresh findings (NF-NNN) were introduced by the revision. Specifically, the new amendments do not invoke any predecessor function or signature that does not exist in the predecessor (the Round-1 NF-001-style failure mode SPEC-20 hit). The §3.8 amendments target real R-numbers in real predecessor specs verified at Round 1.

**Recommendation:** Proceed to Stage 1 (TASK-SPLITTER) once spec-critic Round 3 confirms. Round 3 should verify:
1. Every §3.8 amendment names an existing R-number in its target spec (Round 1 already verified this by inspection; Round 3 should re-verify against the new A1-A8 set).
2. The R37b (G1 free-list interaction) gating language under both strategy branches (`DisableUnderDelta` / `BorderClean`) plus the `streaming-no-recycle` cargo feature gate is implementable; particularly that the worker arena can correctly consult `border_referenced_set` at the moment of `free_list.pop()`.
3. The R37c defensive `PREVIOUS_LIVE_VERSION + 1` language is consistent with the SPEC-22 R9a / TASK-0476 wording so that merge-order shuffling among SPEC-20/21/22 produces a coherent absolute version number at landing time.
4. The SPEC-22 SparseNet adoption in §4.9 honors the SPEC-22 R10a/R22 4×-threshold contract at finalize-time (`to_dense(Some(id_range))` rejection path).

---

## Round 1 closure audit

Column key:
- **C** = CLOSED: revision edits demonstrably resolve the finding.
- **D-strong** = DEFERRED with strong rationale (explicit gating mechanism in-spec or in this log).
- **D-weak** = DEFERRED but rationale is handwavy.
- **NC** = NOT_CLOSED despite claimed closure in §11.

### CRITICAL (2)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-001 | **C** | §3.8 Amendments to Predecessor Specs section authored. 8 entries (A1-A8) following the SPEC-22 §3.8 canonical four-field schema. A1 amends SPEC-04 R12 (border-id allocation for streaming pipeline). A2 amends SPEC-06 (Message enum gains `RequestWork`/`NoMoreWork`; PROTOCOL_VERSION bump per R37c). A3 amends SPEC-07 GridConfig (three new fields: `chunk_size`, `streaming_strategy`, `dispatch_mode`; optional fourth `max_pending_lifetime` per R37g). A4 amends SPEC-09 Benchmark trait (default-impl-bearing addition per R10; explicit Phase B effort estimate of ~30 LoC with per-generator overrides on opt-in basis). A5 amends SPEC-13 (coordinator gains 5 new pull-mode states with explicit transition tuples; worker gains 2 new pull-mode states with explicit transition tuples). A6 amends SPEC-22 R10b/R10c (free-list interaction broadened from delta-only to streaming-or-delta scope). A7 amends SPEC-19 §3.2 (`BorderGraph::extend_with_chunk_borders` method addition). A8 amends SPEC-04 §4.5 (split() unchanged; chunked pipeline additive). |
| SC-002 | **C** | Frontmatter `Depends on:` extended from 5 specs (SPEC-01/02/04/05/13) to 12 specs. Added: SPEC-06 (Wire Protocol — Message enum amended via §3.8 A2), SPEC-07 (GridConfig amended via §3.8 A3), SPEC-09 (Benchmark trait amended via §3.8 A4), SPEC-13 (FSMs amended via §3.8 A5), SPEC-17 (Transport Layer — pull-protocol round-trip over `ChannelTransport` for tests), SPEC-18 (Wire Format v2 — serde of new variants), SPEC-19 (Delta Protocol — `BorderGraph` interaction, R36 / §3.7 cross-references, amended via §3.8 A7), SPEC-22 (Arena Management — SparseNet for PartitionAccumulator, protected tombstones for streaming border safety, I3' relaxation cross-checked at §3.5; amended via §3.8 A6). Each addition carries a parenthetical justification inline. |

### HIGH (7)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-003 | **C** | Frontmatter `Discussions consumed:` line added with `DISC-009 v2` listed as the primary taxonomy anchor (and DISC-004 v2 also, closing SC-010 in the same edit). §1 carries a one-paragraph cross-reference placing SPEC-21 at DISC-009 v2's level-3 generation-protocol streaming layer ("SPEC-21 covers DISC-009 v2's generation-protocol streaming level (level 3); the pull-based dispatch in §3.6 corresponds to the on-demand operating mode in the same taxonomy"). §3.6 prose cites DISC-009 v2 implicitly via the section's amendment framing. |
| SC-004 | **C** | §5.2 carries the REF-015 streaming-level reconciliation paragraph: REF-015 (Mackie & Sato 2015) establishes streaming at level 1 (rule-level) per `docs/theory-bridge.md` L170; SPEC-21 generalizes to level 3 (generation-protocol) per DISC-009 v2; REF-015 is retained as the closest published precedent at the next-lower streaming level, NOT as direct evidence for the level-3 pipeline architecture. Frontmatter REF-015 entry annotated to point at §5.2 for the reconciliation. |
| SC-005 | **C** | The Round 1 finding SC-005 is a duplicate surface of SC-002 (predecessors missing from `Depends on:`); both surfaces are reconciled by the frontmatter extension under SC-002. The Round 1 review §Mandatory list explicitly notes this overlap. No separate edit is required beyond the frontmatter extension. The R37c PROTOCOL_VERSION sequencing decision (which the Round 1 review also tags as the "SC-005" finding under the body §"Findings — HIGH" enumeration) is closed at §3.7 R37c with defensive `PREVIOUS_LIVE_VERSION + 1` language and §3.8 A2 documenting the bump. |
| SC-006 | **C** | §4.9 PartitionAccumulator redesigned around SPEC-22 SparseNet (R22). New `AccumulatorNet { Sparse(SparseNet), Dense(Net) }` enum; defaults to SparseNet at construction (`Self { subnet: AccumulatorNet::Sparse(SparseNet::new()), ... }`); finalizes to dense `Net` via `to_dense(Some(id_range.as_range()))` (SPEC-22 §4.6 signature) only at pipeline end. The `id_range > 4 × live_agent_count` threshold contract is enforced at finalize-time per SPEC-22 R10a/R22; dense path rejected with `PartitionError::DenseAllocationExceedsThreshold` (SPEC-22 R30). The §4.9 introductory paragraph explicitly documents that SC-006 is closed by this design and that R23's "MUST be sized to `max_agent_id_in_this_worker + 1`" applies to the dense-finalized form, NOT the in-progress SparseNet accumulator. AC-010 (HVM4 WNF Evaluation) cross-referenced for the frame-reuse pattern. §3.8 A6 records the SPEC-22 interaction. |
| SC-007 | **C** | §3.7 R37b authored: streaming pipeline MUST honor SPEC-22 R10b/R10c protected-tombstone discipline for any AgentId in `border_map` or pending-connection store, regardless of `delta_mode`. Two strategies preserved: Strategy A (`DisableUnderDelta`, default — workers MUST NOT pop from free-list while delta mode active and chunked dispatch in progress) and Strategy B (`BorderClean`, opt-in — workers MAY pop only for IDs not in locally-cached border_entries set). Alternative one-liner closure: cargo feature gate `streaming-no-recycle` to disable free-list outright during streaming. The G1 violation scenario (recycled slot ID assigned to NEW agent while still-pending border wire references OLD agent at that slot) is explicitly described in R37b's closing paragraph. §3.8 A6 records the SPEC-22 R10b conditional broadening (delta-only → delta-or-streaming). |
| SC-008 | **C** | R10 amended to specify a default implementation of `make_net_stream` that wraps `self.make_net(size)` via the `default_chunked_iter` helper (closure of SC-008 explicit). The default-impl path materializes the net then slices it into chunks (memory-equivalent to v1; no streaming benefit, but no break). All 13 SPEC-09 implementations remain valid without per-implementation edits. R11 reframed as the source-of-truth materialization path with explicit relationship documented ("R11 is the source-of-truth materialization path; R10 either (a) wraps R11 via the default impl (memory-equivalent to v1) or (b) is overridden by the implementor for true bounded-memory streaming"). §3.8 A4 records the SPEC-09 amendment with explicit Phase B effort estimate (~30 LoC trait amendment + per-generator overrides on opt-in basis vs ~520 LoC mechanical implementation). |
| SC-009 | **C** | R15 amended to explicitly note the I3'/R15 reconciliation: R15 is a generator-phase contract strictly stronger than SPEC-01 I3' (post-SPEC-22 §3.8 A1). R27's I3 clause replaced with I3' clause referencing SPEC-22 §3.8 A1 and SPEC-02 R10 / SPEC-22 §3.8 A3. Closing-note paragraph at end of §3.5 documents the formal reconciliation: R15 applies to the generation phase only; once dispatched, the worker arena MAY recycle slot IDs per I3' / SPEC-22 R1-R10c; `src/partition/streaming.rs` MUST NOT assume monotonicity post-dispatch (e.g., MUST NOT write `assert!(new_id > old_max_id)` patterns; cross-references SPEC-22 §3.8 A6 forbidden assertion list). |

### MEDIUM (10)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-010 | **C** | Closed in the same frontmatter edit as SC-003: `Discussions consumed:` line lists both DISC-004 v2 and DISC-009 v2. §4.2 trait doc-comment continues to cite DISC-004 v2 §1.6 for the partition-quality independence argument. §5.1 implicit reference via ARG-002 Passo 10 preserved. |
| SC-011 | **C** | Frontmatter `Code analyses consumed:` line added with AC-007 (HVM2 Reduction Engine — informs §4.6 `install_connection` border detection during the streaming loop), AC-010 (HVM4 WNF Evaluation — informs §4.9 `PartitionAccumulator` frame-reuse pattern), AC-014 (Bench Methodology — canonical methodology reference for §7.4 T10 peak-memory measurement). §4.6 introductory paragraph cites AC-007 explicitly ("The on-the-fly border detection in `install_connection` (below) follows the AC-007 (HVM2 Reduction Engine) pattern: detect cross-partition pairs at the moment of connection, not in a separate pass"). §4.9 introductory paragraph cites AC-010. R37 cites AC-014 for the throughput methodology ("methodology for 'higher throughput' measurement follows AC-014"). |
| SC-012 | **C** | R16 closing sentence added: the iterator's pull-based `Iterator::next` interface naturally supports the pull dispatch model (R32) without async coordination — the coordinator drives the iterator one `next()` call per `RequestWork` message. Async channels (`tokio::sync::mpsc`) are required only for the push dispatch model when generation and dispatch overlap in time. The reconciliation explicitly closes the apparent R16 vs R32 tension. |
| SC-013 | **C** | §3.7 R37e authored: in push mode (default for `num_workers ≤ 2` under `DispatchMode::Auto`), no `NoMoreWork` is sent; the worker receives a single `AssignPartition` per SPEC-05 merge protocol. `NoMoreWork` is meaningful only in pull mode (R31). Worker FSM and coordinator FSM MUST NOT cross-pollute the variant. The two modes share the variant in the wire format (so version sequencing in R37c is single-mode-agnostic) but the protocol is mode-specific. |
| SC-014 | **C** | R26 reworded with explicit closure: "When `chunk_size` is set to `u32::MAX` (or a sentinel value indicating 'no chunking'), the pipeline MUST short-circuit to SPEC-04 `split()` after collecting the full stream into a single `Net` via the R10 default-impl path. The merge result MUST be **isomorphic** (SPEC-00 §6.12, `nets_isomorphic`) to the v1 `split()`-produced result; bit-identical layout is NOT guaranteed because of SPEC-22 arena-management amendments (free-list, SparseNet, `freeport_redirects` propagation). T6 (§7.2) measures isomorphism, not byte-equality (closes SC-014)." |
| SC-015 | **C** | §3.8 A5 authored. Coordinator FSM gains 5 new states with explicit transition tuples (`Init → DispatchingFirst`, `DispatchingFirst → AwaitingResults`, `AwaitingResults + RequestWork → GeneratingNext` (if stream not exhausted) or `SendingNoMoreWork` (if exhausted), `GeneratingNext + chunk-ready → AwaitingResults` (after sending `AssignPartition`), `SendingNoMoreWork + all-acks → AwaitingFinalResults`, `AwaitingFinalResults + all-results → Merge` (BSP barrier per R37d)). Worker FSM gains 2 new states with explicit transition tuples (`ReducingChunk + chunk-done → AwaitingChunkAfterResult` (also emits `RequestWork`), `AwaitingChunkAfterResult + AssignPartition → ReducingChunk`, `AwaitingChunkAfterResult + NoMoreWork → FinalReduction`, `FinalReduction + reduction-done → SendFinalResult → Done`). The push-mode FSMs are unchanged; pull-only states gated on `DispatchMode::Pull` in `GridConfig`. |
| SC-016 | **C** | §3.7 R37g authored: pending-store memory bound `MAX_PENDING_LIFETIME` (default 16, configurable via `GridConfig.max_pending_lifetime`). Generators MUST resolve any forward reference within at most `MAX_PENDING_LIFETIME` chunks. The pipeline MAY enforce this as a `debug_assert!` that fires when `pending` retains an entry across more than `MAX_PENDING_LIFETIME` chunk boundaries. Generators violating the bound MUST be either (a) refactored to emit forward-referenced agents earlier, or (b) explicitly excluded from streaming mode and forced to use the R10 default-impl materialization path. Bounds pending-store peak memory at O(`MAX_PENDING_LIFETIME` × max_forward_refs_per_chunk). |
| SC-017 | **C** | §3.7 R37f authored: under the conjunction `delta_mode && streaming_active`, R36 elevates from SHOULD to MUST and the coordinator MUST call `BorderGraph::extend_with_chunk_borders(&new_borders)` after each `install_connection` invocation that yields a border wire, before chunk N+1's `AssignPartition` is dispatched. Failure means the coordinator's border-redex detection misses cross-chunk active pairs and the M5 milestone target ("ep_con 100M coordinator-side") is unreachable. SPEC-19 owns the implementation; SPEC-21 owns the call-site discipline. §3.8 A7 records the SPEC-19 amendment with the new method signature `pub fn extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)`. |
| SC-018 | **C** | §3.5 R29b authored: §4.8's allocation policy promoted to a numbered requirement. §3.8 A1 records the SPEC-04 R12 amendment with verbatim Old text ("Border IDs start at `max_existing_freeport_id(net) + 1`") and verbatim New text ("Border IDs follow SPEC-04 R12 in the batch path (`split()`-based, when `chunk_size = u32::MAX`). In the streaming path (`generate_and_partition_chunked`), border IDs MUST start at 0 and increment monotonically when no Lafont FreePorts are present in any batch, OR at `max_lafont_freeport_id_in_first_batch + 1` when the first batch carries Lafont FreePorts"). |
| SC-019 | **C** | §3.7 R37d authored: the BSP barrier under pull dispatch is the moment all workers acknowledge `NoMoreWork`. Before this moment, individual workers MAY complete reductions on their accumulated chunks and MAY emit `PartitionResult` messages, but MUST NOT begin the merge phase. Workers MUST wait for `NoMoreWork` before transitioning to the final-reduction state. This preserves G1 by reducing pull dispatch to a single "logical BSP round" regardless of wall-clock interleaving — the pull pattern shifts the timing of `AssignPartition` messages but does not introduce new barrier semantics relative to push mode. Cross-reference: SPEC-13 worker FSM (amended via §3.8 A5). |

### LOW (5)

| ID | Verdict | Evidence / Diff pointer |
|----|:------:|------------------------|
| SC-020 | **D-strong** | Tsourakakis 2014 (FENNEL) and Stanton & Kliot 2012 (LDG) are cited inline in §4.4 but absent from `biblioteca/referencias.bib` and `docs/theory-bridge.md`. Per the Round 2 prompt, theory-bridge.md is TCC-root territory and out of scope for SPEC-21 author. The FennelStreamingStrategy doc-comment now annotates these as "REF-TBD (TCC-root cleanup; not yet registered in `docs/theory-bridge.md`; see §11 Change Log)" so the obligation is auditable. The bibliography registration will be picked up by the BIBLIOTECARIO agent at the next theory-bridge maintenance pass. Same scope-handling pattern as SPEC-22 SC-013. The deferral is gated by §11 Change Log acknowledgement and the in-spec annotation. |
| SC-021 | **C** | §4.3 `RoundRobinStreamingStrategy::finalize()` returns `chunks_processed: 0` with explicit comment that the field is pipeline-owned. §4.6 pipeline pseudocode adds a `chunks_seen: u64` counter incremented per iteration; Step 7 of the pseudocode stitches `result.stats.chunks_processed = chunks_seen` after `strategy.finalize()`. T1 in §7.1 verifies the pipeline-stitched count, not the strategy-returned value. A note paragraph between §4.3 and §4.4 documents the ownership convention explicitly ("The strategy returns `chunks_processed: 0` as a placeholder; the pipeline owns the field and stitches the actual count into the returned `ChunkedPartitionResult.stats` before returning to the caller"). |
| SC-022 | **C** | §4.1 opening "Type origins" paragraph documents `WorkerId` origin: newtype `pub struct WorkerId(pub u32)` defined in SPEC-04 (matches CLAUDE.md "Newtype pattern for IDs"); SPEC-21 imports it from `crate::partition::WorkerId` and does not redeclare it. |
| SC-023 | **C** | Same §4.1 opening "Type origins" paragraph documents `PortId` origin: defined in SPEC-02 as `u8` (or `u32` per the live code) with values bounded to `0..=2` corresponding to the principal port (0) and at most two auxiliary ports (1, 2) per the agent's symbol arity (ERA: 0 aux; CON/DUP: 2 aux). |
| SC-024 | **C** | R24 default-clause reworded: "The `chunk_size` parameter MUST be configurable via `GridConfig` (SPEC-07). The default value SHOULD be 10,000 agents pending benchmark calibration (Q2). The default MUST be re-evaluated and either confirmed or replaced before v2 release (closes SC-024). **(MUST for configurability; SHOULD for the placeholder default)**". Tags the placeholder as benchmark-TBD so it does not survive into v2 release without empirical justification. |

### Closure-audit summary

- **23 genuinely CLOSED** by substantive edits.
- **1 DEFERRED with strong rationale**: SC-020 (FENNEL/LDG REF-NNN registration, downstream of SPEC-21's territory; explicit gating via the §11 Change Log acknowledgement, the §4.4 doc-comment "REF-TBD" annotation, and the Round 2 prompt's territorial scoping). Round 1 itself flagged SC-020 as "TCC-root cleanup, not SPEC-21 fault"; this Round 2 honors that scoping.
- **0 NOT_CLOSED**: every CRITICAL and HIGH from Round 1 has either an inline edit or a structured amendment in §3.8.

No findings are falsely claimed closed. The §11 Change Log is substantively accurate; the per-finding row in §11 maps 1:1 to the verdicts in this log.

---

## Fresh findings (Round 2)

**None.**

Sanity audit performed:
- §3.8 amendments target real R-numbers in real predecessor specs. SPEC-04 R12 verified. SPEC-06 Message enum verified (variant set listed in Old text). SPEC-07 GridConfig verified (struct exists with prior fields). SPEC-09 Benchmark trait `make_net` (R2) verified. SPEC-13 coordinator/worker FSM existence verified. SPEC-22 R10b/R10c verified at SPEC-22 §3.1 (just-closed Round 2 §3.8 A10). SPEC-19 §3.2 BorderGraph verified. SPEC-04 §4.5 split() verified.
- The `extend_with_chunk_borders` method (§3.8 A7) is a NEW method on SPEC-19's `BorderGraph`. The signature is precisely specified (`pub fn extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)`) with idempotency and no-op semantics documented. SPEC-19 owns the implementation; this is a SPEC-19 amendment via SPEC-21 §3.8 A7. Consistent with SPEC-22's pattern of authoring new methods on predecessor types via §3.8 amendments (e.g., SPEC-22 A10 amended SPEC-19's `BorderGraph` contract with a new constraint).
- The `RecyclePolicy::DisableUnderDelta` / `BorderClean` enum variants are SPEC-22-defined; the SPEC-21 R37b text reuses them by name (does not re-declare). Wire-level enum name preserved per §3.8 A6 ("the field name is misleading post-SPEC-21 but stable").
- The `streaming-no-recycle` cargo feature gate is a SPEC-21-introduced compile-time feature. Adding a cargo feature does not require a predecessor amendment (`Cargo.toml` is project-level, not spec-owned).
- The `MAX_PENDING_LIFETIME` constant and `GridConfig.max_pending_lifetime` field (R37g) are SPEC-21-defined. The optional `GridConfig` extension is acknowledged in §3.8 A3 as the optional fourth field.
- The `default_chunked_iter` helper (R10 default impl) is a SPEC-21-defined helper located in `src/bench/streaming.rs` (or `src/io/streaming.rs`). The helper signature is precisely specified.
- The `AccumulatorNet { Sparse(SparseNet), Dense(Net) }` enum is SPEC-21-defined. SparseNet is the SPEC-22-defined type; Net is the SPEC-02 / SPEC-22-amended type. Both reside in `crate::net::*`. No NF.
- The `PartitionError::DenseAllocationExceedsThreshold` error variant is SPEC-22-defined (via SPEC-22 R30); SPEC-21 references it by name. No NF.
- The `Net::is_behaviorally_equal` helper used in T6 / T7 isomorphism tests is SPEC-22-defined (via SPEC-22 R21). No NF.

No claim in the revised SPEC-21 invokes a predecessor function or signature that does not exist in the predecessor. The Round-1 NF-001 failure mode (SPEC-20's `Net::union` issue) does NOT recur in this closure.

---

## Cross-spec consistency re-audit

Re-verification of every cross-spec reference in the revised SPEC-21:

| SPEC-21 ref | Target | Verdict |
|-------------|--------|---------|
| R1 | SPEC-04 R21 (`PartitionStrategy`) | OK — trait declared as streaming counterpart. |
| R7 | SPEC-04 R6 (C1) | OK. |
| R10 | SPEC-09 / `src/bench/mod.rs` `Benchmark` trait | OK — default-impl decision specified; §3.8 A4 records amendment. |
| R15 | SPEC-01 I3' (post-SPEC-22 §3.8 A1) | OK — explicit reconciliation note in R15 closing sentence and §3.5 closing note. |
| R17 | SPEC-04 split() unchanged | OK — additive design honored; §3.8 A8 records the explicit "split() unchanged" clarification. |
| R21 | SPEC-04 `Partition` (R28 split outputs) | OK. |
| R23 | SPEC-04 §4.5 + SPEC-22 R22 SparseNet | OK — §4.9 design explicitly notes R23 applies to dense-finalized form, NOT the in-progress SparseNet accumulator. |
| R26 | SPEC-04 split() | OK — isomorphism-not-bit-identity clarified. |
| R27 (I3' clause) | SPEC-01 I3' + SPEC-02 R10 + SPEC-22 §3.8 A3 | OK. |
| R28 | SPEC-04 §4.8 (assert_coverage_and_disjunction, assert_border_consistency) | OK. |
| R29 | SPEC-04 R16-R18 | OK — verbatim re-use. |
| R29b | SPEC-04 R12 (border-id allocation) | OK — promoted to numbered requirement; §3.8 A1 records amendment. |
| R30 | SPEC-13 coordinator FSM | OK — §3.8 A5 specifies new states with transition tuples. |
| R31 | SPEC-06 `Message` enum + PROTOCOL_VERSION | OK — §3.8 A2 records amendment; R37c records PROTOCOL_VERSION sequencing decision. |
| R32 | SPEC-13 coordinator/worker FSM | OK — §3.8 A5 specifies. |
| R33 | SPEC-01 G1, D1, D5; SPEC-20 dynamic departure | OK; G1 termination semantics under pull dispatch closed by R37d. |
| R34 | SPEC-07 `GridConfig` | OK — §3.8 A3 records amendment. |
| R36 | SPEC-19 delta protocol, BorderGraph | OK — elevated to MUST under `delta_mode && streaming_active` per R37f; §3.8 A7 records SPEC-19 amendment. |
| R37b | SPEC-22 R10b/R10c protected tombstones | OK — §3.8 A6 records the broadening from delta-only to delta-or-streaming. |
| R37c | SPEC-06 PROTOCOL_VERSION + SPEC-18 R28 + SPEC-22 R9a / §3.8 A9 | OK — defensive `PREVIOUS_LIVE_VERSION + 1` language matches SPEC-22 TASK-0476 / R9a. |
| R37d | SPEC-13 worker FSM (per §3.8 A5) | OK — BSP-barrier semantics under pull dispatch documented. |
| R37e | SPEC-05 merge protocol | OK — push-mode termination scoping documented. |
| R37f | SPEC-19 §3.2 (`BorderGraph::extend_with_chunk_borders`) | OK — §3.8 A7 records SPEC-19 amendment with new method signature. |
| R37g | SPEC-21-internal (`MAX_PENDING_LIFETIME`, `GridConfig.max_pending_lifetime`) | OK — bounds pending-store memory at O(`MAX_PENDING_LIFETIME` × max_forward_refs_per_chunk). |
| §4.8 / R29b | SPEC-04 R12 (border-id allocation) | OK — §3.8 A1 records amendment. |
| §4.9 | SPEC-22 R22 SparseNet, R10a/R30 threshold rejection | OK — design explicitly adopts SparseNet, finalizes via `to_dense(Some(id_range))`. |
| §5.1 | DISC-004 v2 §1.6, ARG-002 Passo 10 | OK — frontmatter `Discussions consumed:` now lists DISC-004 v2. |
| §5.2 | REF-001, REF-002, REF-015 (level reconciliation) | OK — REF-015 streaming-level reconciliation paragraph added. |
| §6.2 | SPEC-04 `split()` and `PartitionPlan` | OK — compatibility bridge code precise. |

**Summary:** 0 contradictions, 0 amendments-needed-but-not-written, 0 frontmatter omissions. All cross-spec references are now backed by structured §3.8 amendment entries.

---

## Theory-bridge audit

Every ARG/DISC/REF/AC ID cited in the revised SPEC-21 was checked against `docs/theory-bridge.md`:

| SPEC-21 citation | Where in SPEC-21 | Resolves in bridge? | Notes |
|------------------|------------------|---------------------|-------|
| REF-001 (Lafont 1990) | Frontmatter + §5.2 | YES (line 159) | Unchanged. |
| REF-002 (Lafont 1997) | Frontmatter + §5.1 + §5.2 | YES (line 160) | Unchanged. |
| REF-005 (Mackie & Pinto 2002) | Frontmatter | YES (line 161) | Soft note — body usage thin, kept for context. |
| REF-015 (Mackie & Sato 2015) | Frontmatter + §5.2 | YES (line 170) | Streaming-level mismatch reconciled in §5.2 — REF-015 is level 1 (rule-level), SPEC-21 is level 3 (generation-protocol); REF-015 retained as next-lower-level precedent. |
| ARG-001 (P1-P6) | Frontmatter + §5.1 + §5.2 | YES (line 18) | Unchanged. |
| ARG-002 (C1-C3) | Frontmatter + §5.1 | YES (line 27) | Unchanged. |
| DISC-004 v2 | Frontmatter + §4.2 + §5.1 | YES (line 96) | NEW in frontmatter (closes SC-010). |
| DISC-009 v2 | Frontmatter + §1 + §3.6 (implicit) | YES (line 124) | NEW (closes SC-003). |
| AC-007 (HVM2 Reduction Engine) | Frontmatter + §4.6 | YES (line 204) | NEW (closes SC-011). |
| AC-010 (HVM4 WNF Evaluation) | Frontmatter + §4.9 | YES (line 209) | NEW (closes SC-011). |
| AC-014 (Bench Methodology) | Frontmatter + R37 | YES (line 218) | NEW (closes SC-011). |
| Tsourakakis 2014 (FENNEL) | §4.4 (REF-TBD) | NOT in bridge | TCC-root cleanup; SC-020 deferred. |
| Stanton & Kliot 2012 (LDG) | §4.4 (REF-TBD) | NOT in bridge | TCC-root cleanup; SC-020 deferred. |

**Downstream theory-bridge cleanup item:** FENNEL (Tsourakakis 2014) and LDG (Stanton & Kliot 2012) need REF-NNN registration in `docs/theory-bridge.md` and bibtex entries in `biblioteca/referencias.bib`. Per the Round 2 prompt, this is TCC-root territory and NOT SPEC-21 author scope. Acknowledged in §11 Change Log; the bridge edits are deferred to the BIBLIOTECARIO agent. Same handling pattern as SPEC-22 SC-013 (DISC-012 stale tag).

**Net theory-bridge audit verdict:** clean. Frontmatter now matches body usage. REF-015 level mismatch reconciled honestly. FENNEL/LDG deferral is the bridge maintainer's responsibility, not SPEC-21's.

---

## Invariant audit (post-revision)

**T-layer (theoretical, T1-T7):** No changes from Round 1. All preserved. **No threat.**

**D-layer (distributed):**
- **D1 (Split/Merge Identity):** Now PROTECTED. R27 + §3.5 D1-extended clause covers streaming pipeline isomorphism contract.
- **D2-D3 (Border completeness, Cross-round border discovery):** Now PROTECTED under streaming+delta. SC-017 closed by R37f + §3.8 A7 (`BorderGraph::extend_with_chunk_borders` discipline).
- **D5 (Exclusive Ownership):** Now PROTECTED. R33 invokes for re-dispatch under pull dispatch (same pattern as SPEC-20 dynamic departure).
- **D6 (Protocol termination):** Now PROTECTED under pull dispatch. R37d documents BSP-barrier semantics.

**I-layer:** All preserved. I3 → I3' reconciliation explicit at R15 + R27 + §3.5 closing note (SC-009 closed).

**C-layer:**
- **C1-C3:** Preserved. R28 debug assertions (SPEC-04 §4.8 `assert_coverage_and_disjunction`, `assert_border_consistency`) operate on finalized output.
- **C2 pending-store bound:** SC-016 closed by R37g (`MAX_PENDING_LIFETIME` configurable bound).

**G-layer:**
- **G1:** No longer multi-vector threatened. SC-007 closed by R37b (free-list × streaming protected-tombstone discipline). SC-019 closed by R37d (BSP-barrier under pull dispatch). G1 is CONDITIONAL on SPEC-22 R10b under streaming+delta mode, mirroring the SPEC-22 / SPEC-19 / SPEC-20 conditional gating pattern. Explicit in R37b's two-strategy formulation.
- **M5 memory bound:** SC-006 closed by SparseNet adoption in §4.9. The dense-arena inflation pathology under FENNEL non-contiguous assignment is eliminated. T10 (peak memory measurement) is achievable under streaming+SparseNet+`to_dense(Some(id_range))`.

**Summary:** SPEC-21 amends I3 → I3' (now formal §3.8 A6 cross-reference to SPEC-22 §3.8 A1) and additionally conditionally extends D1/D2/D3 contracts via R37b/R37f. All amendments are now structured §3.8 entries. The "5 invariants touched without acknowledgement" Round 1 critique is resolved: each touch has a structured amendment.

---

## Untestability catalog (post-revision)

| Req | Untestability reason | Severity | Resolution status |
|-----|---------------------|----------|-------------------|
| R5 | Two SHOULDs nested (FENNEL provided + alpha configurable) | RESOLVED | Q3 acknowledged with fixed-default disposition; per-benchmark alpha calibration is a separate task per AC-014 methodology. |
| R6 | "8x memory reduction" baseline-comparison | RESOLVED | Comparison baseline is the v1 dense `Net` (~64 bytes per agent) vs the FENNEL `assignment_cache` HashMap entry (~8 bytes per agent). Methodology per AC-014. |
| R8 | "deterministic across invocations" iteration-order ambiguity | ACCEPTED | T1 tests round-robin determinism (deterministic by construction). FENNEL `HashMap`-based assignment cache iteration order is not a concern because `allocate_batch` does NOT iterate the cache — it reads from it via key lookup. The non-determinism risk is only if `make_net_stream` itself produces non-deterministic batch order; documented in OQ-A as a generator obligation. |
| R10 | `make_net_stream(...)` default-impl ambiguity | RESOLVED (SC-008) | Default impl specified explicitly. |
| R22 | "MUST NOT buffer the full stream" | ACCEPTED | T10 (§7.4) measures peak memory; "MUST NOT buffer" is a structural property verified at code-review time (the pipeline does not collect the iterator before partitioning). |
| R26 | `chunk_size = u32::MAX` "v1 behavior" definition | RESOLVED (SC-014) | Isomorphism (not bit-identity) clarified. |
| R27 (I3' clause) | I3' invariant compatibility | RESOLVED (SC-009) | I3' clause replaces I3 clause; closing note documents reconciliation. |
| R32 | 7-step pull dispatch protocol | RESOLVED (SC-015) | §3.8 A5 specifies FSM transition tuples for both coordinator and worker. |
| R36 | "MUST be compatible with delta protocol" | RESOLVED (SC-017) | Elevated to MUST under conjunction `delta_mode && streaming_active`; `BorderGraph::extend_with_chunk_borders` discipline specified. |
| R37 | "SHOULD reduce idle time for heterogeneous workers" | ACCEPTED | Methodology cites AC-014 (`std::time::Instant` wall-clock with warmup discard, statistical methodology per SPEC-09 §3.5). T14 simulates heterogeneous workers. |

---

## Specialist self-flagged zones

§8 Open Questions audit:
- **Q1** — PARTIALLY RESOLVED via SPEC-22 SparseNet adoption (closes the FENNEL-pathology aspect of SC-006); residual O(N/K) per-worker growth deferred to ROADMAP 2.19 / SPEC-25.
- **Q2** — ACCEPT defer (placeholder; R24 mandates re-evaluation before v2 release per SC-024).
- **Q3** — Fixed-default disposition (`alpha = 1.0`); per-benchmark calibration a separate task; if calibration shows fixed-default materially worse than batch FENNEL on representative benchmarks, FENNEL drops to FUTURE scope. Decision documented in §8 Q3.
- **Q4** — RESOLVED via R37b + R37f + §3.8 A6/A7 (closes SC-007 + SC-017).
- **Q5** — Acknowledged as non-issue (R28 assertions operate on finalized output, not intermediate states).
- **Q6** — RESOLVED via SPEC-22 SparseNet adoption in §4.9 (closes SC-006).
- **OQ-A (brief) — Determinism of streaming order:** The R8 testability discussion documents this. R8 covers strategy determinism; generator determinism is the generator's obligation. Documented as a generator-side obligation in R10 prose.
- **OQ-B (brief) — Backpressure / flow control:** RESOLVED (SC-012, R16 closing sentence).
- **OQ-C (brief) — Termination signaling in push mode:** RESOLVED (SC-013, R37e).
- **OQ-D (brief) — v1 backward compatibility:** RESOLVED (SC-014, R26 reword).
- **OQ-E (brief) — Memory bounds on pending store:** RESOLVED (SC-016, R37g).

No remaining "Decision deferred to implementation" tags in §8. All deferrals are either to a separate spec (SPEC-22 / SPEC-25 for residual memory growth) or to a different agent's territory (BIBLIOTECARIO for SC-020 FENNEL/LDG REF-NNN registration).

---

## Mandatory vs Recommended (Round 2)

**MANDATORY (Round 1 list):** All 9 mandatory items CLOSED inline.

- SC-001 — CLOSED (§3.8 A1-A8).
- SC-002 — CLOSED (frontmatter `Depends on:` extended to 12 specs).
- SC-003 — CLOSED (frontmatter `Discussions consumed:` adds DISC-009 v2).
- SC-004 — CLOSED (§5.2 REF-015 streaming-level reconciliation).
- SC-005 — CLOSED (overlapping surface with SC-002; reconciled via frontmatter extension; PROTOCOL_VERSION sequencing closed at R37c).
- SC-006 — CLOSED (§4.9 SparseNet adoption).
- SC-007 — CLOSED (R37b + §3.8 A6 + optional `streaming-no-recycle` cargo gate).
- SC-008 — CLOSED (R10 default-impl + §3.8 A4).
- SC-009 — CLOSED (R15 + R27 + §3.5 closing note + R37c PROTOCOL_VERSION sequencing).

**RECOMMENDED (Round 1 list):** 14 of 15 CLOSED inline; 1 DEFERRED (SC-020, TCC-root cleanup territory).

---

## Checklist

### Consistency
- [x] All terms match SPEC-00 definitions.
- [x] Type signatures compatible with predecessor specs (`WorkerId` and `PortId` origins documented in §4.1).
- [x] No contradictions with predecessor requirements (R15 stricter than I3', explicitly reconciled; §3.8 A1-A8 records every amendment with structured Old/New text).
- [x] Data flow assumptions match predecessor outputs (SPEC-04 `PartitionPlan` compatibility OK; `ChunkedPartitionResult` convertibility documented in §6.2).

### Testability
- [x] Every MUST requirement has a testable criterion (R10 default-impl, R22 structural property, R26 isomorphism, R27 I3'-clause all resolved).
- [x] Boundary conditions defined (R35 short-stream edge case; T13 short-stream test).
- [x] Error conditions specified (R19 empty pending store; R37g pending-store bound debug-assert).

### Completeness
- [x] Pseudocode provided for non-trivial operations (§4.6 install_connection + chunked pipeline pseudocode; §3.8 A5 FSM transition tuples).
- [x] All edge cases documented (Q5 root port acknowledged; Q6 sparse arena resolved via SparseNet adoption).
- [x] Rust type signatures for all public types/functions (`WorkerId`, `PortId` origins documented; `extend_with_chunk_borders` signature in §3.8 A7).
- [x] No undefined terms or dangling references (DISC-009 v2 cited; FENNEL/LDG annotated as REF-TBD with TCC-root cleanup obligation).

### Invariant Preservation
- [x] T1-T7 maintained.
- [x] D1-D6 maintained (D1 D5 OK; D2/D3 protected under streaming+delta via R37f; SC-007 closed for free-list × streaming).
- [x] I1-I4 maintained (I3 → I3' explicitly reconciled).
- [x] G1 not violatable by any valid operation sequence (CONDITIONAL on SPEC-22 R10b under streaming+delta mode, mirroring SPEC-22 / SPEC-19 / SPEC-20 conditional gating; explicit in R37b's two-strategy formulation).

---

## Verdict

**CONDITIONAL_PASS pending Round 3 spec-critic confirmation.**

Round 2 closure is SUBSTANTIAL: 23/24 findings CLOSED inline; 1 finding (SC-020) DEFERRED to the correct territory (TCC-root cleanup, BIBLIOTECARIO scope) with explicit gating; 0 NOT_CLOSED; 0 fresh NF-NNN findings. The spec is implementable as-is; the residual obligation is the FENNEL/LDG REF-NNN registration in `docs/theory-bridge.md` and `biblioteca/referencias.bib`, which is not blocking and is owned by a different agent.

Stage 1 (TASK-SPLITTER) and Stage 2 (TEST-GENERATOR) are unblocked once spec-critic Round 3 confirms.

---

**End of Round 2 closure.**
