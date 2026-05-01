//! Graph isomorphism checker for IC nets (SPEC-08 R23-R24, SPEC-09 R4).
//!
//! Verifies that two nets are structurally equal modulo AgentId renaming.
//! Used by the benchmark framework to verify the Fundamental Property (G1)
//! on every datapoint.

use crate::net::{total_ports, AgentId, Net, PortRef, Symbol};
use std::collections::HashMap;

/// Verifies whether two nets are isomorphic: structurally equal modulo
/// AgentId renaming.
///
/// Two nets are isomorphic if there exists a bijection f: AgentId_a -> AgentId_b
/// such that:
/// - For every live agent id in a, f(id) is live in b with the same symbol.
/// - For every port connection in a, the mapped connection exists in b.
/// - FreePort references are preserved without renaming.
pub fn nets_isomorphic(a: &Net, b: &Net) -> bool {
    // Quick reject: count agents per symbol
    let counts_a = count_agents_by_symbol(a);
    let counts_b = count_agents_by_symbol(b);
    if counts_a != counts_b {
        return false;
    }

    // Both empty -> isomorphic
    if counts_a.is_empty() {
        return true;
    }

    // Group agents by symbol for candidate matching
    let groups_b = group_agents_by_symbol(b);

    // Collect all live agent IDs in a, sorted by symbol for deterministic ordering
    // (agents with fewer candidates first to prune early)
    let mut agents_a: Vec<AgentId> = a.live_agents().map(|ag| ag.id).collect();
    agents_a.sort_by_key(|&id| {
        let sym = a.get_agent(id).unwrap().symbol;
        let candidate_count = groups_b.get(&sym).map_or(0, |v| v.len());
        (candidate_count, id)
    });

    let mut mapping: HashMap<AgentId, AgentId> = HashMap::new();
    let mut reverse: HashMap<AgentId, AgentId> = HashMap::new();

    backtrack(a, b, &agents_a, 0, &mut mapping, &mut reverse, &groups_b)
}

/// Iterative backtracking search for a valid bijection.
///
/// Uses an explicit stack instead of recursion to avoid stack overflow
/// on large nets (>700 agents hit the default 1MB Windows stack limit).
fn backtrack(
    a: &Net,
    b: &Net,
    agents_a: &[AgentId],
    start_index: usize,
    mapping: &mut HashMap<AgentId, AgentId>,
    reverse: &mut HashMap<AgentId, AgentId>,
    groups_b: &HashMap<Symbol, Vec<AgentId>>,
) -> bool {
    // Each frame tracks: which agent index we're matching, and which
    // candidate index within that agent's candidate list we'll try next.
    let mut stack: Vec<usize> = Vec::with_capacity(agents_a.len() - start_index);
    let mut index = start_index;

    loop {
        if index == agents_a.len() {
            return true; // Complete bijection found
        }

        let id_a = agents_a[index];
        let sym = a.get_agent(id_a).unwrap().symbol;
        let candidates = match groups_b.get(&sym) {
            Some(c) => c,
            None => {
                // Backtrack
                if let Some(cand_idx) = stack.pop() {
                    index -= 1;
                    let prev_a = agents_a[index];
                    let prev_sym = a.get_agent(prev_a).unwrap().symbol;
                    let prev_candidates = &groups_b[&prev_sym];
                    // Undo the mapping that got us here
                    if let Some(&mapped_b) = mapping.get(&prev_a) {
                        reverse.remove(&mapped_b);
                    }
                    mapping.remove(&prev_a);
                    // Continue searching from the next candidate
                    if try_candidates_from(
                        a,
                        b,
                        prev_a,
                        prev_candidates,
                        cand_idx,
                        mapping,
                        reverse,
                    ) {
                        stack.push(cand_idx);
                        index += 1;
                        continue;
                    } else {
                        // Keep backtracking
                        continue;
                    }
                }
                return false;
            }
        };

        // Try candidates starting from index 0
        let mut found = false;
        for (ci, &id_b) in candidates.iter().enumerate() {
            if reverse.contains_key(&id_b) {
                continue;
            }
            if is_consistent(a, b, id_a, id_b, mapping) {
                mapping.insert(id_a, id_b);
                reverse.insert(id_b, id_a);
                stack.push(ci + 1); // Next candidate to try on backtrack
                index += 1;
                found = true;
                break;
            }
        }

        if !found {
            // Backtrack
            loop {
                if let Some(cand_start) = stack.pop() {
                    index -= 1;
                    let prev_a = agents_a[index];
                    let prev_sym = a.get_agent(prev_a).unwrap().symbol;
                    let prev_candidates = &groups_b[&prev_sym];
                    // Undo mapping
                    if let Some(&mapped_b) = mapping.get(&prev_a) {
                        reverse.remove(&mapped_b);
                    }
                    mapping.remove(&prev_a);
                    // Try remaining candidates
                    let mut backtrack_found = false;
                    #[allow(clippy::needless_range_loop)]
                    for ci in cand_start..prev_candidates.len() {
                        let id_b = prev_candidates[ci];
                        if reverse.contains_key(&id_b) {
                            continue;
                        }
                        if is_consistent(a, b, prev_a, id_b, mapping) {
                            mapping.insert(prev_a, id_b);
                            reverse.insert(id_b, prev_a);
                            stack.push(ci + 1);
                            index += 1;
                            backtrack_found = true;
                            break;
                        }
                    }
                    if backtrack_found {
                        break;
                    }
                    // Continue backtracking
                } else {
                    return false; // Exhausted all possibilities
                }
            }
        }
    }
}

