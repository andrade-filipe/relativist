//! ChurchMul benchmark (SPEC-09 R17b).

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::encoding::build_mul;
use crate::net::Net;

/// ChurchMul: Church numeral multiplication (SPEC-09 R17b, SHOULD).
///
/// Size N is factored as sqrt(N) * sqrt(N). Produces non-canonical
/// Church numerals with DUP sharing after optimal reduction.
pub struct ChurchMul;

impl Benchmark for ChurchMul {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::ChurchMul
    }

    fn describe(&self, size: u32) -> String {
        let s = (size as f64).sqrt() as u64;
        format!("Church mul({s}, {s}) = {}", s * s)
    }

    fn make_net(&self, size: u32) -> Net {
        let s = (size as f64).sqrt() as u64;
        build_mul(s, s)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![4, 9, 16, 25, 100]
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
    fn test_church_mul_reduces() {
        let b = ChurchMul;
        let mut net = b.make_net(9); // mul(3,3)
        reduce_all(&mut net);
        assert!(net.redex_queue.is_empty());
    }

    #[test]
    fn test_church_mul_verify() {
        let b = ChurchMul;
        let mut seq = b.make_net(4); // mul(2,2)
        let mut dist = b.make_net(4);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
