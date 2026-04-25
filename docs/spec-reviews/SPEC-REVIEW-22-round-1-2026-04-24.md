# SPEC-REVIEW-22 — Round 1 (Adversarial)

**Date:** 2026-04-24
**Reviewer:** spec-critic (adversarial)
**Target:** `specs/SPEC-22-arena-management.md` (Status: Draft v1)
**Predecessors consulted (verbatim):** SPEC-00, SPEC-01 v3.1, SPEC-02 Revised v3, SPEC-04, SPEC-05, SPEC-19, SPEC-20, SPEC-23 (forward ref).
**Live code consulted:** `relativist-core/src/net/core.rs` (Net struct L24-62), `relativist-core/src/partition/helpers.rs`, `relativist-core/src/merge/grid.rs`.
**Theory bridge consulted:** `docs/theory-bridge.md` (last updated 2026-04-24).
**Coherence brief consulted (NOT relied upon):** `docs/briefings/SPEC-22-coherence-brief-2026-04-24.md`.

---

## 1. Summary

**Gate decision: BLOCK.**

SPEC-22's body content is largely sound (the free-list mechanism is well-motivated; the SparseNet design is internally consistent), but the spec ships with a structural defect that makes literal implementation unsafe and at least three cross-spec amendment gaps that would corrupt the v2 grid cycle if implemented as written. The specific blockers fall into three families: (1) the §4.1 `Net` struct definition silently deletes the load-bearing `freeport_redirects` field that already exists in the live codebase and that SPEC-05 / SPEC-19 depend on; (2) the amendment surface is incomplete — SPEC-22 amends SPEC-01 I3 but not its sibling SPEC-02 R2/R10 ("never reused"), and the frontmatter omits SPEC-04 and SPEC-05 from `Depends on:` even though R10a/R12/R22 explicitly amend `build_subnet()` and `merge()`; (3) the free-list / `BorderGraph` (SPEC-19) interaction is unaddressed, opening a slot-id-stability hole that, in the worst case, causes the coordinator to read the wrong agent type at a recycled `AgentId` between rounds. Items (1) and (2) are CRITICAL; item (3) is HIGH but mitigable by stating that either free-list recycling is forbidden in delta mode, or that `BorderGraph` must be invalidated on recycle.

**Severity-bucketed counts:**

| Severity | Count |
|----------|-------|
| CRITICAL | 4 |
| HIGH     | 7 |
| MEDIUM   | 6 |
| LOW      | 4 |
| **Total** | **21** |

**Top-3 concerns by severity (one line each):**

1. **SC-001 (CRITICAL):** `Net` struct in §4.1 omits `freeport_redirects: HashMap<u32, PortRef>`; literal implementation deletes a load-bearing field that SPEC-05 uses for FreePort-to-FreePort redirects.
2. **SC-002 (CRITICAL):** SPEC-22 amends SPEC-01 I3 but does not amend SPEC-02 R2 ("monotonically increasing, never reused") or SPEC-02 R10 ("`next_id` MUST be strictly greater than any `AgentId` in use") — these are the implementation-level twins of I3 and now contradict the new I3' semantics.
3. **SC-005 (HIGH):** SPEC-19 `BorderGraph` stores `AgentPort(id, port)` references that are valid at the time of the last delta; SPEC-22 R10's per-worker ID-range constraint does not prevent the coordinator from reading a recycled `AgentId` slot whose live agent now has a different `Symbol` than the `BorderState` was indexed against. G1 threat under delta mode (SPEC-19/SPEC-20).

**Recommendation:** Round 2 must be triggered. Fix the four CRITICAL findings before any task-splitter pass; the seven HIGH findings should be closed before the test-generator opens T1-T18.

---

## 2. Findings (organized by severity)

Axis legend (A-L): A = Frontmatter integrity · B = Definitions / Glossary · C = Predecessor consistency · D = Cross-spec amendment soundness · E = Algorithmic correctness · F = Invariant preservation · G = Testability · H = Edge / boundary completeness · I = Forward compatibility · J = Open question hygiene · K = Configuration / feature gating · L = Rationale calibration.

### 2.1 CRITICAL

#### SC-001 — `Net` struct in §4.1 silently deletes `freeport_redirects`

- **Axis:** D (cross-spec amendment soundness) + E (algorithmic correctness).
- **Location:** §4.1, lines 184-197 (`pub struct Net { … }` definition).
- **Evidence:** §4.1 lists exactly 6 fields: `agents`, `ports`, `redex_queue`, `next_id`, `root`, `free_list`. The live code at `relativist-core/src/net/core.rs:L24-L62` defines 7 fields; the seventh is `pub freeport_redirects: HashMap<u32, PortRef>` (L61, `#[serde(skip)]`, gated `#[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]`). The field is referenced from `merge/grid.rs`, `merge/helpers.rs`, `partition/types.rs`, `partition/compact.rs`, `partition/helpers.rs`. SPEC-22 makes no mention of this field anywhere.
- **Impact if unresolved:** The task-splitter and developer would, following §4.1 literally, either (a) delete `freeport_redirects` and silently break SPEC-05 `rebuild_free_port_index` and SPEC-19 delta initialization (R10), or (b) re-introduce the field as an undocumented seventh field with no guidance on whether SPEC-22 expects it to be cleared in `to_dense()`, copied in `to_sparse()`, or handled at all. Either path violates ARG-002 C1-C3 (border bijection) under multi-round operation.
- **Suggested resolution:** Either (a) add `freeport_redirects` to the §4.1 struct (preferred — full struct definition), with explicit guidance on whether free-list recycling clears the map for the recycled id; or (b) prepend §4.1 with the disclaimer "the snippet shows ONLY fields ADDED OR MODIFIED by this spec; existing SPEC-02 fields including `freeport_redirects` are unchanged" and add a normative requirement that the `to_sparse()` / `to_dense()` round-trip (R21) preserves `freeport_redirects`. Option (a) is cleaner; option (b) is acceptable if §4.6's `to_dense()` block is amended to copy `freeport_redirects` (currently it constructs the dense `Net` without that field, which is a separate symptom of the same defect).
- **Invariant affected:** D1c (FreePort bijectivity, ARG-002 C3); G1 under SPEC-05 merge.
- **Pesquisador prediction:** **CONFIRMED.** The brief (§5a, §7 [CRITICAL-1]) is correct. I add the further observation that §4.6 `SparseNet::to_dense()` (lines 452-483) constructs a `Net { agents, ports, redex_queue, next_id, root, free_list: Vec::new() }` — six fields — silently omitting `freeport_redirects`. So the omission is not isolated to the struct diagram; it propagates into the conversion code, which would emit invalid `Net` values if compiled against the live struct. This deepens SC-001 from "documentation typo" to "active code-spec divergence on two surfaces (struct + conversion)".

#### SC-002 — Amendment surface is incomplete: SPEC-02 R2 and SPEC-02 R10 not amended

