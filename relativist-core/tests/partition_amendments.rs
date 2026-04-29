//! TASK-0411 — Integration tests for SPEC-04 §3.8 A3 + A4 amendments.
//!
//! A3: `PartitionPlan::allocate_border_ids(count: u32) -> Result<Range<u32>, PartitionError>`
//!     Dynamic border-id allocator with overflow guard (SPEC-20 R24d).
//! A4: `remap_partition_ids(partition: Partition, new_range: IdRange) -> Result<Partition, PartitionError>`
//!     Renumbers a reclaimed partition's agents into a fresh `IdRange`
//!     before it participates in `Net::union` (SPEC-20 R30).
//!
//! Test IDs map 1:1 to TEST-SPEC-0411.
//! TDD RED phase: at first these fail to compile (methods do not yet exist);
//! after the GREEN phase all pass.

use std::collections::HashMap;
use std::collections::VecDeque;

use relativist_core::error::PartitionError;
use relativist_core::net::{Agent, Net, PortRef, Symbol, DISCONNECTED, PORTS_PER_SLOT};
use relativist_core::partition::{remap_partition_ids, IdRange, Partition, PartitionPlan};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Creates a `PartitionPlan` with a specified `next_border_id` cursor.
///
/// Simulates a plan that has already consumed `next_border_id` border IDs.
fn make_plan_with_cursor(next_border_id: u32) -> PartitionPlan {
    PartitionPlan::with_next_border_id(next_border_id)
}

/// `p0`: 4 Con agents at `AgentId` values [10, 11, 12, 13].
/// Internal wires: agent 10 aux1 ↔ agent 11 aux1, agent 12 aux1 ↔ agent 13 aux1.
/// FreePort(42) on agent 10 aux2 (border_id marker, untouched by remap).
/// IdRange [10, 14) to mirror a partition freshly produced by split().
///
/// This fixture simulates a reclaimed partition post-split, which `remap_partition_ids`
/// will renumber into a fresh `IdRange` (SPEC-20 R30).
fn make_p0() -> Partition {
    // Arena must accommodate index 13 (max id used).
    let arena_len = 14;
    let mut agents = vec![None; arena_len];
    let mut ports = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

    // Insert 4 Con agents at ids 10, 11, 12, 13.
    for id in [10u32, 11, 12, 13] {
        agents[id as usize] = Some(Agent {
            symbol: Symbol::Con,
            id,
        });
    }

    // Internal wire: agent 10 aux1 (port 1) ↔ agent 11 aux1 (port 1).
    ports[10 * PORTS_PER_SLOT + 1] = PortRef::AgentPort(11, 1);
    ports[11 * PORTS_PER_SLOT + 1] = PortRef::AgentPort(10, 1);

    // Internal wire: agent 12 aux1 (port 1) ↔ agent 13 aux1 (port 1).
    ports[12 * PORTS_PER_SLOT + 1] = PortRef::AgentPort(13, 1);
    ports[13 * PORTS_PER_SLOT + 1] = PortRef::AgentPort(12, 1);

    // FreePort(42) on agent 10 aux2 (port 2) — border_id, left unchanged by remap.
    ports[10 * PORTS_PER_SLOT + 2] = PortRef::FreePort(42);

    // Build free_port_index: bid 42 → AgentPort(10, 2).
    let mut free_port_index = HashMap::new();
    free_port_index.insert(42u32, PortRef::AgentPort(10, 2));

    let subnet = Net {
        agents,
        ports,
        redex_queue: VecDeque::new(),
        next_id: 14,
        root: None,
        freeport_redirects: HashMap::new(),
        free_list: Vec::new(),
        id_range: None,
        border_entries_shadow: None,
        recycle_policy: relativist_core::net::core::RecyclePolicy::DisableUnderDelta,
        is_in_delta_round: false,
        #[cfg(debug_assertions)]
        protected_tombstones: None,
        #[cfg(debug_assertions)]
        free_list_pops: 0,
        #[cfg(debug_assertions)]
        free_list_pops_border: 0,
        #[cfg(debug_assertions)]
        free_list_pops_non_border: 0,
    };

    Partition {
        subnet,
        worker_id: 0,
        free_port_index,
        id_range: IdRange { start: 10, end: 14 },
        border_id_start: 40,
        border_id_end: 43,
    }
}

