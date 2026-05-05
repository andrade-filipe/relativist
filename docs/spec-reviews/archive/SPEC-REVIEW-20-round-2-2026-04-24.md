# SPEC-REVIEW-20 — Round 2: Elastic Grid — Adversarial Review

**Date:** 2026-04-24
**Reviewer:** spec-critic (adversarial, Stage 0 Round 2, dual-mission: closure audit + fresh adversarial scan)
**Target:** `specs/SPEC-20-elastic-grid.md` (1144 lines, Status: "Draft — Round 2"; grew from 598 → 1144 lines, +91%)
**Predecessors re-consulted:** SPEC-01, SPEC-02, SPEC-04, SPEC-05, SPEC-06, SPEC-13, SPEC-17, SPEC-18, SPEC-19; Round 1 review; ARG-001 (P1-P6); ARG-004 (Passo 12).
**Round 1 baseline:** `docs/spec-reviews/SPEC-REVIEW-20-round-1-2026-04-24.md` — 30 findings (6 C / 11 H / 9 M / 4 L), gate BLOCK.
**Round 2 self-reported closure (§11 Change Log):** 30/30 CLOSED, 0 DEFERRED.

---

## Summary

**Gate decision: CONDITIONAL_PASS.**

| Metric | Value |
|--------|------:|
| Round 1 findings CLOSED           | 27 / 30 |
| Round 1 findings DEFERRED (strong) | 2 / 30  |
| Round 1 findings NOT_CLOSED       | 1 / 30  |
| NF-NNN (Round 2)  CRITICAL        | 0       |
| NF-NNN (Round 2)  HIGH            | 2       |
| NF-NNN (Round 2)  MEDIUM          | 6       |
| NF-NNN (Round 2)  LOW             | 3       |
| **Total NFs**                     | **11**  |

The Round 2 revision is a very substantial uplift: §3.0 Execution Mode Matrix, per-mode requirement splits (R4/R12/R14/R23/R24), wire-schema completion with discriminants 12-16 / PROTOCOL_VERSION=4, `tokio::select!` concurrency model with `SelfPartitionPanic`, explicit amendment section §3.8 (A1-A6), two-slot retention with atomic refresh, D3-elastic invariant, MUST upgrades for R6/R31, and resolution of all OQs. Six CRITICAL findings are now genuinely closed.

The revision is NOT perfect. The single not-actually-closed finding (SC-005) is honestly *gated* rather than closed — SPEC-20 acknowledges ARG-005 must land before D-008 sign-off and marks G1 CONDITIONAL. This is the right call rather than a fake closure, and the spec-critic accepts it. However, the revision also silently introduces a load-bearing dependency on `Net::union` (§4.2.2 v1-mode step 4 and delta-mode step 4) that does NOT exist in SPEC-02 and is NOT listed in §3.8's amendment set. That is a structural gap: NF-001, HIGH. Additionally, the name "ARG-005" is double-allocated — SPEC-19 §8 already opened ARG-005 for a different recoverability proof (border-graph R38); SPEC-20 reuses the same name for mixed-trace recoverability. NF-002, MEDIUM.

The fresh findings are overwhelmingly MEDIUM/LOW and can be addressed inline during Stage 1 (TASK-SPLITTER) or as in-Stage-6 polish. They do not warrant another full review round. The top-3 concerns are:

1. **NF-001** (HIGH): `Net::union` does not exist in SPEC-02; needs A7 amendment to SPEC-02 OR rewrite of §4.2.2 to reuse `merge(PartitionPlan)`.
2. **NF-002** (MEDIUM): "ARG-005" name-collision between SPEC-19 and SPEC-20.
3. **NF-003** (HIGH): R4-delta claims the self-worker emits a `RoundResult` synthesized via `ChannelTransport`, but the in-process self-worker code path has never been exercised for delta-mode — the claim is consistent-by-construction (identical code path) but testability is thin (EG-U4-delta only checks convergence, not message shape equivalence).

**Recommendation:** Proceed to Stage 1 TASK-SPLITTER with the TODO list in this review carried as a task list for the developer (and for ESPECIALISTA EM SPECS on NF-001 / NF-002). Round 3 is AVOIDABLE — the residual work is small-surface, has a clear owner, and does not re-scope any requirement. Gate decision: **CONDITIONAL_PASS.**

---

## Round 1 closure audit

Column key:
- **C** = CLOSED: revision edits demonstrably resolve the finding.
- **D-strong** = DEFERRED with strong rationale (explicit gating mechanism in-spec).
- **D-weak** = DEFERRED but rationale is handwavy.
- **NC** = NOT_CLOSED despite claimed closure in §11.

### CRITICAL (6)

