//! Streaming generators for the benchmark suite (SPEC-21 §3.2, R10-R16).
//!
//! This module provides:
//! - [`default_chunked_iter`]: wraps an eager `Net` into a single-batch stream
//!   (the R10 default-impl path; memory-equivalent to v1 but API-compatible).
//! - [`ep_annihilation_stream`]: native streaming for ERA-ERA pairs (R12 MUST).
//! - [`dual_tree_stream`]: native streaming for `dual_tree` with forward
//!   references (R12 SHOULD, R14).
//! - [`r15_monotonicity_checked`] / [`R15MonotonicityChecker`]: debug-mode
//!   wrapper that asserts R15 monotonicity across batches (TASK-0544).
//!
//! **Pure-Core constraint (R9 / R16):** This module has NO async, NO tokio, NO I/O.
//! The iterators are synchronous; integration with async channels is the
//! coordinator's responsibility.
//!
//! # Post-dispatch monotonicity discipline (§3.5 closing note)
//!
//! R15 is a **generator-phase** contract only. Code in this module MUST NOT
//! assert monotonicity on agents created post-dispatch (e.g., by worker arenas
//! running SPEC-22 free-list recycling). The post-dispatch monotonicity-assumed
//! patterns (`assert!(new_id > old_max_id)`) are FORBIDDEN here per SPEC-22 §3.8
//! A6 and the §3.5 closing note (closes SC-009).

use crate::net::{AgentId, Net, PortRef, Symbol};
use crate::partition::streaming::{AgentBatch, ConnectionDirective};

// ---------------------------------------------------------------------------
// default_chunked_iter (SPEC-21 R10 default-impl path)
// ---------------------------------------------------------------------------

/// Wraps an eagerly-materialized `Net` into a single-batch `AgentBatch` stream.
///
/// This is the R10 **default-impl path**: the entire net is materialized upfront
/// (by the caller's `make_net(size)` call), then wrapped in one `AgentBatch`
/// containing all agents. All connection directives are `Resolved` (no forward
/// references arise from a fully-materialized net where all agents are known).
///
/// # Memory note
///
/// The default path forfeits the memory benefit of streaming — it holds the full
/// net in the single batch. The benefit is unlocked only by per-generator native
/// overrides (e.g., `ep_annihilation_stream`, `dual_tree_stream`).
///
/// # Chunk-size argument
///
/// For the default impl, `chunk_size` is **ignored** — the net is always emitted
/// as a single batch. This matches the R10 spec note ("the default implementation
/// wraps R11 via a single-batch wrapper"). The argument exists only for API
/// uniformity with native streaming overrides.
///
/// # Determinism
///
/// Agents are walked in `AgentId` order (indices 0..agents.len()), skipping
/// `None` (tombstoned) slots. The resulting batch is deterministic.
pub fn default_chunked_iter(net: Net) -> Box<dyn Iterator<Item = AgentBatch>> {
    // Collect all live agents in id order.
    let agents: Vec<(AgentId, Symbol)> = net
        .agents
        .iter()
        .enumerate()
        .filter_map(|(id, slot)| slot.as_ref().map(|agent| (id as AgentId, agent.symbol)))
        .collect();

    // Collect all resolved wires: walk port array and emit each wire once
    // (only emit when source_id < target_id to avoid duplicates).
    let mut connections: Vec<ConnectionDirective> = Vec::new();
    let ports_per_slot = crate::net::PORTS_PER_SLOT;
    for id in 0..net.agents.len() {
        if net.agents[id].is_none() {
            continue;
        }
        let num_ports = net.agents[id]
            .as_ref()
            .map_or(0u8, |a| crate::net::total_ports(a.symbol));
        for port in 0..num_ports {
            let idx = id * ports_per_slot + port as usize;
            if idx >= net.ports.len() {
                break;
            }
            match net.ports[idx] {
                PortRef::AgentPort(tgt_id, tgt_port) => {
                    // Emit only when id < tgt_id to avoid duplicating wires.
                    if (id as AgentId) < tgt_id {
                        connections.push(ConnectionDirective::Resolved {
                            source: (id as AgentId, port),
                            target: (tgt_id, tgt_port),
                        });
                    }
                }
                PortRef::FreePort(fp_id) => {
                    // QA-D010-003: emit FreePortInterface so Lafont interface
                    // wires are preserved through the streaming pipeline.
                    // (Pre-fix this branch was a silent drop, breaking T6
                    // isomorphism for any net with an interface.)
                    //
                    // DISCONNECTED sentinel (`FreePort(u32::MAX)`) is skipped
                    // because it represents an erased / disconnected port,
                    // not a real Lafont interface wire (see net::DISCONNECTED).
                    if fp_id != u32::MAX {
                        connections.push(ConnectionDirective::FreePortInterface {
                            agent_port: (id as AgentId, port),
                            free_port_id: fp_id,
                        });
                    }
                }
            }
        }
    }

    let batch = AgentBatch {
        agents,
        connections,
    };
    Box::new(std::iter::once(batch))
}