- **Axis:** D (cross-spec amendment soundness).
- **Location:** Frontmatter (`Amends:` line); §3.3 R24 (the I3 → I3' amendment).
- **Evidence:** SPEC-22 frontmatter says "Amends: SPEC-01 I3 (Monotonicity of AgentIds — relaxed for free-list variant), SPEC-02 R12 (remove_agent — extended for free-list)." SPEC-02 R2 (verbatim, SPEC-02 line 37): "The `AgentId` type MUST be `u32`, monotonically increasing, never reused within an execution (cf. SPEC-01, I3). **(MUST)**". SPEC-02 R10 (verbatim, SPEC-02 line 58): "The field `next_id` MUST be strictly greater than any `AgentId` in use in the net (cf. SPEC-01, I3). After creating `k` agents, `next_id` MUST be incremented by `k`."
  After applying SPEC-22 R3, `create_agent` does NOT increment `next_id` on the recycle path (SPEC-22 R3 explicitly forbids it). After R2 (free-list push), an `AgentId` IS reused. Both directly contradict SPEC-02 R2 and the second sentence of SPEC-02 R10 ("After creating `k` agents, `next_id` MUST be incremented by `k`").
- **Impact if unresolved:** SPEC-02 remains the canonical source for the `Net` API. Downstream readers of SPEC-02 (task-splitter, test-generator, reviewer, qa) will continue to enforce R2/R10 verbatim, producing tasks/tests that fail under the new I3' semantics, OR silently break when they read a recycled ID and expect the old monotonicity. The contradiction is hard-block: two specs cannot both be authoritative on opposite claims.
- **Suggested resolution:** Add to SPEC-22 frontmatter: `Amends: …, SPEC-02 R2 (relaxes "never reused" to "uniqueness via free-list"), SPEC-02 R10 (relaxes "incremented by k" to "incremented by the count of non-recycle creations within a `create_agent` batch")`. Author a `§3.8 Amendments` block giving each of (SPEC-01 I3, SPEC-02 R2, SPEC-02 R10, SPEC-02 R12) a structured `Old text / New text / Rationale` triple, matching the SPEC-19 / SPEC-20 pattern.
- **Invariant affected:** I3 / D4 (the implementation-side counterpart of the theoretical invariant).
- **Pesquisador prediction:** **CONFIRMED and EXPANDED.** The brief flags R2; I am additionally calling out R10's second sentence as in scope, since SPEC-22 R3 explicitly violates it on the recycle path.

#### SC-003 — SPEC-04 and SPEC-05 missing from `Depends on:` despite explicit amendments

- **Axis:** A (frontmatter integrity) + D.
- **Location:** Frontmatter line 4 (`Depends on:`); §3.1 R10a (line 72), §3.1 R12 (line 76), §3.2 R22 (line 138).
- **Evidence:** Frontmatter says "Depends on: SPEC-02 (Net Representation), SPEC-01 (Invariants), SPEC-03 (Reduction Engine)." But:
  - R10a: "The `build_subnet()` operation (SPEC-04) SHOULD populate the free-list of each partition with the `None` slots that fall within that partition's ID range." — direct amendment of SPEC-04 §4.5.
  - R12: "The `merge()` operation (SPEC-05) MUST handle free-lists from multiple partitions." — direct amendment of SPEC-05 §4.2 merge algorithm.
  - R22: "`SparseNet` SHOULD be used as the representation during subnet construction in `build_subnet()` (SPEC-04, `src/partition/helpers.rs`)." — second amendment of SPEC-04.
  Verified against SPEC-04 line 377 (`build_subnet`) and SPEC-05 line 322 (`fn merge(plan: PartitionPlan) -> (Net, u32)`).
- **Impact if unresolved:** sdd-pipeline's predecessor-walk for SPEC-22 (per Relativist agent dispatch policy) will not load SPEC-04 / SPEC-05 into context. The task-splitter cannot generate the `partition/helpers.rs` and `merge/engine.rs` tasks that R10a/R12/R22 require. Round 1 review of cross-spec consistency would silently miss the amendment soundness check.
- **Suggested resolution:** Append SPEC-04 and SPEC-05 to `Depends on:`. If the author wishes to distinguish "depends on" (need to read) from "amends" (need to update), add an `Amends:` line listing SPEC-04 §4.5 (build_subnet), SPEC-05 §4.2 (merge), separately from the SPEC-01/SPEC-02 amendments tracked in SC-002.
- **Invariant affected:** None directly; the gap blocks the pipeline machinery before invariants are at risk.
- **Pesquisador prediction:** **CONFIRMED.**

#### SC-004 — No `§3.8 Amendments` block (SPEC-19 / SPEC-20 pattern)

- **Axis:** A + D + L.
- **Location:** §3 (Requirements) — sections 3.1, 3.2, 3.3, 3.4 exist; no §3.8.
- **Evidence:** SPEC-19 §3.8 and SPEC-20 §3.8 each carry a structured "Amendments to predecessor specs" block where each amendment is presented as `Target spec / R-number / Old text / New text / Rationale`. SPEC-22 lists amendments in the frontmatter (one line) and discusses the I3 relaxation prose-style in R24-R27, but there is no structured block. Combined with SC-002 (incomplete amendment scope) and SC-003 (missing Depends-on entries), this means the *amendments themselves are not auditable* without re-reading the requirements section line by line — which is what this review had to do.
- **Impact if unresolved:** especialista-specs (the only agent that may edit specs) cannot reverse-engineer the intended amendment surface from the frontmatter alone, so cross-spec integrity gradually drifts as later specs are written against SPEC-22's "true" amendment list (which is not currently extractable). Same risk for the theory-bridge maintainer, who indexes amendments per spec.
- **Suggested resolution:** Add §3.8 with one entry per amendment. At minimum: `(SPEC-01 I3 → I3')`, `(SPEC-02 R2)`, `(SPEC-02 R10)`, `(SPEC-02 R12)`, `(SPEC-04 §4.5 build_subnet)`, `(SPEC-05 §4.2 merge)`. Each entry must follow the SPEC-19/SPEC-20 four-field schema.
- **Invariant affected:** None directly; structural hygiene.
- **Pesquisador prediction:** **CONFIRMED.** Brief calls this a "soft blocker"; I am scoring it CRITICAL because (a) it intersects SC-002 and SC-003 — without §3.8 those gaps are easy to miss in Round 2 — and (b) the SPEC-19/SPEC-20 pattern was made canonical for v2 specs, so deviation is a normative gap.

### 2.2 HIGH

#### SC-005 — Free-list recycling × SPEC-19 `BorderGraph`: slot-id-stability gap

- **Axis:** F (invariant preservation) + I (forward compatibility).
- **Location:** §3.1 R7 (no-port-references invariant, line 64), R10 (per-worker ID range, line 70); SPEC-19 §3.2 R8-R12 (BorderGraph structure).
- **Evidence:** SPEC-19 R9 (verbatim): "the `BorderGraph` MUST store a `BorderState` containing: …" — the BorderState contains `side_a: PortRef` and `side_b: PortRef`, which encode `AgentPort(id, port)` against the *worker's local* AgentId space. SPEC-22 R7 only constrains the *port array* and *redex queue* and *root* of the local `Net`; the coordinator's `BorderGraph` is NOT part of the worker's `Net` and is NOT touched by `remove_agent`. SPEC-22 R10 constrains recycled IDs to the worker's `[start, end)` range, so the recycled ID is in the same space the BorderGraph indexes against — but R10 does NOT protect the recycled ID from being a target of an existing `BorderState.side_a == AgentPort(recycled_id, port)`. Concretely: round N partition produces border `B = (border_id, AgentPort(47, 0), AgentPort(123, 0))`. After local reduction in round N+1, ID 47 is recycled to a different agent type via free-list. The coordinator detects `B.is_redex == true` and dispatches a `CommutationBatch` indexing `AgentPort(47, 0)`. The worker's local `agents[47]` is now a different symbol; the rule applied differs from the rule the BorderGraph computed.
- **Impact if unresolved:** Direct G1 violation under delta mode (SPEC-19) and under SPEC-20 R24b-delta. If SPEC-22 R28 ("always-on by default, no feature gate") composes naively with SPEC-19 R0c (delta mode immutable per run), the system silently produces incorrect normal forms. ARG-005 INV-REC's induction is unsound because the basis assumes `(B_k, {N_w,k}) ~ μ_k` — but recycle breaks the slot-level isomorphism.
- **Suggested resolution:** Add R10b (HIGH-strength): "Free-list recycling MUST be disabled when delta mode is active (`GridConfig.delta_mode == true`), OR `remove_agent` MUST notify the coordinator via a new `RecycleNotice(border_ids_referencing_id)` border-delta variant such that the coordinator can invalidate `BorderState` entries whose endpoints reference the recycled id." Alternatively, restrict free-list to IDs that are NOT referenced in any partition's `border_id` graph (ID is "border-clean") — the worker can verify this locally because SPEC-04's `border_entries[i]` is partition-local. Whichever option is chosen, add an explicit cross-reference to SPEC-19 (and to ARG-005 INV-REC) in the rationale.
- **Invariant affected:** G1 (under SPEC-19 / SPEC-20); ARG-005 P7 (delta-reporting completeness); ARG-006 P12 (mixed-trace recoverability).
- **Pesquisador prediction:** **CONFIRMED and SHARPENED.** Brief identifies the issue (§5e, §7 [HIGH-2]); I add the concrete failure trace above and the cross-reference to ARG-005's INV-REC theorem.

#### SC-006 — `to_dense()` blindly populates free-list with ALL `None` slots; violates R10 in partition context

- **Axis:** E (algorithmic correctness).
- **Location:** §4.6, lines 474-479.
- **Evidence:** §4.6 `SparseNet::to_dense()`:
  ```rust
  // Populate free-list with None slots that fall within the ID range
  for i in 0..arena_len {
      if net.agents[i].is_none() {
          net.free_list.push(i as AgentId);
      }
  }
  ```
  The comment claims "within the ID range" but the loop iterates `0..arena_len` and pushes every `None` index. There is no `Range<AgentId>` parameter to `to_dense()`. R10a (line 72) says `build_subnet()` SHOULD populate the partition's free-list with `None` slots within the partition's ID range. R10 (line 70) says workers MUST NOT use free-list IDs outside their range. But `to_dense()` will push IDs from `[0, partition.id_range.start)` (gap before the partition) into the free-list. The next `create_agent` on the worker pops one of these out-of-range IDs and silently violates D4.
- **Impact if unresolved:** D4 (ID Uniqueness After Distributed Reduction) violation on every `to_dense()`-then-`create_agent` sequence in a partitioned context. R10 is unenforceable at the API surface as currently specified.
- **Suggested resolution:** Change `to_dense()`'s signature to `pub fn to_dense(&self, id_range: Option<Range<AgentId>>) -> Net`. When `id_range` is `Some`, the free-list is populated only with `None` indices within `[id_range.start, id_range.end)`. When `None`, current behavior is preserved (whole-net case). Update R20 and the §4.6 prose accordingly. Add a unit test in §7 (T_new): "`SparseNet::to_dense(Some(100..200))` produces a `Net` whose free-list is a subset of `[100, 200)`."
- **Invariant affected:** D4 (D4a hard MUST).
- **Pesquisador prediction:** **CONFIRMED via OQ-A and Connection 2 in the brief.** Brief raises this as an open question; I am promoting it to a HIGH finding because it's a concrete D4 violation, not a stylistic question.

#### SC-007 — Serde format change without SPEC-18 wire-version bump

- **Axis:** I (forward compatibility) + D.
- **Location:** §3.1 R9 (line 68); SPEC-18 versioning machinery (referenced).
- **Evidence:** R9: "The free-list MUST be included in serde serialization/deserialization of `Net` (SPEC-02 R24-R26). A deserialized net MUST have a valid free-list…" SPEC-22 does NOT mention SPEC-18 (`Wire Format v2`) or any `PROTOCOL_VERSION` bump. SPEC-02 R24-R26 define the bincode `Net` layout; adding a new `Vec<AgentId>` field changes that layout. Bincode's default behavior is positional/length-prefixed — appending a field at the end of `Net` with no version tag means a v1 deserializer reading a v2-serialized `Net` either (a) reads the `free_list` length as garbage and overruns, or (b) errors with `UnexpectedEof` if length-prefixing is exact. Neither is acceptable for a wire protocol where coordinator and workers may run different binaries during rolling upgrades.
- **Impact if unresolved:** Wire-protocol incompatibility between v1 and v2 binaries, with no version-negotiation pathway. Persisted `.bin` files (e.g., from `results/locked/v1_local_baseline/`) become unreadable.
- **Suggested resolution:** Add R9a: "The introduction of `free_list` in the `Net` serialized layout MUST be coordinated with SPEC-18 (`Wire Format v2`). The `PROTOCOL_VERSION` constant MUST be bumped to N+1 in conjunction with this change. v1 (`PROTOCOL_VERSION = 2`) deserializers MUST reject v2 (`PROTOCOL_VERSION = 3`) serialized nets with a clear `UnsupportedVersion` error. A migration path for persisted v1 binaries (where applicable) MUST be documented in §6 Migration Path." Cross-reference SPEC-18 in `Depends on:`.
- **Invariant affected:** R26 (round-trip identity, indirectly, across binary versions).
- **Pesquisador prediction:** **CONFIRMED via OQ-D and Connection 3 in brief.**

#### SC-008 — R23 ("MUST NOT use SparseNet in reduction hot path") is a non-testable performance directive

- **Axis:** G (testability).
- **Location:** §3.2 R23 (line 140).
- **Evidence:** R23: "`SparseNet` MUST NOT be used in the reduction hot path. The reduction engine (SPEC-03) relies on O(1) guaranteed indexed access… HashMap lookup has O(1) amortized complexity but with a 5-10x worse constant factor due to hashing and cache misses. The hybrid approach (R22: sparse for construction, dense for reduction) avoids this. **(MUST NOT)**" There is no test in §7 for R23; there cannot be a unit test, because the requirement is about design choice / call-graph topology, not about state at any point in time. The "5-10x worse" claim is unattributed (no benchmark, no AC reference for that specific factor).
- **Impact if unresolved:** R23 is unenforceable in CI. A future contributor could legitimately add a `SparseNet`-based reduction step and pass all tests. The MUST NOT becomes an honor-code rule.
- **Suggested resolution:** Either (a) demote R23 from a Requirement (§3) to a Design constraint (§4 / §5.2), removing the MUST NOT formally and documenting the design rationale; or (b) make R23 testable: define a "reduction hot path" lint (e.g., `src/reduction/**/*.rs` may not import `crate::net::sparse::SparseNet`) and reference that lint as the verification mechanism. Option (a) is cleaner; option (b) is enforceable in CI.
- **Invariant affected:** None.
- **Pesquisador prediction:** **CONFIRMED via [MEDIUM-2] in brief.** I am promoting to HIGH because untestable MUST NOT requirements are a chronic source of spec rot.

#### SC-009 — R22/R30 `SHOULD` is too weak given a known O(max_id) memory pathology

- **Axis:** L (rationale calibration) + K (configuration).
- **Location:** §3.2 R22 (line 138), §3.4 R30 (line 174).
- **Evidence:** R22 (SHOULD): use `SparseNet` during `build_subnet()` to avoid `vec![None; max_id + 1]` allocation. R30 (SHOULD): make this configurable via `sparse_build: bool`, default `true`. `relativist-core/src/partition/helpers.rs` confirms the dense allocation at L160-204 (per the brief's §5d). The TCC milestone M5 (`docs/next-steps.md`) targets `ep_con 100M` on a 2 GB coordinator; with `ContiguousIdStrategy`, `max_id` per partition can approach the global `next_id`, making `vec![None; 100_000_000]` ~ 800 MB per partition — pathological for the TCC scenario.
- **Impact if unresolved:** v2 ships with the dense-build pathology unfixed by default if a developer interprets SHOULD as "may skip". M5 milestone is unreachable.
- **Suggested resolution:** Promote R22 to MUST when `partition.id_range.end - partition.id_range.start > N_THRESHOLD * partition.live_agent_count` (i.e., when the dense arena would be more than `N_THRESHOLD` × the live-agent count; reasonable threshold: 4-8). Below threshold, leave as SHOULD. Alternatively, make R30's flag default to `true` AND mark `sparse_build = false` as REQUIRES_REVIEW (i.e., a user must explicitly opt out). Cite the M5 milestone in §5.
- **Invariant affected:** None directly, but indirectly threatens ARG-004 V4 (feasibility under workload conditions) by shipping with a known scaling pathology.
- **Pesquisador prediction:** **CONFIRMED via [MEDIUM-1] in brief.**

#### SC-010 — SPEC-03 reduction-engine assertions on `next_id` not amended

- **Axis:** D + F.
- **Location:** §3.1 R3, R24; SPEC-01 §4.3 `assert_next_id_valid` (line 639 in SPEC-02; SPEC-01's I3 verification clause).
- **Evidence:** SPEC-22 R24 (I3 → I3') states "`next_id` MUST be strictly greater than any `AgentId` ever assigned (whether currently live, in the free-list, or previously freed and re-assigned)." This preserves the `next_id > max(id_in_use)` property in spirit. However, SPEC-02's `assert_next_id_valid` (verbatim from SPEC-02 lines 639-649) asserts `(i as u32) < self.next_id` ONLY for slots where `slot.is_some()`. Under free-list recycling, `next_id` may equal the highest assigned ID + 1 even when the highest ID is currently in the free-list. The assertion still passes (the slot is `None`), so the existing assertion is consistent with I3'. **But:** SPEC-03 (reduction engine) uses `create_agent` from inside CON-DUP commutation. CON-DUP creates 4 new agents. If only 2 free-list slots are available, the order in which the 4 calls happen determines the `(recycled_id_1, recycled_id_2, fresh_id_3, fresh_id_4)` tuple. SPEC-22 §4.7 acknowledges this in the table but does not require any test that exercises CON-DUP under partial free-list availability. Worse, SPEC-03 may have debug assertions on the order of returned IDs (e.g., `assert!(new_id > old_max_id)`); these would fire under recycling.
- **Impact if unresolved:** Hidden assertion failures under existing SPEC-03 debug builds when free-list is non-empty during commutation. T7 ("Invariant T1 after recycling") in SPEC-22 §7 covers structural invariants but not SPEC-03's own assertion language.
- **Suggested resolution:** Audit SPEC-03 (and its implementation in `src/reduction/`) for any `assert!(new_id > X)` or `debug_assert!(net.next_id > k)` calls that assume monotonic returned IDs across `create_agent` within a single rule. Add SPEC-03 to `Depends on:` (already there per frontmatter — confirmed) but add a §3.8 amendment: "SPEC-03 debug assertions on `next_id` monotonicity across rule application MUST be reformulated as I3'-compatible (uniqueness, not monotonicity of returned IDs)."
- **Invariant affected:** I3' (when checked against legacy SPEC-03 assertions); T5 (CON-DUP topology, indirectly, under recycle).
- **Pesquisador prediction:** **CONFIRMED via §5c in brief.**

#### SC-011 — Q1 (`SparseNet` × `freeport_redirects`) deferred but load-bearing

- **Axis:** J (open question hygiene).
- **Location:** §8 Q1 (line 610-611).
- **Evidence:** Q1: "Should `SparseNet` support `freeport_redirects`? The current `Net` has a `freeport_redirects: HashMap<u32, PortRef>` field (used during merge for FreePort resolution). If `SparseNet` is used for partition construction, it may need this field. Alternatively, `freeport_redirects` could be external to both `Net` and `SparseNet`. **Decision deferred to implementation.**" R22 simultaneously says `SparseNet` SHOULD be used during `build_subnet()`. SPEC-04's `build_subnet` (line 377) produces partitions consumed by SPEC-05 `merge`, which calls `rebuild_free_port_index` (live code) using `freeport_redirects`. So Q1 is not optional; it gates whether R22 is implementable at all in delta mode.
- **Impact if unresolved:** "Decision deferred to implementation" means the developer makes an architectural decision at code-time that affects SPEC-19 / SPEC-05 contracts. This is exactly the kind of decision specs are supposed to lock down.
- **Suggested resolution:** Resolve Q1 in this spec. Two acceptable resolutions: (a) `SparseNet` includes `freeport_redirects: HashMap<u32, PortRef>` as a field, making conversion lossless; (b) explicitly forbid `SparseNet` use in any context that produces a partition consumed by `merge` or `BorderGraph::initialize` (i.e., R22 is restricted to non-grid construction paths). Resolution (a) is the lower-risk choice. Document chosen resolution in §3.2 and remove Q1 from §8.
- **Invariant affected:** D1c (FreePort bijectivity, ARG-002 C3).
- **Pesquisador prediction:** **CONFIRMED via Connection 1 in brief.**

### 2.3 MEDIUM

#### SC-012 — Missing AC citations in frontmatter despite body-text use

- **Axis:** A (frontmatter integrity).
- **Location:** Frontmatter line 7 (`References:`); §5.1 line 511-515; §5.2 line 522-523.
- **Evidence:** §5.2 says "HVM2 (AC-006) uses a flat array (`node[]`/`vars[]`) specifically for reduction speed. The Haskell prototype (AC-001) uses `Map AgentId Agent`…". Frontmatter `References:` line lists only `REF-002, REF-003, REF-014`. Theory bridge (`docs/theory-bridge.md`) lists AC-001 (Haskell IC.Core), AC-006 (HVM2 Types + Memory), AC-007 (HVM2 Reduction), AC-009 (HVM4 Term + Heap), AC-011 (HVM4 Threading + Work-Stealing), AC-015 (Cross-Cutting Synthesis) — all relevant; only AC-001 and AC-006 are body-text-cited.
- **Impact if unresolved:** Theory-bridge audit drift. The bridge's "spec-critic" usage rule says "If an ARG/DISC/REF/AC ID not in this file is cited, flag as unresolved." Inverse: AC IDs cited in body but not declared in frontmatter make the audit asymmetric.
- **Suggested resolution:** Replace `References: REF-002 (Lafont 1997), REF-003 (HVM2 — arena management), REF-014 (Kahl — GC impact on parallel reduction)` with the SPEC-02-style multi-line:
  ```
  References consumed: REF-002, REF-003, REF-014
  Code analyses consumed: AC-001, AC-006, AC-011 (free-list ↔ HVM4 static heap partitioning), AC-015 (CC-4 ID space)
  ```
  Optionally also `Arguments consumed: ARG-002 (C1-C3 partitioning, since R22 affects `build_subnet`)`.
- **Invariant affected:** None.
- **Pesquisador prediction:** **CONFIRMED via §3 (theory bridge audit) in brief.**

#### SC-013 — Theory bridge has stale "SPEC-22 (Job submission)" tag

- **Axis:** A (frontmatter integrity, downstream).
- **Location:** `docs/theory-bridge.md` line 142 (DISC-012 v2 entry).
- **Evidence:** Verbatim: "Informs: SPEC-22 (Job submission), SPEC-23 (Encoding pipeline), SPEC-25 (Recipe generation)." But SPEC-22 is "Arena Management and Memory Efficiency" — has nothing to do with job submission. This is a theory-bridge metadata error from when SPEC-22 was a different topic in earlier drafts.
- **Impact if unresolved:** sdd-pipeline / pesquisador may follow this DISC-012 → SPEC-22 link expecting job-submission content and find arena management instead. Wastes context-budget; could mislead Round 2 reasoning.
- **Suggested resolution:** This is a TCC-root cleanup task, NOT a SPEC-22 author task. Surface to TCC-root via the pesquisador handoff: edit `docs/theory-bridge.md` line 142 to remove "SPEC-22 (Job submission)" or remap it to whatever spec actually consumes DISC-012 v2's job-submission content (likely a future SPEC-25 or SPEC-26). SPEC-22 itself takes no action.
- **Invariant affected:** None.
- **Pesquisador prediction:** **CONFIRMED via §6 Identified Gaps #9.** Tagged correctly as "TCC-root cleanup, not SPEC-22 fault" by the brief, and I concur.

#### SC-014 — `next_id` on `to_sparse()` round-trip not monotonicity-preserving

- **Axis:** E (algorithmic correctness) + H (boundary cases).
- **Location:** §4.6 `to_sparse()` (line 423-445), `to_dense()` (line 452-483); §3.2 R21 (round-trip).
- **Evidence:** R21 says `Net::to_sparse().to_dense()` MUST produce a structurally-equal net "modulo `None` slots that are trimmed: the resulting dense net has no trailing `None` slots beyond `max_id`." Counterexample: original `Net` has `next_id = 100`, `agents.len() = 100`, but only ID 0-49 are live (50-99 are all `None` from removals). `to_sparse()` copies `next_id = 100` correctly. `to_dense()` computes `max_id = 49` (from sparse.agents.keys().max()), so `arena_len = 50`. The result has `next_id = 100` (preserved) but `agents.len() = 50`. Now `next_id = 100 > agents.len() = 50`. SPEC-02 §4.5.2 `create_agent` resizes the arena to fit `next_id`, so the next `create_agent` would `agents.resize(101, None)` and write at `agents[100]`. This works BUT the assertion `assert!((i as u32) < self.next_id)` (SPEC-02 line 642) still passes. So the round-trip is not bit-exact, but is "behaviorally equal" — it's worth specifying which one R21 means.
- **Impact if unresolved:** R21's "structurally equal" is ambiguous. Tests that compare via serde + bincode round-trip will FAIL because the byte-level representation differs (different `agents.len()`, different `ports.len()`). T14 in §7 ("Build a dense `Net` with 10 agents (including some `None` slots from removals). Convert to `SparseNet`, then back to `Net`. Assert the result is structurally equal to the original modulo trailing `None` slots and free-list population.") attempts to handle this but is informal.
- **Suggested resolution:** Define R21 precisely: "The conversion `Net::to_sparse().to_dense()` produces a net that is *behaviorally equal* (same observable post-state for any sequence of create_agent / remove_agent / connect / disconnect / get_target operations) but NOT byte-equal. Specifically, the result MAY have shorter `agents` and `ports` vectors if the original had trailing `None` slots and unused port slots." Add a helper `Net::is_behaviorally_equal(&self, other: &Net) -> bool` in §4.6 that captures this relation. Update T14 to use this helper.
- **Invariant affected:** None at runtime; specification clarity issue.
- **Pesquisador prediction:** Brief did not flag. Identified independently.

#### SC-015 — Free-list memory cap deferred ("no cap, 40 MB acceptable") without scaling argument

- **Axis:** L (rationale calibration).
- **Location:** §8 Q3 (line 614).
- **Evidence:** Q3: "For a net with 10M agents that are all annihilated, the free-list holds 10M `u32` entries = 40 MB. This is small compared to the arena itself (10M * `Option<Agent>` = ~80 MB), but not negligible. Should the free-list have a maximum size, beyond which freed IDs are simply discarded? **Tentatively: no cap. 40 MB for 10M agents is acceptable. Revisit if benchmarks show otherwise.**" 100M agents (per M5 milestone) → 400 MB free-list, on top of 800 MB arena (vec<Option<Agent>>) — total 1.2 GB per partition for an `ep_con 100M` workload. This exceeds the 2 GB coordinator memory budget cited in `docs/next-steps.md` M5 if not partitioned. The brief's §6 OQ-A also touches this.
- **Impact if unresolved:** TCC milestone M5 (`ep_con 100M` on 2 GB coordinator) is at risk. The "tentative no cap" decision was made for a 10M-scale scenario without re-validating against 100M.
- **Suggested resolution:** Reframe Q3 to commit to a scaling rule: "Free-list size is bounded by `partition.live_agent_count * 4` bytes worst-case. For partitions exceeding `MAX_FREELIST_BYTES = 64 MB`, the free-list MUST switch to a `BTreeSet<AgentId>` (smaller in pathological growth) or compact bitmap representation." Or explicitly state "Free-list memory cap is a SPEC-23 (compact memory) concern; SPEC-22 leaves it uncapped as v1 free-list semantics."
- **Invariant affected:** None directly.
- **Pesquisador prediction:** Brief touches OQ-A; my SC-015 is partially overlapping but focuses on the M5 scaling argument the brief did not surface.

#### SC-016 — Thread-safety / `Send` + `Sync` not addressed

- **Axis:** I (forward compatibility) + H.
- **Location:** §4.4 `SparseNet` definition (line 295-313). No mention of `Send`/`Sync` anywhere in SPEC-22.
- **Evidence:** SPEC-22 introduces `SparseNet` containing `HashMap<AgentId, Agent>` and `HashMap<(AgentId, PortId), PortRef>` — both `Send + Sync` if their contents are. The current `Net` is `Send + Sync` because all fields are. SPEC-22 R22 proposes building partitions in parallel via `SparseNet`. If `build_subnet` is called in parallel for different partitions, each partition's `SparseNet` must be `Send`-able. The spec does not declare or test this.
- **Impact if unresolved:** A future task to parallelize `build_subnet` (e.g., via `rayon`) may discover at code-time that `SparseNet` cannot be sent across thread boundaries because of some derive limitation. Specifying it now closes the door.
- **Suggested resolution:** Add R18a: "`SparseNet` MUST be `Send + Sync` (statically verifiable via a `static_assertions::assert_impl_all!(SparseNet: Send, Sync);` or equivalent compile-time check). Same for `Net` after the `free_list` field is added."
- **Invariant affected:** None.
- **Pesquisador prediction:** **CONFIRMED via OQ-B in brief.**

#### SC-017 — `unsafe` boundary not addressed (forward to SPEC-23)

- **Axis:** I + L.
- **Location:** Whole spec; cf. SPEC-23 (forward reference).
- **Evidence:** CLAUDE.md mandates `// SAFETY:` comment on any `unsafe` block. SPEC-22 introduces no `unsafe` (its design sketches are entirely safe Rust). SPEC-23 (`Amends: SPEC-22`) introduces bit-packed `PortRef = u32` with accessor methods that may require `unsafe transmute` between integer and enum forms. SPEC-22 should explicitly affirm that its design is unsafe-free, so that SPEC-23 takes responsibility for the first `unsafe` boundary in `net/types.rs`.
- **Impact if unresolved:** A developer implementing SPEC-22 may accidentally use a pattern (e.g., `*const u32` access into a HashMap bucket) that would conflict with SPEC-23's bit-packed migration.
- **Suggested resolution:** Add to §4 prose or §5: "SPEC-22 implementations MUST be expressible in safe Rust. Any `unsafe` blocks are out of scope for SPEC-22 and deferred to SPEC-23 (`Compact Memory Representation`)."
- **Invariant affected:** None.
- **Pesquisador prediction:** **CONFIRMED via OQ-C in brief.**

### 2.4 LOW

#### SC-018 — R6 ("free-list MUST NOT contain duplicates") underspecifies enforcement granularity

- **Axis:** G (testability).
- **Location:** §3.1 R6 (line 62).
- **Evidence:** "In debug mode, this invariant SHOULD be verified by assertion." SHOULD, not MUST. T10 in §7 (line 587) tests via "direct manipulation in test" — implying the production `remove_agent` path cannot trigger duplicates. But §4.3 `remove_agent` shows no debug assertion guarding against `agents[id].is_some()` being false at entry, which would be the duplicate-add path. The spec is internally consistent (R6 → R7 chain) but the enforcement is hand-waved.
- **Impact if unresolved:** A future contributor introducing a "soft remove" path could double-push without any debug assertion firing.
- **Suggested resolution:** Strengthen R6's last sentence to "In debug mode, this invariant MUST be verified by assertion in `remove_agent`: before pushing `id` to the free-list, `debug_assert!(!self.free_list.contains(&id))`. Alternatively, an O(1) `HashSet<AgentId>` shadow may be maintained in debug builds." The HashSet shadow would be cleaner; either is acceptable.
- **Invariant affected:** R6 itself.

#### SC-019 — §4.7 effects table loses precision for CON-DUP free-list interaction

- **Axis:** L.
- **Location:** §4.7 line 495 (CON-DUP row).
- **Evidence:** "CON-DUP Commutation | 2 | 4 | -2 from free-list, then +2 new IDs if free-list was depleted". This describes the steady state but ignores that the order of `remove_agent` (push 2 IDs) and the 4 `create_agent` calls determines whether the recycled IDs are used. If CON-DUP first removes both agents (push id_a, id_b), then creates 4, then create #1 pops id_b, create #2 pops id_a, create #3 and #4 use fresh IDs. If the rule order is interleaved (remove id_a, create C1 reuses id_a, remove id_b, create C2 reuses id_b, create C3 fresh, create C4 fresh), the result is the same agent count but different ID assignments. The spec does not pin down the intra-rule order.
- **Impact if unresolved:** Two correct implementations of CON-DUP may produce non-byte-equal nets (different recycled-ID assignments) that are nonetheless isomorphic. Tests that assert specific IDs (T6 in §7 line 578, "Assert `next_id == 4`") would pass under both implementations but tests that assert which agent is at ID `k` would fail nondeterministically.
- **Suggested resolution:** State explicitly in §4.7 (or §5.3): "The intra-rule order of `remove_agent` and `create_agent` calls within a CON-DUP commutation is implementation-defined. SPEC-22 only guarantees the steady-state counts shown in the table. Tests MUST NOT assert specific recycled-ID assignments within a single rule."
- **Invariant affected:** None (already implied by I3' uniqueness rather than monotonicity).

#### SC-020 — T2 in §7 ("LIFO ordering") asserts implementation-defined specific IDs

- **Axis:** G.
- **Location:** §7.1 T2 (line 570).
- **Evidence:** "Create 5 agents (IDs 0-4), remove IDs 1, 3, 2 (in that order). Create 3 new agents. Assert they receive IDs 2, 3, 1 (LIFO pop order). Assert `next_id` is unchanged at 5." This assertion is correct under R5 (LIFO), but it relies on R5 being preserved as a strict ordering guarantee. R5 is currently MUST. Cross-check passed.
  However: the test description couples LIFO (R5) with a specific test that would break if R5 were ever weakened (e.g., changed to "implementation-defined ordering"). That's intentional. But if SC-019's resolution says "intra-rule order is implementation-defined", then T2 (which is single-threaded test code, not a rule) is fine — flag for clarity not for incorrectness.
- **Impact if unresolved:** Minor coupling between T2 testability and R5 LIFO commitment. Not a defect.
- **Suggested resolution:** Add a sentence to T2: "T2 verifies the LIFO contract from R5. T2 is NOT a guarantee about ID ordering inside reduction rules (see SC-019 resolution)."
- **Invariant affected:** None.

#### SC-021 — Forward-compat note for SPEC-23 (compact memory) missing

- **Axis:** I.
- **Location:** Whole spec. SPEC-23 frontmatter says `Amends: SPEC-22`.
- **Evidence:** SPEC-23 will replace `enum PortRef` with `pub struct PortRef(u32)`. SPEC-22's `SparseNet::ports: HashMap<(AgentId, PortId), PortRef>` will need a different key type after SPEC-23 (or the same key type interpreted differently). SPEC-22 does not flag this forward-compatibility concern.
- **Impact if unresolved:** SPEC-23 will need to amend SPEC-22 anyway (the frontmatter already says so), so the issue will surface. But the forward-compat note belongs in SPEC-22 to scope the work.
- **Suggested resolution:** Add to §8 Q5 (new): "SPEC-23 will migrate `PortRef` to a bit-packed `u32`. The `SparseNet::ports` HashMap key may benefit from migrating to `(AgentId, PortId)` packed into a single `u32` at that time. SPEC-22 ships with semantic enum keys; the compact key migration is gated on SPEC-23 landing."
- **Invariant affected:** None.

---

## 3. Cross-spec consistency audit

Per the prompt, every R-number in SPEC-22 that names another spec was verified against the target spec text. Findings:

| SPEC-22 ref | Target | Verbatim target text | Verdict |
|-------------|--------|----------------------|---------|
| R2 / §4.3 — "as per SPEC-02 R12" | SPEC-02 R12 | "MUST mark the agent's slot as `None`, disconnect all its ports from the port array, and NOT reuse the ID." | **CONTRADICTION** — SPEC-22 R2 reuses the ID via free-list. Amendment declared in frontmatter but never written as a structured `Old/New text` block. (See SC-002, SC-004.) |
| R3 / §4.2 — "existing behavior per SPEC-02 R11" | SPEC-02 R11 | "MUST create a new agent with the next available ID, insert it into the agent arena (expanding if necessary), and return the assigned `AgentId`. Expected complexity: O(1) amortized." | **PARTIAL CONTRADICTION** — R11 says "the next available ID" which under I3 means `next_id`. Under SPEC-22 R3, "next available" can be a free-list slot. R11 needs clarification or amendment. |
| R7 — "ports for the recycled ID … MUST contain DISCONNECTED" | SPEC-02 §4.4, SPEC-01 I6 | "DISCONNECTED is transient. After each reduction rule, no port of a live agent may contain DISCONNECTED..." | **OK** — SPEC-22 R7 is compatible with the transient nature; the recycled ID's ports are DISCONNECTED at the moment of reuse (just before connections are re-established by `connect`). |
| R9 — "free-list MUST be included in serde serialization (SPEC-02 R24-R26)" | SPEC-02 R24-R26 | "MUST be serializable for transmission... format MUST be self-contained... preserve identity." | **AMENDMENT NEEDED** — adding a field changes the serialized layout; SPEC-18 wire-version bump unaddressed (see SC-007). |
| R10 — "ID space partitioning (SPEC-04 R16-R19, SPEC-01 D4)" | SPEC-04 R16-R19 (verified §4.7 line 417-439) | Static `[i*chunk_size, (i+1)*chunk_size)` partitioning. | **OK** — R10 is consistent with SPEC-04 §4.7. |
| R10a — "`build_subnet()` operation (SPEC-04)" | SPEC-04 §4.5 line 377 | `build_subnet(net, worker_agents[i], sigma, border_entries[i])` | **OK structurally** — but SPEC-04 not in `Depends on:` (SC-003); R10a is a SHOULD that SHOULD be MUST given SC-009. |
| R12 — "`merge()` operation (SPEC-05) MUST handle free-lists from multiple partitions" | SPEC-05 §4.2 line 322 `fn merge(plan: PartitionPlan) -> (Net, u32)` | Current merge does not touch free-lists. | **AMENDMENT NEEDED, NOT WRITTEN** — SPEC-05 not in `Depends on:` (SC-003); R12's "MUST handle" is unimplementable without an explicit SPEC-05 amendment specifying *how*. R12 says "the resulting net's free-list MUST contain only IDs that correspond to `None` slots in the merged arena" — this is an O(N) post-merge scan that SPEC-05 is silent on. |
| R22 — "`build_subnet()` (SPEC-04, `src/partition/helpers.rs`)" | Same | Same. | **OK** but SHOULD too weak (SC-009). |
| R23 — "reduction engine (SPEC-03)" | SPEC-03 | (general reference) | **OK structurally** but R23 untestable (SC-008). |
| R24 — "Invariant I3 (Monotonicity, SPEC-01) MUST be relaxed" | SPEC-01 I3 verbatim line 289-296 | "next_id MUST be strictly greater than any AgentId currently in use... IDs are never reused." | **AMENDMENT WRITTEN** — R24 provides the I3' replacement text. This is the one amendment SPEC-22 actually got right structurally. |
| R25 — "D4 (ID Uniqueness After Distributed Reduction, SPEC-01)" | SPEC-01 D4 verbatim line 208-216 | Per-partition disjointness. | **OK** — R10's per-worker constraint preserves D4. |
| R26 — "T1 (Port Linearity), I1 (Bidirectional Consistency), I2 (Reference Validity) MUST hold for SparseNet" | SPEC-01 T1, I1, I2 | All present. | **OK** — adapted formulations are sound. I6 (ERA cleanliness) mentioned in R17 covers the SparseNet equivalent. |
| §5.2 — "AC-006 / AC-001" | theory-bridge.md | Both present. | **OK semantically** but not declared in frontmatter (SC-012). |

**Summary:** 4 contradictions / amendments-needed-but-not-written, 2 frontmatter omissions (already filed as SC-001, SC-002, SC-003, SC-004, SC-007, SC-012). No invented R-numbers.

---

## 4. Theory-bridge audit

Every ARG/DISC/REF/AC ID cited in SPEC-22 was checked against `docs/theory-bridge.md`:

| SPEC-22 citation | Where in SPEC-22 | Resolves in bridge? | Notes |
|------------------|------------------|---------------------|-------|
| REF-002 (Lafont 1997) | Frontmatter | YES — Foundations § | OK |
| REF-003 (HVM2 — arena management) | Frontmatter | YES — Implementation/Technique § | OK |
| REF-014 (Kahl — GC impact on parallel reduction) | Frontmatter | YES — Implementation/Technique § | OK |
| AC-001 | §5.2 body | YES — Haskell Prototype § | NOT in frontmatter (SC-012) |
| AC-006 | §5.2 body | YES — HVM2 § | NOT in frontmatter (SC-012) |

**Downstream theory-bridge cleanup item:** DISC-012 v2's "Informs" line lists "SPEC-22 (Job submission)". SPEC-22 is Arena Management. Stale tag from earlier draft naming. Filed as SC-013 with TCC-root remediation path. Not SPEC-22's responsibility.

**Net theory-bridge audit verdict:** clean for ARG/DISC/REF; AC frontmatter incomplete; DISC-012 → SPEC-22 tag stale (downstream).

---

## 5. Invariant audit

SPEC-22 explicitly amends I3 (R24 → I3'). Let me audit each layer:

**T-layer (theoretical, T1-T7):**
- **T1 (Port Linearity):** Preserved. Free-list slots have all ports DISCONNECTED (R4(b), R7); the recycled agent's ports are re-established by `connect` calls in the rule that creates it, exactly as the non-recycle path. T1 is checked post-rule per existing SPEC-02 R20. **No threat.**
- **T2 (Principal-port interaction), T3 (Disjointness), T4 (Strong confluence), T5 (Rule correctness):** Independent of ID allocation strategy. **No threat.**
- **T6 (Termination-preserving), T7 (Confluence-derived determinism):** Independent of ID allocation. **No threat.**

**D-layer (distributed):**
- **D1 (FreePort bijectivity, including D1c):** The `freeport_redirects` field is part of D1c's machinery. SC-001 (struct omission) and SC-011 (Q1 deferred) directly threaten D1c. **THREATENED — see SC-001, SC-011.**
- **D2 (Border completeness):** Free-list × `BorderGraph` interaction (SC-005) threatens D2 under delta mode. **THREATENED.**
- **D3 (Cross-round border discovery):** Same. **THREATENED — see SC-005.**
- **D4 (ID Uniqueness After Distributed Reduction):** Preserved in spirit by R10/R25; threatened in code by SC-006 (`to_dense()` populates free-list with out-of-range IDs). **THREATENED — see SC-006.**
- **D5 (Sequential merge ordering):** Independent. **No threat.**
- **D6 (Protocol termination):** Independent. **No threat.**

**I-layer (implementation):**
- **I1 (Bidirectional consistency):** Preserved by `connect` semantics; recycle path defensively re-initializes ports to DISCONNECTED. **No threat.**
- **I2 (Reference validity):** Preserved; R7 explicitly forbids dangling references to free-list IDs. **No threat.**
- **I3 (Monotonicity):** **EXPLICITLY AMENDED to I3' (Uniqueness).** R24 is structurally correct. The text is sound. The remaining concern is that SPEC-02 R2 / R10 still carry old I3 phrasing (SC-002).
- **I4 (Stale-redex tolerance):** Independent. **No threat.**
- **I5 (BSP isolation):** Independent. **No threat.**
- **I6 (ERA Auxiliary Slot Cleanliness):** R17 introduces a SparseNet-specific equivalent. **OK** (but the equivalent should be assigned a number, e.g., I6-sparse, or a sentence in I6 itself amended to cover the sparse case).
- **I7 (Root Port Consistency):** R20 (`to_dense`) copies `root` directly. **No threat.**

**G-layer:**
- **G1 (Equivalence between local and distributed):** Threatened by SC-001 (FreePort redirects), SC-005 (BorderGraph slot recycling), and SC-006 (out-of-range free-list IDs). Three independent vectors of attack on G1. **THREATENED — multi-vector.**

**Summary:** SPEC-22 nominally amends only I3. In practice, the spec touches D1c, D2, D3, D4, and G1 — five additional invariants — without acknowledging the touch. SC-001, SC-005, SC-006, SC-011 are the patches needed to close those threats.

---

## 6. Untestability catalog

Requirements where the assertion is fundamentally non-deterministic, untestable in isolation, or relies on undefined-by-spec implementation choices:

| Req | Untestability reason | Severity | Resolution |
|-----|----------------------|----------|------------|
| R23 | "MUST NOT be used in the reduction hot path" — performance/structural directive, no point-in-time state to assert. | HIGH (SC-008) | Demote to design constraint or replace with import-graph lint. |
| R28 | "Negligible overhead" — performance claim, untestable as a unit assertion. | LOW | Acceptable as rationale; ensure it is not phrased as a MUST. (Currently MUST in line 170 — should be downgraded to SHOULD or moved to §5.) |
| R30 | "Configurable via `sparse_build: bool`" — testable only insofar as a config flag exists; the default value claim "(true)" is testable at construction time. | OK once SC-009 resolves the SHOULD/MUST tension. |
| §4.7 table | Per-rule free-list effects are stated as steady-state counts. The intra-rule order is unspecified (SC-019). Tests must avoid asserting specific recycled-ID assignments. | LOW (SC-019) | Add explicit non-determinism note. |
| R6 SHOULD | "In debug mode, this invariant SHOULD be verified by assertion." Vague enforcement granularity (SC-018). | LOW | Strengthen to MUST with explicit assertion location. |

---

## 7. Specialist self-flagged zones (RZ-N)

SPEC-22 carries 4 explicit "deferred / open" zones in §8 (Open Questions). Audit:

- **Q1 (RZ-1 — `freeport_redirects` × SparseNet):** Tagged "Decision deferred to implementation." **REJECT defer.** This is load-bearing for grid integration. Filed as SC-011 (HIGH, must be resolved in spec).
- **Q2 (RZ-2 — `NetOps` shared trait):** Tagged "Decision deferred to spec review. If adopted, separate spec amendment." **ACCEPT defer.** This is a refactoring concern; SPEC-22 doesn't need it to be implementable. The `Decision deferred` is cleanly scoped.
- **Q3 (RZ-3 — Free-list memory cap):** Tagged "Tentatively no cap." **PARTIAL REJECT.** Filed as SC-015 (MEDIUM): the 10M scenario was used; M5 requires 100M scaling check. Either commit to no-cap with M5 justification or commit to a cap.
- **Q4 (RZ-4 — Free-list sorted for determinism):** Tagged "No sorting needed." **ACCEPT.** Reasoning is sound (T7/G1 deterministic up to isomorphism, not up to specific IDs). Test brittleness concern (existing 1181 tests asserting specific IDs) is real but is a v1 → v2 test-update task, not a SPEC-22 defect. Recommend: add a one-line acknowledgement to Q4 ("Existing v1 tests asserting specific AgentId values may need adjustment; this is task-splitter scope, not spec scope.").

---

## 8. Mandatory vs Recommended

**MANDATORY (must fix before implementation begins):**

- SC-001 — Add `freeport_redirects` to §4.1 struct (or disclaimer + §4.6 fix).
- SC-002 — Amend SPEC-02 R2 and SPEC-02 R10 in addition to R12.
- SC-003 — Add SPEC-04, SPEC-05 (and SPEC-18 per SC-007) to `Depends on:`.
- SC-004 — Author §3.8 Amendments block.
- SC-005 — Add R10b (or equivalent) addressing free-list × `BorderGraph` slot-id stability.
- SC-006 — Fix `to_dense()` ID-range filtering or document the divergence.
- SC-007 — Coordinate serde format change with SPEC-18 PROTOCOL_VERSION bump.
- SC-008 — Demote R23 to design constraint OR replace with CI-enforceable lint.
- SC-009 — Promote R22 to MUST under defined memory threshold OR document the SHOULD justification against M5.
- SC-010 — Verify SPEC-03 assertions are I3'-compatible.
- SC-011 — Resolve Q1 (`SparseNet` × `freeport_redirects`).

**RECOMMENDED (should fix; spec is implementable without):**

- SC-012 — Add AC-001, AC-006 to `Code analyses consumed:` frontmatter.
- SC-013 — TCC-root cleanup of theory-bridge DISC-012 stale tag (not SPEC-22's responsibility).
- SC-014 — Define R21 round-trip equivalence precisely.
- SC-015 — Validate free-list memory cap against M5 (100M).
- SC-016 — Declare `Send + Sync` for `SparseNet`.
- SC-017 — Affirm SPEC-22 implementations are unsafe-free.
- SC-018 — Strengthen R6 enforcement granularity.
- SC-019 — Document intra-rule order non-determinism.
- SC-020 — Annotate T2 with R5 coupling note.
- SC-021 — Add SPEC-23 forward-compatibility note.

---

## 9. Checklist

### Consistency
- [x] All terms match SPEC-00 definitions
- [ ] Type signatures compatible with predecessor specs (FAIL — SC-001)
- [ ] No contradictions with predecessor requirements (FAIL — SC-002, SC-003, SPEC-02 R2/R10/R12)
- [ ] Data flow assumptions match predecessor outputs (FAIL — SC-006 `to_dense()` × R10)

### Testability
- [ ] Every MUST requirement has a testable criterion (FAIL — R23 SC-008)
- [x] Boundary conditions defined (0, 1, MAX) — covered by T1-T18
- [ ] Error conditions specified (PARTIAL — R6 enforcement vague, SC-018)

### Completeness
- [x] Pseudocode provided for non-trivial operations
- [ ] All edge cases documented (FAIL — SC-005 BorderGraph × recycle, SC-019 intra-rule order)
- [x] Rust type signatures for all public types/functions
- [ ] No undefined terms or dangling references (FAIL — SC-001 `freeport_redirects` undefined in spec but referenced by predecessors)

### Invariant Preservation
- [x] T1-T7 maintained by all operations
- [ ] D1-D6 maintained by all operations (FAIL — D1c via SC-001/SC-011, D2/D3 via SC-005, D4 via SC-006)
- [x] I1-I7 maintained by all operations (I3 explicitly amended to I3')
- [ ] G1 not violatable by any valid operation sequence (FAIL — three vectors via SC-001, SC-005, SC-006)

---

## 10. Verdict

**BLOCK.** Round 2 (especialista-specs) is required to address all 11 MANDATORY findings before any task-splitter or test-generator pass. The recommended findings (10 items) should be folded into the same Round 2 to avoid a third pass.

The spec's *core idea* (free-list + SparseNet, hybrid construction/reduction strategy) is sound and well-motivated. The blocker is execution: SPEC-22 is written as if it stands alone, but it touches at least 5 predecessor specs (SPEC-01, SPEC-02, SPEC-04, SPEC-05, SPEC-18) and 1 forward spec (SPEC-19's BorderGraph). Once the amendment surface is properly declared (SC-002, SC-003, SC-004) and the cross-spec edge cases are closed (SC-001, SC-005, SC-006, SC-011), this spec should reach CONDITIONAL_PASS in Round 2.

---

**End of Round 1.**