// ---------------------------------------------------------------------------
// A3 — allocate_border_ids
// ---------------------------------------------------------------------------

/// UT-0411-01: Two successive calls yield non-overlapping, contiguous ranges;
/// cursor advances to the sum of both counts.
#[test]
fn allocate_border_ids_disjoint_successive_calls() {
    let mut plan = make_plan_with_cursor(0);

    let r1 = plan
        .allocate_border_ids(5)
        .expect("first allocation must succeed");
    let r2 = plan
        .allocate_border_ids(7)
        .expect("second allocation must succeed");

    assert_eq!(r1, 0..5, "first range must be [0, 5)");
    assert_eq!(r2, 5..12, "second range must be [5, 12)");

    // Disjoint: ranges must not overlap.
    assert!(r1.end <= r2.start, "ranges must be disjoint");
}

/// UT-0411-02: After N calls with sizes c_i, the cursor equals Σ c_i.
#[test]
fn allocate_border_ids_cursor_advances() {
    let mut plan = make_plan_with_cursor(0);

    let sizes = [3u32, 10, 1, 50, 7];
    let expected_sum: u32 = sizes.iter().sum();

    for &size in &sizes {
        plan.allocate_border_ids(size)
            .expect("allocation must succeed");
    }

    // Allocate 0 more — returned range should start at expected_sum.
    let probe = plan
        .allocate_border_ids(1)
        .expect("probe allocation must succeed");
    assert_eq!(
        probe.start, expected_sum,
        "cursor must equal sum of all previous counts"
    );
}

/// UT-0411-03: `allocate_border_ids(0)` is a legal no-op.
/// Returns an empty range; cursor unchanged.
#[test]
fn allocate_border_ids_zero_count_is_legal_no_op() {
    let mut plan = make_plan_with_cursor(100);

    let r = plan
        .allocate_border_ids(0)
        .expect("zero-count allocation must not error");

    assert!(r.is_empty(), "returned range must be empty for count=0");

    // Cursor must remain at 100: next real allocation starts there.
    let next = plan
        .allocate_border_ids(1)
        .expect("next allocation must succeed");
    assert_eq!(
        next.start, 100,
        "cursor must be unchanged after zero-count call"
    );
}

/// UT-0411-04: Near `u32::MAX`, returns `BorderIdSpaceExhausted` with correct fields.
/// Cursor must NOT advance on error.
#[test]
fn allocate_border_ids_exhaustion_error() {
    let available: u32 = 5;
    let cursor = u32::MAX - available; // next_border_id = u32::MAX - 5
    let mut plan = make_plan_with_cursor(cursor);

    let result = plan.allocate_border_ids(10); // request 10, only 5 available

    match result {
        Err(PartitionError::BorderIdSpaceExhausted {
            requested,
            available: avail,
        }) => {
            assert_eq!(requested, 10, "reported requested count must match");
            assert_eq!(
                avail, available,
                "reported available must equal u32::MAX - cursor"
            );
        }
        other => panic!("expected BorderIdSpaceExhausted, got {:?}", other),
    }

    // Cursor must be unchanged — partial allocation is forbidden.
    let probe = plan
        .allocate_border_ids(1)
        .expect("a smaller allocation within available space must succeed");
    assert_eq!(
        probe.start, cursor,
        "cursor must be unchanged after failed allocation"
    );
}

// ---------------------------------------------------------------------------
// A3 — edge cases
// ---------------------------------------------------------------------------

