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
/// The `agents` and `ports` Vecs are sized to `max_id + 1` (preserving
/// the original ID indexing scheme). Unused slots are None/DISCONNECTED.
pub fn build_subnet(
    net: &Net,
    worker_agents: &[AgentId],
    sigma: &HashMap<AgentId, WorkerId>,
    border_entries: &[(AgentId, PortId, u32)],
    worker_id: WorkerId,
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

    Net {
        agents,
        ports,
        redex_queue,
        next_id: 0, // Caller sets this based on ID range
        root: None, // Caller sets this based on R28
        freeport_redirects: std::collections::HashMap::new(),
    }
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
        let subnet = build_subnet(&net, &[], &sigma, &[], 0);
        assert_eq!(subnet.agents.len(), 0);
    }

    // S2: Single agent, no borders
    #[test]
    fn test_build_subnet_single_agent() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(5));

        let sigma = make_sigma(&[(a, 0)]);
        let subnet = build_subnet(&net, &[a], &sigma, &[], 0);

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
        let subnet = build_subnet(&net, &[a, b], &sigma, &[], 0);

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
        let subnet = build_subnet(&net, &[a], &sigma, &border_entries_w0, 0);

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
        let subnet = build_subnet(&net, &[a, c], &sigma, &[], 0);

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
        let subnet = build_subnet(&net, &[a, b, c], &sigma, &[(c, 0, 100)], 0);

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
        let subnet = build_subnet(&net, &[a], &sigma, &[], 0);

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
}
