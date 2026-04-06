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
}
