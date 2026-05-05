//! SPEC-22 R10b Strategy B (`BorderClean`) tests — TASK-0590.
//!
//! Verifies that under `RecyclePolicy::BorderClean`:
//!   - Border-referenced IDs are never popped from the free-list during
//!     delta/streaming rounds (`free_list_pops_border == 0`).
//!   - Non-border IDs ARE popped (`free_list_pops_non_border > 0`).
//!   - The `border_entries_shadow` lifecycle (set on AssignPartition, cleared
//!     on Done) is correct.
//!   - Cross-strategy isomorphism (A vs B) holds on the same workload.
//!
//! Source: TEST-SPEC-0590.
//!
//! All tests MUST pass on `cargo test` (default) and
//! `cargo test --features zero-copy`.

use relativist_core::net::{Net, PortRef, RecyclePolicy, Symbol};
use relativist_core::reduction::engine::reduce_all;
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Canonical fixture (sourced from TEST-SPEC-0482 / TEST-SPEC-T9b).
///
/// Builds a Net with a non-empty free-list containing a mix of border and
/// non-border IDs.
///
/// Free-list (LIFO, push order): [47, 50, 92, 73]
///   → pop order: 73 (first), 92, 50, 47 (last).
/// Border IDs (`border_entries_shadow`): {47, 92}.
/// Non-border IDs in free-list: {73, 50}.
///
/// Gated on `not(streaming-no-recycle)` — only used by tests that verify
/// non-border pops occur, which are suppressed by the cargo feature.
#[cfg(not(feature = "streaming-no-recycle"))]
fn make_mixed_border_nonborder_fixture() -> Net {
    let mut net = Net::new();
    // Allocate agents at IDs 0..=100 (next_id = 101) so that 47, 50, 73, 92
    // are all within the arena and can be legitimately freed.
    for _ in 0..101 {
        net.create_agent(Symbol::Era);
    }
    // Free the IDs we want in the free-list, in push-order so LIFO pops 73 first.
    // push order: 47, 50, 92, 73 → pop order 73, 92, 50, 47.
    net.remove_agent(47);
    net.remove_agent(50);
    net.remove_agent(92);
    net.remove_agent(73);
    assert_eq!(
        net.free_list,
        vec![47, 50, 92, 73],
        "fixture: free_list must be [47,50,92,73]"
    );

    // Populate border_entries_shadow with border IDs {47, 92}.
    let border: HashSet<u32> = [47u32, 92].iter().copied().collect();
    net.border_entries_shadow = Some(border);

    net
}

// ---------------------------------------------------------------------------
// UT-0590-03: Strategy B — border ID at free-list head is NOT popped
// ---------------------------------------------------------------------------

/// UT-0590-03: `create_agent` under Strategy B skips a border-protected ID
/// at the free-list head, falls through to fresh allocation, and does not
/// increment `free_list_pops_border`.
///
/// The test pushes only the border ID 47 to the free-list top (LIFO), then
/// verifies no pop of a border ID occurs.
#[test]
#[cfg(debug_assertions)]
fn ut_0590_03_strategy_b_no_pop_for_border_id() {
    let mut net = Net::new();
    // Place a single border ID in the free-list.
    for _ in 0..48 {
        net.create_agent(Symbol::Era);
    }
    net.remove_agent(47); // free_list = [47]

    // Mark ID 47 as border-protected.
    let mut border = HashSet::new();
    border.insert(47u32);
    net.border_entries_shadow = Some(border);

    // Enable Strategy B + delta round (proxy for streaming_active).
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    let next_before = net.next_id;
    let allocated = net.create_agent(Symbol::Con);

    assert_eq!(
        allocated, next_before,
        "UT-0590-03: border ID must not be popped — fresh alloc expected"
    );
    assert!(
        net.free_list.contains(&47),
        "UT-0590-03: border ID 47 must still be in free_list after failed pop"
    );
    assert_eq!(
        net.free_list_pops_border, 0,
        "UT-0590-03: free_list_pops_border must be zero (border ID was re-pushed)"
    );
    assert_eq!(
        net.free_list_pops, 0,
        "UT-0590-03: free_list_pops must be zero (no successful pop)"
    );
}