// ---------------------------------------------------------------------------
// ep_annihilation_stream (SPEC-21 R12 MUST, R13 informative)
// ---------------------------------------------------------------------------

/// Streaming generator for ERA-ERA annihilation pairs (SPEC-21 R12 MUST).
///
/// Emits batches of independent ERA-ERA pairs. Per R13 (informative), each
/// pair consists of two ERA agents connected at principal ports (port 0 ↔ port 0).
/// No cross-batch wires exist, so all directives are `Resolved` (R13: no forward
/// references).
///
/// # Pair-batching discipline (§4.5)
///
/// Each batch contains an integer number of pairs. `pairs_per_batch =
/// chunk_size / 2` (minimum 1 to avoid zero-length batches). A batch may
/// have fewer agents if `size` is not divisible by `pairs_per_batch`.
///
/// # R15 (generator-phase monotonicity)
///
/// Agent IDs are assigned as `2k` and `2k+1` for pair `k` (0-based).
/// The sequence is globally unique and strictly increasing across batches.
///
/// # Parameters
///
/// - `size`: number of ERA-ERA pairs (total agents = `2 * size`).
/// - `chunk_size`: target batch size in agents (minimum 2 = 1 pair; rounded down
///   to nearest even number to avoid splitting a pair across batches).
pub fn ep_annihilation_stream(
    size: u32,
    chunk_size: usize,
) -> Box<dyn Iterator<Item = AgentBatch>> {
    // Pairs per batch: floor(chunk_size / 2), minimum 1.
    let pairs_per_batch = (chunk_size / 2).max(1);

    let iter = EpAnnihilationIter {
        size,
        pairs_per_batch,
        next_pair: 0,
    };
    Box::new(iter)
}

struct EpAnnihilationIter {
    size: u32,
    pairs_per_batch: usize,
    next_pair: u32,
}

impl Iterator for EpAnnihilationIter {
    type Item = AgentBatch;

    fn next(&mut self) -> Option<AgentBatch> {
        if self.next_pair >= self.size {
            return None;
        }
        let batch_pairs = (self.pairs_per_batch as u32).min(self.size - self.next_pair);
        let batch_agents_start = self.next_pair * 2;

        let agents: Vec<(AgentId, Symbol)> = (0..batch_pairs)
            .flat_map(|i| {
                let pair_start = batch_agents_start + i * 2;
                [(pair_start, Symbol::Era), (pair_start + 1, Symbol::Era)]
            })
            .collect();

        let connections: Vec<ConnectionDirective> = (0..batch_pairs)
            .map(|i| {
                let pair_start = batch_agents_start + i * 2;
                ConnectionDirective::Resolved {
                    source: (pair_start, 0u8),
                    target: (pair_start + 1, 0u8),
                }
            })
            .collect();

        self.next_pair += batch_pairs;

        Some(AgentBatch {
            agents,
            connections,
        })
    }
}

// ---------------------------------------------------------------------------
// dual_tree_stream (SPEC-21 R12 SHOULD, R14 forward references)
// ---------------------------------------------------------------------------