/// EC-1: Allocate exactly `u32::MAX - cursor` — maximal range; then allocate 1
/// more → `BorderIdSpaceExhausted`.
#[test]
fn allocate_border_ids_maximal_then_exhausted() {
    let cursor: u32 = 10;
    let maximal = u32::MAX - cursor;
    let mut plan = make_plan_with_cursor(cursor);

    let r = plan
        .allocate_border_ids(maximal)
        .expect("maximal allocation must succeed");
    assert_eq!(r.start, cursor);
    assert_eq!(r.end, u32::MAX);

    // Now exhausted — any further allocation must fail.
    let result = plan.allocate_border_ids(1);
    assert!(
        matches!(result, Err(PartitionError::BorderIdSpaceExhausted { .. })),
        "must return BorderIdSpaceExhausted after exhaustion"
    );
}

// ---------------------------------------------------------------------------
// A4 — remap_partition_ids
// ---------------------------------------------------------------------------

/// UT-0411-05: `agent_count` and symbol multiset are preserved after remap.
/// New agent ids must be in the `new_range`.
#[test]
fn remap_preserves_agent_count_and_symbols() {
    let p0 = make_p0();
    let new_range = IdRange {
        start: 200,
        end: 204,
    };

    let p = remap_partition_ids(p0.clone(), new_range).expect("remap must succeed");

    assert_eq!(
        p.subnet.count_live_agents(),
        4,
        "agent count must be preserved"
    );

    // Symbol multiset check: all 4 agents must be Con.
    let symbols: Vec<Symbol> = p
        .subnet
        .agents
        .iter()
        .filter_map(|slot| slot.as_ref().map(|a| a.symbol))
        .collect();
    assert_eq!(symbols.len(), 4, "must have exactly 4 live agents");
    assert!(
        symbols.iter().all(|&s| s == Symbol::Con),
        "all agents must be Con"
    );

    // New IDs must be exactly {200, 201, 202, 203}.
    let mut new_ids: Vec<u32> = p
        .subnet
        .agents
        .iter()
        .enumerate()
        .filter_map(|(i, slot)| slot.as_ref().map(|_| i as u32))
        .collect();
    new_ids.sort();
    assert_eq!(
        new_ids,
        vec![200, 201, 202, 203],
        "agent ids must be in new_range"
    );
}

/// UT-0411-06: Every internal `PortRef::AgentPort(old_id, port)` is rewritten
/// to the corresponding new id; no stale references to old range [10, 14) remain.
#[test]
fn remap_rewrites_internal_edges() {
    let p0 = make_p0();
    let new_range = IdRange {
        start: 200,
        end: 204,
    };
    let p = remap_partition_ids(p0, new_range).expect("remap must succeed");

    // Walk every port slot in the new net.
    for slot in p.subnet.ports.iter() {
        if let PortRef::AgentPort(id, _port) = slot {
            // No reference to the old range [10, 14) must remain.
            assert!(
                !(10..14).contains(id),
                "stale AgentPort reference to old id {} found after remap",
                id
            );
        }
    }

    // Specifically check the internal wire: agent 200 aux1 ↔ agent 201 aux1.
    let p200_aux1_idx = 200 * PORTS_PER_SLOT + 1;
    let p201_aux1_idx = 201 * PORTS_PER_SLOT + 1;
    assert_eq!(
        p.subnet.ports[p200_aux1_idx],
        PortRef::AgentPort(201, 1),
        "agent 200 aux1 must point to agent 201 aux1"
    );
    assert_eq!(
        p.subnet.ports[p201_aux1_idx],
        PortRef::AgentPort(200, 1),
        "agent 201 aux1 must point to agent 200 aux1"
    );
}

