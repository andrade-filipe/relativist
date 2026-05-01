//! TASK-0601 — LIFO non-protected stalemate edge-case fix (QA-D010-016).
//!
//! Background. Under SPEC-22 R10b Strategy B, when a Net is in a delta
//! round (or streaming-active round), `create_agent` consults the
//! free-list LIFO top: if the popped ID is *border-protected*
//! (per `border_entries_shadow`), the previous code re-pushed it and
//! fell through to fresh `next_id` allocation. Deeper non-border IDs
//! never got recycled — they languished forever because the LIFO stack
//! always served the protected top. QA-D010-016 flagged this as a
//! starvation-style edge case (no current test triggered it).
//!
//! Fix (TASK-0601, dispatch brief 2026-04-30):
//! - When the LIFO top is protected, scan deeper for the first
//!   non-protected entry. If found, swap (push protected top back,
//!   recycle the deeper entry).
//! - When *every* free-list entry is protected, fall back to fresh
//!   `next_id` allocation AND emit `tracing::warn!` once per occurrence.
//!   Increment the `lifo_stalemate_fallbacks` debug counter for tests.
//!
//! References:
//! - SPEC-21 §3 streaming dispatch fairness invariant.
//! - SPEC-22 R10b Strategy B (TASK-0590) — per-id protection gate.
//! - QA-D010-016 — LIFO non-protected stalemate.
//!
//! Concrete bounds chosen at impl time (per TEST-SPEC-0601 §"Known spec
//! ambiguity"): N=20 free-list entries; gate <= 30 rounds for bounded
//! completion. The fix is O(free_list.len()) per recycle attempt — the
//! N=20 budget keeps the worst-case scan well under the gate.

// Imports are needed only by tests gated `not(feature = "streaming-no-recycle")`.
// Under the streaming-no-recycle feature, all tests in this file are excluded
// (the LIFO recycle path being exercised here is compile-out under that feature).
#[cfg(not(feature = "streaming-no-recycle"))]
use std::collections::HashSet;

#[cfg(not(feature = "streaming-no-recycle"))]
use relativist_core::net::{Net, RecyclePolicy, Symbol};

/// IT-0601-02 — `stale_chunk_completes_under_adversarial_arrivals`.
///
/// Reproduces the QA-D010-016 starvation scenario as a free-list
/// stalemate: a partition Net with N=20 free-list entries, of which the
/// LIFO TOP (most recently pushed) is border-protected but the deeper
/// entries are NOT. Under the OLD code, every `create_agent` call would
/// see the protected top, re-push it, and fall through to fresh
/// allocation — the deeper non-protected entries would never be popped.
/// Under the FIX, the deeper entries are recycled within bounded steps.
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
#[test]
fn it_0601_02_stale_chunk_completes_under_adversarial_arrivals() {
    let mut net = Net::new();

    // Build N=20 free-list entries by create+remove pairs (IDs 0..20).
    let n: u32 = 20;
    let ids: Vec<u32> = (0..n).map(|_| net.create_agent(Symbol::Era)).collect();
    for id in &ids {
        net.remove_agent(*id);
    }
    assert_eq!(net.free_list.len(), n as usize);

    // Mark ONLY the LIFO top (the most recently pushed entry) as
    // border-protected. Under the OLD code, every recycle attempt would
    // see this top, re-push it, and fall through to fresh allocation —
    // the 19 deeper entries would languish forever.
    let lifo_top = *net.free_list.last().expect("free-list pre-populated");
    let mut border = HashSet::new();
    border.insert(lifo_top);
    net.border_entries_shadow = Some(border);

    // Engage Strategy B + delta-round so the protection gate fires.
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    // Issue 19 fresh creates (one for each non-protected entry).
    // With the FIX, each one recycles a non-border ID from the deeper
    // free-list. Without the fix, each would fall through to next_id.
    let pops_before = net.free_list_pops;
    for _ in 0..(n as usize - 1) {
        net.create_agent(Symbol::Era);
    }
    let pops_delta = net.free_list_pops - pops_before;

    // Acceptance criterion: the 19 non-protected entries are all
    // recycled — bounded completion, NOT starvation.
    assert_eq!(
        pops_delta, 19,
        "IT-0601-02: TASK-0601 fix must recycle every non-protected entry \
         (19 deeper IDs); under the old code this would be 0. Got {}",
        pops_delta
    );

    // Bounded round budget: gate <= 30 rounds. Each recycle is one round
    // here, plus optional fall-throughs — well within the budget.
    assert!(
        pops_delta <= 30,
        "IT-0601-02: bounded completion within <= 30 rounds (got {})",
        pops_delta
    );

    // The protected top remains in the free-list (untouched).
    assert!(
        net.free_list.contains(&lifo_top),
        "IT-0601-02: border-protected top must remain in free-list after recycling deeper entries"
    );
    assert_eq!(
        net.free_list.len(),
        1,
        "IT-0601-02: only the protected top remains (19 deeper entries recycled)"
    );

    // No stalemate fallbacks: there were always non-protected entries
    // available, so the fall-back-to-fresh-allocation branch never fired.
    assert_eq!(
        net.lifo_stalemate_fallbacks, 0,
        "IT-0601-02: no stalemate fallback expected when non-protected entries exist"
    );
}