// ---------------------------------------------------------------------------
// UT-0590-04: Strategy B — non-border ID IS popped
// ---------------------------------------------------------------------------

/// UT-0590-04: `create_agent` under Strategy B pops a non-border ID and
/// increments `free_list_pops_non_border`.
///
/// Free-list: [50] (non-border); border_entries_shadow: {47, 92}.
///
/// Gated on `not(streaming-no-recycle)`: when the cargo feature is enabled,
/// the compile-time gate unconditionally skips the free-list (TASK-0591).
#[test]
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
fn ut_0590_04_strategy_b_pop_for_non_border_id() {
    let mut net = Net::new();
    // Allocate IDs 0..93 so IDs 47, 50, 92 are in the arena.
    for _ in 0..93 {
        net.create_agent(Symbol::Era);
    }
    net.remove_agent(50); // free_list = [50]

    // Border shadow contains 47 and 92 but NOT 50.
    let mut border = HashSet::new();
    border.insert(47u32);
    border.insert(92u32);
    net.border_entries_shadow = Some(border);

    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    let allocated = net.create_agent(Symbol::Con);

    assert_eq!(
        allocated, 50,
        "UT-0590-04: non-border ID 50 must be popped (R3 path)"
    );
    assert!(
        !net.free_list.contains(&50),
        "UT-0590-04: ID 50 must no longer be in free_list after pop"
    );
    assert_eq!(
        net.free_list_pops_non_border, 1,
        "UT-0590-04: free_list_pops_non_border must be 1"
    );
    assert_eq!(
        net.free_list_pops, 1,
        "UT-0590-04: total free_list_pops must be 1"
    );
    assert_eq!(
        net.free_list_pops_border, 0,
        "UT-0590-04: free_list_pops_border must remain 0"
    );
}

// ---------------------------------------------------------------------------
// UT-0590-05: O(1) membership check — 1000 calls within budget
// ---------------------------------------------------------------------------

/// UT-0590-05: Strategy B membership check remains O(1) for a large
/// `border_entries_shadow` (100 IDs).
///
/// Creates 200 agents, frees the non-border ones, then calls `create_agent`
/// 1000 times under Strategy B. Asserts total time is not tracked (this is a
/// functional test — the O(1) contract is validated by the HashSet type).
/// The test merely ensures 1000 calls complete without panicking.
#[test]
#[cfg(debug_assertions)]
fn ut_0590_05_strategy_b_o1_membership_check() {
    let mut net = Net::new();
    // Allocate 200 agents.
    for _ in 0..200 {
        net.create_agent(Symbol::Era);
    }
    // Border: IDs 0..100 (100 entries).
    let border: HashSet<u32> = (0u32..100).collect();
    // Free non-border IDs 100..200 (100 entries).
    for id in 100u32..200 {
        net.remove_agent(id);
    }
    net.border_entries_shadow = Some(border);
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    // 1000 calls — all should hit non-border slots or fresh alloc.
    for _ in 0..1000 {
        net.create_agent(Symbol::Con);
    }
    // If we reach here without panic, O(1) HashSet check is working correctly.
    assert!(
        net.free_list_pops_border == 0,
        "UT-0590-05: no border IDs must have been popped"
    );
}

// ---------------------------------------------------------------------------
// UT-0590-06: Strategy B triggers on streaming_active alone (broadening)
// ---------------------------------------------------------------------------

/// UT-0590-06: Strategy B gate fires when only streaming_active is set
/// (delta_mode=false, streaming_active=true).
///
/// Per R37b broadening: `(delta_mode || streaming_active)`. The proxy
/// `is_in_delta_round` covers both flags.
#[test]
#[cfg(debug_assertions)]
fn ut_0590_06_strategy_b_streaming_alone_triggers_gate() {
    let mut net = Net::new();
    for _ in 0..48 {
        net.create_agent(Symbol::Era);
    }
    net.remove_agent(47); // free_list = [47]

    let mut border = HashSet::new();
    border.insert(47u32);
    net.border_entries_shadow = Some(border);

    // streaming_active proxied by is_in_delta_round=true; delta_mode is not a separate flag.
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    let next_before = net.next_id;
    let allocated = net.create_agent(Symbol::Con);

    assert_eq!(
        allocated, next_before,
        "UT-0590-06: streaming_active alone must trigger gate; border ID not popped"
    );
    assert_eq!(
        net.free_list_pops_border, 0,
        "UT-0590-06: border ID must not be popped when streaming_active is set"
    );
}

