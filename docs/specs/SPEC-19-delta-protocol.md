# SPEC-19: Delta Protocol and Stateful Workers

**Status:** Draft — §3.2 R8-R12 amended per SPEC-22 §3.8 A10 (BorderGraph recycle-policy-aware contract); §3.2 amended per SPEC-21 §3.8 A7 (BorderGraph gains `extend_with_chunk_borders` method); §3.4 R35a added per D-011 Phase A (CompactSubnet `free_list` wire encoding; PROTOCOL_VERSION bump to `PREVIOUS_LIVE_VERSION + 1`)
**Depends on:** SPEC-01 (Invariants), SPEC-05 (Merge and Grid Cycle), SPEC-06 (Wire Protocol), SPEC-17 (Transport Abstraction), SPEC-18 (Wire Format v2)
**Amends:** SPEC-22 §3.8 A10 (§3.2 BorderGraph — recycle-protection under delta mode; RecyclePolicy, protected tombstones); SPEC-21 §3.8 A7 (§3.2 BorderGraph gains `extend_with_chunk_borders(&mut self, new_borders: &HashMap<u32, (PortRef, PortRef)>)` for incremental border merging under `delta_mode && streaming_active`; SPEC-21 R37f)
**Amends:** SPEC-01 G1, D3, D6; SPEC-05 R24-R30a; SPEC-06 R2, R3
**ROADMAP items:** 2.26 (Delta-Only Protocol with Stateful Workers), 2.34 (Coordinator-Free Round), 2.35 (Delta-Based Merge with BorderGraph)
**References consumed:** REF-001 (Lafont 1990), REF-002 (Lafont 1997), REF-003 (HVM2), REF-005 (Mackie & Pinto 2002), REF-013 (Mackie 1997), REF-014 (Kahl 2015)
**Discussions consumed:** DISC-005 v2 (cross-boundary protocol), DISC-006 v2 (communication overhead, break-even analysis), DISC-008 v2 (serialization as operational cost)
**Arguments consumed:** ARG-001 (central argument, P1-P6), ARG-002 (partitioning, C1-C3), ARG-003 (merge protocol completeness), ARG-004 (viability, overhead decomposition), **ARG-005** (delta border completeness; closes §8 OQ-1 / R38 / R39 / R40 — added 2026-04-24). See `codigo/relativist/docs/theory-bridge.md` for absolute paths.
**Briefings consumed:** BRIEF-20260415-v2-codebase-assessment (Sections 1, 3, 4.2, 6), BRIEF-20260415-v2-fundamentacao-teorica (Gap 1, P2/P3 analysis, Tier 4)
**External references:** Pregel (Malewicz et al. 2010), Giraph (Ching et al. 2015), LCC-BSP (Frontiers of CS 2018), Haeupler et al. (CMU, round-optimal distributed graph algorithms)

---

## 1. Purpose

This spec defines the delta protocol for Relativist's distributed reduction: a protocol where workers are **stateful** (they retain their partition across BSP rounds), communication carries only **border deltas** (changes to border port connectivity) instead of full partitions, and the coordinator maintains a lightweight **BorderGraph** instead of the full merged net. The delta protocol replaces the v1 full-partition-per-round BSP cycle (SPEC-05 R24-R30a, SPEC-06 R2-R3) with three layered features:

1. **Coordinator-Free Round** (Section 3.1): When all workers report zero border redexes after local reduction, the coordinator skips the merge-redistribute cycle entirely. Workers keep their partitions and continue reducing locally. This is the simplest entry point to stateful workers.

2. **BorderGraph and Delta-Based Merge** (Section 3.2): The coordinator holds a `BorderGraph` data structure tracking inter-partition border connectivity instead of the full merged net. Workers send border deltas after each round rather than full partitions. When a border redex is detected (two sides of a border wire are both principal ports), the coordinator dispatches it to the appropriate worker for resolution. This replaces the full `merge() + split()` cycle for most rounds.

3. **Delta-Only Protocol** (Section 3.3): The complete stateful worker protocol. Round 0 sends an `InitialPartition` (one-time full partition). Rounds 1+ send `RoundStart` with border deltas only. Workers report `RoundResult` with border deltas and stats (no full partition). At convergence, the coordinator sends `FinalStateRequest` and workers return their full partition for a final merge. Wire cost drops from O(N) per round to O(border_changes) per round.

The v1 BSP cycle ships the entire partition to each worker every round, producing wire cost proportional to partition size regardless of how much work was done. For workloads like `dual_tree` and `cascade_cross` under strict BSP, 95%+ of the bytes on the wire are retransmitted identical data (ROADMAP 2.26). The delta protocol eliminates this waste by transmitting only what changed, achieving the architectural pattern used by Pregel, Giraph, and modern BSP graph processing frameworks.

The correctness of the delta protocol rests on strong confluence (SPEC-01, T4 = P1): the distributed state (coordinator's border graph + union of worker partition interiors) is a well-defined decomposition of the sequential intermediate state, unique up to confluence equivalence. The formal proof requires a new decomposition argument (see Section 3.5, invariant amendments, and Section 8, open questions).

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary), SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-05 (Merge and Grid Cycle), and SPEC-06 (Wire Protocol) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Stateful Worker** | A worker that retains its partition in local memory across BSP rounds. In v1, workers are stateless: they receive a full partition each round via `AssignPartition` and return it via `PartitionResult`. Under the delta protocol, workers receive a full partition once (`InitialPartition` at round 0) and thereafter receive only delta updates (`RoundStart`). |
| **Border Delta** | A change to the connectivity of a border port during a round. Formally: a tuple `(border_id: u32, new_target: PortRef)` indicating that the port previously connected to `FreePort(border_id)` in a given partition is now connected to `new_target`. A border delta is emitted when a local reduction rule reconnects an agent's port that was connected to a FreePort (Boundary) sentinel. |
| **BorderGraph** | A coordinator-side data structure that tracks the connectivity of all border wires across partitions. For each `border_id`, the `BorderGraph` stores the current `PortRef` endpoints on both sides (the two partitions that share that border). The `BorderGraph` is sufficient to detect border redexes (when both endpoints are principal ports) without reconstructing the full merged net. |
| **BorderState** | The state of a single border wire within the `BorderGraph`. Contains the two current endpoints (`side_a: PortRef`, `side_b: PortRef`), the worker IDs that own each side (`worker_a: WorkerId`, `worker_b: WorkerId`), and a derived flag indicating whether the border is an active pair (`is_redex: bool`). |
| **Coordinator-Free Round** | A BSP round where all workers report zero border redexes and the coordinator determines that no merge is needed. Workers retain their partitions and continue reducing locally (or, if they also report zero local redexes, the net has converged to normal form). The coordinator skips the merge-redistribute cycle entirely. |
| **Delta BSP Loop** | The modified BSP loop that replaces the v1 full-partition loop. Instead of the cycle split -> distribute -> reduce_local -> collect -> merge -> resolve_borders, the delta BSP loop follows: init_dispatch -> {reduce_local -> report_deltas -> update_border_graph -> dispatch_border_redexes} -> final_collect -> final_merge. |
| **Internally Stable** | A worker partition is internally stable when it has zero local redexes remaining after `reduce_all`. An internally stable partition may still have border ports whose remote endpoints form active pairs, detectable only by the coordinator via the `BorderGraph`. |
| **Global Normal Form** | The state where all workers are internally stable AND the `BorderGraph` contains zero active pairs. This is the distributed equivalent of the net having no redexes. |
| **Initial Dispatch** | The one-time operation at round 0 where the coordinator sends each worker its full `InitialPartition`. This is the only time the complete partition crosses the wire (until the final state collection at convergence). |
| **Final State Collection** | The convergence-time operation where the coordinator requests full partition state from all workers via `FinalStateRequest` / `FinalStateResult`, performs a final `merge()`, and produces the output net. |

---

## 3. Requirements

### 3.1 Coordinator-Free Round

This is the simplest entry point to stateful workers. It can be implemented independently of the full delta protocol (ROADMAP 2.34).

**R1.** After each round of local reduction, each worker MUST inspect its `free_port_index` to determine whether any border port participates in a potential active pair. A border port participates in a potential active pair if the port connected to a `FreePort(border_id)` is a principal port (`AgentPort(_, 0)`). The worker reports this status in its round result. **(MUST)**

