//! Reduction loop: reduce_step, reduce_all, reduce_n.
//!
//! Drives the reduction engine by dequeuing redexes, dispatching rules,
//! and collecting statistics.

use crate::net::Net;

use super::dispatch::{get_rule, get_specific_rule, normalize_pair, Rule, SpecificRule};
use super::rules::{interact_anni, interact_comm, interact_eras, interact_void};

// ---------------------------------------------------------------------------
// StepResult
// ---------------------------------------------------------------------------

/// Result of a single reduction step (SPEC-03 Section 4.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepResult {
    /// A redex was successfully reduced. Contains the applied rule (4-category)
    /// and the specific rule (6-variant) for per-rule counting.
    Reduced(Rule, SpecificRule),
    /// The queue is empty: net is in Normal Form.
    NormalForm,
}

// ---------------------------------------------------------------------------
// reduce_step
// ---------------------------------------------------------------------------

/// Executes a single reduction step (SPEC-03 Section 4.6.1).
///
/// Dequeues pairs from the redex queue until a valid (non-stale) one is found,
/// applies the corresponding rule, and returns the result.
///
/// Complexity: O(1) amortized (each stale discard is O(1);
/// rule application is O(1)).
pub fn reduce_step(net: &mut Net) -> StepResult {
    loop {
        // 1. Dequeue next pair
        let (a_id, b_id) = match net.redex_queue.pop_front() {
            Some(pair) => pair,
            None => return StepResult::NormalForm,
        };

        // 2. Verify validity (discard stale)
        if !net.is_valid_redex(a_id, b_id) {
            continue;
        }

        // 3. Normalize pair for dispatch (R9: sym_a <= sym_b)
        let (a, b) = normalize_pair(a_id, b_id, net);

        // 4. Determine rule (4-category + 6-specific)
        let sym_a = net.agents[a as usize].unwrap().symbol;
        let sym_b = net.agents[b as usize].unwrap().symbol;
        let rule = get_rule(sym_a, sym_b);
        let specific = get_specific_rule(sym_a, sym_b);

        // 5. Apply rule
        match rule {
            Rule::Anni => interact_anni(net, a, b),
            Rule::Comm => interact_comm(net, a, b),
            Rule::Eras => interact_eras(net, a, b),
            Rule::Void => interact_void(net, a, b),
        }

        // 6. Verify invariants in debug mode (SPEC-03 Section 4.6.1)
        // Note: FreePort bidirectionality (I1) is temporarily violated during
        // partitioned sub-net reduction (R26), but assert_adjacency_consistent
        // already skips FreePort targets. Safe to run here.
        #[cfg(debug_assertions)]
        net.assert_all_invariants();

        return StepResult::Reduced(rule, specific);
    }
}

// ---------------------------------------------------------------------------
// ReductionStats
// ---------------------------------------------------------------------------

/// Statistics of a completed (or partial) reduction (SPEC-03 Section 4.6.2).
///
/// Tracks the total number of interactions and per-rule breakdowns.
/// Managed by callers (`reduce_all`, `reduce_n`), not by `reduce_step`.
#[derive(Debug, Clone, Default)]
pub struct ReductionStats {
    /// Total number of interactions performed.
    pub total_interactions: u64,
    /// Number of Annihilation interactions (CON-CON + DUP-DUP).
    pub anni_count: u64,
    /// Number of Commutation interactions (CON-DUP).
    pub comm_count: u64,
    /// Number of Erasure interactions (CON-ERA + DUP-ERA).
    pub eras_count: u64,
    /// Number of Void interactions (ERA-ERA).
    pub void_count: u64,
    /// Per-rule interaction counts (6 Lafont rules).
    /// Index order: [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    /// Corresponds to `SpecificRule` enum discriminants.
    pub interactions_by_rule: [u64; 6],
}

// ---------------------------------------------------------------------------
// reduce_all
// ---------------------------------------------------------------------------