/// Streaming generator for the `dual_tree` benchmark (SPEC-21 R12 SHOULD, R14).
///
/// Generates a balanced binary tree pair: two mirrored trees of CON agents
/// connected root-to-root. The generation order is bottom-up: leaves first,
/// then internal nodes level-by-level, root last.
///
/// # Forward references (R14)
///
/// Child nodes connect to parent nodes. When a child is emitted in batch `k`,
/// its parent may not yet exist (it lives in batch `k+j` for some `j > 0`).
/// The child batch carries `Pending` directives for the parent-child wires;
/// the pending store resolves them when the parent's batch arrives.
///
/// # R15 monotonicity
///
/// Agent IDs are assigned sequentially starting from 0. The dual tree uses
/// two separate subtrees; IDs are globally unique and monotonically increasing.
///
/// # Parameters
///
/// - `depth`: tree depth (total agents per tree = `2^depth - 1`; leaves = `2^(depth-1)`).
/// - `chunk_size`: target batch size; batches are filled by BFS layer order.
///
/// # R37g pending lifetime bound
///
/// For `dual_tree(depth)` with chunk_size `c`, the maximum pending lifetime
/// (chunks between Pending emission and resolution) is
/// `ceil(2^(depth-1) / c)` — the number of chunks needed to exhaust the leaf
/// layer. For `depth ≤ 16` and `c ≥ 1`, this is at most `2^15 = 32768` chunks.
/// Operators MUST ensure `GridConfig.max_pending_lifetime` is large enough for
/// their chosen (depth, chunk_size) combination. For `depth ≤ 10` with default
/// `chunk_size = 10_000`, the bound is ≤ 1 chunk (all leaves fit in 1 batch).
pub fn dual_tree_stream(depth: u32, chunk_size: usize) -> Box<dyn Iterator<Item = AgentBatch>> {
    if depth == 0 {
        return Box::new(std::iter::empty());
    }

    // We build the full agent layout eagerly (but without connections),
    // then emit batches in BFS level order with Pending directives for
    // child→parent cross-batch wires. The "streaming" benefit here is
    // that we emit agents in bounded batches rather than materializing
    // the full Net object — the AgentBatch has a lighter memory footprint.
    //
    // For very large trees (depth > 16), the in-memory agent index may
    // be large; the default-impl path may be preferable in that case
    // (per R37g doc-comment above).
    let batches = build_dual_tree_batches(depth, chunk_size);
    Box::new(batches.into_iter())
}

