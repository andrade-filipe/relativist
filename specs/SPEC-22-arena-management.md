# SPEC-22: Arena Management and Memory Efficiency

**Status:** Reviewed v2.1 — R10b/R10c trigger broadened per SPEC-21 §3.8 A6 (`delta_mode` → `(delta_mode || streaming_active)`)
**Depends on:** SPEC-02 (Net Representation), SPEC-01 (Invariants), SPEC-03 (Reduction Engine), SPEC-04 (Partitioning — amended via R10a/R22), SPEC-05 (Merge — amended via R12), SPEC-18 (Wire Format v2 — PROTOCOL_VERSION bump per R9a), SPEC-19 (Delta Protocol — BorderGraph slot-id stability per R10b)
**Amends:** SPEC-01 I3 (Monotonicity of AgentIds — relaxed to I3' Uniqueness for free-list variant), SPEC-02 R2 (relaxes "never reused" to "uniqueness via free-list"), SPEC-02 R10 (relaxes "incremented by k" to "incremented by the count of non-recycle creations"), SPEC-02 R11 (clarifies "next available ID" to include free-list pop), SPEC-02 R12 (extended for free-list push), SPEC-03 §4.3 debug-assertion language (reformulated as I3'-compatible), SPEC-04 §4.5 build_subnet (free-list population per partition range), SPEC-05 §4.2 merge (free-list reconciliation), SPEC-18 PROTOCOL_VERSION (bump for `Net.free_list` serde layout change), SPEC-19 §3.2 BorderGraph contract (recycling forbidden in delta mode OR border-clean precondition); SPEC-21 §3.8 A6 (R10b/R10c trigger condition broadened from `delta_mode == true` to `(delta_mode || streaming_active) && id ∈ border_referenced_set`; SPEC-21 R37b)
**ROADMAP items:** 2.33 (Arena Recycling / GC During Reduction — free-list variant only), 2.32 (Sparse Net Representation)
**References consumed:** REF-002 (Lafont 1997), REF-003 (HVM2 — arena management), REF-014 (Kahl — GC impact on parallel reduction)
**Code analyses consumed:** AC-001 (Haskell IC.Core — `Map AgentId Agent` baseline), AC-006 (HVM2 Types + Memory — flat arena rationale), AC-009 (HVM4 Term + Heap — bump allocation), AC-011 (HVM4 Threading + Work-Stealing — static heap partitioning ↔ free-list per ID range), AC-015 (Cross-Cutting Synthesis — CC-4 ID space)
**Arguments consumed:** ARG-002 C1-C3 (border bijection — informs §3.8 SPEC-04/SPEC-05 amendments), ARG-005 INV-REC (delta recoverability — informs SC-005 BorderGraph constraint)

---

## 1. Purpose

This spec defines two independent mechanisms for improving memory efficiency in Relativist's net representation:

1. **Arena Recycling via Free-List (ROADMAP 2.33, free-list variant):** A free-list of recycled `AgentId` slots that allows `create_agent` to reuse slots vacated by consumed agents, preventing the agent arena from growing indefinitely during long reductions. This is the free-list variant only — no renumbering, no compaction, no remapping.

2. **Sparse Net Representation (ROADMAP 2.32):** An alternative net representation using `HashMap<AgentId, Agent>` and `HashMap<(AgentId, PortId), PortRef>` instead of the dense `Vec<Option<Agent>>` and flat `Vec<PortRef>`. This eliminates wasted memory for consumed agent slots and is useful for construction and partitioning phases where proportionality to live agents matters.

These two features are **independent**. Arena recycling operates on the existing `Vec`-based `Net`. The sparse net is a completely new representation type. Both MAY be implemented separately and neither depends on the other.

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary) and SPEC-02 (Net Representation) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Free-List** | A stack (LIFO) of `AgentId` values corresponding to agent slots that have been freed by `remove_agent`. The free-list enables slot reuse without renumbering. Implemented as `Vec<AgentId>` within the `Net` struct. |
| **Recycled Slot** | An agent slot whose `AgentId` was previously assigned to a live agent, which was subsequently consumed by a reduction rule and added to the free-list. A recycled slot's `agents[id]` entry is `None` and its `AgentId` is present in the free-list. |
| **Tombstone** | An agent slot in the `Vec<Option<Agent>>` arena where `agents[id] == None` and the slot was previously occupied. In the absence of a free-list, tombstones persist for the lifetime of the net. With a free-list, tombstones are reclaimed when `create_agent` pops their ID from the free-list. |
| **Tombstone Ratio** | The fraction of tombstone slots over total arena length: `(arena_len - live_count) / arena_len`. A high tombstone ratio (>0.5) indicates significant memory waste. |
| **Arena Recycling** | The process of reusing tombstone slots via the free-list, as opposed to always allocating new IDs via `next_id` increment. Recycling prevents arena growth when agent creation and destruction are balanced (e.g., commutation rules that destroy 2 agents and create 4). |
| **SparseNet** | An alternative net representation where agents and ports are stored in `HashMap` collections. Memory is proportional to live agents only. No tombstone slots, no padding, no wasted port array entries. |
| **Dense-to-Sparse Conversion** | The operation `Net::to_sparse() -> SparseNet` that converts a dense `Vec`-based net to a sparse `HashMap`-based net, skipping `None` slots. Complexity: O(live_agents). |
| **Sparse-to-Dense Conversion** | The operation `SparseNet::to_dense() -> Net` that converts a sparse net back to a dense `Vec`-based net. The resulting dense net's arena has no tombstones. Complexity: O(live_agents). |
| **Hybrid Approach** | The strategy of using `SparseNet` for construction and partitioning (where memory proportionality matters), then converting to dense `Net` before the reduction loop (where O(1) indexed access matters). |

---

## 3. Requirements

### 3.1 Arena Recycling / Free-List (ROADMAP 2.33)

**R1.** The `Net` struct MUST contain a free-list field: `free_list: Vec<AgentId>`. The free-list stores the `AgentId` values of slots that have been freed by `remove_agent` and are available for reuse by `create_agent`. **(MUST)**

**R2.** When `remove_agent(id)` is called, after marking `agents[id] = None` and disconnecting all ports (as per SPEC-02 R12), the operation MUST push `id` onto the free-list. **(MUST)**

**R3.** When `create_agent(symbol)` is called, it MUST first check the free-list:
- If the free-list is non-empty, pop an `AgentId` from the free-list, reuse that slot in the arena, and return the recycled ID. The `next_id` field MUST NOT be incremented.
- If the free-list is empty, allocate a new slot by using `next_id` and incrementing it (existing behavior per SPEC-02 R11).
**(MUST)**

**R4.** When reusing a recycled slot, `create_agent` MUST:
- (a) Set `agents[id] = Some(Agent { symbol, id })` where `id` is the popped free-list value.
- (b) Ensure the port array slots for the recycled ID (`id * 3 + 0`, `id * 3 + 1`, `id * 3 + 2`) are initialized to `DISCONNECTED`.
- (c) NOT expand the arena or port array (the slot already exists within bounds).
**(MUST)**

**R5.** The free-list MUST use LIFO (stack) ordering. Popping from the end of a `Vec<AgentId>` is O(1). The LIFO policy maximizes temporal locality: recently freed slots are reused first, which tends to keep hot cache lines active. **(MUST)**

**R6.** The free-list MUST NOT contain duplicate `AgentId` values. Each freed ID appears in the free-list at most once. The `remove_agent` operation MUST NOT push an ID that is already in the free-list. In debug mode, this invariant MUST be verified by an explicit assertion in `remove_agent` immediately before the push: `debug_assert!(!self.free_list.contains(&id))`. Implementations MAY maintain an O(1) `HashSet<AgentId>` shadow of the free-list under `#[cfg(debug_assertions)]` to avoid the O(n) cost of `Vec::contains` in debug builds; the shadow is OPTIONAL and is not part of the release-build state. This closes SC-018. **(MUST)**

**R7.** An `AgentId` in the free-list MUST NOT be referenced by any `PortRef::AgentPort(id, _)` in the port array, the redex queue, or the `root` field. The slots for a free-list ID in the port array MUST contain `DISCONNECTED`. This is guaranteed by `remove_agent` disconnecting all ports before pushing to the free-list. **(MUST)**

**R8.** The `Net::new()` and `Net::with_capacity()` constructors MUST initialize the free-list as empty (`Vec::new()`). **(MUST)**

**R9.** The free-list MUST be included in serde serialization/deserialization of `Net` (SPEC-02 R24-R26). A deserialized net MUST have a valid free-list: every ID in the free-list MUST correspond to a `None` slot in the arena, and every `None` slot in the arena SHOULD be in the free-list (slots that were `None` before free-list introduction MAY not be in the free-list for backward compatibility). **(MUST)**

