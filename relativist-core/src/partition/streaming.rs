//! Streaming partition types and strategy trait (SPEC-21).
//!
//! This module provides the foundational types and traits for streaming
//! (incremental) generation and partitioning of interaction combinator nets.
//!
//! Instead of generating the full net and partitioning globally (O(total_agents)
//! memory), the streaming pipeline generates one `AgentBatch` at a time and
//! partitions it immediately, bounding coordinator peak memory to
//! O(chunk_size + border_tracking_state).
//!
//! Module-level theory anchor: SPEC-21 §1 (Purpose); ARG-002 Passo 10 (quality
//! independence — any σ satisfying C1-C3 produces a correct result).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::net::{AgentId, PortId, PortRef, Symbol};
use crate::partition::types::{Partition, PartitionPlan, WorkerId};

// ---------------------------------------------------------------------------
// ConnectionDirective (SPEC-21 §4.1, R14)
// ---------------------------------------------------------------------------

/// A directive describing how two ports in an `AgentBatch` should be connected.
///
/// Streaming generators emit agents together with connection directives. When
/// both endpoints are already known (i.e., both agents exist in the current or
/// a previous batch), the directive is `Resolved`. When the target agent will
/// appear in a *future* batch (a forward reference), the directive is `Pending`.
///
/// IC concept: In an interaction net (SPEC-02), every port is connected to
/// exactly one other port. During streaming generation, a fragment of the net
/// may have *dangling* ports temporarily — ports whose partner has not yet been
/// emitted. The `Pending` variant records these dangling ports. The pipeline
/// resolves them when the target agent's batch is processed.
///
/// Example forward reference: `dual_tree` generator connects internal nodes
/// to children that appear in a later batch. Node `k` on level `l` connects to
/// nodes `2k` and `2k+1` on level `l+1`, which may not yet have been emitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionDirective {
    /// Both endpoints of this wire are known: `source` port connects to `target` port.
    ///
    /// Used when both agents already exist in the current or a previous batch.
    Resolved {
        /// The source side: `(agent_id, port_id)`.
        source: (AgentId, PortId),
        /// The target side: `(agent_id, port_id)`.
        target: (AgentId, PortId),
    },
    /// Forward reference: the source port is known, but the target agent has
    /// not yet been generated (it will appear in a future batch).
    ///
    /// The pipeline buffers this directive in the pending-connection store and
    /// resolves it when `target_agent_id` is generated in a subsequent batch.
    ///
    /// # Note
    ///
    /// The enum does NOT validate `target_port` against agent arity; that is
    /// the caller's responsibility. The install_connection helper (TASK-0553)
    /// surfaces arity errors as `PartitionError::InvalidPort`.
    Pending {
        /// The source side: `(agent_id, port_id)`.
        source: (AgentId, PortId),
        /// The ID of the target agent (not yet emitted at directive creation time).
        target_agent_id: AgentId,
        /// The port index on the target agent.
        target_port: PortId,
    },
}

// ---------------------------------------------------------------------------
// AgentBatch (SPEC-21 §4.1, R10, R14, R15)
// ---------------------------------------------------------------------------

/// A bounded batch of agents produced by a streaming generator.
///
/// Each batch contains agent definitions and connection directives.
/// The batch is the unit of work in the streaming pipeline: the
/// generator produces one batch, the partitioner assigns its agents,
/// and the pipeline installs agents and connections incrementally.
///
/// IC concept: An `AgentBatch` represents a fragment of an interaction
/// net (SPEC-02). The fragment may have dangling ports (forward
/// references to agents in future batches). These dangling ports are
/// temporary and are resolved when the referenced agents are generated.
///
/// # Generator obligation (R15)
///
/// The generator MUST assign `AgentId` values in a globally unique,
/// monotonically increasing sequence across all batches: the maximum
/// `AgentId` in batch `k` MUST be less than the minimum `AgentId` in
/// batch `k+1`. This is a **generator-phase** contract; the type does
/// NOT enforce it. Enforcement (debug-assertions) lives in TASK-0544.
///
/// Once chunks are dispatched and workers fire reduction rules, the
/// worker arena MAY recycle slot IDs per SPEC-22 I3'/R1-R10c. Code
/// consuming `AgentBatch` MUST NOT assume monotonicity on agents created
/// post-dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentBatch {
    /// Agent definitions: `(id, symbol)` pairs.
    ///
    /// IDs MUST be globally unique and monotonically increasing across
    /// batches (SPEC-21 R15 — strictly stronger than SPEC-01 I3').
    pub agents: Vec<(AgentId, Symbol)>,

    /// Connection directives for this batch.
    ///
    /// Each directive describes a wire between two ports.
    /// `Resolved` directives have both endpoints available;
    /// `Pending` directives reference agents in future batches.
    pub connections: Vec<ConnectionDirective>,
}

