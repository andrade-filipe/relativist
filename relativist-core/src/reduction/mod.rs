//! Reduction engine for Interaction Combinators (SPEC-03).
//!
//! Implements the 6 interaction rules (CON-CON, DUP-DUP, ERA-ERA,
//! CON-DUP, CON-ERA, DUP-ERA), dispatch, and the reduction loop
//! (reduce_step, reduce_all, reduce_n).

pub mod dispatch;
pub mod engine;
pub mod rules;

// Re-exports: convenience access via `crate::reduction::*`
pub use dispatch::{get_rule, get_specific_rule, normalize_pair, Rule, SpecificRule};
pub use engine::{
    reduce_all, reduce_border_once, reduce_n, reduce_step, ReductionStats, StepResult,
};
pub use rules::{interact_anni, interact_comm, interact_eras, interact_void};

use crate::net::Net;

/// Counts the number of **valid** (non-stale) active pairs currently queued in
/// `net.redex_queue`, per SPEC-01 I4 stale-pruning semantics.
///
/// SPEC-27 v3 R4 (closure of Round 1 SC-005) tightens the semantics of
/// `DecodeError::NotNormalForm.redexes`: the field MUST count valid active
/// pairs after stale-entry pruning per SPEC-01 I4, NOT `net.redex_queue.len()`.
/// This helper is the canonical detector consumed by every `Decoder` impl.
///
/// An entry `(a, b)` is **valid** iff:
/// - both `a` and `b` are live agents in `net.agents`, AND
/// - their principal ports (`AgentPort(_, 0)`) are mutually connected
///   (i.e., `net.get_target(AgentPort(a, 0)) == AgentPort(b, 0)`).
///
/// Internally delegates to `Net::is_valid_redex`, which is the same detector
/// used by `reduce_step` / `reduce_all` to discard stale entries (R17, I4).
///
/// **Read-only:** does NOT mutate `net.redex_queue`. Callers that want to drain
/// stale entries should use the engine's drain helpers instead.
///
/// Complexity: O(Q) where Q is the queue length.
#[allow(dead_code)] // Consumed by `decode_biguint` (TASK-0712) and HornerCodec (TASK-0715).
pub(crate) fn count_valid_active_pairs(net: &Net) -> usize {
    net.redex_queue
        .iter()
        .filter(|(a, b)| net.is_valid_redex(*a, *b))
        .count()
}

#[cfg(test)]
mod count_valid_active_pairs_tests {
    use super::*;
    use crate::encoding::arithmetic::{build_add, build_mul};
    use crate::net::{PortRef, Symbol};
    use crate::reduction::reduce_all;

    // UT-0709-01: empty queue -> 0.
    #[test]
    fn count_valid_pairs_zero_on_empty_queue() {
        let net = Net::new();
        assert!(net.redex_queue.is_empty());
        assert_eq!(count_valid_active_pairs(&net), 0);
    }

    // UT-0709-02: one live redex -> 1. Note `Net::connect` auto-enqueues
    // principal-to-principal connections, so we do NOT push_back manually.
    #[test]
    fn count_valid_pairs_includes_live_redex() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        assert_eq!(net.redex_queue.len(), 1);
        assert_eq!(count_valid_active_pairs(&net), 1);
    }

    // UT-0709-03: agent removed -> stale entry pruned out (returns 0 even if
    // queue still has the pair). `connect` auto-enqueues.
    #[test]
    fn count_valid_pairs_excludes_stale_after_remove_agent() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        net.remove_agent(a);

        let queue_len_before = net.redex_queue.len();
        let pruned = count_valid_active_pairs(&net);

        assert_eq!(pruned, 0, "stale entry must NOT count as a valid redex");
        assert_eq!(
            net.redex_queue.len(),
            queue_len_before,
            "helper MUST NOT mutate redex_queue"
        );
    }

    // UT-0709-04: stale via disconnect (live agents but principals no longer
    // mutually connected) returns 0.
    #[test]
    fn count_valid_pairs_excludes_stale_after_disconnect() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Manually push a stale entry: (a, b) was never wired, so the redex
        // is stale (a.p0 and b.p0 are DISCONNECTED).
        net.redex_queue.push_back((a, b));

        assert_eq!(count_valid_active_pairs(&net), 0);

        // Now wire a.p0 to a fresh ERA: connect auto-enqueues (a, c). The queue
        // contains both the stale (a, b) and the live (a, c).
        let c = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(c, 0));
        assert_eq!(net.redex_queue.len(), 2);
        assert_eq!(count_valid_active_pairs(&net), 1);
    }

    // UT-0709-05: net reduced to NF -> helper returns 0.
    #[test]
    fn count_valid_pairs_zero_on_normal_form() {
        let mut net = build_add(2, 3);
        reduce_all(&mut net);
        assert_eq!(count_valid_active_pairs(&net), 0);

        let mut net = build_mul(7, 9);
        reduce_all(&mut net);
        assert_eq!(count_valid_active_pairs(&net), 0);
    }
}