**R2.** The worker's round result MUST include a `has_border_activity: bool` field. This field MUST be `true` if any border port's local endpoint is a principal port (i.e., the local side of some border wire could potentially participate in a border redex, pending the remote side's state). If no border port has a principal port endpoint, the field MUST be `false`. **(MUST)**

**R3.** The coordinator MUST track per-worker border activity status after each round. When ALL workers report `has_border_activity: false`, the coordinator MAY skip the merge-redistribute cycle for that round. The workers retain their partition state and proceed to reduce locally in the next round (which will be a no-op if they are already internally stable). **(MUST for tracking; MAY for skip)**

**R4.** When ALL workers report both `has_border_activity: false` AND zero local redexes (`local_redexes: 0` in `WorkerRoundStats`), the coordinator MUST conclude that the net has reached Global Normal Form. The coordinator MUST then initiate Final State Collection (R27-R29) to produce the output. **(MUST)**

**R5.** The coordinator-free round optimization MUST NOT alter the final result. Whether the coordinator skips a merge round or performs it, the same Normal Form MUST be reached (guaranteed by strong confluence, SPEC-01 T4). **(MUST)**

**R6.** In strict BSP mode (`GridConfig.strict_bsp = true`), the coordinator-free round SHOULD be the primary convergence acceleration mechanism: rounds where no borders are active avoid the O(A_total + B) merge cost entirely. **(SHOULD)**

**R7.** The coordinator-free round MUST be compatible with both the v1 full-partition protocol and the v2 delta protocol. Under v1, the coordinator simply does not send `AssignPartition` for the skipped round (workers retain their state from the previous `AssignPartition`). Under v2, the coordinator sends a `RoundStart` with empty border deltas. **(MUST)**

### 3.2 BorderGraph and Delta-Based Merge

This section specifies the `BorderGraph` data structure and the delta-based merge protocol that replaces the full `merge() + split()` cycle (ROADMAP 2.35).

**R8.** The system MUST define a `BorderGraph` data structure in the `merge` module (e.g., `src/merge/border_graph.rs`) that tracks the connectivity state of all border wires. The `BorderGraph` is the coordinator's lightweight representation of inter-partition connectivity, replacing the full merged net for rounds 1+. **(MUST)**

**R9.** For each `border_id` in the original partition plan's border map, the `BorderGraph` MUST store a `BorderState` containing:
- `border_id: u32` -- the border identifier.
- `side_a: PortRef` -- the current port endpoint on one side of the border.
- `side_b: PortRef` -- the current port endpoint on the other side of the border.
- `worker_a: WorkerId` -- the worker that owns the `side_a` endpoint.
- `worker_b: WorkerId` -- the worker that owns the `side_b` endpoint.
- `is_redex: bool` -- derived flag: `true` if and only if both `side_a` and `side_b` are principal ports (`AgentPort(_, 0)`).
**(MUST)**

**R10.** The `BorderGraph` MUST be initialized from the `PartitionPlan`'s border map and `free_port_index` data at the end of the initial partitioning phase (round 0). The initialization MUST populate all `BorderState` entries with the endpoints as they exist after `split()`. **(MUST)**

**R11.** The `BorderGraph` MUST provide a method `apply_deltas(worker_id: WorkerId, deltas: &[BorderDelta])` that updates border states based on worker-reported changes. For each delta `(border_id, new_target)`:
- If the `worker_id` owns `side_a` of `border_id`, update `side_a = new_target`.
- If the `worker_id` owns `side_b` of `border_id`, update `side_b = new_target`.
- Recompute `is_redex` after the update.
**(MUST)**

**R12.** The `BorderGraph` MUST provide a method `detect_border_redexes() -> Vec<(u32, BorderState)>` that returns all border entries where `is_redex == true`. This replaces the exhaustive border scan in the full merge (SPEC-05 R12). **(MUST)**

**R12a. BorderGraph — Recycle-Policy-Aware Contract (Amendment A10)**

> **Amendment A10 (SPEC-22 §3.8 A10 / R10b, R10c):** When `GridConfig.delta_mode == true`, worker-side free-list recycling is constrained as follows to preserve `BorderGraph` slot-id stability:
>
> - **Strategy A (`RecyclePolicy::DisableUnderDelta`, default):** Workers MUST NOT pop from the free-list during a delta-mode round. `create_agent` falls back to `next_id` allocation. The free-list still accumulates pushes from `remove_agent` and is drained at the next clean partition boundary (after `reconstruct` per R38).
> - **Strategy B (`RecyclePolicy::BorderClean`):** Workers MAY pop from the free-list only for IDs not present in `border_entries` (partition-local `HashSet<AgentId>` shadow per SPEC-04 R20-R22). If the popped ID is border-referenced, it MUST be re-pushed to the free-list (or stored in a side-list for reuse after the next `reconstruct`) and a fresh `next_id` MUST be allocated instead.
>
> The choice between strategies is a `GridConfig.recycle_under_delta: RecyclePolicy` field (default: `RecyclePolicy::DisableUnderDelta` = Strategy A, the conservative choice). Strategy B is opt-in for benchmarks.
>
> **Protected tombstones (R10c):** When `remove_agent(id)` is called on a worker whose `id` IS border-referenced in the coordinator's `BorderGraph`, the agent's port slots MUST still be set to `DISCONNECTED`, the slot MUST be set to `agents[id] = None`, but the ID MUST NOT be pushed to the free-list. The slot becomes a *protected tombstone* that persists until the next `reconstruct`/clean-boundary moment.
>
> **Threat model prevented:** Round N produces border `B = (border_id, AgentPort(47, 0), AgentPort(123, 0))`; in round N+1 a worker recycles ID 47 to a different `Symbol`; coordinator dispatches a `CommutationBatch` indexing `AgentPort(47, 0)`; the worker's local `agents[47]` now resolves to a different rule than the BorderGraph computed → G1 violation. R12a/R10b prevent the recycle in step 2. Closes SC-005. See SPEC-22 R10b, R10c.

**(MUST)**

**R12b. BorderGraph — Incremental Extension API for Streaming Pipeline (Amendment A7)**

> **Amendment A7 (SPEC-21 §3.8 A7 / R37f):** The `BorderGraph` MUST provide a method:
>
> ```rust
> pub fn extend_with_chunk_borders(
>     &mut self,
>     new_borders: &HashMap<u32, (PortRef, PortRef)>,
> )
> ```
>
> **Method semantics:**
> - Merges new border entries into the existing `BorderGraph`, populating one fresh `BorderState` per `(border_id, (side_a, side_b))` pair using the same convention as R10 (initialization from a `PartitionPlan`'s border map).
> - MUST be called by the coordinator after each `install_connection` invocation (SPEC-21 §4.6) that yields a border wire under the conjunction `delta_mode && streaming_active`, BEFORE the next chunk's `AssignPartition` is dispatched.
> - **Idempotent on previously-seen border IDs:** if a `border_id` already exists in the graph, the method MUST leave its `BorderState` unchanged (preserving any deltas already applied via R11 `apply_deltas`).
> - **No-op if `new_borders.is_empty()`:** safe to call unconditionally.
>
> **Call-site discipline (per SPEC-21 R37f).** Without this extension API, the coordinator's `BorderGraph` becomes stale after chunk 1 under combined delta+streaming, missing cross-chunk active pairs and silently violating G1. The when-to-call contract is owned by SPEC-21 R37f; the implementation contract is owned by SPEC-19 (this requirement). The actual call-site production is TASK-0588.
>
> **Ownership split (per SC-017 closure).** SPEC-19 owns the `extend_with_chunk_borders` implementation; SPEC-21 owns the call-site discipline.

**(MUST)**

**R13.** When a border redex is detected (both endpoints are principal ports), the coordinator MUST resolve it. The resolution strategy MUST use option (a) from ROADMAP 2.35: the coordinator requests the two involved agents from their respective workers, performs the interaction locally using the standard reduction rules (SPEC-03), and sends the resulting port reconnections back to the workers as deltas. This stays within the star topology and does not require worker-to-worker communication. **(MUST)**

**R14.** The border redex resolution at the coordinator MUST use the same 6 interaction rules as local reduction (SPEC-01, T5; SPEC-03). The coordinator MUST have access to the `interact_*` functions. Border redex resolution at the coordinator does NOT violate the Core/Infrastructure layer boundary because the coordinator already calls `merge()` and `reduce_all()` in v1 (SPEC-13 R6-R8 permit coordinator access to core modules). **(MUST)**

**R15.** After resolving a border redex at the coordinator, the coordinator MUST:
1. Send the resulting port updates to the affected workers as deltas (new agent connections replacing the consumed agents' connections).
2. Update the `BorderGraph` to reflect the resolution: the consumed border entry is removed or updated based on the new topology.
3. If the resolution creates new border wires (possible in CON-DUP commutation, which creates 4 new agents with auxiliary ports that may inherit FreePort connections), the coordinator MUST add new `BorderState` entries to the `BorderGraph` for these new borders.
**(MUST)**

**R16.** The `BorderGraph` MUST handle border removal. When a border redex is resolved by an annihilation rule (CON-CON, DUP-DUP) or void rule (ERA-ERA), both agents are consumed and the border wire ceases to exist. The coordinator MUST remove the corresponding `BorderState` entry. **(MUST)**

**R17.** The `BorderGraph` MUST handle border erasure. When a reduction rule within a worker erases an agent connected to a `FreePort(border_id)`, the worker reports a delta `(border_id, DISCONNECTED)` or `(border_id, None)`. The coordinator MUST handle this by marking the border side as disconnected. If both sides are disconnected, the border entry MUST be removed. **(MUST)**

**R18.** The `BorderGraph` MUST have time complexity O(B) for initialization (where B is the number of borders), O(|deltas|) for `apply_deltas`, and O(B) worst-case for `detect_border_redexes`. In practice, the redex detection SHOULD be incremental: maintain a set of border IDs with `is_redex == true`, updated during `apply_deltas`, so that `detect_border_redexes` returns the set in O(|redex_set|) time. **(MUST for complexity bounds; SHOULD for incremental detection)**

**R19.** The `BorderGraph` MUST be a pure data structure in the core layer (`src/merge/`). It MUST NOT depend on tokio, async, or I/O. **(MUST)**

### 3.3 Delta-Only Protocol (Stateful Workers)

This section specifies the complete stateful worker protocol (ROADMAP 2.26).

**R20.** The delta protocol MUST be activated via a configuration flag `GridConfig.delta_mode: bool`, defaulting to `false`. When `delta_mode` is `false`, the v1 full-partition protocol (SPEC-05 R24-R30a, SPEC-06 R2-R3) is used unchanged. When `delta_mode` is `true`, the delta BSP loop is used. **(MUST)**

**R21.** The delta BSP loop MUST follow this lifecycle for each worker:

1. **Round 0 (Initial Dispatch):** The coordinator sends `InitialPartition` containing the full `Partition` (one-time). The worker stores this partition in its local state. This is the only time the complete partition crosses the wire until convergence.
2. **Rounds 1+ (Delta Rounds):** The coordinator sends `RoundStart` containing border deltas for this round (new connections to apply, freed border IDs). The worker applies these deltas to its stored partition, runs `reduce_all`, and reports a `RoundResult` with its own border deltas and stats.
3. **Convergence (Final Collection):** When the coordinator determines Global Normal Form (R4), it sends `FinalStateRequest` to each worker. Each worker responds with `FinalStateResult` containing its full current partition. The coordinator performs a final `merge()` (SPEC-05 R1-R11) and produces the output net.

**(MUST)**

**R22.** At round 0, the worker MUST store the received `InitialPartition` in persistent local state (not discarded after reduction). The worker's partition state MUST survive across rounds. The worker MUST NOT require a full partition retransmission for any subsequent round. **(MUST)**

**R23.** At each delta round (rounds 1+), the coordinator MUST send a `RoundStart` message containing:
- `round: u32` -- the round number.
- `border_deltas: Vec<BorderDelta>` -- port reconnections that the worker must apply to its stored partition. Each delta is `(border_id: u32, new_target: PortRef)`, meaning "the remote side of `border_id` is now connected to `new_target`; update the local port connected to `FreePort(border_id)` accordingly." In practice, this means: if a border redex was resolved at the coordinator, the worker receives deltas reflecting the new agents/connections that replaced the consumed agents.
- `resolved_borders: Vec<u32>` -- border IDs whose redexes have been resolved at the coordinator. The worker MUST remove the corresponding `FreePort` entries from its partition (the border wire no longer exists after resolution).
- `new_borders: Vec<(u32, PortRef)>` -- new border entries created by CON-DUP expansion at the coordinator. The worker MUST add corresponding `FreePort` entries to its partition.
- `local_reconnections: Vec<LocalReconnection>` -- internal port reconnections (DC-B3) that the worker must apply to its stored partition. Each entry is `(agent_id, port, new_target)`, meaning "the port `port` of agent `agent_id` is now connected to `new_target`; update the local port array accordingly." These arise when a CON-DUP border-redex resolution at the coordinator produces new agents whose auxiliary ports must be rewired to existing local agents in the worker's partition (non-border internal reconnections that neither `border_deltas`, `resolved_borders`, nor `new_borders` can express).
- `pending_commutations: Vec<PendingCommutation>` -- AgentId allocation requests (DC-B5). Each entry carries `(request_id, target_symbols, local_wiring)` where `target_symbols: Vec<Symbol>` (defined in §3.4 R33, NF-001 Shape A) lists the per-slot symbols of the `target_symbols.len()` siblings the worker MUST mint, and `local_wiring: Vec<LocalWiringHint>` (defined in §3.4 R33) carries the intra-worker edges that the worker MUST apply to those freshly minted agent(s) after allocation, using slot-marker placeholders (see R23a) for sibling references inside the same `PendingCommutation`. Each request means "allocate `target_symbols.len()` new AgentIds (slot `k` minted as a `target_symbols[k]` agent of its native arity), apply the `local_wiring` edges on those agents' ports, and echo the slot-0 minted id back in the next `RoundResult.minted_agents` paired with this `request_id`." The worker MUST allocate from its own reserved `id_range` (SPEC-04) and MUST NOT treat the minted agents as reducible in the round in which they are created if their principal port is a `FreePort(new_bid)` (consistent with DC-B4 border pinning). The FreePort-pinning clause gates ONLY targets of kind `FreePort`; principal-principal redexes between minted siblings produced by `local_wiring` are in-round reducible (see R23a clause 5).
**(MUST)**

**R23a. Slot-to-AgentId substitution and ordering (DC-B5 local wiring).**

The `local_wiring` vector inside each `PendingCommutation` uses a compact per-request slot namespace that the worker MUST resolve to concrete `AgentId` values before calling `Net::connect` (§3.4 R33b). The following five clauses specify the substitution algorithm:

1. **Per-request scope.** `local_wiring` slots are local to the owning `PendingCommutation` (they name siblings inside one `CommutationBatch`). Slots are NOT global to the round. A slot index `s` in batch A has no relationship to slot `s` in batch B. The worker MUST NOT cross-reference slot namespaces across different `PendingCommutation` entries in the same `RoundStart`.
2. **Mint-then-wire ordering.** For each `PendingCommutation` in `RoundStart.pending_commutations`, the worker MUST allocate all `arity` sibling `AgentId` values (via `Net::create_agent`, consuming from the worker's `IdRange` monotonically) BEFORE applying any entry of that request's `local_wiring`. The per-request mint vector `minted_ids_per_pc[k]` holds the `AgentId` allocated for slot `k` of that request.
3. **Slot-marker decoding.** For every `LocalWiringHint { src_slot, src_port, target }` where `target = PortRef::AgentPort(x, p)` with `x >= SLOT_MARKER_BASE`, the worker MUST replace `x` with `minted_ids_per_pc[x - SLOT_MARKER_BASE]` before calling `connect`. If `(x - SLOT_MARKER_BASE) >= pc.target_symbols.len()` the worker MUST reject with `ProtocolError::MalformedLocalWiring` (R33c case 3, symmetric to R48a).
4. **Concrete-AgentId pass-through.** For every `LocalWiringHint { src_slot, src_port, target }` where `target = PortRef::AgentPort(x, p)` with `x < SLOT_MARKER_BASE`, the worker MUST treat `x` as a concrete live `AgentId` in its own partition and pass it unchanged to `connect`. The worker MAY (debug-only) assert that the agent exists in `partition.subnet`; SHOULD-warn (do not reject) on a dangling live id, for symmetry with the resolver's lenient dangling-FreePort path in `border_resolver::emit_erasure_principal`.
5. **In-round reducibility of minted-to-minted principal pairs.** When `local_wiring` connects two minted principal ports (case (a) below), `Net::connect` enqueues the active pair on the local redex queue (`net/core.rs:198-200`). The worker MUST process these redexes in the same round inside `reduce_all` (R24.2). This is consistent with CON-DUP being the only agent-count-increasing rule and with T4 strong confluence: in-round consumption of a minted sibling pair collapses to the same normal form as deferring it. The FreePort-pinning clause in R23 does NOT apply here because the pinned case targets border FreePorts, not local principal pairs.
6. **Duplicate-key detection site (NF-003).** Before the first call to `Net::connect` for a given `PendingCommutation`, the worker MUST construct a `HashSet<(u8, u8)>` over `(hint.src_slot, hint.src_port)` tuples for every entry in `pc.local_wiring`. On insert collision the worker MUST reject with `MalformedLocalWiringReason::DuplicateSourcePort` (R33c case 5) BEFORE any wiring is applied. This ensures a malformed batch is never partially applied -- `Net::connect`'s last-write-wins semantics would otherwise silently mask the duplicate. The check is a pure local pre-pass with no interaction with `reduce_all`.

**R23a determinism guarantee.** The entries in `local_wiring` MUST be applied sequentially in the emitted order. `bincode`'s default `Vec<T>` encoding preserves element order, so this is a spec pin (not a wire-format change). The resolver emits `local_wiring` in the deterministic order of `CommutationBatch` construction (`border_resolver.rs:820-866` for CON-DUP, `:1023-1080` for CON-ERA/DUP-ERA); the worker MUST NOT reorder. A reordered `local_wiring` is a protocol violation and MUST be rejected via R33c case 5 when detectable (e.g., duplicate `(src_slot, src_port)` keys after canonicalization), per the pre-pass site pinned in R23a clause 6.

**(MUST)**

**R24.** At each delta round, the worker MUST:
1. Apply the received `RoundStart` instructions to its stored partition in the following strict sub-order:
   1. Apply `border_deltas` (update port connections on existing border FreePorts).
   2. Remove `resolved_borders` (drop the corresponding `FreePort` entries).
   3. Insert `new_borders` (add the coordinator-minted FreePort entries).
   4. Apply `local_reconnections` (DC-B3) on pre-existing agents.
   5. For each `PendingCommutation pc` in `pending_commutations` (DC-B5), execute the three-step mint-then-wire protocol:
      - **R24.1.6a (mint):** allocate `pc.target_symbols.len()` fresh `AgentId` values from the worker's `IdRange` via `Net::create_agent`. For each `PendingCommutation pc`, the agent at slot `k` MUST be minted with symbol `pc.target_symbols[k]` taken directly from the wire payload (NF-001 Shape A); the worker MUST NOT attempt to reconstruct slot symbols from resolver-side batch layouts. Store the minted ids in `minted_ids_per_pc` in slot order.
      - **R24.1.6b (wire):** apply `pc.local_wiring` in the emitted order (R23a). For each `LocalWiringHint`, compute the source `PortRef::AgentPort(minted_ids_per_pc[hint.src_slot], hint.src_port)`, decode the target per R23a clause 3 or 4, and call `Net::connect(source, target)`. This is the sole sanctioned primitive for applying `local_wiring`; `Net` does NOT expose a `wire_agents` method and Stage 1 MUST NOT add one.
      - **R24.1.6c (echo):** append `MintedAgent { request_id: pc.request_id, minted_agent_id: minted_ids_per_pc[0] }` to the response buffer. The echoed id is the mint-time id (pre-reduction). Even if `reduce_all` (R24.2) consumes the minted agent within the same round, the echoed id MUST be the one allocated in R24.1.6a. D-004's `register_minted_agents` depends on this semantics.
2. Run `reduce_all` on the stored partition (existing behaviour).
3. Rebuild the `free_port_index` (SPEC-05 R20-R22).
4. Compute border deltas: for each `border_id` in its `free_port_index`, if the port connected to `FreePort(border_id)` has changed since the last report, emit a delta `(border_id, new_endpoint)`.
5. Send a `RoundResult` with the computed deltas, the echoed `minted_agents`, and stats.

**R24 ordering invariant.** The five sub-steps of step 1 above MUST run to completion before step 2 (`reduce_all`). In particular, `Net::connect` inside R24.1.6b may enqueue principal-principal redexes between minted siblings; these redexes MUST NOT be drained until R24.2. This preserves R21's single-round semantics and R38's recoverability argument: the distributed intermediate state at the start of R24.2 is isomorphic to the sequential state after the coordinator applied the equivalent `CommutationBatch` centrally.
**(MUST)**

**R25.** The worker MUST track the previous state of each border port to compute deltas. The worker MUST maintain a `previous_border_state: HashMap<u32, PortRef>` that records the last-reported endpoint for each border ID. After `reduce_all` and `rebuild_free_port_index`, the worker compares the current `free_port_index` against `previous_border_state` to identify changes. Only changed entries are emitted as deltas. **(MUST)**

**R26.** The `RoundResult` message sent by the worker MUST contain:
- `round: u32` -- the round number.
- `border_deltas: Vec<BorderDelta>` -- the border changes detected in this round.
- `stats: WorkerRoundStats` -- the same per-worker statistics as v1 (SPEC-05 R37).
- `has_border_activity: bool` -- whether any border port has a principal-port endpoint (R2).
- `minted_agents: Vec<MintedAgent>` -- fulfilled AgentId allocation responses (DC-B5). Each entry is `(request_id, minted_agent_id)`, pairing each `PendingCommutation` request received in the corresponding `RoundStart` with the `AgentId` the worker has allocated from its `id_range` for that request. The vector MUST be empty in rounds where no `pending_commutations` were received. Every `request_id` in this vector MUST correspond to a request in the prior `RoundStart.pending_commutations` for the same round; orphan or duplicate `request_id` values are a protocol violation (see R44).
**(MUST)**

**R27.** At convergence (Global Normal Form, R4), the coordinator MUST send `FinalStateRequest` to every worker. **(MUST)**

**R28.** Upon receiving `FinalStateRequest`, each worker MUST respond with `FinalStateResult` containing its current full partition (the same `Partition` type used in v1's `PartitionResult`). **(MUST)**

**R29.** The coordinator MUST collect all `FinalStateResult` messages, construct a `PartitionPlan` from the collected partitions and the `BorderGraph`'s remaining border map, and invoke `merge()` (SPEC-05 R1-R11) to produce the final output net. If the `BorderGraph` has zero remaining borders (all resolved), the merge is trivial (union of agents with no boundary reconnection). **(MUST)**

**R30.** The delta protocol MUST preserve the v1 capability of `max_rounds` (SPEC-05 R29). If `max_rounds` is reached without convergence, the coordinator MUST initiate Final State Collection (R27-R29) and return the partially reduced net with a non-convergence indicator in metrics. **(MUST)**

### 3.4 Wire Protocol Extensions

This section defines the new `Message` variants required by the delta protocol. These MUST be appended to the existing `Message` enum after `RegisterNack` (discriminant 6) to preserve discriminant stability (SPEC-06 R5). The discriminant assignments here assume SPEC-18 has already appended its variants; if SPEC-18 and SPEC-19 are implemented together, the discriminant numbering MUST be coordinated.

**R31.** The `Message` enum MUST be extended with the following coordinator-to-worker variants:

| Discriminant | Variant | Direction | Payload |
|:---:|---------|:---------:|---------|
| 7 | `InitialPartition` | C->W | `round: u32` (always 0), `partition: Partition` |
| 8 | `RoundStart` | C->W | `round: u32`, `border_deltas: Vec<BorderDelta>`, `resolved_borders: Vec<u32>`, `new_borders: Vec<(u32, PortRef)>`, `local_reconnections: Vec<LocalReconnection>` (DC-B3), `pending_commutations: Vec<PendingCommutation>` (DC-B5, each PC carries `local_wiring: Vec<LocalWiringHint>` -- see R33) |
| 10 | `FinalStateRequest` | C->W | `round: u32` |

**(MUST)**

**R32.** The `Message` enum MUST be extended with the following worker-to-coordinator variants:

| Discriminant | Variant | Direction | Payload |
|:---:|---------|:---------:|---------|
| 9 | `RoundResult` | W->C | `round: u32`, `border_deltas: Vec<BorderDelta>`, `stats: WorkerRoundStats`, `has_border_activity: bool`, `minted_agents: Vec<MintedAgent>` (DC-B5) |
| 11 | `FinalStateResult` | W->C | `round: u32`, `partition: Partition` |

**(MUST)**

**R33.** The `BorderDelta` type MUST be defined as:
```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BorderDelta {
    pub border_id: u32,
    pub new_target: PortRef,
}
```

The following additional wire types MUST be defined to support DC-B3 (local
reconnection dispatch) and DC-B5 (2-phase AgentId allocation):

```rust
/// DC-B3: internal port reconnection dispatched from coordinator to worker
/// after a CON-DUP border-redex resolution produces new agents whose
/// auxiliary ports are not themselves borders.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalReconnection {
    /// Agent on the worker side whose port needs rewiring.
    pub agent_id: AgentId,
    /// Port of that agent (0 = principal, 1..arity = aux).
    pub port: u8,
    /// New endpoint this port connects to.
    pub new_target: PortRef,
}

/// DC-B5: AgentId allocation request sent from coordinator to worker via RoundStart.
/// Workers allocate one or more fresh AgentIds per request (one per sibling slot
/// inside the batch), apply `local_wiring` edges, and echo the slot-0 minted id
/// back in `RoundResult.minted_agents`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct PendingCommutation {
    /// Correlation ID, unique within this round -- coordinator picks.
    pub request_id: u32,
    /// Per-slot symbols for the siblings the worker MUST mint for this request
    /// (NF-001 Shape A). Slot `k` is minted as a `target_symbols[k]` agent; the
    /// vector mirrors the resolver's `CommutationBatch.target_symbols` 1-to-1.
    /// Replaces the pre-NF-001 `(symbol_type, arity)` pair, which lost slot
    /// 1..N symbols on heterogeneous CON-DUP mints (`target_symbols ==
    /// [Dup, Con]`). Length is bounded by 16 via the `encode_request_id`
    /// assertion at `border_resolver.rs:318-322` (slot-marker namespace
    /// reservation); the effective `arity` of the request is
    /// `target_symbols.len()` and is always >= 1 (see R33 NF-004 clause).
    pub target_symbols: Vec<Symbol>,
    /// Intra-worker edges the worker MUST apply to the minted sibling(s) after
    /// allocation and before `reduce_all`. Entries reference siblings by
    /// slot-marker (see R23a) or concrete local AgentIds. `FreePort` targets
    /// are reserved and rejected via R33c (see R48a and SC-008).
    pub local_wiring: Vec<LocalWiringHint>,
}
```

**`target_symbols.len() == 0` is a protocol violation (NF-004).** Every `PendingCommutation` MUST mint at least one sibling; `target_symbols` MUST be non-empty. The worker MUST reject an empty `target_symbols` with `MalformedLocalWiringReason::ZeroArity` (R33c case 7). Rationale: the resolver never emits a zero-sibling batch (every rule in §3.2 mints >= 1 agent); R24.1.6c's `MintedAgent` echo assumes `minted_ids_per_pc[0]` is well-defined; and the symmetry with R48b (which legalizes empty `local_wiring` but NOT empty `target_symbols`) keeps the "0-mint, N-wire" degenerate case out of the wire grammar.

```rust
/// DC-B5: one intra-worker edge the worker MUST apply to a freshly minted
/// sibling agent inside a `PendingCommutation`. Mirrors the resolver's
/// `(u8, u8, PortRef)` emission shape 1-to-1, preserving the slot-marker
/// placeholder encoding used by the resolver when the concrete worker
/// AgentId is not yet known (`border_resolver.rs:820-866`).
///
/// The `(u8, u8, u8, u8)` four-byte shape is EXPLICITLY PROHIBITED: `AgentId`
/// is `u32`, so a one-byte id field would truncate any live agent id >= 256
/// into an adjacent id, producing silent misrouting of the minted agent's
/// port in every production fixture (`id_range.start` is typically in the
/// hundreds already). Keeping `target: PortRef` matches the resolver field
/// at `border_resolver.rs:358` without lossy re-encoding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct LocalWiringHint {
    /// Slot index of the minted sibling whose port is the source of this edge.
    /// Range `[0, pc.target_symbols.len())` of the owning `PendingCommutation`
    /// (R33c case 1).
    pub src_slot: u8,
    /// Port on the minted sibling: 0 = principal, 1..=sibling_arity = aux.
    /// Range validation per R33c case 2.
    pub src_port: u8,
    /// Target endpoint. Three target categories are distinguished at the
    /// receiving worker (R23a):
    /// (a) **sibling-slot placeholder** -- `AgentPort(SLOT_MARKER_BASE + s, p)`
    ///     where `s` identifies another sibling of the same request. Decoded
    ///     per R23a clause 3. ALLOWED.
    /// (b) **concrete local agent** -- `AgentPort(live_id, p)` with
    ///     `live_id < SLOT_MARKER_BASE` identifying a pre-existing agent in
    ///     the worker's partition. Pass-through per R23a clause 4. ALLOWED.
    /// (c) **FreePort** -- `FreePort(bid)`. RESERVED; rejected via R33c case 6.
    ///     All cross-partition edges MUST be expressed as `PendingNewBorder`
    ///     on the containing `BorderResolution`, never as `local_wiring`
    ///     (see R48a and SC-008).
    pub target: PortRef,
}

/// DC-B5: Worker's response to a PendingCommutation request.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]
pub struct MintedAgent {
    /// Matches the PendingCommutation.request_id from the RoundStart.
    pub request_id: u32,
    /// The AgentId the worker has allocated for slot 0 of this request
    /// (mint-time id, not post-reduction id; see R24.1.6c and SC-010).
    pub minted_agent_id: AgentId,
}
```

**R33b. Sanctioned application primitive.** The worker MUST apply every entry of `PendingCommutation.local_wiring` via `Net::connect(source, target)` (`net/core.rs:177-200`). `Net::connect` is already documented as O(1), idempotent under the bidirectional-edge invariant, and automatic redex-queue aware (principal-principal pairs enqueue to `redex_queue`, consistent with R23a clause 5). Stage 1 MUST NOT introduce a new `Net::wire_agents` primitive; any such introduction is an architectural breach of the surgical scope of D-005. Note that `Net::connect`'s `FreePort`-redirect bookkeeping branch (`net/core.rs:186-194`) cannot be triggered by `local_wiring` because FreePort targets are rejected at R33c case 6, not applied.

**R33c. Error path.** The worker MUST reject a `PendingCommutation` whose `local_wiring` is internally inconsistent with a new `ProtocolError::MalformedLocalWiring { request_id: u32, reason: MalformedLocalWiringReason }`. The `reason` enum MUST enumerate at minimum the following seven cases (`symbol_count` below is a shorthand for `pc.target_symbols.len()`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MalformedLocalWiringReason {
    /// Case 1 (MUST reject): `src_slot >= pc.target_symbols.len()` -- names
    /// a sibling slot that the request did not declare.
    SrcSlotOutOfRange { src_slot: u8, symbol_count: u8 },
    /// Case 2 (MUST reject): `src_port` exceeds the port count of the
    /// symbol at `src_slot` (CON=2, DUP=2, ERA=0; principal port = index 0).
    SrcPortOutOfRange { src_slot: u8, src_port: u8 },
    /// Case 3 (MUST reject): sibling-slot placeholder target with
    /// `sibling_slot >= pc.target_symbols.len()` -- mirrors R48a (stray
    /// slot-marker).
    TargetSiblingOutOfRange { sibling_slot: u8, symbol_count: u8 },
    /// Case 4 (SHOULD warn, not reject): concrete-AgentId target where the
    /// id is absent from the worker's partition. Symmetric to the resolver's
    /// lenient dangling-FreePort path at `border_resolver.rs:1061-1077`.
    /// Reported via `tracing::warn!`; the edge is applied anyway and logged.
    DanglingConcreteAgent { agent_id: AgentId, port: u8 },
    /// Case 5 (MUST reject): duplicate `(src_slot, src_port)` keys in the
    /// same request -- two entries trying to rewrite the same minted port.
    /// `Net::connect`'s last-write-wins semantics would silently mask this.
    /// Detection site pinned by R23a clause 6 (HashSet pre-pass).
    DuplicateSourcePort { src_slot: u8, src_port: u8 },
    /// Case 6 (MUST reject): `target = PortRef::FreePort(_)`. RESERVED for
    /// future wire break; today all cross-partition edges MUST surface as
    /// `PendingNewBorder` on the containing `BorderResolution` (see R48a,
    /// SC-008).
    ReservedForFuture { border_id: u32 },
    /// Case 7 (MUST reject): `pc.target_symbols` is empty (NF-004). Every
    /// `PendingCommutation` MUST mint at least one sibling; a zero-mint
    /// request is a protocol violation. Rejected before R24.1.6a allocation
    /// so `minted_ids_per_pc[0]` (consumed by R24.1.6c echo) is always
    /// well-defined for non-rejected requests.
    ZeroArity,
}
```

Case 4 is a WARN (not a reject) for symmetry with the resolver-side leniency. Cases 1, 2, 3, 5, 6, 7 are hard rejects: the worker MUST abort the round, emit `RegisterNack { reason: MalformedLocalWiring(...) }`, and the coordinator MUST treat this as a protocol violation consistent with R48's stray-token rule (NACK the worker).

**(MUST for cases 1, 2, 3, 5, 6, 7 reject; SHOULD for case 4 warn)**

**R34.** All new message variants MUST satisfy the same serialization requirements as existing variants: serde + bincode (SPEC-06 R4, R11), round-trip identity (SPEC-06 R14), and CRC32 integrity (SPEC-06 R6-R10). This requirement extends verbatim to `LocalWiringHint` and the updated `PendingCommutation`. In particular: (a) `LocalWiringHint` MUST derive `#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]` under `--features zero-copy` and pass `bytecheck`/`CheckBytes` validation via `rkyv::access` (SPEC-18 Q1); the same derive MUST be applied to `PendingCommutation` (already present) and the per-slot `Symbol` element type carried inside `target_symbols: Vec<Symbol>` (NF-001 Shape A) -- `Symbol` is a `#[repr(u8)]` enum, so its archived form is 1-byte-aligned and the enclosing `Vec<Symbol>` does not perturb the existing 4-byte alignment imposed by `PortRef` inside `LocalWiringHint`; (b) the alignment audit of `Partition` archived size (SPEC-18 Q5) MUST be re-baselined because `(u8, u8, PortRef)` forces 4-byte alignment via the `u32` payload inside `PortRef`, whereas the rejected `(u8, u8, u8, u8)` shape would have been 1-byte aligned; (c) the `--features zero-copy` test baseline on close of D-005 MUST be at least 1192 (up from 1186), adding at minimum: +1 round-trip test for `LocalWiringHint`, +1 corrupt-bytes rejection test, +1 slot-marker substitution integration test, +3 tests for `MalformedLocalWiringReason` cases 1, 2/3, and 5 (case 6 is covered by R48a's test; case 4 is a warn, not an error path; case 7 ZeroArity SHOULD add +1 rejection test). **(MUST)**

**R35.** The `InitialPartition` and `FinalStateResult` variants carry full `Partition` payloads and MUST benefit from all wire optimizations: `CompactSubnet` encoding (SPEC-04), bincode v2 varint encoding (SPEC-18 R1-R3), and optional LZ4 compression (SPEC-18 R8-R11). The R35 exemption carving out wire optimization from `InitialPartition`/`FinalStateResult` does NOT apply to `RoundStart.pending_commutations.local_wiring`: that field lives on the hot path of every delta round and MUST participate in all SPEC-18 R20-R27 rkyv optimizations in the same way as `border_deltas` and `resolved_borders`. **(MUST)**

**R35a. `CompactSubnet` wire encoding of `Net.free_list` (closes QA-D009-001).**

`CompactSubnet` (`relativist-core/src/partition/compact.rs:38`) is the serde adapter that transports `Partition.subnet` across the wire (R35). When SPEC-22 R1 added `free_list: Vec<AgentId>` to `Net`, the adapter was NOT extended to carry it: `from_net()` drops the field and `into_net()` reconstructs an empty `Vec<AgentId>`. This produces silent divergence on TCP transport — the coordinator's sender-side `Net` carries a populated free-list, and the worker's receiver-side `Net` materializes with an empty free-list — which violates SPEC-22 R10a (per-partition free-list population by `build_subnet`), R10b/R10c (protected-tombstone semantics under delta or streaming), and R12a (free-list reconciliation across `merge`). The defects are not visible on in-process transports (bincode is bypassed), so the bug is mode-asymmetric: in-process integration tests pass, TCP-mode tests silently lose ID-reuse information across the wire.

The following clauses are normative:

- **(a) Wire-form preservation.** `CompactSubnet` MUST encode `free_list: Vec<AgentId>` as a new struct field positioned immediately after `root` (the field at `compact.rs:57` that is the current last member). Both `from_net()` (`compact.rs:64-83`) and `into_net()` (`compact.rs:88-128`) MUST round-trip this field identically. `from_net()` MUST initialize the new field to `net.free_list.clone()`. `into_net()` MUST construct the returned `Net` with `free_list: self.free_list` (replacing the current `free_list: Vec::new()` at `compact.rs:113`). The post-amendment struct definition MUST be:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct CompactSubnet {
    pub agent_arena_len: u32,
    pub live: Vec<(AgentId, Agent, [PortRef; PORTS_PER_SLOT])>,
    pub redex_queue: VecDeque<(AgentId, AgentId)>,
    pub next_id: AgentId,
    pub root: Option<PortRef>,
    /// NEW (R35a): Mirrors `Net.free_list` (SPEC-22 R1). LIFO ordering MUST
    /// be preserved across the round-trip; see clause (b).
    pub free_list: Vec<AgentId>,
}
```

The new field is the LAST member of the struct; positioning is normative (NOT a "MAY shift later" surface). No earlier field's encoded position changes, which keeps the existing alignment audit (SPEC-18 R34(b)) intact. **(MUST)**

- **(b) Round-trip invariant.** `Net -> CompactSubnet -> Net` MUST preserve `free_list` byte-for-byte AND in-order (LIFO sequence preserved per SPEC-22 R5). Multiset equality alone is INSUFFICIENT: the order of pops in subsequent `create_agent` calls is observable through `next_id` increment patterns (SPEC-22 §3.8 A4) and through any `debug_assertions` build's `protected_tombstones` shadow set (SPEC-22 R10c). Implementations MUST NOT permute, deduplicate, sort, or compact the wire form of `free_list` — `Vec<AgentId>` round-trip is element-by-element identity. The header comment at `compact.rs:17-18` MUST be amended to enumerate `free_list` (with an explicit "ordering preserved" annotation) in the round-trip-preserved fields list. The `nets_equivalent` test helper at `compact.rs:152-158` MUST be extended to compare `free_list` via the standard `Vec<AgentId>` `PartialEq` (which IS order-sensitive); without this extension the helper would silently mark divergent free-lists as equivalent. **(MUST)**

- **(c) Empty / non-empty boundary.** `Net::new()` produces `free_list: Vec::new()` (SPEC-22 R8); the wire round-trip MUST preserve this as an empty `Vec<AgentId>` (NOT as missing/None/absent). The minimum acceptance suite MUST include at least: (i) one round-trip test for `free_list == vec![]` (sender state after `Net::new()`), and (ii) one round-trip test for `free_list != vec![]` populated by at least one `remove_agent` call followed by a serde round-trip via the adapter. (i) alone is insufficient — today's bug masks (i) by coincidence (sender empty + receiver default-empty == agreement); (ii) is the test that fails today and MUST be the gating fixture for the R35a fix. **(MUST)**

- **(d) rkyv (`zero-copy` feature) coverage.** `CompactSubnet` already derives rkyv per SPEC-18 R21. The new `free_list: Vec<AgentId>` field is `Vec<u32>` (since `AgentId = u32`); rkyv handles `Vec<u32>` natively with 4-byte alignment, so no perturbation of the existing alignment audit (SPEC-18 Q5 / R34(b)) is introduced. The acceptance suite MUST add at least one `--features zero-copy` round-trip test for `CompactSubnet` covering a populated `free_list`, mirroring the existing `rkyv_round_trip_compact_subnet_minimal` fixture at `compact.rs:301-334`. **(MUST)**

- **(e) PROTOCOL_VERSION bump.** This is a `Net` (and therefore `CompactSubnet`) wire layout change. The `PROTOCOL_VERSION` constant at `protocol/coordinator.rs` MUST increment from `PREVIOUS_LIVE_VERSION` to `PREVIOUS_LIVE_VERSION + 1` upon R35a landing. Defensive `PREVIOUS_LIVE_VERSION + 1` language is mandated (NOT a hardcoded absolute integer) in line with the precedent established by SPEC-06 R3b, SPEC-19 R37 (D-005 bump), and SPEC-22 R9a (D-009 bump). At the time of writing the live constant is `6` (post D-009 + D-010 stacking, verified at `protocol/coordinator.rs:197`); R35a's bump therefore lands at `7`, but implementers MUST read the current value at amendment time rather than hardcoding it. v_pre deserializers MUST reject v_post `CompactSubnet` payloads via the existing `UnsupportedVersion` reject path (SPEC-18 R31, mirroring SPEC-22 R9a's v2 -> v3 reject pattern). The version check MUST fire **symmetrically** (NF-002 pattern, mirroring SPEC-19 R37): a worker connecting with `protocol_version == N` to a coordinator whose `PROTOCOL_VERSION == M != N` MUST be rejected during `Register` regardless of which side is older; both sides validate against their own `PROTOCOL_VERSION` constant. No mid-session bincode-decode failure on the new trailing `free_list` field can occur in production because `Register` is the first message of every session — implementers MUST NOT add defensive decode-retry paths for that case. The bump is justified for the same reason SPEC-19 R37 justified its bump and SPEC-22 R9a justified its bump: bincode does not tolerate trailing fields on the decode side — adding `free_list` to the `CompactSubnet` struct definition causes a length-mismatch deserialization error on pre-bump receivers, which is exactly what the version negotiation prevents. Production cost is zero (v1 frozen on `v1-feature-complete`; no over-the-wire pre-R35a coordinator exists). **(MUST)**

- **(f) Reconciliation interaction.** R35a MUST be implemented atomically with the R12a free-list-merge reconciliation path (SPEC-22 §3.8 A8 / R12). The wire round-trip is what makes R12 soundness hold over the network: without R35a the coordinator merges partitions whose free-lists were silently emptied on the wire, producing a merged `Net` whose `next_id` is correct but whose ID-reuse opportunities are permanently lost. The memory-waste compounds across rounds and is observable as monotonic arena growth at the merged-net layer despite the per-partition `remove_agent` calls. Implementations MUST NOT ship R35a clauses (a)-(e) without simultaneously verifying R12 reconciliation, and MUST NOT ship R12 reconciliation without R35a. **R35a is orthogonal to** the SPEC-22 R10b strategy choice (DisableUnderDelta vs BorderClean) and to the `streaming-no-recycle` cargo gate (SPEC-22 §3.8 A6 alternative closure). Even when worker-side recycling is suppressed during a delta or streaming round (Strategy A or the cargo gate), the wire MUST still carry `free_list` because (i) `InitialPartition` (round 0) carries the `build_subnet`-populated per-partition free-list per SPEC-22 R10a, BEFORE any delta or streaming flag governs recycling, and (ii) `FinalStateResult` collects the final partition state at convergence, after which `merge` must reconcile per R12. Implementations MUST NOT optimize away the wire field for runs where the live free-list is observed empty at send time — the field is unconditional. **(MUST)**

- **(g) Acceptance gate.** R35a is acceptance-gated on the Phase E-4 smoke test in `codigo/relativist/docs/plans/2026-04-30-d-011-tier3-hardening-plus-bench-enablement.md` — `docker compose run bench-tcp --benchmark ep_annihilation --sizes 1000 --workers 2 --chunk-size 100 --mode tcp` — producing a CSV with G1 isomorphism passing AND tracing evidence that at least one worker's post-deserialization `free_list` is non-empty for at least one partition delivered via `InitialPartition` or `FinalStateResult`. The latter half is the differentiator: a passing G1 alone does NOT prove R35a is honored (G1 holds even when free-lists are silently dropped, because tombstone semantics are independent of correctness). Tracing instrumentation under `tracing::debug!` at the worker's `into_net()` call site is the cheapest way to surface the differentiator. **(SHOULD for tracing instrumentation; MUST for the smoke test itself)**

QA-D009-001 origin: `codigo/relativist/docs/qa/QA-PHASE-D009-spec22-arena-2026-04-27.md`. Pre-tracking task: `codigo/relativist/docs/backlog/TASK-0595-compactsubnet-free-list-followup.md`. **(MUST for clauses (a)-(f); SHOULD for clause (g) tracing instrumentation, MUST for the smoke test gate.)**

**R36.** The `RoundStart` and `RoundResult` variants carry only border deltas and stats. These payloads are expected to be small (single-digit percent of partition size for most workloads). LZ4 compression SHOULD still be applied when the payload exceeds the compression threshold (SPEC-18 R9), but for typical delta payloads below the threshold, compression SHOULD be skipped. **(SHOULD)**

**R37.** The discriminant assignments in R31 and R32 MUST be coordinated with SPEC-18. If SPEC-18 assigns different discriminants to these variants, SPEC-19 defers to the coordinated numbering. The requirement is that all delta protocol variants are appended at the end of the enum (SPEC-06 R5) and assigned stable discriminants. Additionally, because the D-005 amendment grows `PendingCommutation` with a new `Vec<LocalWiringHint>` field (bincode does NOT tolerate missing trailing fields on the decode side -- it returns a deserialization error), the wire protocol version MUST be bumped: `PROTOCOL_VERSION` at `protocol/coordinator.rs:570` MUST increment from `2` to `3`. The existing `HandshakeAck` rejection path (`protocol/coordinator.rs:66-81`) already handles version mismatch by closing the connection with a `RegisterNack`; no additional migration code is required. Production cost is zero: v1 is frozen, v2 is dev-branch only, no over-the-wire v2 coordinator currently exists. The version check fires symmetrically (NF-002): a worker connecting with `protocol_version == N` to a coordinator whose `PROTOCOL_VERSION == M != N` is rejected during `Register` regardless of which side is older. Both sides validate against their own `PROTOCOL_VERSION` constant. No v2 coordinator can reach `PendingCommutation` decode with a v3 worker (or vice versa) because `Register` is the first message of every session; mid-session bincode-decode failures on the new trailing `Vec<LocalWiringHint>` field therefore cannot occur in production, and implementers MUST NOT add defensive decode-retry paths for that case. **(MUST)**

**R48. Agent ID allocation coordination invariant (DC-B5).**

The coordinator MUST allocate `request_id` values from a monotonically increasing counter scoped to the BSP run (not per-round). Each worker MUST allocate AgentId values from its own reserved AgentId range (SPEC-05 partitions the AgentId space by partition index; workers MUST NOT overlap the coordinator-reserved range `u32::MAX - 10_000 .. u32::MAX`, which is reserved for future use). The coordinator MUST treat a `MintedAgent` response whose `request_id` does not match any outstanding `PendingCommutation` as a protocol violation (nack the worker). **(MUST)**

*Note on numbering: this requirement is labelled R48 because R44-R47 were already assigned to §3.6 and §3.7 at the time of the initial SPEC-19 draft. The amendment owner (DC-B5) originally named this "R44"; the binding identifier is R48. All cross-references in task bundles, reviews, and amendment memos referring to "R44 (DC-B5)" MUST be read as R48.*

**R48a. Stray slot-marker guard in `local_wiring` (DC-B5, SC-007).**

The worker MUST reject, with `ProtocolError::MalformedLocalWiring { reason: MalformedLocalWiringReason::TargetSiblingOutOfRange { sibling_slot, symbol_count } }` (R33c case 3), any `PendingCommutation` whose `local_wiring` contains a sibling-slot placeholder target `PortRef::AgentPort(x, _)` with `x >= SLOT_MARKER_BASE` and `(x - SLOT_MARKER_BASE) >= pc.target_symbols.len()`. This is the wire-layer symmetric of R48's coordinator-side stray-token guard on `MintedAgent.request_id`: the resolver's `encode_request_id` assertion (`border_resolver.rs:318-322`) protects the resolver-output boundary, and R48a protects the worker-input boundary. The resolver's slot-marker namespace is capped at 16 per batch via DC-B5's `CommutationBatch.target_symbols` length bound; any `sibling_slot` at or above `pc.target_symbols.len()` is a resolver bug or a corrupted wire, never a legal emission. **(MUST)**

**R48b. Empty `local_wiring` is legal (DC-B5, SC-011).**

A `PendingCommutation` MAY carry an empty `local_wiring` vector. An empty `local_wiring` means the minted agent(s) of this request have no intra-worker edges to apply (for example, a CON-ERA/DUP-ERA resolution where both aux targets of the consumed agent were `FreePort` values that the resolver routed to `PendingNewBorder` instead, per SC-008). The worker MUST NOT treat empty `local_wiring` as an error. This parallels R26's pairing rule where `minted_agents` MUST echo every `PendingCommutation` including those with empty wiring. **(MUST)**

### 3.5 Invariant Amendments

This section documents how the delta protocol affects the formal invariants of SPEC-01. Invariants not mentioned here are unaffected (see BRIEF-20260415-v2-codebase-assessment, Section 1: T1-T7, D1-D2, D4-D5, I1-I7 are structurally unchanged).

**R38. Amendment to G1 (Fundamental Property).**

The v1 formulation of G1 is:
```
reduce_all(net) ~ extract_result(run_grid(net, n))
```

Under the delta protocol, the coordinator does not hold the full merged net between rounds. The intermediate state is distributed. G1 MUST be reformulated as:

```
reduce_all(net) ~ extract_result(run_grid_delta(net, n))
```

where `run_grid_delta` denotes the delta BSP loop, and `extract_result` performs the Final State Collection (R27-R29) followed by `merge()`.

The correctness argument MUST establish that at every point during the delta BSP loop, the distributed state is **recoverable**: the sequential intermediate state `mu_k` (the net after k total interactions) is isomorphic to `merge(reconstruct(border_graph, worker_partitions))`, where `reconstruct` builds a `PartitionPlan` from the coordinator's `BorderGraph` and the workers' current partition states. Strong confluence (T4) guarantees that this decomposition exists and is unique up to isomorphism.

The full formal proof of the recoverability property is pending (see Section 8, Open Questions: DISC-011 + ARG-005). This spec defines the design; the theoretical proof is a separate deliverable.

**(MUST for the reformulation; proof pending)**

**R39. Amendment to D3 (Border Completeness).**

The v1 formulation of D3 requires exhaustive border scan during merge to detect all border redexes. Under the delta protocol, border redex detection is performed incrementally via the `BorderGraph`:

- D3a (amended): Border redex detection MUST be performed by the `BorderGraph.detect_border_redexes()` method, which inspects the `is_redex` flag of each border entry. The `is_redex` flag MUST be recomputed each time a border delta is applied (R11).
- D3b (amended): Each detected border redex MUST be resolved by the coordinator (R13-R15) before workers proceed to the next round, OR deferred to a subsequent round if the coordinator determines that resolution can be batched. The protocol MUST iterate until no border redexes remain (Global Normal Form, R4).
- D3c (unchanged): Emergent border redexes created by CON-DUP commutation during local reduction MUST be captured by the worker's border delta report. If a newly created agent's principal port becomes connected to a `FreePort(border_id)`, the worker's delta MUST report this change, enabling the coordinator to detect the emergent border redex via the `BorderGraph`.
- D3d (amended): The delta communication MUST be equivalent to exhaustive detection. Formally: for every border redex that would be detected by a full `merge() + findBorderRedexes()`, the `BorderGraph` MUST detect the same redex within the same round (given correct delta reporting by workers). This equivalence is the core correctness claim of the delta protocol and is pending formal proof (Section 8).

**(MUST)**

**R40. Amendment to D6 (Protocol Termination).**

The v1 formulation of D6 bounds the round count by R_lenient <= 1 and R_strict <= N (total interactions). Under the delta protocol, D6 MUST be reformulated:

- **Delta mode, lenient behavior:** The coordinator resolves border redexes immediately upon detection (R13). If all border redexes in round k are resolved at the coordinator, and workers report zero new border activity in round k+1, the protocol converges. The round count is bounded by the depth of the cross-partition cascade chain plus 1. In the absence of cross-partition cascades, R_delta_lenient = 1.
- **Delta mode, strict behavior:** Border redexes are dispatched to workers for resolution in subsequent rounds (analogous to SPEC-05 R30a strict mode). R_delta_strict <= N (same bound as v1 strict mode, since each round consumes at least one interaction from the finite budget T7).
- **Termination condition (Global Normal Form):** The protocol terminates when all workers report zero local redexes AND the `BorderGraph` contains zero active pairs (R4). This is equivalent to the v1 condition (merged net has empty redex queue) but does not require reconstructing the full merged net.
- **Progress guarantee:** Each delta round MUST consume at least one interaction from the global interaction budget (T7). If workers have local redexes, they reduce them. If the coordinator detects border redexes, it resolves them (or dispatches them). Since the total interaction count is finite and invariant (T7), the protocol terminates.

**(MUST)**

### 3.6 Configuration

**R41.** The `GridConfig` structure (SPEC-05, Section 4.1) MUST be extended with:
```rust
pub struct GridConfig {
    // ... existing fields (num_workers, max_rounds, strict_bsp) ...

    /// Enable delta protocol (stateful workers).
    ///
    /// - `false` (default): v1 full-partition protocol (SPEC-05 R24-R30a).
    /// - `true`: delta BSP loop with stateful workers (SPEC-19).
    pub delta_mode: bool,

    /// Enable coordinator-free round optimization.
    ///
    /// When `true`, the coordinator skips merge-redistribute when all
    /// workers report zero border activity. Effective in both delta
    /// and full-partition modes.
    ///
    /// Default: `true` (enabled when delta_mode is true).
    pub coordinator_free_rounds: bool,
}
```
**(MUST)**

**R42.** `delta_mode` MUST default to `false` to preserve backwards compatibility. No existing caller, test, or benchmark MUST change behavior when `delta_mode` is not explicitly set. **(MUST)**

**R43.** `coordinator_free_rounds` MUST default to `true` when `delta_mode` is `true`, and SHOULD default to `false` when `delta_mode` is `false` (in v1 mode, coordinator-free rounds require stateful workers, which v1 does not have). **(MUST for delta default; SHOULD for v1 default)**

**R44.** The coordinator-free round optimization (Section 3.1) MAY be implemented independently of the full delta protocol. If `coordinator_free_rounds` is `true` and `delta_mode` is `false`, the coordinator MUST still track worker border activity but operate the v1 protocol otherwise (workers retain partitions from the previous `AssignPartition`). **(MAY)**

### 3.7 Metrics

**R45.** The `GridMetrics` structure (SPEC-05, Section 4.1) MUST be extended with delta-specific counters:
```rust
pub struct GridMetrics {
    // ... existing fields ...

    /// Number of coordinator-free rounds (rounds where merge was skipped).
    pub coordinator_free_rounds: u32,

    /// Border deltas received from workers, per round.
    pub border_deltas_received_per_round: Vec<u32>,

    /// Border deltas sent to workers (from coordinator resolution), per round.
    pub border_deltas_sent_per_round: Vec<u32>,

    /// Border redexes resolved at the coordinator, per round.
    pub coordinator_border_resolutions_per_round: Vec<u32>,

    /// Bytes sent per round (delta mode: RoundStart payloads only,
    /// excluding InitialPartition and FinalStateResult).
    pub delta_bytes_sent_per_round: Vec<usize>,

    /// Bytes received per round (delta mode: RoundResult payloads only).
    pub delta_bytes_received_per_round: Vec<usize>,

    /// Time for border redex resolution at coordinator, per round.
    pub coordinator_resolve_time_per_round: Vec<Duration>,

    /// Time for BorderGraph.apply_deltas, per round.
    pub border_graph_update_time_per_round: Vec<Duration>,
}
```
**(MUST)**

**R46.** The delta-specific metrics MUST enable the following analyses:
1. **Wire cost reduction:** Compare `delta_bytes_sent_per_round` + `delta_bytes_received_per_round` (delta mode) against `bytes_sent_per_round` + `bytes_received_per_round` (v1 mode) to measure the wire cost savings.
2. **Coordinator-free round effectiveness:** `coordinator_free_rounds / rounds` gives the fraction of rounds where merge was avoided.
3. **Border redex distribution:** `coordinator_border_resolutions_per_round` vs `border_deltas_received_per_round` shows how much border activity is generated per round.
4. **Overhead decomposition:** `coordinator_resolve_time_per_round` + `border_graph_update_time_per_round` gives the coordinator's per-round overhead under delta mode, analogous to `merge_time_per_round` + `border_reduce_time_per_round` in v1 mode.
**(MUST)**

**R47.** The metrics MUST be sufficient to calculate the break-even point under delta mode, extending the analysis in ROADMAP 2.40. Specifically, the per-round overhead `c_o` under delta mode MUST be measurable as:
```
c_o_delta = (delta_send_time + delta_recv_time + border_graph_update_time + coordinator_resolve_time) / round
```
compared to v1's:
```
c_o_v1 = (partition_time + network_send_time + network_recv_time + merge_time + border_reduce_time) / round
```
**(MUST)**

---

## 4. Design

### 4.1 Types

```rust
use std::collections::HashMap;
use std::time::Duration;

/// A change to a border port's connectivity.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BorderDelta {
    /// The border ID affected.
    pub border_id: u32,
    /// The new port target on the reporting worker's side.
    /// `PortRef::FreePort(u32::MAX)` (DISCONNECTED) indicates the
    /// agent connected to this border has been erased.
    pub new_target: PortRef,
}

/// State of a single border wire, tracked by the coordinator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderState {
    pub border_id: u32,
    /// Current endpoint on the side_a worker.
    pub side_a: PortRef,
    /// Current endpoint on the side_b worker.
    pub side_b: PortRef,
    /// Worker owning side_a.
    pub worker_a: WorkerId,
    /// Worker owning side_b.
    pub worker_b: WorkerId,
    /// True iff both side_a and side_b are AgentPort(_, 0).
    pub is_redex: bool,
}

/// Coordinator-side data structure tracking all border connectivity.
///
/// Replaces the full merged net for rounds 1+. The coordinator
/// uses this to detect border redexes without reconstructing the
/// full net from all worker partitions.
#[derive(Debug, Clone)]
pub struct BorderGraph {
    /// All border states, indexed by border_id.
    borders: HashMap<u32, BorderState>,
    /// Per-worker list of border_ids that the worker participates in.
    worker_borders: Vec<Vec<u32>>,
    /// Set of border_ids where is_redex == true (incremental tracking).
    active_redexes: HashSet<u32>,
}
```

### 4.2 BorderGraph Operations

```
impl BorderGraph {
    /// Initialize from a PartitionPlan after split().
    fn from_partition_plan(plan: &PartitionPlan) -> Self {
        let mut borders = HashMap::new();
        let mut worker_borders = vec![Vec::new(); plan.partitions.len()];
        let mut active_redexes = HashSet::new();

        for (border_id, (orig_a, orig_b)) in &plan.borders {
            // Find which partitions own each side
            let (worker_a, side_a) = find_border_owner(plan, border_id, 0);
            let (worker_b, side_b) = find_border_owner(plan, border_id, 1);

            let is_redex = is_principal_pair(side_a, side_b);
            if is_redex {
                active_redexes.insert(*border_id);
            }

            borders.insert(*border_id, BorderState {
                border_id: *border_id,
                side_a, side_b,
                worker_a, worker_b,
                is_redex,
            });

            worker_borders[worker_a as usize].push(*border_id);
            worker_borders[worker_b as usize].push(*border_id);
        }

        BorderGraph { borders, worker_borders, active_redexes }
    }

    /// Apply deltas reported by a worker. O(|deltas|).
    fn apply_deltas(&mut self, worker_id: WorkerId, deltas: &[BorderDelta]) {
        for delta in deltas {
            if let Some(state) = self.borders.get_mut(&delta.border_id) {
                // Update the side owned by this worker
                if state.worker_a == worker_id {
                    state.side_a = delta.new_target;
                } else if state.worker_b == worker_id {
                    state.side_b = delta.new_target;
                }

                // Recompute is_redex
                let was_redex = state.is_redex;
                state.is_redex = is_principal_pair(state.side_a, state.side_b);

                // Update incremental redex set
                if state.is_redex && !was_redex {
                    self.active_redexes.insert(delta.border_id);
                } else if !state.is_redex && was_redex {
                    self.active_redexes.remove(&delta.border_id);
                }

                // Handle double-erasure: both sides disconnected
                if state.side_a == DISCONNECTED && state.side_b == DISCONNECTED {
                    self.active_redexes.remove(&delta.border_id);
                    self.borders.remove(&delta.border_id);
                    // Note: worker_borders entries become stale but harmless
                }
            }
        }
    }

    /// Return all border redexes. O(|active_redexes|).
    fn detect_border_redexes(&self) -> Vec<(u32, &BorderState)> {
        self.active_redexes.iter()
            .filter_map(|bid| self.borders.get(bid).map(|s| (*bid, s)))
            .collect()
    }

    /// Number of remaining borders.
    fn len(&self) -> usize {
        self.borders.len()
    }

    /// True if no border redexes exist.
    fn has_no_redexes(&self) -> bool {
        self.active_redexes.is_empty()
    }
}
```

### 4.3 Delta BSP Loop

The delta BSP loop replaces `run_grid` (SPEC-05 R24-R30a) when `delta_mode` is enabled.

```
fn run_grid_delta(
    net: Net,
    config: &GridConfig,
    strategy: &dyn PartitionStrategy,
) -> (Net, GridMetrics) {

    // ── Phase 0: Initial split and dispatch ──────────────────────
    let plan = split(net, config.num_workers, strategy);
    let mut border_graph = BorderGraph::from_partition_plan(&plan);
    let mut metrics = GridMetrics::default();

    // Send InitialPartition to each worker (one-time full partition)
    for partition in &plan.partitions {
        send_to_worker(partition.worker_id, Message::InitialPartition {
            round: 0,
            partition: partition.clone(),
        });
    }

    // ── Phase 1: Delta BSP loop ─────────────────────────────────
    let mut round = 0;
    loop {
        round += 1;

        // 1a. Resolve any border redexes detected by BorderGraph
        let border_redexes = border_graph.detect_border_redexes();
        let mut coordinator_deltas: HashMap<WorkerId, Vec<BorderDelta>> = HashMap::new();

        for (bid, state) in &border_redexes {
            // Request agents from workers, resolve at coordinator
            let resolution = resolve_border_redex_at_coordinator(
                bid, state, &mut border_graph
            );
            // Distribute resulting deltas to affected workers
            for (worker_id, deltas) in resolution.worker_deltas {
                coordinator_deltas.entry(worker_id)
                    .or_default()
                    .extend(deltas);
            }
        }

        // 1b. Send RoundStart to each worker
        for worker_id in 0..config.num_workers {
            let deltas = coordinator_deltas
                .remove(&worker_id)
                .unwrap_or_default();
            send_to_worker(worker_id, Message::RoundStart {
                round,
                border_deltas: deltas,
                resolved_borders: vec![/* from resolution */],
                new_borders: vec![/* from CON-DUP expansion */],
            });
        }

        // 1c. Collect RoundResult from all workers
        let results = collect_all_round_results(config.num_workers);

        // 1d. Apply worker deltas to BorderGraph
        let mut all_stable = true;
        let mut all_zero_redexes = true;
        for result in &results {
            border_graph.apply_deltas(result.worker_id, &result.border_deltas);
            if result.has_border_activity {
                all_stable = false;
            }
            if result.stats.local_redexes > 0 {
                all_zero_redexes = false;
            }
        }

        // 1e. Update metrics
        metrics.rounds = round;
        // ... record per-round metrics ...

        // 1f. Check convergence: Global Normal Form
        if all_stable && all_zero_redexes && border_graph.has_no_redexes() {
            break; // converged
        }

        // 1g. Check coordinator-free round
        if all_stable && border_graph.has_no_redexes()
           && config.coordinator_free_rounds {
            metrics.coordinator_free_rounds += 1;
            // Skip merge, workers continue reducing locally
            continue;
        }

        // 1h. Check max_rounds
        if let Some(max) = config.max_rounds {
            if round >= max {
                metrics.converged = false;
                break;
            }
        }
    }

    // ── Phase 2: Final State Collection ─────────────────────────
    for worker_id in 0..config.num_workers {
        send_to_worker(worker_id, Message::FinalStateRequest { round });
    }
    let final_partitions = collect_all_final_states(config.num_workers);

    // Reconstruct PartitionPlan from collected partitions + border_graph
    let final_plan = reconstruct_partition_plan(final_partitions, &border_graph);

    // Final merge (SPEC-05 R1-R11)
    let (mut result_net, border_count) = merge(final_plan);

    // Resolve any remaining border redexes (lenient post-merge)
    if !config.strict_bsp {
        reduce_all(&mut result_net);
    }

    metrics.converged = true;
    (result_net, metrics)
}
```

### 4.4 Worker-Side Delta State Machine

```
enum WorkerDeltaState {
    /// Waiting for InitialPartition.
    WaitingInit,
    /// Holding a partition, waiting for RoundStart or FinalStateRequest.
    Idle { partition: Partition },
    /// Reducing locally.
    Reducing { partition: Partition },
    /// Sending RoundResult.
    Reporting { partition: Partition },
    /// Sending FinalStateResult.
    ReturningFinal { partition: Partition },
    /// Terminal.
    Done,
    /// Error.
    Error(String),
}
```

**Worker delta loop:**

```
fn worker_delta_loop(stream: TransportStream) {
    let mut state = WorkerDeltaState::WaitingInit;
    let mut previous_border_state: HashMap<u32, PortRef> = HashMap::new();

    loop {
        match recv_message(&stream) {
            Message::InitialPartition { round, partition } => {
                // Initialize border state tracking
                previous_border_state = partition.free_port_index.clone();

                // Reduce locally
                let stats = reduce_all(&mut partition.subnet);
                rebuild_free_port_index(&mut partition);

                // Compute initial border deltas
                let deltas = compute_border_deltas(
                    &partition.free_port_index,
                    &previous_border_state,
                );
                previous_border_state = partition.free_port_index.clone();

                let has_border_activity = check_border_activity(&partition);

                send_message(&stream, Message::RoundResult {
                    round,
                    border_deltas: deltas,
                    stats: build_worker_stats(stats, &partition),
                    has_border_activity,
                });

                state = WorkerDeltaState::Idle { partition };
            }

            Message::RoundStart { round, border_deltas, resolved_borders, new_borders } => {
                let partition = extract_partition(&mut state);

                // Apply coordinator deltas to local partition
                apply_border_deltas(&mut partition, &border_deltas);
                remove_resolved_borders(&mut partition, &resolved_borders);
                add_new_borders(&mut partition, &new_borders);

                // Reduce locally
                let stats = reduce_all(&mut partition.subnet);
                rebuild_free_port_index(&mut partition);

                // Compute border deltas vs previous state
                let deltas = compute_border_deltas(
                    &partition.free_port_index,
                    &previous_border_state,
                );
                previous_border_state = partition.free_port_index.clone();

                let has_border_activity = check_border_activity(&partition);

                send_message(&stream, Message::RoundResult {
                    round,
                    border_deltas: deltas,
                    stats: build_worker_stats(stats, &partition),
                    has_border_activity,
                });

                state = WorkerDeltaState::Idle { partition };
            }

            Message::FinalStateRequest { round } => {
                let partition = extract_partition(&mut state);
                send_message(&stream, Message::FinalStateResult {
                    round,
                    partition,
                });
                state = WorkerDeltaState::Done;
            }

            Message::Shutdown => {
                state = WorkerDeltaState::Done;
                break;
            }

            _ => {
                // Unexpected message
                state = WorkerDeltaState::Error("unexpected message".into());
                break;
            }
        }
    }
}
```

### 4.5 Border Delta Computation

```rust
/// Compute border deltas by comparing current free_port_index
/// against the previous snapshot.
fn compute_border_deltas(
    current: &HashMap<u32, PortRef>,
    previous: &HashMap<u32, PortRef>,
) -> Vec<BorderDelta> {
    let mut deltas = Vec::new();

    // Ports that changed or appeared
    for (bid, current_target) in current {
        match previous.get(bid) {
            Some(prev_target) if prev_target == current_target => {
                // No change, skip
            }
            _ => {
                deltas.push(BorderDelta {
                    border_id: *bid,
                    new_target: *current_target,
                });
            }
        }
    }

    // Ports that disappeared (agent erased)
    for (bid, _) in previous {
        if !current.contains_key(bid) {
            deltas.push(BorderDelta {
                border_id: *bid,
                new_target: DISCONNECTED,
            });
        }
    }

    deltas
}
```

### 4.6 Convergence Detection

The convergence detection logic uses a two-level check:

**Level 1 (per-round):** After collecting all `RoundResult` messages:
```
coordinator_free = all workers report has_border_activity == false
workers_done = all workers report local_redexes == 0
graph_clear = border_graph.has_no_redexes()

if coordinator_free && workers_done && graph_clear:
    → Global Normal Form reached. Initiate Final State Collection.
elif coordinator_free && graph_clear:
    → Coordinator-free round. Workers continue reducing locally.
else:
    → Border redexes exist or workers have pending work.
       Resolve border redexes and continue.
```

**Level 2 (between rounds):** After applying worker deltas to the `BorderGraph`:
```
new_border_redexes = border_graph.detect_border_redexes()
if new_border_redexes.is_empty():
    → No border work needed this round.
else:
    → Resolve at coordinator (R13-R15) and dispatch deltas.
```

### 4.7 Message Flow Diagrams

**Normal delta round (border redex exists):**

```
Coordinator                Worker_0               Worker_1
    │                         │                       │
    ├─ RoundStart(deltas) ──>│                       │
    ├─ RoundStart(deltas) ──────────────────────────>│
    │                         │                       │
    │                 reduce_all()             reduce_all()
    │                         │                       │
    │<── RoundResult(deltas) ─┤                       │
    │<── RoundResult(deltas) ─────────────────────────┤
    │                         │                       │
    ├─ apply_deltas()         │                       │
    ├─ detect_border_redexes()│                       │
    ├─ resolve_at_coordinator()                       │
    │                         │                       │
```

**Coordinator-free round (no border redexes):**

```
Coordinator                Worker_0               Worker_1
    │                         │                       │
    ├─ RoundStart(empty) ───>│                       │
    ├─ RoundStart(empty) ──────────────────────────>│
    │                         │                       │
    │                 reduce_all()             reduce_all()
    │                         │                       │
    │<── RoundResult(empty) ──┤                       │
    │<── RoundResult(empty) ──────────────────────────┤
    │                         │                       │
    │  all has_border_activity == false                │
    │  → skip merge, continue                         │
    │                         │                       │
```

**Convergence and final collection:**

```
Coordinator                Worker_0               Worker_1
    │                         │                       │
    │  Global Normal Form detected                    │
    │  (all zero redexes + graph clear)               │
    │                         │                       │
    ├─ FinalStateRequest ───>│                       │
    ├─ FinalStateRequest ──────────────────────────>│
    │                         │                       │
    │<── FinalStateResult ────┤                       │
    │<── FinalStateResult ────────────────────────────┤
    │                         │                       │
    ├─ merge(final_partitions)│                       │
    ├─ reduce_all() (lenient) │                       │
    │                         │                       │
    ├─ Shutdown ────────────>│                       │
    ├─ Shutdown ────────────────────────────────────>│
    │                         │                       │
```

---

## 5. Rationale

### 5.1 Why delta protocol instead of just optimizing transport?

The v1 architecture ships entire partitions every round. Transport optimizations (ROADMAP 2.22 TCP tuning, 2.23 bincode v2, 2.24 zero-copy, 2.25 UDS) reduce the per-byte cost but do not change the O(N) per-round wire cost scaling. For `ep_annihilation_con` 20M agents at w=2, each partition is ~750 MB before optimization; even a 5x compression only reduces this to ~150 MB per round per worker.

The delta protocol changes the scaling: per-round wire cost is O(border_changes), not O(N). For workloads like `ep_annihilation_con` where agents annihilate internally and border activity is minimal, the per-round delta payload can be as small as a few KB. This is the difference between a constant factor improvement and an asymptotic improvement.

The break-even analysis (ROADMAP 2.40) shows c_o/c_r = 2.2 in v1. Transport optimizations alone cannot achieve the required c_o/c_r < 0.50 for w=2 break-even. The delta protocol is the only architectural change that can close this gap by reducing the per-round overhead from partition-proportional to change-proportional.

### 5.2 Why coordinator-side border resolution (option a)?

ROADMAP 2.35 presents two options for border redex resolution: (a) coordinator requests agents from workers and resolves locally, or (b) workers resolve peer-to-peer. Option (a) is chosen because:

1. **Star topology compatibility:** v1's architecture is a star topology (SPEC-13). Option (a) requires no worker-to-worker communication, which would need new connection management, discovery, and security infrastructure.
2. **Simplicity:** The coordinator already has access to the reduction engine (SPEC-13 R6-R8). Resolving a single border redex requires only the two interacting agents (locality, T2), which are small (< 100 bytes total).
3. **Correctness argument:** The coordinator resolves the border redex as if it were performing the merge step of v1, but only for the two agents involved. Strong confluence guarantees that this is equivalent to any other reduction order.
4. **Performance:** Border redexes involve transferring at most 2 agents (each ~32 bytes) and receiving back the resolution deltas. This is negligible compared to the partition-size transfers of v1.

Option (b) remains viable for a future peer-to-peer extension (ROADMAP 2.4 hierarchical coordination) but is out of scope for SPEC-19.

### 5.3 Why three layered features?

The three features (coordinator-free round, BorderGraph, delta protocol) are designed as layers that can be implemented and shipped incrementally:

1. **Coordinator-free round (R1-R7):** ~300 lines, no new data structures, no protocol changes beyond adding `has_border_activity` to `WorkerRoundStats`. Can be shipped as a standalone optimization for strict BSP mode.
2. **BorderGraph (R8-R19):** ~400 lines, pure data structure in the core layer. Can be developed and tested independently of the wire protocol. Provides the foundation for both delta merge and coordinator-free round enhancement.
3. **Delta protocol (R20-R37):** ~800 lines, requires BorderGraph and new message variants. The full stateful worker protocol. Ships last.

This layering allows each feature to be A/B benchmarked against the previous baseline, producing clear evidence of each optimization's contribution.

### 5.4 Why not streaming reduction instead?

Streaming reduction (ROADMAP 2.16) exchanges partial results incrementally without BSP barriers. While streaming is more powerful in theory (REF-015, Mackie and Sato), it requires a fundamentally different protocol architecture (event-driven coordinator, interaction budgets instead of rounds, incremental merge) and has deeper theoretical gaps (Gap 2 in BRIEF-20260415-v2-fundamentacao-teorica). The delta protocol preserves the BSP model (which has a well-understood correctness argument) while eliminating the wire cost that makes BSP impractical for large nets.

---

## 6. Migration Path

### 6.1 v1 Stateless to v2 Stateful

The migration preserves full backwards compatibility:

1. **Phase A (coordinator-free round only):** Add `has_border_activity` to `WorkerRoundStats` (R2). Add `coordinator_free_rounds` to `GridConfig` (R41). The v1 protocol is unchanged; the coordinator merely tracks whether merge can be skipped. Zero regression risk.

2. **Phase B (BorderGraph):** Implement `BorderGraph` in `src/merge/border_graph.rs` (R8-R19). The `BorderGraph` is initialized from `PartitionPlan` after each `split()` and can be used for diagnostic assertions in v1 mode (verify that `BorderGraph.detect_border_redexes()` matches the actual border redexes found by `merge()`). No protocol changes.

3. **Phase C (delta protocol):** Add new `Message` variants (R31-R37). Implement `run_grid_delta` as a parallel code path alongside `run_grid` in `src/merge/grid.rs` (or a new `src/merge/grid_delta.rs`). The choice is governed by `GridConfig.delta_mode`. The v1 `run_grid` remains unchanged and continues to be the default.

4. **Phase D (v1 deprecation):** After delta protocol is validated (all 690+ tests pass in delta mode, G1 equivalence confirmed for all benchmarks), v1 full-partition mode MAY be deprecated but MUST remain available behind the `delta_mode = false` flag for at least one major version.

### 6.2 Feature Flags

The delta protocol SHOULD be feature-gated during development:

```toml
[features]
delta-protocol = []  # Enables delta protocol code paths
```

When the feature is disabled, the `Message` enum retains only v1 variants and `GridConfig.delta_mode` is not available. This prevents delta protocol code from affecting v1 binary size and compile time during the transition period.

### 6.3 Coordinator FSM Updates

The coordinator FSM (SPEC-13 R19, R21) MUST be extended with new states for the delta protocol:

| State | Description | Transition |
|-------|-------------|-----------|
| `InitialDispatching` | Sending `InitialPartition` to all workers (round 0) | All sent -> `WaitingForResults` |
| `DeltaDispatching` | Sending `RoundStart` to all workers (rounds 1+) | All sent -> `WaitingForResults` |
| `ResolvingBorders` | Resolving border redexes at coordinator via `BorderGraph` | Resolution complete -> `DeltaDispatching` or `CollectingFinalState` |
| `CollectingFinalState` | Sending `FinalStateRequest`, awaiting `FinalStateResult` from all workers | All collected -> `FinalMerging` |
| `FinalMerging` | Executing `merge()` on collected partitions | Merge complete -> `Done` |

These states are additive (appended to the existing FSM) and do not affect the v1 state machine when `delta_mode` is `false`.

### 6.4 Worker FSM Updates

The worker FSM (SPEC-13 R24, R25) MUST be extended:

| State | Description | Transition |
|-------|-------------|-----------|
| `WaitingInit` | Waiting for `InitialPartition` | Received -> `Reducing` |
| `Idle` | Waiting for `RoundStart` or `FinalStateRequest` or `Shutdown` | `RoundStart` -> `Reducing`; `FinalStateRequest` -> `ReturningFinal`; `Shutdown` -> `Done` |
| `Reducing` | Running `reduce_all` on stored partition | Complete -> `Reporting` |
| `Reporting` | Sending `RoundResult` | Sent -> `Idle` |
| `ReturningFinal` | Sending `FinalStateResult` | Sent -> `Idle` (awaiting `Shutdown`) |
| `Done` | Terminal | (terminal) |

---

## 7. Test Strategy

### 7.1 Unit Tests (BorderGraph)

**T1.** `BorderGraph::from_partition_plan` correctly initializes all border states from a known `PartitionPlan`. Verify border count, worker assignments, and `is_redex` flags.

**T2.** `BorderGraph::apply_deltas` correctly updates a single border's endpoint and recomputes `is_redex`. Test cases: (a) non-redex -> redex transition, (b) redex -> non-redex transition, (c) no change.

**T3.** `BorderGraph::apply_deltas` with erasure: delta with `DISCONNECTED` target. Verify border is marked appropriately. Double-erasure (both sides disconnected) removes the border entry.

**T4.** `BorderGraph::detect_border_redexes` returns the correct set of active pairs. Test with 0, 1, and multiple border redexes.

**T5.** `compute_border_deltas` correctly identifies changes between current and previous `free_port_index`. Test cases: (a) port reconnected (different target), (b) port unchanged, (c) port erased (present in previous, absent in current), (d) new port (absent in previous, present in current).

### 7.2 Integration Tests (G1 Equivalence)

**T6.** `run_grid_delta` produces the same Normal Form as `reduce_all` for `ep_annihilation_con(100)` at w=2. This is the fundamental G1 test for the delta protocol.

**T7.** `run_grid_delta` produces the same Normal Form as `run_grid` (v1) for `ep_annihilation_con(1000)` at w=2 and w=4. Cross-protocol equivalence.

**T8.** `run_grid_delta` with `cascade_cross(10)` under strict BSP at w=2. Verify `rounds > 1` (multi-round delta protocol) and G1 equivalence. This exercises emergent border redexes across multiple delta rounds.

**T9.** `run_grid_delta` with `dual_tree(20)` at w=4. Verify G1 equivalence for a workload with deep cross-partition cascades.

**T10.** `run_grid_delta` with `church_add(5, 3)` at w=2. Verify decoded result matches sequential reduction. This exercises the full encoding/reduction/decoding workflow under delta mode.

### 7.3 Coordinator-Free Round Tests

**T11.** For `ep_annihilation_con(100)` at w=2 with `coordinator_free_rounds = true`: verify that `metrics.coordinator_free_rounds >= 1` (at least one round was skipped because no borders were active).

**T12.** For a net that is already in Normal Form: `run_grid_delta` terminates in 0 rounds with no wire traffic beyond `InitialPartition` and `FinalStateResult`.

### 7.4 Wire Cost Tests

**T13.** For `ep_annihilation_con(10000)` at w=2, compare the total bytes transmitted under delta mode vs v1 mode. The delta mode MUST transmit at least 10x fewer bytes in rounds 1+ (excluding `InitialPartition` and `FinalStateResult`).

**T14.** For `dual_tree(20)` at w=4 under strict BSP, the per-round delta payload MUST be less than 10% of the full partition size for rounds after the first.

### 7.5 Property-Based Tests

**T15.** For randomly generated terminating nets (using the existing property-based test infrastructure): `run_grid_delta(net, w)` MUST produce the same Normal Form as `reduce_all(net)` for w in {2, 3, 4}.

**T16.** Border delta completeness: after each round, the `BorderGraph` state MUST match what would be obtained by performing a full `merge()` and inspecting the border wires. This is the operational verification of D3 (amended).

### 7.6 Regression Tests

**T17.** All 690 existing v1 tests MUST pass with `delta_mode = false`. Zero regression.

**T18.** All 690 existing v1 tests SHOULD also pass with `delta_mode = true` (where applicable -- tests that directly construct `run_grid` with specific arguments may need adaptation).

---

## 8. Open Questions

**OQ-1. Formal proof of G1 extension (CRITICAL).**

The delta protocol's correctness depends on the claim that the distributed state (coordinator's `BorderGraph` + union of worker partition interiors) is recoverable to a net isomorphic to the sequential intermediate state. This claim is supported by strong confluence (T4) but requires a formal proof. Specifically:

1. A new discussion (DISC-011) MUST be opened to formalize the **distributed state decomposition**: define what it means for the distributed state to be "equivalent" to the centralized merged net, and prove that the decomposition is unique up to isomorphism.
2. A new argument (ARG-005) MUST be derived from DISC-011 to extend ARG-001's Layer 2 proof to the delta protocol. ARG-005 MUST show that P2 (split/merge identity) holds in a weaker form where the coordinator never reconstructs the full net between rounds, and that P3 (border completeness) holds when border redexes are detected via delta communication rather than exhaustive scan.

**Until DISC-011 and ARG-005 are completed, the delta protocol's design (this spec) is valid but its correctness proof is incomplete.** The spec can be implemented and empirically validated (Section 7, T6-T10 provide strong empirical evidence), but the formal argument is a separate work item.

**Status (2026-04-24): CLOSED.**
- DISC-011 v2 written: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\exploracoes\DISC-011-v2-distributed-state-decomposition.md`
- ARG-005 written: `C:\Users\Filipe\Desktop\TCC_interaction_combinators_for_grid_computing\discussoes\argumentos\ARG-005-delta-border-completeness.md`
- Strength: Moderado-Forte (same class as ARG-001/ARG-003)
- Gates closed: R38 (G1 reformulated), R39 (incremental ≡ exhaustive border detection via D3a-d), R40 (D6 termination preserved via T7 + progress guarantee)
- Three new premises introduced: P7 (C-DEL1, delta-reporting completeness), P8 (C-DEL2, delta-reporting soundness), P9 (determinism of `reconstruct`); plus E-FAIL1 as a scope condition.
- Theorem (INV-REC) proved by induction on rounds with four sub-obligations covering: (1) internal-only reductions, (2) cross-partition border redexes (4 sub-cases including emergent), (3) cascade propagation, (4) DC-B5 1-round-latency state with `pending` component. Degenerate n=1 case and coordinator-free rounds also covered explicitly.
- Empirical signature still pending: tests T6-T16 of this spec (SPEC-19) execution
- See `codigo/relativist/docs/theory-bridge.md` for the full bridge to TCC-root artifacts.

**Rationale for "CLOSED" despite empirical pendency:** the formal argument is complete and reviewed (DISC-011 went through 2 rounds of adversarial ping-pong before consolidating into v2; ARG-005 follows). Empirical validation is a SEPARATE concern (covered by Stage 5 QA when the implementation runs). The OQ being CLOSED means no further theoretical work blocks Stage 1 onwards.

**OQ-2. Delta semantics for CON-DUP border expansion.**

When a border redex involves CON-DUP commutation, 4 new agents are created. Two of these agents replace the original CON in one partition, and two replace the original DUP in the other partition. The new agents may have auxiliary ports that need to be connected across partitions (creating new border wires). The coordinator must:
- Create new `border_id` values for the new cross-partition connections.
- Send these new border IDs to the affected workers.
- Update the `BorderGraph` with the new border entries.

The exact delta format for this expansion needs careful design. The `new_borders` field in `RoundStart` (R23) handles this, but the detailed protocol for multi-agent expansion deltas should be validated against all 6 interaction rules to ensure completeness.

**OQ-3. Interaction with dynamic workers (ROADMAP 2.2, 2.3).**

When a new worker joins mid-execution under the delta protocol, the coordinator must:
1. Request current state from existing workers (`FinalStateRequest` mid-run).
2. Re-partition the reconstructed net for K+1 workers.
3. Send `InitialPartition` to the new worker and delta updates to existing workers reflecting the re-partitioning.

This interaction is non-trivial and is explicitly deferred to the spec for dynamic workers. SPEC-19 assumes a static worker set for the duration of the delta BSP loop.

**OQ-4. Memory overhead of `previous_border_state` on workers.**

Each worker maintains a `HashMap<u32, PortRef>` tracking the previous border state (R25). For nets with many borders (e.g., `ep_annihilation_con` 10M at w=4 has ~1.25M borders per worker), this adds ~20 MB per worker. This is negligible compared to partition state but should be monitored. An alternative is to use a generation counter instead of a full snapshot, computing deltas incrementally during `reduce_all`. This optimization is deferred.

**OQ-5. Strict BSP semantics under delta mode.**

The interaction between `strict_bsp = true` and `delta_mode = true` needs clarification. In v1 strict mode, the coordinator skips `reduce_all` after merge and re-partitions for the next round. In delta mode, "strict" could mean: (a) the coordinator does not resolve border redexes at all, deferring them to workers via deltas (but workers cannot reduce cross-partition pairs without the remote agent), or (b) the coordinator resolves border redexes but does not run a full `reduce_all` on the merged net (which does not apply in delta mode since there is no merged net). The most natural interpretation is that delta mode with strict BSP means: the coordinator resolves only the border redexes detected by the `BorderGraph` and dispatches resulting deltas to workers, without any additional reduction at the coordinator. This is the interpretation assumed in R40.

**OQ-6. Wire protocol version coordination with SPEC-18.**

The discriminant assignments in R31-R32 must be coordinated with SPEC-18. If SPEC-18 uses discriminants 7-11 for different purposes, the numbering must be adjusted. A coordination meeting between the SPEC-18 and SPEC-19 authors is recommended before implementation.

**OQ-7. 2-phase AgentId allocation latency on CON-DUP resolutions (DC-B5).**

The DC-B5 design choice (R23, R26, R48, and the new `PendingCommutation` /
`MintedAgent` wire types defined in R33) adopts Option (B) from the 2.26-B
spec-critic verdict: workers allocate new AgentId values locally from their
own `id_range`, and echo them back to the coordinator in the next round's
`RoundResult.minted_agents`. This respects SPEC-04's `id_range` invariant
(agent IDs stay within each worker's reserved range, preserving the dense
arena layout) at the cost of **one extra BSP round of latency** per CON-DUP
border-redex resolution: the coordinator detects the border redex at round
N, dispatches `pending_commutations` at round N+1, and can only finalize
the resulting border-graph entries (using the concrete agent IDs) at round
N+2.

The alternative Option (A) — coordinator-local AgentId allocation from a
reserved high range (e.g. `u32::MAX - 10_000 ..`) — would eliminate the
1-round latency but would break two invariants: (i) SPEC-04's `id_range`
dense-layout requirement, creating sparse gaps in the worker's agent
arena; and (ii) the arena-index invariant (`Net.agents` is a `Vec` indexed
by `AgentId as usize`, so `u32::MAX - k` indices force allocation of a
16 GB arena or a pivot to a hash-based arena). R48 reserves the
`u32::MAX - 10_000 .. u32::MAX` range to keep Option (A) re-openable as
a future optimization (e.g. for a streaming coordinator or hash-arena
workers) without breaking backwards compatibility of the wire protocol.

The concrete cost of the 1-round latency is bounded: CON-DUP border
redexes are rare on the benchmark workloads of interest (Profile B
expansion+collapse dominates CON-DUP *inside* worker partitions, not at
borders), and the per-round coordinator wall-clock cost is already
dominated by network I/O. Empirical validation against Option (A)
(coordinator-local allocation, as a feature-flagged variant) is deferred
to v2 benchmarks.

**See:** `docs/spec-reviews/SPEC-19-section-3.3-2.26B-design-choices-2026-04-17.md`
(DC-B5 section, lines 531-708) for the full design argument.

---

## 9. Changelog

| Date | Author | Change |
|------|--------|--------|
| 2026-04-24 | especialista-em-specs (post-DEBATEDOR) | **ARG-005 CLOSED.** §8 OQ-1 marked CLOSED with the new "Status (2026-04-24)" subsection (R38/R39/R40 gates released). Frontmatter "Arguments consumed:" updated to include ARG-005. Added cross-reference to `codigo/relativist/docs/theory-bridge.md`. **No semantic changes to any requirement R-NN.** This is purely a status-tracking update reflecting completed theoretical work. Source artifacts: `discussoes/argumentos/ARG-005-delta-border-completeness.md`, `discussoes/exploracoes/DISC-011-v2-distributed-state-decomposition.md`, `discussoes/exploracoes/DISC-013-arg-005-disambiguation.md`. |
| 2026-04-17 | spec-critic 2.26-B + especialista-em-specs | Amended R23 (add `local_reconnections`, `pending_commutations`), R26 (add `minted_agents`), R31 row for `RoundStart` (+2 fields), R32 row for `RoundResult` (+1 field), R33 (add `LocalReconnection`, `PendingCommutation`, `MintedAgent` structs). Added R48 (Agent ID allocation coordination invariant). Added OQ-7 (2-phase echo latency tradeoff). Ratifies DC-B3 and DC-B5 from the 2.26-B spec-review verdict. |
| 2026-04-23 | spec-critic D-005 + especialista-em-specs | Amended `PendingCommutation` wire encoding to carry `local_wiring: Vec<LocalWiringHint>` (Shape A from SPEC-REVIEW-19 §3.4; `(u8, u8, u8, u8)` rejected as lossy). Added `LocalWiringHint` struct mirroring the resolver's `(u8, u8, PortRef)` triple one-to-one. Added R23a (slot-to-AgentId substitution, per-request scope, mint-then-wire ordering, in-round reducibility of minted sibling principal pairs). Amended R24 (five-step sub-order inside step 1, R24.1.6a/b/c mint-wire-echo, sanctioned primitive `Net::connect`). Added R33b (sanctioned application primitive; `Net::wire_agents` explicitly forbidden). Added R33c (`ProtocolError::MalformedLocalWiringReason` with 6 cases; 5 MUST-reject + 1 SHOULD-warn). Added R48a (stray slot-marker guard, symmetric to R48 worker-side). Added R48b (empty `local_wiring` is legal). Bumped `PROTOCOL_VERSION` 2->3 (R37). Amended R34 (`rkyv` coverage + `--features zero-copy` baseline 1186 -> 1192) and R35 (wire-opt exemption does NOT apply to `local_wiring` hot path). Resolves SPEC-REVIEW-19 §3.4 D-005 (2 CRITICAL + 4 HIGH + 4 MEDIUM + 2 LOW). Round 2 redraft (2026-04-23): resolved SPEC-REVIEW-19-REREVIEW NF-001 by replacing `PendingCommutation.{symbol_type, arity}` with `target_symbols: Vec<Symbol>` (Shape A), rewriting R24.1.6a to mint slot `k` from `pc.target_symbols[k]` directly, and propagating the rename through R23a clause 3, R33c cases 1/3 (`arity` -> `symbol_count`), R34 alignment audit, and R48a; NF-002 by pinning symmetric `PROTOCOL_VERSION` validation in R37; NF-003 by adding R23a clause 6 (HashSet-based duplicate pre-pass before any `Net::connect`); NF-004 by rejecting empty `target_symbols` via new `MalformedLocalWiringReason::ZeroArity` (R33c case 7). NF-005 deferred to sdd-pipeline. |
| 2026-04-30 | especialista-em-specs (D-011 Phase A) | **R35a added — `CompactSubnet` wire encoding of `Net.free_list`.** Closes QA-D009-001 (CRITICAL, deferred from D-009 closure on 2026-04-27). The serde adapter at `relativist-core/src/partition/compact.rs:38` was silently dropping `Net.free_list` on serialization (`from_net()` did not capture it; `into_net()` reset it to `Vec::new()` at `compact.rs:113`), violating SPEC-22 R10a/R10b/R10c/R12 over the TCP transport path. R35a clauses (a)-(g): wire-form preservation as a new struct field after `root`; round-trip invariant extended to `free_list` and the `nets_equivalent` test helper at `compact.rs:152-158`; empty-vs-populated boundary discipline; `--features zero-copy` rkyv coverage extension; **PROTOCOL_VERSION bump from `PREVIOUS_LIVE_VERSION` to `PREVIOUS_LIVE_VERSION + 1`** (live constant currently `6` post D-009 + D-010 at `protocol/coordinator.rs:197`; R35a lands at `7`, but defensive language mandated in line with SPEC-06 R3b / SPEC-19 R37 / SPEC-22 R9a precedents); atomic-with-R12 reconciliation interaction; Phase E-4 smoke test as the acceptance gate. Frontmatter status line updated; no other R-NN modified. Spec-only amendment — implementation deferred to D-011 Phase B-1 (developer agent). Cross-references propagated to SPEC-04 §A7, SPEC-22 §3.8 A11, SPEC-18 R28+R33. Source artifacts: `codigo/relativist/docs/qa/QA-PHASE-D009-spec22-arena-2026-04-27.md`, `codigo/relativist/docs/backlog/TASK-0595-compactsubnet-free-list-followup.md`, `codigo/relativist/docs/spec-reviews/SPEC-19-R35a-closure-2026-04-30.md`. |
