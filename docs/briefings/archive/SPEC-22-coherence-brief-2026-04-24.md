# BRIEF: SPEC-22 Arena Management — Coherence Brief for spec-critic Round 1

**Generated:** 2026-04-24
**Scope:** Wave 2 / v2 Pre-DEV Spec Pipeline — SPEC-22 coherence brief for adversarial Round 1 review
**Downstream consumer:** `spec-critic` (adversarial review agent)

---

## Executive Summary

SPEC-22 specifies two independent memory-efficiency mechanisms for the `Net` type: (1) an arena free-list that recycles `AgentId` slots vacated by `remove_agent`, and (2) a `SparseNet` type backed by `HashMap` for use during construction and partitioning. The spec is self-coherent in isolation but has five structural issues that spec-critic must probe. The most serious: SPEC-22's `Net` struct definition in §4.1 omits the `freeport_redirects: HashMap<u32, PortRef>` field that already exists in the live codebase (`relativist-core/src/net/core.rs` L61), meaning the spec-as-written would silently delete a load-bearing field if implemented literally. Additionally, SPEC-22 lacks a `§3.8 Amendments` section — it amends SPEC-01 I3 and SPEC-02 R12 in the frontmatter but specifies those amendments inside §3.3 rather than in a dedicated amendment block, inconsistent with the pattern established by SPEC-19 and SPEC-20. The theory bridge audit is **clean**: all cited IDs (REF-002, REF-003, REF-014) are present in `docs/theory-bridge.md`, but two AC-NNN identifiers listed in the brief's predicted natural anchors (AC-006, AC-007, AC-009, AC-011, AC-015) are NOT cited in SPEC-22 at all — a missed citation opportunity that weakens the spec's justification for the hybrid dense/sparse design.

---

## Relevant Context

### 1. Spec Scope Summary

SPEC-22 defines two independent features under the banner of "Arena Management and Memory Efficiency" (§1):

**Feature 1 — Arena Recycling via Free-List (ROADMAP 2.33):** Adds a `free_list: Vec<AgentId>` field to the existing `Net` struct. When `remove_agent` is called, the freed `AgentId` is pushed onto the free-list (LIFO stack). When `create_agent` is called, it pops from the free-list before falling back to `next_id` increment. This prevents the agent arena from growing indefinitely during reduction workloads with agent turnover (e.g., CON-DUP commutation). The free-list is always-on (R28, no feature gate), partition-local, and constrained to each worker's assigned ID range (R10). Amends SPEC-01 I3 (relaxes monotonicity to uniqueness — I3') and SPEC-02 R12 (extends `remove_agent` to push freed IDs).

**Feature 2 — Sparse Net Representation (ROADMAP 2.32):** Defines a new `SparseNet` type (in `src/net/sparse.rs`) with `agents: HashMap<AgentId, Agent>` and `ports: HashMap<(AgentId, PortId), PortRef>` (R13). Provides conversion operations `Net::to_sparse()` and `SparseNet::to_dense()` with round-trip identity (R21). Intended for `build_subnet()` in SPEC-04 to avoid the memory inflation of dense arena allocation for sparse ID ranges (R22). Explicitly forbidden in the reduction hot path (R23).

The two features share the module (`net/`) but are otherwise independent — R28/R29 state both are always-on.

`(source: specs/SPEC-22-arena-management.md, §1, §3.1, §3.2, §3.3, §3.4)`

---

### 2. Predecessor / Amendment Graph

SPEC-22 declares in its frontmatter:

| Relation | Target |
|----------|--------|
| Depends on | SPEC-02 (Net Representation), SPEC-01 (Invariants), SPEC-03 (Reduction Engine) |
| Amends | SPEC-01 I3 (relaxed), SPEC-02 R12 (extended) |
| ROADMAP items | 2.33 (Arena Recycling), 2.32 (Sparse Net) |

`(source: specs/SPEC-22-arena-management.md, frontmatter)`

