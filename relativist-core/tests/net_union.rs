//! TASK-0410 — Integration tests for `Net::union` (SPEC-20 §3.8 A7).
//!
//! `Net::union(self, other) -> Net` is a purely structural concatenation of
//! two nets under a caller-guaranteed disjoint-`AgentId` precondition.
//! It is consumed by SPEC-20 §4.2.2 v1-mode/delta-mode departure recovery
//! step 4 to compose reclaimed partitions with surviving partitions before
//! the re-split. See ARG-006 P12 (mixed-trace recoverability) for the
//! confluence justification of the surrounding cycle.
//!
//! Test IDs map 1:1 to TEST-SPEC-0410. Test bodies drive the TDD RED
//! phase: at first they fail to compile (the `union` method does not yet
//! exist); after the GREEN phase they all pass.
//!
//! NOTE: TEST-SPEC-0410 originally referenced a `Net::empty()` alias.
//! The Stage 6 refactor (RV-001) removed that alias in favour of
//! `Net::new()`, the single zero constructor. Spec text that reads
//! `a.union(empty)` is realised in code as `a.union(Net::new())`.
//!
//! `force_agent_id_range`, `compact_net`, and `sparse_net` are
//! TEST-ONLY fixtures simulating SPEC-04 `remap_partition_ids`
//! (TASK-0411) output. DO NOT copy these helpers into production code
//! or into other test files; they bypass `Net::create_agent`'s
//! monotonic invariant. The realistic upstream caller is
//! `remap_partition_ids` (RV-002).

use std::collections::HashSet;
use std::panic;

use relativist_core::net::{Agent, AgentId, Net, PortRef, Symbol};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// `net_a`: 2 CON agents with `AgentId` values `[0, 1]`, one internal wire
/// (a.aux1 ↔ b.aux1), one border `FreePort(10)` on a.aux2.
/// Root is `AgentPort(0, 0)` so UT-0410-06 can assert root-follows-self.
fn build_net_a() -> Net {
    let mut net = Net::new();
    let a0 = net.create_agent(Symbol::Con);
    let a1 = net.create_agent(Symbol::Con);
    assert_eq!(a0, 0);
    assert_eq!(a1, 1);

    net.connect(PortRef::AgentPort(a0, 1), PortRef::AgentPort(a1, 1));
    net.connect(PortRef::AgentPort(a0, 2), PortRef::FreePort(10));
    net.root = Some(PortRef::AgentPort(0, 0));
    net
}

/// `net_b`: 3 agents (CON, DUP, ERA) with `AgentId` values `[100, 101, 102]`,
/// two internal wires (b0.aux1 ↔ b1.aux1, b0.aux2 ↔ b1.aux2), two border
/// `FreePort(20)` and `FreePort(21)` on b1.principal and b2.principal
/// respectively. Root is `AgentPort(100, 0)`. AgentIds are forced into the
/// `[100..103]` range by direct construction (see `force_agent_id_range`)
/// because `create_agent` only emits sequential IDs.
fn build_net_b() -> Net {
    let mut net = Net::new();
    force_agent_id_range(&mut net, 100, &[Symbol::Con, Symbol::Dup, Symbol::Era]);

    net.connect(PortRef::AgentPort(100, 1), PortRef::AgentPort(101, 1));
    net.connect(PortRef::AgentPort(100, 2), PortRef::AgentPort(101, 2));
    net.connect(PortRef::AgentPort(101, 0), PortRef::FreePort(20));
    net.connect(PortRef::AgentPort(102, 0), PortRef::FreePort(21));
    net.root = Some(PortRef::AgentPort(100, 0));
    net
}

/// Builds a small "collision" net that contains `AgentId == 5` so it
/// overlaps any net containing the same id. Used by UT-0410-07 to drive
/// the debug-only precondition check.
fn build_collision_net_with_id_5() -> Net {
    let mut net = Net::new();
    force_agent_id_range(&mut net, 5, &[Symbol::Era]);
    net
}