/// UT-0411-07: `FreePort` entries are bit-identical after remap (border_ids
/// are rebased separately via A3 on a distinct axis).
#[test]
fn remap_leaves_freeports_unchanged() {
    let p0 = make_p0();
    let new_range = IdRange {
        start: 200,
        end: 204,
    };
    let p = remap_partition_ids(p0, new_range).expect("remap must succeed");

    // FreePort(42) was on agent 10 aux2 (port 2).
    // After remap, agent 10 → 200, so the port is at 200 aux2.
    let p200_aux2_idx = 200 * PORTS_PER_SLOT + 2;
    assert_eq!(
        p.subnet.ports[p200_aux2_idx],
        PortRef::FreePort(42),
        "FreePort(42) must be unchanged after remap"
    );
}

/// UT-0411-08: Undersized `new_range` returns `NewRangeTooSmall`.
/// `p0` is consumed (Rust ownership), but the error is returned immediately.
#[test]
fn remap_too_small_range_error() {
    let p0 = make_p0(); // 4 agents
    let undersized = IdRange {
        start: 200,
        end: 202,
    }; // size = 2 < 4

    let result = remap_partition_ids(p0, undersized);

    match result {
        Err(PartitionError::NewRangeTooSmall {
            partition_size,
            range_size,
        }) => {
            assert_eq!(partition_size, 4, "partition_size must be 4");
            assert_eq!(range_size, 2, "range_size must be 2");
        }
        other => panic!("expected NewRangeTooSmall, got {:?}", other),
    }
}

/// UT-0411-09: Remap with same range as current ids is idempotent.
#[test]
fn remap_idempotent_when_range_equals_existing_ids() {
    let p0 = make_p0();
    let same_range = IdRange { start: 10, end: 14 };

    let p = remap_partition_ids(p0.clone(), same_range).expect("idempotent remap must succeed");

    // Agent count and ids must be identical.
    assert_eq!(
        p.subnet.count_live_agents(),
        p0.subnet.count_live_agents(),
        "agent count must be unchanged"
    );

    let mut orig_ids: Vec<u32> = p0
        .subnet
        .agents
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|_| i as u32))
        .collect();
    orig_ids.sort();

    let mut new_ids: Vec<u32> = p
        .subnet
        .agents
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|_| i as u32))
        .collect();
    new_ids.sort();

    assert_eq!(
        orig_ids, new_ids,
        "ids must be identical after idempotent remap"
    );
}

// ---------------------------------------------------------------------------
// A4 — edge cases
// ---------------------------------------------------------------------------

/// EC-2: Empty partition — remap succeeds, returns empty partition.
#[test]
fn remap_empty_partition() {
    let empty = Partition {
        subnet: Net::new(),
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 0 },
        border_id_start: 0,
        border_id_end: 0,
    };

    let new_range = IdRange { start: 0, end: 0 };
    let result = remap_partition_ids(empty, new_range);

    let p = result.expect("empty partition remap must succeed");
    assert_eq!(p.subnet.count_live_agents(), 0, "result must be empty");
}

/// EC-3: `new_range` strictly larger than partition — first N ids used;
/// remainder is silently unused. No error.
#[test]
fn remap_oversized_range_uses_only_needed_ids() {
    let p0 = make_p0(); // 4 agents
    let oversized = IdRange {
        start: 500,
        end: 600,
    }; // size 100 > 4

    let p = remap_partition_ids(p0, oversized).expect("oversized range must succeed");

    // Must still have exactly 4 live agents.
    assert_eq!(p.subnet.count_live_agents(), 4);

    // All agent ids must be in [500, 504) — only the first 4 ids used.
    let mut new_ids: Vec<u32> = p
        .subnet
        .agents
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.as_ref().map(|_| i as u32))
        .collect();
    new_ids.sort();
    assert_eq!(new_ids, vec![500, 501, 502, 503]);
}

// ---------------------------------------------------------------------------
// Stage 6 REFACTOR — new tests (RV-001/QA-001..QA-006, EC-A/B/C/E/F/G/H/I/J/L)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// EC-G — RV-001/QA-001: cursor supplied to PartitionPlan prevents collision
// ---------------------------------------------------------------------------

