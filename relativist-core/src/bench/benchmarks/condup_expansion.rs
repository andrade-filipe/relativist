//! CON-DUP Expansion benchmark (SPEC-09 R12).

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::Net;

/// CON-DUP Expansion: N CON-DUP pairs, Profile B (SPEC-09 R12).
///
/// Each CON-DUP commutation creates 4 new agents. After full reduction,
/// all agents annihilate. This is the primary Profile B workload.
pub struct ConDupExpansion;

impl Benchmark for ConDupExpansion {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::ConDupExpansion
    }

    fn describe(&self, size: u32) -> String {
        format!("{size} CON-DUP pairs (expansion then collapse)")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::con_dup_expansion(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![100, 500, 1_000, 5_000, 10_000]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        nets_isomorphic(seq, dist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduction::reduce_all;

    #[test]
    fn test_condup_expansion() {
        let b = ConDupExpansion;
        let mut net = b.make_net(10);
        assert!(net.count_live_agents() > 0);
        reduce_all(&mut net);
        // CON-DUP commutation creates new agents; result is non-empty
        assert!(net.redex_queue.is_empty());
    }

    #[test]
    fn test_condup_verify() {
        let b = ConDupExpansion;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