/// Helper that places agents at specific (non-sequential) `AgentId`
/// positions in the arena. Mimics what `remap_partition_ids` (TASK-0411)
/// will do when SPEC-20 §4.2.2 runs the departure recovery cycle: a
/// partition is renumbered into a disjoint range before being unioned.
///
/// Direct field manipulation is fine here because the test fixture is
/// outside the `Net` API surface and emits a structurally valid Net
/// (agents[id] = Some(Agent { id, ..}); ports of that agent are sized
/// correctly; no wires set yet — caller does that).
fn force_agent_id_range(net: &mut Net, base: AgentId, symbols: &[Symbol]) {
    use relativist_core::net::{port_index, DISCONNECTED, PORTS_PER_SLOT};
    let max_id = base + symbols.len() as u32;
    if (net.agents.len() as u32) < max_id {
        net.agents.resize(max_id as usize, None);
    }
    let required_ports = (max_id as usize) * PORTS_PER_SLOT;
    if net.ports.len() < required_ports {
        net.ports.resize(required_ports, DISCONNECTED);
    }
    for (offset, sym) in symbols.iter().enumerate() {
        let id = base + offset as u32;
        net.agents[id as usize] = Some(Agent { symbol: *sym, id });
        for p in 0..3u8 {
            net.ports[port_index(id, p)] = DISCONNECTED;
        }
    }
    if net.next_id < max_id {
        net.next_id = max_id;
    }
}

/// Returns the set of live `AgentId`s in a net, for multiset comparisons
/// where ordering inside the agent arena is irrelevant.
fn live_agent_ids(net: &Net) -> HashSet<AgentId> {
    net.live_agents().map(|a| a.id).collect()
}

/// Returns the multiset (sorted Vec) of live agent symbols.
fn live_symbols_multiset(net: &Net) -> Vec<Symbol> {
    let mut v: Vec<Symbol> = net.live_agents().map(|a| a.symbol).collect();
    v.sort_by_key(|s| *s as u8);
    v
}

/// Returns all `FreePort` border ids reachable through agent ports as a
/// sorted multiset. Used by UT-0410-04 to verify that border free-ports
/// are preserved on both sides of a union.
fn collect_border_ids(net: &Net) -> Vec<u32> {
    let mut ids: Vec<u32> = Vec::new();
    for agent in net.live_agents() {
        for p in 0..3u8 {
            if let PortRef::FreePort(bid) = net.get_target(PortRef::AgentPort(agent.id, p)) {
                if bid != u32::MAX {
                    ids.push(bid);
                }
            }
        }
    }
    ids.sort_unstable();
    ids
}

/// Returns all internal `(AgentId, PortId)` ↔ `(AgentId, PortId)` wires
/// canonicalised so that the lower endpoint always appears first.
/// Each wire appears exactly once, matching UT-0410-05.
fn collect_internal_wires(net: &Net) -> Vec<((AgentId, u8), (AgentId, u8))> {
    let mut wires: HashSet<((AgentId, u8), (AgentId, u8))> = HashSet::new();
    for agent in net.live_agents() {
        for p in 0..3u8 {
            let src = (agent.id, p);
            if let PortRef::AgentPort(tid, tp) = net.get_target(PortRef::AgentPort(agent.id, p)) {
                let endpoint = (tid, tp);
                let canon = if src <= endpoint {
                    (src, endpoint)
                } else {
                    (endpoint, src)
                };
                wires.insert(canon);
            }
        }
    }
    let mut out: Vec<_> = wires.into_iter().collect();
    out.sort();
    out
}

/// QA-002 fixture variant A — compact net.
///
/// Builds a net whose live `AgentId`s are sequential starting at 0
/// (`{0..N-1}`), exactly as `Net::create_agent` would produce them.
/// Mimics the upstream contract of `remap_partition_ids` (SPEC-04 A4)
/// when the partition was renumbered into a contiguous low range.
fn compact_net(symbols: &[Symbol]) -> Net {
    let mut net = Net::new();
    for sym in symbols {
        net.create_agent(*sym);
    }
    net
}

/// QA-002 fixture variant B — sparse net.
///
/// Builds a net whose live `AgentId`s sit in a non-contiguous range
/// starting at `base`, while `next_id` matches the highest used id + 1
/// — the contract that `remap_partition_ids` (SPEC-04 §3.8 A4) is
/// obliged to honour. Verifies that `Net::union` survives the
/// realistic shape of its upstream caller.
fn sparse_net(base: AgentId, symbols: &[Symbol]) -> Net {
    let mut net = Net::new();
    force_agent_id_range(&mut net, base, symbols);
    net
}