**R9a.** The introduction of `free_list` in the `Net` serialized layout MUST be coordinated with SPEC-18 (`Wire Format v2`). SPEC-18 currently sets `PROTOCOL_VERSION = 2` (verified against `specs/SPEC-18-wire-format-v2.md` line 536). The arrival of SPEC-22 in the wire-relevant `Net` payload MUST bump `PROTOCOL_VERSION` from `2` to `3`. v2 (`PROTOCOL_VERSION = 2`) deserializers MUST reject v3 (`PROTOCOL_VERSION = 3`) serialized nets with the existing version-mismatch path (SPEC-18 R31's `UnsupportedVersion` reject pattern, mirrored from SPEC-20 R37 v3-vs-v4 rejection clause). v3 deserializers MAY ALSO reject v2 nets, OR MAY tolerate them as nets with an empty `free_list` (deserializer-defined; document the chosen path in §6 Migration). Persisted v1/v2 `.bin` files (e.g., `results/locked/v1_local_baseline/`) become unreadable by v3 binaries; this is acceptable because v1 baseline binaries are frozen and not consumed by v2/v3 code paths. **(MUST)**

This closes SC-007. The amendment is recorded in §3.8 A8.

**R10.** The free-list MUST be compatible with ID space partitioning (SPEC-04 R16-R19, SPEC-01 D4). When a worker creates agents during reduction, it MUST draw recycled IDs from the free-list only if those IDs fall within the worker's assigned ID range `[start, end)`. IDs in the free-list that fall outside the worker's range MUST NOT be used by that worker. **(MUST)**

**R10a.** The `build_subnet()` operation (SPEC-04) MUST populate the free-list of each partition with the `None` slots that fall within that partition's ID range `[id_range.start, id_range.end)`. This ensures partitions start with a pre-populated free-list for immediate recycling during local reduction. The MUST upgrade (was SHOULD in v1 of this spec) closes SC-009: under `ContiguousIdStrategy` the dense allocation of `vec![None; max_id + 1]` is pathological at M5 scale (100M agents). The free-list MUST contain only IDs strictly within the partition's range; IDs outside `[id_range.start, id_range.end)` are forbidden in this partition's free-list (closes SC-006). **(MUST)**

**R10b.** Free-list recycling MUST preserve `BorderGraph` slot-id stability (SPEC-19 §3.2 R8-R12). An `AgentId` that is referenced by any `BorderState.side_a` or `BorderState.side_b` in the coordinator's `BorderGraph` (i.e., the ID is a border-endpoint live during cross-partition delta exchange) MUST NOT be recycled by the worker that owns it. Two compliant implementation strategies are normative:

- **Strategy A (delta-mode disable):** When `GridConfig.delta_mode == true` (SPEC-19), the worker MUST NOT pop from the free-list during a round. `create_agent` falls back to `next_id` allocation. The free-list still accumulates pushes from `remove_agent` and is drained at the next clean partition boundary (after `reconstruct` per SPEC-19 R38).
- **Strategy B (border-clean precondition):** The worker MAY pop from the free-list only if the popped ID is NOT present in the partition's `border_entries[i]` set (SPEC-04 R20-R22 makes this set partition-local and locally inspectable in O(1) via a `HashSet<AgentId>` shadow). If the popped ID is border-referenced, it MUST be re-pushed to the free-list (or stored in a side-list for reuse after the next reconstruct) and a fresh `next_id` MUST be allocated instead.

The choice between Strategy A and Strategy B is a `GridConfig.recycle_under_delta: RecyclePolicy` field (default: `RecyclePolicy::DisableUnderDelta` = Strategy A, the conservative choice). Strategy B is opt-in for benchmarks where measurement of the recycle benefit under delta mode is desired. **(MUST)**

This closes SC-005. The threat model R10b prevents: round N produces border `B = (border_id, AgentPort(47, 0), AgentPort(123, 0))`; in round N+1 worker recycles ID 47 to a different `Symbol`; coordinator dispatches a `CommutationBatch` indexing `AgentPort(47, 0)`; the worker's local `agents[47]` now resolves to a different rule than the BorderGraph computed → G1 violation. R10b prevents the recycle in step 2.

> **Amendment A6 (SPEC-21 §3.8 A6 / R37b):** The R10b trigger condition is broadened from `delta_mode == true` to `(delta_mode || streaming_active) && id ∈ border_referenced_set`. The same recycle-vs-border-identity hazard arises whenever a coordinator-side border-tracking surface exists; streaming pipelines (SPEC-21 §3.3 R17 `generate_and_partition_chunked`) maintain a coordinator-side `border_map: HashMap<u32, (PortRef, PortRef)>` and a pending-connection store that both reference live `AgentId`s, exposing the same threat as delta mode. Under streaming alone (no delta), if a worker's free-list pops a slot ID that is referenced by an active border in the coordinator's `border_map`, and the slot is reassigned to a new agent before chunk N+1's `AssignPartition`, the border-target identity becomes ambiguous and `merge` (SPEC-05) can wire two distinct logical agents together — the same G1 violation pattern as the delta-mode threat, just under a different active-tracking surface.
>
> Strategy A and Strategy B both extend identically to streaming-active context: under Strategy A, workers MUST NOT pop from the free-list while `streaming_active`; under Strategy B, the `border_referenced_set` SHALL be sourced from the streaming-pipeline's `border_map` (not just the delta-protocol BorderGraph) when `streaming_active && !delta_mode`. The two normative strategies (`RecyclePolicy::DisableUnderDelta` / `RecyclePolicy::BorderClean`) are renamed conceptually to `DisableUnderBorderTracking` / `BorderClean` in implementation, **but the wire-level enum name `RecyclePolicy::DisableUnderDelta` is preserved for backward compatibility** (the field name is misleading post-SPEC-21 but stable; same precedent as SC-013 / DISC-012 stale-tag handling).
>
> **Alternative one-liner closure (per SPEC-21 R37b).** An implementation MAY use the cargo feature gate `streaming-no-recycle` that disables the worker free-list outright during streaming. This satisfies R37b trivially without requiring any further amendments. SPEC-21 §3.8 A6 documents this as the "valid one-liner closure" path.
>
> Closes SC-007.

**R10c.** When `remove_agent(id)` is called on a worker whose `id` IS border-referenced in the coordinator's `BorderGraph` **OR (per Amendment A6) in the coordinator's streaming `border_map`** (i.e., `(delta_mode || streaming_active) && id ∈ border_referenced_set`), the agent's port slots MUST still be set to `DISCONNECTED` (per SPEC-02 R12 and SPEC-22 R7), the slot MUST be set to `agents[id] = None`, but the ID MUST NOT be pushed to the free-list (under either Strategy A or Strategy B). The slot becomes a *protected tombstone* that persists until the next `reconstruct`/clean-boundary moment. The protected-tombstone set MAY be tracked via `Net::protected_tombstones: HashSet<AgentId>` (debug builds only) so a debug assertion can verify R10b at the moment of `create_agent`. **(MUST)**

**R11.** The `count_live_agents()` operation (SPEC-02 R16a) MUST NOT count free-list entries as live agents. The free-list does not change the semantics of `count_live_agents`; it only affects which `None` slots are available for reuse. **(MUST)**

**R12.** The `merge()` operation (SPEC-05) MUST handle free-lists from multiple partitions. After merging, the resulting net's free-list MUST contain only IDs that correspond to `None` slots in the merged arena. IDs that were in a partition's free-list but whose slots are now occupied in the merged net MUST NOT appear in the merged free-list. **(MUST)**

### 3.2 Sparse Net Representation (ROADMAP 2.32)

**R13.** A `SparseNet` type MUST be defined (in `src/net/sparse.rs`) with the following fields:
- `agents: HashMap<AgentId, Agent>` — maps each live agent's ID to the agent.
- `ports: HashMap<(AgentId, PortId), PortRef>` — maps each port to its connected target.
- `redex_queue: VecDeque<(AgentId, AgentId)>` — queue of active pairs (same semantics as `Net`).
- `next_id: AgentId` — next available ID (same semantics as `Net`).
- `root: Option<PortRef>` — root observation port (same semantics as `Net`).
- `freeport_redirects: HashMap<u32, PortRef>` — same semantics as `Net.freeport_redirects` (SPEC-02 / live `net/core.rs:L49-L61`). Closes Q1 / SC-011: `SparseNet` MUST carry this field so that conversions are lossless and `SparseNet`-based `build_subnet` (R22) is consumable by `merge` (SPEC-05) without losing border-redirect state.
**(MUST)**

**R14.** `SparseNet` MUST implement the same logical operations as `Net` (SPEC-02 R11-R16b):
- `create_agent(symbol: Symbol) -> AgentId` — insert into the HashMap, increment `next_id`. **(MUST)**
- `remove_agent(id: AgentId)` — remove from the HashMap, remove port entries. **(MUST)**
- `connect(a: PortRef, b: PortRef)` — insert bidirectional entries in the port HashMap; enqueue redex if both are principal ports. **(MUST)**
- `disconnect(port: PortRef)` — remove bidirectional entries from the port HashMap. **(MUST)**
- `get_target(port: PortRef) -> PortRef` — lookup in the port HashMap. **(MUST)**
- `get_agent(id: AgentId) -> Option<&Agent>` — lookup in the agent HashMap. **(MUST)**
- `get_agent_mut(id: AgentId) -> Option<&mut Agent>` — mutable lookup. **(MUST)**
- `is_reduced(&self) -> bool` — redex queue empty check. **(MUST)**
- `count_live_agents(&self) -> usize` — `self.agents.len()`. **(MUST)**
- `live_agents(&self) -> impl Iterator<Item = &Agent>` — iterate over HashMap values. **(MUST)**
**(MUST)**

**R15.** `SparseNet` operations MUST have the following complexity:
- `create_agent`: O(1) amortized (HashMap insert).
- `remove_agent`: O(1) amortized (HashMap remove + up to 3 port removals).
- `connect`: O(1) amortized (2 HashMap inserts + conditional redex enqueue).
- `disconnect`: O(1) amortized (2 HashMap removals).
- `get_target`: O(1) amortized (HashMap lookup).
- `get_agent` / `get_agent_mut`: O(1) amortized (HashMap lookup).
- `count_live_agents`: O(1) (`HashMap::len()`).
**(MUST)**

**R16.** `SparseNet` MUST NOT store entries for consumed agents. When `remove_agent(id)` is called, the agent is removed from `self.agents` and all associated port entries `(id, 0)`, `(id, 1)`, `(id, 2)` are removed from `self.ports`. There are no tombstone slots. Memory is strictly proportional to live agents. **(MUST)**

**R17.** `SparseNet` MUST NOT store port entries for ERA auxiliary ports. Since ERA agents have arity 0, entries `(era_id, 1)` and `(era_id, 2)` MUST NOT exist in the port HashMap. This is the sparse equivalent of SPEC-01 I6 (ERA Auxiliary Slot Cleanliness). **(MUST)**

**R18.** `SparseNet` MUST derive or implement `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize`. **(MUST)**

**R19.** A conversion function `Net::to_sparse(&self) -> SparseNet` MUST be provided. It MUST:
- Iterate over the dense arena, inserting only `Some(agent)` entries into the sparse agents HashMap.
- For each live agent, copy port entries from the flat port array into the sparse ports HashMap, skipping ERA auxiliary slots and `DISCONNECTED` entries.
- Copy the redex queue, `next_id`, and `root` directly.
- Complexity: O(A) where A is the dense arena length.
**(MUST)**

**R20.** A conversion function `SparseNet::to_dense(&self) -> Net` MUST be provided. It MUST:
- Determine the maximum `AgentId` in the sparse agents HashMap.
- Allocate a `Vec<Option<Agent>>` of size `max_id + 1`, initialized to `None`.
- Allocate a `Vec<PortRef>` of size `(max_id + 1) * 3`, initialized to `DISCONNECTED`.
- Insert all agents and port entries from the sparse representation into the dense vectors.
- Copy the redex queue, `next_id`, and `root` directly.
- Complexity: O(max_id) for allocation + O(live_agents) for insertion.
**(MUST)**

**R21.** Round-trip conversions MUST preserve **behavioural equality**, defined as follows (closes SC-008's ambiguity flag and SC-014):

> Two `Net` values `n1` and `n2` are *behaviourally equal* iff for every sequence of public-API operations `[create_agent, remove_agent, connect, disconnect, get_target, get_agent, count_live_agents, is_reduced, reduce_step, reduce_all]`, the observable post-state of `n1` after the sequence and the observable post-state of `n2` after the same sequence agree on every public field projection (live-agent set, port-target relation, redex queue contents up to ordering, root, `next_id`, `freeport_redirects`). Specifically: `n1.agents.iter().filter(|x| x.is_some()).collect::<Vec<_>>() == n2.agents.iter().filter(|x| x.is_some()).collect::<Vec<_>>()`, and analogous projections for `ports` (only `AgentPort` entries, ignoring trailing `DISCONNECTED`) and `freeport_redirects` (full equality). Byte-equality of the underlying `Vec`s is NOT required: `agents.len()` and `ports.len()` MAY differ between `n1` and `n2` because trailing `None`/`DISCONNECTED` slots are trimmed by `to_sparse().to_dense()`.

The two round-trips MUST satisfy:
- `Net::to_sparse().to_dense(None)` MUST produce a net **behaviourally equal** to the original.
- `SparseNet::to_dense(None).to_sparse()` MUST produce a sparse net **structurally equal** (full `==`) to the original (sparse `HashMap` representations are inherently free of trailing-slot ambiguity).

The `Net::is_behaviorally_equal(&self, other: &Net) -> bool` helper MUST be provided in `src/net/core.rs` and MUST be used by tests T14 / T8 instead of `==`. The serde-bincode round-trip test (T8) MUST use byte-equality after explicit normalization (see §7 T8 update). **(MUST)**

**R22.** `SparseNet` MUST be used as the representation during subnet construction in `build_subnet()` (SPEC-04, `src/partition/helpers.rs`) when the dense arena would inflate beyond a threshold. Specifically:

- If `partition.id_range.end - partition.id_range.start > 4 * partition.live_agent_count`, `build_subnet` MUST use `SparseNet` and then call `to_dense(Some(partition.id_range.clone()))` before the reduction loop. This is the M5-target case (SC-009).
- Otherwise, `build_subnet` MAY use the existing dense path or the sparse-then-dense path; the choice is a `PartitionConfig.sparse_build: bool` flag with default `true` (R30).

Under the current `ContiguousIdStrategy`, the last worker's dense subnet allocates `vec![None; max_id + 1]` even when it owns a fraction of the agents. The MUST upgrade above (was SHOULD in v1) closes SC-009: at M5 scale (100M agents on 2 GB coordinator), a worker holding 10M live agents in a partition with `id_range = 0..100M` would allocate 800 MB of `vec![None; 100M]` under the dense path — pathological. The 4× threshold provides a clean safety margin. **(MUST)**

**R23.** `SparseNet` is a design-time choice intended for construction and partitioning, NOT for the reduction hot path. The reduction engine (SPEC-03) relies on O(1) guaranteed indexed access to `agents[id]` and `ports[id * 3 + port]`; HashMap lookup has O(1) amortized complexity but with a 5-10x worse constant factor due to hashing and cache misses (per AC-006 HVM2 flat-array rationale; AC-001 Haskell `Map AgentId Agent` baseline; SC-008 demoted from MUST NOT for testability). This is enforced by a CI lint at the import-graph level rather than by a runtime requirement: `src/reduction/**/*.rs` MUST NOT contain `use crate::net::sparse::SparseNet;` (or any other path resolving to `SparseNet`). The lint MUST be added to the existing `cargo clippy -- -D warnings` gate via a custom rule or an equivalent grep-based pre-commit check. The hybrid approach (R22: sparse for construction, dense for reduction) is the design intent. **(DESIGN CONSTRAINT — enforced by CI lint, not runtime invariant)**

This closes SC-008. R23 is no longer a "MUST NOT" runtime requirement; it is a CI-enforceable structural rule, captured in `.github/workflows/`-level tooling (SPEC-15 / cicd agent owns the actual lint authoring; SPEC-22 owns the rule statement).

### 3.3 Invariant Amendments

**R24.** Invariant I3 (Monotonicity of AgentIds, SPEC-01) MUST be relaxed for nets with an active free-list. The amended invariant is:

> **I3' (Uniqueness of AgentIds):** Each `AgentId` in use belongs to exactly one live agent in the arena. `AgentId` values in the free-list belong to no agent (`agents[id] == None`). No two live agents share the same `AgentId`. The `next_id` field MUST be strictly greater than any `AgentId` ever assigned (whether currently live, in the free-list, or previously freed and re-assigned).

The change: monotonicity (IDs always increase) is relaxed to uniqueness (IDs are never shared). The `next_id` field remains monotonic and serves as an upper bound on the ID space, but `create_agent` MAY return IDs less than `next_id` when recycling from the free-list. **(MUST)**

**R25.** The relaxation of I3 to I3' MUST NOT violate D4 (ID Uniqueness After Distributed Reduction, SPEC-01). In distributed execution:
- Free-list IDs are bound to the worker's assigned ID range `[start, end)` (R10).
- A recycled ID is reused only within the same partition where it was freed.
- The free-list is partition-local; no cross-partition ID reuse occurs.
- Therefore, D4 (disjointness of ID sets across partitions) is preserved.
**(MUST)**

**R26.** Invariants T1 (Port Linearity), I1 (Bidirectional Consistency), and I2 (Reference Validity) MUST hold for `SparseNet` with adapted verification:
- T1/I1: For every port entry `(a_id, p) -> q` in the sparse ports HashMap, there MUST exist a reverse entry `q -> AgentPort(a_id, p)` (unless `q` is a `FreePort`). The root port exception (SPEC-01 T1) applies unchanged.
- I2: For every `AgentPort(id, p)` value in the sparse ports HashMap, `self.agents.contains_key(&id)` MUST be `true` and `p <= arity(agents[id].symbol)`.
**(MUST)**

**R27.** Debug assertions (SPEC-01, Section 4.3; SPEC-02 R20) MUST be extended to verify free-list consistency:
- After `remove_agent`: if the ID was recycled (R10b not triggered), the freed ID is in the free-list, `agents[id] == None`, and port slots are `DISCONNECTED`. If the ID is a protected tombstone (R10c), `agents[id] == None`, port slots are `DISCONNECTED`, the ID is NOT in the free-list, and the protected-tombstone shadow set (when maintained in debug builds) contains the ID.
- After `create_agent` with recycled ID: the ID is no longer in the free-list, `agents[id] == Some(_)`, and the free-list contains no duplicates.
- After `create_agent` (any path): the returned ID is NOT in the protected-tombstone shadow set. This guards against accidental recycling under R10b.
- Periodic (or per-step in debug mode): no free-list ID is referenced by any `PortRef` in the port array.
**(MUST)**

**R27a.** SPEC-03 (Reduction Engine) debug assertions on `next_id` and on `create_agent` return values MUST be reformulated as I3'-compatible (uniqueness, not monotonicity of returned IDs). Specifically (closes SC-010):

- Any existing `assert!(new_id > old_max_id)` or `debug_assert!(new_id > self.next_id - 1)` patterns inside SPEC-03's 6 rule implementations MUST be removed or replaced with `debug_assert!(self.agents[new_id as usize].is_some())` (uniqueness check).
- The CON-DUP commutation rule (which creates 4 agents per fire) MUST NOT assume the 4 returned IDs form a contiguous monotonic block. Two of the 4 IDs MAY come from the free-list and be smaller than `next_id` at the start of the rule; the other 2 MAY be allocated fresh and be larger.
- `assert_next_id_valid` (SPEC-02 §4.5 / SPEC-03 audit per the live `relativist-core/src/net/core.rs`) is preserved: its check `(i as u32) < self.next_id` for `slot.is_some()` is consistent with I3' (free-list IDs are in `None` slots, so they don't trip the assertion). No edit needed there; only the in-rule assertions need the I3' audit.

The amendment is recorded in §3.8 A6. The audit is a Stage 3 DEVELOPER task: scan `src/reduction/` for assertion sites and reformulate per the bullet list above. Test T7 (Section 7) validates the post-audit behaviour; an additional test SHOULD be added (T7a) that reduces a CON-DUP redex with a non-empty free-list and asserts I3' on the resulting 4 agents (uniqueness, not monotonicity).

### 3.4 Configuration

**R28.** Arena recycling MUST be always-on by default (no feature gate). The free-list adds negligible overhead (one `Vec<AgentId>` field, one `pop`/`push` per create/remove) and provides significant memory savings for workloads with high agent turnover. **(MUST)**

**R29.** The `SparseNet` type MUST be always available (no feature gate). It is a data structure, not an optional behavior. Whether a particular code path uses `SparseNet` or `Net` is a design choice at each call site, not a runtime configuration. **(MUST)**

**R30.** The hybrid approach (R22: `SparseNet` for `build_subnet`, `Net` for reduction) MUST be configurable via a `sparse_build: bool` flag in `PartitionConfig` or equivalent. Default: `true` (use sparse construction). Setting `false` reverts to the current dense construction for backward compatibility or when the overhead of conversion is undesirable. Note that under R22's threshold rule (`id_range > 4 × live_agent_count`), `sparse_build = false` MUST be rejected at the top of `build_subnet` with a clear `PartitionError::DenseAllocationExceedsThreshold` because the dense path would inflate beyond M5 budget. The flag's `false` setting is therefore only honored when the threshold is NOT exceeded. **(MUST)**

### 3.5 Cross-cutting MUSTs

**R31.** SPEC-22 implementations MUST be expressible in safe Rust (no `unsafe` blocks). Any future migration of `Net.ports` or `SparseNet.ports` to a bit-packed representation requires `unsafe transmute`-style accessors and is the responsibility of SPEC-23 (`Compact Memory Representation`). SPEC-22 explicitly takes no `unsafe` boundary. This closes SC-017. **(MUST)**

**R32.** Free-list memory budget at M5 scale (100M agents): worst-case `4 × 100M = 400 MB` per partition is acceptable IFF the partition is the sole tenant of a process; in multi-partition single-process configurations, the implementation MUST switch the free-list representation from `Vec<AgentId>` to a compact bitmap (`bitvec::BitVec` or equivalent, 1 bit per slot in `[0, max_id)` = 12.5 MB for max_id = 100M) when `free_list.len() * 4 > MAX_FREELIST_BYTES` (default `MAX_FREELIST_BYTES = 64 MB`). The bitmap representation MUST preserve the LIFO contract via a high-water-mark cursor (the bitmap is a set; the cursor is the "most recently set" index, scanning downward on `pop`). This closes SC-015 against the M5 milestone (the 10M scenario in original Q3 was insufficient). The bitmap representation is OPTIONAL for v1 implementations targeting <10M; mandatory for v2 / M5 implementations. **(MUST at M5; MAY at <10M)**

### 3.8 Amendments to Predecessor Specs (closes SC-002, SC-003, SC-004)

SPEC-22 amends the following requirements of predecessor specs. The amendments are formal and MUST be cross-referenced in those specs' next revision; ESPECIALISTA EM SPECS owns the cross-reference patches. The structure of this block follows the canonical SPEC-20 §3.8 / SPEC-19 §3.8 pattern: each entry gives the target spec, the target requirement number, the old text (verbatim), the new text, and a rationale.

**A1. SPEC-01 I3 amendment (Monotonicity → Uniqueness).**
- *Old text (SPEC-01 I3, lines 289-296):* "Each `AgentId` value MUST be assigned to at most one agent during the lifetime of a `Net`. The `next_id` field MUST be strictly greater than any `AgentId` currently in use. Once an agent is removed, its `AgentId` MUST NOT be reassigned to a new agent. This guarantees IDs are stable references and uniquely identify agents."
- *New text (SPEC-22 R24 / I3'):* "**I3' (Uniqueness of AgentIds):** Each `AgentId` in use belongs to exactly one live agent in the arena. `AgentId` values in the free-list belong to no agent (`agents[id] == None`). No two live agents share the same `AgentId`. The `next_id` field MUST be strictly greater than any `AgentId` ever assigned (whether currently live, in the free-list, or previously freed and re-assigned). Stability of IDs is preserved during a `BorderGraph`-active round (R10b/R10c) by means of protected tombstones."
- *Rationale:* Monotonicity is sufficient but not necessary for uniqueness; the free-list provides uniqueness without monotonicity, recovering O(slot)-bounded memory under high-turnover workloads.

**A2. SPEC-02 R2 amendment.**
- *Old text (SPEC-02 R2, line 37):* "The `AgentId` type MUST be `u32`, monotonically increasing, never reused within an execution (cf. SPEC-01, I3). **(MUST)**"
- *New text:* "The `AgentId` type MUST be `u32`. IDs MUST be unique among live agents (cf. SPEC-01, I3'). IDs MAY be reused via the free-list mechanism (SPEC-22 R1-R10c), under the constraints that (a) the recycled slot is fully cleared (port slots `DISCONNECTED`, `agents[id] == None`, `freeport_redirects` entry purged) before reuse, and (b) the recycled ID is NOT a protected tombstone (R10c) at the moment of reuse. **(MUST)**"
- *Rationale:* "Never reused" was a sufficiency condition for I3; under I3' it is replaced by a uniqueness condition with explicit clearing protocol. Closes SC-002 part 1.

**A3. SPEC-02 R10 amendment.**
- *Old text (SPEC-02 R10, line 58):* "The field `next_id` MUST be strictly greater than any `AgentId` in use in the net (cf. SPEC-01, I3). After creating `k` agents, `next_id` MUST be incremented by `k`."
- *New text:* "The field `next_id` MUST be strictly greater than any `AgentId` ever assigned in the net (live, in the free-list, or previously freed and re-assigned; cf. SPEC-01, I3'). After creating `k` agents in a single batch where `r` of those `k` come from the free-list and `f = k - r` are fresh allocations, `next_id` MUST be incremented by exactly `f` (the count of fresh, non-recycle allocations). When the free-list is empty, `r = 0` and the increment equals the original `k`."
- *Rationale:* Closes SC-002 part 2. The fresh-vs-recycle decomposition is the natural accounting under I3'.

**A4. SPEC-02 R11 amendment (clarification, not relaxation).**
- *Old text (SPEC-02 R11):* "MUST create a new agent with the next available ID, insert it into the agent arena (expanding if necessary), and return the assigned `AgentId`. Expected complexity: O(1) amortized."
- *New text:* "MUST create a new agent with the next available ID — defined as `free_list.pop()` if the free-list is non-empty AND the popped ID is not a protected tombstone (R10b/R10c), otherwise `next_id` (with `next_id` incremented). MUST insert the agent into the arena (expanding if necessary, only on the fresh-allocation path) and return the assigned `AgentId`. Expected complexity: O(1) amortized."
- *Rationale:* Closes SC-009 / SPEC-02 R11 partial-contradiction flag. The free-list path and the `next_id` path are now both subsumed under "next available ID". Closes the R3-vs-R11 ambiguity flagged in the cross-spec audit.

**A5. SPEC-02 R12 amendment.**
- *Old text (SPEC-02 R12):* "MUST mark the agent's slot as `None`, disconnect all its ports from the port array, and NOT reuse the ID."
- *New text:* "MUST mark the agent's slot as `None`, disconnect all its ports from the port array, purge any `freeport_redirects` entry keyed by the agent's ID, and (if the ID is NOT a protected tombstone per R10c) push the ID onto `free_list` (SPEC-22 R2/R6). The ID MAY be reused on a subsequent `create_agent` call per A2/A4."
- *Rationale:* Lifts the no-reuse clause and threads the `freeport_redirects` cleanup. Originally declared in this spec's frontmatter; now formalized with target-spec text.

**A6. SPEC-03 §4.3 amendment (debug assertion language).**
- *Old text (SPEC-03 §4.3 — generic prose, no specific R-number):* Rule implementations MAY contain `debug_assert!(new_id > old_max_id)` or equivalent monotonicity assertions on `create_agent` return values.
- *New text (SPEC-22 R27a):* Rule implementations MUST NOT assume monotonicity of returned IDs across multiple `create_agent` calls in a single rule fire. Allowed assertion patterns: `debug_assert!(self.agents[new_id as usize].is_some())` (uniqueness check post-create), `debug_assert!(self.next_id > new_id)` (next_id upper-bound check). Forbidden assertion patterns: `assert!(new_id > old_max_id)`, `assert!(new_id == self.next_id - 1)`, or any other monotonicity claim.
- *Rationale:* Closes SC-010. The CON-DUP commutation is the load-bearing case (creates 4 agents; recycling makes the 4 IDs non-monotonic).

**A7. SPEC-04 §4.5 build_subnet amendment.**
- *Old text (SPEC-04 §4.5):* `build_subnet(net, worker_agents[i], sigma, border_entries[i])` produces a subnet for partition `i` using the dense `Net` representation; no free-list mention.
- *New text:* `build_subnet` MUST populate the partition subnet's `free_list` with all `None` slots in `[partition.id_range.start, partition.id_range.end)` after the live agents are placed. When the dense-arena threshold check fires (`id_range > 4 × live_agent_count`), `build_subnet` MUST use `SparseNet` internally and call `to_dense(Some(partition.id_range.clone()))` before returning (R10a, R22). The exposed signature MAY remain `Net` to preserve API stability; the sparse path is an implementation detail.
- *Rationale:* Closes SC-003 / SC-006 / SC-009. Makes free-list partition-correct from the first `create_agent` call and avoids the M5 dense-allocation pathology.

**A8. SPEC-05 §4.2 merge amendment.**
- *Old text (SPEC-05 §4.2 line 322):* `fn merge(plan: PartitionPlan) -> (Net, u32)` — current merge does not touch free-lists.
- *New text:* `merge` MUST construct the merged net's `free_list` as follows: walk every input partition's `free_list`; for each ID, check whether the ID is occupied in the merged arena; if `None` in the merged arena, push to merged free-list; if `Some` in the merged arena (because the slot was filled by a different partition's `Some` agent), discard the entry. Complexity: O(sum of |partition.free_list|). The post-merge free-list MUST satisfy the SPEC-22 R6 no-duplicates invariant; this is automatic given that pre-merge each partition's free-list is duplicate-free and partitions own disjoint ID ranges (D4).
- *Rationale:* Closes SC-003. Without this amendment, R12 ("merge MUST handle free-lists") is unimplementable.

**A9. SPEC-18 PROTOCOL_VERSION amendment.**
- *Old text (SPEC-18 R28, line 163; live constant `PROTOCOL_VERSION = 2`):* "The `PROTOCOL_VERSION` constant MUST be bumped from `1` to `2`. This is a one-time, intentional wire compatibility break."
- *New text (SPEC-22 R9a):* "The `PROTOCOL_VERSION` constant MUST be bumped from `2` to `3` upon SPEC-22 landing in the wire-relevant `Net` payload. v2 deserializers MUST reject v3 nets with `UnsupportedVersion`. The wire break is justified by the `free_list` field addition; v1/v2 binaries cannot deserialize v3 nets without producing length-mismatch errors. Migration path documented in §6 of this spec."
- *Rationale:* Closes SC-007. Without an explicit version bump, mixed-version deployments would silently corrupt deserialization.

**A10. SPEC-19 §3.2 BorderGraph contract amendment.**
- *Old text (SPEC-19 R8-R12):* The `BorderGraph` stores `BorderState` entries indexed by `border_id`, each carrying `side_a: PortRef`, `side_b: PortRef`, `worker_a: WorkerId`, `worker_b: WorkerId`, and a derived `is_redex: bool`. No interaction with worker-side ID recycling is specified.
- *New text (SPEC-22 R10b/R10c):* When `GridConfig.delta_mode == true`, worker-side free-list recycling is constrained as follows: under `RecyclePolicy::DisableUnderDelta` (default), workers MUST NOT pop from the free-list; under `RecyclePolicy::BorderClean`, workers MAY pop from the free-list only for IDs not present in `border_entries`. Border-referenced IDs become protected tombstones (R10c) and are NOT recycled until the next `reconstruct` clean boundary.
- *Rationale:* Closes SC-005. Without this amendment, free-list recycling × `BorderGraph` slot-id stability is a G1 violation under delta mode (the coordinator's `BorderState.side_a = AgentPort(47, 0)` may be invalidated by intra-round recycling).

---

## 4. Design

### 4.1 Free-List Data Structure

The free-list is a `Vec<AgentId>` used as a stack (LIFO). It is a NEW field of the `Net` struct, ADDED to the existing SPEC-02 / live-code definition. The struct definition below is the COMPLETE post-SPEC-22 layout, including the pre-existing `freeport_redirects` field (defined in SPEC-02 / live `relativist-core/src/net/core.rs:L24-L62`). The `freeport_redirects` field is unchanged by SPEC-22 — it is reproduced here only because §4.1 must show the canonical full struct so that §4.6 conversion code (`to_dense`/`to_sparse`) can reference every field without ambiguity. This closes SC-001.

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct Net {
    /// Agent arena. Pre-existing per SPEC-02; unchanged.
    pub agents: Vec<Option<Agent>>,

    /// Flat port array. Pre-existing per SPEC-02; unchanged.
    pub ports: Vec<PortRef>,

    /// Queue of active pairs. Pre-existing per SPEC-02; unchanged.
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Monotonic upper bound on assigned AgentIds. Pre-existing per
    /// SPEC-02 R10; semantics now per I3' (R24): MUST be strictly greater
    /// than any AgentId ever assigned (live, in free-list, or previously
    /// freed and re-assigned).
    pub next_id: AgentId,

    /// Root observation port. Pre-existing per SPEC-02; unchanged.
    pub root: Option<PortRef>,

    /// FreePort-to-FreePort redirections used during merge.
    /// Pre-existing per SPEC-02 / live code (`net/core.rs:L49-L61`); UNCHANGED
    /// by SPEC-22. Conversion functions §4.6 MUST preserve this field
    /// (closes SC-001 second surface). Not serialized: only relevant
    /// during the grid cycle.
    #[serde(skip)]
    #[cfg_attr(feature = "zero-copy", rkyv(with = rkyv::with::Skip))]
    pub freeport_redirects: HashMap<u32, PortRef>,

    /// NEW (SPEC-22 R1): free-list of recycled AgentId slots available
    /// for reuse. LIFO ordering: `create_agent` pops from the end,
    /// `remove_agent` pushes to the end. Invariant: every ID in this list
    /// has `agents[id] == None` and port slots == DISCONNECTED. Under
    /// distributed execution (R10), every ID in this list MUST fall within
    /// the partition's owning ID range; under delta mode (R10b),
    /// border-referenced IDs are protected tombstones (R10c), NOT free-list
    /// members.
    pub free_list: Vec<AgentId>,
}
```

**Note on `freeport_redirects` × free-list interaction.** When `remove_agent(id)` is called and `id` is recycled to the free-list, the worker MUST also remove any entry `(id, _)` from `freeport_redirects` (because the entry's `u32` key is the AgentId of a port endpoint that no longer exists). Failure to do so would leave a stale redirect that, after the slot is recycled, references a *different* agent than the redirect was authored against. This closes the `freeport_redirects` × recycle interaction implied by SC-001. The implementation MUST add this to the `remove_agent` body (§4.3 below).

### 4.2 Modified `create_agent`

```rust
impl Net {
    /// Creates a new agent with the given symbol.
    /// Returns the assigned AgentId.
    ///
    /// If the free-list is non-empty, reuses a recycled slot (O(1)).
    /// Otherwise, allocates a new slot at next_id (O(1) amortized).
    ///
    /// Postcondition: agents[id] == Some(Agent { symbol, id }).
    /// Invariant I3' (uniqueness): the returned ID is not held by any other live agent.
    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        if let Some(id) = self.free_list.pop() {
            // Reuse a recycled slot
            debug_assert!(
                self.agents[id as usize].is_none(),
                "Free-list invariant violated: slot {} is not None",
                id
            );

            let agent = Agent { symbol, id };
            self.agents[id as usize] = Some(agent);

            // Port slots already exist and should be DISCONNECTED.
            // Re-initialize them defensively.
            let base = (id as usize) * PORTS_PER_SLOT;
            for offset in 0..PORTS_PER_SLOT {
                debug_assert!(
                    self.ports[base + offset] == DISCONNECTED,
                    "Free-list invariant violated: port slot {} is not DISCONNECTED",
                    base + offset
                );
                self.ports[base + offset] = DISCONNECTED;
            }

            id
        } else {
            // No recycled slots available; allocate a new one
            let id = self.next_id;
            self.next_id += 1;

            let agent = Agent { symbol, id };

            if self.agents.len() <= id as usize {
                self.agents.resize((id as usize) + 1, None);
            }
            self.agents[id as usize] = Some(agent);

            let required_len = (id as usize + 1) * PORTS_PER_SLOT;
            if self.ports.len() < required_len {
                self.ports.resize(required_len, DISCONNECTED);
            }

            id
        }
    }
}
```

### 4.3 Modified `remove_agent`

```rust
impl Net {
    /// Removes an agent from the net and adds the slot to the free-list.
    ///
    /// Postcondition: agents[id] == None, port slots == DISCONNECTED.
    /// If `id` is NOT border-referenced (R10b/R10c), id is in free_list (recycled).
    /// If `id` IS border-referenced under delta mode, id becomes a protected
    /// tombstone and is NOT pushed to the free-list.
    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents[id as usize] {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let port = PortRef::AgentPort(id, p);
                self.disconnect(port);
            }
            self.agents[id as usize] = None;

            // Remove any stale freeport_redirects entry keyed by this id
            // (closes SC-001 freeport_redirects × recycle interaction).
            self.freeport_redirects.remove(&(id as u32));

            // Push the freed slot onto the free-list for recycling, UNLESS
            // the id is border-referenced under delta mode (R10b/R10c).
            // The border-reference check is the responsibility of the caller's
            // GridConfig.recycle_under_delta policy:
            //   - DisableUnderDelta: skip the push entirely while delta is active.
            //   - BorderClean: consult `border_entries` HashSet.
            // In a non-distributed (single-net) context, this guard is a no-op
            // and the push always happens.
            if !self.is_border_protected(id) {
                self.free_list.push(id);
            }
            // else: protected tombstone (R10c). Slot stays None; ports stay
            // DISCONNECTED; ID is NOT in free_list. Reclamation happens at
            // the next reconstruct/clean-boundary moment.
        }
    }

    /// Returns true if `id` is referenced by any active BorderState.side_a
    /// or side_b in the coordinator's BorderGraph and therefore must NOT
    /// be recycled (SPEC-22 R10b/R10c).
    ///
    /// In single-net (non-distributed) and v1 (non-delta) contexts, this
    /// MUST return `false`. Under delta mode, the implementation consults
    /// the worker's locally-cached border-id set (SPEC-04 R20-R22 makes
    /// `border_entries` partition-local).
    ///
    /// The exact wiring of border state to the Net is out of scope for
    /// SPEC-22; SPEC-19 owns that contract. SPEC-22 only requires that
    /// the predicate is honored at the recycle decision point.
    fn is_border_protected(&self, _id: AgentId) -> bool {
        // Default behavior in pure-net contexts: never protected.
        // Distributed call sites override this via injected border state
        // (the exact mechanism is implementation-defined; an enum field
        // `recycle_policy: RecyclePolicy` on Net is one valid wiring).
        false
    }
}
```

### 4.4 SparseNet Type

```rust
use std::collections::{HashMap, VecDeque};

/// Sparse interaction net representation.
///
/// Uses HashMaps instead of Vecs for agents and ports.
/// Memory is proportional to live agents only (no tombstone slots).
/// Intended for construction and partitioning phases (R22, R23).
///
/// NOT intended for the reduction hot path — use Net::to_dense()
/// before reduce_all/reduce_step.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SparseNet {
    /// Live agents, keyed by AgentId.
    pub agents: HashMap<AgentId, Agent>,

    /// Port connections, keyed by (AgentId, PortId).
    /// Only stores entries for live ports (no ERA auxiliary slots,
    /// no DISCONNECTED entries).
    pub ports: HashMap<(AgentId, PortId), PortRef>,

    /// Queue of active pairs (same semantics as Net.redex_queue).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next available AgentId (monotonically increasing).
    pub next_id: AgentId,

    /// Root observation port (same semantics as Net.root).
    pub root: Option<PortRef>,

    /// FreePort redirect map (same semantics as Net.freeport_redirects).
    /// Required for lossless conversion and for SparseNet-based build_subnet
    /// (R22) to feed merge (SPEC-05) without losing border state. Closes
    /// SC-011 (was Q1 in earlier draft).
    #[serde(skip)]
    pub freeport_redirects: HashMap<u32, PortRef>,
}
```

**Send + Sync.** `SparseNet` MUST be `Send + Sync` (closes SC-016). All field types (`HashMap<K,V>` with `K, V: Send + Sync`, `VecDeque<T>` with `T: Send + Sync`, `AgentId`, `Option<PortRef>`) are `Send + Sync`. The implementation MUST add a compile-time assertion equivalent to `static_assertions::assert_impl_all!(SparseNet: Send, Sync)`. Same applies to the post-SPEC-22 `Net` (the new `free_list: Vec<AgentId>` is `Send + Sync` since `AgentId: Send + Sync`).

### 4.5 SparseNet Operations

```rust
impl SparseNet {
    pub fn new() -> Self {
        SparseNet {
            agents: HashMap::new(),
            ports: HashMap::new(),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
            freeport_redirects: HashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        SparseNet {
            agents: HashMap::with_capacity(capacity),
            ports: HashMap::with_capacity(capacity * PORTS_PER_SLOT),
            redex_queue: VecDeque::new(),
            next_id: 0,
            root: None,
            freeport_redirects: HashMap::new(),
        }
    }

    pub fn create_agent(&mut self, symbol: Symbol) -> AgentId {
        let id = self.next_id;
        self.next_id += 1;

        let agent = Agent { symbol, id };
        self.agents.insert(id, agent);
        // No port entries created yet; they are created by connect().

        id
    }

    pub fn remove_agent(&mut self, id: AgentId) {
        if let Some(agent) = self.agents.remove(&id) {
            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                self.disconnect(PortRef::AgentPort(id, p));
            }
        }
    }

