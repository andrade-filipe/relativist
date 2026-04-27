//! Helper functions for net partitioning (SPEC-04).

use std::collections::{BTreeSet, HashMap};

use std::collections::VecDeque;

use crate::merge::GridConfig;
use crate::net::{total_ports, AgentId, Net, PortId, PortRef, DISCONNECTED, PORTS_PER_SLOT};

use super::types::{IdRange, WorkerId};

/// Returns the maximum FreePort ID in the net's port array, or `None` if
/// there are no FreePort entries (excluding DISCONNECTED sentinels).
///
/// Used by the split function (SPEC-04 R12) to compute the starting border
/// ID: `border_id_start = max_freeport_id(net).unwrap_or(0) + 1`, ensuring
/// that new border IDs never collide with pre-existing (Lafont) FreePort IDs.
///
/// Complexity: O(P) where P is the size of the port array.
pub fn max_freeport_id(net: &Net) -> Option<u32> {
    let mut max_id: Option<u32> = None;
    for &port_ref in &net.ports {
        if let PortRef::FreePort(id) = port_ref {
            if port_ref != DISCONNECTED {
                max_id = Some(match max_id {
                    Some(current) => current.max(id),
                    None => id,
                });
            }
        }
    }
    max_id
}

/// Computes the static ID space ranges for `num_workers` workers (SPEC-04 Section 4.7).
///
/// Each worker gets a compact, disjoint ID range starting from `base_next_id`.
/// The chunk size is proportional to the existing net size with a minimum of
/// 100,000 IDs per worker, providing ample room for agent creation during
/// local reduction without allocating multi-billion-entry sparse arrays.
///
/// The last worker's range extends to `u32::MAX` as a safety margin.
///
/// Panics if `num_workers == 0`.
pub fn compute_id_ranges(num_workers: u32, base_next_id: u32) -> Vec<IdRange> {
    assert!(num_workers > 0, "num_workers must be >= 1");

    // Each worker gets enough IDs for substantial agent creation.
    // Minimum 100K per worker; proportional to existing net size.
    let min_chunk: u64 = 100_000;
    let proportional: u64 = (base_next_id as u64).saturating_mul(10);
    let chunk_size: u64 = min_chunk.max(proportional);

    (0..num_workers)
        .map(|i| {
            let start_64 = base_next_id as u64 + (i as u64) * chunk_size;
            let end_64 = if i == num_workers - 1 {
                u32::MAX as u64
            } else {
                base_next_id as u64 + ((i + 1) as u64) * chunk_size
            };
            let start = (start_64.min(u32::MAX as u64 - 1)) as u32;
            let end = (end_64.min(u32::MAX as u64)) as u32;
            IdRange { start, end }
        })
        .collect()
}

/// SPEC-20 R8, R13, R30 (TASK-0421): Computes the disjoint ID ranges for
/// the current round's active membership, accounting for hybrid-coordinator
/// mode.
///
/// K_eff = active_workers.len() + (1 if hybrid_coordinator else 0).
///
/// Under hybrid mode, the coordinator's self-partition always takes
/// `partition_index = 0` and is assigned the first range. Remote workers
/// are assigned indices `1..K_eff` based on their `WorkerId` ascending.
///
/// Under non-hybrid mode, remote workers take all indices `0..num_workers`
/// by `WorkerId` ascending.
pub fn compute_round_id_ranges(
    config: &GridConfig,
    active_workers: &BTreeSet<WorkerId>,
    base_next_id: u32,
) -> HashMap<WorkerId, IdRange> {
    let k = active_workers.len() as u32;
    let k_eff = k + if config.hybrid_coordinator { 1 } else { 0 };

    if k_eff == 0 {
        return HashMap::new();
    }

    let ranges = compute_id_ranges(k_eff, base_next_id);
    let mut map = HashMap::with_capacity(k_eff as usize);

    if config.hybrid_coordinator {
        // R8: self-partition is at index 0 (reserved WorkerId 0).
        map.insert(0, ranges[0]);
    }

    // Assign ranges to remote workers by WorkerId ascending.
    // Index offset is 1 if hybrid, 0 if not.
    let offset = if config.hybrid_coordinator { 1 } else { 0 };
    for (i, &worker_id) in active_workers.iter().enumerate() {
        map.insert(worker_id, ranges[i + offset]);
    }

    // --- Invariant Defense (TASK-0452, MF-003 / QA-006) ---
    // D4-elastic (SPEC-20 R11a): every consumed `partition_index` must be
    // in `[0, K_eff)` and dense — no gaps, no duplicates, no out-of-range.
    // Tests this property *after* the map is built: the previous tautological
    // `K_eff == K_eff` check has been replaced by the real positional
    // density invariant (MF-003), extended with a per-index pass (QA-006).
    #[cfg(debug_assertions)]
    {
        debug_assert_eq!(
            map.len() as u32,
            k_eff,
            "D4-elastic violated: |map| = {} but K_eff = {}",
            map.len(),
            k_eff
        );
        for (&wid, range) in &map {
            debug_assert!(
                range.start < range.end,
                "D4-elastic violated: degenerate IdRange for worker {}: {:?}",
                wid,
                range
            );
        }
        // Density: build the set of consumed `partition_index` values and
        // assert it equals exactly `{0, 1, ..., K_eff - 1}` (R11a).
        let mut consumed_indices: Vec<u32> = Vec::with_capacity(k_eff as usize);
        if config.hybrid_coordinator {
            consumed_indices.push(0);
        }
        for &wid in active_workers {
            if let Some(idx) = partition_index_of(wid, active_workers, config.hybrid_coordinator) {
                consumed_indices.push(idx);
            }
        }
        consumed_indices.sort();
        consumed_indices.dedup();
        debug_assert_eq!(
            consumed_indices.len() as u32,
            k_eff,
            "D4-elastic violated: dense partition_index set [0, {}) was not formed: got {:?}",
            k_eff,
            consumed_indices
        );
        for (i, &idx) in consumed_indices.iter().enumerate() {
            debug_assert_eq!(
                idx, i as u32,
                "D4-elastic violated: gap or duplicate in partition_index sequence at position {}: {:?}",
                i, consumed_indices
            );
        }
    }

    map
}