/// Builds all batches for a dual tree of the given `depth`.
///
/// # ID assignment
///
/// IDs are assigned in **emission order** (not level-order) so that R15 is
/// trivially satisfied: since we number agents 0, 1, 2, … in the order we
/// emit them, max(batch k) < min(batch k+1) holds by construction.
///
/// The emission order within each tree is bottom-up: deepest level first
/// (leaves), then shallower levels, root last. Within a level, nodes are
/// emitted left-to-right (stable among siblings). The left tree is emitted
/// fully before the right tree.
///
/// We maintain a mapping `level_order_idx → emission_id` (called `id_map`)
/// so that parent/child wire references can be translated from the structural
/// level-order indices to the emission-order IDs.
///
/// # Wire encoding
///
/// Every node emits its child→parent wire at the time the child is emitted.
/// Since we go bottom-up, the parent has **not yet been emitted** when its
/// child appears, so these wires are always `Pending`. The parent is
/// identified by its emission-order ID (looked up from `id_map`).
///
/// The left-root → right-root wire is also `Pending` (right root is emitted
/// after the entire left tree).
///
/// # Root-to-root wire
///
/// Emitted as `Pending` from the left root (last node of the left tree) to
/// the right root (last node of the right tree).
fn build_dual_tree_batches(depth: u32, chunk_size: usize) -> Vec<AgentBatch> {
    if depth == 0 {
        return vec![];
    }

    let nodes_per_tree: u32 = (1u32 << depth) - 1;
    let total_nodes = 2 * nodes_per_tree as usize;

    // Compute the level of level-order index i (0-based, root=0).
    // level = floor(log2(i+1))
    let level_of = |i: u32| -> u32 {
        if i == 0 {
            return 0;
        }
        31u32 - (i + 1).leading_zeros()
    };

    // Build the bottom-up emission order for one tree: deepest level first,
    // within each level left-to-right (ascending index).
    let mut tree_emission_order: Vec<u32> = (0..nodes_per_tree).collect();
    tree_emission_order.sort_by(|&a, &b| {
        // Descending by level (deepest first), then ascending by index within level.
        level_of(b).cmp(&level_of(a)).then(a.cmp(&b))
    });

    // Assign emission IDs: left tree gets IDs 0..(nodes_per_tree-1),
    // right tree gets IDs nodes_per_tree..(2*nodes_per_tree-1),
    // both in bottom-up emission order.
    //
    // id_map[left][level_order_idx] = emission_id
    // For right tree: emission_id = id_map + nodes_per_tree
    let mut id_map: Vec<u32> = vec![0u32; nodes_per_tree as usize];
    for (emission_pos, &level_order_idx) in tree_emission_order.iter().enumerate() {
        id_map[level_order_idx as usize] = emission_pos as u32;
    }

    // Concatenate: left tree (IDs 0..N-1) then right tree (IDs N..2N-1),
    // each in bottom-up order. This gives globally monotone IDs across batches.
    let all_ordered: Vec<(AgentId, u32)> = tree_emission_order
        .iter()
        .map(|&lo_idx| {
            let eid = id_map[lo_idx as usize];
            (eid, lo_idx)
        })
        .chain(tree_emission_order.iter().map(|&lo_idx| {
            let eid = nodes_per_tree + id_map[lo_idx as usize];
            (eid, lo_idx)
        }))
        .collect();

    // Sanity: emission IDs in all_ordered should be 0..total_nodes in order.
    debug_assert!(
        all_ordered
            .iter()
            .enumerate()
            .all(|(i, (eid, _))| *eid == i as u32),
        "ID assignment must be strictly 0..total_nodes in emission order"
    );

    let mut batches: Vec<AgentBatch> = Vec::new();
    let mut global_idx = 0usize;

    while global_idx < total_nodes {
        let end = (global_idx + chunk_size).min(total_nodes);
        let batch_slice = &all_ordered[global_idx..end];

        let agents: Vec<(AgentId, Symbol)> = batch_slice
            .iter()
            .map(|&(eid, _)| (eid, Symbol::Con))
            .collect();

        let mut connections: Vec<ConnectionDirective> = Vec::new();

        // FreePort ID base: above all agent IDs (0..2*nodes_per_tree).
        // Left tree leaf FreePorts:  freeport_base + (child_lo_idx - nodes_per_tree)
        // Right tree leaf FreePorts: freeport_base + (nodes_per_tree + 1) + (child_lo_idx - nodes_per_tree)
        // This gives 2*(nodes_per_tree+1) unique IDs, all above the agent ID space,
        // safely separated from border IDs (which start at 0 and grow slowly upward).
        //
        // The defensive offset is REQUIRED for the partition::streaming path: the
        // QA-D010-004 `reserved_freeport_ids` set only pre-registers a batch's
        // FreePortInterface IDs at batch-start, so a border allocated in batch K
        // can collide with a FreePortInterface declared in batch K+M. Keeping
        // FP IDs above the agent space (and therefore above realistic border
        // counters for these benchmarks) avoids the collision empirically.
        // R37c construction-isomorphism is preserved by `nets_graph_isomorphic`
        // (which permits a FreePort bijection) on the bench harness side —
        // see `bench/isomorphism.rs`.
        let freeport_base = 2 * nodes_per_tree;
        let freeport_right_offset = nodes_per_tree + 1;

        for &(eid, lo_idx) in batch_slice {
            // Determine tree membership.
            // Left tree: emission IDs 0..(nodes_per_tree-1).
            // Right tree: emission IDs nodes_per_tree..(2*nodes_per_tree-1).
            let is_right = eid >= nodes_per_tree;
            let tree_offset_eid = if is_right { nodes_per_tree } else { 0 };
            let tree_fp_offset = if is_right { freeport_right_offset } else { 0 };

            // Child → parent wire (every non-root node).
            if lo_idx > 0 {
                let parent_lo = (lo_idx - 1) / 2;
                let parent_eid = id_map[parent_lo as usize] + tree_offset_eid;
                let is_left_child = lo_idx % 2 == 1; // odd = left child of parent
                let parent_port: u8 = if is_left_child { 1 } else { 2 };
                let child_port: u8 = 0; // principal port

                // In bottom-up order the parent always has a higher emission ID
                // than any of its descendants → parent always comes LATER.
                // So the parent is never in a previous batch; it may be in the
                // SAME batch (if chunk_size spans both child and parent).
                //
                // Check: is parent_eid also in this batch slice?
                let parent_in_batch = batch_slice.iter().any(|&(e, _)| e == parent_eid);

                if parent_in_batch {
                    // Parent emitted in same batch → Resolved.
                    connections.push(ConnectionDirective::Resolved {
                        source: (eid, child_port),
                        target: (parent_eid, parent_port),
                    });
                } else {
                    // Parent in a later batch → Pending.
                    connections.push(ConnectionDirective::Pending {
                        source: (eid, child_port),
                        target_agent_id: parent_eid,
                        target_port: parent_port,
                    });
                }
            }

            // Leaf aux-port connections (FreePortInterface directives).
            //
            // For each CON node, aux ports 1 (left child) and 2 (right child)
            // connect to children. If a child position is outside the tree
            // (child_lo_idx >= nodes_per_tree), the child is a Lafont FreePort
            // (the net interface — equivalent to a FreePort leaf in the batch
            // `dual_tree` generator). We emit a FreePortInterface directive so
            // the pipeline installs the wire directly without border allocation.
            //
            // FreePort ID formula (unique per child position per tree):
            //   freeport_base + tree_fp_offset + (child_lo_idx - nodes_per_tree)
            let left_child_lo = 2 * lo_idx + 1;
            let right_child_lo = 2 * lo_idx + 2;

            if left_child_lo >= nodes_per_tree {
                // Left aux port (port 1) connects to a leaf FreePort.
                let fp_id = freeport_base + tree_fp_offset + (left_child_lo - nodes_per_tree);
                connections.push(ConnectionDirective::FreePortInterface {
                    agent_port: (eid, 1u8),
                    free_port_id: fp_id,
                });
            }
            if right_child_lo >= nodes_per_tree {
                // Right aux port (port 2) connects to a leaf FreePort.
                let fp_id = freeport_base + tree_fp_offset + (right_child_lo - nodes_per_tree);
                connections.push(ConnectionDirective::FreePortInterface {
                    agent_port: (eid, 2u8),
                    free_port_id: fp_id,
                });
            }

            // Left root → right root wire (emitted when left root is processed).
            // Left root has lo_idx=0, emission ID = id_map[0] = nodes_per_tree-1
            // (last left-tree node in emission order, since root is emitted last).
            let left_root_eid = id_map[0]; // emission ID of the left-tree root
            let right_root_eid = nodes_per_tree + id_map[0]; // same position in right tree
            if eid == left_root_eid && !is_right {
                // Right root is in a later batch → Pending.
                connections.push(ConnectionDirective::Pending {
                    source: (left_root_eid, 0u8),
                    target_agent_id: right_root_eid,
                    target_port: 0u8,
                });
            }
        }

        batches.push(AgentBatch {
            agents,
            connections,
        });

        global_idx = end;
    }

    batches
}

