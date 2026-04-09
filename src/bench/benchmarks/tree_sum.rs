//! TreeSum benchmark (SPEC-09 R14-R15).

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::Net;

/// TreeSum: sum of N ones via Church addition (SPEC-09 R14).
///
/// Builds add(1, N-1) which reduces to Church(N).
/// Exercises the encoding layer with increasing problem sizes.
pub struct TreeSum;

impl Benchmark for TreeSum {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::TreeSum
    }

    fn describe(&self, size: u32) -> String {
        format!("tree sum of {size} ones")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::tree_sum(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![10, 50, 100, 500, 1_000]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        nets_isomorphic(seq, dist)
    }
}

/// TreeSumBalanced: balanced binary tree sum (SPEC-09 R15).
///
/// Same result as TreeSum but with a balanced reduction tree,
/// providing better parallelism opportunities.
pub struct TreeSumBalanced;

impl Benchmark for TreeSumBalanced {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::TreeSumBalanced
    }

    fn describe(&self, size: u32) -> String {
        format!("balanced tree sum of {size} ones")
    }

    fn make_net(&self, size: u32) -> Net {
        // For now, same as TreeSum (balanced tree construction
        // requires composing multiple add nets which is deferred)
        generators::tree_sum(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![10, 50, 100, 500, 1_000]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        nets_isomorphic(seq, dist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{decode_nat, discover_root};
    use crate::reduction::reduce_all;

    #[test]
    fn test_tree_sum_correctness() {
        let b = TreeSum;
        let mut net = b.make_net(10);
        reduce_all(&mut net);
        discover_root(&mut net);
        assert_eq!(decode_nat(&net), Some(10));
    }

    #[test]
    fn test_tree_sum_verify() {
        let b = TreeSum;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        discover_root(&mut seq);
        discover_root(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