/// Helper: try candidates starting from a given index. Returns true if a
/// consistent candidate was found and mapped.
fn try_candidates_from(
    a: &Net,
    b: &Net,
    id_a: AgentId,
    candidates: &[AgentId],
    start: usize,
    mapping: &mut HashMap<AgentId, AgentId>,
    reverse: &mut HashMap<AgentId, AgentId>,
) -> bool {
    #[allow(clippy::needless_range_loop)]
    for ci in start..candidates.len() {
        let id_b = candidates[ci];
        if reverse.contains_key(&id_b) {
            continue;
        }
        if is_consistent(a, b, id_a, id_b, mapping) {
            mapping.insert(id_a, id_b);
            reverse.insert(id_b, id_a);
            return true;
        }
    }
    false
}

/// Check if mapping agent_a -> agent_b is consistent with existing mapping.
///
/// For each port of agent_a, check that the target either:
/// - Is an unmapped agent (no constraint yet — will be checked later)
/// - Maps to the corresponding target of agent_b's same port
/// - Is a FreePort with the same index
/// - Is DISCONNECTED on both sides
fn is_consistent(
    a: &Net,
    b: &Net,
    agent_a: AgentId,
    agent_b: AgentId,
    mapping: &HashMap<AgentId, AgentId>,
) -> bool {
    is_consistent_with_fp_strictness(a, b, agent_a, agent_b, mapping, true)
}

/// Internal worker for `is_consistent` parameterised by whether FreePort
/// labels must match by index (strict, IC composition semantics) or are
/// allowed to be related by an as-yet-unknown bijection (relaxed,
/// graph-structural semantics — used by `nets_graph_isomorphic`).
fn is_consistent_with_fp_strictness(
    a: &Net,
    b: &Net,
    agent_a: AgentId,
    agent_b: AgentId,
    mapping: &HashMap<AgentId, AgentId>,
    strict_freeport: bool,
) -> bool {
    let sym = a.get_agent(agent_a).unwrap().symbol;
    let num_ports = total_ports(sym);

    for p in 0..num_ports {
        let target_a = a.get_target(PortRef::AgentPort(agent_a, p));
        let target_b = b.get_target(PortRef::AgentPort(agent_b, p));

        match (target_a, target_b) {
            // Both point to agents
            (PortRef::AgentPort(ta_id, ta_port), PortRef::AgentPort(tb_id, tb_port)) => {
                // Ports must match
                if ta_port != tb_port {
                    return false;
                }
                // Self-reference in a must map to self-reference in b
                if ta_id == agent_a {
                    if tb_id != agent_b {
                        return false;
                    }
                } else if let Some(&mapped_b) = mapping.get(&ta_id) {
                    // Target is already mapped — must match
                    if mapped_b != tb_id {
                        return false;
                    }
                }
                // If ta_id is not yet mapped, no constraint (will be validated later)
            }
            // Both are FreePort — strict mode requires identical index;
            // relaxed mode accepts any FP↔FP correspondence (the implied
            // bijection is checked in a second pass after the agent
            // bijection is found).
            (PortRef::FreePort(fa), PortRef::FreePort(fb)) => {
                if strict_freeport && fa != fb {
                    return false;
                }
                // In relaxed mode, the only constraint is "both are
                // FreePorts" — already guaranteed by this match arm.
                // DISCONNECTED-vs-real-FP is also rejected (fa==MAX iff fb==MAX).
                if (fa == u32::MAX) != (fb == u32::MAX) {
                    return false;
                }
            }
            // Mixed types -> not consistent
            _ => return false,
        }
    }
    true
}