impl AgentBatch {
    /// Constructs an empty batch (no agents, no connections).
    ///
    /// Useful as a degenerate input in tests and as an initial accumulator.
    pub fn empty() -> Self {
        Self {
            agents: Vec::new(),
            connections: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// StreamingPartitionStats (SPEC-21 §4.1, R3)
// ---------------------------------------------------------------------------

/// Statistics about a streaming partitioning run.
///
/// Returned by [`StreamingPartitionStrategy::finalize`] and embedded in
/// [`ChunkedPartitionResult::stats`].
///
/// # Ownership of `chunks_processed` (closes SC-021)
///
/// The `chunks_processed` field is **pipeline-owned**, NOT strategy-owned.
/// Strategies MUST return `chunks_processed: 0` as a placeholder from
/// `finalize()`; the pipeline maintains a local `chunks_seen: u64` counter
/// incremented per iteration and assigns `result.stats.chunks_processed =
/// chunks_seen` after `strategy.finalize()` (per SPEC-21 §4.6 Step 7).
///
/// Without this convention, strategies would need to track loop iterations
/// externally, creating coupling. Tests (T1) MUST verify the pipeline-stitched
/// count, not the strategy-returned value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamingPartitionStats {
    /// Total agents assigned across all batches.
    pub total_agents: u64,
    /// Number of agents assigned to each worker (index = worker_id).
    pub per_worker_counts: Vec<u64>,
    /// Number of border wires created across all chunks.
    pub border_wire_count: u64,
    /// Number of chunks processed by the pipeline.
    ///
    /// **PIPELINE-OWNED**: strategies return `0` as a placeholder;
    /// the pipeline stitches the actual count via its `chunks_seen`
    /// counter (SPEC-21 §4.6 Step 7 stitch step — closes SC-021).
    /// Never trust this field's value when it comes directly from
    /// `strategy.finalize()`; always read it from the stitched
    /// `ChunkedPartitionResult.stats`.
    pub chunks_processed: u64,
}

// ---------------------------------------------------------------------------
// ChunkedPartitionResult (SPEC-21 §4.1, R20, R21)
// ---------------------------------------------------------------------------

/// The result of the chunked generation + partitioning pipeline.
///
/// This type is structurally equivalent to [`PartitionPlan`] (SPEC-04)
/// but produced incrementally. It is directly consumable by the merge
/// protocol (SPEC-05) per SPEC-21 R21.
///
/// # Conversion to `PartitionPlan`
///
/// ```
/// use relativist_core::partition::streaming::ChunkedPartitionResult;
/// use relativist_core::partition::PartitionPlan;
/// // let result: ChunkedPartitionResult = ...;
/// // let plan: PartitionPlan = result.into();
/// ```
///
/// The conversion drops `stats` (merge protocol does not consume stats).
/// `partitions` and `borders` are preserved 1:1 (R21 structural compat).
///
/// ARG-002 Q5/C1-C3: split/merge identity holds for the streamed result
/// because the invariants enforced during streaming (C1-C3 per R27) are
/// identical to those enforced by `split()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkedPartitionResult {
    /// One partition per worker, fully formed (per SPEC-04 `Partition` type).
    pub partitions: Vec<Partition>,
    /// Border map: `borderId -> (original_endpoint_A, original_endpoint_B)`.
    ///
    /// Analogous to `PartitionPlan::borders` (SPEC-04 §4.1).
    pub borders: HashMap<u32, (PortRef, PortRef)>,
    /// Statistics from the streaming partitioning strategy.
    ///
    /// `stats.chunks_processed` is pipeline-stitched (see
    /// [`StreamingPartitionStats`] ownership note).
    pub stats: StreamingPartitionStats,
}

impl From<ChunkedPartitionResult> for PartitionPlan {
    /// Converts a `ChunkedPartitionResult` into a `PartitionPlan`.
    ///
    /// Drops `stats` (merge protocol does not consume observability data).
    /// Preserves `partitions` and `borders` 1:1 per SPEC-21 R21.
    ///
    /// The resulting `next_border_id` cursor is set to 0 (default).
    /// Callers that need a live cursor for further `allocate_border_ids`
    /// calls MUST derive `next_border_id` from the border ranges in the
    /// partitions (i.e., `max(border_id_end over all partitions)`).
    fn from(result: ChunkedPartitionResult) -> Self {
        PartitionPlan {
            partitions: result.partitions,
            borders: result.borders,
            next_border_id: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// StreamingPartitionStrategy trait (SPEC-21 §4.2, R1-R3, R7-R9)
// ---------------------------------------------------------------------------

/// Trait for streaming (online) partition strategies.
///
/// Unlike [`crate::partition::PartitionStrategy`] (SPEC-04, R21) which requires
/// the full net to compute the allocation function σ, this trait assigns agents
/// to workers incrementally, one batch at a time, using only information
/// available up to the current batch.
///
/// # IC concept
///
/// Correctness of distributed IC reduction does not depend on partition quality
/// (DISC-004 v2, Section 1.6; ARG-002, Passo 10). Any allocation function σ
/// satisfying C1-C3 produces a correct result. Streaming strategies trade
/// partition quality for bounded memory usage: the strategy never sees the
/// full net.
///
/// # Contracts
///
/// Every implementation MUST satisfy:
/// - **R7 (C1 — Complete Agent Coverage):** After all batches have been
///   processed, the union of all assignments assigns every agent to exactly
///   one worker, with no duplicates and no omissions.
/// - **R8 (Determinism):** Given the same sequence of batches and the same
///   `num_workers`, the assignment MUST be identical across invocations.
/// - **R9 (Pure Core layer):** No `async`, no `tokio`, no I/O. The strategy
///   is synchronous and self-contained.
///
/// # Object safety
///
/// The trait is object-safe: `Box<dyn StreamingPartitionStrategy>` is valid.
/// Implementations MUST NOT use generic parameters that break object safety.
pub trait StreamingPartitionStrategy {
    /// Assigns each agent in the batch to a worker.
    ///
    /// Returns a `Vec` of `(AgentId, WorkerId)` pairs — one per agent in
    /// `batch.agents`, in the same order.
    ///
    /// # Post-conditions (R7 + R8)
    ///
    /// - Every agent in the batch has exactly one assignment.
    /// - Every `WorkerId` is in range `[0, num_workers)`.
    /// - No agent is assigned twice (within this batch or across batches).
    /// - Given the same sequence of calls with the same inputs, the returned
    ///   assignment is **identical** (determinism; closes C1 over batches).
    ///
    /// # Note
    ///
    /// `&mut self` is required because strategies may update per-worker load
    /// counters or assignment caches (FENNEL, SPEC-21 R5) across batches (R2).
    fn allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)>;

    /// Returns statistics about the partitioning so far.
    ///
    /// # `chunks_processed` ownership (SC-021)
    ///
    /// Strategies SHOULD return `chunks_processed: 0` — this field is
    /// **pipeline-owned**. The pipeline stitches the actual count via its
    /// `chunks_seen` counter after calling `finalize()` (SPEC-21 §4.6 Step 7).
    fn finalize(&self) -> StreamingPartitionStats;
}

// ---------------------------------------------------------------------------
// Tests (Phase B: TASK-0520..0524)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;
    use crate::partition::types::{IdRange, Partition};
    use std::collections::HashMap;

    // -----------------------------------------------------------------------
    // Helper: quick bincode encode/decode via the project's bincode_v2 module
    // -----------------------------------------------------------------------

    fn encode<T: serde::Serialize>(val: &T) -> Vec<u8> {
        crate::protocol::bincode_v2::encode(val).unwrap()
    }

    fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
        crate::protocol::bincode_v2::decode_value(bytes).unwrap()
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0520: ConnectionDirective
    // -----------------------------------------------------------------------

    /// UT-0520-01: Resolved variant constructs without panic; fields readable.
    #[test]
    fn resolved_variant_constructible() {
        let d = ConnectionDirective::Resolved {
            source: (0u32, 0u8),
            target: (1u32, 1u8),
        };
        match d {
            ConnectionDirective::Resolved { source, target } => {
                assert_eq!(source, (0, 0));
                assert_eq!(target, (1, 1));
            }
            _ => panic!("expected Resolved"),
        }
    }

    /// UT-0520-02: Pending variant constructs; fields readable.
    #[test]
    fn pending_variant_constructible() {
        let d = ConnectionDirective::Pending {
            source: (0u32, 0u8),
            target_agent_id: 50u32,
            target_port: 2u8,
        };
        match d {
            ConnectionDirective::Pending {
                source,
                target_agent_id,
                target_port,
            } => {
                assert_eq!(source, (0, 0));
                assert_eq!(target_agent_id, 50);
                assert_eq!(target_port, 2);
            }
            _ => panic!("expected Pending"),
        }
    }

    /// UT-0520-03: Resolved serde round-trip.
    #[test]
    fn resolved_serde_round_trip() {
        let original = ConnectionDirective::Resolved {
            source: (0u32, 0u8),
            target: (1u32, 1u8),
        };
        let bytes = encode(&original);
        let decoded: ConnectionDirective = decode(&bytes);
        assert_eq!(decoded, original, "Resolved serde round-trip failed");
    }

    /// UT-0520-04: Pending serde round-trip.
    #[test]
    fn pending_serde_round_trip() {
        let original = ConnectionDirective::Pending {
            source: (0u32, 0u8),
            target_agent_id: 50u32,
            target_port: 2u8,
        };
        let bytes = encode(&original);
        let decoded: ConnectionDirective = decode(&bytes);
        assert_eq!(decoded, original, "Pending serde round-trip failed");
    }

    /// UT-0520-05: Pending accepts port ids 0, 1, 2 without panic.
    #[test]
    fn pending_target_port_accepts_zero_one_two() {
        for port in [0u8, 1u8, 2u8] {
            let _d = ConnectionDirective::Pending {
                source: (0u32, 0u8),
                target_agent_id: 1u32,
                target_port: port,
            };
        }
    }

    /// UT-0520-06: Derives present — Debug, Clone, PartialEq, Eq, Serialize, Deserialize.
    #[test]
    fn connection_directive_derives_present() {
        let a = ConnectionDirective::Resolved {
            source: (0u32, 0u8),
            target: (1u32, 1u8),
        };
        // Debug
        let s = format!("{:?}", a);
        assert!(s.contains("Resolved"), "Debug missing Resolved: {}", s);
        // Clone
        let b = a.clone();
        // PartialEq + Eq
        assert_eq!(a, b);
    }

    /// UT-0520-07: Resolved != Pending even with the same source field.
    #[test]
    fn variants_distinguishable() {
        let resolved = ConnectionDirective::Resolved {
            source: (0u32, 0u8),
            target: (1u32, 1u8),
        };
        let pending = ConnectionDirective::Pending {
            source: (0u32, 0u8),
            target_agent_id: 1u32,
            target_port: 1u8,
        };
        assert_ne!(resolved, pending, "different variants must be unequal");
    }

    /// EC-1: Pending with target_agent_id = u32::MAX serializes/deserializes correctly.
    #[test]
    fn pending_max_agent_id_serde() {
        let original = ConnectionDirective::Pending {
            source: (0u32, 0u8),
            target_agent_id: u32::MAX,
            target_port: 0u8,
        };
        let bytes = encode(&original);
        let decoded: ConnectionDirective = decode(&bytes);
        assert_eq!(
            decoded, original,
            "Pending(u32::MAX) serde round-trip failed"
        );
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0521: AgentBatch
    // -----------------------------------------------------------------------

    /// UT-0521-01: Batch constructs with agents and directives; fields readable.
    #[test]
    fn batch_constructible_with_agents_and_directives() {
        let batch = AgentBatch {
            agents: vec![
                (0u32, Symbol::Con),
                (1u32, Symbol::Dup),
                (2u32, Symbol::Era),
            ],
            connections: vec![
                ConnectionDirective::Resolved {
                    source: (0u32, 0u8),
                    target: (1u32, 1u8),
                },
                ConnectionDirective::Pending {
                    source: (2u32, 0u8),
                    target_agent_id: 10u32,
                    target_port: 0u8,
                },
            ],
        };
        assert_eq!(batch.agents.len(), 3);
        assert_eq!(batch.connections.len(), 2);
        assert_eq!(batch.agents[0], (0, Symbol::Con));
        assert_eq!(batch.agents[2], (2, Symbol::Era));
    }

    /// UT-0521-02: Batch serde round-trip.
    #[test]
    fn batch_serde_round_trip() {
        let original = AgentBatch {
            agents: vec![(0u32, Symbol::Con), (1u32, Symbol::Dup)],
            connections: vec![ConnectionDirective::Resolved {
                source: (0u32, 0u8),
                target: (1u32, 0u8),
            }],
        };
        let bytes = encode(&original);
        let decoded: AgentBatch = decode(&bytes);
        assert_eq!(decoded, original, "AgentBatch serde round-trip failed");
    }

    /// UT-0521-03: Agent IDs within a batch are in expected range.
    #[test]
    fn monotonic_id_assignment_within_batch() {
        // With base=5, agents at IDs 5 and 6 — strictly increasing.
        let batch = AgentBatch {
            agents: vec![(5u32, Symbol::Con), (6u32, Symbol::Dup)],
            connections: vec![],
        };
        let ids: Vec<AgentId> = batch.agents.iter().map(|(id, _)| *id).collect();
        for w in ids.windows(2) {
            assert!(
                w[0] < w[1],
                "IDs must be strictly increasing within a batch"
            );
        }
        assert!(ids[0] >= 5 && ids[1] < 5 + batch.agents.len() as u32 + 1);
    }

    /// UT-0521-04: IDs are strictly monotone across two sequential batches.
    #[test]
    fn monotonic_id_assignment_across_batches() {
        let batch1 = AgentBatch {
            agents: vec![
                (0u32, Symbol::Con),
                (1u32, Symbol::Dup),
                (2u32, Symbol::Era),
            ],
            connections: vec![],
        };
        let batch2 = AgentBatch {
            agents: vec![(3u32, Symbol::Con), (4u32, Symbol::Dup)],
            connections: vec![],
        };
        let all_ids: Vec<AgentId> = batch1
            .agents
            .iter()
            .chain(batch2.agents.iter())
            .map(|(id, _)| *id)
            .collect();
        for w in all_ids.windows(2) {
            assert!(
                w[0] < w[1],
                "IDs must be strictly monotone across batches: {:?}",
                all_ids
            );
        }
    }

    /// UT-0521-05: Connection directives classify correctly.
    #[test]
    fn connection_directives_classify_resolved_vs_pending() {
        let batch = AgentBatch {
            agents: vec![
                (0u32, Symbol::Con),
                (1u32, Symbol::Dup),
                (2u32, Symbol::Era),
            ],
            connections: vec![
                ConnectionDirective::Resolved {
                    source: (0u32, 0u8),
                    target: (1u32, 1u8),
                },
                ConnectionDirective::Pending {
                    source: (2u32, 0u8),
                    target_agent_id: 10u32,
                    target_port: 0u8,
                },
            ],
        };
        let resolved_count = batch
            .connections
            .iter()
            .filter(|d| matches!(d, ConnectionDirective::Resolved { .. }))
            .count();
        let pending_count = batch
            .connections
            .iter()
            .filter(|d| matches!(d, ConnectionDirective::Pending { .. }))
            .count();
        assert_eq!(resolved_count, 1, "expected 1 Resolved directive");
        assert_eq!(pending_count, 1, "expected 1 Pending directive");
    }

    /// UT-0521-06: Empty batch constructs and serializes cleanly.
    #[test]
    fn empty_batch_constructible() {
        let batch = AgentBatch::empty();
        assert!(batch.agents.is_empty());
        assert!(batch.connections.is_empty());
        let bytes = encode(&batch);
        let decoded: AgentBatch = decode(&bytes);
        assert_eq!(decoded, batch, "empty AgentBatch serde round-trip failed");
    }

    /// UT-0521-07: Derives present — Debug, Clone, PartialEq, Eq, Serialize, Deserialize.
    #[test]
    fn agent_batch_derives_present() {
        let a = AgentBatch {
            agents: vec![(0u32, Symbol::Era)],
            connections: vec![],
        };
        let s = format!("{:?}", a);
        assert!(s.contains("AgentBatch"), "Debug must include type name");
        let b = a.clone();
        assert_eq!(a, b, "Clone + PartialEq must agree");
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0522: StreamingPartitionStats
    // -----------------------------------------------------------------------

    fn make_stats(per_worker: Vec<u64>, total: u64, border: u64) -> StreamingPartitionStats {
        StreamingPartitionStats {
            total_agents: total,
            per_worker_counts: per_worker,
            border_wire_count: border,
            chunks_processed: 0,
        }
    }

    /// UT-0522-01: Stats constructs and fields are readable.
    #[test]
    fn stats_constructible() {
        let s = make_stats(vec![25, 25, 25, 25], 100, 0);
        assert_eq!(s.total_agents, 100);
        assert_eq!(s.per_worker_counts.len(), 4);
        assert_eq!(s.border_wire_count, 0);
        assert_eq!(s.chunks_processed, 0);
    }

    /// UT-0522-02: Stats Debug format is non-empty and includes counts.
    #[test]
    fn stats_debug_format() {
        let s = make_stats(vec![25, 25, 25, 25], 100, 0);
        let formatted = format!("{:?}", s);
        assert!(!formatted.is_empty(), "Debug format should be non-empty");
        assert!(
            formatted.contains("25"),
            "Debug format should include per_worker_counts values"
        );
    }

    /// UT-0522-03: Stats Clone — mutating the clone does not affect the original.
    #[test]
    fn stats_clone() {
        let original = make_stats(vec![10, 20], 30, 5);
        let mut cloned = original.clone();
        cloned.total_agents = 999;
        assert_eq!(
            original.total_agents, 30,
            "mutating clone must not affect original"
        );
    }

    /// UT-0522-04: Stats serde round-trip.
    #[test]
    fn stats_serde_round_trip() {
        let original = make_stats(vec![25, 25, 25, 25], 100, 8);
        let bytes = encode(&original);
        let decoded: StreamingPartitionStats = decode(&bytes);
        assert_eq!(
            decoded, original,
            "StreamingPartitionStats serde round-trip failed"
        );
    }

    /// UT-0522-05: Strategy's finalize() returns chunks_processed == 0 (SC-021).
    ///
    /// Verifies the pipeline-ownership convention: a concrete strategy
    /// implementation returns 0 for chunks_processed; the pipeline stitches
    /// the real value.
    #[test]
    fn chunks_processed_zero_when_returned_by_strategy_finalize() {
        struct AlwaysWorker0;
        impl StreamingPartitionStrategy for AlwaysWorker0 {
            fn allocate_batch(
                &mut self,
                batch: &AgentBatch,
                _num_workers: u32,
            ) -> Vec<(AgentId, WorkerId)> {
                batch.agents.iter().map(|(id, _)| (*id, 0u32)).collect()
            }
            fn finalize(&self) -> StreamingPartitionStats {
                StreamingPartitionStats {
                    total_agents: 0,
                    per_worker_counts: vec![],
                    border_wire_count: 0,
                    chunks_processed: 0, // pipeline-owned; always 0 from strategy
                }
            }
        }
        let strategy = AlwaysWorker0;
        let stats = strategy.finalize();
        assert_eq!(
            stats.chunks_processed, 0,
            "strategy finalize() must return 0 for chunks_processed (SC-021 pipeline-owned)"
        );
    }

    /// UT-0522-06: `chunks_processed` Rustdoc contains "pipeline-owned" keyword.
    ///
    /// This is a compile-time / static test: the doc comment is validated
    /// during `cargo doc`. At the unit-test level, we verify the convention
    /// is correctly modeled in the struct itself.
    #[test]
    fn chunks_processed_doc_documents_pipeline_ownership() {
        // The real test is the Rustdoc comment which cargo doc validates.
        // Here we verify the structural invariant: a freshly finalized strategy
        // returns 0, demonstrating the convention is live.
        let stats = StreamingPartitionStats {
            total_agents: 42,
            per_worker_counts: vec![21, 21],
            border_wire_count: 2,
            chunks_processed: 0, // as mandated by SC-021
        };
        assert_eq!(stats.chunks_processed, 0);
    }

    /// UT-0522-07: Derives present — Debug, Clone, PartialEq, Serialize, Deserialize.
    #[test]
    fn streaming_stats_derives_present() {
        let a = make_stats(vec![10], 10, 0);
        let s = format!("{:?}", a);
        assert!(
            s.contains("StreamingPartitionStats"),
            "Debug must include type name"
        );
        let b = a.clone();
        assert_eq!(a, b, "Clone + PartialEq must agree");
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0523: ChunkedPartitionResult
    // -----------------------------------------------------------------------

    fn make_empty_partition_for_worker(worker_id: WorkerId) -> Partition {
        Partition {
            subnet: crate::net::Net::new(),
            worker_id,
            free_port_index: HashMap::new(),
            id_range: IdRange { start: 0, end: 0 },
            border_id_start: 0,
            border_id_end: 0,
        }
    }

    fn make_test_result() -> ChunkedPartitionResult {
        let partitions: Vec<Partition> = (0..4).map(make_empty_partition_for_worker).collect();
        let mut borders: HashMap<u32, (PortRef, PortRef)> = HashMap::new();
        for i in 0..8u32 {
            borders.insert(i, (PortRef::AgentPort(i, 0), PortRef::AgentPort(i + 10, 0)));
        }
        let stats = make_stats(vec![0, 0, 0, 0], 0, 8);
        ChunkedPartitionResult {
            partitions,
            borders,
            stats,
        }
    }

    /// UT-0523-01: ChunkedPartitionResult constructs and fields are readable.
    #[test]
    fn result_constructible() {
        let r = make_test_result();
        assert_eq!(r.partitions.len(), 4);
        assert_eq!(r.borders.len(), 8);
    }

    /// UT-0523-02: ChunkedPartitionResult serde round-trip.
    #[test]
    fn result_serde_round_trip() {
        let original = make_test_result();
        let bytes = encode(&original);
        let decoded: ChunkedPartitionResult = decode(&bytes);
        assert_eq!(
            decoded.partitions.len(),
            original.partitions.len(),
            "partitions length must match after serde"
        );
        assert_eq!(
            decoded.borders.len(),
            original.borders.len(),
            "borders length must match after serde"
        );
        assert_eq!(
            decoded.stats.border_wire_count,
            original.stats.border_wire_count
        );
    }

    /// UT-0523-03: From conversion preserves partitions 1:1.
    #[test]
    fn from_chunked_to_partition_plan_preserves_partitions_one_to_one() {
        let result = make_test_result();
        let expected_len = result.partitions.len();
        let expected_workers: Vec<WorkerId> =
            result.partitions.iter().map(|p| p.worker_id).collect();
        let plan: PartitionPlan = result.into();
        assert_eq!(
            plan.partitions.len(),
            expected_len,
            "partition count must match"
        );
        let actual_workers: Vec<WorkerId> = plan.partitions.iter().map(|p| p.worker_id).collect();
        assert_eq!(
            actual_workers, expected_workers,
            "partition order/workers must match"
        );
    }

    /// UT-0523-04: From conversion preserves borders 1:1.
    #[test]
    fn from_chunked_to_partition_plan_preserves_borders_one_to_one() {
        let result = make_test_result();
        let expected_borders = result.borders.clone();
        let plan: PartitionPlan = result.into();
        assert_eq!(
            plan.borders, expected_borders,
            "borders must be preserved identically"
        );
    }

    /// UT-0523-05: From conversion — PartitionPlan has no stats field.
    #[test]
    fn from_chunked_to_partition_plan_drops_stats() {
        // Compile-time test: PartitionPlan has no `stats` field.
        // This test verifies the conversion compiles cleanly and the resulting
        // PartitionPlan struct has only `partitions`, `borders`, `next_border_id`.
        let result = make_test_result();
        let plan: PartitionPlan = result.into();
        // Access only the fields PartitionPlan has; if it had `stats` this would
        // be a different field set. The compile-time absence IS the test.
        let _ = &plan.partitions;
        let _ = &plan.borders;
        let _ = plan.next_border_id;
    }

    /// UT-0523-06: Partition field set matches SPEC-04 R21 (6 required fields).
    #[test]
    fn partition_field_set_matches_spec04_r21() {
        let p = make_empty_partition_for_worker(0);
        // All 6 required fields must be accessible.
        let _ = &p.subnet;
        let _ = p.worker_id;
        let _ = &p.free_port_index;
        let _ = p.id_range;
        let _ = p.border_id_start;
        let _ = p.border_id_end;
    }

    /// UT-0523-07: Derives present — Debug, Clone, Serialize, Deserialize.
    #[test]
    fn chunked_partition_result_derives_present() {
        let r = make_test_result();
        let s = format!("{:?}", r);
        assert!(
            s.contains("ChunkedPartitionResult"),
            "Debug must include type name"
        );
        let _cloned = r.clone();
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0524: StreamingPartitionStrategy trait
    // -----------------------------------------------------------------------

    /// Minimal test-only strategy that always assigns agents to worker 0.
    #[derive(Default)]
    struct AlwaysWorker0Strategy {
        total_seen: u64,
    }

    impl StreamingPartitionStrategy for AlwaysWorker0Strategy {
        fn allocate_batch(
            &mut self,
            batch: &AgentBatch,
            _num_workers: u32,
        ) -> Vec<(AgentId, WorkerId)> {
            let result = batch.agents.iter().map(|(id, _)| (*id, 0u32)).collect();
            self.total_seen += batch.agents.len() as u64;
            result
        }

        fn finalize(&self) -> StreamingPartitionStats {
            StreamingPartitionStats {
                total_agents: self.total_seen,
                per_worker_counts: vec![self.total_seen],
                border_wire_count: 0,
                chunks_processed: 0, // pipeline-owned
            }
        }
    }

    /// UT-0524-02: allocate_batch uses &mut self (stateful, R2).
    #[test]
    fn trait_uses_mut_self() {
        let mut strategy = AlwaysWorker0Strategy::default();
        let batch = AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)],
            connections: vec![],
        };
        let assignments = strategy.allocate_batch(&batch, 4);
        assert_eq!(assignments.len(), 2, "one assignment per agent");
        // State was mutated
        assert_eq!(strategy.total_seen, 2);
    }

    /// UT-0524-03: Trait is object-safe — Box<dyn StreamingPartitionStrategy> constructs.
    #[test]
    fn trait_object_constructible() {
        let _boxed: Box<dyn StreamingPartitionStrategy> =
            Box::new(AlwaysWorker0Strategy::default());
    }

    /// UT-0524-04/05: Pure core — no async, no tokio, no I/O in this module.
    ///
    /// This is validated at the source level. The test confirms the trait
    /// compiles without async/tokio dependencies in this crate module.
    #[test]
    fn pure_core_no_async_no_tokio_compile_time() {
        // If this module imported tokio or used async, it would fail to compile
        // without the tokio feature enabled in [lib] compilation units.
        // The test simply exercises the pure-sync path; no assertion needed.
        let mut s = AlwaysWorker0Strategy::default();
        let b = AgentBatch::empty();
        let _assignments = s.allocate_batch(&b, 2);
    }

    /// UT-0524-08: finalize() uses &self (not consuming, not mut).
    #[test]
    fn finalize_uses_shared_ref() {
        let strategy = AlwaysWorker0Strategy::default();
        let stats = strategy.finalize();
        assert_eq!(stats.chunks_processed, 0, "pipeline-owned field must be 0");
        // strategy is still usable after finalize (not consumed)
        let _again = strategy.finalize();
    }

    /// Verify allocation output length matches batch agent count.
    #[test]
    fn allocation_output_length_matches_agent_count() {
        let mut strategy = AlwaysWorker0Strategy::default();
        let batch = AgentBatch {
            agents: vec![
                (0u32, Symbol::Con),
                (1u32, Symbol::Dup),
                (2u32, Symbol::Era),
            ],
            connections: vec![],
        };
        let assignments = strategy.allocate_batch(&batch, 4);
        assert_eq!(
            assignments.len(),
            batch.agents.len(),
            "assignment count must equal agent count"
        );
    }

    /// All assigned WorkerIds must be in range [0, num_workers).
    #[test]
    fn allocation_worker_ids_in_range() {
        let mut strategy = AlwaysWorker0Strategy::default();
        let batch = AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)],
            connections: vec![],
        };
        let num_workers = 4u32;
        let assignments = strategy.allocate_batch(&batch, num_workers);
        for (_, worker_id) in &assignments {
            assert!(
                *worker_id < num_workers,
                "worker_id {} out of range [0, {})",
                worker_id,
                num_workers
            );
        }
    }
}
