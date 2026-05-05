//! MixedNet benchmark (SPEC-09 R16).

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::Net;

/// MixedNet: N pairs of each rule type (SPEC-09 R16).
///
/// Exercises all 6 interaction rules. After reduction, 4N ERA agents remain
/// (from commutation expansions). Profile B.
pub struct MixedNet;

impl Benchmark for MixedNet {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::MixedNet
    }

    fn describe(&self, size: u32) -> String {
        format!("{size} pairs per rule type (all 6 rules)")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::mixed_rules(size)
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
    fn test_mixed_net_reduces() {
        let b = MixedNet;
        let mut net = b.make_net(10);
        let initial = net.count_live_agents();
        reduce_all(&mut net);
        assert!(net.count_live_agents() < initial);
    }

    #[test]
    fn test_mixed_net_verify() {
        let b = MixedNet;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