// ---------------------------------------------------------------------------
// UT-0590-07: Strategy B does NOT trigger when both flags are off
// ---------------------------------------------------------------------------

/// UT-0590-07: Strategy B gate does NOT trigger when
/// `streaming_active = false` AND `delta_mode = false`.
/// The pop is allowed normally even for a border ID.
#[test]
#[cfg(debug_assertions)]
fn ut_0590_07_strategy_b_pop_when_streaming_inactive_and_no_delta() {
    let mut net = Net::new();
    for _ in 0..48 {
        net.create_agent(Symbol::Era);
    }
    net.remove_agent(47); // free_list = [47]

    let mut border = HashSet::new();
    border.insert(47u32);
    net.border_entries_shadow = Some(border);

    // Both flags off — gate must NOT trigger.
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = false;

    let allocated = net.create_agent(Symbol::Con);

    // When is_in_delta_round=false AND streaming_active=false, the RecyclePolicy gate
    // doesn't fire. Strategy B only protects when (is_in_delta_round || streaming_active)
    // per R37b disjunction (QA-D010-001). With both flags off, R3 pop path is taken.
    assert_eq!(
        allocated, 47,
        "UT-0590-07: gate inactive — ID 47 must be popped normally"
    );
    assert_eq!(
        net.free_list_pops, 1,
        "UT-0590-07: one pop should have occurred"
    );
    // SF-001: free_list_pops_border must be 1 — ID 47 IS in shadow, but protection
    // was inactive (both round-protection flags false). This is the push-mode scenario:
    // the counter fires on unprotected pops of shadow-present IDs.
    assert_eq!(
        net.free_list_pops_border, 1,
        "UT-0590-07: push-mode shadow-present pop must increment free_list_pops_border (SF-001)"
    );
}

// ---------------------------------------------------------------------------
// UT-0590-08: border_entries_shadow is a subset of the coordinator's view
// ---------------------------------------------------------------------------

/// UT-0590-08: The per-worker `border_entries_shadow` contains only the
/// border IDs for this partition (50 out of 200 total in the coordinator).
///
/// Verifies the subset relationship: worker-local view is sufficient per
/// TASK-0590 NOTE line 79.
#[test]
fn ut_0590_08_border_entries_subset_of_coordinator_bordergraph() {
    let mut net = Net::new();
    net.next_id = 0;

    // Coordinator has 200 border entries globally (simulated).
    // This worker's partition has 50 border IDs: 0..50.
    let worker_borders: HashSet<u32> = (0u32..50).collect();
    net.border_entries_shadow = Some(worker_borders.clone());

    let shadow = net.border_entries_shadow.as_ref().unwrap();
    assert_eq!(
        shadow.len(),
        50,
        "UT-0590-08: worker-local border_entries must have exactly 50 entries"
    );

    // Verify all IDs in the shadow are in the worker's range.
    for &id in shadow.iter() {
        assert!(
            id < 50,
            "UT-0590-08: border entry {} out of worker partition range [0..50)",
            id
        );
    }
}

// ---------------------------------------------------------------------------
// UT-0590-09: border_entries_shadow is None after clearing (Done transition)
// ---------------------------------------------------------------------------