    pub fn connect(&mut self, a: PortRef, b: PortRef) {
        // Set bidirectional entries (only for AgentPort endpoints)
        if let PortRef::AgentPort(a_id, a_port) = a {
            self.ports.insert((a_id, a_port), b);
        }
        if let PortRef::AgentPort(b_id, b_port) = b {
            self.ports.insert((b_id, b_port), a);
        }

        // Detect new redex
        if let (PortRef::AgentPort(a_id, 0), PortRef::AgentPort(b_id, 0)) = (a, b) {
            self.redex_queue.push_back((a_id, b_id));
        }
    }

    pub fn disconnect(&mut self, port: PortRef) {
        if let PortRef::AgentPort(id, p) = port {
            if let Some(target) = self.ports.remove(&(id, p)) {
                if let PortRef::AgentPort(t_id, t_port) = target {
                    self.ports.remove(&(t_id, t_port));
                }
            }
        }
    }

    pub fn get_target(&self, port: PortRef) -> PortRef {
        match port {
            PortRef::AgentPort(id, p) => {
                self.ports.get(&(id, p)).copied().unwrap_or(DISCONNECTED)
            }
            PortRef::FreePort(_) => DISCONNECTED,
        }
    }

    pub fn get_agent(&self, id: AgentId) -> Option<&Agent> {
        self.agents.get(&id)
    }