/// EC-G: A `PartitionPlan` built with a known cursor (simulating the
/// `reconstruct_partition_plan_from_collected` fix) does NOT collide with
/// border IDs already in the borders map.
///
/// Demonstrates the RV-001/QA-001 invariant: after any reconstruction path
/// that preserves existing border IDs, `allocate_border_ids` must yield
/// an ID range disjoint from the existing ones.
#[test]
fn ec_g_reconstruct_cursor_prevents_border_id_collision() {
    // Simulate a split that consumed border IDs 0, 1, 2.
    // next_border_id after split == 3 (== wire_class.border_id_end).
    let cursor_after_split: u32 = 3;

    // Build a plan that reflects the post-split state: borders 0..2 are live.
    let mut borders = std::collections::HashMap::new();
    borders.insert(0u32, (PortRef::FreePort(0), PortRef::FreePort(1)));
    borders.insert(1u32, (PortRef::FreePort(2), PortRef::FreePort(3)));
    borders.insert(2u32, (PortRef::FreePort(4), PortRef::FreePort(5)));

    // The "correct" reconstruction: cursor is set to the post-split value.
    let mut plan = PartitionPlan {
        partitions: vec![],
        borders,
        next_border_id: cursor_after_split,
    };

    // Allocate one fresh border ID. Must be disjoint from {0, 1, 2}.
    let r = plan
        .allocate_border_ids(1)
        .expect("allocation on properly-cursored plan must succeed");

    // The allocated range must not overlap with the already-live border IDs.
    for existing_id in [0u32, 1, 2] {
        assert!(
            !r.contains(&existing_id),
            "allocate_border_ids returned id {} which is already in the borders map (EC-G / RV-001)",
            existing_id,
        );
    }

    // Specifically: the first fresh id must be >= cursor_after_split.
    assert_eq!(
        r.start, cursor_after_split,
        "fresh allocation must start at cursor_after_split={}; got {}..{} (EC-G)",
        cursor_after_split, r.start, r.end,
    );
}

// ---------------------------------------------------------------------------
// EC-E — QA-004: empty partition remap with non-zero-start new_range
// ---------------------------------------------------------------------------

/// EC-E: `remap_partition_ids` on an empty partition with `new_range.start > 0`
/// must set `subnet.next_id = new_range.start`, not 0.
///
/// This tests the QA-004 fix: the fast-path used `Net::new()` (next_id=0),
/// violating the documented postcondition `next_id = new_range.start + agent_count`.
#[test]
fn ec_e_empty_partition_remap_sets_correct_next_id() {
    let empty = Partition {
        subnet: Net::new(),
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 0 },
        border_id_start: 0,
        border_id_end: 0,
    };

    let new_range = IdRange {
        start: 500,
        end: 600,
    };
    let result = remap_partition_ids(empty, new_range).expect("empty remap must succeed");

    assert_eq!(
        result.id_range.start, 500,
        "id_range.start must equal new_range.start (EC-E)"
    );
    assert_eq!(
        result.subnet.next_id, 500,
        "subnet.next_id must equal new_range.start (=500) after empty remap; \
         got {} — postcondition `next_id = new_range.start + agent_count` violated (EC-E / QA-004)",
        result.subnet.next_id,
    );
    assert_eq!(
        result.subnet.count_live_agents(),
        0,
        "result must have zero live agents (EC-E)"
    );
}

// ---------------------------------------------------------------------------
// EC-F — QA-002: empty partition remap clears stale free_port_index
// ---------------------------------------------------------------------------