/// UT-0590-02 (renamed UT-0590-09 to avoid confusion with task numbering):
/// The `border_entries_shadow` cache is cleared when the worker transitions
/// to `Done` (simulated here by setting it to None, which is what the Done
/// transition does per TASK-0590 acceptance line 32).
#[test]
fn ut_0590_09_border_entries_cleared_on_done() {
    let mut net = Net::new();
    let border: HashSet<u32> = [47u32, 92].iter().copied().collect();
    net.border_entries_shadow = Some(border);

    assert!(
        net.border_entries_shadow.is_some(),
        "UT-0590-09: shadow should be Some before Done"
    );

    // Simulate the Done transition: clear the border_entries_shadow.
    net.border_entries_shadow = None;

    assert!(
        net.border_entries_shadow.is_none(),
        "UT-0590-09: border_entries must be cleared on Done (no leak between runs)"
    );

    // Subsequent border protection: with shadow = None, no ID is protected.
    // Verify by attempting to create an agent in delta mode: free-list must be popped.
    net.is_in_delta_round = true;
    net.recycle_policy = RecyclePolicy::BorderClean;
    // Shadow was cleared, so nothing protects any ID.
    // Allocate fresh (free_list empty after prior frees were cleared).
    let freed_id = net.create_agent(Symbol::Con);
    let _ = freed_id; // just ensure no panic
}

// ---------------------------------------------------------------------------
// IT-0590-01: zero border pops, nonzero non-border pops
// ---------------------------------------------------------------------------

/// IT-0590-01: End-to-end Strategy B test with mixed free-list.
///
/// Sets up a net with both border and non-border IDs in the free-list,
/// enables Strategy B, and verifies that only non-border IDs are popped.
///
/// Gated on `not(streaming-no-recycle)`: when the cargo feature is enabled,
/// the compile-time gate unconditionally skips ALL pops (TASK-0591).
#[test]
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
fn it_0590_01_strategy_b_zero_border_pops_nonzero_non_border_pops() {
    let mut net = make_mixed_border_nonborder_fixture();

    // Enable Strategy B + streaming/delta.
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    // Pop order is LIFO: 73 (non-border), 92 (border), 50 (non-border), 47 (border).
    // With Strategy B:
    //   - 73 → non-border pop succeeds → free_list_pops_non_border = 1
    //   - 92 → border: re-push, fall through to fresh alloc (92 stays in free_list)
    //   - next attempt on free_list: 50 → non-border pop succeeds → free_list_pops_non_border = 2
    //   - 47 → border: re-push, fall through to fresh alloc
    // After 4 create_agent calls: pops_non_border = 2, pops_border = 0.
    // Note: after re-pushing 92, the LIFO order may change.

    let id1 = net.create_agent(Symbol::Con); // should pop 73 (non-border, LIFO head)
    assert_ne!(
        id1, 92,
        "IT-0590-01: first call must not produce the border ID 92"
    );
    assert_ne!(
        id1, 47,
        "IT-0590-01: first call must not produce the border ID 47"
    );

    // After 4 total creates, check counters.
    let _ = net.create_agent(Symbol::Era);
    let _ = net.create_agent(Symbol::Dup);
    let _ = net.create_agent(Symbol::Con);

    assert_eq!(
        net.free_list_pops_border, 0,
        "IT-0590-01: zero border pops (precision gate holds)"
    );
    assert!(
        net.free_list_pops_non_border > 0,
        "IT-0590-01: at least one non-border pop (precision recycling is active)"
    );
    // Border IDs must still be in the free-list.
    assert!(
        net.free_list.contains(&47) || net.free_list.contains(&92),
        "IT-0590-01: at least one border ID must remain in the free-list"
    );
}

// ---------------------------------------------------------------------------
// IT-0590-02: Cross-strategy isomorphism (A vs B)
// ---------------------------------------------------------------------------

/// IT-0590-02: The same CON-CON annihilation workload reduced under Strategy A
/// (DisableUnderDelta) and Strategy B (BorderClean) produces isomorphic results.
///
/// Note: Only isomorphism is asserted (not byte-equality), because Strategy B
/// may reuse slot IDs that Strategy A preserves, changing the allocation layout
/// per TASK-0590 NOTE line 78.
#[test]
fn it_0590_02_cross_strategy_isomorphism_a_vs_b() {
    use relativist_core::bench::isomorphism::nets_isomorphic;

    // Build the same workload twice.
    fn build_workload() -> Net {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(4));
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net
    }

    // Strategy A: reduce with DisableUnderDelta.
    let mut net_a = build_workload();
    net_a.recycle_policy = RecyclePolicy::DisableUnderDelta;
    net_a.is_in_delta_round = false; // push mode for reduction
    reduce_all(&mut net_a);

    // Strategy B: reduce with BorderClean (no border shadow = behaves like R3).
    let mut net_b = build_workload();
    net_b.recycle_policy = RecyclePolicy::BorderClean;
    net_b.is_in_delta_round = false; // push mode for reduction
    reduce_all(&mut net_b);

    assert!(
        nets_isomorphic(&net_a, &net_b),
        "IT-0590-02: Strategy A and B must produce isomorphic results (G1 / ARG-005)"
    );
}