    pub fn get_agent_mut(&mut self, id: AgentId) -> Option<&mut Agent> {
        self.agents.get_mut(&id)
    }

    pub fn is_reduced(&self) -> bool {
        self.redex_queue.is_empty()
    }

    pub fn count_live_agents(&self) -> usize {
        self.agents.len()
    }

    pub fn live_agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.values()
    }
}
```

### 4.6 Conversion Functions

The conversion functions preserve every field of the source representation, including `freeport_redirects` (closes SC-001 second surface). The `to_dense` signature accepts an OPTIONAL `id_range` parameter that scopes free-list population (closes SC-006).

```rust
impl Net {
    /// Converts this dense Net to a SparseNet.
    /// Skips None slots in the arena and DISCONNECTED entries in the port array.
    /// Preserves `freeport_redirects` (per SC-001).
    /// Complexity: O(arena_len).
    pub fn to_sparse(&self) -> SparseNet {
        let live_count = self.count_live_agents();
        let mut sparse = SparseNet::with_capacity(live_count);

        for agent in self.agents.iter().flatten() {
            sparse.agents.insert(agent.id, *agent);

            let num_ports = total_ports(agent.symbol);
            for p in 0..num_ports {
                let idx = port_index(agent.id, p);
                let target = self.ports[idx];
                if target != DISCONNECTED {
                    sparse.ports.insert((agent.id, p), target);
                }
            }
        }

        sparse.redex_queue = self.redex_queue.clone();
        sparse.next_id = self.next_id;
        sparse.root = self.root;
        sparse.freeport_redirects = self.freeport_redirects.clone();

        sparse
    }
}