/// IT-0601-02b — `stalemate_fallback_when_every_entry_protected`.
///
/// The dual scenario: ALL free-list entries are border-protected. The
/// fix MUST fall back to fresh `next_id` allocation, increment the
/// `lifo_stalemate_fallbacks` counter, and (per the dispatch brief)
/// emit a tracing warn — exactly once per stalemate occurrence.
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
#[test]
fn it_0601_02b_stalemate_fallback_when_every_entry_protected() {
    let mut net = Net::new();

    // Pre-populate 4 free-list entries.
    let ids: Vec<u32> = (0..4).map(|_| net.create_agent(Symbol::Era)).collect();
    for id in &ids {
        net.remove_agent(*id);
    }

    // Mark EVERY entry as border-protected.
    let border: HashSet<u32> = ids.iter().copied().collect();
    net.border_entries_shadow = Some(border);

    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    // First create: triggers a stalemate fallback.
    let _ = net.create_agent(Symbol::Era);
    assert_eq!(
        net.lifo_stalemate_fallbacks, 1,
        "IT-0601-02b: first create with all-protected free-list must increment stalemate counter"
    );
    // No pops occurred — every entry was protected.
    assert_eq!(
        net.free_list_pops, 0,
        "IT-0601-02b: stalemate path must NOT increment free_list_pops"
    );
    // The free-list is still intact (every entry re-pushed).
    assert_eq!(
        net.free_list.len(),
        4,
        "IT-0601-02b: free-list intact after stalemate fallback"
    );
}

/// IT-0601-03 — `fifo_path_unchanged_no_perf_regression`.
///
/// Sanity test: the FIFO (push-mode) path is NOT modified by this task.
/// In push mode (`is_in_delta_round=false`, no streaming), the recycle
/// path bypasses the Strategy B protection gate entirely — no extra
/// scan of the free-list, no stalemate counter increment.
///
/// We use the same workload as IT-0601-02 but switch to push mode.
/// Under push mode, every entry is recycled (border-or-not), and the
/// stalemate counter remains zero — the new code path is dormant.
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
#[test]
fn it_0601_03_fifo_path_unchanged_no_perf_regression() {
    let mut net = Net::new();

    let n: u32 = 20;
    let ids: Vec<u32> = (0..n).map(|_| net.create_agent(Symbol::Era)).collect();
    for id in &ids {
        net.remove_agent(*id);
    }
    assert_eq!(net.free_list.len(), n as usize);

    // Mark the LIFO top as border-protected (irrelevant in push mode).
    let lifo_top = *net.free_list.last().expect("free-list pre-populated");
    let mut border = HashSet::new();
    border.insert(lifo_top);
    net.border_entries_shadow = Some(border);

    // Push mode: gate disengaged.
    net.recycle_policy = RecyclePolicy::DisableUnderDelta;
    net.is_in_delta_round = false;

    // Issue N=20 fresh creates: every entry should pop (border or not).
    for _ in 0..n {
        net.create_agent(Symbol::Era);
    }

    // Push-mode invariant: free_list drained completely.
    assert!(
        net.free_list.is_empty(),
        "IT-0601-03: push mode drains the free-list (FIFO path unchanged)"
    );
    assert_eq!(
        net.free_list_pops, n as u64,
        "IT-0601-03: every entry recycled in push mode (no protection gate)"
    );

    // The TASK-0601 stalemate counter MUST remain at zero in push mode
    // (the new scan logic is gated behind `strategy_b_protect_engaged`).
    assert_eq!(
        net.lifo_stalemate_fallbacks, 0,
        "IT-0601-03: push mode does NOT engage the protection gate — \
         stalemate counter must stay zero"
    );
}