/// Same as [`nets_isomorphic`] but treats FreePort labels as anonymous
/// interface vertices: the predicate accepts any bijection over FreePort
/// indices (subject to the wire structure being preserved). Use this for
/// SPEC-09 R37c construction-isomorphism, where the streaming generator and
/// the eager generator MAY label FreePorts differently — for example,
/// `dual_tree_stream` keeps Lafont FreePort IDs above the agent-id space
/// (`freeport_base = 2 * nodes_per_tree`) to avoid colliding with the
/// `partition::streaming` border allocator (QA-D010-004), while
/// `io::generators::dual_tree` numbers FreePorts from 0 in DFS-leaf order.
/// Both nets describe the same graph but with different external labels.
///
/// `nets_isomorphic` (strict-FP) MUST still be used wherever the FreePort
/// labels are part of the IC composition signature (e.g., the T8 sentinel
/// or a future net-composition equality check).
///
/// Algorithm: two-pass. Pass 1 finds an agent bijection using a relaxed
/// per-port consistency check that ignores FreePort labels. Pass 2 walks
/// the agent bijection and derives the implied FreePort bijection from the
/// agent-port wires; the bijection is a function (no two distinct FP_a
/// map to the same FP_b) and an injection (and thus a bijection on equal
/// FP-counts).
pub fn nets_graph_isomorphic(a: &Net, b: &Net) -> bool {
    // Quick reject: same agent counts per symbol.
    let counts_a = count_agents_by_symbol(a);
    let counts_b = count_agents_by_symbol(b);
    if counts_a != counts_b {
        return false;
    }
    if counts_a.is_empty() {
        return true;
    }

    // Pass 1: agent bijection with relaxed FreePort matching.
    let groups_b = group_agents_by_symbol(b);
    let mut agents_a: Vec<AgentId> = a.live_agents().map(|ag| ag.id).collect();
    agents_a.sort_by_key(|&id| {
        let sym = a.get_agent(id).unwrap().symbol;
        let candidate_count = groups_b.get(&sym).map_or(0, |v| v.len());
        (candidate_count, id)
    });

    let mut mapping: HashMap<AgentId, AgentId> = HashMap::new();
    let mut reverse: HashMap<AgentId, AgentId> = HashMap::new();
    if !backtrack_relaxed(a, b, &agents_a, &mut mapping, &mut reverse, &groups_b) {
        return false;
    }

    // Pass 2: derive and verify FreePort bijection.
    let mut fp_fwd: HashMap<u32, u32> = HashMap::new();
    let mut fp_rev: HashMap<u32, u32> = HashMap::new();
    for (&agent_a_id, &agent_b_id) in &mapping {
        let sym = a.get_agent(agent_a_id).unwrap().symbol;
        let nports = total_ports(sym);
        for p in 0..nports {
            let ta = a.get_target(PortRef::AgentPort(agent_a_id, p));
            let tb = b.get_target(PortRef::AgentPort(agent_b_id, p));
            if let (PortRef::FreePort(fa), PortRef::FreePort(fb)) = (ta, tb) {
                if fa == u32::MAX && fb == u32::MAX {
                    continue; // both DISCONNECTED — no bijection constraint
                }
                if fa == u32::MAX || fb == u32::MAX {
                    return false; // mixed
                }
                match fp_fwd.get(&fa) {
                    Some(&prev) if prev != fb => return false,
                    Some(_) => {} // already consistent
                    None => {
                        if fp_rev.contains_key(&fb) {
                            // Some other fa' already mapped to fb — bijection violated.
                            return false;
                        }
                        fp_fwd.insert(fa, fb);
                        fp_rev.insert(fb, fa);
                    }
                }
            }
        }
    }

    true
}