// ---------------------------------------------------------------------------
// UT-0410-01 — `union(self, Net::new()) == self`
// ---------------------------------------------------------------------------

#[test]
fn ut_0410_01_union_empty_right() {
    let a = build_net_a();
    let n = a.clone().union(Net::new());

    assert_eq!(n.count_live_agents(), a.count_live_agents());
    assert_eq!(live_agent_ids(&n), live_agent_ids(&a));
    assert_eq!(live_symbols_multiset(&n), live_symbols_multiset(&a));
    assert_eq!(collect_border_ids(&n), collect_border_ids(&a));
    assert_eq!(collect_internal_wires(&n), collect_internal_wires(&a));
    assert_eq!(
        n.root, a.root,
        "root must follow self after empty-right union"
    );
    assert_eq!(n.next_id, a.next_id);
}

// ---------------------------------------------------------------------------
// UT-0410-02 — `Net::new().union(self) == self` (root follows the empty
// net's `None`; a re-split downstream is what re-establishes the root in
// SPEC-20 §4.2.2). The test asserts agent / wire / freeport equality but
// allows root to follow `self`'s self-side semantics — SPEC-20 §3.8 A7
// pins `root_follows_self`, so the right-hand `a`'s root is dropped here.
// ---------------------------------------------------------------------------

#[test]
fn ut_0410_02_union_empty_left() {
    let a = build_net_a();
    let n = Net::new().union(a.clone());

    assert_eq!(n.count_live_agents(), a.count_live_agents());
    assert_eq!(live_agent_ids(&n), live_agent_ids(&a));
    assert_eq!(live_symbols_multiset(&n), live_symbols_multiset(&a));
    assert_eq!(collect_border_ids(&n), collect_border_ids(&a));
    assert_eq!(collect_internal_wires(&n), collect_internal_wires(&a));
    // Root follows `self` (the empty Net) — empty has no root.
    assert_eq!(n.root, None, "root follows self; empty-self has no root");
    assert_eq!(n.next_id, a.next_id);
}

// ---------------------------------------------------------------------------
// UT-0410-03 — disjoint ids, agents preserved, no collision
// ---------------------------------------------------------------------------

#[test]
fn ut_0410_03_union_disjoint_ids_preserves_agents() {
    let a = build_net_a();
    let b = build_net_b();
    let n = a.clone().union(b.clone());

    assert_eq!(n.count_live_agents(), 5);

    let mut expected: HashSet<AgentId> = HashSet::new();
    expected.extend([0u32, 1, 100, 101, 102]);
    assert_eq!(live_agent_ids(&n), expected);

    let mut expected_symbols: Vec<Symbol> = Vec::new();
    expected_symbols.extend(live_symbols_multiset(&a));
    expected_symbols.extend(live_symbols_multiset(&b));
    expected_symbols.sort_by_key(|s| *s as u8);
    assert_eq!(live_symbols_multiset(&n), expected_symbols);

    assert_eq!(n.next_id, std::cmp::max(a.next_id, b.next_id));
}

// ---------------------------------------------------------------------------
// UT-0410-04 — both sides' FreePort border-ids are preserved
// ---------------------------------------------------------------------------

#[test]
fn ut_0410_04_union_preserves_freeports_from_both_sides() {
    let a = build_net_a();
    let b = build_net_b();
    let n = a.clone().union(b.clone());

    let mut expected: Vec<u32> = Vec::new();
    expected.extend(collect_border_ids(&a));
    expected.extend(collect_border_ids(&b));
    expected.sort_unstable();

    assert_eq!(collect_border_ids(&n), expected);
}

// ---------------------------------------------------------------------------
// UT-0410-05 — every internal wire from both inputs survives, no spurious
// cross-net wire materializes (resolution is `merge`'s job, not `union`'s)
// ---------------------------------------------------------------------------

#[test]
fn ut_0410_05_union_preserves_internal_wires() {
    let a = build_net_a();
    let b = build_net_b();

    let wires_a = collect_internal_wires(&a);
    let wires_b = collect_internal_wires(&b);
    let n = a.union(b);
    let wires_n = collect_internal_wires(&n);

    for w in &wires_a {
        assert!(
            wires_n.contains(w),
            "wire from net_a missing in union: {:?}",
            w
        );
    }
    for w in &wires_b {
        assert!(
            wires_n.contains(w),
            "wire from net_b missing in union: {:?}",
            w
        );
    }
    assert_eq!(
        wires_n.len(),
        wires_a.len() + wires_b.len(),
        "union introduced or dropped wires"
    );
}