/// Reduces the net to Normal Form (empty redex queue).
///
/// WARNING: does not terminate if the net is non-terminating.
/// For potentially non-terminating nets, use `reduce_n`.
///
/// Complexity: O(S) where S is the total number of interactions to Normal Form
/// (invariant T7 from SPEC-01 guarantees S is unique for the given net).
pub fn reduce_all(net: &mut Net) -> ReductionStats {
    let mut stats = ReductionStats {
        total_interactions: 0,
        anni_count: 0,
        comm_count: 0,
        eras_count: 0,
        void_count: 0,
        interactions_by_rule: [0; 6],
    };

    loop {
        match reduce_step(net) {
            StepResult::NormalForm => return stats,
            StepResult::Reduced(rule, specific) => {
                stats.total_interactions += 1;
                match rule {
                    Rule::Anni => stats.anni_count += 1,
                    Rule::Comm => stats.comm_count += 1,
                    Rule::Eras => stats.eras_count += 1,
                    Rule::Void => stats.void_count += 1,
                }
                stats.interactions_by_rule[specific as usize] += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// reduce_n
// ---------------------------------------------------------------------------

/// Reduces the net by at most `budget` interactions (SPEC-03 Section 4.6.3).
///
/// Useful for:
/// - Granularity control in the grid (workers execute a budget and return
///   a partial result).
/// - Safeguard against non-terminating nets (SPEC-01, I5).
///
/// Returns statistics of the interactions performed (may be < budget
/// if Normal Form is reached before the budget is exhausted).
pub fn reduce_n(net: &mut Net, budget: usize) -> ReductionStats {
    let mut stats = ReductionStats {
        total_interactions: 0,
        anni_count: 0,
        comm_count: 0,
        eras_count: 0,
        void_count: 0,
        interactions_by_rule: [0; 6],
    };

    for _ in 0..budget {
        match reduce_step(net) {
            StepResult::NormalForm => return stats,
            StepResult::Reduced(rule, specific) => {
                stats.total_interactions += 1;
                match rule {
                    Rule::Anni => stats.anni_count += 1,
                    Rule::Comm => stats.comm_count += 1,
                    Rule::Eras => stats.eras_count += 1,
                    Rule::Void => stats.void_count += 1,
                }
                stats.interactions_by_rule[specific as usize] += 1;
            }
        }
    }

    stats
}

// ---------------------------------------------------------------------------
// reduce_border_once
// ---------------------------------------------------------------------------

/// Processes every redex currently in the queue exactly once, deferring any
/// new cascades to the next call (SPEC-05 R30a, strict BSP mode).
///
/// Snapshots the current `net.redex_queue`, empties it, and applies one
/// reduction step per snapshotted pair. Any new redexes produced by those
/// reductions accumulate in `net.redex_queue` and are intentionally left
/// there for a future round to consume — they are NOT reduced in this call.
///
/// Stale pairs (both agents no longer alive, or the principal-principal
/// invariant broken) are skipped silently, as in `reduce_step`.
///
/// Complexity: O(k) where k is the number of redexes in the initial queue.
/// Each rule application is O(1) (SPEC-03 R14).
///
/// Semantics guarantee:
///   `reduce_border_once(net)` performs at most one interaction per redex
///   present when the call started. Any cascade produced by those
///   interactions is still pending in `net.redex_queue` when the call
///   returns.
pub fn reduce_border_once(net: &mut Net) -> ReductionStats {
    let mut stats = ReductionStats {
        total_interactions: 0,
        anni_count: 0,
        comm_count: 0,
        eras_count: 0,
        void_count: 0,
        interactions_by_rule: [0; 6],
    };

    // Snapshot the current queue and clear it. Any new redexes created by
    // rule application during this call will land back in net.redex_queue
    // via Net::connect and remain there as the deferred-cascade set.
    let initial: std::collections::VecDeque<(crate::net::AgentId, crate::net::AgentId)> =
        std::mem::take(&mut net.redex_queue);
    let mut deferred: std::collections::VecDeque<(crate::net::AgentId, crate::net::AgentId)> =
        std::collections::VecDeque::new();

    for (a_id, b_id) in initial {
        // Skip stale pairs (same logic as reduce_step's loop).
        if !net.is_valid_redex(a_id, b_id) {
            continue;
        }

        // Move any cascades accumulated from the previous iteration off the
        // live queue so reduce_step sees only this one pair.
        deferred.extend(std::mem::take(&mut net.redex_queue));

        // Place this pair as the sole item in the live queue, then drive
        // exactly one reduction step.
        debug_assert!(net.redex_queue.is_empty());
        net.redex_queue.push_back((a_id, b_id));

        match reduce_step(net) {
            StepResult::NormalForm => {
                // Pair became stale between validity check and reduce_step
                // (should not happen with single-threaded code, but handled
                // defensively).
            }
            StepResult::Reduced(rule, specific) => {
                stats.total_interactions += 1;
                match rule {
                    Rule::Anni => stats.anni_count += 1,
                    Rule::Comm => stats.comm_count += 1,
                    Rule::Eras => stats.eras_count += 1,
                    Rule::Void => stats.void_count += 1,
                }
                stats.interactions_by_rule[specific as usize] += 1;
            }
        }
    }

    // Everything left in net.redex_queue after the final iteration is a
    // newly-created cascade; merge it with any earlier deferred cascades.
    deferred.extend(std::mem::take(&mut net.redex_queue));
    net.redex_queue = deferred;

    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{PortRef, Symbol};

    // -----------------------------------------------------------------------
    // StepResult tests
    // -----------------------------------------------------------------------

    // T1: StepResult has exactly 2 variants
    #[test]
    fn test_step_result_exhaustive() {
        fn describe(sr: StepResult) -> &'static str {
            match sr {
                StepResult::Reduced(_, _) => "reduced",
                StepResult::NormalForm => "normal_form",
            }
        }
        assert_eq!(describe(StepResult::NormalForm), "normal_form");
        assert_eq!(
            describe(StepResult::Reduced(Rule::Anni, SpecificRule::ConCon)),
            "reduced"
        );
    }

    // T2: StepResult derives Debug, Clone, Copy, PartialEq, Eq
    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_step_result_derives() {
        let a = StepResult::NormalForm;
        let b = a; // Copy
        let c = a.clone(); // Clone
        assert_eq!(a, b); // PartialEq
        assert_eq!(a, c);
        assert_ne!(
            StepResult::NormalForm,
            StepResult::Reduced(Rule::Void, SpecificRule::EraEra)
        );
        // Debug
        assert_eq!(format!("{:?}", StepResult::NormalForm), "NormalForm");
    }

    // -----------------------------------------------------------------------
    // reduce_step tests
    // -----------------------------------------------------------------------

    // T3: Empty net returns NormalForm
    #[test]
    fn test_reduce_step_empty_net() {
        let mut net = Net::new();
        assert_eq!(reduce_step(&mut net), StepResult::NormalForm);
    }

    // T4: ERA-ERA pair returns Reduced(Void, EraEra)
    #[test]
    fn test_reduce_step_era_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Void, SpecificRule::EraEra)
        );
        // Both removed
        assert!(net.get_agent(a).is_none());
        assert!(net.get_agent(b).is_none());
    }

    // T5: CON-CON pair returns Reduced(Anni, ConCon)
    #[test]
    fn test_reduce_step_con_con() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // Give aux ports somewhere to go
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Anni, SpecificRule::ConCon)
        );
    }

    // T6: DUP-DUP pair returns Reduced(Anni, DupDup)
    #[test]
    fn test_reduce_step_dup_dup() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Anni, SpecificRule::DupDup)
        );
    }

    // T7: CON-DUP pair returns Reduced(Comm, ConDup)
    #[test]
    fn test_reduce_step_con_dup() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Comm, SpecificRule::ConDup)
        );
    }

    // T8: CON-ERA pair returns Reduced(Eras, ConEra)
    #[test]
    fn test_reduce_step_con_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Eras, SpecificRule::ConEra)
        );
    }

    // T9: DUP-ERA pair returns Reduced(Eras, DupEra)
    #[test]
    fn test_reduce_step_dup_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Eras, SpecificRule::DupEra)
        );
    }

    // T10: Stale redex is silently discarded
    #[test]
    fn test_reduce_step_stale_redex_discarded() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);

        // Create two redexes: (a,b) and (c,d)
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        // Break (a,b) to make it stale: disconnect, then reconnect to FreePort
        net.disconnect(PortRef::AgentPort(a, 0));
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));

        // reduce_step should skip stale (a,b) and process valid (c,d)
        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Void, SpecificRule::EraEra)
        );
        // c and d should be removed
        assert!(net.get_agent(c).is_none());
        assert!(net.get_agent(d).is_none());
    }

    // T11: All stale redexes return NormalForm
    #[test]
    fn test_reduce_step_all_stale() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        // Remove both to make redex stale
        net.remove_agent(a);
        net.remove_agent(b);

        // Queue has stale entry, should return NormalForm
        assert_eq!(reduce_step(&mut net), StepResult::NormalForm);
    }

    // T12: reduce_step applies the correct rule (net state mutated)
    #[test]
    fn test_reduce_step_mutates_net() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        let x = net.create_agent(Symbol::Dup);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(x, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(0));
        // All of x's ports need valid connections for debug assertions (T1)
        net.connect(PortRef::AgentPort(x, 0), PortRef::FreePort(5));
        net.connect(PortRef::AgentPort(x, 2), PortRef::FreePort(6));

        let before = net.count_live_agents(); // 3
        reduce_step(&mut net);
        let after = net.count_live_agents(); // 3 (CON-ERA: -2 +2)

        // CON-ERA creates 2 new ERA, removes CON + ERA = balance 0 on the pair
        // Plus the context agent x = 1 + 2 new = 3
        assert_eq!(after, before); // balance 0 for interact_eras
    }

    // E1: Multiple redexes -- reduce_step processes exactly one
    #[test]
    fn test_reduce_step_one_at_a_time() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);

        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        // First step: processes one redex
        let result1 = reduce_step(&mut net);
        assert!(matches!(result1, StepResult::Reduced(Rule::Void, _)));

        // Second step: processes the other
        let result2 = reduce_step(&mut net);
        assert!(matches!(result2, StepResult::Reduced(Rule::Void, _)));

        // Third step: NormalForm
        assert_eq!(reduce_step(&mut net), StepResult::NormalForm);
    }

    // E2: Reversed pair (DUP-CON) dispatches correctly via normalize_pair
    #[test]
    fn test_reduce_step_reversed_pair() {
        let mut net = Net::new();
        // Create DUP first, then CON -- reversed from canonical order
        let dup = net.create_agent(Symbol::Dup);
        let con = net.create_agent(Symbol::Con);

        net.connect(PortRef::AgentPort(dup, 0), PortRef::AgentPort(con, 0));
        net.connect(PortRef::AgentPort(dup, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(dup, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(con, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(con, 2), PortRef::FreePort(3));

        // Should still dispatch as Comm/ConDup despite reversed creation order
        let result = reduce_step(&mut net);
        assert_eq!(
            result,
            StepResult::Reduced(Rule::Comm, SpecificRule::ConDup)
        );
    }

    // -----------------------------------------------------------------------
    // ReductionStats tests
    // -----------------------------------------------------------------------

    // T1: ReductionStats has all 7 fields
    #[test]
    fn test_reduction_stats_fields() {
        let stats = ReductionStats {
            total_interactions: 1,
            anni_count: 2,
            comm_count: 3,
            eras_count: 4,
            void_count: 5,
            interactions_by_rule: [10, 20, 30, 40, 50, 60],
        };
        assert_eq!(stats.total_interactions, 1);
        assert_eq!(stats.anni_count, 2);
        assert_eq!(stats.comm_count, 3);
        assert_eq!(stats.eras_count, 4);
        assert_eq!(stats.void_count, 5);
        assert_eq!(stats.interactions_by_rule, [10, 20, 30, 40, 50, 60]);
    }

    // T2: ReductionStats derives Debug, Clone
    #[test]
    fn test_reduction_stats_derives() {
        let stats = ReductionStats {
            total_interactions: 0,
            anni_count: 0,
            comm_count: 0,
            eras_count: 0,
            void_count: 0,
            interactions_by_rule: [0; 6],
        };
        let cloned = stats.clone();
        assert_eq!(cloned.total_interactions, 0);
        // Debug
        let debug = format!("{:?}", stats);
        assert!(debug.contains("ReductionStats"));
    }

    // T3: Default initialization (all zeros)
    #[test]
    fn test_reduction_stats_zero_init() {
        let stats = ReductionStats {
            total_interactions: 0,
            anni_count: 0,
            comm_count: 0,
            eras_count: 0,
            void_count: 0,
            interactions_by_rule: [0; 6],
        };
        assert_eq!(stats.total_interactions, 0);
        assert_eq!(stats.anni_count, 0);
        assert_eq!(stats.comm_count, 0);
        assert_eq!(stats.eras_count, 0);
        assert_eq!(stats.void_count, 0);
        assert_eq!(stats.interactions_by_rule, [0; 6]);
    }

    // -----------------------------------------------------------------------
    // reduce_all tests
    // -----------------------------------------------------------------------

    // T4: Empty net returns stats with total_interactions = 0
    #[test]
    fn test_reduce_all_empty_net() {
        let mut net = Net::new();
        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 0);
    }

    // T5: Single ERA-ERA pair
    #[test]
    fn test_reduce_all_era_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 1);
        assert_eq!(stats.void_count, 1);
        assert_eq!(stats.interactions_by_rule[SpecificRule::EraEra as usize], 1);
    }

    // T6: Single CON-CON pair
    #[test]
    fn test_reduce_all_con_con() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 1);
        assert_eq!(stats.anni_count, 1);
        assert_eq!(stats.interactions_by_rule[SpecificRule::ConCon as usize], 1);
    }

    // T7: Single CON-DUP pair (commutation creates 4 new agents)
    #[test]
    fn test_reduce_all_con_dup() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 1);
        assert_eq!(stats.comm_count, 1);
        assert_eq!(stats.interactions_by_rule[SpecificRule::ConDup as usize], 1);
    }

    // T8: Multi-step: CON-ERA creates 2 ERA which auto-connect to FreePort
    //     (no cascading ERA-ERA in this setup since created ERAs connect to FreePort)
    #[test]
    fn test_reduce_all_con_era_stats() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 1);
        assert_eq!(stats.eras_count, 1);
        assert_eq!(stats.interactions_by_rule[SpecificRule::ConEra as usize], 1);
    }

    // T9: Stats consistency: total == anni + comm + eras + void
    #[test]
    fn test_reduce_all_stats_consistency_category() {
        let mut net = Net::new();
        // Create multiple redexes of different types
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let stats = reduce_all(&mut net);
        assert_eq!(
            stats.total_interactions,
            stats.anni_count + stats.comm_count + stats.eras_count + stats.void_count
        );
    }

    // T10: Stats consistency: total == sum(interactions_by_rule)
    #[test]
    fn test_reduce_all_stats_consistency_specific() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let stats = reduce_all(&mut net);
        let sum: u64 = stats.interactions_by_rule.iter().sum();
        assert_eq!(stats.total_interactions, sum);
    }

    // T11: Net is in normal form after reduce_all
    #[test]
    fn test_reduce_all_leaves_normal_form() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        reduce_all(&mut net);
        assert_eq!(reduce_step(&mut net), StepResult::NormalForm);
    }

    // E1: Net with agents but no redexes
    #[test]
    fn test_reduce_all_no_redexes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Connect aux ports only -- no principal-to-principal = no redex
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(4));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(5));

        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 0);
    }

    // E2: Multiple same-type redexes (3x ERA-ERA)
    #[test]
    fn test_reduce_all_multiple_era_era() {
        let mut net = Net::new();
        for _ in 0..3 {
            let a = net.create_agent(Symbol::Era);
            let b = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        }

        let stats = reduce_all(&mut net);
        assert_eq!(stats.total_interactions, 3);
        assert_eq!(stats.void_count, 3);
        assert_eq!(stats.interactions_by_rule[SpecificRule::EraEra as usize], 3);
    }

    // -----------------------------------------------------------------------
    // reduce_n tests
    // -----------------------------------------------------------------------

    // T1: Empty net with budget=10
    #[test]
    fn test_reduce_n_empty_net() {
        let mut net = Net::new();
        let stats = reduce_n(&mut net, 10);
        assert_eq!(stats.total_interactions, 0);
    }

    // T2: Single ERA-ERA with budget=10 (stops at NormalForm before budget)
    #[test]
    fn test_reduce_n_era_era_under_budget() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let stats = reduce_n(&mut net, 10);
        assert_eq!(stats.total_interactions, 1);
        assert_eq!(stats.void_count, 1);
    }

    // T3: Budget=0 performs no reductions
    #[test]
    fn test_reduce_n_budget_zero() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let stats = reduce_n(&mut net, 0);
        assert_eq!(stats.total_interactions, 0);
        // Redex still exists
        assert!(net.is_valid_redex(a, b));
    }

    // T4: Budget=1 on net with 3 ERA-ERA pairs
    #[test]
    fn test_reduce_n_budget_one_of_three() {
        let mut net = Net::new();
        for _ in 0..3 {
            let a = net.create_agent(Symbol::Era);
            let b = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        }

        let stats = reduce_n(&mut net, 1);
        assert_eq!(stats.total_interactions, 1);
    }

    // T5: Budget exactly equals required steps
    #[test]
    fn test_reduce_n_exact_budget() {
        let mut net = Net::new();
        for _ in 0..3 {
            let a = net.create_agent(Symbol::Era);
            let b = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        }

        let stats = reduce_n(&mut net, 3);
        assert_eq!(stats.total_interactions, 3);
        // Should be in normal form
        assert_eq!(reduce_step(&mut net), StepResult::NormalForm);
    }

    // T6: Budget exceeds required steps
    #[test]
    fn test_reduce_n_excess_budget() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let stats = reduce_n(&mut net, 100);
        assert_eq!(stats.total_interactions, 1);
    }

    // T7: Stats consistency: total == anni + comm + eras + void
    #[test]
    fn test_reduce_n_stats_consistency_category() {
        let mut net = Net::new();
        // CON-CON pair
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let stats = reduce_n(&mut net, 10);
        assert_eq!(
            stats.total_interactions,
            stats.anni_count + stats.comm_count + stats.eras_count + stats.void_count
        );
    }

    // T8: Stats consistency: total == sum(interactions_by_rule)
    #[test]
    fn test_reduce_n_stats_consistency_specific() {
        let mut net = Net::new();
        for _ in 0..2 {
            let a = net.create_agent(Symbol::Era);
            let b = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        }

        let stats = reduce_n(&mut net, 5);
        let sum: u64 = stats.interactions_by_rule.iter().sum();
        assert_eq!(stats.total_interactions, sum);
    }

    // T9: Net NOT in normal form when budget < required
    #[test]
    fn test_reduce_n_partial_not_normal_form() {
        let mut net = Net::new();
        for _ in 0..3 {
            let a = net.create_agent(Symbol::Era);
            let b = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        }

        reduce_n(&mut net, 1);
        // Net still has 2 more redexes
        assert_ne!(reduce_step(&mut net), StepResult::NormalForm);
    }

    // E1: Budget=usize::MAX on small net terminates
    #[test]
    fn test_reduce_n_max_budget() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let stats = reduce_n(&mut net, usize::MAX);
        assert_eq!(stats.total_interactions, 1);
    }

    // E2: reduce_n then reduce_all finishes the job
    #[test]
    fn test_reduce_n_then_reduce_all() {
        let mut net = Net::new();
        for _ in 0..3 {
            let a = net.create_agent(Symbol::Era);
            let b = net.create_agent(Symbol::Era);
            net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        }

        let stats1 = reduce_n(&mut net, 1);
        assert_eq!(stats1.total_interactions, 1);

        let stats2 = reduce_all(&mut net);
        assert_eq!(stats2.total_interactions, 2);

        // Combined = 3
        assert_eq!(stats1.total_interactions + stats2.total_interactions, 3);
    }
}