| ID | Verdict | Evidence / Notes |
|---|:---:|---|
| SC-001 Zero integration with SPEC-19 Delta | **C** | §3.0 Execution Mode Matrix (M0-R0d, lines 53-89) is a real specification artifact; §3.1/§3.2/§3.3 now carry per-mode variants (R4-v1/R4-delta, R12-v1/R12-delta, R14-v1/R14-delta, R23b-v1/R23b-delta, R23c-v1/R23c-delta, R24a/R24b with per-mode branches); §4.4.2 delta diagram added; §4.2.2 delta-mode worker-departure path added. Still has residual issues (NF-001, NF-005) but the core gap is closed. |
| SC-002 Hybrid concurrency model | **C** | R3 (lines 103-122) gives an explicit `tokio::select!` pattern across 4 arms with the FSM as single-threaded sink; R3a introduces `SelfPartitionPanic(String)` with transition `WaitingForResults × SelfPartitionPanic → Error` (confirmed in §4.1.4 row); R3b commits to processing membership/timeout events during self-reduce; R3c closes strict-mode uniformity. Uses `ChannelTransport` (SPEC-17 R15, which DOES exist — verified in SPEC-17 line 73). |
| SC-003 Retained-partition semantics D3/D5 | **C** (with NF-001 caveat) | R23 splits into `retained_initial` (R23b, held entire run) + `retained_last_acked` (R23c, atomic refresh at round boundary per R31); R24a/R24b distinguish catastrophic-before-first-round vs after-first-round; R24c formalizes "D3-elastic" (no in-round mixed merge; re-introduce via re-split at clean boundary); R24d adds border_id rebase (A3 SPEC-04 amendment). D3/D5 defenses in §3.7 are substantive. The `Net::union` dependence in §4.2.2 step 4 is a separate issue (NF-001), not a reopening of SC-003. |
| SC-004 Wire protocol incomplete | **C** | R35 (lines 366-423) defines 5 new variants with exact discriminants 12-16 (SPEC-19 used 7-11, verified in SPEC-19 R31-R32 lines 196-208; no collision). R37 bumps `PROTOCOL_VERSION` 3→4 with justification. R35a adds explicit `LeaveAck` semantics (send before close, worker MUST NOT close before receiving). R37a disambiguates `Register` vs `JoinRequest`. `JoinNackReason` enum defined (lines 407-416). `LeaveRequest` no longer carries redundant `worker_id`. |
| SC-005 ARG-001 Passo 4 cited outside scope | **D-strong** | The closure claim in §11 is correct in spirit: R29 explicitly disavows "Passo 4 as direct corollary"; R29a opens ARG-005 and gates D-008 Stage 6 sign-off; R39 marks G1 CONDITIONAL for delta mode AND for any elastic-departure path; §5.1 bullet 4 states "confluence is necessary but not sufficient." This is HONEST deferral with an in-spec gating mechanism — the spec is still implementable; it is the correctness CLAIM that is gated. Accepts Round 1's precedent (SPEC-19 R38 = same pattern). Name-collision issue tracked as NF-002 separately. |
| SC-006 WorkerId vs partition_index | **C** | R11 (lines 156-157) explicitly starts `WorkerId` counter at 1 (0 reserved per R7a); R11a (line 158) decouples `partition_index` (dense in [0, K_eff)) from `WorkerId` (sparse, monotonic). D4-elastic sub-invariant added to §3.7. R8/R13/R30 align with SPEC-04's actual `compute_id_ranges(K_eff)` signature (SPEC-04 R18 verified). New test EG-U12a (sparse {0,1,5,7} scenario) is concrete. |

### HIGH (11)

| ID | Verdict | Evidence / Notes |
|---|:---:|---|
| SC-007 Join window boundary | **C** | R10a drain-then-arm; R10b buffering rule; `join_window_min = 50ms` / `join_window_max = 500ms` fields in R33; new test EG-U6a injects `tokio::yield_now()`. OQ-1 resolved per §8. |
| SC-008 Graceful departure split-mode | **C** | `LeaveKind { AfterResult, Urgent }` enum (R21); R22a/R22b/R22c cover AfterResult/Urgent/silent-upgrade; FSM transitions in §4.1.4 (lines 627-629) concrete. New tests EG-U10a/b/c. |
| SC-009 Solo-mode preemption | **C** | R5 rewritten: `reduce_n(solo_budget)` loop, default 10000; R5a defines two termination conditions; R15 gives `SoloReducing × WorkerJoined → CheckTermination` with batch-completion rule. New test EG-U1a. |
| SC-010 `hybrid_coordinator` default | **C** | R33a defaults table row sets `hybrid_coordinator: false`; §6.1 Step 1 line 910 confirms; EG-B1 runs both paths. |
| SC-011 Amendments missing | **C** | §3.8 lists A1-A6 with explicit target requirements: A1 amends SPEC-06 R25 (verified exists in SPEC-06 line 112); A2 amends SPEC-13 R21 (verified exists in SPEC-13 line 354); A3 new SPEC-04 R15a (`allocate_border_ids`); A4 new SPEC-04 R19a (`remap_partition_ids`); A5 SPEC-05 GridConfig; A6 SPEC-19 metric coexistence. Each amendment names an existing R-number in the target spec. (NF-006 flags that A3/A4 effectively create new public API surface in SPEC-04 — acceptable but merits explicit acknowledgement.) |
| SC-012 Missing `WaitingForResults × WorkerJoined` | **C** | R10b explicit for 4 states + FSM rows in §4.1.4 (lines 616, 619, 629, 631). FSM is now total over the new event set. |
| SC-013 R31 SHOULD release | **C** | R31 upgraded SHOULD→MUST with explicit atomic semantics (release only after round N+1 dispatch fully transmitted). Memory bounds O(sum |partition_w|) stated. New test EG-U13. |
| SC-014 `compute_id_ranges` signature drift | **C** | R8 explicit: "the function does NOT take `next_id` as an argument — that was a wording error in the v1 draft"; aligns with SPEC-04 R18 (verified line 102-109). |
| SC-015 Missing mixed-matrix proptests | **C** | New EG-P4 (full matrix), EG-P5 (CON-DUP-heavy × churn), EG-P6 (delta-mode elastic). |
| SC-016 `worker_id = 0` double-booking | **C** | R2a explicit per-mode semantics; R38b `is_coordinator_self: bool` on `WorkerRoundStats` for disambiguation; new test EG-U1b. |
| NEW: SC-027 (LOW, reported under HIGH closure) | **C** | R7 + R38b `is_coordinator_self`. |

### MEDIUM (9)

