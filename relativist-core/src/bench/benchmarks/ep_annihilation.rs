//! EP-Annihilation benchmarks: ERA, CON, DUP (SPEC-09 R9-R11).

use crate::bench::{Benchmark, BenchmarkId};
use crate::io::generators;
use crate::net::Net;

/// EP-Annihilation ERA: N ERA-ERA pairs, Profile A (SPEC-09 R9).
pub struct EPAnnihilation;

impl Benchmark for EPAnnihilation {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::EPAnnihilation
    }

    fn describe(&self, size: u32) -> String {
        format!("{size} ERA-ERA pairs (void annihilation)")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::ep_annihilation(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![100, 500, 1_000, 5_000, 10_000, 50_000, 100_000]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        seq.count_live_agents() == 0 && dist.count_live_agents() == 0
    }
}

/// EP-Annihilation CON: N CON-CON pairs, Profile A (SPEC-09 R10).
pub struct EPAnnihilationCon;

impl Benchmark for EPAnnihilationCon {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::EPAnnihilationCon
    }

    fn describe(&self, size: u32) -> String {
        format!("{size} CON-CON pairs (cross annihilation)")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::ep_annihilation_con(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![100, 500, 1_000, 5_000, 10_000, 50_000, 100_000]
    }

    fn verify(&self, seq: &Net, dist: &Net) -> bool {
        seq.count_live_agents() == 0 && dist.count_live_agents() == 0
    }
}

/// EP-Annihilation DUP: N DUP-DUP pairs, Profile A (SPEC-09 R11).
pub struct EPAnnihilationDup;

impl Benchmark for EPAnnihilationDup {
    fn id(&self) -> BenchmarkId {
        BenchmarkId::EPAnnihilationDup
    }

    fn describe(&self, size: u32) -> String {
        format!("{size} DUP-DUP pairs (parallel annihilation)")
    }

    fn make_net(&self, size: u32) -> Net {
        generators::ep_annihilation_dup(size)
    }

    fn default_sizes(&self) -> Vec<u32> {
        vec![100, 500, 1_000, 5_000, 10_000, 50_000, 100_000]
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
    fn test_ep_annihilation_era() {
        let b = EPAnnihilation;
        let mut net = b.make_net(10);
        assert_eq!(net.count_live_agents(), 20);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_ep_annihilation_con() {
        let b = EPAnnihilationCon;
        let mut net = b.make_net(10);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_ep_annihilation_dup() {
        let b = EPAnnihilationDup;
        let mut net = b.make_net(10);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_ep_verify() {
        let b = EPAnnihilation;
        let mut seq = b.make_net(5);
        let mut dist = b.make_net(5);
        reduce_all(&mut seq);
        reduce_all(&mut dist);
        assert!(b.verify(&seq, &dist));
    }

    #[test]
    fn test_ep_default_sizes_include_haskell() {
        let sizes = EPAnnihilation.default_sizes();
        assert!(sizes.contains(&100));
        assert!(sizes.contains(&500));
        assert!(sizes.contains(&1000));
    }
}