// ---------------------------------------------------------------------------
// UT-0410-06 — root_follows_self
// ---------------------------------------------------------------------------

#[test]
fn ut_0410_06_union_root_port_follows_self() {
    let a = build_net_a();
    let b = build_net_b();
    assert_eq!(a.root, Some(PortRef::AgentPort(0, 0)));
    assert_eq!(b.root, Some(PortRef::AgentPort(100, 0)));

    let n = a.clone().union(b);
    assert_eq!(n.root, a.root, "root must follow self per SPEC-20 §3.8 A7");
}

// ---------------------------------------------------------------------------
// UT-0410-07 — debug-only panic on overlapping ids
// ---------------------------------------------------------------------------

#[cfg(debug_assertions)]
#[test]
fn ut_0410_07_union_panics_on_overlapping_ids_debug_build() {
    // Both nets contain AgentId == 5, so the disjointness precondition is
    // violated; in a debug build this fires the SPEC-20 A7 debug_assert.
    let mut left = Net::new();
    force_agent_id_range(&mut left, 5, &[Symbol::Con]);
    let right = build_collision_net_with_id_5();

    let result = panic::catch_unwind(panic::AssertUnwindSafe(move || {
        let _ = left.union(right);
    }));

    let err = result.expect_err("union must panic on overlapping AgentIds in debug builds");
    let msg = if let Some(s) = err.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        String::new()
    };
    assert!(
        msg.contains("SPEC-20") && msg.contains("A7"),
        "panic payload must cite SPEC-20 A7; got {:?}",
        msg
    );
    assert!(
        msg.contains("5"),
        "panic payload must mention the offending AgentId 5; got {:?}",
        msg
    );
}

// ---------------------------------------------------------------------------
// EC-1 — both inputs empty → empty
// ---------------------------------------------------------------------------

#[test]
fn ec_1_union_empty_with_empty_is_empty() {
    let n = Net::new().union(Net::new());
    assert_eq!(n.count_live_agents(), 0);
    assert_eq!(collect_border_ids(&n), Vec::<u32>::new());
    assert_eq!(n.root, None);
    assert_eq!(n.next_id, 0);
    assert_eq!(n, Net::new());
}

// ---------------------------------------------------------------------------
// EC-2 — one side has no internal wires
// ---------------------------------------------------------------------------

#[test]
fn ec_2_union_one_side_zero_wires() {
    // wire-less side: a single ERA agent at id 200 with no connections.
    let mut wireless = Net::new();
    force_agent_id_range(&mut wireless, 200, &[Symbol::Era]);

    let a = build_net_a();
    let wires_a = collect_internal_wires(&a);
    let n = a.union(wireless);
    let wires_n = collect_internal_wires(&n);

    assert_eq!(
        wires_n, wires_a,
        "wireless union must preserve net_a's wires exactly with no additions"
    );
}

// ---------------------------------------------------------------------------
// EC-3 — both sides share a `border_id` (e.g. both have `FreePort(10)`).
// `union` is structural; the duplicate appears as two distinct entries in
// the resulting net's border-id multiset.
// ---------------------------------------------------------------------------

#[test]
fn ec_3_union_duplicate_border_ids_preserved() {
    let a = build_net_a(); // border 10 on (0, 2)
    let mut clash = Net::new();
    force_agent_id_range(&mut clash, 50, &[Symbol::Con]);
    clash.connect(PortRef::AgentPort(50, 0), PortRef::FreePort(10));

    let n = a.clone().union(clash);
    let mut bids = collect_border_ids(&n);
    bids.sort_unstable();
    assert_eq!(
        bids,
        vec![10, 10],
        "duplicate border id 10 must survive on both sides; merge() resolves later"
    );
}

// ---------------------------------------------------------------------------
// EC-4 (QA-001) — `next_id` at `u32::MAX` panics in debug builds.
//
// `Net::union` returns `next_id = max(self.next_id, other.next_id)`. If
// that maximum is `u32::MAX`, the next `Net::create_agent` call would
// wrap `next_id` to `0` and reuse `AgentId 0`, silently breaching D4.
// SPEC-20 §3.8 A7 makes this a caller-side precondition; `Net::union`
// surfaces violations as a `debug_assert!` panic.
// ---------------------------------------------------------------------------