**Amendment placement issue:** The amendments to I3 and R12 are specified in §3.3 (Requirements, Invariant Amendments) as R24 (I3 → I3'), R25 (D4 compatibility), R26 (T1/I1/I2 for SparseNet), and R27 (debug assertions). There is NO `§3.8 Amendments` section with a structured amendment block (target spec, requirement number, old text, new text, rationale). Compare SPEC-19 and SPEC-20, which established the `§3.8` pattern for the v2 spec pipeline. This structural inconsistency is a direct gap for the task-splitter, who needs amendment tasks to point at specific SPEC/R-number pairs.

**Natural insertion points in v1 specs not listed as dependencies:**

- **SPEC-04 partition/helpers.rs `build_subnet()`** — SPEC-22 R10a and R22 both reference `build_subnet()` but SPEC-04 is not listed in the frontmatter's `Depends on`. The `SparseNet` hybrid approach (R22, R30) is a direct amendment to `build_subnet()` behavior in SPEC-04. `spec-critic` should flag this as a missing dependency.
- **SPEC-05 merge** — R12 explicitly states that `merge()` in SPEC-05 must handle free-lists from multiple partitions. SPEC-05 is not listed in `Depends on`.
- **SPEC-19 delta protocol** — R10 states that free-list IDs must stay within worker ID ranges; the delta protocol's stateful worker architecture (SPEC-19 §3.3) means partitions are retained across rounds. There is no cross-reference to SPEC-19 in SPEC-22.

`(source: specs/SPEC-22-arena-management.md, §3.1 R10, R10a, R12, R22; specs/SPEC-04-partition.md; specs/SPEC-05-merge.md; specs/SPEC-19-delta-protocol.md)`

---

### 3. Theory Bridge Audit

SPEC-22 cites three reference identifiers in its frontmatter:

| Cited ID | Resolves in theory-bridge.md? | Notes |
|----------|-------------------------------|-------|
| REF-002 (Lafont 1997) | YES | Listed under "Foundations"; cited by all specs |
| REF-003 (HVM2 — arena management) | YES | Listed under "Implementation / Technique" |
| REF-014 (Kahl — GC impact on parallel reduction) | YES | Listed under "Implementation / Technique" |

**Theory bridge audit result: ALL 3 cited IDs resolve. No broken citations.**

`(source: docs/theory-bridge.md, §References; specs/SPEC-22-arena-management.md, frontmatter)`

**However — missing AC citations:** The spec's §5.2 rationale ("Why SparseNet is Not Used for Reduction") explicitly references "HVM2 (AC-006)" by name and "the Haskell prototype (AC-001)":

> "HVM2 (AC-006) uses a flat array (`node[]`/`vars[]`) specifically for reduction speed. The Haskell prototype (AC-001) uses `Map AgentId Agent` (tree-based, O(log n)) and this is identified as a performance bottleneck."
> `(source: specs/SPEC-22-arena-management.md, §5.2)`

Despite this, neither AC-006 nor AC-001 (nor AC-009, AC-011, AC-015) appear in the spec's `References consumed:` frontmatter field. The theory bridge states that AC-NNN identifiers appearing in spec rationale should be listed in `Code analyses consumed:`. This is a citation hygiene gap — spec-critic should require adding `Code analyses consumed: AC-001, AC-006` (at minimum) to the frontmatter.

The predicted natural AC anchors for SPEC-22 (AC-006 HVM2 Types+Memory, AC-009 HVM4 Term+Heap, AC-011 HVM4 Threading, AC-015 Cross-Cutting Synthesis) are all present in the theory bridge but none are cited. AC-006 is the most relevant: it documents HVM2's `node[]`/`vars[]` flat array arena, RBag (redex bag), GNet (global net), and TMem (thread-local memory) — the direct ancestor of the free-list design choice.

`(source: docs/theory-bridge.md, §Code Analyses; specs/SPEC-22-arena-management.md, §5.1, §5.2)`

---

### 4. Reference Implementations

The theory bridge documents the following code analyses as directly relevant to SPEC-22's design:

**AC-006 — HVM2 Types + Memory**
Path: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\biblioteca\analise-codigo\` (AC-006-HVM2-types-memory.md)
Relevance: HVM2 uses a flat array arena (`node[]`, `vars[]`) with thread-local allocation via `TMem` (thread memory, a bump allocator). GNet (global net) contains a global redex bag (RBag). HVM2's bump allocation is NOT a free-list — it never recycles. This is the design divergence SPEC-22 improves upon for Relativist's workload (commutation-dominated reduction creates and destroys agents, unlike HVM2's functional-programming workload which is expansion-dominated).
Design claim to verify: SPEC-22's free-list is an improvement over HVM2's bump-only allocation for IC reduction with balanced create/destroy cycles. `spec-critic` should verify this claim is accurately characterized in §5.1.

**AC-007 — HVM2 Reduction Engine**
Path: same directory (AC-007-HVM2-reduction.md)
Relevance: Atomic link with ownership. HVM2's reduction involves ownership transfer of port references. SPEC-22 should confirm the free-list does not interfere with ownership semantics if the arena is ever extended toward HVM2-style atomic operations.

**AC-009 — HVM4 Term + Heap**
Path: same directory (AC-009-HVM4-term-heap.md)
Relevance: HVM4 uses a 64-bit term encoding (SUB+TAG+EXT+VAL) with a **unified heap** and bump allocation with cache-line padding. The decision NOT to use bit-packed agents in SPEC-22 (it defers to SPEC-23 for compact representation) is consistent with the AC-009 design but needs explicit acknowledgment.

**AC-011 — HVM4 Threading + Work-Stealing**
Path: same directory (AC-011-HVM4-threading.md)
Relevance: HVM4 uses static heap partitioning (each thread gets a contiguous slice of the unified heap). SPEC-22 R10 mirrors this: free-list recycling is constrained to the worker's assigned ID range `[start, end)`. The connection to AC-011 should be cited.

**AC-015 — Cross-Cutting Synthesis**
Path: same directory (AC-015-cross-cutting-synthesis.md)
Relevance: Comparison across 8 axes for all implementations. The hybrid dense/sparse strategy in SPEC-22 (R22/R23) reflects CC-1 (bit-packed port access) and CC-2 (incremental redex detection) from AC-015. Not cited.

`(source: docs/theory-bridge.md, §Code Analyses AC-006, AC-007, AC-009, AC-011, AC-015)`

---

### 5. Cross-Spec Consistency Hot-Spots

#### 5a. SPEC-02 Net struct — Missing `freeport_redirects` field

**CRITICAL.** SPEC-22 §4.1 defines the `Net` struct as:

```rust
pub struct Net {
    pub agents: Vec<Option<Agent>>,
    pub ports: Vec<PortRef>,
    pub redex_queue: VecDeque<(AgentId, AgentId)>,
    pub next_id: AgentId,
    pub root: Option<PortRef>,
    pub free_list: Vec<AgentId>,   // NEW — SPEC-22
}
```

The live codebase at `relativist-core/src/net/core.rs` (L24-62) has a sixth field:

```rust
    pub freeport_redirects: HashMap<u32, PortRef>,
```

This field is load-bearing: it records FreePort-to-FreePort redirections during local partition reduction and is used by `rebuild_free_port_index` (SPEC-05) to recover border FreePort references. It is skipped during serialization (`#[serde(skip)]`) because it is only relevant during the grid cycle. SPEC-22's struct definition silently omits it. Implementing SPEC-22 §4.1 literally would require removing this field or regressing the delta protocol.

**spec-critic must flag this as a CRITICAL structural inconsistency.** SPEC-22 should either (a) add `freeport_redirects` to its struct definition in §4.1, or (b) explicitly state that §4.1 shows only the fields being added by SPEC-22, not the complete struct.

`(source: relativist-core/src/net/core.rs, L24-62; specs/SPEC-22-arena-management.md, §4.1)`

#### 5b. SPEC-02 R12 — `remove_agent` amendment

SPEC-22 states it amends SPEC-02 R12 in its frontmatter. However, R12 in SPEC-02 currently reads: "The `remove_agent` operation MUST mark the agent's slot as `None`, disconnect all its ports from the port array, and NOT reuse the ID." SPEC-22 R2 and §4.3 add the free-list push. The amendment changes "NOT reuse the ID" to "push the ID onto the free-list for potential reuse." This directly contradicts SPEC-02 R12's current text without providing a structured amendment block (old text / new text). The amendment also conflicts with R10 in SPEC-02 ("IDs are never reused"), which is I3 by another name. SPEC-22 only amends I3 in SPEC-01, but needs to also amend R2 and R10 in SPEC-02 (the implementation-level counterparts).

`(source: specs/SPEC-22-arena-management.md, §3.1 R2, frontmatter; specs/SPEC-02-net-representation.md, R10, R12)`

#### 5c. SPEC-03 Reduction — no mention of free-list

SPEC-22 lists SPEC-03 as a dependency but does not amend any requirement in SPEC-03. The six interaction rules in SPEC-03 call `remove_agent` and `create_agent`. With the free-list, `create_agent` now has branching behavior (pop vs. allocate) and `remove_agent` pushes to the free-list. SPEC-22 §4.7 correctly analyzes the per-rule free-list effect (agent balance table). However, SPEC-03 contains assertions and invariant checks that verify `next_id` monotonicity after each rule. If those assertions compare `next_id` to the last returned `AgentId` (assuming monotonic increase), they will fail when the free-list returns a recycled (lower) ID. SPEC-22 does not address this. The spec-critic should ask: does SPEC-03's debug mode assertion for I3 need updating to use I3' semantics?

`(source: specs/SPEC-22-arena-management.md, §4.7; specs/SPEC-03-reduction.md implied; specs/SPEC-01-invariantes.md, §4.3 assert_next_id_valid)`

#### 5d. SPEC-04 Partition / SPEC-05 Merge — unlisted dependencies with requirements

SPEC-22 R10a ("The `build_subnet()` operation SHOULD populate the free-list of each partition with the `None` slots that fall within that partition's ID range") directly amends `build_subnet()` in `src/partition/helpers.rs` (SPEC-04's primary function). SPEC-22 R12 ("The `merge()` operation MUST handle free-lists from multiple partitions") directly amends `merge()` in `src/merge/engine.rs`. Both SPEC-04 and SPEC-05 are absent from SPEC-22's `Depends on:` frontmatter. This means spec-critic, task-splitter, and test-generator cannot know from the frontmatter alone that SPEC-22 will require changes to those modules.

The current `build_subnet()` implementation (`relativist-core/src/partition/helpers.rs`, L160-204) allocates `vec![None; max_id + 1]` for the agents arena, which produces a sparse allocation proportional to `max_id` rather than live agent count. This is exactly the memory inflation SPEC-22 R22 proposes to fix with the sparse hybrid. However, the SHOULD qualifier on R22/R30 means this is optional — making the improvement opt-in via `PartitionConfig.sparse_build: bool` (R30). spec-critic should assess whether the SHOULD is justified or whether this should be MUST given that R22 identifies a known pathological behavior.

`(source: specs/SPEC-22-arena-management.md, R10a, R12, R22, R30; relativist-core/src/partition/helpers.rs, L160-204)`

#### 5e. SPEC-19 Delta Protocol — free-list and stateful workers

SPEC-22 R10 requires that recycled IDs stay within `[start, end)` worker ranges. Under the delta protocol (SPEC-19 §3.3), workers are **stateful** and retain their partition across BSP rounds. The free-list accumulates across rounds, not just within a single round. This raises a question SPEC-22 does not address: when the coordinator dispatches a `CommutationBatch` (SPEC-19) with `local_wiring` hints to a worker, and the worker creates new agents using free-list slots, are the recycled `AgentId` values stable across rounds? Specifically, if round N recycles ID `47` for a CON agent, and round N+1 the coordinator references a border by the prior occupant's agent port encoding, will the delta resolution be correct?

This is the "slot-id stability" question. SPEC-19's `BorderState` tracks `(border_id, side_a: PortRef, side_b: PortRef)` using `AgentPort(id, port)` encoding. If the same `AgentId` is recycled for a different agent in a different round, and the `BorderGraph` is not updated, the coordinator's view would silently reference the wrong agent. SPEC-22 R7 guarantees that a free-list ID is not referenced by any PortRef in the port array at recycle time (within the partition). But the coordinator's `BorderGraph` is a separate data structure — it is not part of the partition and is not cleaned by the worker's `remove_agent`. This cross-system interaction is a genuine gap.

`(source: specs/SPEC-22-arena-management.md, R7, R10; specs/SPEC-19-delta-protocol.md, §2 BorderGraph definition, §3.3)`

#### 5f. SPEC-23 Compact Memory — forward reference gap

SPEC-23 (`Amends: SPEC-22`) treats SPEC-22 as a precursor and further amends the `Net` struct to use bit-packed `u32` PortRef. SPEC-23's `SparseNet` equivalent would use compact PortRef values as HashMap keys. SPEC-22's `SparseNet` uses `HashMap<(AgentId, PortId), PortRef>` with semantic enum-based PortRef. After SPEC-23 migration, this key would change to `(u32, u8)` or similar. SPEC-22 does not acknowledge this forward compatibility concern. The `SparseNet::ports` field design may be obsolete the moment SPEC-23 ships.

`(source: specs/SPEC-23-compact-memory.md, frontmatter "Amends: SPEC-22"; specs/SPEC-22-arena-management.md, §4.4 SparseNet struct definition)`

---

### 6. Open Questions (spec-provided + pesquisador-added)

**From §8 of SPEC-22:**

- **Q1.** Should `SparseNet` support `freeport_redirects`? If `SparseNet` is used for partition construction, it may need the FreePort redirection mechanism. Currently deferred. Implication: without `freeport_redirects`, `SparseNet`-built partitions cannot participate in grid cycles that use the delta protocol's border management.

- **Q2.** Should `SparseNet` implement a trait shared with `Net`? Deferred to spec review. This is load-bearing for the hybrid usage of R22/R30: without a shared trait, call sites must explicitly convert, and polymorphic code cannot exist.

- **Q3.** Free-list memory overhead cap: 40 MB for 10M annihilated agents. Tentatively no cap. Deferred.

- **Q4.** Should the free-list be sorted for determinism? No. The spec correctly argues that ID allocation order does not affect normal form topology (I3' uniqueness vs. I3 monotonicity). However, this means regression tests using specific `AgentId` values in assertions will break if recycling is enabled, since IDs will no longer monotonically increase. The test suite (1181 tests) likely contains such assertions. SPEC-22's R9 notes backward compatibility for deserialized nets but doesn't address test brittleness.

**Pesquisador-added OQs:**

- **OQ-A. `SparseNet::to_dense()` populates the free-list with ALL `None` slots.** SPEC-22 §4.6 (`to_dense()` implementation) pushes every `None` slot in the resulting dense arena onto the free-list. This includes "structural `None` gaps" created by the contiguous-ID strategy (e.g., a partition for worker 1 with agents 100-200 will have a dense arena of size 201 with slots 0-99 all `None` and all on the free-list). Worker 0 then has 100 "free" slots it must never use (they're outside its ID range). R10 requires filtering by ID range, but the `to_dense()` function does not apply this filter — it blindly populates all `None` slots. This may be intentional (the consumer is expected to filter), but R10a says `build_subnet()` SHOULD populate filtered free-lists, creating an inconsistency between the two entry points.

- **OQ-B. Thread safety claim.** SPEC-22 never uses the word "thread-safe" or "Send/Sync". The current `Net` is not `Send` + `Sync` (it contains `HashMap` and `VecDeque`, which are not `Sync`). Under BSP, each worker holds its own partition exclusively during a round — single-threaded access by design. However, SPEC-22 R22/R30 proposes using `SparseNet` during `build_subnet()`, which runs in the coordinator's thread before dispatch. If `SparseNet` construction is meant to run in parallel for multiple partitions, thread safety would be relevant. The spec should either confirm single-threaded invariant or explicitly annotate thread safety.

- **OQ-C. `unsafe` boundary.** The CLAUDE.md coding standard prohibits `unsafe` without `// SAFETY:` comment. SPEC-22 introduces no `unsafe` code in its design sketches. However, SPEC-23 (the forward reference) introduces bit-packed access patterns that may require unsafe transmutes. SPEC-22 should confirm that the free-list and SparseNet designs are implementable entirely without `unsafe`, since SPEC-23 will later introduce the first potential `unsafe` boundary in `net/types.rs`. This forward planning prevents accidentally using safe-API patterns in SPEC-22 that would conflict with SPEC-23's bit-packed accessor migration.

- **OQ-D. Backward compatibility of free-list serialization.** R9 requires the free-list to be included in serde serialization. This changes the binary format of `Net`. Any persisted `.bin` files from v1 will lack the `free_list` field. SPEC-02 R24-R26 define the serialization format. SPEC-22 does not define a migration path for deserializing v1-era binary files or bumping the format version. SPEC-18 (Wire Format v2) manages the protocol version; what manages the `Net` struct version?

`(source: specs/SPEC-22-arena-management.md, §8; relativist-core/src/net/core.rs; specs/SPEC-18-wire-format-v2.md; CLAUDE.md coding standards)`

---

### 7. Round 1 Attack Surface for spec-critic

The following are the highest-value adversarial angles, framed as questions Round 1 must answer:

**[CRITICAL-1] Does the `Net` struct definition in §4.1 silently delete `freeport_redirects`?**
The struct in §4.1 shows 6 fields. The live codebase has 7 (includes `freeport_redirects: HashMap<u32, PortRef>`). If a developer implements §4.1 as written, they must decide what happens to this field. SPEC-22 never mentions it. This is either a spec error (the struct should show 7 fields) or an uncommunicated decision (§4.1 is illustrative not definitive). Either interpretation is a spec deficiency. Impact: any implementation following §4.1 literally will either break SPEC-05 merge semantics or silently reintroduce the field with no spec guidance.

**[CRITICAL-2] Are SPEC-04 and SPEC-05 listed as dependencies?**
R10a amends `build_subnet()` (SPEC-04). R12 amends `merge()` (SPEC-05). Neither is in `Depends on:`. The task-splitter cannot generate tasks for the SPEC-04/SPEC-05 amendment phases without this information. Round 1 should flag this as a blocking dependency graph error.

**[HIGH-1] Does I3 → I3' relaxation need to amend SPEC-02 R2 ("AgentId type MUST be u32, monotonically increasing, never reused")?**
SPEC-01 I3 is amended by SPEC-22 R24. But SPEC-02 R2 contains its own "never reused" statement that is not listed in SPEC-22's frontmatter amendments. Amending I3 without amending its sibling R2 creates a contradiction between SPEC-01 and SPEC-02 that downstream specs will need to resolve.

**[HIGH-2] Does the delta protocol's `BorderGraph` maintain correctness when worker agents are recycled across rounds?**
SPEC-22 R7 guarantees free-list IDs are not referenced in the local port array at recycle time. But the coordinator's `BorderGraph` (SPEC-19 §2) stores `AgentPort(id, port)` references that were valid at the time of the last border delta. If a worker recycles ID `X` (previously a CON-agent) and the `BorderGraph` still has a `BorderState` entry for `AgentPort(X, 0)`, the coordinator's border detection logic will silently read the wrong agent type. This is a G1 threat under the delta protocol. The spec-critic must ask: does SPEC-22 R10 (ID range constraint) alone prevent this, or does the free-list require an explicit "notify coordinator of recycled IDs" protocol extension?

**[HIGH-3] Does the free-list preserve G1 (Fundamental Property) under the v1 lenient BSP protocol (non-delta)?**
Under lenient BSP, the coordinator merges all partitions and calls `reduce_all()` on the merged net. The merged net's free-list (R12) must be valid. If a partition's free-list contains IDs that are occupied in another partition (due to contiguous-ID strategy producing gaps), the merged free-list could contain occupied slots. R12 says "IDs that were in a partition's free-list but whose slots are now occupied in the merged net MUST NOT appear in the merged free-list." How is this efficiently checked? The merge function in `relativist-core/src/merge/core.rs` does not currently have any free-list handling. This is an O(N) post-merge scan not discussed in SPEC-22.

**[MEDIUM-1] Should R22 be MUST instead of SHOULD, given that the current `build_subnet()` has a known pathological memory allocation?**
The SHOULD for sparse construction (R22/R30) means the memory inflation bug in `build_subnet()` is not fixed by this spec in a binding way. For the TCC benchmark scenario (`ep_con 100M` on 2GB coordinator, per next-steps.md Milestone M5), this is not an optional optimization. spec-critic should challenge whether SHOULD is the right normative level.

**[MEDIUM-2] Is R23 ("SparseNet MUST NOT be used in the reduction hot path") testable as a spec requirement?**
R23 is a performance constraint, not a behavioral constraint. It cannot be tested with a property test or unit test — it can only be enforced by code review or lint. SPEC-22 §7 does not include a test for R23. The spec-critic should flag that R23 is a design rule, not a verifiable requirement, and may belong in the Design section (§4) rather than the Requirements section (§3).

**[LOW-1] Is the invariant I3' (SPEC-22 R24) formally compatible with SPEC-13 System Architecture's ID allocation assumptions?**
SPEC-13 defines the coordinator FSM and module dependency graph. The `compute_id_ranges()` function assumes monotonically increasing IDs for range allocation. Under I3', `next_id` remains monotonic (it is only incremented, never decremented). The ID ranges are still disjoint. This is likely safe, but SPEC-22 does not explicitly confirm SPEC-13 compatibility. `spec-critic` should request a one-sentence clarification in SPEC-22 §5.3.

---

## Primary Sources

| # | File | Relevance |
|---|------|-----------|
| 1 | `specs/SPEC-22-arena-management.md` | The spec under review — primary source |
| 2 | `relativist-core/src/net/core.rs` | Live `Net` struct — reveals missing `freeport_redirects` field |
| 3 | `specs/SPEC-01-invariantes.md` | I3, D4, T1, G1 — invariants being amended or at risk |
| 4 | `specs/SPEC-02-net-representation.md` | R2, R6, R12 — requirements being amended (partially unlisted) |
| 5 | `specs/SPEC-19-delta-protocol.md` | BorderGraph — slot-id stability interaction with free-list |
| 6 | `specs/SPEC-23-compact-memory.md` | Forward reference: amends SPEC-22; SparseNet PortRef key conflict |
| 7 | `docs/theory-bridge.md` | Citation audit — AC-006/AC-001 referenced in body but not frontmatter |
| 8 | `relativist-core/src/partition/helpers.rs` | `build_subnet()` L160-204 — confirms dense allocation pathology |
| 9 | `docs/next-steps.md` | M5 milestone target: `ep_con 100M` on 2GB — frames urgency of SHOULD→MUST question |
| 10 | `specs/SPEC-04-partition.md` | Unlisted dependency for R10a and R22 |
| 11 | `specs/SPEC-05-merge.md` | Unlisted dependency for R12 |

---

## Non-Obvious Connections

**Connection 1 — `freeport_redirects` + `SparseNet` Q1 are the same gap.** SPEC-22 §8 Q1 asks "Should `SparseNet` support `freeport_redirects`?" This question arises precisely because SPEC-22 §4.1 does not include `freeport_redirects` in the `Net` struct diagram. If the spec-author had included it, Q1 would naturally answer itself (both `Net` and `SparseNet` need the same mechanism to participate in grid cycles). The omission of `freeport_redirects` in §4.1 and the open Q1 in §8 are symptom and cause, not two independent issues.

**Connection 2 — `to_dense()` free-list population + R10 ID range constraint.** SPEC-22 §4.6 `to_dense()` pushes ALL `None` slots onto the free-list. SPEC-22 R10 says workers must only use free-list IDs in their assigned range. But `to_dense()` is the function that *creates* the free-list for a just-converted sparse net. If `to_dense()` is called on a partition (not a global net), the resulting free-list will contain IDs from the entire arena including gap slots outside the worker's ID range. These invalid IDs will be present until the worker's first `create_agent` call, which must then filter them. R10 implies filtering but `to_dense()` does not implement it. The filter belongs either in `to_dense()` (as a parameter: `id_range: Option<Range<AgentId>>`) or in the `create_agent` implementation (which must know its worker's range). Currently neither is specified.

**Connection 3 — R9 serialization compatibility + SPEC-18 wire format version.** SPEC-22 R9 adds `free_list` to serde serialization. SPEC-18 manages the protocol-level wire format version (and already bumped `PROTOCOL_VERSION` 2→3 for SPEC-19 §3.4). Serialized `Net` objects travel over the wire as `AssignPartition` / `PartitionResult` payloads in v1, and as `InitialPartition` / `FinalStateResult` in the delta protocol. If SPEC-22 adds a new field to `Net` serialization without a corresponding format-version bump, deserialization of v2 nets by v1 workers will fail silently with serde's default field handling (unknown fields are ignored with `#[serde(deny_unknown_fields)]` or silently dropped without it). The spec does not clarify which behavior applies. This is a versioning gap between SPEC-22 and SPEC-18.

---

## Identified Gaps

1. **No `§3.8 Amendments` block.** Pattern established by SPEC-19/SPEC-20. The amendments to SPEC-01 I3 and SPEC-02 R12 (listed in frontmatter) are buried in §3.3 as requirements R24/R27 rather than structured as `Old text / New text / Rationale` blocks. Downstream agents (task-splitter, SPEC-02 maintainer) need the structured format.

2. **`freeport_redirects` absent from §4.1 struct definition.** Verified against live codebase. Not a spec design decision — likely an oversight.

3. **SPEC-02 R2 and R10 amendments not listed.** These contain the same "monotonically increasing, never reused" language as SPEC-01 I3. Amending I3 without amending its SPEC-02 counterparts creates a cross-spec contradiction.

4. **SPEC-04 and SPEC-05 missing from `Depends on:`.** R10a and R12 directly require changes to `build_subnet()` and `merge()`.

5. **AC citations absent from frontmatter.** AC-006 is mentioned by name in §5.2 body text but not in `Code analyses consumed:`.

6. **SPEC-19 cross-reference for slot-id stability absent.** The delta protocol's `BorderGraph` stores `AgentPort(id, port)` references. Free-list recycling of those `id` values in a subsequent round is a potential correctness hole not addressed in either spec.

7. **`to_dense()` does not respect worker ID range filter.** Gap between R10 (runtime constraint) and §4.6 (implementation that violates the constraint at construction time).

8. **Format version bump not addressed.** Adding `free_list` to `Net` serde changes the serialized format. SPEC-18's versioning mechanism should be invoked.

9. **The theory bridge has no DISC cross-reference to SPEC-22.** DISC-012 v2 (Job Submission, Problem Encoding, Result Decoding) is listed in the theory bridge as informing "SPEC-22 (Job submission)" — but SPEC-22 is about arena management, not job submission. This appears to be a theory-bridge labeling error from an earlier naming draft where SPEC-22 may have been the job submission spec. SPEC-22 itself does not cite DISC-012 and does not appear to be about job submission. This is NOT a SPEC-22 gap — it is a theory-bridge metadata error for the pesquisador to flag for TCC-root correction.

---