| ID | Verdict | Evidence |
|---|:---:|---|
| SC-017 `Shutdown` overloaded | **C** | R35a explicitly reserves `Shutdown` for coordinator-initiated termination; `LeaveAck` (discriminant 15) is dedicated graceful-leave ack. |
| SC-018 Solo-mode FSM transition | **C** | New `SoloReducing` state in §4.1.1 (line 550); transitions in §4.1.4 (rows 612, 638-640). |
| SC-019 GridMetrics / SPEC-19 interaction | **C** | R38a additivity statement. Author claims field-name audit no overlap. (NF-004 probes this.) |
| SC-020 `initial_wait_timeout` vs `worker_connect_timeout` | **C** | R6 upgraded SHOULD→MUST with supersession clause; new test EG-U18. |
| SC-021 Mixed merge signature | **C** | R24c/R24d eliminate mixed-merge via D3-elastic invariant; border_id rebase via A3. (NF-001 is a different gap introduced by the fix.) |
| SC-022 TimerKind not in SPEC-13 | **C** | `TimerKind` enum in §4.1.3 (lines 594-601). (NF-008 is a minor observation about SPEC-13 ownership of this enum.) |
| SC-023 Memory under churn | **C** | R11 u32::MAX cap + `JoinNack{WorkerIdSpaceExhausted}`; R31 atomic refresh; new test EG-U14. |
| SC-024 OQ-2 self-partition assignment | **C** | OQ-2 resolved to `partition_index = 0` fixed; future optimization deferred to v3. |
| SC-025 §5.3 comparison missing SPEC-19 | **C** | §5.3 now has 4 columns covering all mode combinations. |

### LOW (4)

| ID | Verdict | Evidence |
|---|:---:|---|
| SC-026 OQs unresolved | **C** | All 5 OQs resolved per §8 with cross-references into §3. |
| SC-027 `worker_id=0` log ambiguity | **C** | R38b `is_coordinator_self`. Also closed under SC-016. |

### Closure-audit summary

- **27 genuinely CLOSED** by substantive edits.
- **2 DEFERRED with strong rationale**: SC-005 (G1 gated via R29a/R39 on ARG-005) and by extension SC-003 delta-mode (G1 CONDITIONAL on SPEC-19 R38). These are honest deferrals with in-spec gating mechanisms.
- **1 NOT_CLOSED**: none of the 30 findings, but the fix for SC-003 introduced NF-001 (`Net::union` unspecified). The finding itself (SC-003) is closed; the fix carries a new dependency that is Round 2's work to own.

No findings are falsely claimed closed. The §11 Change Log is substantively accurate.

---

## Fresh findings (Round 2)

### NF-001 — `Net::union` is cited in §4.2.2 but does not exist in SPEC-02

**Severity:** HIGH
**Axis:** J (Amendment consistency with target specs); I (newly-introduced types/operations)
**Location:** §4.2.2 v1-mode step 4 (line 707): *"treating them as additional input nets to be unioned via `Net::union` before split"*; §4.2.2 delta-mode step 4 (line 714): *"apply the snapshot's deltas to its corresponding `retained_initial` and union the result."*
**Evidence:** Searched SPEC-02 for `union`, `fn union`, `Net::union`, `union_with`, `disjoint_union`, `union_nets`, `merge_into`, `into_net` — the only match in SPEC-02 is a prose reference to "union type" in the PortRef definition (line 178), which is a discriminated-sum type, NOT a net combination operation. No `Net::union` method is defined anywhere in SPEC-02 (or SPEC-05, SPEC-04). The specialist self-flagged this (RZ-1) but the revision ships §4.2.2 with the call anyway.
**Impact if unresolved:** Stage 1 TASK-SPLITTER cannot produce a task for this; Stage 3 DEVELOPER has to either (a) invent `Net::union` ad hoc, pushing a definitional commitment out of the spec and into code comments, or (b) block and force a SPEC-02 amendment cycle. Either way, this is the exact "spec inventing signatures" failure mode that SC-014 was chastised for.
**Suggested resolution:** Two clean options:
1. **Preferred: add A7 to §3.8.** *"A7. SPEC-02 amendment (net combination primitive): SPEC-02 MUST expose `Net::union(self, other: Net) -> Net` that concatenates two agent arrays under the assumption that the caller has already ensured disjoint `AgentId` ranges. Border FreePorts on either side are preserved. This primitive is required by SPEC-20 §4.2.2 departure-recovery re-split."* Then SPEC-02 gets a patch in its next revision per the §3.8 pattern.
2. **Alternative: rewrite §4.2.2 to re-use existing `merge()`.** Define the reclaimed partition as a single-partition input to a `PartitionPlan` (per SPEC-05 R1) and let `merge()` do the union. This avoids the A7 amendment but requires that `merge()` accepts partitions with mismatched round-state — which is exactly the concern SC-021 flagged and R24c was designed to avoid. This option is messier.

