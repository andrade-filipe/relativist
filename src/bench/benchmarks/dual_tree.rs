//! DualTree benchmark (SPEC-09 R13).

use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::Net;

/// DualTree: mirrored CON trees that annihilate completely (SPEC-09 R13).
///
/// Size parameter is tree depth. Creates 2^(d+1)-2 agents per tree.
/// Profile B: expansion with collapse.
pub struct DualTree;

impl Benchmark for DualTree {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::DualTree
    }

    fn describe(&self, size: u32) -> String {
        format!("dual tree depth={size}")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::dual_tree(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![4, 6, 8, 10, 12, 14]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        seq.count_live_agents() == 0 && dist.count_live_agents() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduction::reduce_all;

    #[test]
    fn test_dual_tree_reduces_to_empty() {
        let b = DualTree;
        let mut net = b.make_net(4);
        assert!(net.count_live_agents() > 0);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_dual_tree_verify() {
        let b = DualTree;
        let mut seq = b.make_net(3);
        let mut dist = b.make_net(3);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