impl SparseNet {
    /// Converts this SparseNet to a dense Net.
    ///
    /// `id_range`:
    ///   - `Some(range)`: the resulting Net's free-list is populated only with
    ///     `None` indices that fall within `[range.start, range.end)`. This is
    ///     the partition-context call site (R10/R10a). Indices outside the
    ///     range are NOT added to the free-list (they belong to other
    ///     partitions; using them would violate D4).
    ///   - `None`: whole-net case. Every `None` index in the arena is added
    ///     to the free-list. Use only in non-partitioned contexts.
    ///
    /// Allocates arena and port array sized to max_id + 1.
    /// Preserves `freeport_redirects` (per SC-001).
    /// Complexity: O(max_id) for allocation + O(live_agents) for insertion +
    /// O(min(arena_len, |id_range|)) for free-list population.
    pub fn to_dense(&self, id_range: Option<core::ops::Range<AgentId>>) -> Net {
        let max_id = self.agents.keys().max().copied().unwrap_or(0);
        let arena_len = (max_id as usize) + 1;

        let mut net = Net {
            agents: vec![None; arena_len],
            ports: vec![DISCONNECTED; arena_len * PORTS_PER_SLOT],
            redex_queue: self.redex_queue.clone(),
            next_id: self.next_id,
            root: self.root,
            freeport_redirects: self.freeport_redirects.clone(),
            free_list: Vec::new(),
        };

        for (&id, &agent) in &self.agents {
            net.agents[id as usize] = Some(agent);
        }

        for (&(id, port), &target) in &self.ports {
            let idx = port_index(id, port);
            net.ports[idx] = target;
        }

        // Populate free-list. SC-006 fix: scope to id_range when provided.
        let (lo, hi) = match id_range {
            Some(r) => (r.start as usize, (r.end as usize).min(arena_len)),
            None => (0, arena_len),
        };
        for i in lo..hi {
            if net.agents[i].is_none() {
                net.free_list.push(i as AgentId);
            }
        }

        net
    }
}
```

**Backward compatibility note.** Call sites that previously invoked the no-arg `to_dense()` from earlier drafts of this spec MUST migrate to `to_dense(None)`. There is no implicit default; the explicit `None` documents the whole-net-vs-partition-scope decision at every call site.

### 4.7 Free-List Behavior During Reduction

During reduction, the 6 interaction rules call `remove_agent` (which pushes to the free-list) and `create_agent` (which pops from the free-list). The agent balance per rule (SPEC-01 T5):

| Rule | Agents Removed | Agents Created | Free-List Effect |
|------|---------------|----------------|-----------------|
| CON-CON Annihilation | 2 | 0 | +2 to free-list (net grows by 0) |
| DUP-DUP Annihilation | 2 | 0 | +2 to free-list (net grows by 0) |
| ERA-ERA Void | 2 | 0 | +2 to free-list (net grows by 0) |
| CON-DUP Commutation | 2 | 4 | -2 from free-list, then +2 new IDs if free-list was depleted |
| CON-ERA Erasure | 2 | 2 | 0 net change (2 freed, 2 recycled) |
| DUP-ERA Erasure | 2 | 2 | 0 net change (2 freed, 2 recycled) |

For annihilation-dominated workloads (e.g., `ep_annihilation`), the free-list accumulates slots that are never reused — the arena doesn't shrink but also doesn't grow. For commutation-dominated workloads (e.g., `con_dup_expansion`), the free-list provides immediate recycling: the 2 destroyed agents' slots are reused for 2 of the 4 new agents, reducing arena growth by 50%.

**Intra-rule order non-determinism (closes SC-019).** The order of `remove_agent` and `create_agent` calls *within* a single rule fire is implementation-defined. Two compliant implementations of CON-DUP may interleave (`remove A; create C1`; `remove B; create C2`; `create C3`; `create C4`) versus serial (`remove A; remove B; create C1; create C2; create C3; create C4`); the steady-state agent count is identical, but the recycled-ID assignment may differ (e.g., C1 reuses `id_a` in interleaved, C1 reuses `id_b` in serial). SPEC-22 only guarantees the steady-state counts in the table above and the I3' uniqueness invariant. Tests MUST NOT assert specific recycled-ID assignments within a single rule; they MAY assert (a) the count of fresh allocations vs recycle pops, (b) the post-rule live-agent count, (c) the post-rule `next_id` value (which depends only on the count of fresh allocations, not on the order). Where a test names specific IDs (e.g., T2's "IDs 2, 3, 1"), the test MUST be a pure `create_agent`/`remove_agent` driver test outside the reduction engine, where the call order IS specified by the test itself.

---

## 5. Rationale

### 5.1 Why Free-List, Not Full Compaction

ROADMAP 2.33 describes two variants: (a) full compaction with ID renumbering, and (b) free-list recycling without renumbering. This spec implements only the free-list variant because:

1. **No renumbering complexity.** Full compaction requires building a remapping table `old_id -> new_id` and updating every `PortRef::AgentPort(old_id, port)` in the port array, redex queue, `freeport_redirects`, and `root`. This is O(live_agents) and invalidates any external references to old IDs (e.g., the border map during distributed execution). The free-list avoids all of this.

2. **No external reference invalidation.** Free-list recycling is purely local: the ID value stays the same, only the agent occupying that slot changes. No external data structure is invalidated.

3. **Minimal code change.** ~150 LoC: one new field on `Net`, modified `create_agent`, modified `remove_agent`, updated constructors, updated serialization, debug assertions. Full compaction would be ~700 LoC with significantly more surface area for bugs.

4. **Sufficient for the TCC scope.** The free-list prevents unbounded arena growth for all workloads where agent creation and destruction are balanced. For workloads where agents are only created (pure expansion with no annihilation), the arena must grow regardless — neither free-list nor compaction helps.

### 5.2 Why SparseNet is Not Used for Reduction

The reduction hot path (`reduce_step` in SPEC-03) accesses `net.agents[id]` and `net.ports[id * 3 + port]` with O(1) guaranteed indexed access. HashMap provides O(1) amortized access, but:

- **Constant factor.** HashMap lookup involves computing a hash, probing the bucket array, and following a pointer. For `u32` keys, this is ~5-10x slower than a direct array index.
- **Cache misses.** HashMap entries are heap-allocated and scattered in memory. Dense array access benefits from spatial locality and hardware prefetching.
- **Benchmarks.** HVM2 (AC-006) uses a flat array (`node[]`/`vars[]`) specifically for reduction speed. The Haskell prototype (AC-001) uses `Map AgentId Agent` (tree-based, O(log n)) and this is identified as a performance bottleneck.

The hybrid approach (sparse for construction/partitioning, dense for reduction) provides the best of both worlds: memory proportionality during construction and raw speed during reduction.

### 5.3 I3 Relaxation Is Safe

The original I3 (monotonicity) was designed to prevent ID collisions. The free-list preserves the essential property (uniqueness) while relaxing the non-essential property (monotonicity). Specifically:

- **Uniqueness is preserved.** A free-list ID is only reused after the previous occupant has been fully removed (all ports disconnected, agent slot set to `None`). No two live agents ever share an ID.
- **D4 compatibility.** Free-list IDs are constrained to the worker's ID range (R10), so cross-partition collisions cannot occur.
- **`next_id` remains monotonic.** The `next_id` field is only incremented, never decremented. It serves as the upper bound of the ID space for a given net. This is important for `build_subnet` and ID range allocation.

---

## 6. Migration Path

### 6.1 Arena Recycling (Free-List)

1. **Add `free_list: Vec<AgentId>` to `Net` struct** in `src/net/core.rs`. Update `Net::new()`, `Net::with_capacity()`, and `#[derive(Serialize, Deserialize)]`.
2. **Modify `remove_agent`** to push the freed ID onto the free-list after disconnecting ports.
3. **Modify `create_agent`** to pop from the free-list before falling back to `next_id` increment.
4. **Update `build_subnet`** in `src/partition/helpers.rs` to populate partitions' free-lists with `None` slots within each partition's ID range.
5. **Update `merge`** in `src/merge/engine.rs` to construct a valid free-list for the merged net.
6. **Add debug assertions** for free-list invariants (R6, R7, R27).
7. **Update existing tests.** All 690 v1 tests MUST pass without modification (free-list is backward compatible; it simply starts empty and accumulates naturally during reduction).
8. **Add new tests** for free-list-specific scenarios (Section 7).