#[cfg(debug_assertions)]
#[test]
fn ec_4_union_panics_on_u32_max_next_id_debug_build() {
    // `left` has next_id = u32::MAX (no live agents — we just push the
    // counter to the boundary). `right` is the empty net. The merged
    // next_id would be u32::MAX, which would wrap on the next
    // create_agent call.
    let mut left = Net::new();
    left.next_id = u32::MAX;
    let right = Net::new();

    let result = panic::catch_unwind(panic::AssertUnwindSafe(move || {
        let _ = left.union(right);
    }));

    let err = result.expect_err("union must panic on u32::MAX next_id in debug builds");
    let msg = if let Some(s) = err.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = err.downcast_ref::<String>() {
        s.clone()
    } else {
        String::new()
    };
    assert!(
        msg.contains("u32::MAX"),
        "panic payload must cite the u32::MAX boundary; got {:?}",
        msg
    );
    assert!(
        msg.contains("D4") || msg.contains("SPEC-20"),
        "panic payload must cite the breached invariant or spec; got {:?}",
        msg
    );
}

// ---------------------------------------------------------------------------
// EC-5 (QA-003) — `freeport_redirects` border-id collision.
//
// `freeport_redirects` is keyed by `border_id (u32)`, NOT by `AgentId`,
// so the disjoint-`AgentId` precondition does NOT prevent collisions.
// The Stage 6 refactor pinned the policy: under `HashMap::extend`,
// `other` wins. SPEC-20 §3.8 A7 documents that this is acceptable
// because the next `split()` call (SPEC-04) re-allocates fresh border
// ids; the collision is transient.
// ---------------------------------------------------------------------------

#[test]
fn ec_5_union_freeport_redirects_collision_other_wins() {
    // We build two nets with disjoint AgentIds but with a colliding
    // freeport_redirects key (border id 10).
    //
    // To populate freeport_redirects, we use `connect(FreePort, FreePort)`
    // — that's the only path that materialises an entry under SPEC-02
    // (see Net::connect). The "agent target" expectation in the QA brief
    // is in spirit: each side maps `10 -> <distinct other-side FreePort>`.
    let mut left = Net::new();
    left.connect(PortRef::FreePort(10), PortRef::FreePort(11));
    let mut right = Net::new();
    right.connect(PortRef::FreePort(10), PortRef::FreePort(22));

    // Sanity-check both sides actually populated the map.
    assert_eq!(
        left.freeport_redirects.get(&10),
        Some(&PortRef::FreePort(11))
    );
    assert_eq!(
        right.freeport_redirects.get(&10),
        Some(&PortRef::FreePort(22))
    );

    let n = left.union(right);

    // Pinned semantics: `HashMap::extend` makes `other` win on key
    // collision. The right-hand value (FreePort(22)) survives.
    assert_eq!(
        n.freeport_redirects.get(&10),
        Some(&PortRef::FreePort(22)),
        "SPEC-20 §3.8 A7: freeport_redirects collision is resolved by \
         HashMap::extend (other wins); split() will re-allocate fresh \
         border ids downstream"
    );

    // The non-colliding right-side key 22 is preserved (right wrote
    // 22 -> FreePort(10) into its own map). The left-side key 11 is
    // also preserved (no collision).
    assert_eq!(n.freeport_redirects.get(&11), Some(&PortRef::FreePort(10)));
    assert_eq!(n.freeport_redirects.get(&22), Some(&PortRef::FreePort(10)));
}

// ---------------------------------------------------------------------------
// EC-6 (QA-006) — redex_queue concatenation order is self-then-other.
// Determinism (T1-T4) holds because the queue's processing order is
// reduction-strategy-defined, not insertion-order-defined; the order
// is documented for diagnostic reproducibility, not for semantics.
// ---------------------------------------------------------------------------