/// EC-F: `remap_partition_ids` on a malformed empty partition whose
/// `free_port_index` has a stale `AgentPort` entry must produce a result
/// with an empty `free_port_index`.
///
/// This tests the QA-002 fix: the fast-path used `..partition` which would
/// preserve any stale entries byte-identical, leaving dangling references.
#[test]
fn ec_f_empty_partition_remap_clears_stale_free_port_index() {
    // Malformed: 0 live agents but a dangling AgentPort(99, 1) in the index.
    let mut malformed_index = HashMap::new();
    malformed_index.insert(42u32, PortRef::AgentPort(99, 1));

    let malformed = Partition {
        subnet: Net::new(), // 0 live agents
        worker_id: 0,
        free_port_index: malformed_index,
        id_range: IdRange { start: 0, end: 0 },
        border_id_start: 0,
        border_id_end: 1,
    };

    let new_range = IdRange {
        start: 100,
        end: 100,
    };
    let result = remap_partition_ids(malformed, new_range).expect("empty remap must succeed");

    assert!(
        result.free_port_index.is_empty(),
        "free_port_index must be empty after empty-partition remap; \
         stale AgentPort entries must not survive (EC-F / QA-002)"
    );
}

// ---------------------------------------------------------------------------
// EC-H — QA-003: debug_assert for ghost AgentPort in port array is tested
//         via normal-path: well-formed partition → no panic, no stale refs
// ---------------------------------------------------------------------------

