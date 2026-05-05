//! ChurchAdd benchmark (SPEC-09 R17a).

use crate::bench::isomorphism::nets_isomorphic;
use crate::bench::{Benchmark, BenchmarkId};
use crate::encoding::build_add;
use crate::net::Net;

/// ChurchAdd: Church numeral addition (SPEC-09 R17a).
///
/// Size N is split as N/2 + ceil(N/2). After reduction, the result
/// is Church(N). Exercises the encoding layer and lambda calculus reduction.
pub struct ChurchAdd;

impl Benchmark for ChurchAdd {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::ChurchAdd
    }

    fn describe(&self, size: u32) -> String {
        let a = size as u64 / 2;
        let b = size as u64 - a;
        format!("Church add({a}, {b}) = {size}")
    }

    fn make_net(&self, size: u32) -> Net {
        let a = size as u64 / 2;
        let b = size as u64 - a;
        build_add(a, b)
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
    fn test_church_add_correctness() {
        let b = ChurchAdd;
        let mut net = b.make_net(10);
        reduce_all(&mut net);
        discover_root(&mut net);
        assert_eq!(decode_nat(&net), Some(10));
    }

    #[test]
    fn test_church_add_verify() {
        let b = ChurchAdd;
        let mut seq = b.make_net(6);
        let mut dist = b.make_net(6);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        discover_root(&mut seq);
        discover_root(&mut dist);
        assert!(b.verify(&seq, &dist));
    }
}