#[test]
fn ec_6_union_redex_queue_self_then_other_order() {
    // Build two nets with one redex each, and disjoint AgentIds.
    let mut left = Net::new();
    let l0 = left.create_agent(Symbol::Con);
    let l1 = left.create_agent(Symbol::Dup);
    left.connect(PortRef::AgentPort(l0, 0), PortRef::AgentPort(l1, 0));

    let mut right = Net::new();
    force_agent_id_range(&mut right, 100, &[Symbol::Con, Symbol::Era]);
    right.connect(PortRef::AgentPort(100, 0), PortRef::AgentPort(101, 0));

    assert_eq!(left.redex_queue.len(), 1, "left must have one redex");
    assert_eq!(right.redex_queue.len(), 1, "right must have one redex");
    let left_redex = left.redex_queue[0];
    let right_redex = right.redex_queue[0];

    let n = left.union(right);
    let queue: Vec<_> = n.redex_queue.iter().copied().collect();
    assert_eq!(
        queue,
        vec![left_redex, right_redex],
        "redex_queue must be concatenated in self-then-other order"
    );
}

// ---------------------------------------------------------------------------
// QA-002 fixture coverage — exercise `Net::union` with the realistic
// shapes that `remap_partition_ids` (TASK-0411) is contracted to emit.
// These complement UT-0410-03..06 (which use the legacy build_net_*
// fixtures) by stress-testing the two canonical shapes:
//
//   a) compact_net — sequential `AgentId`s starting at 0.
//   b) sparse_net  — non-contiguous range with `next_id == max + 1`.
// ---------------------------------------------------------------------------

#[test]
fn qa_002_union_compact_with_sparse_preserves_agents_and_wires() {
    // compact: ids {0, 1} (CON, DUP), one internal wire.
    let mut compact = compact_net(&[Symbol::Con, Symbol::Dup]);
    compact.connect(PortRef::AgentPort(0, 1), PortRef::AgentPort(1, 1));

    // sparse: ids {500, 501, 502} (CON, ERA, CON), one internal wire and
    // one border free-port. `next_id` is 503 by construction.
    let mut sparse = sparse_net(500, &[Symbol::Con, Symbol::Era, Symbol::Con]);
    sparse.connect(PortRef::AgentPort(500, 1), PortRef::AgentPort(502, 2));
    sparse.connect(PortRef::AgentPort(500, 0), PortRef::FreePort(7));

    let wires_compact = collect_internal_wires(&compact);
    let wires_sparse = collect_internal_wires(&sparse);

    let n = compact.union(sparse);

    assert_eq!(n.count_live_agents(), 5);
    let mut expected: HashSet<AgentId> = HashSet::new();
    expected.extend([0u32, 1, 500, 501, 502]);
    assert_eq!(live_agent_ids(&n), expected);

    // Wires from both sides survive verbatim.
    let wires_n = collect_internal_wires(&n);
    let mut expected_wires = wires_compact;
    expected_wires.extend(wires_sparse);
    expected_wires.sort();
    let mut actual_wires = wires_n;
    actual_wires.sort();
    assert_eq!(actual_wires, expected_wires);

    // `next_id` follows max(compact.next_id=2, sparse.next_id=503).
    assert_eq!(n.next_id, 503);

    // Sparse-side border survives.
    assert_eq!(collect_border_ids(&n), vec![7u32]);
}

#[test]
fn qa_002_union_sparse_with_compact_preserves_agents_and_wires() {
    // Symmetric to the test above, but with sparse on the LEFT (root
    // follows self → root from sparse).
    let mut sparse = sparse_net(50, &[Symbol::Con, Symbol::Dup]);
    sparse.connect(PortRef::AgentPort(50, 1), PortRef::AgentPort(51, 1));
    sparse.root = Some(PortRef::AgentPort(50, 0));

    let mut compact = compact_net(&[Symbol::Era, Symbol::Con]);
    compact.connect(PortRef::AgentPort(0, 0), PortRef::AgentPort(1, 0));
    compact.root = Some(PortRef::AgentPort(0, 0));

    let sparse_root = sparse.root;

    let n = sparse.union(compact);

    assert_eq!(n.count_live_agents(), 4);
    let mut expected: HashSet<AgentId> = HashSet::new();
    expected.extend([0u32, 1, 50, 51]);
    assert_eq!(live_agent_ids(&n), expected);

    // Root follows self (sparse).
    assert_eq!(n.root, sparse_root);

    // next_id = max(52, 2) = 52.
    assert_eq!(n.next_id, 52);
}