// ---------------------------------------------------------------------------
// R15MonotonicityChecker (SPEC-21 R15, TASK-0544)
// ---------------------------------------------------------------------------

/// Debug-mode wrapper iterator that asserts R15 monotonicity across batches.
///
/// Wraps a `Box<dyn Iterator<Item = AgentBatch>>` and, in debug builds
/// (`#[cfg(debug_assertions)]`), asserts that each batch's minimum `AgentId`
/// is strictly greater than the previous batch's maximum `AgentId`.
///
/// In release builds, this wrapper is a zero-overhead passthrough (the
/// `last_max_id` field is compiled away and the assertion is dead code).
///
/// # Usage
///
/// ```rust
/// use relativist_core::bench::streaming::{ep_annihilation_stream, r15_monotonicity_checked};
/// let stream = r15_monotonicity_checked(ep_annihilation_stream(100, 10));
/// let batches: Vec<_> = stream.collect();
/// ```
///
/// # Scope (§3.5 closing note)
///
/// This wrapper enforces the **generator-phase** contract only. It MUST NOT
/// be applied to agents created post-dispatch (e.g., by worker arenas). See
/// the module-level doc-comment for the post-dispatch discipline.
pub struct R15MonotonicityChecker {
    inner: Box<dyn Iterator<Item = AgentBatch>>,
    // TASK-0598 (QA-D010-014): always-present counter field; the assertion body
    // that *reads* / writes it remains gated by `#[cfg(debug_assertions)]` at
    // the use site. On release the field stays at its initial `None` value;
    // `#[allow(dead_code)]` suppresses the dead-code warning on release where
    // the read/write sites are compiled out.
    #[allow(dead_code)]
    last_max_id: Option<AgentId>,
}

impl Iterator for R15MonotonicityChecker {
    type Item = AgentBatch;

    fn next(&mut self) -> Option<AgentBatch> {
        let batch = self.inner.next()?;

        #[cfg(debug_assertions)]
        {
            if !batch.agents.is_empty() {
                let min_id = batch.agents.iter().map(|(id, _)| *id).min().unwrap();
                let max_id = batch.agents.iter().map(|(id, _)| *id).max().unwrap();

                if let Some(prev_max) = self.last_max_id {
                    debug_assert!(
                        min_id > prev_max,
                        "R15 monotonicity violation: current batch min_id={} \
                         must be > previous batch max_id={}",
                        min_id,
                        prev_max
                    );
                }
                self.last_max_id = Some(max_id);
            }
        }

        Some(batch)
    }
}

/// Wraps a stream with R15 monotonicity checking (debug builds only).
///
/// In release builds this is a zero-overhead passthrough — the checker struct
/// is still constructed but the assertion body is compiled away.
pub fn r15_monotonicity_checked(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
) -> R15MonotonicityChecker {
    R15MonotonicityChecker {
        inner: stream,
        // TASK-0598: field always-present (no cfg gate).
        last_max_id: None,
    }
}

