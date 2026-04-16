//! ChurchSumOfSquares benchmark (SPEC-09 R17d, demonstrative).
//!
//! Computes `sum_{i=1..N} i^2` by composing `wire_mul_into(Church(i), Church(i))`
//! into a right-associated `wire_add_into` chain. Verified against the
//! Archimedes/Faulhaber closed form `N*(N+1)*(2N+1)/6`.
//!
//! This benchmark is **demonstrative, not comparative**: it exists so the TCC
//! article/defense can exhibit the Relativist grid executing a recognizable
//! arithmetic computation end-to-end. It is explicitly NOT part of the frozen
//! performance campaigns (v1_local_baseline, v1_stress). See USAGE_GUIDE.md
//! Section 11.8 for reproduction instructions.

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::encoding::{build_sum_of_squares, decode_nat_or_shared};
use crate::net::Net;

/// ChurchSumOfSquares: sum of squares via composed mul/add (SPEC-09 R17d).
///
/// Size `N` parameterizes the upper bound of the sum: the net encodes
/// `1^2 + 2^2 + ... + N^2` as a right-associated chain of `add` applied
/// to `mul(Ch(i), Ch(i))` sub-computations.
pub struct ChurchSumOfSquares;

impl Benchmark for ChurchSumOfSquares {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::ChurchSumOfSquares
    }

    fn describe(&self, size: u32) -> String {
        let n = size as u64;
        let expected = n * (n + 1) * (2 * n + 1) / 6;
        format!("Sum of squares 1..{n}^2 = {expected}")
    }

    fn make_net(&self, size: u32) -> Net {
        build_sum_of_squares(size as u64)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![5, 10, 30, 50, 100]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        // Primary check: value equivalence via decode. This is the whole point
        // of this benchmark — both sides must decode to the same natural number.
        let v_seq = decode_nat_or_shared(seq);
        let v_dist = decode_nat_or_shared(dist);
        if let (Some(a), Some(b)) = (v_seq, v_dist) {
            if a == b {
                return true;
            }
        }
        // Fallback: structural isomorphism. If the decoders fail for any reason
        // (e.g. shared-chain walk stops at a pattern not yet covered), we still
        // want the suite to accept structurally identical results.
        nets_isomorphic(seq, dist)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::{run_grid, GridConfig};
    use crate::partition::ContiguousIdStrategy;
    use crate::reduction::reduce_all;

    #[test]
    fn test_church_sum_of_squares_correctness() {
        let b = ChurchSumOfSquares;
        let mut net = b.make_net(10);
        reduce_all(&mut net);
        assert_eq!(decode_nat_or_shared(&net), Some(385));
    }

    #[test]
    fn test_church_sum_of_squares_verify_sequential() {
        let b = ChurchSumOfSquares;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }

    #[test]
    fn test_church_sum_of_squares_grid_equivalence() {
        let b = ChurchSumOfSquares;
        let net = b.make_net(10);

        // Sequential reference
        let mut seq = net.clone();
        reduce_all(&mut seq);
        assert_eq!(decode_nat_or_shared(&seq), Some(385));

        // Distributed (in-process grid, 4 workers)
        let config = GridConfig {
            num_workers: 4,
            max_rounds: None,
            strict_bsp: false,
        };
        let strategy = ContiguousIdStrategy;
        let (dist, _metrics) = run_grid(net, &config, &strategy);
        assert_eq!(decode_nat_or_shared(&dist), Some(385));

        // Verifier accepts the pair
        assert!(b.verify(&seq, &dist));
    }

    #[test]
    fn test_church_sum_of_squares_describe() {
        let b = ChurchSumOfSquares;
        assert_eq!(b.describe(10), "Sum of squares 1..10^2 = 385");
        assert_eq!(b.describe(5), "Sum of squares 1..5^2 = 55");
    }
}
