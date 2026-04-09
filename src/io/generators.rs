//! Pre-built example net generators (SPEC-12 R32-R50, SPEC-09).
//!
//! Each generator creates a parametric IC net for benchmarking or
//! testing. Size parameter `n` controls the number of agents/pairs.

use crate::net::{Net, PortRef, Symbol};

/// Available example nets, matching benchmark profiles from SPEC-09.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExampleNet {
    /// N ERA-ERA annihilation pairs (Profile A). SPEC-09 R9.
    EpAnnihilation,
    /// N CON-CON annihilation pairs (Profile A). SPEC-09 R10.
    EpAnnihilationCon,
    /// N DUP-DUP annihilation pairs (Profile A). SPEC-09 R10.
    EpAnnihilationDup,
    /// N CON-DUP commutation pairs (Profile B). SPEC-09 R11.
    ConDupExpansion,
    /// Dual tree of depth D (Profile B/C). SPEC-09 R12.
    DualTree,
    /// Mixed net: ERA-ERA + CON-CON + CON-DUP in thirds (Profile C). SPEC-09 R14.
    MixedRules,
    /// Chain of N CON agents with ERA at head (Profile C). SPEC-09 R16a.
    ErasurePropagation,
    /// N items summed via Church add in a left-fold chain. SPEC-09 R14.
    TreeSum,
}

/// Generate the specified example net with the given size parameter.
pub fn generate(example: ExampleNet, size: u32) -> Net {
    match example {
        ExampleNet::EpAnnihilation => ep_annihilation(size),
        ExampleNet::EpAnnihilationCon => ep_annihilation_con(size),
        ExampleNet::EpAnnihilationDup => ep_annihilation_dup(size),
        ExampleNet::ConDupExpansion => con_dup_expansion(size),
        ExampleNet::DualTree => dual_tree(size),
        ExampleNet::MixedRules => mixed_rules(size),
        ExampleNet::ErasurePropagation => erasure_propagation(size),
        ExampleNet::TreeSum => tree_sum(size),
    }
}

/// N ERA-ERA annihilation pairs (TASK-0171).
///
/// Creates N pairs of ERA agents connected at principal ports.
/// Each pair annihilates in one step (void rule), yielding an empty net.
/// Total agents: 2N, total interactions: N.
pub fn ep_annihilation(n: u32) -> Net {
    let mut net = Net::new();
    for _ in 0..n {
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
    }
    net
}

/// N CON-CON annihilation pairs (SPEC-12 R38).
///
/// Creates N pairs of CON agents connected principal-to-principal,
/// with auxiliary ports connected to free ports.
/// After reduction: 0 CON agents, free ports cross-reconnected.
pub fn ep_annihilation_con(n: u32) -> Net {
    let mut net = Net::new();
    let mut free_id = 0u32;
    for _ in 0..n {
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(free_id));
        free_id += 1;
    }
    net
}

/// N DUP-DUP annihilation pairs (SPEC-12 R38a).
///
/// Creates N pairs of DUP agents connected principal-to-principal,
/// with auxiliary ports connected to free ports.
/// After reduction: 0 DUP agents, free ports reconnected in parallel pattern.
pub fn ep_annihilation_dup(n: u32) -> Net {
    let mut net = Net::new();
    let mut free_id = 0u32;
    for _ in 0..n {
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(free_id));
        free_id += 1;
    }
    net
}