/// SPEC-20 R11a (TASK-0420): Computes the dense partition index `[0, K_eff)`
/// for a given `worker_id` in the current round's active set.
///
/// If the grid is in hybrid-coordinator mode, `WorkerId 0` is permanently
/// reserved for the coordinator's self-partition and always assigned
/// `partition_index = 0`. Remote workers are assigned indices `1..K_eff`
/// based on their `WorkerId` ascending.
///
/// In non-hybrid mode, remote workers take all indices `0..K_eff` by
/// `WorkerId` ascending.
///
/// Returns `None` if the `worker_id` is not in the active set.
pub fn partition_index_of(
    worker_id: WorkerId,
    active_workers: &BTreeSet<WorkerId>,
    hybrid_mode: bool,
) -> Option<u32> {
    if hybrid_mode && worker_id == 0 {
        return Some(0);
    }

    let position = active_workers.iter().position(|&id| id == worker_id)?;
    let offset = if hybrid_mode { 1 } else { 0 };
    Some((position as u32) + offset)
}

/// Result of wire classification (SPEC-04 Section 4.4, Step 4).
pub struct WireClassification {
    /// Border map: borderId -> (original endpoint A, original endpoint B).
    pub borders: HashMap<u32, (PortRef, PortRef)>,

    /// Per-worker border entries: `border_entries[worker_id]` contains
    /// `(agent_id, port_id, border_id)` for each border wire touching that worker.
    pub border_entries: Vec<Vec<(AgentId, PortId, u32)>>,

    /// The next border ID after all assignments (exclusive end).
    pub border_id_end: u32,

    /// The first border ID assigned (inclusive start).
    pub border_id_start: u32,
}

/// Classifies all wires in the net as internal, interface, or border
/// (SPEC-04 Section 4.4, Step 4 of the split algorithm).
///
/// A border wire is detected only from the side with the smaller AgentId
/// (`agent_id < other_id`), but FreePort entries are generated for BOTH
/// partitions in a single pass.
///
/// Complexity: O(A * PORTS_PER_SLOT) where A is the number of live agents.
pub fn classify_wires(
    net: &Net,
    sigma: &HashMap<AgentId, WorkerId>,
    num_workers: u32,
) -> WireClassification {
    let border_id_start = max_freeport_id(net).map_or(0, |id| id + 1);
    let mut border_id_counter = border_id_start;
    let mut borders = HashMap::new();
    let mut border_entries: Vec<Vec<(AgentId, PortId, u32)>> = vec![vec![]; num_workers as usize];

    for (i, slot) in net.agents.iter().enumerate() {
        let agent = match slot {
            Some(a) => a,
            None => continue,
        };
        let agent_id = i as AgentId;

        for port_id in 0..total_ports(agent.symbol) as PortId {
            let target = net.get_target(PortRef::AgentPort(agent_id, port_id));

            if let PortRef::AgentPort(other_id, other_port) = target {
                // Only process from the smaller-ID side to avoid duplicates
                if agent_id < other_id {
                    let w_a = sigma[&agent_id];
                    let w_b = sigma[&other_id];

                    if w_a != w_b {
                        // Border wire: assign new border ID
                        let bid = border_id_counter;
                        border_id_counter += 1;

                        borders.insert(
                            bid,
                            (
                                PortRef::AgentPort(agent_id, port_id),
                                PortRef::AgentPort(other_id, other_port),
                            ),
                        );

                        border_entries[w_a as usize].push((agent_id, port_id, bid));
                        border_entries[w_b as usize].push((other_id, other_port, bid));
                    }
                }
            }
            // FreePort -> interface wire (no action)
            // DISCONNECTED -> skip (no action)
            // AgentPort with same worker -> internal wire (no action)
        }
    }

    WireClassification {
        borders,
        border_entries,
        border_id_end: border_id_counter,
        border_id_start,
    }
}