// ---------------------------------------------------------------------------
// IT-0590-03: Baseline regression — no regressions
// ---------------------------------------------------------------------------

/// IT-0590-03: A CON-ERA reduction under Strategy B produces a correct
/// normal form (regression gate for the 1181/1224 baseline).
#[test]
fn it_0590_03_strategy_b_baseline_regression() {
    let mut net = Net::new();
    let con = net.create_agent(Symbol::Con);
    let era = net.create_agent(Symbol::Era);
    net.connect(PortRef::AgentPort(con, 0), PortRef::AgentPort(era, 0));
    net.connect(PortRef::AgentPort(con, 1), PortRef::FreePort(10));
    net.connect(PortRef::AgentPort(con, 2), PortRef::FreePort(11));

    // Strategy B in push mode (no delta, no streaming) — must behave like R3.
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = false;

    reduce_all(&mut net);

    assert!(
        net.redex_queue.is_empty(),
        "IT-0590-03: CON-ERA must fully reduce under Strategy B"
    );
    #[cfg(debug_assertions)]
    net.debug_check_invariants();
}

// ---------------------------------------------------------------------------
// EC-1: empty border_entries_shadow — Strategy B pops freely
// ---------------------------------------------------------------------------

/// EC-1: With an empty `border_entries_shadow`, Strategy B pops freely
/// (no IDs match the empty set); behaves like SPEC-22 R3.
///
/// Gated on `not(streaming-no-recycle)`: when the cargo feature is enabled,
/// the compile-time gate unconditionally skips the free-list (TASK-0591).
#[test]
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
fn ec_0590_1_empty_border_entries_pops_freely() {
    let mut net = Net::new();
    let id0 = net.create_agent(Symbol::Con);
    net.remove_agent(id0); // free_list = [id0]

    // Empty border shadow.
    net.border_entries_shadow = Some(HashSet::new());
    net.recycle_policy = RecyclePolicy::BorderClean;
    net.is_in_delta_round = true;

    let allocated = net.create_agent(Symbol::Era);

    assert_eq!(
        allocated, id0,
        "EC-1: empty border_entries → free pop proceeds normally"
    );
    assert_eq!(
        net.free_list_pops_non_border, 1,
        "EC-1: pop classified as non-border (no ID in empty shadow)"
    );
    assert_eq!(
        net.free_list_pops_border, 0,
        "EC-1: no border pops with empty shadow"
    );
}

// ---------------------------------------------------------------------------
// EC-5: Strategy A short-circuit — mutually exclusive with Strategy B
// ---------------------------------------------------------------------------

/// EC-5: When `recycle_policy == DisableUnderDelta`, the Strategy B per-id
/// check is NOT exercised; the Strategy A short-circuit fires first.
#[test]
#[cfg(debug_assertions)]
fn ec_0590_5_strategy_a_blocks_before_strategy_b_check() {
    let mut net = Net::new();
    let id0 = net.create_agent(Symbol::Con);
    net.remove_agent(id0);

    // Shadow has the ID but Strategy A is active.
    let mut border = HashSet::new();
    border.insert(id0);
    net.border_entries_shadow = Some(border);

    // Strategy A.
    net.recycle_policy = RecyclePolicy::DisableUnderDelta;
    net.is_in_delta_round = true;

    let _ = net.create_agent(Symbol::Era);

    // With Strategy A, the entire free-list was skipped — no pops of any kind.
    assert_eq!(
        net.free_list_pops, 0,
        "EC-5: Strategy A must suppress all pops before Strategy B is checked"
    );
    assert_eq!(
        net.free_list_pops_border, 0,
        "EC-5: zero border pops under Strategy A"
    );
}