/// N CON-DUP commutation pairs (TASK-0173).
///
/// Creates N independent CON-DUP pairs connected at principal ports,
/// with auxiliary ports connected to free ports.
/// Each pair triggers commutation (expansion), creating 4 new agents.
pub fn con_dup_expansion(n: u32) -> Net {
    let mut net = Net::new();
    let mut free_id = 0u32;
    for _ in 0..n {
        let c = net.create_agent(Symbol::Con);
        let d = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        net.connect(PortRef::AgentPort(c, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(d, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(d, 2), PortRef::FreePort(free_id));
        free_id += 1;
    }
    net
}

/// Dual tree of depth D (TASK-0174).
///
/// Two mirrored binary trees of CON agents, connected principal-to-principal
/// at the roots. Leaves connect to free ports. Reduction triggers cascading
/// annihilation from root to leaves.
/// Agents: 2*(2^D - 1), depth D.
pub fn dual_tree(depth: u32) -> Net {
    let mut net = Net::new();
    let mut free_id = 0u32;

    fn build_tree(net: &mut Net, depth: u32, free_id: &mut u32) -> PortRef {
        if depth == 0 {
            let fp = PortRef::FreePort(*free_id);
            *free_id += 1;
            return fp;
        }
        let node = net.create_agent(Symbol::Con);
        let left = build_tree(net, depth - 1, free_id);
        let right = build_tree(net, depth - 1, free_id);
        net.connect(PortRef::AgentPort(node, 1), left);
        net.connect(PortRef::AgentPort(node, 2), right);
        PortRef::AgentPort(node, 0)
    }

    let root_a = build_tree(&mut net, depth, &mut free_id);
    let root_b = build_tree(&mut net, depth, &mut free_id);
    net.connect(root_a, root_b);

    net
}

/// Mixed-rule net: N pairs of each of the 6 interaction rule types (SPEC-12 R41).
///
/// For each iteration (0..N), creates one pair of each type:
///   1. ERA-ERA (void/annihilation)
///   2. CON-CON (annihilation)
///   3. DUP-DUP (annihilation)
///   4. CON-DUP (commutation)
///   5. CON-ERA (erasure)
///   6. DUP-ERA (erasure)
///
/// Total: 6N initial redex pairs. All auxiliary ports connect to fresh free ports,
/// ensuring no cross-pair interactions before the initial redexes are resolved.
pub fn mixed_rules(n: u32) -> Net {
    let mut net = Net::new();
    let mut free_id = 0u32;

    for _ in 0..n {
        // 1. ERA-ERA
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        // 2. CON-CON
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        for agent in [a, b] {
            for port in [1u8, 2] {
                net.connect(PortRef::AgentPort(agent, port), PortRef::FreePort(free_id));
                free_id += 1;
            }
        }

        // 3. DUP-DUP
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        for agent in [a, b] {
            for port in [1u8, 2] {
                net.connect(PortRef::AgentPort(agent, port), PortRef::FreePort(free_id));
                free_id += 1;
            }
        }

        // 4. CON-DUP
        let c = net.create_agent(Symbol::Con);
        let d = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        for agent in [c, d] {
            for port in [1u8, 2] {
                net.connect(PortRef::AgentPort(agent, port), PortRef::FreePort(free_id));
                free_id += 1;
            }
        }

        // 5. CON-ERA
        let c = net.create_agent(Symbol::Con);
        let e = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(e, 0));
        for port in [1u8, 2] {
            net.connect(PortRef::AgentPort(c, port), PortRef::FreePort(free_id));
            free_id += 1;
        }

        // 6. DUP-ERA
        let d = net.create_agent(Symbol::Dup);
        let e = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(d, 0), PortRef::AgentPort(e, 0));
        for port in [1u8, 2] {
            net.connect(PortRef::AgentPort(d, port), PortRef::FreePort(free_id));
            free_id += 1;
        }
    }

    net
}

/// Erasure propagation chain: ERA connected to head of N CON chain (SPEC-09 R16a).
///
/// Creates a chain of N CON agents connected through auxiliary ports,
/// with an ERA agent connected to the head CON's principal port.
/// The ERA propagates through the chain, erasing each CON and creating
/// 2 new ERA agents per step (sequential dependency, Profile C).
///
/// Total agents: N CON + 1 ERA = N+1. After reduction: 2N ERA agents remain.
pub fn erasure_propagation(n: u32) -> Net {
    let mut net = Net::new();
    if n == 0 {
        return net;
    }

    let mut free_id = 0u32;

    // Create CON chain
    let cons: Vec<_> = (0..n).map(|_| net.create_agent(Symbol::Con)).collect();

    // Chain CONs: each CON.p1 -> next CON.p0 (principal)
    for i in 0..cons.len() - 1 {
        net.connect(
            PortRef::AgentPort(cons[i], 1),
            PortRef::AgentPort(cons[i + 1], 0),
        );
    }

    // Last CON.p1 -> free port (chain tail)
    net.connect(
        PortRef::AgentPort(cons[cons.len() - 1], 1),
        PortRef::FreePort(free_id),
    );
    free_id += 1;

    // Each CON.p2 -> free port (unused auxiliary)
    for &c in &cons {
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(free_id));
        free_id += 1;
    }

    // ERA connected to head CON's principal port (creates the initial redex)
    let era = net.create_agent(Symbol::Era);
    net.connect(PortRef::AgentPort(era, 0), PortRef::AgentPort(cons[0], 0));

    net
}