/// Variant of `backtrack` that uses the relaxed (FreePort-labels-ignored)
/// per-port consistency check. Otherwise identical to the strict
/// `backtrack` — same iterative structure, same backtracking discipline.
#[allow(clippy::too_many_arguments)]
fn backtrack_relaxed(
    a: &Net,
    b: &Net,
    agents_a: &[AgentId],
    mapping: &mut HashMap<AgentId, AgentId>,
    reverse: &mut HashMap<AgentId, AgentId>,
    groups_b: &HashMap<Symbol, Vec<AgentId>>,
) -> bool {
    let mut stack: Vec<usize> = Vec::with_capacity(agents_a.len());
    let mut index = 0usize;

    loop {
        if index == agents_a.len() {
            return true;
        }
        let id_a = agents_a[index];
        let sym = a.get_agent(id_a).unwrap().symbol;
        let candidates = match groups_b.get(&sym) {
            Some(c) => c,
            None => return false,
        };

        let mut found = false;
        for (ci, &id_b) in candidates.iter().enumerate() {
            if reverse.contains_key(&id_b) {
                continue;
            }
            if is_consistent_with_fp_strictness(a, b, id_a, id_b, mapping, false) {
                mapping.insert(id_a, id_b);
                reverse.insert(id_b, id_a);
                stack.push(ci + 1);
                index += 1;
                found = true;
                break;
            }
        }

        if !found {
            // Backtrack
            loop {
                if let Some(cand_start) = stack.pop() {
                    index -= 1;
                    let prev_a = agents_a[index];
                    let prev_sym = a.get_agent(prev_a).unwrap().symbol;
                    let prev_candidates = &groups_b[&prev_sym];
                    if let Some(&mapped_b) = mapping.get(&prev_a) {
                        reverse.remove(&mapped_b);
                    }
                    mapping.remove(&prev_a);

                    let mut bf = false;
                    #[allow(clippy::needless_range_loop)]
                    for ci in cand_start..prev_candidates.len() {
                        let id_b = prev_candidates[ci];
                        if reverse.contains_key(&id_b) {
                            continue;
                        }
                        if is_consistent_with_fp_strictness(a, b, prev_a, id_b, mapping, false) {
                            mapping.insert(prev_a, id_b);
                            reverse.insert(id_b, prev_a);
                            stack.push(ci + 1);
                            index += 1;
                            bf = true;
                            break;
                        }
                    }
                    if bf {
                        break;
                    }
                } else {
                    return false;
                }
            }
        }
    }
}

/// Count live agents per symbol.
fn count_agents_by_symbol(net: &Net) -> HashMap<Symbol, usize> {
    let mut counts = HashMap::new();
    for agent in net.live_agents() {
        *counts.entry(agent.symbol).or_insert(0) += 1;
    }
    counts
}

/// Fast structural check: two nets have identical agent counts per symbol.
///
/// Weaker than `nets_isomorphic`: it is a necessary but not sufficient
/// condition for isomorphism. Use when full backtracking isomorphism is
/// intractable (L3 mitigation — see PHASE1-FINDINGS.md). A pass here
/// means "G1 weak pass"; a fail means G1 definitely fails.
pub fn nets_match_counts(a: &Net, b: &Net) -> bool {
    count_agents_by_symbol(a) == count_agents_by_symbol(b)
}

