//! CascadeCross benchmark (SPEC-09 R18).
//!
//! A left/right chain of CON agents wired so that each CON-CON annihilation
//! at the left/right boundary produces the next active pair on the very next
//! link of the chain. Under lenient BSP (default), the coordinator's
//! reduce_all collapses the whole cascade in a single round. Under strict
//! BSP (SPEC-05 R30a), the cascade is deferred across rounds — one border
//! resolution per round — exposing the multi-round behaviour required by
//! Phase 3 LAN measurements.
//!
//! Topology of cascade_cross(N), N >= 1:
//!
//!   L_0, L_1, ..., L_{N-1}  (ids 0..N-1, assigned to worker 0 with W=2)
//!   R_0, R_1, ..., R_{N-1}  (ids N..2N-1, assigned to worker 1 with W=2)
//!
//! Wires:
//!   L_0.0 <-> R_0.0                    (initial border active pair)
//!   L_k.1 <-> L_{k+1}.0  for k in 0..N-1  (left aux-to-principal chain)
//!   R_k.2 <-> R_{k+1}.0  for k in 0..N-1  (right aux-to-principal chain)
//!
//! All other aux ports bind to distinct FreePorts.
//!
//! CON-CON annihilation rule: link(a.1's neighbor, b.2's neighbor) and
//! link(a.2's neighbor, b.1's neighbor). After the L_k-R_k annihilation,
//! the first link connects L_{k+1}.0 with R_{k+1}.0, creating the next
//! principal-principal active pair.
//!
//! Profile: B (expansion with collapse). Initial agents: 2N. Initial
//! redexes: 1. Rounds (lenient): 1. Rounds (strict, W=2, expected): N.
//! Total interactions to Normal Form: N (each reduces one CON-CON pair).

use crate::bench::{Benchmark, BenchmarkId};
use crate::net::{Net, PortRef, Symbol};

/// Builds the cascade_cross(N) net described above.
///
/// N == 0 returns an empty net (no agents, no redexes).
pub fn build_cascade_cross(n: u32) -> Net {
    let mut net = Net::new();
    if n == 0 {
        return net;
    }
    let n_us = n as usize;

    // Create left and right chains in order so ids are contiguous:
    // L_0..L_{N-1} get ids 0..N-1, R_0..R_{N-1} get ids N..2N-1.
    let mut left = Vec::with_capacity(n_us);
    for _ in 0..n {
        left.push(net.create_agent(Symbol::Con));
    }
    let mut right = Vec::with_capacity(n_us);
    for _ in 0..n {
        right.push(net.create_agent(Symbol::Con));
    }

    // Initial border active pair: L_0.0 <-> R_0.0
    net.connect(
        PortRef::AgentPort(left[0], 0),
        PortRef::AgentPort(right[0], 0),
    );

    // Fresh free-port id counter for terminal aux connections.
    let mut free_id: u32 = 0;
    let mut next_free = || {
        let fid = free_id;
        free_id += 1;
        PortRef::FreePort(fid)
    };

    // Left chain: L_k.1 <-> L_{k+1}.0 ; L_k.2 <-> FreePort
    for k in 0..n_us {
        if k + 1 < n_us {
            net.connect(
                PortRef::AgentPort(left[k], 1),
                PortRef::AgentPort(left[k + 1], 0),
            );
        } else {
            // Last left agent: no next link, aux 1 free.
            net.connect(PortRef::AgentPort(left[k], 1), next_free());
        }
        net.connect(PortRef::AgentPort(left[k], 2), next_free());
    }

    // Right chain: R_k.2 <-> R_{k+1}.0 ; R_k.1 <-> FreePort
    for k in 0..n_us {
        net.connect(PortRef::AgentPort(right[k], 1), next_free());
        if k + 1 < n_us {
            net.connect(
                PortRef::AgentPort(right[k], 2),
                PortRef::AgentPort(right[k + 1], 0),
            );
        } else {
            net.connect(PortRef::AgentPort(right[k], 2), next_free());
        }
    }

    net
}

/// CascadeCross: deferred-cascade CON chain (SPEC-09 R18).
///
/// Size parameter is the cascade length N. Produces 2N CON agents.
/// Profile B. See module docs for the full topology.
pub struct CascadeCross;

impl Benchmark for CascadeCross {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::CascadeCross
    }

    fn describe(&self, size: u32) -> String {
        format!("cascade_cross(N={size}): {} CON agents, 1 initial redex, {} annihilations to NF", 2 * size, size)
    }

    fn make_net(&self, size: u32) -> Net {
        build_cascade_cross(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![10, 50, 100, 500, 1_000]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        // Cascade_cross is fully collapsing: every CON is annihilated.
        // Correctness check is exact (sizes are small enough for G1).
        seq.count_live_agents() == 0 && dist.count_live_agents() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::{run_grid, GridConfig};
    use crate::partition::ContiguousIdStrategy;
    use crate::reduction::reduce_all;

    // T-cc-1: make_net(5) produces the expected topology.
    // 2N = 10 live agents, exactly 1 initial redex, all CON.
    #[test]
    fn test_cc_make_net_topology() {
        let b = CascadeCross;
        let net = b.make_net(5);
        assert_eq!(net.count_live_agents(), 10, "expected 2N = 10 live agents");
        assert_eq!(
            net.redex_queue.len(),
            1,
            "expected exactly 1 initial redex"
        );
        // The redex must be principal-principal and cross-chain.
        let (a_id, b_id) = net.redex_queue[0];
        // ids 0 and 5 (L_0 and R_0 under a size-5 cascade).
        assert!(
            (a_id == 0 && b_id == 5) || (a_id == 5 && b_id == 0),
            "initial redex should be (L_0, R_0) = (0, 5), got ({}, {})",
            a_id,
            b_id
        );
    }

    // T-cc-2: sequential reduce_all drives cascade_cross(10) to Normal Form.
    // All 20 CON agents should annihilate (10 CON-CON annihilations total).
    #[test]
    fn test_cc_sequential_reduces_to_empty() {
        let b = CascadeCross;
        let mut net = b.make_net(10);
        assert_eq!(net.count_live_agents(), 20);
        let stats = reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
        assert_eq!(
            stats.total_interactions, 10,
            "cascade_cross(10) should perform exactly 10 annihilations"
        );
    }

    // T-cc-3: verify() accepts two independently-reduced copies as equal.
    #[test]
    fn test_cc_verify_pass() {
        let b = CascadeCross;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }

    // T-cc-4: default sizes include the small-size smoke set.
    #[test]
    fn test_cc_default_sizes() {
        let sizes = CascadeCross.default_sizes();
        assert!(sizes.contains(&10));
        assert!(sizes.contains(&50));
        assert!(sizes.contains(&100));
    }

    // T-cc-5 (RED until F3): cascade_cross(N) under strict_bsp with W=2
    // should require >= 2 rounds — one per cascading annihilation. Under
    // the current lenient implementation, reduce_all at the coordinator
    // collapses the whole cascade in round 1, so this test fails until
    // the strict branch is wired into the grid loop.
    #[test]
    fn test_cc_strict_multi_round() {
        let b = CascadeCross;
        let net = b.make_net(5);

        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert!(
            metrics.rounds >= 2,
            "expected >= 2 rounds for cascade_cross(5) under strict_bsp, got {}",
            metrics.rounds
        );
    }
}