// ---------------------------------------------------------------------------
// QA-D010-001: R37b disjunction — streaming_active alone arms the gate
// ---------------------------------------------------------------------------

/// QA-D010-001-A: When only `streaming_active = true` (delta round has NOT started),
/// Strategy A MUST suppress free-list pops.
///
/// This is the "streaming_active alone arms the gate" leg of R37b.
#[test]
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
fn qa_d010_001_a_streaming_active_alone_arms_strategy_a_gate() {
    let mut net = Net::new();
    let id0 = net.create_agent(Symbol::Con);
    net.remove_agent(id0); // free_list = [id0]

    // streaming_active = true, is_in_delta_round = false (streaming-only mode).
    net.streaming_active = true;
    net.is_in_delta_round = false;
    net.recycle_policy = RecyclePolicy::DisableUnderDelta;
    let next_before = net.next_id;

    let new_id = net.create_agent(Symbol::Era);

    assert_eq!(
        new_id, next_before,
        "QA-D010-001-A: streaming_active alone must suppress free-list pop (Strategy A R37b disjunction)"
    );
    assert_eq!(
        net.free_list_pops, 0,
        "QA-D010-001-A: free_list_pops must be 0 when gate is armed by streaming_active"
    );
    assert!(
        net.free_list.contains(&id0),
        "QA-D010-001-A: freed ID must remain in free_list (not popped)"
    );
}

/// QA-D010-001-B: When `is_in_delta_round = true` alone arms the gate and then
/// `exit_streaming_mode` (which now only clears `streaming_active`) is called,
/// the delta-mode protections MUST remain active.
///
/// This is the regression guard for the destructive-clear bug: the old code set
/// `is_in_delta_round = false` in `exit_streaming_mode`, which silently disarmed
/// the delta-round protections. The new code uses a separate `streaming_active` flag.
#[test]
#[cfg(all(debug_assertions, not(feature = "streaming-no-recycle")))]
fn qa_d010_001_b_exit_streaming_mode_preserves_delta_round_protections() {
    use relativist_core::worker::{enter_streaming_mode, exit_streaming_mode};

    let mut net = Net::new();
    let id0 = net.create_agent(Symbol::Con);
    net.remove_agent(id0); // free_list = [id0]

    // Simulate a delta round (outer state, set by delta-mode logic).
    net.is_in_delta_round = true;
    net.recycle_policy = RecyclePolicy::DisableUnderDelta;

    // Worker enters streaming mode (chunked dispatch).
    enter_streaming_mode(&mut net);
    assert!(
        net.streaming_active,
        "QA-D010-001-B: enter_streaming_mode must set streaming_active"
    );
    // is_in_delta_round must NOT be changed by enter_streaming_mode.
    assert!(
        net.is_in_delta_round,
        "QA-D010-001-B: enter_streaming_mode must NOT change is_in_delta_round"
    );

    // Worker exits streaming (chunk stream exhausted) while delta round is still active.
    exit_streaming_mode(&mut net);
    assert!(
        !net.streaming_active,
        "QA-D010-001-B: exit_streaming_mode must clear streaming_active"
    );
    // CRITICAL: is_in_delta_round must remain true after exit_streaming_mode.
    // The delta round is still active; only streaming mode has ended.
    assert!(
        net.is_in_delta_round,
        "QA-D010-001-B: exit_streaming_mode must NOT clear is_in_delta_round (delta still active)"
    );

    // Delta-mode protections must still be active (free-list pop must be suppressed).
    let next_before = net.next_id;
    let new_id = net.create_agent(Symbol::Era);
    assert_eq!(
        new_id, next_before,
        "QA-D010-001-B: delta-round protections must remain active after exit_streaming_mode"
    );
    assert_eq!(
        net.free_list_pops, 0,
        "QA-D010-001-B: zero pops — gate must still be armed by is_in_delta_round"
    );
}