/// Builds a sub-net for one partition (SPEC-04 Section 4.5, Step 5).
///
/// Creates a `Net` containing only the agents in `worker_agents`, with:
/// - Internal wires copied directly from the original net.
/// - Border wires replaced by `FreePort(bid)` connections.
/// - Interface wires (pre-existing FreePorts) copied as-is.
/// - Redex queue populated with only internal Active Pairs.
///
/// SPEC-22 R10a: the returned net has `id_range = Some(id_range.clone())` and
/// its `free_list` is populated with all `None` slots in `id_range` (ascending
/// iteration). This makes recycled IDs within the partition's range immediately
/// available to the worker's local `create_agent` calls.
///
/// The `agents` and `ports` Vecs are sized to `max_id + 1` (preserving
/// the original ID indexing scheme). Unused slots are None/DISCONNECTED.
///
/// Pass `0..u32::MAX` as `id_range` for non-distributed (whole-net) contexts
/// where ID-range enforcement is not needed.
pub fn build_subnet(
    net: &Net,
    worker_agents: &[AgentId],
    sigma: &HashMap<AgentId, WorkerId>,
    border_entries: &[(AgentId, PortId, u32)],
    worker_id: WorkerId,
    id_range: core::ops::Range<AgentId>,
) -> Net {
    if worker_agents.is_empty() {
        return Net::new();
    }

    let max_id = *worker_agents.iter().max().unwrap() as usize;
    let agents_len = max_id + 1;
    let ports_len = agents_len * PORTS_PER_SLOT;

    // Initialize with None/DISCONNECTED
    let mut agents: Vec<Option<crate::net::Agent>> = vec![None; agents_len];
    let mut ports: Vec<PortRef> = vec![DISCONNECTED; ports_len];

    // Build a set of border overrides: (agent_id, port_id) -> FreePort(bid)
    let mut border_overrides: HashMap<(AgentId, PortId), u32> = HashMap::new();
    for &(agent_id, port_id, bid) in border_entries {
        border_overrides.insert((agent_id, port_id), bid);
    }

    // Copy agents and their port connections
    for &agent_id in worker_agents {
        let agent = net.agents[agent_id as usize]
            .as_ref()
            .expect("worker_agents should only contain live agent IDs");

        agents[agent_id as usize] = Some(*agent);

        // Copy all PORTS_PER_SLOT port entries (preserves uniform layout)
        for port_id in 0..PORTS_PER_SLOT as PortId {
            let idx = agent_id as usize * PORTS_PER_SLOT + port_id as usize;

            if let Some(&bid) = border_overrides.get(&(agent_id, port_id)) {
                // Border wire: replace with FreePort(bid)
                ports[idx] = PortRef::FreePort(bid);
            } else {
                // Internal, interface, or DISCONNECTED: copy as-is
                ports[idx] = net.ports[idx];
            }
        }
    }

    // Populate redex queue with only internal Active Pairs
    let mut redex_queue = VecDeque::new();
    for &(a_id, b_id) in &net.redex_queue {
        // Both agents must be in this partition
        if sigma.get(&a_id) == Some(&worker_id) && sigma.get(&b_id) == Some(&worker_id) {
            // Verify both agents still exist in our sub-net
            if agents[a_id as usize].is_some() && agents[b_id as usize].is_some() {
                redex_queue.push_back((a_id, b_id));
            }
        }
    }

    // SPEC-22 R10a: populate free_list with in-range None slots (ascending scan).
    // Clamp iteration to arena_len to avoid out-of-bounds (TEST-SPEC-0481 UT-0481-05).
    let arena_len = agents.len() as AgentId;
    let range_start = id_range.start;
    let range_end = id_range.end.min(arena_len);
    let mut free_list = Vec::new();
    for id in range_start..range_end {
        if agents.get(id as usize).is_some_and(|s| s.is_none()) {
            free_list.push(id);
        }
    }

    Net {
        agents,
        ports,
        redex_queue,
        next_id: 0, // Caller sets this based on ID range
        root: None, // Caller sets this based on R28
        freeport_redirects: std::collections::HashMap::new(),
        free_list,
        id_range: Some(id_range),
        border_entries_shadow: None,
        recycle_policy: crate::net::core::RecyclePolicy::DisableUnderDelta,
        is_in_delta_round: false,
        #[cfg(debug_assertions)]
        protected_tombstones: None,
    }
}