### 6.2 SparseNet

1. **Create `src/net/sparse.rs`** with the `SparseNet` type and its operations.
2. **Add `mod sparse;` to `src/net/mod.rs`** and export `SparseNet`.
3. **Implement conversion functions** `Net::to_sparse()` and `SparseNet::to_dense()`.
4. **Optionally integrate with `build_subnet`** (R22) behind a configuration flag (R30).
5. **Add tests** for SparseNet operations, invariant preservation, and conversion round-trips.

### 6.3 Ordering

The free-list (6.1) SHOULD be implemented first because it is simpler (~150 LoC vs ~600 LoC), directly benefits the reduction phase (which is the hot path), and does not require a new type. SparseNet (6.2) MAY be implemented independently afterward.

---

## 7. Test Strategy

### 7.1 Free-List Tests

**T1. Basic recycling.** Create 3 agents, remove the middle one, create a new agent. Assert the new agent reuses the middle ID. Assert `agents[middle_id]` is `Some` with the new symbol. Assert the free-list is empty after reuse.

**T2. LIFO ordering.** Create 5 agents (IDs 0-4), remove IDs 1, 3, 2 (in that order). Create 3 new agents. Assert they receive IDs 2, 3, 1 (LIFO pop order). Assert `next_id` is unchanged at 5. *Note (SC-020):* T2 is a pure-driver test that verifies the LIFO contract from R5; the call order (`create`, `remove`, `create`) is fixed by the test. T2 is NOT a guarantee about ID ordering inside reduction rules — see §4.7's intra-rule non-determinism note (SC-019). Tests asserting specific recycled-ID assignments WITHIN a single rule fire are forbidden by R27a / SC-019.

