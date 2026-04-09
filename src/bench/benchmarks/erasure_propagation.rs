//! ErasurePropagation benchmark (SPEC-09 R16a).

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::Net;

/// ErasurePropagation: ERA propagates through N-CON chain (SPEC-09 R16a).
///
/// Sequential dependency (Profile C): each step depends on the previous.
/// After reduction, 2N ERA agents remain from the erasure cascade.
pub struct ErasurePropagation;

impl Benchmark for ErasurePropagation {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::ErasurePropagation
    }

    fn describe(&self, size: u32) -> String {
        format!("erasure propagation chain length={size}")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::erasure_propagation(size)
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
    fn test_erasure_propagation_reduces() {
        let b = ErasurePropagation;
        let mut net = b.make_net(10);
        assert_eq!(net.count_live_agents(), 11); // 10 CON + 1 ERA
        reduce_all(&mut net);
        // After reduction, ERA agents remain from cascade
        assert!(net.count_live_agents() > 0);
        assert!(net.redex_queue.is_empty());
    }

    #[test]
    fn test_erasure_propagation_verify() {
        let b = ErasurePropagation;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