/// SPEC-22 §3.4 R30: `build_subnet` with config-driven M5 threshold guard.
///
/// Wraps `build_subnet` with a threshold check when `config.sparse_build == false`:
/// - If `id_range_size > 4 × live_count`: returns
///   `Err(PartitionError::DenseAllocationExceedsThreshold { ... })`.
/// - If `id_range_size <= 4 × live_count` OR `config.sparse_build == true`:
///   delegates to `build_subnet` and returns `Ok(net)`.
///
/// The `partition_index` parameter is passed through to the error payload
/// for diagnostic purposes (identifies which partition triggered the guard).
///
/// # Threshold
///
/// The 4× factor is a fixed safety margin (SPEC-22 R22). It is NOT parameterized.
/// A dense arena with `id_range_size > 4 × live_count` would have >75% `None`
/// slots, reaching the 800 MiB pathology at ~200K agents (M5 evidence base).
// 8 params — justified: config, partition_index, net, agents, sigma, borders, worker_id, id_range.
// The signature mirrors `build_subnet` with config prepended; further reduction would
// require a builder struct (not warranted for this guard-only wrapper).
#[allow(clippy::too_many_arguments)]
pub fn build_subnet_with_config(
    config: &super::types::PartitionConfig,
    partition_index: usize,
    net: &Net,
    worker_agents: &[AgentId],
    sigma: &HashMap<AgentId, WorkerId>,
    border_entries: &[(AgentId, PortId, u32)],
    worker_id: WorkerId,
    id_range: core::ops::Range<AgentId>,
) -> Result<Net, crate::error::PartitionError> {
    if !config.sparse_build {
        // SPEC-22 R30: threshold check for the dense path.
        let id_range_size = (id_range.end as u64).saturating_sub(id_range.start as u64);
        let live_count = worker_agents.len() as u64;
        if id_range_size > 4 * live_count {
            return Err(crate::error::PartitionError::DenseAllocationExceedsThreshold {
                partition_index,
                id_range_size,
                live_count,
            });
        }
    }
    // All other cases: sparse_build=true (any ratio) or dense below threshold.
    Ok(build_subnet(net, worker_agents, sigma, border_entries, worker_id, id_range))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;

    // T1: Empty net returns None
    #[test]
    fn test_max_freeport_id_empty_net() {
        let net = Net::new();
        assert_eq!(max_freeport_id(&net), None);
    }

    // T2: Net with agents but no FreePort connections returns None
    #[test]
    fn test_max_freeport_id_no_freeports() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        // Principal ports connected to each other (not FreePort)
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert_eq!(max_freeport_id(&net), None);
    }

    // T3: Net with one FreePort connection returns that ID
    #[test]
    fn test_max_freeport_id_single() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        assert_eq!(max_freeport_id(&net), Some(5));
    }

    // T4: Returns maximum among multiple FreePort IDs
    #[test]
    fn test_max_freeport_id_multiple() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(42));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(7));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(99));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(50));
        assert_eq!(max_freeport_id(&net), Some(99));
    }

    // T5: DISCONNECTED (FreePort(u32::MAX)) is excluded
    #[test]
    fn test_max_freeport_id_excludes_disconnected() {
        let mut net = Net::new();
        // Creating an agent leaves aux ports as DISCONNECTED = FreePort(u32::MAX)
        let a = net.create_agent(Symbol::Con);
        // Only connect principal port to a real FreePort
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));
        // Ports 1 and 2 are DISCONNECTED
        let result = max_freeport_id(&net);
        assert_eq!(result, Some(5)); // u32::MAX excluded
    }

    // E1: FreePort(0) is valid and returned
    #[test]
    fn test_max_freeport_id_zero() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        assert_eq!(max_freeport_id(&net), Some(0));
    }

    // -----------------------------------------------------------------------
    // compute_id_ranges tests
    // -----------------------------------------------------------------------

    // R1: Single worker gets range from base to u32::MAX
    #[test]
    fn test_id_ranges_single_worker() {
        let ranges = compute_id_ranges(1, 10);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, 10);
        assert_eq!(ranges[0].end, u32::MAX);
    }

    // R2: Two workers produce contiguous disjoint ranges
    #[test]
    fn test_id_ranges_two_workers() {
        let ranges = compute_id_ranges(2, 100);
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start, 100);
        assert_eq!(ranges[1].end, u32::MAX);
        // Contiguous: first ends where second starts
        assert_eq!(ranges[0].end, ranges[1].start);
    }

    // R3: 8 workers produce contiguous ranges
    #[test]
    fn test_id_ranges_eight_workers() {
        let ranges = compute_id_ranges(8, 0);
        assert_eq!(ranges.len(), 8);
        assert_eq!(ranges[0].start, 0);
        assert_eq!(ranges[7].end, u32::MAX);
        for i in 0..7 {
            assert_eq!(ranges[i].end, ranges[i + 1].start);
        }
    }

    // R4: Ranges from non-zero base are disjoint and contiguous
    #[test]
    fn test_id_ranges_nonzero_base() {
        let base = 50;
        let ranges = compute_id_ranges(4, base);
        assert_eq!(ranges[0].start, base);
        assert_eq!(ranges[3].end, u32::MAX);
        for i in 0..3 {
            assert_eq!(ranges[i].end, ranges[i + 1].start);
        }
    }

    // R5: Last worker extends to u32::MAX
    #[test]
    fn test_id_ranges_last_worker_extends_to_max() {
        let ranges = compute_id_ranges(3, 10);
        assert_eq!(ranges[2].end, u32::MAX);
        // All ranges have positive size
        for r in &ranges {
            assert!(r.end > r.start);
        }
    }

    // R6: Each worker gets at least 100K IDs (min chunk guarantee)
    #[test]
    fn test_id_ranges_min_chunk_size() {
        let ranges = compute_id_ranges(4, 5);
        // With base=5, proportional=50 < min=100_000, so chunk=100_000
        // Worker 0: [5, 100_005), Worker 1: [100_005, 200_005), etc.
        for range in &ranges[..3] {
            assert!(range.end - range.start >= 100_000);
        }
    }

    // E2: Panics on 0 workers
    #[test]
    #[should_panic(expected = "num_workers must be >= 1")]
    fn test_id_ranges_zero_workers_panics() {
        compute_id_ranges(0, 0);
    }

    // -----------------------------------------------------------------------
    // classify_wires tests
    // -----------------------------------------------------------------------

    // Helper: create a sigma map from a list of (agent_id, worker_id) pairs
    fn make_sigma(pairs: &[(AgentId, WorkerId)]) -> HashMap<AgentId, WorkerId> {
        pairs.iter().copied().collect()
    }

    // W1: Empty net — no borders
    #[test]
    fn test_classify_wires_empty_net() {
        let net = Net::new();
        let sigma = HashMap::new();
        let result = classify_wires(&net, &sigma, 2);
        assert!(result.borders.is_empty());
        assert_eq!(result.border_entries.len(), 2);
        assert!(result.border_entries[0].is_empty());
        assert!(result.border_entries[1].is_empty());
    }

    // W2: Two agents in same partition (internal wire) — no borders
    #[test]
    fn test_classify_wires_internal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let sigma = make_sigma(&[(a, 0), (b, 0)]); // same worker
        let result = classify_wires(&net, &sigma, 2);
        assert!(result.borders.is_empty());
    }

    // W3: Two agents in different partitions — one border wire
    #[test]
    fn test_classify_wires_single_border() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]); // different workers
        let result = classify_wires(&net, &sigma, 2);

        assert_eq!(result.borders.len(), 1);
        assert_eq!(result.border_entries[0].len(), 1);
        assert_eq!(result.border_entries[1].len(), 1);
        // Same border ID in both entries
        assert_eq!(result.border_entries[0][0].2, result.border_entries[1][0].2);
    }

    // W4: Border ID starts after max existing FreePort
    #[test]
    fn test_classify_wires_border_id_after_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Principal ports cross partitions
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Aux ports use FreePort IDs
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(20));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(5));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(15));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // max_freeport_id = 20, so border IDs start at 21
        assert_eq!(result.border_id_start, 21);
        let bid = result.border_entries[0][0].2;
        assert!(bid >= 21);
    }

    // W5: Multiple border wires
    #[test]
    fn test_classify_wires_multiple_borders() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // All ports cross partitions
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 2));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 1));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // 3 border wires (principal + 2 aux cross)
        assert_eq!(result.borders.len(), 3);
        assert_eq!(result.border_entries[0].len(), 3);
        assert_eq!(result.border_entries[1].len(), 3);
    }

    // W6: Interface wire (FreePort) — not classified as border
    #[test]
    fn test_classify_wires_interface_ignored() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));

        let sigma = make_sigma(&[(a, 0)]);
        let result = classify_wires(&net, &sigma, 2);
        assert!(result.borders.is_empty());
    }

    // W7: border_id_start and border_id_end bracket the assigned IDs
    #[test]
    fn test_classify_wires_border_id_range() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        assert_eq!(result.border_id_end, result.border_id_start + 1);
    }

    // W8: No borders when net has no FreePort -> start at 0
    #[test]
    fn test_classify_wires_no_preexisting_freeport() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // No pre-existing FreePorts (only DISCONNECTED), start at 0
        assert_eq!(result.border_id_start, 0);
    }

    // W9: Each border appears exactly once in borders map
    #[test]
    fn test_classify_wires_no_duplicate_borders() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let result = classify_wires(&net, &sigma, 2);

        // Only principal-principal is a border (1 border wire)
        assert_eq!(result.borders.len(), 1);
    }

    // -----------------------------------------------------------------------
    // build_subnet tests
    // -----------------------------------------------------------------------

    // S1: Empty worker agents -> empty net
    #[test]
    fn test_build_subnet_empty() {
        let net = Net::new();
        let sigma = HashMap::new();
        let subnet = build_subnet(&net, &[], &sigma, &[], 0, 0..u32::MAX);
        assert_eq!(subnet.agents.len(), 0);
    }

    // S2: Single agent, no borders
    #[test]
    fn test_build_subnet_single_agent() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));

        let sigma = make_sigma(&[(a, 0)]);
        let subnet = build_subnet(&net, &[a], &sigma, &[], 0, 0..u32::MAX);

        assert!(subnet.agents[a as usize].is_some());
        assert_eq!(
            subnet.ports[a as usize * PORTS_PER_SLOT],
            PortRef::FreePort(5)
        );
    }

    // S3: Internal wire preserved
    #[test]
    fn test_build_subnet_internal_wire() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let sigma = make_sigma(&[(a, 0), (b, 0)]);
        let subnet = build_subnet(&net, &[a, b], &sigma, &[], 0, 0..u32::MAX);

        // Principal ports still connected to each other
        assert_eq!(
            subnet.ports[a as usize * PORTS_PER_SLOT],
            PortRef::AgentPort(b, 0)
        );
        assert_eq!(
            subnet.ports[b as usize * PORTS_PER_SLOT],
            PortRef::AgentPort(a, 0)
        );
    }

    // S4: Border wire replaced by FreePort(bid)
    #[test]
    fn test_build_subnet_border_wire() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let sigma = make_sigma(&[(a, 0), (b, 1)]);
        let border_entries_w0 = vec![(a, 0 as PortId, 42u32)];
        let subnet = build_subnet(&net, &[a], &sigma, &border_entries_w0, 0, 0..u32::MAX);

        // a's principal port now points to FreePort(42)
        assert_eq!(
            subnet.ports[a as usize * PORTS_PER_SLOT],
            PortRef::FreePort(42)
        );
    }

    // S5: Unused slots are None/DISCONNECTED
    #[test]
    fn test_build_subnet_unused_slots() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0
        let _b = net.create_agent(Symbol::Era); // id=1
        let c = net.create_agent(Symbol::Era); // id=2
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::FreePort(1));

        // Only include a and c (skip b at id=1)
        let sigma = make_sigma(&[(a, 0), (c, 0)]);
        let subnet = build_subnet(&net, &[a, c], &sigma, &[], 0, 0..u32::MAX);

        // Slot 1 (b) should be None
        assert!(subnet.agents[1].is_none());
        // Slot 1's ports should be DISCONNECTED
        for p in 0..PORTS_PER_SLOT {
            assert_eq!(subnet.ports[PORTS_PER_SLOT + p], DISCONNECTED);
        }
    }

    // S6: Redex queue contains only internal pairs
    #[test]
    fn test_build_subnet_redex_queue() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        // a-b: internal pair in worker 0
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // c-d: cross-partition pair
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let sigma = make_sigma(&[(a, 0), (b, 0), (c, 0), (d, 1)]);
        let subnet = build_subnet(&net, &[a, b, c], &sigma, &[(c, 0, 100)], 0, 0..u32::MAX);

        // Only (a, b) should be in the queue, not (c, d)
        assert_eq!(subnet.redex_queue.len(), 1);
        let (ra, rb) = subnet.redex_queue[0];
        assert!((ra == a && rb == b) || (ra == b && rb == a));
    }

    // S7: Interface wire (FreePort) preserved
    #[test]
    fn test_build_subnet_interface_wire() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(99));

        let sigma = make_sigma(&[(a, 0)]);
        let subnet = build_subnet(&net, &[a], &sigma, &[], 0, 0..u32::MAX);

        assert_eq!(
            subnet.ports[a as usize * PORTS_PER_SLOT],
            PortRef::FreePort(99)
        );
    }

    // === compute_round_id_ranges tests (TASK-0421) ===

    #[test]
    fn test_compute_round_id_ranges_hybrid() {
        let config = GridConfig {
            hybrid_coordinator: true,
            ..GridConfig::default()
        };

        let mut active = BTreeSet::new();
        active.insert(10);
        active.insert(5);

        let ranges = compute_round_id_ranges(&config, &active, 100);

        // K_eff = 2 (remote) + 1 (hybrid) = 3
        assert_eq!(ranges.len(), 3);

        // Coordinator (WorkerId 0) must be at index 0
        assert!(ranges.contains_key(&0));
        let r0 = ranges[&0];

        // Workers 5 and 10 must be sorted
        assert!(ranges.contains_key(&5));
        assert!(ranges.contains_key(&10));
        let r5 = ranges[&5];
        let r10 = ranges[&10];

        // Ranges must be contiguous and sorted by partition index
        // index 0: Worker 0
        // index 1: Worker 5
        // index 2: Worker 10
        assert_eq!(r0.end, r5.start);
        assert_eq!(r5.end, r10.start);
        assert_eq!(r10.end, u32::MAX);
    }

    #[test]
    fn test_compute_round_id_ranges_non_hybrid() {
        let config = GridConfig {
            hybrid_coordinator: false,
            ..GridConfig::default()
        };

        let mut active = BTreeSet::new();
        active.insert(1);
        active.insert(2);

        let ranges = compute_round_id_ranges(&config, &active, 0);

        // K_eff = 2
        assert_eq!(ranges.len(), 2);
        assert!(!ranges.contains_key(&0));
        assert!(ranges.contains_key(&1));
        assert!(ranges.contains_key(&2));

        let r1 = ranges[&1];
        let r2 = ranges[&2];
        assert_eq!(r1.end, r2.start);
        assert_eq!(r2.end, u32::MAX);
    }

    #[test]
    fn test_compute_round_id_ranges_empty() {
        let config = GridConfig::default();
        let active = BTreeSet::new();
        let ranges = compute_round_id_ranges(&config, &active, 0);
        assert!(ranges.is_empty());
    }

    // === partition_index_of tests (TASK-0420) ===

    #[test]
    fn test_partition_index_of_hybrid() {
        let mut active = BTreeSet::new();
        active.insert(10);
        active.insert(5);

        // Reserved ID 0 (hybrid)
        assert_eq!(partition_index_of(0, &active, true), Some(0));

        // Remote workers (offset by 1)
        assert_eq!(partition_index_of(5, &active, true), Some(1));
        assert_eq!(partition_index_of(10, &active, true), Some(2));

        // Missing ID
        assert_eq!(partition_index_of(7, &active, true), None);
    }

    #[test]
    fn test_partition_index_of_non_hybrid() {
        let mut active = BTreeSet::new();
        active.insert(10);
        active.insert(5);

        // No ID 0 reservation
        assert_eq!(partition_index_of(0, &active, false), None);

        // Remote workers (no offset)
        assert_eq!(partition_index_of(5, &active, false), Some(0));
        assert_eq!(partition_index_of(10, &active, false), Some(1));
    }

    // -----------------------------------------------------------------------
    // TASK-0481: build_subnet populates partition free-list (SPEC-22 R10a)
    // -----------------------------------------------------------------------

    /// UT-0481-01: build_subnet populates free_list with in-range None slots.
    #[test]
    fn build_subnet_populates_free_list_in_range() {
        use std::collections::HashSet;
        let mut net = Net::new();
        // Create 200 agents (IDs 0..199)
        for _ in 0..200 { net.create_agent(Symbol::Con); }
        // Remove 4 agents
        net.remove_agent(50);
        net.remove_agent(75);
        net.remove_agent(90);
        net.remove_agent(150);
        // Partition 0: id_range 0..100
        let p0_agents: Vec<_> = (0u32..100).filter(|&id| net.get_agent(id).is_some()).collect();
        let sigma: HashMap<AgentId, WorkerId> = (0u32..200)
            .filter_map(|id| net.get_agent(id).map(|_| (id, if id < 100 { 0 } else { 1 })))
            .collect();
        let p0 = build_subnet(&net, &p0_agents, &sigma, &[], 0, 0..100);
        let free_ids: HashSet<AgentId> = p0.free_list.iter().copied().collect();
        assert!(free_ids.contains(&50), "R10a: 50 must be in p0 free_list");
        assert!(free_ids.contains(&75), "R10a: 75 must be in p0 free_list");
        assert!(free_ids.contains(&90), "R10a: 90 must be in p0 free_list");
        assert!(!free_ids.contains(&150), "R10a: 150 is in partition 1, must NOT be in p0 free_list");
    }

    /// UT-0481-02: build_subnet excludes out-of-range None slots.
    #[test]
    fn build_subnet_excludes_out_of_range_none_slots() {
        let mut net = Net::new();
        for _ in 0..200 { net.create_agent(Symbol::Con); }
        net.remove_agent(50);
        net.remove_agent(150); // partition 1 only
        let p0_agents: Vec<_> = (0u32..100).filter(|&id| net.get_agent(id).is_some()).collect();
        let sigma: HashMap<AgentId, WorkerId> = (0u32..200)
            .filter_map(|id| net.get_agent(id).map(|_| (id, if id < 100 { 0 } else { 1 })))
            .collect();
        let p0 = build_subnet(&net, &p0_agents, &sigma, &[], 0, 0..100);
        assert!(!p0.free_list.iter().any(|&id| id >= 100),
            "R10a: free_list must not contain ids >= 100");
    }

    /// UT-0481-03: partition 1 only contains partition 1 freed agents.
    #[test]
    fn build_subnet_partition_1_only_contains_partition_1_freed() {
        use std::collections::HashSet;
        let mut net = Net::new();
        for _ in 0..200 { net.create_agent(Symbol::Con); }
        net.remove_agent(50);
        net.remove_agent(150);
        let p1_agents: Vec<_> = (100u32..200).filter(|&id| net.get_agent(id).is_some()).collect();
        let sigma: HashMap<AgentId, WorkerId> = (0u32..200)
            .filter_map(|id| net.get_agent(id).map(|_| (id, if id < 100 { 0 } else { 1 })))
            .collect();
        let p1 = build_subnet(&net, &p1_agents, &sigma, &[], 1, 100..200);
        let free_ids: HashSet<AgentId> = p1.free_list.iter().copied().collect();
        assert!(free_ids.contains(&150), "R10a: 150 must be in p1 free_list");
        assert!(!free_ids.contains(&50), "R10a: 50 is in partition 0, must NOT be in p1 free_list");
    }

    /// UT-0481-04: empty id_range yields empty free_list.
    #[test]
    fn build_subnet_empty_id_range_yields_empty_free_list() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let sigma = make_sigma(&[(a, 0)]);
        let p = build_subnet(&net, &[a], &sigma, &[], 0, 0..0);
        assert!(p.free_list.is_empty(), "degenerate range: free_list must be empty");
    }

    /// UT-0481-05: id_range clamped to arena_len (no out-of-bounds panic).
    #[test]
    fn build_subnet_id_range_clamped_to_arena_len() {
        let mut net = Net::new();
        for _ in 0..100 { net.create_agent(Symbol::Con); }
        let agents_in_range: Vec<_> = (0u32..100).filter(|&id| net.get_agent(id).is_some()).collect();
        let sigma: HashMap<AgentId, WorkerId> = (0u32..100)
            .filter_map(|id| net.get_agent(id).map(|_| (id, 0u32)))
            .collect();
        // id_range extends beyond arena — should clamp to arena_len=100
        let p = build_subnet(&net, &agents_in_range, &sigma, &[], 0, 100..300);
        // No agents in [100..300) — all slots live in [0..100), but range starts at 100
        // so free_list is empty (arena_len is 100; clamped end = min(300, 100) = 100 > 100 = false)
        assert!(p.free_list.is_empty(), "clamped range [100..100) yields empty free_list");
    }

    /// UT-0481-06: build_subnet sets id_range on returned net.
    #[test]
    fn build_subnet_sets_id_range_on_returned_net() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let sigma = make_sigma(&[(a, 0)]);
        let p = build_subnet(&net, &[a], &sigma, &[], 0, 0..100);
        assert_eq!(p.id_range, Some(0..100), "R10a: id_range must be set on subnet");
    }

    /// UT-0481-07: LIFO-compatible push order (ascending push -> LIFO top is highest).
    #[test]
    fn build_subnet_lifo_compatible_push_order() {
        let mut net = Net::new();
        for _ in 0..100 { net.create_agent(Symbol::Con); }
        net.remove_agent(50);
        net.remove_agent(75);
        net.remove_agent(90);
        let p0_agents: Vec<_> = (0u32..100).filter(|&id| net.get_agent(id).is_some()).collect();
        let sigma: HashMap<AgentId, WorkerId> = (0u32..100)
            .filter_map(|id| net.get_agent(id).map(|_| (id, 0u32)))
            .collect();
        let p0 = build_subnet(&net, &p0_agents, &sigma, &[], 0, 0..100);
        // Ascending iteration pushes [50, 75, 90]; LIFO top is 90
        assert_eq!(p0.free_list, vec![50, 75, 90], "R10a: ascending push order");
        assert_eq!(p0.free_list.last(), Some(&90), "LIFO top must be 90");
    }

    // -----------------------------------------------------------------------
    // TASK-0484: PartitionConfig.sparse_build + DenseAllocationExceedsThreshold
    // (SPEC-22 §3.4 R30)
    // -----------------------------------------------------------------------

    // UT-0484-01: PartitionConfig::default().sparse_build == true.
    #[test]
    fn sparse_build_default_is_true() {
        let cfg = crate::partition::PartitionConfig::default();
        assert!(cfg.sparse_build, "SPEC-22 R30: sparse_build default must be true");
    }

    // UT-0484-02: sparse_build=false, id_range == 4 * live_count (boundary, not exceeded)
    // -> build_subnet_with_config returns Ok.
    #[test]
    fn sparse_build_false_below_threshold_succeeds() {
        use crate::partition::PartitionConfig;

        // 10 live agents, id_range = 0..40 (40 == 4 * 10, boundary, NOT exceeded)
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Era);
        }
        // Wire all principal ports to FreePorts (T1 compliance)
        for i in 0..10u32 {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents: Vec<u32> = (0..10).collect();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig { sparse_build: false };
        let result = build_subnet_with_config(
            &cfg, 0, &net, &agents, &sigma, &[], 0, 0..40,
        );
        assert!(
            result.is_ok(),
            "boundary not exceeded (id_range=40, live=10): must succeed, got {:?}", result
        );
    }

    // UT-0484-03: sparse_build=false, id_range == 5 * live_count (exceeds threshold)
    // -> build_subnet_with_config returns Err(DenseAllocationExceedsThreshold).
    #[test]
    fn sparse_build_false_above_threshold_rejects() {
        use crate::partition::PartitionConfig;
        use crate::error::PartitionError;

        // 10 live agents, id_range = 0..50 (50 > 4 * 10 = 40, exceeds)
        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Era);
        }
        for i in 0..10u32 {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents: Vec<u32> = (0..10).collect();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig { sparse_build: false };
        let result = build_subnet_with_config(
            &cfg, 0, &net, &agents, &sigma, &[], 0, 0..50,
        );
        assert!(
            matches!(
                result,
                Err(PartitionError::DenseAllocationExceedsThreshold { .. })
            ),
            "exceeded threshold (id_range=50, live=10): expected DenseAllocationExceedsThreshold, got {:?}",
            result
        );
    }

    // UT-0484-04: sparse_build=true, threshold exceeded -> Ok (sparse path, no rejection).
    #[test]
    fn sparse_build_true_above_threshold_uses_sparse_path() {
        use crate::partition::PartitionConfig;

        let mut net = Net::new();
        for _ in 0..10 {
            net.create_agent(Symbol::Era);
        }
        for i in 0..10u32 {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents: Vec<u32> = (0..10).collect();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig { sparse_build: true };
        // id_range = 0..50 (50 > 4*10; threshold exceeded) but sparse_build=true -> Ok
        let result = build_subnet_with_config(
            &cfg, 0, &net, &agents, &sigma, &[], 0, 0..50,
        );
        assert!(
            result.is_ok(),
            "sparse_build=true: must not reject above threshold, got {:?}", result
        );
    }

    // UT-0484-05: error fields contain the actual id_range_size and live_count.
    #[test]
    fn error_field_id_range_size_correct() {
        use crate::partition::PartitionConfig;
        use crate::error::PartitionError;

        // id_range = 0..500, live_count = 100 → id_range_size=500, live=100
        let mut net = Net::new();
        for _ in 0..100 {
            net.create_agent(Symbol::Era);
        }
        for i in 0..100u32 {
            let port_idx = i as usize * PORTS_PER_SLOT;
            net.ports[port_idx] = PortRef::FreePort(1_000_000 + i);
        }
        let agents: Vec<u32> = (0..100).collect();
        let sigma: HashMap<AgentId, WorkerId> = agents.iter().map(|&id| (id, 0u32)).collect();

        let cfg = PartitionConfig { sparse_build: false };
        let result = build_subnet_with_config(
            &cfg, 7, &net, &agents, &sigma, &[], 0, 0..500,
        );
        match result {
            Err(PartitionError::DenseAllocationExceedsThreshold {
                partition_index,
                id_range_size,
                live_count,
            }) => {
                assert_eq!(partition_index, 7, "partition_index must match the passed index");
                assert_eq!(id_range_size, 500, "id_range_size must be 500");
                assert_eq!(live_count, 100, "live_count must be 100");
            }
            other => panic!("expected DenseAllocationExceedsThreshold, got {:?}", other),
        }
    }

    // UT-0484-06: error variant is in PartitionError, derives Debug, and matches.
    #[test]
    fn error_variant_in_partition_error_enum() {
        use crate::error::PartitionError;

        let err = PartitionError::DenseAllocationExceedsThreshold {
            partition_index: 0,
            id_range_size: 500,
            live_count: 100,
        };
        // Must be Debug-printable and match the variant
        let s = format!("{:?}", err);
        assert!(
            s.contains("DenseAllocationExceedsThreshold"),
            "error variant must be Debug-printable: {}", s
        );
        assert!(
            matches!(err, PartitionError::DenseAllocationExceedsThreshold { .. }),
            "error variant must match DenseAllocationExceedsThreshold"
        );
    }
}