/// Group live agent IDs by symbol.
fn group_agents_by_symbol(net: &Net) -> HashMap<Symbol, Vec<AgentId>> {
    let mut groups: HashMap<Symbol, Vec<AgentId>> = HashMap::new();
    for agent in net.live_agents() {
        groups.entry(agent.symbol).or_default().push(agent.id);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};

    // T1: Two empty nets
    #[test]
    fn test_empty_nets_isomorphic() {
        assert!(nets_isomorphic(&Net::new(), &Net::new()));
    }

    // T2: Identical single-agent nets
    #[test]
    fn test_single_agent_isomorphic() {
        let mut a = Net::new();
        a.create_agent(Symbol::Era);
        let mut b = Net::new();
        b.create_agent(Symbol::Era);
        assert!(nets_isomorphic(&a, &b));
    }

    // T3: Different symbols
    #[test]
    fn test_different_symbols_not_isomorphic() {
        let mut a = Net::new();
        a.create_agent(Symbol::Con);
        let mut b = Net::new();
        b.create_agent(Symbol::Dup);
        assert!(!nets_isomorphic(&a, &b));
    }

    // T4: Same topology, different AgentIds
    #[test]
    fn test_same_topology_different_ids() {
        let mut a = Net::new();
        let c1 = a.create_agent(Symbol::Con);
        let e1 = a.create_agent(Symbol::Era);
        a.connect(PortRef::AgentPort(c1, 0), PortRef::AgentPort(e1, 0));

        let mut b = Net::new();
        // Create extra agent to shift IDs
        let _dummy = b.create_agent(Symbol::Era);
        b.remove_agent(_dummy);
        let c2 = b.create_agent(Symbol::Con);
        let e2 = b.create_agent(Symbol::Era);
        b.connect(PortRef::AgentPort(c2, 0), PortRef::AgentPort(e2, 0));

        assert!(nets_isomorphic(&a, &b));
    }

    // T5: Same agents, different connectivity
    #[test]
    fn test_different_connectivity_not_isomorphic() {
        let mut a = Net::new();
        let c1 = a.create_agent(Symbol::Con);
        let e1 = a.create_agent(Symbol::Era);
        a.connect(PortRef::AgentPort(c1, 0), PortRef::AgentPort(e1, 0));

        let mut b = Net::new();
        let c2 = b.create_agent(Symbol::Con);
        let e2 = b.create_agent(Symbol::Era);
        b.connect(PortRef::AgentPort(c2, 1), PortRef::AgentPort(e2, 0));

        assert!(!nets_isomorphic(&a, &b));
    }

    // T6: CON-CON pair with swapped IDs
    #[test]
    fn test_con_con_swapped_ids() {
        let mut a = Net::new();
        let a0 = a.create_agent(Symbol::Con);
        let a1 = a.create_agent(Symbol::Con);
        a.connect(PortRef::AgentPort(a0, 0), PortRef::AgentPort(a1, 0));

        let mut b = Net::new();
        let b0 = b.create_agent(Symbol::Con);
        let b1 = b.create_agent(Symbol::Con);
        // Same structure, different wiring order
        a.connect(PortRef::AgentPort(a0, 1), PortRef::AgentPort(a0, 2));
        a.connect(PortRef::AgentPort(a1, 1), PortRef::AgentPort(a1, 2));
        b.connect(PortRef::AgentPort(b0, 0), PortRef::AgentPort(b1, 0));
        b.connect(PortRef::AgentPort(b0, 1), PortRef::AgentPort(b0, 2));
        b.connect(PortRef::AgentPort(b1, 1), PortRef::AgentPort(b1, 2));

        assert!(nets_isomorphic(&a, &b));
    }

    // T7: EP-annihilation produces isomorphic empty nets
    #[test]
    fn test_ep_annihilation_isomorphic() {
        use crate::io::generators::ep_annihilation;
        use crate::reduction::reduce_all;

        let mut a = ep_annihilation(10);
        let mut b = ep_annihilation(10);
        reduce_all(&mut a);
        reduce_all(&mut b);
        assert!(nets_isomorphic(&a, &b));
    }

    // T8: Different FreePort indices
    #[test]
    fn test_different_freeport_not_isomorphic() {
        let mut a = Net::new();
        let ag_a = a.create_agent(Symbol::Era);
        a.connect(PortRef::AgentPort(ag_a, 0), PortRef::FreePort(0));

        let mut b = Net::new();
        let ag_b = b.create_agent(Symbol::Era);
        b.connect(PortRef::AgentPort(ag_b, 0), PortRef::FreePort(1));

        assert!(!nets_isomorphic(&a, &b));
    }

    // T9: Different agent counts
    #[test]
    fn test_different_agent_counts() {
        let mut a = Net::new();
        a.create_agent(Symbol::Era);
        a.create_agent(Symbol::Era);
        a.create_agent(Symbol::Era);

        let mut b = Net::new();
        b.create_agent(Symbol::Era);
        b.create_agent(Symbol::Era);

        assert!(!nets_isomorphic(&a, &b));
    }

    // T10: Church numeral isomorphism
    #[test]
    fn test_church_numeral_isomorphic() {
        use crate::encoding::encode_nat;
        let a = encode_nat(5);
        let b = encode_nat(5);
        assert!(nets_isomorphic(&a, &b));
    }

    // T11: Different Church numerals
    #[test]
    fn test_different_church_not_isomorphic() {
        use crate::encoding::encode_nat;
        let a = encode_nat(3);
        let b = encode_nat(4);
        assert!(!nets_isomorphic(&a, &b));
    }

    // T12: Performance — 41-agent net
    #[test]
    fn test_isomorphism_performance() {
        use crate::encoding::encode_nat;
        let a = encode_nat(20); // 41 agents
        let b = encode_nat(20);
        let start = std::time::Instant::now();
        assert!(nets_isomorphic(&a, &b));
        assert!(start.elapsed().as_secs() < 1);
    }

    // Edge case: self-loop (Church 0 lam_x)
    #[test]
    fn test_self_loop_isomorphic() {
        use crate::encoding::encode_nat;
        let a = encode_nat(0);
        let b = encode_nat(0);
        assert!(nets_isomorphic(&a, &b));
    }
}