// ---------------------------------------------------------------------------
// Tests (Phase D: TASK-0540..0544)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::benchmarks::{
        cascade_cross::CascadeCross, dual_tree::DualTree, ep_annihilation::EPAnnihilation,
    };
    use crate::bench::Benchmark;

    // -----------------------------------------------------------------------
    // TEST-SPEC-0540: default_chunked_iter + Benchmark::make_net_stream
    // -----------------------------------------------------------------------

    /// UT-0540-01: Default impl path — collect stream and verify agent count
    /// matches make_net for ep_annihilation.
    #[test]
    fn default_impl_path_equivalence_ep_annihilation() {
        let b = EPAnnihilation;
        // Use make_net_stream (default impl).
        let stream = b.make_net_stream(20, 5);
        let batches: Vec<AgentBatch> = stream.collect();
        let total_agents: usize = batches.iter().map(|b| b.agents.len()).sum();
        // ep_annihilation(20) = 40 agents
        let expected_net = b.make_net(20);
        let expected_agents = expected_net.agents.iter().filter(|s| s.is_some()).count();
        assert_eq!(
            total_agents, expected_agents,
            "default impl must produce same agent count as make_net"
        );
    }

    /// UT-0540-02: Default impl path — collect stream and verify agent count
    /// matches make_net for dual_tree.
    #[test]
    fn default_impl_path_equivalence_dual_tree() {
        let b = DualTree;
        let stream = b.make_net_stream(4, 5);
        let batches: Vec<AgentBatch> = stream.collect();
        let total_agents: usize = batches.iter().map(|b| b.agents.len()).sum();
        let expected_net = b.make_net(4);
        let expected_agents = expected_net.agents.iter().filter(|s| s.is_some()).count();
        assert_eq!(
            total_agents, expected_agents,
            "default impl must produce same agent count for dual_tree"
        );
    }

    /// UT-0540-03: Default impl returns a single batch (chunk_size ignored).
    ///
    /// Uses `CascadeCross` which does not override `make_net_stream`, so it
    /// exercises the default impl path (wraps `make_net` into one batch).
    #[test]
    fn default_impl_returns_single_batch() {
        let b = CascadeCross;
        let batches: Vec<_> = b.make_net_stream(20, 5).collect();
        assert_eq!(batches.len(), 1, "default impl must return exactly 1 batch");
    }

    /// UT-0540-04: Default impl — chunk_size argument is ignored (same output for any size).
    ///
    /// Uses `CascadeCross` (no native override) to test the default impl path.
    #[test]
    fn default_impl_chunk_size_argument_ignored() {
        let b = CascadeCross;
        let batches1: Vec<_> = b.make_net_stream(20, 1).collect();
        let batches100: Vec<_> = b.make_net_stream(20, 100).collect();
        assert_eq!(batches1.len(), 1);
        assert_eq!(batches100.len(), 1);
        assert_eq!(
            batches1[0].agents.len(),
            batches100[0].agents.len(),
            "both chunk sizes must produce the same agent count"
        );
    }

    /// UT-0540-05: All existing benchmark implementations compile unchanged.
    ///
    /// This is a compile-time regression gate: if any benchmark impl broke
    /// due to the trait amendment, this test would fail to compile.
    #[test]
    fn all_existing_benchmarks_compile_unchanged() {
        use crate::bench::benchmarks::{
            cascade_cross::CascadeCross, church_add::ChurchAdd, church_mul::ChurchMul,
            church_sum_of_squares::ChurchSumOfSquares, condup_expansion::ConDupExpansion,
            erasure_propagation::ErasurePropagation, mixed_net::MixedNet, tree_sum::TreeSum,
        };
        // Calling make_net_stream exercises the default impl for each.
        // If the trait amendment had broken backward-compat, this wouldn't compile.
        let _ = CascadeCross.make_net_stream(4, 2);
        let _ = ChurchAdd.make_net_stream(2, 2);
        let _ = ChurchMul.make_net_stream(2, 2);
        let _ = ChurchSumOfSquares.make_net_stream(2, 2);
        let _ = ConDupExpansion.make_net_stream(4, 2);
        let _ = ErasurePropagation.make_net_stream(4, 2);
        let _ = MixedNet.make_net_stream(4, 2);
        let _ = TreeSum.make_net_stream(4, 2);
        let _ = EPAnnihilation.make_net_stream(4, 2);
        let _ = DualTree.make_net_stream(4, 2);
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0541: ep_annihilation_stream
    // -----------------------------------------------------------------------

    /// UT-0541-01: Every batch has even agent count (pair-batch invariant).
    #[test]
    fn pair_batch_invariant() {
        let stream = ep_annihilation_stream(20, 4);
        for batch in stream {
            assert_eq!(
                batch.agents.len() % 2,
                0,
                "every batch must have even agent count (pairs)"
            );
        }
    }

    /// UT-0541-02: No Pending directives (R13: no cross-batch wires).
    #[test]
    fn resolved_only_no_pending() {
        let stream = ep_annihilation_stream(20, 4);
        for batch in stream {
            for directive in &batch.connections {
                assert!(
                    matches!(directive, ConnectionDirective::Resolved { .. }),
                    "ep_annihilation_stream must produce only Resolved directives (R13)"
                );
            }
        }
    }

    /// UT-0541-03: R15 monotonicity across batches (max id in batch k < min in k+1).
    #[test]
    fn r15_monotonicity_across_batches() {
        let batches: Vec<_> = ep_annihilation_stream(20, 4).collect();
        for w in batches.windows(2) {
            let max_k = w[0].agents.iter().map(|(id, _)| *id).max().unwrap();
            let min_k1 = w[1].agents.iter().map(|(id, _)| *id).min().unwrap();
            assert!(
                max_k < min_k1,
                "R15: max_id in batch k ({}) must be < min_id in batch k+1 ({})",
                max_k,
                min_k1
            );
        }
    }

    /// UT-0541-04: Total agents == 2 * size.
    #[test]
    fn total_agents_eq_2x_size() {
        let size = 20u32;
        let total: usize = ep_annihilation_stream(size, 4)
            .map(|b| b.agents.len())
            .sum();
        assert_eq!(
            total,
            (2 * size) as usize,
            "ep_annihilation must produce 2*size agents"
        );
    }

    /// UT-0541-05: Each batch has exactly agents.len()/2 Resolved directives.
    #[test]
    fn each_batch_pairs_are_resolved_internally() {
        let batches: Vec<_> = ep_annihilation_stream(20, 4).collect();
        for batch in batches {
            let pairs = batch.agents.len() / 2;
            assert_eq!(
                batch.connections.len(),
                pairs,
                "each batch must have exactly agents.len()/2 Resolved directives"
            );
        }
    }

    /// UT-0541-06: Partial last batch — total still equals 2*size.
    #[test]
    fn partial_last_batch_handled() {
        let size = 7u32;
        let total: usize = ep_annihilation_stream(size, 4)
            .map(|b| b.agents.len())
            .sum();
        assert_eq!(
            total,
            (2 * size) as usize,
            "partial last batch: total must still equal 2*size"
        );
    }

    /// UT-0541-07: chunk_size=1 works (pair = 2 agents per batch).
    #[test]
    fn chunk_size_one_works() {
        let size = 10u32;
        let batches: Vec<_> = ep_annihilation_stream(size, 1).collect();
        // chunk_size=1 → 1/2 = 0 pairs_per_batch → clamped to 1 → 2 agents per batch
        assert_eq!(
            batches.len(),
            size as usize,
            "with chunk_size=1, one pair per batch = size batches"
        );
        for batch in &batches {
            assert_eq!(
                batch.agents.len(),
                2,
                "each batch has exactly 1 pair = 2 agents"
            );
        }
    }

    /// EC-1: size=0 → empty stream.
    #[test]
    fn ep_stream_size_zero_empty() {
        let batches: Vec<_> = ep_annihilation_stream(0, 4).collect();
        assert!(batches.is_empty(), "size=0 must produce empty stream");
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0542: dual_tree_stream
    // -----------------------------------------------------------------------

    /// UT-0542-03: R15 monotonicity across batches for dual_tree_stream.
    #[test]
    fn r15_monotonicity_dual_tree() {
        let batches: Vec<_> = dual_tree_stream(4, 4).collect();
        // Only check if there are at least 2 non-empty batches.
        let non_empty: Vec<_> = batches.iter().filter(|b| !b.agents.is_empty()).collect();
        for w in non_empty.windows(2) {
            let max_k = w[0].agents.iter().map(|(id, _)| *id).max().unwrap();
            let min_k1 = w[1].agents.iter().map(|(id, _)| *id).min().unwrap();
            assert!(
                max_k < min_k1,
                "R15: max_id in batch k ({}) must be < min_id in batch k+1 ({})",
                max_k,
                min_k1
            );
        }
    }

    /// dual_tree_stream produces the correct total agent count.
    #[test]
    fn dual_tree_stream_total_agent_count() {
        for depth in [2u32, 3, 4, 5] {
            let total: usize = dual_tree_stream(depth, 4).map(|b| b.agents.len()).sum();
            let expected = 2 * ((1u32 << depth) - 1) as usize;
            assert_eq!(
                total, expected,
                "dual_tree(depth={}) must produce {} total agents",
                depth, expected
            );
        }
    }

    /// dual_tree_stream with chunk_size > size emits all agents in one batch.
    #[test]
    fn dual_tree_stream_large_chunk_single_batch() {
        let depth = 3u32;
        let total_nodes = 2 * ((1u32 << depth) - 1) as usize;
        let batches: Vec<_> = dual_tree_stream(depth, total_nodes + 100).collect();
        let total_agents: usize = batches.iter().map(|b| b.agents.len()).sum();
        assert_eq!(
            total_agents, total_nodes,
            "with large chunk_size, all agents emitted"
        );
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0544: R15MonotonicityChecker
    // -----------------------------------------------------------------------

    /// R15 checker passes valid ep_annihilation stream without assertion.
    #[test]
    fn r15_checker_passes_valid_stream() {
        let stream = ep_annihilation_stream(20, 4);
        let checked = r15_monotonicity_checked(stream);
        let total: usize = checked.map(|b| b.agents.len()).sum();
        assert_eq!(total, 40, "checker must not alter agent count");
    }

    /// R15 checker detects non-monotonic batches (debug builds only).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "R15 monotonicity violation")]
    fn r15_checker_detects_violation_in_debug() {
        // Deliberately non-monotonic: batch 2 starts at 0 (same IDs as batch 1).
        let batch1 = AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)],
            connections: vec![],
        };
        let batch2_bad = AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)], // WRONG: IDs repeat
            connections: vec![],
        };
        let stream: Box<dyn Iterator<Item = AgentBatch>> =
            Box::new(vec![batch1, batch2_bad].into_iter());
        let mut checked = r15_monotonicity_checked(stream);
        let _ = checked.next(); // first batch OK
        let _ = checked.next(); // second batch → panic
    }

    /// R15 checker is a passthrough in release (no side-effects on agents).
    #[test]
    fn r15_checker_passthrough_consistency() {
        let batches_raw: Vec<_> = ep_annihilation_stream(10, 2).collect();
        let batches_checked: Vec<_> =
            r15_monotonicity_checked(ep_annihilation_stream(10, 2)).collect();
        assert_eq!(
            batches_raw.len(),
            batches_checked.len(),
            "checker must not alter batch count"
        );
        for (a, b) in batches_raw.iter().zip(batches_checked.iter()) {
            assert_eq!(
                a.agents.len(),
                b.agents.len(),
                "checker must not alter agents"
            );
        }
    }

    // ---------------------------------------------------------------------------
    // QA-D010-003: default_chunked_iter must propagate FreePort interface wires
    // (T6 isomorphism preservation).
    // ---------------------------------------------------------------------------

    /// QA-D010-003-A: a Net with a CON agent connected to 3 FreePorts (Lafont
    /// root + 2 aux interface wires) MUST yield 3 FreePortInterface directives
    /// through `default_chunked_iter`. Pre-fix this branch silently dropped
    /// every FreePort, returning 0 directives.
    #[test]
    fn qa_d010_003_a_default_chunked_iter_emits_freeport_interfaces() {
        use crate::net::Net;
        let mut net = Net::new();
        let con = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(con, 0), PortRef::FreePort(10));
        net.connect(PortRef::AgentPort(con, 1), PortRef::FreePort(11));
        net.connect(PortRef::AgentPort(con, 2), PortRef::FreePort(12));

        let batch = default_chunked_iter(net)
            .next()
            .expect("default_chunked_iter must yield one batch");

        let fp_count = batch
            .connections
            .iter()
            .filter(|d| matches!(d, ConnectionDirective::FreePortInterface { .. }))
            .count();
        assert_eq!(
            fp_count, 3,
            "QA-D010-003-A: default_chunked_iter must emit 3 FreePortInterface directives, got {fp_count}"
        );
    }

    /// QA-D010-003-B (EC): a Net with NO FreePort interface (only agent-to-agent
    /// connections) yields zero FreePortInterface directives — no spurious emission.
    #[test]
    fn qa_d010_003_b_no_freeports_yields_no_freeport_directives() {
        use crate::net::Net;
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let batch = default_chunked_iter(net)
            .next()
            .expect("default_chunked_iter must yield one batch");

        let fp_count = batch
            .connections
            .iter()
            .filter(|d| matches!(d, ConnectionDirective::FreePortInterface { .. }))
            .count();
        assert_eq!(
            fp_count, 0,
            "QA-D010-003-B: empty-FreePort net must yield zero FreePortInterface directives"
        );
    }
}