/// EC-H: After `remap_partition_ids` on a well-formed partition, no port slot
/// should reference an agent id from the *old* range.
///
/// This test exercises the port-rewrite loop path where `id_map.get()` always
/// succeeds (well-formed input). A ghost AgentPort(999, _) would be caught by
/// the debug_assert in debug builds; this UT proves the well-formed path is clean.
#[test]
fn ec_h_remap_produces_no_stale_agent_port_references() {
    let p0 = make_p0(); // agents at ids [10, 11, 12, 13]
    let new_range = IdRange {
        start: 300,
        end: 304,
    };

    let result = remap_partition_ids(p0, new_range).expect("remap must succeed");

    // After remap, no port slot should reference old range [10, 14).
    for (idx, slot) in result.subnet.ports.iter().enumerate() {
        if let PortRef::AgentPort(id, _) = slot {
            assert!(
                !(10u32..14).contains(id),
                "port[{}] references stale old-range id {}; all AgentPort \
                 targets must be in new range [300, 304) (EC-H / QA-003)",
                idx,
                id,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// EC-I — QA-005: IdRange::len on valid empty range; debug_assert for inverted
//         verified indirectly (tested in debug builds via len/is_empty)
// ---------------------------------------------------------------------------

/// EC-I: `IdRange::len` and `is_empty` on a normal empty range (`start == end`)
/// return the correct values without panicking.
///
/// The `debug_assert!` for inverted ranges (`start > end`) fires only in debug
/// builds and would require `#[should_panic]`; we instead verify that the
/// *correct* boundary (`start == end`) is handled cleanly.
#[test]
fn ec_i_idrange_empty_boundary_correct() {
    let empty_range = IdRange { start: 42, end: 42 };
    assert_eq!(
        empty_range.len(),
        0,
        "len() of empty range (start==end) must be 0 (EC-I)"
    );
    assert!(
        empty_range.is_empty(),
        "is_empty() of empty range (start==end) must be true (EC-I)"
    );

    // Verify the boundary between empty and non-empty.
    let single = IdRange { start: 42, end: 43 };
    assert_eq!(
        single.len(),
        1,
        "len() of single-element range must be 1 (EC-I)"
    );
    assert!(
        !single.is_empty(),
        "is_empty() of single-element range must be false (EC-I)"
    );
}

// ---------------------------------------------------------------------------
// EC-J — QA-006: redex_queue with a well-formed pair is preserved after remap
// ---------------------------------------------------------------------------

/// EC-J: A `redex_queue` entry whose both sides are live in the partition
/// is preserved (remapped) after `remap_partition_ids`.
///
/// This tests the symmetric-pair path of the `debug_assert!` added in QA-006:
/// for a well-formed input both sides are live, the assert does not fire, and
/// the pair is remapped correctly.
#[test]
fn ec_j_redex_queue_symmetric_pair_is_remapped() {
    // Build a 2-agent partition with one redex pair (both principals connected).
    let arena_len = 2;
    let mut agents = vec![None; arena_len];
    let mut ports = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

    agents[0] = Some(Agent {
        symbol: Symbol::Con,
        id: 0,
    });
    agents[1] = Some(Agent {
        symbol: Symbol::Era,
        id: 1,
    });
    // Principal ports connected to each other (the redex).
    ports[0] = PortRef::AgentPort(1, 0); // agent 0 principal → agent 1 principal
    ports[PORTS_PER_SLOT] = PortRef::AgentPort(0, 0); // agent 1 principal → agent 0 principal

    let subnet = Net {
        agents,
        ports,
        redex_queue: VecDeque::from([(0u32, 1u32)]),
        next_id: 2,
        root: None,
        freeport_redirects: HashMap::new(),
        free_list: Vec::new(),
        id_range: None,
        border_entries_shadow: None,
        recycle_policy: relativist_core::net::core::RecyclePolicy::DisableUnderDelta,
        is_in_delta_round: false,
        #[cfg(debug_assertions)]
        protected_tombstones: None,
        #[cfg(debug_assertions)]
        free_list_pops: 0,
        #[cfg(debug_assertions)]
        free_list_pops_border: 0,
        #[cfg(debug_assertions)]
        free_list_pops_non_border: 0,
    };

    let p = Partition {
        subnet,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 2 },
        border_id_start: 0,
        border_id_end: 0,
    };

    let new_range = IdRange {
        start: 1000,
        end: 1002,
    };
    let result = remap_partition_ids(p, new_range).expect("remap must succeed");

    // The redex queue must contain the remapped pair.
    assert_eq!(
        result.subnet.redex_queue.len(),
        1,
        "redex_queue must still have one entry after remap (EC-J)"
    );
    let (ra, rb) = result.subnet.redex_queue[0];
    // Both ids must be in the new range.
    assert!(
        (1000..1002).contains(&ra) && (1000..1002).contains(&rb),
        "remapped redex pair ({}, {}) must be in new range [1000, 1002) (EC-J)",
        ra,
        rb,
    );
    // The pair must be a valid flip of (0, 1) → ({1000 or 1001}, {1000 or 1001}).
    assert_ne!(ra, rb, "redex pair elements must be distinct (EC-J)");
}

// ---------------------------------------------------------------------------
// EC-A / EC-B / EC-C — boundary UTs for allocate_border_ids near u32::MAX
// ---------------------------------------------------------------------------

/// EC-A: `allocate_border_ids(0)` at exactly `cursor == u32::MAX - 1`.
/// Zero-count is a no-op; cursor unchanged; empty range returned.
#[test]
fn ec_a_allocate_zero_at_max_minus_one_cursor() {
    let cursor = u32::MAX - 1;
    let mut plan = make_plan_with_cursor(cursor);

    let r = plan
        .allocate_border_ids(0)
        .expect("zero-count at MAX-1 cursor must not error (EC-A)");

    assert!(
        r.is_empty(),
        "zero-count allocation must return empty range (EC-A)"
    );

    // Cursor unchanged: next real allocation still starts at MAX-1.
    let next = plan
        .allocate_border_ids(1)
        .expect("allocation of 1 at MAX-1 cursor must succeed (EC-A)");
    assert_eq!(
        next,
        (u32::MAX - 1)..u32::MAX,
        "allocation after no-op must start at the unchanged cursor (EC-A)"
    );
}

/// EC-B: `allocate_border_ids(0)` at exactly `cursor == u32::MAX`.
/// Zero-count is always a no-op — even when cursor is fully exhausted.
#[test]
fn ec_b_allocate_zero_at_fully_exhausted_cursor() {
    let cursor = u32::MAX;
    let mut plan = make_plan_with_cursor(cursor);

    let r = plan
        .allocate_border_ids(0)
        .expect("zero-count at MAX cursor must not error (EC-B)");

    assert!(
        r.is_empty(),
        "zero-count at MAX cursor must return empty range (EC-B)"
    );
    assert_eq!(
        r,
        u32::MAX..u32::MAX,
        "empty range at MAX must be MAX..MAX (EC-B)"
    );
}

/// EC-C: `allocate_border_ids(1)` at exactly `cursor == u32::MAX`.
/// `available = u32::MAX - u32::MAX = 0`; requesting 1 must return
/// `BorderIdSpaceExhausted { requested: 1, available: 0 }`.
#[test]
fn ec_c_allocate_one_at_fully_exhausted_cursor() {
    let cursor = u32::MAX;
    let mut plan = make_plan_with_cursor(cursor);

    let result = plan.allocate_border_ids(1);

    match result {
        Err(PartitionError::BorderIdSpaceExhausted {
            requested,
            available,
        }) => {
            assert_eq!(requested, 1, "reported requested must be 1 (EC-C)");
            assert_eq!(
                available, 0,
                "reported available must be 0 (not u32::MAX) when cursor == u32::MAX (EC-C)"
            );
        }
        other => panic!("expected BorderIdSpaceExhausted, got {:?} (EC-C)", other),
    }
}

// ---------------------------------------------------------------------------
// EC-L — boundary: new_range.start + agent_count, postcondition on next_id
// ---------------------------------------------------------------------------

/// EC-L: `remap_partition_ids` sets `subnet.next_id = new_range.start + agent_count`
/// (the documented postcondition) even when `new_range.start` is large.
///
/// This test uses a moderate but non-trivial `new_range.start` value (1_000_000)
/// to verify the postcondition without allocating a multi-GiB arena.
/// (Placing agents at `u32::MAX - 2` would require a 4 GiB Vec — not a valid
/// unit test; see SS-2 stress scenario in QA-TASK-0411.)
#[test]
fn ec_l_remap_next_id_postcondition_at_large_range_start() {
    let large_start: u32 = 1_000_000;

    let arena_len = 2;
    let mut agents = vec![None; arena_len];
    agents[0] = Some(Agent {
        symbol: Symbol::Con,
        id: 0,
    });
    agents[1] = Some(Agent {
        symbol: Symbol::Con,
        id: 1,
    });

    let subnet = Net {
        agents,
        ports: vec![DISCONNECTED; arena_len * PORTS_PER_SLOT],
        redex_queue: VecDeque::new(),
        next_id: 2,
        root: None,
        freeport_redirects: HashMap::new(),
        free_list: Vec::new(),
        id_range: None,
        border_entries_shadow: None,
        recycle_policy: relativist_core::net::core::RecyclePolicy::DisableUnderDelta,
        is_in_delta_round: false,
        #[cfg(debug_assertions)]
        protected_tombstones: None,
        #[cfg(debug_assertions)]
        free_list_pops: 0,
        #[cfg(debug_assertions)]
        free_list_pops_border: 0,
        #[cfg(debug_assertions)]
        free_list_pops_non_border: 0,
    };

    let p = Partition {
        subnet,
        worker_id: 0,
        free_port_index: HashMap::new(),
        id_range: IdRange { start: 0, end: 2 },
        border_id_start: 0,
        border_id_end: 0,
    };

    let new_range = IdRange {
        start: large_start,
        end: large_start + 100, // more than enough for 2 agents
    };

    let result =
        remap_partition_ids(p, new_range).expect("remap with large start must succeed (EC-L)");

    // next_id postcondition: new_range.start + agent_count = 1_000_000 + 2 = 1_000_002.
    assert_eq!(
        result.subnet.next_id,
        large_start + 2,
        "subnet.next_id must equal new_range.start + agent_count after remap; \
         got {} (EC-L postcondition)",
        result.subnet.next_id,
    );

    assert_eq!(
        result.id_range.start, large_start,
        "id_range.start must equal new_range.start (EC-L)"
    );
}