**T3. Free-list exhaustion.** Create 3 agents, remove all 3 (free-list has 3 entries). Create 5 agents. Assert first 3 reuse recycled IDs, last 2 allocate new IDs from `next_id`.

**T4. Port slot reinitialization.** Create a CON agent (ID=0), connect its ports, remove it (pushed to free-list). Create a new ERA agent. Assert the recycled ERA's auxiliary port slots are `DISCONNECTED`. Assert no stale connections from the previous CON agent exist in the port array.

**T5. Reduction with recycling.** Build a net with 100 CON-CON annihilation pairs. Run `reduce_all`. Assert `next_id` is 200 (no new IDs allocated — all annihilations are -2 agents). Assert the free-list contains 200 entries. Assert `count_live_agents() == 0`.

**T6. Commutation recycling.** Build a CON-DUP active pair. Reduce it. Assert the free-list has 0 entries (2 removed, 2 of the 4 created agents reused the freed slots). Assert `next_id == 4` (original 2 + 2 new IDs for the remaining 2 agents).

**T7. Invariant T1 after recycling.** Build a non-trivial net (e.g., Church(3) + Church(2) addition). Run `reduce_all` with free-list enabled. Assert the normal form passes T1 (bidirectionality), I2 (reference validity), and I3' (uniqueness) assertions.

**T7a. CON-DUP under partial free-list (closes SC-010).** Pre-populate a net with a CON-DUP redex AND 2 IDs in the free-list. Reduce the redex once. Assert the 4 new agents satisfy I3' (uniqueness — `agents[id].is_some()` for all 4 returned IDs and no duplicates), but DO NOT assert any monotonicity relation between the 4 returned IDs. Assert that no `assert!(new_id > old_max)` pattern in `src/reduction/` fires (verified by running with `cargo test --release` AND `cargo test` in debug, both passing).

**T8. Serialization round-trip (closes SC-014).** Build a net, reduce partially (some free-list entries), serialize via serde + bincode, deserialize. Assert the deserialized net is `is_behaviorally_equal` to the original AND the deserialized free-list matches the original (set equality, since LIFO order is preserved by `Vec` serde). Assert continuing reduction from the deserialized net produces a normal form `is_behaviorally_equal` to continuing reduction from the original.

**T8a. SC-007 wire-version rejection.** Serialize a v3-format net (with non-empty free-list) using `PROTOCOL_VERSION = 3`. Attempt deserialization with a `PROTOCOL_VERSION = 2` deserializer (simulated via the SPEC-18 version-mismatch handshake). Assert `UnsupportedVersion` error is returned, NOT a length-mismatch or silent-drop.

**T9. Distributed ID range compliance.** Create a net, partition it with ID ranges `[0, 100)` and `[100, 200)`. During reduction on partition 0, verify that recycled IDs are all in `[0, 100)`. Verify that partition 1's recycled IDs are all in `[100, 200)`.

**T9a. BorderGraph protected tombstone (closes SC-005, Strategy A).** Set up a 2-partition delta-mode scenario with `RecyclePolicy::DisableUnderDelta` (default). Worker 0 owns IDs `[0, 100)` with a border at `AgentPort(47, 0)`. During round N, simulate agent 47 being consumed by a local rule (e.g., its principal-port partner is consumed too). Assert: (a) `worker_0.net.agents[47] == None`, (b) `worker_0.net.free_list` does NOT contain 47, (c) on the next `create_agent`, the returned ID is NOT 47, (d) after `reconstruct` at the next clean boundary, ID 47 is reclaimable. This validates R10c (protected tombstone semantics).

**T9b. BorderGraph border-clean (closes SC-005, Strategy B).** Repeat T9a with `RecyclePolicy::BorderClean`. Pre-populate `worker_0`'s `border_entries` HashSet with `{47}`. Trigger `remove_agent(47)`. Assert: (a) free-list does NOT contain 47, (b) free-list still receives non-border-referenced freed IDs (e.g., remove ID 50, which is not in `border_entries`; assert 50 IS in free-list).

**T10. Free-list no-duplicate invariant.** Remove the same agent twice (e.g., via direct manipulation in test). Assert that the debug assertion catches the duplicate free-list entry.

### 7.2 SparseNet Tests

**T11. Construction and agent count.** Create a `SparseNet`, add 5 agents of various symbols. Assert `count_live_agents() == 5`. Remove 2 agents. Assert `count_live_agents() == 3`. Assert no tombstones exist (memory is proportional to live agents).

**T12. Bidirectionality (I1 for SparseNet).** Create 3 agents in a SparseNet, connect them in a chain. For every port entry `(id, p) -> target` in the ports HashMap, assert the reverse entry exists. Assert T1 holds.

**T13. ERA cleanliness (I6 for SparseNet).** Create an ERA agent in a SparseNet. Assert that `ports.get(&(era_id, 1))` and `ports.get(&(era_id, 2))` return `None`. Connect the ERA's principal port. Assert auxiliary port entries still do not exist.

**T14. Conversion round-trip (Dense -> Sparse -> Dense).** Build a dense `Net` with 10 agents (including some `None` slots from removals). Convert to `SparseNet`, then back to `Net` via `to_dense(None)`. Assert the result is `is_behaviorally_equal` to the original (closes SC-014 ambiguity). Assert `freeport_redirects` is preserved bit-exact (closes SC-001 second-surface).

**T14a. Partition-scoped to_dense (closes SC-006).** Build a `SparseNet` containing agents at IDs `{50, 51, 75, 99, 130, 175}`. Call `to_dense(Some(50..100))`. Assert the resulting `Net.free_list` is exactly `{52, 53, ..., 74, 76, ..., 98}` (the `None` slots within `[50, 100)`); IDs in `[0, 50)` and `[100, max_id]` MUST NOT appear. Also call `to_dense(Some(100..200))` on the same sparse net and assert the resulting free-list is exactly `{100, 101, ..., 129, 131, ..., 174, 176, ..., 199}`.

