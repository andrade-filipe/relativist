//! DualTree benchmark (SPEC-09 R13).

use crate::bench::streaming::dual_tree_stream;
use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::sparse::SparseNet;
use crate::net::Net;
use crate::partition::streaming::AgentBatch;

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

    /// Native streaming override (SPEC-21 R12 SHOULD, R14 forward references).
    ///
    /// Emits agents in bottom-up BFS order (leaves first). Child→parent wires
    /// use `Pending` directives when the parent hasn't been emitted yet.
    fn make_net_stream(
        &self,
        size: u32,
        chunk_size: usize,
    ) -> Box<dyn Iterator<Item = AgentBatch>> {
        dual_tree_stream(size, chunk_size)
    }

    /// Native sparse-construction override (D-011 Phase D-1, TASK-0606).
    ///
    /// `dual_tree` is the only benchmark in the v2 suite that supports the
    /// SPEC-22 R12 sparse-net path (per the D-011 plan §D-1 scope). Building
    /// directly into a `SparseNet` lets the harness sample VmHWM after the
    /// recursive build but before any dense-arena allocation, so the
    /// construction-phase memory peak (SPEC-09 R18a) reflects the sparse
    /// representation only.
    ///
    /// SPEC-09 R37c construction-isomorphism: the returned `SparseNet`, after
    /// `to_dense(None)`, is graph-isomorphic to `make_net(size)` for every
    /// `size`. UT-0606-01 in `io::generators::tests` enforces this.
    fn make_sparse_net(&self, size: u32) -> Result<SparseNet, String> {
        Ok(generators::dual_tree_sparse(size))
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