/// Tree sum: N Church(1) values summed via left-fold addition (SPEC-09 R14).
///
/// Builds: add(add(add(Church(1), Church(1)), Church(1)), ..., Church(1))
/// Result after reduction: Church(N).
pub fn tree_sum(n: u32) -> Net {
    use crate::encoding::build_add;

    if n == 0 {
        return crate::encoding::encode_nat(0);
    }
    if n == 1 {
        return crate::encoding::encode_nat(1);
    }

    // Build left-fold: add(1, 1) then add(result, 1) for each additional item
    // Using build_add which creates a complete net for each addition
    build_add(1, (n - 1) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reduction::reduce_all;

    #[test]
    fn test_ep_annihilation_basic() {
        let net = ep_annihilation(10);
        assert_eq!(net.count_live_agents(), 20);
        assert_eq!(net.redex_queue.len(), 10);
    }

    #[test]
    fn test_ep_annihilation_reduces_to_empty() {
        let mut net = ep_annihilation(10);
        let stats = reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
        assert_eq!(stats.total_interactions, 10);
    }

    #[test]
    fn test_ep_annihilation_con_reduces_to_empty() {
        let mut net = ep_annihilation_con(5);
        assert_eq!(net.count_live_agents(), 10);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_ep_annihilation_dup_reduces_to_empty() {
        let mut net = ep_annihilation_dup(5);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_con_dup_expansion() {
        let net = con_dup_expansion(5);
        assert_eq!(net.count_live_agents(), 10); // 5 pairs of 2
        assert_eq!(net.redex_queue.len(), 5);
    }

    #[test]
    fn test_con_dup_expansion_reduces() {
        let mut net = con_dup_expansion(3);
        let stats = reduce_all(&mut net);
        assert!(stats.total_interactions > 0);
        // Each CON-DUP commutation produces 4 agents, which may annihilate
        assert!(net.count_live_agents() > 0);
    }

    #[test]
    fn test_dual_tree_depth_0() {
        let net = dual_tree(0);
        // Depth 0: no agents, just two free ports connected
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_dual_tree_depth_1() {
        let net = dual_tree(1);
        // 2 CON agents at roots, connected principal-principal
        assert_eq!(net.count_live_agents(), 2);
        assert_eq!(net.redex_queue.len(), 1);
    }

    #[test]
    fn test_dual_tree_depth_3() {
        let net = dual_tree(3);
        // 2*(2^3 - 1) = 14 agents
        assert_eq!(net.count_live_agents(), 14);
    }

    #[test]
    fn test_dual_tree_reduces_to_empty() {
        let mut net = dual_tree(3);
        reduce_all(&mut net);
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_mixed_rules() {
        let net = mixed_rules(3);
        // Per iteration: ERA-ERA(2) + CON-CON(2) + DUP-DUP(2) + CON-DUP(2) + CON-ERA(2) + DUP-ERA(2) = 12 agents
        // 3 iterations = 36 agents, 18 redex pairs (6 per iteration)
        assert_eq!(net.count_live_agents(), 36);
        assert_eq!(net.redex_queue.len(), 18);
    }

    #[test]
    fn test_mixed_rules_reduces() {
        let mut net = mixed_rules(3);
        let stats = reduce_all(&mut net);
        // At least ERA-ERA + CON-CON + DUP-DUP annihilate (3*3=9), plus erasure rules
        assert!(stats.total_interactions >= 9);
    }

    #[test]
    fn test_generate_dispatch() {
        let net = generate(ExampleNet::EpAnnihilation, 5);
        assert_eq!(net.count_live_agents(), 10);

        let net = generate(ExampleNet::DualTree, 2);
        assert_eq!(net.count_live_agents(), 6);
    }

    #[test]
    fn test_ep_annihilation_zero() {
        let net = ep_annihilation(0);
        assert_eq!(net.count_live_agents(), 0);
    }
}