**T15. Conversion round-trip (Sparse -> Dense -> Sparse).** Build a `SparseNet` with 10 agents. Convert to `Net` via `to_dense(None)`, then back to `SparseNet`. Assert structural equality (full `==`, since `SparseNet` representations have no trailing-slot ambiguity).

**T16. Sparse build_subnet.** Build a large net (1000 agents), partition it for 4 workers using `SparseNet`-based construction. Convert each partition to `Net`. Reduce all partitions. Merge. Assert the result is isomorphic to sequential reduction of the original net (G1).

**T17. Redex detection in SparseNet.** Create 2 agents in a SparseNet, connect their principal ports. Assert the redex queue contains the pair. Create 2 more agents, connect auxiliary-to-auxiliary. Assert the redex queue still has exactly 1 entry.

**T18. Serialization round-trip for SparseNet.** Build a SparseNet, serialize with serde + bincode, deserialize. Assert structural equality.

---

## 8. Open Questions

**Q1. RESOLVED (closes SC-011).** `SparseNet` MUST carry `freeport_redirects: HashMap<u32, PortRef>` as a field (R13). Conversions are lossless. `SparseNet`-based `build_subnet` (R22) is consumable by `merge` (SPEC-05) without losing border-redirect state. **No remaining ambiguity.**

**Q2. Should `SparseNet` implement a trait shared with `Net`?** A `NetOps` trait with `create_agent`, `connect`, `get_target`, etc. would allow polymorphic code. However, SPEC-02 does not define such a trait, and adding one is a larger architectural change. The trait approach is cleaner but adds indirection. **Decision deferred to SPEC review. If adopted, it should be a separate spec amendment.** (Round 1 reviewer: ACCEPT defer — refactoring concern, not implementability concern.)

**Q3. RESOLVED against M5 (closes SC-015).** R32 commits the v1 path to no-cap (40 MB at 10M is acceptable, verified) AND commits the M5 path to a `bitvec::BitVec` representation when `free_list.len() * 4 > MAX_FREELIST_BYTES = 64 MB` (12.5 MB at 100M, vs 400 MB unbounded). Implementations MAY ship the `Vec` representation for v1 milestones; v2 / M5 implementations MUST ship the bitmap fallback.

**Q4. Should the free-list be sorted for deterministic ID allocation?** LIFO ordering means the order of ID reuse depends on the order of `remove_agent` calls, which depends on reduction strategy. This does not affect correctness (I3' guarantees uniqueness regardless of allocation order), but it means two runs with different reduction orders may assign different IDs to the same logical agents. This is already true with the current monotonic allocation (different reduction orders create agents at different `next_id` values). **No sorting needed. Determinism of IDs is not a requirement; determinism of normal form topology is (T6/G1). Acknowledgement: existing v1 tests asserting specific `AgentId` values may need adjustment; this is a Stage 1 task-splitter scope item, not a SPEC-22 defect.**

**Q5. SPEC-23 forward-compatibility (closes SC-021).** SPEC-23 (`Compact Memory Representation`) will replace `enum PortRef` with `pub struct PortRef(u32)` (bit-packed). The `SparseNet::ports` HashMap key `(AgentId, PortId)` may benefit from migrating to a single `u32` packed key when SPEC-23 lands. SPEC-22 ships with semantic enum keys; the compact key migration is gated on SPEC-23 landing. SPEC-23's frontmatter already declares `Amends: SPEC-22`; the actual amendment text will be authored at SPEC-23 review time. SPEC-22 has no further obligation here beyond the explicit hand-off.

---

## 11. Change Log

### Round 2 — 2026-04-25 — Closure pass

Closure of all 21 findings from `SPEC-REVIEW-22-round-1-2026-04-24.md` (verdict BLOCK; 4 CRITICAL, 7 HIGH, 6 MEDIUM, 4 LOW).

| Finding | Severity | Verdict | Where addressed |
|---------|----------|---------|-----------------|
| SC-001 | CRITICAL | CLOSED | §4.1 Net struct now includes `freeport_redirects` (full struct definition, with rkyv attrs). §4.6 `to_dense()` and `to_sparse()` preserve the field. §4.3 `remove_agent` purges the entry on recycle. |
| SC-002 | CRITICAL | CLOSED | Frontmatter `Amends:` extended; §3.8 A2 (SPEC-02 R2) and A3 (SPEC-02 R10) author the structured Old/New text triples. |
| SC-003 | CRITICAL | CLOSED | Frontmatter `Depends on:` now lists SPEC-04, SPEC-05, SPEC-18, SPEC-19. |
| SC-004 | CRITICAL | CLOSED | §3.8 Amendments to Predecessor Specs block authored, populated with A1-A10 (10 amendments) following the SPEC-20 §3.8 / SPEC-19 §3.8 canonical pattern. |
| SC-005 | HIGH | CLOSED | §3.1 R10b adds the BorderGraph slot-id-stability constraint with two normative strategies (`DisableUnderDelta` / `BorderClean`); R10c defines protected-tombstone semantics. §3.8 A10 records the SPEC-19 amendment. T9a + T9b validate. |
| SC-006 | HIGH | CLOSED | §4.6 `to_dense(id_range: Option<Range<AgentId>>)` signature change; free-list population is now scoped to `id_range`. R10a upgraded to MUST. T14a validates partition-scoped behaviour. |
| SC-007 | HIGH | CLOSED | §3.1 R9a mandates SPEC-18 PROTOCOL_VERSION bump 2→3 with v2-vs-v3 rejection clause (mirrors SPEC-20 R37). §3.8 A9 amends SPEC-18 R28. T8a validates rejection. |
| SC-008 | HIGH | CLOSED | R23 demoted from MUST NOT to design constraint with CI-enforceable lint (`src/reduction/**/*.rs` MUST NOT import `SparseNet`). |
| SC-009 | HIGH | CLOSED | R10a + R22 upgraded SHOULD → MUST under the `id_range > 4 × live_agent_count` threshold. R30 strengthens `sparse_build` flag with rejection at threshold. §3.8 A4 amends SPEC-02 R11 to disambiguate vs SPEC-22 R3. |
| SC-010 | HIGH | CLOSED | R27a authored: SPEC-03 debug assertions reformulated as I3'-compatible; A6 in §3.8 records the SPEC-03 amendment. T7a validates CON-DUP under partial free-list. |
| SC-011 | HIGH | CLOSED | R13 SparseNet field list now includes `freeport_redirects`; §4.4 struct + §4.5 constructors updated. Q1 in §8 marked RESOLVED. |
| SC-012 | MEDIUM | CLOSED | Frontmatter `References consumed:` + new `Code analyses consumed:` (AC-001, AC-006, AC-009, AC-011, AC-015) + `Arguments consumed:` (ARG-002, ARG-005). |
| SC-013 | LOW | DEFERRED (TCC-root cleanup, acknowledged) | Per Round 1 prompt, theory-bridge stale "SPEC-22 (Job submission)" tag at line 142 is TCC-root territory, NOT SPEC-22 author scope. Acknowledged here; the bridge edit will be picked up by the bridge maintainer. SPEC-22 takes no action on theory-bridge.md. |
| SC-014 | MEDIUM | CLOSED | R21 redefined in terms of `is_behaviorally_equal`; helper signature mandated; T8 / T14 updated to use the helper. |
| SC-015 | MEDIUM | CLOSED | R32 authored (free-list memory budget at M5: bitmap fallback when `free_list.len() * 4 > 64 MB`). Q3 in §8 marked RESOLVED. |
| SC-016 | MEDIUM | CLOSED | §4.4 Send + Sync paragraph mandates `static_assertions::assert_impl_all!(SparseNet: Send, Sync)` and the same for the post-SPEC-22 `Net`. |
| SC-017 | MEDIUM | CLOSED | R31 authored: SPEC-22 implementations MUST be safe-Rust-only; `unsafe` deferred to SPEC-23. |
| SC-018 | LOW | CLOSED | R6 SHOULD → MUST with explicit `debug_assert!(!self.free_list.contains(&id))` location and optional `HashSet<AgentId>` shadow under `#[cfg(debug_assertions)]`. |
| SC-019 | LOW | CLOSED | §4.7 intra-rule order non-determinism note authored; tests forbidden from asserting specific recycled-ID assignments within a rule fire. |
| SC-020 | LOW | CLOSED | T2 annotated with R5-LIFO coupling note; cross-references SC-019. |
| SC-021 | LOW | CLOSED | Q5 in §8 added: SPEC-23 forward-compatibility hand-off documented. |

**Status transition:** Draft → Reviewed v2.

**Closure verdict:** All CRITICAL (4/4) and all HIGH (7/7) findings CLOSED inline. 5/6 MEDIUM CLOSED inline; 1 MEDIUM (SC-013) DEFERRED with explicit gating (it is downstream theory-bridge cleanup, not SPEC-22 territory). All 4 LOW CLOSED inline. 0 NOT_CLOSED. No new fresh findings (NF-NNN) were introduced by the revision (the closure log audits this in §3 of `SPEC-REVIEW-22-round-2-2026-04-25.md`).

**Notable scope additions in this round:**
- §3.5 Cross-cutting MUSTs (R31, R32) — new section.
- §3.8 Amendments to Predecessor Specs (A1-A10) — new section (SPEC-19/SPEC-20 canonical pattern).
- §4.6 `to_dense` signature change: `to_dense(&self) -> Net` → `to_dense(&self, id_range: Option<Range<AgentId>>) -> Net`. Call-site migration is documented at the end of §4.6.
- §4.4 `SparseNet` struct extended with `freeport_redirects` field.
- §4.3 `remove_agent` extended with `is_border_protected` guard and `freeport_redirects` purge.

**Stage gate:** This closure log lands BEFORE Stage 1 (TASK-SPLITTER) and Stage 2 (TEST-GENERATOR) per the SDD pipeline contract.