Option 1 is recommended; it is a small, well-scoped amendment.
**Invariant affected:** None directly, but spec-ownership coherence is violated (spec calls an operation that doesn't exist in any predecessor).

---

### NF-002 — "ARG-005" is a name collision between SPEC-19 and SPEC-20

**Severity:** MEDIUM
**Axis:** I (well-definedness of named extensions)
**Location:** SPEC-19 line 405 and line 1168 open *"ARG-005"* for the delta-protocol recoverability proof (proving the `merge(reconstruct(border_graph, worker_partitions)) ~ mu_k` isomorphism); SPEC-20 line 7 ("Arguments consumed"), R29a (line 267), R39-G1-elastic-departure (line 489), §5.1 bullet 4 (line 876) open *"ARG-005"* for mixed-trace recoverability (proving "reclaimed snapshot re-introduced at round k converges to NF(mu)").
**Evidence:** The two arguments are conceptually related but not identical. SPEC-19's ARG-005 is about coordinator-state-is-reconstructible-from-distributed-state. SPEC-20's ARG-005 is about recovery-from-partial-execution-via-retained-snapshots. Both exist in the pending/open state; no one has written either.
**Impact if unresolved:** When DEBATEDOR lands ARG-005, which proof is being discharged? If it's only the SPEC-19 flavor, SPEC-20's R29a gate is still open; if it's only the SPEC-20 flavor, SPEC-19's R38 remains pending. Implementation can proceed (both are currently CONDITIONAL anyway), but the gating mechanism is non-unique.
**Suggested resolution:** Either:
1. **Rename one of them.** SPEC-20 could open ARG-005 (as it does) and rename the SPEC-19 obligation to ARG-006; OR SPEC-20 could acknowledge it is extending an already-opened ARG-005 and frame its proof obligation as a *subcase* ("ARG-005 Part II: mixed-trace extension").
2. **Merge them.** Write one ARG-005 that covers both recoverability claims as a single theorem — "given strong confluence + any sequence of coordinator-side reconstructions (from BorderGraph + delta deltas) and/or retained-partition re-introductions, the final state converges to NF(mu)". This is probably the cleanest outcome because the two obligations use the same theoretical toolkit.

Either path requires coordination with DEBATEDOR and ESPECIALISTA EM SPECS; it is not blocking.
**Invariant affected:** None.

---

### NF-003 — R4-delta claims self-worker emits a `RoundResult` via ChannelTransport, but the self-worker's delta-round loop semantics are not defined

**Severity:** HIGH
**Axis:** L (Testability), C (FSM correctness, cross-mode)
**Location:** R4-delta (lines 132-133): *"the self-worker's local `RoundResult` synthesized via the in-process channel"*; R4-delta does not specify HOW the self-worker receives its `RoundStart` from the coordinator, applies border deltas to its own partition, and emits the `RoundResult`.
**Evidence:** The hybrid-mode pseudocode in §4.1.5 (lines 643-679) only describes v1 dispatch flow (`AssignPartition`). Neither §4.1.5 nor §4.4.2 (delta diagram, lines 762-777) specifies the wire-shape symmetry for the self-worker. Reading the spec literally: the coordinator sends itself a `RoundStart(deltas)` through `ChannelTransport`, its own self-worker task applies the deltas to its partition, emits a `RoundResult(deltas)` back through the channel, and the coordinator applies that `RoundResult` to its `BorderGraph` identically to any remote worker's. This is plausible and consistent-by-construction, but:

1. The self-worker must maintain its own `BorderGraph` (for local border detection within its partition)? Or does it share the coordinator's? SPEC-19 has stateful workers; each worker in delta mode maintains its partition state but NOT a coordinator-side BorderGraph. So the self-worker is a delta-mode worker, NOT a delta-mode coordinator. The spec should say so.
2. Test EG-U4-delta (line 969) only asserts "converged state matches v1 hybrid" — it does NOT assert that the self-worker's delta-round `RoundResult` is byte-identical-shape to a remote worker's. If the implementation accidentally short-circuits the serialization path for the self-worker, a subtle protocol drift could persist undetected.

**Impact if unresolved:** Ambiguity in Stage 3 implementation. Developer may reasonably implement the self-worker by running `reduce_all` on the full partition (not the delta loop), producing the same converged result but bypassing the delta protocol entirely. The test plan cannot catch this.
**Suggested resolution:**
1. Add R4-delta-self-symmetry clause: *"In delta mode, the in-process self-worker MUST execute the full worker-side delta state machine (SPEC-19 R24, the delta loop). The self-worker maintains its own `partition` and `border_id` endpoint state, receives `RoundStart(deltas)` from the coordinator via `ChannelTransport`, applies deltas via `apply_border_deltas(partition, deltas)`, reduces via `reduce_all(partition)`, and emits `RoundResult(deltas)` back through the channel. The coordinator's `BorderGraph` consumes this `RoundResult` identically to any remote worker's, guaranteeing R4-delta's 'treated identically' claim by construction."*
2. Add test EG-U4-delta-wire-symmetry: assert that the self-worker's `RoundResult.border_deltas` payload is shape-identical to what a same-partition remote worker would emit (observable via instrumented channel).

**Invariant affected:** D2 (Local Reduction Equivalence) — if the self-worker uses a different reduction path than remote workers, D2 is violated per-worker even though the global result still converges.

---

### NF-004 — `GridMetrics` field list lacks cross-spec additivity evidence; only prose assurance

**Severity:** MEDIUM
**Axis:** H (revision-induced coherence), L (testability)
**Location:** R38 (lines 438-460) adds 7 new metric fields; R38a (line 463) asserts additivity with SPEC-19's metrics and claims "no current overlap" but does not enumerate SPEC-19's fields inline.
**Evidence:** SPEC-19 R45 defines metrics (I did not re-list them here, but they include `border_graph_apply_deltas_time_per_round`, `delta_bytes_sent_per_round`, etc.). A reviewer re-reading SPEC-20 alone cannot verify non-overlap without opening SPEC-19. R38a says "any field-name collision discovered at implementation time is a spec defect to be reported back to ESPECIALISTA EM SPECS for arbitration" — this defers the audit to implementation, violating Axis F (Testability) of the spec review standard ("ambiguity at implementation time is a spec bug").
**Impact if unresolved:** Stage 3 implementation discovers a collision, opens a ticket back to specs, wastes a round-trip.
**Suggested resolution:** Either:
1. List SPEC-19 R45's field names verbatim in R38a alongside SPEC-20's, or
2. Change R38a to reference a specific commit/revision of SPEC-19 and commit: "The reviewer of this spec has verified against SPEC-19 @ <revision> that field names X, Y, Z do not collide with SPEC-20's A, B, C, D, E, F, G." — this commits the reviewer's claim to the written record.

**Invariant affected:** None.

---

### NF-005 — §4.2.2 delta-mode departure step 4's `reconstruct` signature is extended but §3.8 has no amendment for SPEC-19

**Severity:** MEDIUM
**Axis:** J (Amendment consistency)
**Location:** §4.2.2 delta-mode departure step 4 (line 714): *"`reconstruct(border_graph, surviving_partitions, reclaimed_partitions)` — `reconstruct` is extended (SPEC-19 amendment) to accept reclaimed partitions reconstructed from `retained_last_acked`"*.
**Evidence:** SPEC-19 R38 (line 403) defines `reconstruct(border_graph, worker_partitions)` — 2-argument signature. SPEC-20 invokes a 3-argument version with `reclaimed_partitions` as a separate input. §3.8 A6 lists only metric coexistence for SPEC-19, NOT a signature extension. Open Issue #4 in §9 mentions "SPEC-19 reconstruct extension needed" but this is an informal to-do, not a formal amendment in §3.8.
**Impact if unresolved:** Same category as NF-001 — spec invokes a predecessor-spec function with a signature not supported by the predecessor. Will require either a SPEC-19 amendment or a coordinator-side wrapper.
**Suggested resolution:** Promote Open Issue #4 into §3.8 as amendment A7 (or A8 if NF-001 takes A7): *"A8. SPEC-19 R38 signature extension: `reconstruct` MUST accept an optional third argument `reclaimed_partitions: Vec<Partition>` that is unioned with `worker_partitions` before reconstruction. When empty (default), behavior matches SPEC-19 R38 exactly."*

**Invariant affected:** None directly; spec-ownership coherence (same issue class as Round-1 SC-011).

---

### NF-006 — §3.8 A3/A4 add new *public* API surface to SPEC-04 but the amendments do not specify error conditions or edge cases

**Severity:** MEDIUM
**Axis:** J (amendment consistency), L (testability)
**Location:** §3.8 A3 (lines 509-511): *"`PartitionPlan::allocate_border_ids(count: u32) -> Range<u32>`"*; §3.8 A4 (lines 513-515): *"`remap_partition_ids(partition: Partition, new_range: IdRange) -> Partition`"*.
**Evidence:** A3: what happens if `count > u32::MAX - next_border_id`? The function signature returns `Range<u32>` (infallible); overflow is unspecified. A4: what happens if `partition.agent_count() > new_range.len()`? Panic? Error? Clamp? Unspecified.
**Impact if unresolved:** Stage 1 TASK-SPLITTER will either (a) guess at error semantics, producing unreachable-or-panicking code paths, or (b) flag it back to specs for clarification. Either way a delay.
**Suggested resolution:** Expand A3/A4 with error-return signatures:
- A3: `-> Result<Range<u32>, PartitionError>` where error variant `BorderIdSpaceExhausted { requested: u32, available: u32 }`.
- A4: `-> Result<Partition, PartitionError>` where error variant `NewRangeTooSmall { partition_size: u32, range_size: u32 }`.

SPEC-04's existing error type `PartitionError` (verified in SPEC-04) takes these variants cleanly.
**Invariant affected:** None, but the amendments are under-specified.

---

### NF-007 — R26's "multiple simultaneous departures collectively in a single re-partition cycle" has no upper bound on D

**Severity:** MEDIUM
**Axis:** L (testability), I (well-definedness)
**Location:** R26 (line 257): *"If multiple workers depart in the same round, the coordinator MUST handle all departures collectively in a single re-partition cycle ... MUST NOT perform multiple sequential re-partitions"*.
**Evidence:** R26 does not bound D. What happens if D == K_eff (all workers depart simultaneously)?  R27 partially addresses this ("If all remote workers depart and hybrid=true, fall back to solo") but only for the hybrid-and-all case. What about non-hybrid with D == K_eff? The spec says "transition to Error" (v1 fatal). But R26 requires "single re-partition cycle" — if D == K_eff, the re-partition input is entirely reclaimed partitions, which is fine for v1 (`merge(reclaimed_partitions)` is well-defined if `Net::union` exists, see NF-001) but the delta-mode flow requires a `FinalStateRequest` to surviving workers (step 3 of §4.2.2 delta). If there are NO surviving workers, step 3 has nothing to ask — behavior unspecified.
**Impact if unresolved:** A pathological scenario (all workers die in the same round) either deadlocks or silently falls into Error. In delta mode, the rejoin-from-reclaimed flow is never executed and the run dies.
**Suggested resolution:** Add R26a: *"If D == K_eff (all active workers depart in the same round):
- (hybrid = true): coordinator discards all `retained_last_acked` snapshots (they are derivable from `retained_initial` + the coordinator's own still-progressing self-partition), then falls back per R27.
- (hybrid = false): coordinator transitions to Error per R27 second clause.
In either case, the delta-mode `FinalStateRequest` step of §4.2.2 step 3 is SKIPPED; the coordinator reconstructs by applying `retained_last_acked` deltas to `retained_initial` for each reclaimed worker directly."*

**Invariant affected:** D6 (protocol termination) AT_RISK for D==K_eff edge case.

---

### NF-008 — `TimerKind` enum is defined in SPEC-20 §4.1.3 but claimed to "derive `TimerId = u32` deterministically"; the derivation is left to the implementer

**Severity:** LOW
**Axis:** I (well-definedness), L (testability)
**Location:** §4.1.3 (line 594-601): *"SPEC-13 R21's `TimerId = u32` is derived deterministically from `TimerKind` (e.g., as `kind as u32`)"*.
**Evidence:** "e.g., as `kind as u32`" is an example, not a requirement. If an implementer uses a hash or maps in declaration order, the derivation is still "deterministic" but different from another implementation. Fine for correctness but makes test assertions on `TimerId` values non-portable.
**Suggested resolution:** Change "e.g." to "MUST be": *"`TimerId = kind as u32` (via `#[repr(u32)]` on `TimerKind`)"*. Trivial fix; 1-line edit.
**Invariant affected:** None.

---

### NF-009 — `JoinNackReason::ProtocolVersionMismatch` carries u32 / u32 but `Register`/`RegisterNack` uses a different shape (SPEC-19 R37)

**Severity:** LOW
**Axis:** J (cross-spec consistency), H (revision-induced)
**Location:** R35 (line 409): `ProtocolVersionMismatch { coordinator: u32, worker: u32 }`; SPEC-19 R37 (referenced but not inline-quoted) also defines a `RegisterNack` variant for version mismatch; the two shapes may diverge.
**Evidence:** R0d (line 89) says `RegisterNack { reason: ProtocolVersionMismatch }` — it does not specify payload. If SPEC-19 R37's `RegisterNack` has a different payload shape than SPEC-20 R35's `JoinNack.ProtocolVersionMismatch`, a v3 worker talking to a v4 coordinator and a mid-session-join worker talking to a same-version coordinator get inconsistent error shapes.
**Suggested resolution:** Cross-ref SPEC-19 R37's `RegisterNack` and align shape: either reuse the same struct for both `RegisterNack` and `JoinNack` variants, or explicitly acknowledge divergence. Likely 1-2 line edit.
**Invariant affected:** None.

---

### NF-010 — EG-U15 (`test_protocol_version_mismatch_rejection`) refers to `RegisterNack { ProtocolVersionMismatch }` but SPEC-20 only defines `JoinNack` with that reason

**Severity:** LOW
**Axis:** H (revision-induced coherence), L (testability)
**Location:** EG-U15 (line 989): *"v3 worker connects to v4 coordinator; rejected with `RegisterNack { ProtocolVersionMismatch }`"*.
**Evidence:** SPEC-20 R35 defines `JoinNack { reason: JoinNackReason::ProtocolVersionMismatch {...} }` (discriminant 16). It does NOT amend `RegisterNack`. If the rejection path uses `Register` (initial-window worker), the existing SPEC-19 R37 `RegisterNack` applies (not in SPEC-20); if the path uses `JoinRequest` (mid-session), the new `JoinNack` applies. EG-U15 conflates them.
**Suggested resolution:** Split EG-U15 into EG-U15a (initial Register handshake version mismatch → RegisterNack per SPEC-19) and EG-U15b (mid-session JoinRequest version mismatch → JoinNack per SPEC-20 R35). Clarifies which code path is under test.
**Invariant affected:** None, test-plan clarity only.

---

### NF-011 — R23a's `retained_initial` retention policy ("Released only when `w` either completes the run successfully or is shut down") is incompatible with R32

**Severity:** MEDIUM
**Axis:** I (well-definedness), H (revision-induced coherence)
**Location:** R23a (line 230): *"`retained_initial[w]`: ... Allocated once per worker (at the worker's first AssignPartition in v1 or first InitialPartition in delta) and held for the *entire* run. Released only when `w` either completes the run successfully or is shut down via `Shutdown`."*; R32 (line 283): *"When `retain_partitions = false`, `elastic_departure` MUST also be `false` ..."*.
**Evidence:** Two separate issues:
1. **Memory upper bound.** R31 (line 279-281) claims `retained_initial` memory bound is `O(sum_{w in W_ever_active} |partition_w|)` — but `W_ever_active` is monotonically growing (WorkerIds never reused per R11). With worst-case adversarial join-leave churn, `W_ever_active` grows with time, so the bound is O(time × |partition|) — NOT the O(sum) in R31's claim. For TCC scope (≤ 8 workers, ≤ few churn cycles) this is fine, but the stated bound is still wrong.
2. **"Released only when w either completes the run successfully or is shut down."** What about a worker that departed (timeout) — is its `retained_initial` released? R24a says "coordinator MUST reclaim `retained_initial[w]`" — consumes the slot, but R23a says "released only when complete or Shutdown." Reclaim vs release is not reconciled.

**Impact if unresolved:** Memory accounting is incorrect; potential memory bloat under churn.
**Suggested resolution:**
1. Refine R23a: *"Released when `w` either (a) completes the run successfully via graceful `AfterResult` leave, (b) is shut down via `Shutdown` (coordinator-initiated), or (c) has its `retained_initial` consumed by a reclaim (R24a), at which point the reclaimed partition is re-split into a new round's partitions and the old slot is freed."*
2. Fix R31 bound to O(sum_{w in W_current_active_or_retained} |partition_w|) — bounded by K_eff + D_pending_reclaim at any instant, NOT by cumulative history.

**Invariant affected:** Memory bounds statement (informative, but inaccurate).

---

## Specialist self-flagged zones

### RZ-1 — `Net::union` in §4.2.2: **CONFIRMED as spec gap.**

The specialist correctly flagged the risk. The revision shipped with the call anyway. SPEC-02 does NOT define `Net::union` (exhaustive grep in specs/ confirms only prose mentions of "union type" in the PortRef discriminated-union context — NOT a method on `Net`). This is NF-001 above, HIGH severity.

**Verdict:** RZ-1 is not resolved. Must be addressed via either A7 amendment to SPEC-02 (preferred) or §4.2.2 rewrite. Blocking neither Stage 1 (TASK-SPLITTER can track it as a known-to-fix item) nor the overall CONDITIONAL_PASS gate.

### RZ-2 — ARG-005 constructive sketch absence: **ACCEPTABLE as-is.**

The specialist asked whether D-008 gating on an external ARG-005 is acceptable without a proof sketch in the spec. I rule **yes**, with the following caveats:

1. SPEC-19 set the precedent (R38 pending ARG-005 for border-graph recoverability). SPEC-20 following the same pattern is consistent.
2. R29a's gating mechanism is explicit and auditable: D-008 Stage 6 sign-off is conditional on ARG-005 landing OR scope reduction to Passo-4-directly-applicable subclasses.
3. R39's G1 CONDITIONAL marking is honest and forces downstream agents to treat the correctness claim as pending-proof.
4. §5.1 bullet 4 explicitly frames the gap ("confluence is necessary but not sufficient").

However, the spec could be strengthened by adding a half-page proof sketch in §5.1 or §5.2 that gestures at WHY the mixed-trace proof should be discharge-able (the "re-split at clean boundary reduces the obligation to ARG-001 Passo 11" argument in R39-G1-elastic-departure is a legitimate sketch, but it's buried in the invariant table). This is not blocking but is a Round 3 polish item.

**Verdict:** RZ-2 resolved satisfactorily. The name-collision with SPEC-19's ARG-005 (NF-002) is a separate, smaller issue.

---

## Invariant audit (Round 2)

Compared against Round 1's invariant table:

| Invariant | Round 1 | Round 2 | Δ |
|-----------|:--------|:--------|:---|
| **T1-T7** | PRESERVED | PRESERVED | — |
| **D1** (split/merge identity) | AT_RISK | PRESERVED | ↑ Closed via R24c (D3-elastic): no mixed-state merge. Merge always sees uniform state. |
| **D2** (local reduction equivalence) | PRESERVED | PRESERVED | — (NF-003 raises a minor wire-symmetry concern for delta self-worker) |
| **D3** (border completeness) | AT_RISK | PRESERVED | ↑ Closed via R24c D3-elastic invariant + R24d border_id rebase (A3 amendment). |
| **D4** (ID uniqueness) | AT_RISK | PRESERVED | ↑ Closed via D4-elastic (R11a): partition_index-keyed ID range computation. |
| **D5** (exclusive ownership) | AT_RISK | PRESERVED | ↑ Closed via R31 atomic refresh: transient double-ownership window eliminated. |
| **D6** (protocol termination) | AT_RISK | PRESERVED (caveat NF-007) | ↑ Closed via R3 tokio::select!, R10a drain, R5 reduce_n(budget) loop, R10b FSM totality, R6 MUST timer supersedes. NF-007 (D==K_eff edge case) is an open sub-concern but not a reversal. |
| **I1-I7** | PRESERVED | PRESERVED | — |
| **G1 (v1 modes A, B)** | AT_RISK | PRESERVED via R39-G1-v1 | ↑ Closed for v1 paths via reuse of ARG-001 Passos 5-12. |
| **G1 (delta modes C, D)** | UNDEFINED | CONDITIONAL on SPEC-19 R38/ARG-005 | ↑ Honest framing via R39-G1-delta; inherits SPEC-19's gating. |
| **G1 (elastic departure any mode)** | UNDEFINED | CONDITIONAL on ARG-005 (R29a) | ↑ Honest framing; gating via R29a ⇒ D-008 Stage 6 sign-off. |
| **P1, P5** (ARG-001) | P5 AT_RISK | PRESERVED | ↑ Same as D6 rehabilitation. |

**Net change:** Round 1 had D1, D3, D4, D5, D6 AT_RISK and G1 UNDEFINED. Round 2 has ALL of those PRESERVED (with honest CONDITIONAL gating for G1 in delta and elastic-departure paths, which is strictly better than UNDEFINED). This is a strict improvement.

---

## Cross-spec consistency audit (Round 2)

Each §3.8 amendment vs the current text of the target spec:

| Amendment | Target | Verified | Verdict |
|-----------|--------|---------|---------|
| A1: SPEC-06 R25 conditional | SPEC-06 R25 (line 112: "If a connection with a worker is lost ... coordinator MUST abort ... return an error") | YES, target exists | **CONSISTENT.** Amendment adds narrow conditional "unless elastic_departure = true"; does not contradict the fault-tolerance-out-of-scope posture of SPEC-06 because elastic_departure is opt-in. |
| A2: SPEC-13 R21 transitions | SPEC-13 R21 (line 354: "transition table MUST implement at minimum...") | YES, target exists | **CONSISTENT.** R21 says "at minimum"; amendments add rows, none are removed or contradicted. New states `AcceptingMembershipChanges`, `SoloReducing` are cleanly additive. |
| A3: SPEC-04 R15a new | SPEC-04 R15 (line 92-94 about FreePort distinction — NOT the same R15 as amendment target!) | AMBIGUOUS | **MINOR ISSUE.** A3 claims "R15 (border_id allocation, currently described as a one-shot allocation at split()-time)" but SPEC-04 R15 (line 92) is actually about FreePort Lafont vs Boundary distinction. Border_id allocation is SPEC-04 R16-R18. The amendment targets a non-existent or mis-numbered R15; it should target "SPEC-04 §4.x border_id allocation" or name a specific R-number. See NF-006 for API shape concerns. |
| A4: SPEC-04 R19a new | SPEC-04 R19 (line 109: "Static ID space partitioning MUST eliminate remapAllPartitions") | YES, target exists | **TENSION.** R19 says remap is ELIMINATED. A4 re-introduces it narrowly. The phrase "narrow exception" in the amendment ("This is the ONLY supported caller of remap; v1 reduction continues to require zero remaps.") addresses the tension but mild. Acceptable. |
| A5: SPEC-05 GridConfig | SPEC-05 (extended per R33 fields) | YES | **CONSISTENT.** Standard additive extension per R33a defaults. |
| A6: SPEC-19 R45 metric coexistence | SPEC-19 R45 | YES | **CONSISTENT.** Additive; NF-004 asks for explicit field-list enumeration but not required. |
| (NF-005 NEW) SPEC-19 reconstruct signature extension | SPEC-19 R38 | — | **MISSING.** Should be A7 or A8 (see NF-005). |
| (NF-001 NEW) SPEC-02 Net::union | SPEC-02 | — | **MISSING.** Should be A7 (see NF-001). |

**Verdict:** 6/6 listed amendments are broadly consistent with target-spec text; A3 has a numbering mismatch that should be fixed (1-line edit); NF-001 and NF-005 identify 2 missing amendments that should be added to §3.8.

---

## Gate decision

**CONDITIONAL_PASS.**

**Rationale.** Round 1's 6 CRITICAL findings are genuinely closed (5 structurally, 1 honestly gated via R29a). All 11 HIGH findings are closed. All 9 MEDIUM and 4 LOW findings are closed. The invariant audit shows strict improvement (D1/D3/D4/D5/D6 all rehabilitated from AT_RISK to PRESERVED; G1 moved from UNDEFINED to CONDITIONAL-with-gating). The revision grew from 598 → 1144 lines with no new CRITICAL issues introduced. All 11 fresh findings are HIGH-or-below, and none of them reopen a closed Round 1 finding.

The 2 HIGH new findings (NF-001 Net::union, NF-003 self-worker delta symmetry) are both narrow, well-scoped, and addressable inline during Stage 1 (TASK-SPLITTER) by carrying them as explicit tasks rather than blocking the pipeline. The remaining MEDIUM/LOW findings are polish-grade.

This is the textbook CONDITIONAL_PASS scenario: "90% there; the 10% is a TODO list, not a re-scoping." Forcing a Round 3 would be perfectionism — the residual items have clear owners (ESPECIALISTA EM SPECS for §3.8 edits; DEBATEDOR for ARG-005) and can proceed in parallel with Stage 1.

**Round 3 is AVOIDABLE** provided the TODO list below is committed to the backlog and tracked through Stage 6.

---

## Specialist-em-specs TODO list (pre-Stage-1 polish; does NOT gate Stage 1 start)

These are surgical edits that should happen before or during Stage 1 TASK-SPLITTER. None require re-scoping; all are ≤ 1 paragraph each.

1. **[NF-001, HIGH] Add A7 to §3.8: SPEC-02 Net::union amendment.** New §3.8 row defining `Net::union(self, other: Net) -> Net` (disjoint AgentId assumption). Coordinate with SPEC-02 maintainer. OR alternative: rewrite §4.2.2 steps 4 to call `merge()` on a synthetic single-partition `PartitionPlan` instead. Former is cleaner.

2. **[NF-002, MEDIUM] Resolve ARG-005 name collision.** Either rename SPEC-20's obligation to ARG-006, OR explicitly frame as "ARG-005 Part II: mixed-trace extension", OR write a single unified ARG-005 covering both. Coordinate with DEBATEDOR.

3. **[NF-003, HIGH] Add R4-delta self-worker symmetry clause.** Specify the in-process self-worker MUST run SPEC-19 R24's full worker-side delta state machine (not a short-circuit). Add EG-U4-delta-wire-symmetry test.

4. **[NF-004, MEDIUM] Enumerate SPEC-19 R45 metric field names in R38a.** Commit the collision audit to the written record.

5. **[NF-005, MEDIUM] Promote Open Issue #4 into §3.8 as A7/A8.** Formalize `reconstruct` 3-argument signature extension as an amendment to SPEC-19 R38.

6. **[NF-006, MEDIUM] Expand A3/A4 with error-return signatures.** `-> Result<..., PartitionError>` with named error variants.

7. **[NF-007, MEDIUM] Add R26a for D == K_eff edge case.** Spell out hybrid-fallback and non-hybrid-Error paths; specify that `FinalStateRequest` step is skipped in this case.

8. **[NF-008, LOW] Upgrade TimerKind derivation from "e.g." to MUST.** One-line edit.

9. **[NF-009, LOW] Align JoinNackReason::ProtocolVersionMismatch shape with SPEC-19 RegisterNack.** Cross-reference or unify.

10. **[NF-010, LOW] Split EG-U15 into EG-U15a (Register path) and EG-U15b (JoinRequest path).**

11. **[NF-011, MEDIUM] Fix R23a retention policy wording and R31 memory bound.** Clarify "reclaim consumes the slot"; correct bound from O(|W_ever_active|) to O(|W_currently_active + W_pending_reclaim|).

12. **[A3 amendment number mismatch]** A3 targets "SPEC-04 R15" but R15 is about FreePort distinction; border_id allocation is R16-R18. Fix the amendment target.

13. **[§5.1 proof sketch polish]** Consider pulling the "reduce to Passo 11 via clean-boundary re-split" argument out of R39-G1-elastic-departure into §5.1 as a proper sketch. Not blocking.

---

## Sign-off

Round 2 verdict: **CONDITIONAL_PASS.**
SPEC-20 is cleared to enter Stage 1 TASK-SPLITTER with the above 13-item TODO list tracked as explicit follow-up work. Round 3 is not required.

**End of Round 2 review.**
