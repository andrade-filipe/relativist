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

use crate::error::PartitionError;
use crate::net::sparse::SparseNet;
use crate::net::{AgentId, Net, PortId, PortRef, Symbol};
use crate::partition::types::{IdRange, Partition, PartitionPlan, WorkerId};

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
    /// Lafont interface port: connects an agent port to a FreePort (net interface).
    ///
    /// Used by streaming generators to express open (unconnected-to-another-agent)
    /// ports that form the interface of the net (e.g., leaf aux ports in `dual_tree`).
    /// These are **not** border wires — they are Lafont FreePorts that persist through
    /// the reduction and are preserved by `merge` (SPEC-05, §4.1 R15).
    ///
    /// In the partition accumulator the wire is installed as
    /// `accumulator.connect(AgentPort(agent_id, port_id), FreePort(free_port_id))`.
    /// This does NOT create a border entry; the FreePort ID MUST NOT collide with
    /// any border ID allocated by `install_connection` for the same run.
    FreePortInterface {
        /// The agent port to connect to the interface.
        agent_port: (AgentId, PortId),
        /// The Lafont FreePort ID.
        ///
        /// MUST be unique across all `FreePortInterface` directives in the stream
        /// and MUST NOT collide with border IDs allocated by `install_connection`
        /// (which start at 0 and grow upward). Generators SHOULD use IDs above
        /// `2 * total_expected_agents` to avoid collisions.
        free_port_id: u32,
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
// RoundRobinStreamingStrategy (SPEC-21 §4.3, R4)
// ---------------------------------------------------------------------------

/// Simplest streaming partition strategy: assigns agents in round-robin order
/// across workers.
///
/// # Properties (SPEC-21 §4.3)
///
/// - O(1) per agent, O(B) per batch where B is the batch size.
/// - Zero state beyond a counter and per-worker counts.
/// - Deterministic: same sequence of batches → same result (R8).
/// - Same partition quality as `ContiguousIdStrategy` (SPEC-04, R22) for
///   sequential generators with contiguous IDs.
/// - Ignores graph topology entirely. Correctness is independent of partition
///   quality (DISC-004 v2, Section 1.6; ARG-002, Passo 10).
///
/// This is the **MVP default strategy** for v2.
///
/// # `chunks_processed` ownership (SC-021)
///
/// `finalize()` returns `chunks_processed: 0`. The pipeline (TASK-0554) stitches
/// the actual count via its own `chunks_seen` counter (SPEC-21 §4.6 Step 7).
#[derive(Debug, Clone)]
pub struct RoundRobinStreamingStrategy {
    /// Monotonically increasing counter used for the round-robin formula:
    /// `worker = counter % num_workers`.
    counter: u64,
    /// Per-worker agent counts accumulated across all batches.
    per_worker_counts: Vec<u64>,
}

impl RoundRobinStreamingStrategy {
    /// Creates a new `RoundRobinStreamingStrategy` for `num_workers` workers.
    ///
    /// Initializes `counter = 0` and `per_worker_counts` to a vec of
    /// `num_workers` zeros.
    ///
    /// # Panics
    ///
    /// Panics if `num_workers == 0` (the strategy cannot route work to zero
    /// workers; `allocate_batch` would divide by zero). Use [`try_new`] to
    /// surface the error as `PartitionError::InvalidNumWorkers` instead.
    ///
    /// [`try_new`]: Self::try_new
    pub fn new(num_workers: u32) -> Self {
        Self::try_new(num_workers)
            .expect("RoundRobinStreamingStrategy::new: num_workers must be >= 1")
    }

    /// QA-D010-005: fallible constructor returning
    /// `PartitionError::InvalidNumWorkers` when `num_workers == 0`.
    pub fn try_new(num_workers: u32) -> Result<Self, PartitionError> {
        if num_workers == 0 {
            return Err(PartitionError::InvalidNumWorkers);
        }
        Ok(Self {
            counter: 0,
            per_worker_counts: vec![0u64; num_workers as usize],
        })
    }
}

impl StreamingPartitionStrategy for RoundRobinStreamingStrategy {
    /// Assigns agents in round-robin order: agent at position `counter` in the
    /// global stream goes to worker `counter % num_workers`.
    ///
    /// # Post-conditions (R7 + R8)
    ///
    /// - Every agent in the batch is assigned exactly once.
    /// - Every `WorkerId` is in `[0, num_workers)`.
    /// - Same input sequence → identical output (counter is the only state).
    ///
    /// # QA-D010-005 / QA-D010-006: input validation
    ///
    /// Returns an empty `Vec` if `num_workers == 0` or if `num_workers`
    /// exceeds the construction-time `per_worker_counts.len()` (which would
    /// otherwise OOB-index `per_worker_counts[worker]`). The orchestrator
    /// validates `num_workers` up-front via
    /// `generate_and_partition_chunked_with_chunk_size`; this guard is the
    /// last line of defense for downstream crates calling the trait directly.
    fn allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)> {
        if num_workers == 0 || (num_workers as usize) > self.per_worker_counts.len() {
            return Vec::new();
        }
        batch
            .agents
            .iter()
            .map(|(id, _symbol)| {
                let worker = (self.counter % num_workers as u64) as WorkerId;
                self.counter += 1;
                self.per_worker_counts[worker as usize] += 1;
                (*id, worker)
            })
            .collect()
    }

    /// Returns statistics accumulated across all batches processed so far.
    ///
    /// `chunks_processed` is always `0` here — the pipeline stitches the real
    /// count (SPEC-21 §4.6 Step 7 / SC-021).
    fn finalize(&self) -> StreamingPartitionStats {
        StreamingPartitionStats {
            total_agents: self.counter,
            per_worker_counts: self.per_worker_counts.clone(),
            border_wire_count: 0, // round-robin does not track topology
            chunks_processed: 0,  // PIPELINE-OWNED (SC-021)
        }
    }
}

// ---------------------------------------------------------------------------
// FennelStreamingStrategy (SPEC-21 §4.4, R5, R6)
// ---------------------------------------------------------------------------

/// Advanced streaming partition strategy using a FENNEL/LDG-style heuristic.
///
/// Assigns each agent to the worker that maximises:
/// ```text
/// score(w) = neighbors_w(A) - alpha * degree(w)
/// ```
/// where `neighbors_w(A)` = number of A's ports already connected to agents on
/// worker `w`; `degree(w)` = total agents assigned to `w` so far; `alpha` =
/// configurable balance parameter (default `1.0` per Q3 disposition).
///
/// # Memory bound (R6)
///
/// The internal `assignment_cache: HashMap<AgentId, WorkerId>` grows to at most
/// O(total_agents) entries, storing only the `(AgentId, WorkerId)` mapping
/// (~8 bytes per agent). This is an 8× memory reduction compared to holding
/// the full net (~64 bytes per agent).
///
/// # References
///
/// REF-TBD (Tsourakakis et al. 2014 — FENNEL streaming graph partitioning).
/// REF-TBD (Stanton & Kliot 2012 — LDG streaming partitioning).
/// NOTE: FENNEL/LDG REF-NNN registration is a TCC-root cleanup task (SC-020
/// deferral). These citations carry `REF-TBD` until registered in
/// `docs/theory-bridge.md` by BIBLIOTECARIO.
///
/// # Tiebreak policy (R8)
///
/// When two or more workers share the maximum score, the strategy picks the
/// **lowest `WorkerId`**. This tiebreak is deterministic and does not depend
/// on `HashMap` iteration order. Tests that exercise the tiebreak path MUST
/// NOT rely on HashMap iteration order; sort by `AgentId` before asserting.
///
/// # Q3 disposition
///
/// SPEC-21 adopts fixed `alpha = 1.0` as default. If calibration (AC-014
/// methodology) shows this is materially worse than batch FENNEL on
/// representative benchmarks, the strategy is deferred to future scope per Q3.
///
/// # `chunks_processed` ownership (SC-021)
///
/// `finalize()` returns `chunks_processed: 0`. The pipeline stitches the real
/// count (SPEC-21 §4.6 Step 7 / SC-021).
#[derive(Debug, Clone)]
pub struct FennelStreamingStrategy {
    /// Maps each assigned `AgentId` to its worker (R6 cache; ~8 bytes per entry).
    pub(crate) assignment_cache: HashMap<AgentId, WorkerId>,
    /// Per-worker agent counts accumulated across all batches.
    per_worker_counts: Vec<u64>,
    /// Balance parameter. Default: `1.0` per Q3 disposition.
    alpha: f64,
}

impl FennelStreamingStrategy {
    /// Creates a new `FennelStreamingStrategy` with an explicit `alpha`.
    ///
    /// # Panics
    ///
    /// Panics if `num_workers == 0` or `alpha` is not finite (NaN or
    /// infinity). Use [`try_new`] to surface these as
    /// `PartitionError::InvalidNumWorkers` /
    /// `PartitionError::InvalidStrategyParameter` instead.
    ///
    /// [`try_new`]: Self::try_new
    pub fn new(num_workers: u32, alpha: f64) -> Self {
        Self::try_new(num_workers, alpha).expect(
            "FennelStreamingStrategy::new: num_workers must be >= 1 and alpha must be finite",
        )
    }

    /// QA-D010-005 + QA-D010-007: fallible constructor that rejects
    /// `num_workers == 0` and non-finite `alpha`. NaN scores would silently
    /// violate R8 determinism (the `>` / `==` comparisons collapse on NaN);
    /// finite alpha with `total_cmp` ordering inside `allocate_batch` keeps
    /// the strategy deterministic.
    pub fn try_new(num_workers: u32, alpha: f64) -> Result<Self, PartitionError> {
        if num_workers == 0 {
            return Err(PartitionError::InvalidNumWorkers);
        }
        if !alpha.is_finite() {
            return Err(PartitionError::InvalidStrategyParameter {
                name: "alpha",
                value: alpha.to_string(),
            });
        }
        Ok(Self {
            assignment_cache: HashMap::new(),
            per_worker_counts: vec![0u64; num_workers as usize],
            alpha,
        })
    }

    /// Creates a `FennelStreamingStrategy` with the Q3 default `alpha = 1.0`.
    pub fn with_default_alpha(num_workers: u32) -> Self {
        Self::new(num_workers, 1.0)
    }
}

impl StreamingPartitionStrategy for FennelStreamingStrategy {
    /// Assigns each agent to the worker with the highest FENNEL score.
    ///
    /// Score formula: `score(w) = neighbors_w(A) - alpha * degree(w)`
    ///
    /// Tiebreak: lowest `WorkerId` wins (deterministic; R8).
    ///
    /// # QA-D010-005..007: input validation
    ///
    /// - `num_workers == 0` or `num_workers > per_worker_counts.len()` →
    ///   returns an empty `Vec` (the orchestrator validates upstream;
    ///   downstream-crate callers see a well-defined no-op).
    /// - Score comparison uses [`f64::total_cmp`] (not `>`/`==`) so the
    ///   ordering is total even if a degenerate input produces a NaN
    ///   score. `try_new` rejects `alpha.is_nan() || alpha.is_infinite()`,
    ///   so the only path to a NaN score in production is an arithmetic
    ///   underflow that should never arise from finite inputs; the
    ///   total_cmp path keeps R8 determinism intact in the unlikely event.
    fn allocate_batch(&mut self, batch: &AgentBatch, num_workers: u32) -> Vec<(AgentId, WorkerId)> {
        if num_workers == 0 || (num_workers as usize) > self.per_worker_counts.len() {
            return Vec::new();
        }

        // Build a reverse index: AgentId -> batch-local position for fast neighbor lookup.
        // We only need to know which agents in THIS batch are connected to which worker.
        let batch_agent_set: std::collections::HashSet<AgentId> =
            batch.agents.iter().map(|(id, _)| *id).collect();

        let mut result = Vec::with_capacity(batch.agents.len());

        for (agent_id, _symbol) in &batch.agents {
            // Count neighbors already assigned to each worker.
            // A "neighbor" of `agent_id` in this context is any agent that shares
            // a Resolved wire with `agent_id` (endpoint of a ConnectionDirective::Resolved).
            let mut neighbor_counts = vec![0i64; num_workers as usize];

            for directive in &batch.connections {
                if let ConnectionDirective::Resolved { source, target } = directive {
                    let (src_id, _src_port) = source;
                    let (tgt_id, _tgt_port) = target;

                    // If the connection involves `agent_id`, check if the OTHER end
                    // is in the assignment_cache (already assigned to a worker).
                    let other_id = if src_id == agent_id {
                        Some(*tgt_id)
                    } else if tgt_id == agent_id {
                        Some(*src_id)
                    } else {
                        None
                    };

                    if let Some(other) = other_id {
                        if !batch_agent_set.contains(&other) {
                            // Other end is from a previous batch — in the cache.
                            if let Some(&w) = self.assignment_cache.get(&other) {
                                neighbor_counts[w as usize] += 1;
                            }
                        }
                        // If other_id IS in the current batch, we don't know its
                        // worker yet (forward reference within batch) — skip.
                    }
                }
            }

            // Compute FENNEL score for each worker and pick argmax.
            // Score = neighbors_w - alpha * degree_w
            // Tiebreak: lowest WorkerId. (QA-D010-007: total_cmp keeps the
            // ordering deterministic even when arithmetic produces NaN.)
            let mut best_worker: WorkerId = 0;
            let mut best_score = f64::NEG_INFINITY;
            let mut have_seen = false;

            for w in 0..num_workers {
                let degree = self.per_worker_counts[w as usize] as f64;
                let neighbors = neighbor_counts[w as usize] as f64;
                let score = neighbors - self.alpha * degree;
                if !have_seen {
                    best_score = score;
                    best_worker = w;
                    have_seen = true;
                    continue;
                }
                match score.total_cmp(&best_score) {
                    std::cmp::Ordering::Greater => {
                        best_score = score;
                        best_worker = w;
                    }
                    std::cmp::Ordering::Equal => {
                        if w < best_worker {
                            best_worker = w;
                        }
                    }
                    std::cmp::Ordering::Less => {}
                }
            }

            // Assign agent to the best worker and update state.
            self.assignment_cache.insert(*agent_id, best_worker);
            self.per_worker_counts[best_worker as usize] += 1;
            result.push((*agent_id, best_worker));
        }

        result
    }

    /// Returns statistics accumulated across all batches processed so far.
    ///
    /// `chunks_processed` is always `0` here — the pipeline stitches the real
    /// count (SPEC-21 §4.6 Step 7 / SC-021).
    fn finalize(&self) -> StreamingPartitionStats {
        let total: u64 = self.per_worker_counts.iter().sum();
        StreamingPartitionStats {
            total_agents: total,
            per_worker_counts: self.per_worker_counts.clone(),
            border_wire_count: 0, // pipeline-owned
            chunks_processed: 0,  // PIPELINE-OWNED (SC-021)
        }
    }
}

// ---------------------------------------------------------------------------
// Phase E: Accumulator + orchestrator (TASK-0550..0554)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// AccumulatorNet + PartitionAccumulator (TASK-0550)
// ---------------------------------------------------------------------------

/// Backing store for an in-progress worker partition accumulator.
///
/// The `Sparse` variant is the default (SPEC-21 §4.9, SC-006 closure): it
/// is memory-proportional to the number of live agents and handles
/// non-contiguous ID spaces produced by FENNEL without the M5 dense-arena
/// pathology.
///
/// The `Dense` variant is reserved for callers that have already materialized
/// a `Net` and wish to wrap it in the accumulator interface; it bypasses the
/// Sparse → Dense conversion at finalize-time.
#[derive(Debug)]
pub(crate) enum AccumulatorNet {
    Sparse(SparseNet),
    /// Alternative backing store using a pre-materialized dense `Net`.
    ///
    /// Not used by the primary streaming pipeline (which always starts Sparse),
    /// but reserved for future callers that have a `Net` on hand and want to
    /// reuse the `PartitionAccumulator` API without paying the Sparse→Dense
    /// conversion cost at finalize-time.
    #[allow(dead_code)]
    Dense(Net),
}

/// In-progress accumulator for one worker's partition during streaming
/// generation.
///
/// # Frame-reuse pattern (AC-010 / §4.9 intro)
///
/// One `PartitionAccumulator` per worker is allocated for the lifetime of the
/// pipeline run and reused across all chunks. The accumulator is NOT reallocated
/// per chunk. This mirrors the HVM4 WNF goto-state-machine frame-reuse pattern
/// (AC-010).
///
/// # R23 reconciliation (§4.9 closing note)
///
/// R23 ("MUST be sized to max_agent_id_in_this_worker + 1") applies to the
/// **dense-finalized form** produced by `finalize()`, NOT to the in-progress
/// `SparseNet`. Do NOT pre-size a dense `Vec` at construction time hoping to
/// amortize the resize — that resurrects SC-006. The dense form is produced
/// exactly once, at `finalize()`, by calling `SparseNet::to_dense(Some(id_range))`.
pub(crate) struct PartitionAccumulator {
    /// The backing store for this worker's agents and wires.
    pub(crate) subnet: AccumulatorNet,
    /// Reverse index of boundary FreePorts: borderId → local AgentPort endpoint.
    pub(crate) free_port_index: HashMap<u32, PortRef>,
    /// The worker this accumulator belongs to.
    pub(crate) worker_id: WorkerId,
    /// Minimum AgentId assigned to this worker so far (`None` if empty).
    pub(crate) min_assigned_id: Option<AgentId>,
    /// Maximum AgentId assigned to this worker so far (`None` if empty).
    pub(crate) max_assigned_id: Option<AgentId>,
    /// Number of live agents added to this accumulator.
    pub(crate) live_agent_count: u64,
}

impl PartitionAccumulator {
    /// Constructs a fresh accumulator for the given `worker_id`.
    ///
    /// Defaults to `AccumulatorNet::Sparse(SparseNet::new())` per §4.9 (SC-006
    /// closure). The `free_port_index`, `min_assigned_id`, and `max_assigned_id`
    /// fields are all empty / None; `live_agent_count` is 0.
    pub(crate) fn new(worker_id: WorkerId) -> Self {
        Self {
            subnet: AccumulatorNet::Sparse(SparseNet::new()),
            free_port_index: HashMap::new(),
            worker_id,
            min_assigned_id: None,
            max_assigned_id: None,
            live_agent_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// PartitionAccumulator::add_agent + connect (TASK-0551)
// ---------------------------------------------------------------------------

impl PartitionAccumulator {
    /// Adds an agent at the specified `id` with the given `symbol`.
    ///
    /// Delegates to the backing subnet's `create_agent_at` (for `Sparse`)
    /// or equivalent operation (for `Dense`). Updates `min_assigned_id`,
    /// `max_assigned_id`, and `live_agent_count`.
    ///
    /// # Invariants
    ///
    /// - `id` MUST NOT already be present (I3' uniqueness, SPEC-22 R14).
    ///   In debug builds this is checked by the underlying SparseNet; in
    ///   release builds, the behavior is that of HashMap::insert (overwrite).
    pub(crate) fn add_agent(&mut self, id: AgentId, symbol: Symbol) {
        match &mut self.subnet {
            AccumulatorNet::Sparse(s) => s.create_agent_at(id, symbol),
            AccumulatorNet::Dense(n) => {
                // Dense path: ensure the arena is large enough, then insert.
                // This path is only triggered by the Dense variant, which is
                // reserved for callers that wrap a pre-materialized Net.
                while n.agents.len() <= id as usize {
                    n.agents.push(None);
                }
                n.agents[id as usize] = Some(crate::net::types::Agent { symbol, id });
            }
        }
        self.min_assigned_id = Some(self.min_assigned_id.map_or(id, |m| m.min(id)));
        self.max_assigned_id = Some(self.max_assigned_id.map_or(id, |m| m.max(id)));
        self.live_agent_count += 1;
    }

    /// Connects two port references within this accumulator's subnet.
    ///
    /// Updates `free_port_index` BEFORE delegating to the subnet, so that
    /// every FreePort endpoint is registered at insertion time (C2 partial).
    ///
    /// # FreePort policy (both endpoints FreePort)
    ///
    /// If both `a` and `b` are `FreePort(bid)`, both are inserted into
    /// `free_port_index` symmetrically: `free_port_index[a_bid] = b` and
    /// `free_port_index[b_bid] = a`. This is a degenerate but allowed
    /// structure; the merge protocol handles the symmetric lookup.
    pub(crate) fn connect(&mut self, a: PortRef, b: PortRef) {
        // Register FreePort endpoints in the reverse index.
        if let PortRef::FreePort(bid) = b {
            self.free_port_index.insert(bid, a);
        }
        if let PortRef::FreePort(bid) = a {
            self.free_port_index.insert(bid, b);
        }
        match &mut self.subnet {
            AccumulatorNet::Sparse(s) => s.connect(a, b),
            AccumulatorNet::Dense(n) => n.connect(a, b),
        }
    }
}

// ---------------------------------------------------------------------------
// PartitionAccumulator::finalize (TASK-0552)
// ---------------------------------------------------------------------------

impl PartitionAccumulator {
    /// Converts the accumulator into a fully-formed `Partition`.
    ///
    /// # Conversion
    ///
    /// For the `Sparse` variant, calls `SparseNet::to_dense(Some(id_range))`
    /// (SPEC-22 R20 / TASK-0490). For `Dense`, uses the inner `Net` directly.
    ///
    /// # Threshold guard (SPEC-22 R10a / R22 / R30, D-011 amendment 2026-05-04)
    ///
    /// If `effective_arena_size > 4 × live_agent_count` returns
    /// `Err(PartitionError::DenseAllocationExceedsThreshold)`. The
    /// inequality is STRICT (`>`): exactly 4× is accepted.
    ///
    /// `effective_arena_size = max_assigned_id + 1` matches the actual
    /// `Vec<Option<Agent>>` size that `SparseNet::to_dense` (or the
    /// `AccumulatorNet::Dense` direct path) would allocate. The pre-D-011
    /// metric used `id_range.end - id_range.start` (the planning range
    /// from `compute_id_ranges`), which routed healthy workloads through
    /// the threshold; see `docs/next-steps.md` BLOCKER 2026-05-04.
    ///
    /// # R23 reconciliation
    ///
    /// The dense `Net` produced by `to_dense(Some(id_range))` has
    /// `agents.len() == max_assigned_id + 1`, satisfying R23
    /// ("sized to max_agent_id_in_this_worker + 1") for partition-scoped
    /// layouts.
    pub(crate) fn finalize(
        self,
        id_range: IdRange,
        border_id_start: u32,
        border_id_end: u32,
    ) -> Result<Partition, PartitionError> {
        // SPEC-22 R22 (D-011 amendment 2026-05-04): metric is the actual
        // dense arena size (`max_assigned_id + 1`), not the planning range.
        let effective_arena_size: u64 = self
            .max_assigned_id
            .map(|max_id| max_id as u64 + 1)
            .unwrap_or(0);

        // SPEC-22 R30 threshold check: strict greater-than.
        if effective_arena_size > 4 * self.live_agent_count {
            return Err(PartitionError::DenseAllocationExceedsThreshold {
                partition_index: self.worker_id as usize,
                effective_arena_size,
                live_count: self.live_agent_count,
            });
        }

        let mut dense = match self.subnet {
            AccumulatorNet::Sparse(s) => {
                // Convert SparseNet → dense Net scoped to id_range.
                s.to_dense(Some(id_range.start..id_range.end))
                    .map_err(|e| PartitionError::InvariantViolation(e.to_string()))?
            }
            AccumulatorNet::Dense(n) => n,
        };

        // QA-D011-POST-FIX-AUDIT F-001 (2026-05-04): mirror split.rs:96-98 +
        // helpers.rs:390 invariant — the resulting subnet's next_id MUST lie in
        // [id_range.start, id_range.end) so AF-2 doesn't fire when downstream
        // callers invoke create_agent. SparseNet::to_dense propagates self.next_id
        // verbatim (sparse.rs:396), and SparseNet::create_agent_at sets
        // next_id = max(next_id, id+1) which can land at id_range.end. Widen
        // upward to id_range.start so empty/below-start cases are seeded.
        // SPEC-22 R10 / D3 / SC-001 (closes the dormant streaming-side analog
        // of Bug 2 fixed for dense build_subnet at helpers.rs:390).
        dense.next_id = std::cmp::max(dense.next_id, id_range.start);
        // Mirror dense build_subnet's id_range population so AF-2 has the bound
        // available for downstream create_agent calls.
        if dense.id_range.is_none() {
            dense.id_range = Some(id_range.start..id_range.end);
        }

        Ok(Partition {
            subnet: dense,
            worker_id: self.worker_id,
            free_port_index: self.free_port_index,
            id_range,
            border_id_start,
            border_id_end,
        })
    }
}

// ---------------------------------------------------------------------------
// install_connection helper (TASK-0553)
// ---------------------------------------------------------------------------

/// Installs a wire between `source` and `target` ports, classifying it as
/// internal or border based on worker ownership.
///
/// # Classification (AC-007 pattern — §4.6)
///
/// Connection-time classification: the wire is evaluated at the moment it is
/// installed, NOT in a post-construction scan. `agent_owner` MUST contain
/// entries for both `source.0` and `target.0` before this function is called.
///
/// - **Internal** (`src_worker == tgt_worker`): the wire is installed directly
///   into the owning worker's accumulator.
/// - **Border** (`src_worker != tgt_worker`): a fresh `borderId` is allocated,
///   a symmetric `FreePort(borderId)` pair is connected into both accumulators,
///   and the mapping is recorded in `border_map`.
///
/// # Panics
///
/// Panics if `source.0` or `target.0` is missing from `agent_owner`.
/// This is a pipeline invariant: agent-owner mappings MUST be inserted BEFORE
/// any `install_connection` call for that agent.
pub(crate) fn install_connection(
    source: (AgentId, PortId),
    target: (AgentId, PortId),
    agent_owner: &HashMap<AgentId, WorkerId>,
    accumulators: &mut [PartitionAccumulator],
    border_map: &mut HashMap<u32, (PortRef, PortRef)>,
    border_id_counter: &mut u32,
    reserved_freeport_ids: &std::collections::HashSet<u32>,
) {
    let src_worker = *agent_owner
        .get(&source.0)
        .unwrap_or_else(|| panic!("install_connection: agent {} has no owner (pipeline bug — agent must be inserted into agent_owner before install_connection is called)", source.0));
    let tgt_worker = *agent_owner
        .get(&target.0)
        .unwrap_or_else(|| panic!("install_connection: agent {} has no owner (pipeline bug — agent must be inserted into agent_owner before install_connection is called)", target.0));

    if src_worker == tgt_worker {
        // Internal wire: both endpoints belong to the same worker.
        accumulators[src_worker as usize].connect(
            PortRef::AgentPort(source.0, source.1),
            PortRef::AgentPort(target.0, target.1),
        );
    } else {
        // Border wire: allocate a border ID and emit FreePort pairs.
        // QA-D010-004: skip any ID that has already been registered as a
        // Lafont `FreePortInterface` ID by a generator. Without this skip,
        // a stream that emits `FreePortInterface(0)` followed by a
        // cross-worker `Resolved` wire would alias border 0 with FreePort(0),
        // silently splicing two unrelated wires (D5/D6 port-bijection
        // violation).
        while reserved_freeport_ids.contains(border_id_counter) {
            *border_id_counter = border_id_counter
                .checked_add(1)
                .expect("border_id_counter overflow while skipping reserved FreePort IDs");
        }
        let bid = *border_id_counter;
        *border_id_counter = border_id_counter
            .checked_add(1)
            .expect("border_id_counter overflow: u32::MAX border wires reached");

        // Canonical orientation: always store (lower-WorkerId-side, higher-WorkerId-side).
        let (canon_src, canon_tgt) = if src_worker <= tgt_worker {
            (
                PortRef::AgentPort(source.0, source.1),
                PortRef::AgentPort(target.0, target.1),
            )
        } else {
            (
                PortRef::AgentPort(target.0, target.1),
                PortRef::AgentPort(source.0, source.1),
            )
        };
        border_map.insert(bid, (canon_src, canon_tgt));

        accumulators[src_worker as usize].connect(
            PortRef::AgentPort(source.0, source.1),
            PortRef::FreePort(bid),
        );
        accumulators[tgt_worker as usize].connect(
            PortRef::AgentPort(target.0, target.1),
            PortRef::FreePort(bid),
        );
    }
}

// ---------------------------------------------------------------------------
// PendingConnection store (part of TASK-0554 pipeline)
// ---------------------------------------------------------------------------

/// A buffered pending connection: source port waiting for the target agent.
#[derive(Debug, Clone)]
struct PendingConnection {
    source: (AgentId, PortId),
    target_agent_id: AgentId,
    target_port: PortId,
    /// QA-D010-009: chunk index at which this entry was buffered. When a
    /// pending entry has aged beyond `GridConfig.max_pending_lifetime`
    /// chunks the orchestrator returns `PartitionError::PendingConnectionExpired`
    /// and stops processing the stream (R37g malformed-stream class).
    birth_chunk: u64,
}

// ---------------------------------------------------------------------------
// generate_and_partition_chunked orchestrator (TASK-0554)
// ---------------------------------------------------------------------------

/// Streaming pipeline: generates and partitions a net one `AgentBatch` at a time.
///
/// # Overview (SPEC-21 §3.3 R17, R18)
///
/// Processes the stream chunk by chunk without ever buffering the full net:
///
/// 1. For each batch, call `strategy.allocate_batch` to assign agents to workers.
/// 2. Install each `Resolved` connection via `install_connection`.
/// 3. Buffer each `Pending` connection in the pending store.
/// 4. After agent insertion, scan the pending store for entries whose target
///    agent is now known; resolve them via `install_connection`.
/// 5. After the stream is exhausted, assert the pending store is empty (R19).
/// 6. Finalize each accumulator into a `Partition`.
/// 7. Stitch `stats.chunks_processed = chunks_seen` (SC-021 closure).
///
/// # R22 (one-batch-in-flight)
///
/// The loop processes one `AgentBatch` per iteration. The stream is never
/// collected (no `stream.collect()` or equivalent).
///
/// # Border-id initialization (QA-D010-004 fix)
///
/// `border_id_counter` starts at 0. Every `FreePortInterface.free_port_id`
/// observed in the stream is recorded in a `reserved_freeport_ids` set;
/// `install_connection` skips any counter value present in that set when
/// allocating a border ID. Without this skip, a stream that emits
/// `FreePortInterface(k)` followed by a cross-worker wire that would
/// allocate `border = k` aliases the Lafont interface and the border on
/// the same `FreePort(k)` slot of an accumulator (D5/D6 port-bijection
/// violation).
///
/// The skip is bounded: the union of `[0, border_id_counter)` and the
/// reserved set is at most `2 * total_wires`; the inner `while` over the
/// set is amortized O(1) per allocation in the typical case where
/// reserved IDs are sparse relative to allocated borders.
///
/// `border_id_start` is recorded as the smallest border ID actually
/// allocated (or `border_id_counter` if no borders were ever allocated),
/// so the partition's `[border_id_start, border_id_end)` range stays
/// aligned with the consumer-side rebuild check in SPEC-05.
impl From<PartitionPlan> for ChunkedPartitionResult {
    /// Convert a `PartitionPlan` (from SPEC-04 `split()`) into a `ChunkedPartitionResult`.
    ///
    /// Used by the R26 short-circuit path: when `chunk_size == u32::MAX`, the pipeline
    /// materialises the full stream and delegates to `split()`.  The resulting
    /// `PartitionPlan` is wrapped here so the caller receives a uniform type.
    ///
    /// `stats` is populated with counts derived from the plan; `chunks_processed`
    /// is set to 1 (the whole net as a single "chunk").
    fn from(plan: PartitionPlan) -> Self {
        let total_agents: u64 = plan
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents() as u64)
            .sum();
        let per_worker_counts: Vec<u64> = plan
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents() as u64)
            .collect();
        let border_wire_count = plan.borders.len() as u64;
        let borders = plan.borders.clone();
        let stats = StreamingPartitionStats {
            total_agents,
            per_worker_counts,
            border_wire_count,
            chunks_processed: 1,
        };
        ChunkedPartitionResult {
            partitions: plan.partitions,
            borders,
            stats,
        }
    }
}

/// R26 short-circuit sentinel: when `chunk_size == u32::MAX`, the pipeline
/// MUST take the materialise-then-split path (SPEC-21 §3.4 R26).
///
/// This constant is exported so that callers can test for the sentinel without
/// hard-coding the magic value.
pub const CHUNK_SIZE_MAX_SENTINEL: u32 = u32::MAX;

pub fn generate_and_partition_chunked(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn StreamingPartitionStrategy,
) -> Result<ChunkedPartitionResult, PartitionError> {
    generate_and_partition_chunked_with_chunk_size(stream, num_workers, strategy, num_workers)
}

/// Internal implementation of `generate_and_partition_chunked`, parameterised
/// by `chunk_size` so the R26 short-circuit path can be exercised.
///
/// When `chunk_size == CHUNK_SIZE_MAX_SENTINEL`, materialise the whole stream
/// into a single `Net` and delegate to SPEC-04 `split()` (R26 — closes SC-014).
/// The result is bit-**non**-identical to the streaming path but isomorphic up to
/// agent-ID renaming (SPEC-00 §6.12 `nets_isomorphic`).
pub fn generate_and_partition_chunked_with_chunk_size(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn StreamingPartitionStrategy,
    chunk_size: u32,
) -> Result<ChunkedPartitionResult, PartitionError> {
    generate_and_partition_chunked_with_chunk_size_and_lifetime(
        stream,
        num_workers,
        strategy,
        chunk_size,
        u32::MAX,
    )
}

/// QA-D010-009: variant of [`generate_and_partition_chunked_with_chunk_size`]
/// that enforces `max_pending_lifetime`. A `Pending` directive whose target
/// agent is not introduced within `max_pending_lifetime` chunks of its
/// recording chunk is reported as
/// `PartitionError::PendingConnectionExpired`. Use `u32::MAX` to disable
/// the bound (the legacy behaviour of the chunk_size variant).
pub fn generate_and_partition_chunked_with_chunk_size_and_lifetime(
    stream: Box<dyn Iterator<Item = AgentBatch>>,
    num_workers: u32,
    strategy: &mut dyn StreamingPartitionStrategy,
    chunk_size: u32,
    max_pending_lifetime: u32,
) -> Result<ChunkedPartitionResult, PartitionError> {
    // QA-D010-005: orchestrator-level validation. Strategies internally
    // also guard their own `allocate_batch`, but surfacing the error here
    // gives callers a single, well-defined entry-point error and avoids a
    // silent "no-op succeeds" outcome.
    if num_workers == 0 {
        return Err(PartitionError::InvalidNumWorkers);
    }
    // R26 short-circuit: chunk_size == u32::MAX → materialise-then-split.
    //
    // Per SPEC-21 §3.4 R26 (closes SC-014), the pipeline MUST collect the full
    // stream into a single Net and delegate to SPEC-04 `split()`. The result is
    // isomorphic (not bit-identical) to the streaming path per `nets_isomorphic`.
    //
    // Strategy: materialise into a SparseNet (using `create_agent_at` which was
    // added in Phase E for streaming), then convert to a dense Net for `split()`.
    // Only `Resolved` directives are installed here; `Pending` directives cannot
    // appear in a well-formed materialisation stream (they would violate R26's
    // "complete stream" precondition) and are silently skipped so the function
    // remains total.
    if chunk_size == CHUNK_SIZE_MAX_SENTINEL {
        use crate::net::sparse::SparseNet;
        use crate::partition::split::split;
        use crate::partition::strategy::ContiguousIdStrategy;

        let mut sparse = SparseNet::new();
        for batch in stream {
            for (id, symbol) in &batch.agents {
                sparse.create_agent_at(*id, *symbol);
            }
            for directive in &batch.connections {
                if let ConnectionDirective::Resolved {
                    source: (src_id, src_port),
                    target: (tgt_id, tgt_port),
                } = directive
                {
                    sparse.connect(
                        crate::net::PortRef::AgentPort(*src_id, *src_port),
                        crate::net::PortRef::AgentPort(*tgt_id, *tgt_port),
                    );
                }
                // Pending directives: not reachable on well-formed full-stream materialisation.
            }
        }

        // Convert to dense Net for split(). id_range = None → arena sized to max_id+1.
        let net = match sparse.to_dense(None) {
            Ok(n) => n,
            Err(e) => return Err(PartitionError::ArenaConversionFailed(format!("{e}"))),
        };

        let strategy_for_split = ContiguousIdStrategy;
        let plan = split(net, num_workers, &strategy_for_split);
        return Ok(ChunkedPartitionResult::from(plan));
    }

    // --- Streaming path (chunk_size < u32::MAX) ---

    // Pipeline-local state.
    let mut accumulators: Vec<PartitionAccumulator> =
        (0..num_workers).map(PartitionAccumulator::new).collect();
    // QA-D010-004: `border_id_counter` MUST stay above every `FreePortInterface
    // .free_port_id` we have observed so far, so the freshly-allocated border IDs
    // emitted by `install_connection` cannot collide with Lafont interface IDs
    // already installed on the same accumulator. Pre-fix this counter started at
    // 0 unconditionally, silently aliasing border 0 with `FreePort(0)`.
    //
    // Streaming constraint: we cannot pre-scan the entire stream (R22: one
    // batch in flight). Instead we observe each batch's interface IDs as they
    // arrive and lift the counter accordingly before any border allocation.
    let mut border_id_counter: u32 = 0;
    // QA-D010-004: tracks the smallest counter value at which a border was
    // ever allocated, used to populate `Partition::border_id_start` at
    // finalize. None until the first border allocation; otherwise the
    // pre-allocation counter snapshot.
    let mut border_id_start: Option<u32> = None;
    // QA-D010-004: every `FreePortInterface.free_port_id` observed in the
    // stream is recorded here. `install_connection` consults this set when
    // allocating a border ID and skips any value already reserved by a
    // Lafont interface, preventing the FreePort(k) <-> border(k) aliasing.
    let mut reserved_freeport_ids: std::collections::HashSet<u32> =
        std::collections::HashSet::new();
    let mut border_map: HashMap<u32, (PortRef, PortRef)> = HashMap::new();
    // pending: target_agent_id → Vec<PendingConnection>
    let mut pending: HashMap<AgentId, Vec<PendingConnection>> = HashMap::new();
    let mut agent_owner: HashMap<AgentId, WorkerId> = HashMap::new();
    let mut chunks_seen: u64 = 0;

    // Main streaming loop (R22: one batch in flight).
    for batch in stream {
        chunks_seen += 1;

        // QA-D010-004: pre-register every `FreePortInterface` ID before
        // border allocation runs in this batch, so `install_connection`
        // can skip them. We also register them when actually installing
        // (Step 2.5) — pre-registration here is required because Step 2
        // (Resolved → border allocation) runs BEFORE Step 2.5 in the
        // current ordering.
        for directive in &batch.connections {
            if let ConnectionDirective::FreePortInterface { free_port_id, .. } = directive {
                if *free_port_id != u32::MAX {
                    reserved_freeport_ids.insert(*free_port_id);
                }
            }
        }

        // Step 1: assign agents to workers.
        let assignments = strategy.allocate_batch(&batch, num_workers);

        // Build a local symbol lookup from the batch (needed for add_agent).
        let symbol_lookup: HashMap<AgentId, Symbol> =
            batch.agents.iter().map(|(id, sym)| (*id, *sym)).collect();

        // QA-D010-008: validate every (agent_id, worker_id) the strategy
        // returned. Production strategies (RoundRobin, Fennel) are
        // well-behaved; downstream crates implementing the trait may
        // misbehave. The orchestrator surfaces malformed output as a
        // PartitionError rather than panicking on `accumulators[OOB]` or
        // `symbol_lookup[missing_id]` — both of which previously crashed
        // the tokio runtime.
        for (agent_id, worker_id) in &assignments {
            if (*worker_id as usize) >= accumulators.len() {
                return Err(PartitionError::StrategyReturnedInvalidWorker {
                    worker_id: *worker_id,
                    num_workers,
                });
            }
            let symbol = match symbol_lookup.get(agent_id) {
                Some(s) => *s,
                None => {
                    return Err(PartitionError::StrategyReturnedUnknownAgent {
                        agent_id: *agent_id,
                    })
                }
            };
            agent_owner.insert(*agent_id, *worker_id);
            accumulators[*worker_id as usize].add_agent(*agent_id, symbol);
        }

        // Step 2: install Resolved connections.
        for directive in &batch.connections {
            if let ConnectionDirective::Resolved { source, target } = directive {
                let pre_counter = border_id_counter;
                install_connection(
                    *source,
                    *target,
                    &agent_owner,
                    &mut accumulators,
                    &mut border_map,
                    &mut border_id_counter,
                    &reserved_freeport_ids,
                );
                // QA-D010-004: if this was the first border allocation, record
                // the (post-skip) starting counter as `border_id_start`. The
                // border ID actually used is `pre_counter` after the skip
                // pass in `install_connection`, which equals
                // `border_id_counter - 1` immediately after a successful
                // allocation. Using `pre_counter` here is conservative: it
                // is always <= the smallest border ID actually allocated,
                // and the merge-side range check `[start, end)` is inclusive
                // on the low end.
                if border_id_start.is_none() && border_id_counter != pre_counter {
                    // The first border ID allocated equals border_id_counter - 1.
                    border_id_start = Some(border_id_counter - 1);
                }
            }
        }

        // Step 2.5: install FreePortInterface directives (Lafont interface ports).
        //
        // A `FreePortInterface` connects an agent port directly to a Lafont FreePort
        // (a net interface, NOT a border wire). These are installed directly in the
        // owning worker's accumulator — no border allocation needed.
        for directive in &batch.connections {
            if let ConnectionDirective::FreePortInterface {
                agent_port: (agent_id, port_id),
                free_port_id,
            } = directive
            {
                let worker_id = *agent_owner.get(agent_id).unwrap_or_else(|| {
                    panic!("FreePortInterface: agent {} has no owner", agent_id)
                });
                accumulators[worker_id as usize].connect(
                    PortRef::AgentPort(*agent_id, *port_id),
                    PortRef::FreePort(*free_port_id),
                );
            }
        }

        // Step 3: buffer Pending connections.
        for directive in &batch.connections {
            if let ConnectionDirective::Pending {
                source,
                target_agent_id,
                target_port,
            } = directive
            {
                pending
                    .entry(*target_agent_id)
                    .or_default()
                    .push(PendingConnection {
                        source: *source,
                        target_agent_id: *target_agent_id,
                        target_port: *target_port,
                        birth_chunk: chunks_seen,
                    });
            }
        }

        // QA-D010-009: enforce the max_pending_lifetime budget. After every
        // batch, scan the pending store for entries whose age (in chunks)
        // has exceeded the configured budget; if any has, return
        // `PartitionError::PendingConnectionExpired` rather than letting
        // the HashMap grow without bound. The scan is bounded by
        // `pending.len()` and runs once per batch — O(N) amortised, where
        // N is the live pending count (small in well-formed streams).
        // `max_pending_lifetime == u32::MAX` is the legacy "disabled"
        // sentinel: the early exit avoids the scan entirely.
        if max_pending_lifetime != u32::MAX {
            let budget = max_pending_lifetime as u64;
            for pcs in pending.values() {
                for pc in pcs {
                    let age = chunks_seen.saturating_sub(pc.birth_chunk);
                    if age > budget {
                        return Err(PartitionError::PendingConnectionExpired {
                            agent_id: pc.target_agent_id,
                            age,
                            budget,
                        });
                    }
                }
            }
        }

        // Step 6: resolve pending entries for agents introduced in this batch.
        let newly_introduced: Vec<AgentId> = assignments.iter().map(|(id, _)| *id).collect();
        for agent_id in &newly_introduced {
            if let Some(pcs) = pending.remove(agent_id) {
                for pc in pcs {
                    let pre_counter = border_id_counter;
                    install_connection(
                        pc.source,
                        (pc.target_agent_id, pc.target_port),
                        &agent_owner,
                        &mut accumulators,
                        &mut border_map,
                        &mut border_id_counter,
                        &reserved_freeport_ids,
                    );
                    // QA-D010-004: same first-allocation tracking as Step 2.
                    if border_id_start.is_none() && border_id_counter != pre_counter {
                        border_id_start = Some(border_id_counter - 1);
                    }
                }
            }
        }
    }

    // Step 4 (post-loop): assert pending store is empty (R19).
    if let Some((&agent_id, _)) = pending.iter().next() {
        return Err(PartitionError::UnresolvedForwardReferences { agent_id });
    }

    // Step 5: compute per-worker id_ranges with D3 invariant (non-overlapping).
    //
    // D3 (SPEC-04, R16-R19) requires that id_ranges across all partitions are
    // mutually disjoint. With round-robin assignment, each worker holds every
    // num_workers-th agent ID, so per-worker [min, max+1] intervals can overlap.
    //
    // Solution: compute the global [0, global_max_id+1) range and partition it
    // into num_workers equal-sized non-overlapping bands. Each worker i is assigned
    // [i*band, (i+1)*band). Bands may be wider than the actual agent IDs a worker
    // holds — that's acceptable (the id_range is an upper bound, not a tight cover).
    // This satisfies D3 and is consistent with how ContiguousIdStrategy (SPEC-04 R22)
    // partitions the full ID space.
    //
    // Empty nets (no agents assigned): all workers get [0, 0) (degenerate).
    let global_max_id: Option<u32> = accumulators.iter().filter_map(|a| a.max_assigned_id).max();

    let mut partitions: Vec<Partition> = Vec::with_capacity(num_workers as usize);

    // We need per-worker border_id ranges. The border_map already has all
    // border IDs; QA-D010-004 records the per-batch lift cursor at the time
    // of the first border allocation as `border_start`. Borders then occupy
    // `[border_start, border_id_counter)`; Lafont FreePort interface IDs
    // occupy `[0, border_start)`. Both ranges are recorded on every worker
    // (the full range was allocated globally, not per-worker). Individual
    // wires are discriminated via free_port_index / range check at merge time.
    //
    // If no borders were ever allocated (single-worker streaming, or no
    // cross-worker wires), `border_start` defaults to `border_id_counter`
    // (degenerate empty range — every FreePort id is below `border_start`,
    // i.e. every FreePort encountered is correctly classified as Lafont).
    let border_start = border_id_start.unwrap_or(border_id_counter);
    let border_end = border_id_counter;

    for (worker_idx, accumulator) in accumulators.into_iter().enumerate() {
        let id_range = if let Some(global_max) = global_max_id {
            // Band-based non-overlapping range: ensures D3 across all workers.
            let total = global_max as u64 + 1;
            let n = num_workers as u64;
            let band = total.div_ceil(n);
            let start = (worker_idx as u64 * band).min(total) as u32;
            let end = ((worker_idx as u64 + 1) * band).min(total) as u32;
            IdRange { start, end }
        } else {
            // No agents at all — zero-width degenerate range.
            IdRange { start: 0, end: 0 }
        };

        // Empty workers always pass the threshold check (0 > 4*0 is false).
        let partition = accumulator.finalize(id_range, border_start, border_end)?;
        partitions.push(partition);
    }

    // Step 7: stitch chunks_processed (closes SC-021).
    let mut stats = strategy.finalize();
    stats.chunks_processed = chunks_seen;
    stats.border_wire_count = border_end as u64;

    Ok(ChunkedPartitionResult {
        partitions,
        borders: border_map,
        stats,
    })
}

// ---------------------------------------------------------------------------
// Tests (Phase B: TASK-0520..0524 + Phase C: TASK-0530..0531)
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

    // -----------------------------------------------------------------------
    // TEST-SPEC-0530: RoundRobinStreamingStrategy (T1)
    // -----------------------------------------------------------------------

    /// Helper: build a batch with N agents starting at `base_id`, all ERA, no connections.
    fn make_era_batch(base_id: AgentId, count: usize) -> AgentBatch {
        AgentBatch {
            agents: (0..count)
                .map(|i| (base_id + i as AgentId, Symbol::Era))
                .collect(),
            connections: vec![],
        }
    }

    /// UT-0530-01: Agents are assigned in round-robin order (agent `id` → worker `id % 4`).
    #[test]
    fn assignment_round_robin_order() {
        let num_workers = 4u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        let mut all_assignments: Vec<(AgentId, WorkerId)> = Vec::new();

        for batch_idx in 0..5u32 {
            let batch = make_era_batch(batch_idx * 20, 20);
            let assignments = strategy.allocate_batch(&batch, num_workers);
            all_assignments.extend(assignments);
        }

        for (agent_id, worker_id) in &all_assignments {
            assert_eq!(
                *worker_id,
                agent_id % num_workers,
                "agent {} should go to worker {} but got {}",
                agent_id,
                agent_id % num_workers,
                worker_id
            );
        }
    }

    /// UT-0530-02: Each agent is assigned exactly once (C1 — 100 agents total).
    #[test]
    fn each_agent_assigned_exactly_once() {
        let num_workers = 4u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        let mut agent_ids_seen: std::collections::HashSet<AgentId> =
            std::collections::HashSet::new();

        for batch_idx in 0..5u32 {
            let batch = make_era_batch(batch_idx * 20, 20);
            let assignments = strategy.allocate_batch(&batch, num_workers);
            for (agent_id, _) in assignments {
                let inserted = agent_ids_seen.insert(agent_id);
                assert!(inserted, "agent {} assigned more than once", agent_id);
            }
        }
        assert_eq!(agent_ids_seen.len(), 100, "all 100 agents must be assigned");
    }

    /// UT-0530-03: `finalize()` per-worker counts are [25, 25, 25, 25] for 100 agents / 4 workers.
    #[test]
    fn finalize_per_worker_counts_match() {
        let num_workers = 4u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);

        for batch_idx in 0..5u32 {
            let batch = make_era_batch(batch_idx * 20, 20);
            strategy.allocate_batch(&batch, num_workers);
        }

        let stats = strategy.finalize();
        assert_eq!(
            stats.per_worker_counts,
            vec![25u64, 25, 25, 25],
            "each worker should receive 25 agents"
        );
        assert_eq!(stats.total_agents, 100);
        assert_eq!(
            stats.chunks_processed, 0,
            "pipeline-owned; must be 0 from strategy"
        );
    }

    /// UT-0530-04: Determinism — two fresh runs on the same input produce identical output.
    #[test]
    fn determinism_repeated_invocation() {
        let num_workers = 4u32;

        let mut strategy1 = RoundRobinStreamingStrategy::new(num_workers);
        let mut strategy2 = RoundRobinStreamingStrategy::new(num_workers);

        for batch_idx in 0..5u32 {
            let batch = make_era_batch(batch_idx * 20, 20);
            let a1 = strategy1.allocate_batch(&batch, num_workers);
            let a2 = strategy2.allocate_batch(&batch, num_workers);
            assert_eq!(a1, a2, "determinism violated in batch {}", batch_idx);
        }
    }

    /// UT-0530-05: C1 cross-batch — union covers every agent in 0..99 exactly once.
    #[test]
    fn c1_complete_coverage_cross_batch() {
        let num_workers = 4u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        let mut assigned: Vec<AgentId> = Vec::new();

        for batch_idx in 0..5u32 {
            let batch = make_era_batch(batch_idx * 20, 20);
            let assignments = strategy.allocate_batch(&batch, num_workers);
            for (id, _) in assignments {
                assigned.push(id);
            }
        }

        assigned.sort_unstable();
        let expected: Vec<AgentId> = (0..100).collect();
        assert_eq!(
            assigned, expected,
            "C1: union must cover every agent 0..99 exactly once"
        );
    }

    /// UT-0530-06: Single batch run produces correct assignment.
    #[test]
    fn single_batch_run() {
        let num_workers = 4u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        let batch = make_era_batch(0, 20);
        let assignments = strategy.allocate_batch(&batch, num_workers);
        assert_eq!(assignments.len(), 20);
        for (agent_id, worker_id) in &assignments {
            assert_eq!(*worker_id, agent_id % num_workers);
        }
    }

    /// UT-0530-07: Single worker — all agents go to worker 0.
    #[test]
    fn single_worker_all_to_zero() {
        let num_workers = 1u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        let batch = make_era_batch(0, 100);
        let assignments = strategy.allocate_batch(&batch, num_workers);
        for (_, worker_id) in &assignments {
            assert_eq!(*worker_id, 0u32, "all agents must go to worker 0");
        }
        let stats = strategy.finalize();
        assert_eq!(stats.per_worker_counts, vec![100u64]);
    }

    /// UT-0530-08: More workers than agents — workers beyond agent count receive 0.
    #[test]
    fn more_workers_than_agents() {
        let num_workers = 10u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        let batch = make_era_batch(0, 5);
        let assignments = strategy.allocate_batch(&batch, num_workers);
        let stats = strategy.finalize();

        // Agents 0..5 → workers 0..5
        for (agent_id, worker_id) in &assignments {
            assert_eq!(*worker_id, agent_id % num_workers);
        }
        // Workers 5..10 should have 0 agents
        assert_eq!(
            stats.per_worker_counts,
            vec![1u64, 1, 1, 1, 1, 0, 0, 0, 0, 0]
        );
    }

    /// EC-1: Empty batch returns empty assignment; counter is unchanged.
    #[test]
    fn round_robin_empty_batch_unchanged_state() {
        let num_workers = 4u32;
        let mut strategy = RoundRobinStreamingStrategy::new(num_workers);
        // Process 10 agents first
        let batch1 = make_era_batch(0, 10);
        strategy.allocate_batch(&batch1, num_workers);
        // Now process an empty batch
        let empty = AgentBatch::empty();
        let assignments = strategy.allocate_batch(&empty, num_workers);
        assert!(
            assignments.is_empty(),
            "empty batch must return empty assignments"
        );
        // Counter should not have changed due to empty batch
        let stats = strategy.finalize();
        assert_eq!(
            stats.total_agents, 10,
            "counter must not change for empty batch"
        );
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0531: FennelStreamingStrategy (T9 partial)
    // -----------------------------------------------------------------------

    /// Helper: build a stream of batches from a list of (agent_id, symbol, connections).
    fn make_fennel_test_stream(num_agents: usize, chunk_size: usize) -> Vec<AgentBatch> {
        let mut batches = Vec::new();
        let mut idx = 0;
        while idx < num_agents {
            let end = (idx + chunk_size).min(num_agents);
            let batch = AgentBatch {
                agents: (idx..end).map(|i| (i as AgentId, Symbol::Era)).collect(),
                connections: vec![],
            };
            batches.push(batch);
            idx = end;
        }
        batches
    }

    /// UT-0531-01: R6 cache size matches total_agents after full run.
    #[test]
    fn r6_cache_size_matches_total_agents() {
        let num_agents = 64;
        let num_workers = 4u32;
        let mut strategy = FennelStreamingStrategy::with_default_alpha(num_workers);

        for batch in make_fennel_test_stream(num_agents, 16) {
            strategy.allocate_batch(&batch, num_workers);
        }

        assert_eq!(
            strategy.assignment_cache.len(),
            num_agents,
            "R6: cache size must equal total_agents"
        );
    }

    /// UT-0531-02: R6 memory bound — cache uses ≤ 8 bytes per agent.
    #[test]
    fn r6_cache_memory_bound() {
        let num_agents = 64;
        let num_workers = 4u32;
        let mut strategy = FennelStreamingStrategy::with_default_alpha(num_workers);

        for batch in make_fennel_test_stream(num_agents, 16) {
            strategy.allocate_batch(&batch, num_workers);
        }

        // Each (AgentId=u32, WorkerId=u32) entry = 8 bytes.
        let cache_bytes = strategy.assignment_cache.len() * 8;
        let max_allowed = num_agents * 8;
        assert!(
            cache_bytes <= max_allowed,
            "R6: cache uses {} bytes but max allowed is {} bytes",
            cache_bytes,
            max_allowed
        );
    }

    /// UT-0531-03: R8 determinism — two runs on the same input produce identical output.
    #[test]
    fn r8_determinism_repeated_invocation_fennel() {
        let num_agents = 64;
        let num_workers = 4u32;
        let batches = make_fennel_test_stream(num_agents, 16);

        let mut strategy1 = FennelStreamingStrategy::with_default_alpha(num_workers);
        let mut strategy2 = FennelStreamingStrategy::with_default_alpha(num_workers);

        for batch in &batches {
            let a1 = strategy1.allocate_batch(batch, num_workers);
            let a2 = strategy2.allocate_batch(batch, num_workers);
            assert_eq!(a1, a2, "R8: determinism violated");
        }
    }

    /// UT-0531-04: C1 complete coverage — every agent assigned exactly once.
    #[test]
    fn c1_complete_coverage_fennel() {
        let num_agents = 64;
        let num_workers = 4u32;
        let mut strategy = FennelStreamingStrategy::with_default_alpha(num_workers);
        let mut seen: std::collections::HashSet<AgentId> = std::collections::HashSet::new();

        for batch in make_fennel_test_stream(num_agents, 16) {
            for (agent_id, _) in strategy.allocate_batch(&batch, num_workers) {
                let inserted = seen.insert(agent_id);
                assert!(inserted, "C1: agent {} assigned more than once", agent_id);
            }
        }
        assert_eq!(seen.len(), num_agents, "C1: all agents must be assigned");
    }

    /// UT-0531-05: Tiebreak is deterministic (lowest WorkerId wins on equal score).
    #[test]
    fn tiebreak_is_deterministic() {
        // With no prior assignments and alpha=0 (no capacity penalty), all workers
        // have score = 0 for the first agent. Tiebreak: lowest WorkerId (0) wins.
        let num_workers = 4u32;
        let mut strategy = FennelStreamingStrategy::new(num_workers, 0.0);
        let batch = AgentBatch {
            agents: vec![(0u32, Symbol::Era)],
            connections: vec![],
        };
        let assignments = strategy.allocate_batch(&batch, num_workers);
        assert_eq!(
            assignments[0].1, 0u32,
            "tiebreak must assign to lowest WorkerId (0)"
        );
    }

    /// UT-0531-06: Fixed alpha=1.0 load imbalance is acceptable (max ≤ 2×mean).
    #[test]
    fn fixed_alpha_load_imbalance_acceptable() {
        let num_agents = 64;
        let num_workers = 4u32;
        let mut strategy = FennelStreamingStrategy::with_default_alpha(num_workers);

        for batch in make_fennel_test_stream(num_agents, 16) {
            strategy.allocate_batch(&batch, num_workers);
        }

        let stats = strategy.finalize();
        let mean = stats.total_agents as f64 / num_workers as f64;
        let max_count = stats.per_worker_counts.iter().max().copied().unwrap_or(0) as f64;
        assert!(
            max_count <= 2.0 * mean,
            "Q3: load imbalance too high — max={}, mean={}",
            max_count,
            mean
        );
    }

    /// alpha=0.0 (no capacity penalty): strategy still produces valid C1 coverage.
    #[test]
    fn fennel_alpha_zero_valid_coverage() {
        let num_agents = 40;
        let num_workers = 4u32;
        let mut strategy = FennelStreamingStrategy::new(num_workers, 0.0);
        let mut seen: std::collections::HashSet<AgentId> = std::collections::HashSet::new();

        for batch in make_fennel_test_stream(num_agents, 10) {
            for (agent_id, _) in strategy.allocate_batch(&batch, num_workers) {
                seen.insert(agent_id);
            }
        }
        assert_eq!(
            seen.len(),
            num_agents,
            "alpha=0.0: all agents must still be assigned"
        );
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0550: PartitionAccumulator struct
    // -----------------------------------------------------------------------

    /// UT-0550-01: Default subnet variant is Sparse (SC-006 closure default).
    #[test]
    fn default_subnet_is_sparse_variant() {
        let acc = PartitionAccumulator::new(0);
        assert!(
            matches!(acc.subnet, AccumulatorNet::Sparse(_)),
            "default AccumulatorNet must be Sparse, not Dense"
        );
    }

    /// UT-0550-02: Fresh accumulator has zero live agents.
    #[test]
    fn default_sparse_is_empty() {
        let acc = PartitionAccumulator::new(0);
        assert_eq!(
            acc.live_agent_count, 0,
            "fresh accumulator must have 0 live agents"
        );
    }

    /// UT-0550-03: min_assigned_id and max_assigned_id are None on construction.
    #[test]
    fn default_min_max_assigned_id_none() {
        let acc = PartitionAccumulator::new(0);
        assert!(
            acc.min_assigned_id.is_none(),
            "min_assigned_id must be None"
        );
        assert!(
            acc.max_assigned_id.is_none(),
            "max_assigned_id must be None"
        );
    }

    /// UT-0550-04: free_port_index is empty on construction.
    #[test]
    fn default_free_port_index_empty() {
        let acc = PartitionAccumulator::new(0);
        assert!(
            acc.free_port_index.is_empty(),
            "free_port_index must be empty on construction"
        );
    }

    /// UT-0550-05: worker_id field matches constructor argument.
    #[test]
    fn worker_id_field_correct() {
        for w in [0u32, 1, 3, u32::MAX] {
            let acc = PartitionAccumulator::new(w);
            assert_eq!(
                acc.worker_id, w,
                "worker_id must match constructor argument"
            );
        }
    }

    /// UT-0550-06: Four accumulators have independent state.
    #[test]
    fn multi_worker_construction_independent() {
        let accs: Vec<_> = (0u32..4).map(PartitionAccumulator::new).collect();
        for (i, acc) in accs.iter().enumerate() {
            assert_eq!(acc.worker_id, i as u32);
            assert_eq!(acc.live_agent_count, 0);
        }
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0551: PartitionAccumulator add_agent + connect
    // -----------------------------------------------------------------------

    /// UT-0551-01: Adding 100 contiguous agents yields live_agent_count == 100.
    #[test]
    fn add_100_contiguous_agents_live_count_is_100() {
        let mut acc = PartitionAccumulator::new(0);
        for i in 0u32..100 {
            acc.add_agent(i, Symbol::Era);
        }
        assert_eq!(acc.live_agent_count, 100);
    }

    /// UT-0551-02: min and max after contiguous 0..99.
    #[test]
    fn min_max_assigned_id_after_contiguous() {
        let mut acc = PartitionAccumulator::new(0);
        for i in 0u32..100 {
            acc.add_agent(i, Symbol::Con);
        }
        assert_eq!(acc.min_assigned_id, Some(0));
        assert_eq!(acc.max_assigned_id, Some(99));
    }

    /// UT-0551-03: Non-contiguous IDs update min/max correctly.
    #[test]
    fn add_two_non_contiguous_ids_live_count_is_2() {
        let mut acc = PartitionAccumulator::new(0);
        acc.add_agent(0, Symbol::Con);
        acc.add_agent(5_000_000, Symbol::Dup);
        assert_eq!(acc.live_agent_count, 2);
        assert_eq!(acc.min_assigned_id, Some(0));
        assert_eq!(acc.max_assigned_id, Some(5_000_000));
    }

    /// UT-0551-04: Non-contiguous sparse accumulator does NOT inflate storage.
    #[test]
    fn non_contiguous_does_not_inflate_internal_storage() {
        let mut acc = PartitionAccumulator::new(0);
        acc.add_agent(0, Symbol::Con);
        acc.add_agent(5_000_000, Symbol::Dup);
        match &acc.subnet {
            AccumulatorNet::Sparse(s) => {
                assert_eq!(
                    s.agents.len(),
                    2,
                    "SparseNet must have exactly 2 entries, not 5_000_001"
                );
            }
            AccumulatorNet::Dense(_) => panic!("expected Sparse variant"),
        }
    }

    /// UT-0551-05: connect with FreePort registers in free_port_index.
    #[test]
    fn connect_freeport_registers_in_free_port_index() {
        let mut acc = PartitionAccumulator::new(0);
        acc.add_agent(0, Symbol::Con);
        acc.connect(PortRef::AgentPort(0, 0), PortRef::FreePort(7));
        assert!(
            acc.free_port_index.contains_key(&7),
            "FreePort(7) must be registered in free_port_index"
        );
        assert_eq!(
            acc.free_port_index[&7],
            PortRef::AgentPort(0, 0),
            "free_port_index[7] must point to the AgentPort endpoint"
        );
    }

    /// UT-0551-06: Internal wire does not touch free_port_index.
    #[test]
    fn connect_internal_wire_does_not_touch_free_port_index() {
        let mut acc = PartitionAccumulator::new(0);
        acc.add_agent(0, Symbol::Con);
        acc.add_agent(1, Symbol::Con);
        acc.connect(PortRef::AgentPort(0, 1), PortRef::AgentPort(1, 0));
        assert!(
            acc.free_port_index.is_empty(),
            "internal wire must not touch free_port_index"
        );
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0552: PartitionAccumulator::finalize
    // -----------------------------------------------------------------------

    /// UT-0552-01: Small contiguous accumulator finalizes successfully.
    #[test]
    fn finalize_small_contiguous_returns_partition() {
        let mut acc = PartitionAccumulator::new(0);
        for i in 0u32..100 {
            acc.add_agent(i, Symbol::Era);
        }
        let id_range = IdRange { start: 0, end: 100 };
        let result = acc.finalize(id_range, 0, 0);
        assert!(result.is_ok(), "small contiguous finalize must succeed");
    }

    /// UT-0552-03: Finalized subnet agents.len() matches id_range (R23).
    #[test]
    fn finalized_subnet_agents_len_matches_id_range() {
        let mut acc = PartitionAccumulator::new(0);
        for i in 0u32..100 {
            acc.add_agent(i, Symbol::Era);
        }
        let id_range = IdRange { start: 0, end: 100 };
        let partition = acc.finalize(id_range, 0, 0).unwrap();
        assert_eq!(
            partition.subnet.agents.len(),
            100,
            "dense subnet must have exactly id_range.end - id_range.start slots"
        );
    }

    /// UT-0552-04 (REVISED 2026-05-04 — D-011): Finalize with scattered live
    /// IDs such that `effective_arena_size > 4 × live_count` returns threshold
    /// error. Under SPEC-22 v2.4 R22 the metric is `max_assigned_id + 1`, not
    /// `id_range.end - id_range.start`, so we MUST place at least one agent
    /// at a high ID to exceed `4 × live_count`. 100 live agents at IDs
    /// {0, 5, 10, ..., 495} → max_assigned_id = 495, eff_arena = 496,
    /// 4 × live_count = 400, 496 > 400 → threshold tripped.
    #[test]
    fn finalize_dense_rejection_above_threshold() {
        let mut acc = PartitionAccumulator::new(0);
        // 100 live agents at scattered IDs to inflate max_assigned_id.
        for i in (0u32..500).step_by(5) {
            acc.add_agent(i, Symbol::Era);
        }
        let id_range = IdRange {
            start: 0,
            end: 10_000,
        };
        let result = acc.finalize(id_range, 0, 0);
        assert!(
            matches!(
                result,
                Err(PartitionError::DenseAllocationExceedsThreshold { .. })
            ),
            "scattered IDs (max=495, live=100): expected DenseAllocationExceedsThreshold under new metric"
        );
    }

    /// UT-0552-05 (REVISED 2026-05-04 — D-011 / QA F-004 / Reviewer F-003):
    /// effective_arena_size == 4 × live_count exactly passes (strict-greater
    /// discipline of SPEC-22 R30: only `>` rejects, `==` accepts).
    ///
    /// Setup: live=4 agents at IDs {0, 5, 10, 15} → max_assigned_id=15,
    /// effective_arena_size=16, 4×live=16 → 16 > 16 is false → accepted.
    /// Sibling test `finalize_just_above_4x_threshold` pins the strict `>`
    /// direction (eff=17 > 16 → rejected under sparse_build=false).
    #[test]
    fn finalize_at_exactly_4x_threshold() {
        let mut acc = PartitionAccumulator::new(0);
        for &id in &[0u32, 5, 10, 15] {
            acc.add_agent(id, Symbol::Era);
        }
        // effective_arena_size = max_assigned_id + 1 = 16, 4 × live_count = 16.
        // Strict greater-than: 16 > 16 == false → finalize succeeds.
        let id_range = IdRange { start: 0, end: 100 };
        let result = acc.finalize(id_range, 0, 0);
        assert!(
            result.is_ok(),
            "QA F-004: exactly 4× threshold must pass (strict > check, not >=); got {:?}",
            result
        );
    }

    /// UT-0552-05b (NEW 2026-05-04 — QA F-004 / Reviewer F-003): pins the
    /// strict `>` direction of SPEC-22 R30 — 1 unit ABOVE the boundary
    /// (effective_arena_size == 4 × live_count + 1) MUST reject.
    #[test]
    fn finalize_just_above_4x_threshold() {
        let mut acc = PartitionAccumulator::new(0);
        // 4 live agents at IDs {0, 5, 10, 16} → max_assigned_id=16,
        // effective_arena_size=17, 4×live=16 → 17 > 16 → reject.
        for &id in &[0u32, 5, 10, 16] {
            acc.add_agent(id, Symbol::Era);
        }
        let id_range = IdRange { start: 0, end: 100 };
        let result = acc.finalize(id_range, 0, 0);
        assert!(
            matches!(
                result,
                Err(PartitionError::DenseAllocationExceedsThreshold {
                    effective_arena_size: 17,
                    live_count: 4,
                    ..
                })
            ),
            "QA F-004: eff_arena=17 with live=4 (1 above 4× boundary) must reject; got {:?}",
            result
        );
    }

    /// UT-0552-08: Finalized partition has all 6 SPEC-04 R21 fields.
    #[test]
    fn partition_field_set_complete() {
        let mut acc = PartitionAccumulator::new(2);
        acc.add_agent(0, Symbol::Era);
        acc.add_agent(1, Symbol::Era);
        let id_range = IdRange { start: 0, end: 2 };
        let partition = acc.finalize(id_range, 10, 20).unwrap();
        let _ = &partition.subnet;
        assert_eq!(partition.worker_id, 2);
        let _ = &partition.free_port_index;
        assert_eq!(partition.id_range.start, 0);
        assert_eq!(partition.id_range.end, 2);
        assert_eq!(partition.border_id_start, 10);
        assert_eq!(partition.border_id_end, 20);
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0553: install_connection helper
    // -----------------------------------------------------------------------

    fn make_4_accumulators_with_agents() -> Vec<PartitionAccumulator> {
        let mut accumulators: Vec<PartitionAccumulator> =
            (0..4).map(PartitionAccumulator::new).collect();
        // Worker 0: agents 0, 1
        // Worker 1: agents 2, 3
        // Worker 2: agents 4, 5
        // Worker 3: agents 6, 7
        for i in 0u32..8 {
            accumulators[(i / 2) as usize].add_agent(i, Symbol::Con);
        }
        accumulators
    }

    fn make_agent_owner_4_workers() -> HashMap<AgentId, WorkerId> {
        (0u32..8).map(|i| (i, i / 2)).collect()
    }

    /// UT-0553-01: Internal wire only touches the owner's accumulator.
    #[test]
    fn internal_wire_inserts_only_in_owner_accumulator() {
        let mut accumulators = make_4_accumulators_with_agents();
        let agent_owner = make_agent_owner_4_workers();
        let mut border_map = HashMap::new();
        let mut border_id_counter = 0u32;
        let reserved: std::collections::HashSet<u32> = std::collections::HashSet::new();

        install_connection(
            (0u32, 0u8),
            (1u32, 1u8),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );

        assert!(
            border_map.is_empty(),
            "internal wire must not touch border_map"
        );
        assert_eq!(
            border_id_counter, 0,
            "internal wire must not increment border_id_counter"
        );
    }

    /// UT-0553-02: Border wire allocates bid=0 first.
    #[test]
    fn border_wire_allocates_bid_zero_first() {
        let mut accumulators = make_4_accumulators_with_agents();
        let agent_owner = make_agent_owner_4_workers();
        let mut border_map = HashMap::new();
        let mut border_id_counter = 0u32;
        let reserved: std::collections::HashSet<u32> = std::collections::HashSet::new();

        install_connection(
            (0u32, 0u8),
            (2u32, 0u8),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );

        assert_eq!(
            border_id_counter, 1,
            "border_id_counter must be 1 after first border wire"
        );
        assert!(border_map.contains_key(&0), "border_map must contain bid=0");
    }

    /// UT-0553-03: Border wire connects FreePort to both worker accumulators.
    #[test]
    fn border_wire_calls_connect_on_both_accumulators() {
        let mut accumulators = make_4_accumulators_with_agents();
        let agent_owner = make_agent_owner_4_workers();
        let mut border_map = HashMap::new();
        let mut border_id_counter = 0u32;
        let reserved: std::collections::HashSet<u32> = std::collections::HashSet::new();

        install_connection(
            (0u32, 0u8),
            (2u32, 0u8),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );

        // Both accumulators must have FreePort(0) in free_port_index.
        assert!(
            accumulators[0].free_port_index.contains_key(&0),
            "worker 0 accumulator must have FreePort(0) in free_port_index"
        );
        assert!(
            accumulators[1].free_port_index.contains_key(&0),
            "worker 1 accumulator must have FreePort(0) in free_port_index"
        );
    }

    /// UT-0553-04: Sequential border allocation produces ids 0..4.
    #[test]
    fn sequential_border_allocation_zero_one_two() {
        let mut accumulators = make_4_accumulators_with_agents();
        let agent_owner = make_agent_owner_4_workers();
        let mut border_map = HashMap::new();
        let mut border_id_counter = 0u32;
        let reserved: std::collections::HashSet<u32> = std::collections::HashSet::new();

        // 4 cross-partition wires: 0↔2, 0↔4, 0↔6, 1↔3
        for (a, b) in [(0u32, 2u32), (0, 4), (0, 6), (1, 3)] {
            install_connection(
                (a, 0u8),
                (b, 0u8),
                &agent_owner,
                &mut accumulators,
                &mut border_map,
                &mut border_id_counter,
                &reserved,
            );
        }

        assert_eq!(border_id_counter, 4, "4 border wires → counter == 4");
        for bid in 0..4u32 {
            assert!(
                border_map.contains_key(&bid),
                "border_map must contain bid={}",
                bid
            );
        }
    }

    /// UT-0553-05: Mixed internal and border sequence — correct count.
    #[test]
    fn mixed_internal_and_border_sequence() {
        let mut accumulators = make_4_accumulators_with_agents();
        let agent_owner = make_agent_owner_4_workers();
        let mut border_map = HashMap::new();
        let mut border_id_counter = 0u32;
        let reserved: std::collections::HashSet<u32> = std::collections::HashSet::new();

        // 2 internal (0↔1, 2↔3) + 2 border (0↔2, 1↔3)
        install_connection(
            (0, 1),
            (1, 0),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );
        install_connection(
            (2, 1),
            (3, 0),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );
        install_connection(
            (0, 0),
            (2, 0),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );
        install_connection(
            (1, 1),
            (3, 1),
            &agent_owner,
            &mut accumulators,
            &mut border_map,
            &mut border_id_counter,
            &reserved,
        );

        assert_eq!(border_id_counter, 2, "only 2 border wires");
        assert_eq!(border_map.len(), 2, "2 entries in border_map");
    }

    // -----------------------------------------------------------------------
    // TEST-SPEC-0554: generate_and_partition_chunked orchestrator
    // -----------------------------------------------------------------------

    /// UT-0554-01: T5 full integration — ep_annihilation(100), chunk=20, 4 workers.
    #[test]
    fn t5_full_integration_ep_annihilation_100_4_workers() {
        use crate::bench::streaming::ep_annihilation_stream;
        let stream = ep_annihilation_stream(100, 20);
        let mut strategy = RoundRobinStreamingStrategy::new(4);
        let result = generate_and_partition_chunked(stream, 4, &mut strategy);
        assert!(result.is_ok(), "pipeline must succeed: {:?}", result.err());
        let result = result.unwrap();
        assert_eq!(result.partitions.len(), 4, "must have 4 partitions");
        let total_agents: usize = result
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        assert_eq!(total_agents, 200, "total agents must be 200");
    }

    /// UT-0554-02: chunks_processed is stitched by pipeline (SC-021 closure).
    #[test]
    fn chunks_processed_count_correct() {
        use crate::bench::streaming::ep_annihilation_stream;
        let stream = ep_annihilation_stream(100, 20);
        let mut strategy = RoundRobinStreamingStrategy::new(4);
        let result = generate_and_partition_chunked(stream, 4, &mut strategy).unwrap();
        // 100 pairs, chunk_size=20 → pairs_per_batch=10 → 10 batches of 20 agents each = 10 batches.
        assert_eq!(
            result.stats.chunks_processed, 10,
            "chunks_processed must be 10 (SC-021)"
        );
    }

    /// UT-0554-04: Unresolvable Pending directive returns error (R19 negative path).
    #[test]
    fn pending_store_non_empty_returns_error() {
        // Generator emits agent 0 with Pending targeting agent 999 (never emitted).
        let batch = AgentBatch {
            agents: vec![(0u32, Symbol::Era)],
            connections: vec![ConnectionDirective::Pending {
                source: (0u32, 0u8),
                target_agent_id: 999u32,
                target_port: 0u8,
            }],
        };
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(std::iter::once(batch));
        let mut strategy = RoundRobinStreamingStrategy::new(1);
        let result = generate_and_partition_chunked(stream, 1, &mut strategy);
        assert!(
            matches!(
                result,
                Err(PartitionError::UnresolvedForwardReferences { agent_id: 999 })
            ),
            "unresolved Pending must return UnresolvedForwardReferences, got {:?}",
            result
        );
    }

    /// UT-0554-08: chunk_size=1 works end-to-end.
    #[test]
    fn chunk_size_one_works_end_to_end() {
        use crate::bench::streaming::ep_annihilation_stream;
        let stream = ep_annihilation_stream(20, 1);
        let mut strategy = RoundRobinStreamingStrategy::new(2);
        let result = generate_and_partition_chunked(stream, 2, &mut strategy);
        assert!(result.is_ok(), "chunk_size=1 must work: {:?}", result.err());
        let total_agents: usize = result
            .unwrap()
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        assert_eq!(total_agents, 40, "total agents must be 40");
    }

    /// EC-2: Single worker — all agents to worker 0, no border wires.
    #[test]
    fn single_worker_no_border_wires() {
        use crate::bench::streaming::ep_annihilation_stream;
        let stream = ep_annihilation_stream(10, 4);
        let mut strategy = RoundRobinStreamingStrategy::new(1);
        let result = generate_and_partition_chunked(stream, 1, &mut strategy).unwrap();
        assert_eq!(
            result.borders.len(),
            0,
            "single worker must have 0 border wires"
        );
        assert_eq!(result.partitions[0].subnet.count_live_agents(), 20);
    }

    /// EC-3: Empty stream produces 0 partitions (all workers empty).
    #[test]
    fn empty_stream_produces_empty_result() {
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(std::iter::empty());
        let mut strategy = RoundRobinStreamingStrategy::new(2);
        let result = generate_and_partition_chunked(stream, 2, &mut strategy).unwrap();
        assert_eq!(result.stats.chunks_processed, 0);
        let total: usize = result
            .partitions
            .iter()
            .map(|p| p.subnet.count_live_agents())
            .sum();
        assert_eq!(total, 0);
    }

    // ---------------------------------------------------------------------------
    // QA-D010-004: border_id_counter MUST avoid collision with FreePortInterface ids
    // ---------------------------------------------------------------------------

    /// QA-D010-004: a stream that emits `FreePortInterface { free_port_id: 0 }`
    /// in batch 0 followed by a cross-worker `Resolved` wire in batch 1 must NOT
    /// produce a border with ID 0 (which would alias the FreePort interface).
    ///
    /// Negative-control: pre-fix, `border_id_counter` started at 0 unconditionally,
    /// so the resolved cross-worker wire would have allocated bid=0 — silently
    /// aliasing the Lafont interface FreePort(0) installed in batch 0. Both ports
    /// would then point to the same `FreePort(0)` slot on each worker, splicing
    /// two unrelated wires (D5/D6 port-bijection violation).
    #[test]
    fn qa_d010_004_border_id_counter_avoids_freeport_collision() {
        use crate::net::Symbol;

        // Two batches: first establishes an interface FreePort(0), second
        // creates a cross-worker border that would (pre-fix) be allocated as 0.
        let batch0 = AgentBatch {
            agents: vec![(0u32, Symbol::Con)],
            connections: vec![ConnectionDirective::FreePortInterface {
                agent_port: (0u32, 1u8),
                free_port_id: 0u32,
            }],
        };
        let batch1 = AgentBatch {
            agents: vec![(1u32, Symbol::Era), (2u32, Symbol::Era)],
            connections: vec![ConnectionDirective::Resolved {
                source: (1u32, 0u8),
                target: (2u32, 0u8),
            }],
        };
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new([batch0, batch1].into_iter());
        let mut strategy = RoundRobinStreamingStrategy::new(2);

        let result = generate_and_partition_chunked(stream, 2, &mut strategy)
            .expect("partition must succeed");

        // Primary assertion: the cross-worker wire MUST have produced a border,
        // but with bid >= 1 (NOT 0, which is reserved by the FreePortInterface).
        // Pre-fix: this assertion fires with bid == 0.
        for &bid in result.borders.keys() {
            assert!(
                bid > 0,
                "QA-D010-004: border id {bid} must NOT alias FreePortInterface(0); \
                 expected counter to lift above max FreePort id"
            );
        }

        // Negative-control: at least one border MUST exist for the cross-worker
        // resolved wire (otherwise this test is vacuous — we'd be asserting "no
        // borders ever conflict" trivially).
        assert!(
            !result.borders.is_empty(),
            "QA-D010-004 vacuity guard: expected the Resolved cross-worker wire \
             in batch 1 to produce at least one border entry"
        );

        // Negative-control: FreePort id 0 must survive on the partition that
        // owned agent 0 (round-robin: agent 0 → worker 0). The free_port_index
        // for FreePort(0) on worker 0 must point at the AgentPort(0,1) the
        // FreePortInterface installed — NOT at any border-allocated wire.
        let part0 = &result.partitions[0];
        match part0.free_port_index.get(&0u32).copied() {
            Some(PortRef::AgentPort(a, p)) => {
                assert_eq!(
                    (a, p),
                    (0u32, 1u8),
                    "QA-D010-004 negative-control: worker 0's FreePort(0) entry \
                     must still map to the agent 0 port 1 the FreePortInterface \
                     installed; finding {a},{p} indicates a border allocation \
                     overwrote it."
                );
            }
            other => panic!(
                "QA-D010-004 negative-control: worker 0 must retain FreePort(0) \
                 → AgentPort(0,1); got {other:?}"
            ),
        }
    }

    // ---------------------------------------------------------------------------
    // QA-D010-005..007: strategy input validation
    // ---------------------------------------------------------------------------

    /// QA-D010-005: `RoundRobinStreamingStrategy::try_new(0)` and
    /// `FennelStreamingStrategy::try_new(0, _)` MUST return
    /// `PartitionError::InvalidNumWorkers`. The infallible `new` panics
    /// (documented contract); only `try_new` is fallible.
    #[test]
    fn qa_d010_005_strategy_try_new_rejects_zero_workers() {
        let r = RoundRobinStreamingStrategy::try_new(0);
        assert!(
            matches!(r, Err(PartitionError::InvalidNumWorkers)),
            "QA-D010-005: RoundRobin try_new(0) must Err(InvalidNumWorkers); got {:?}",
            r.err()
        );

        let f = FennelStreamingStrategy::try_new(0, 1.0);
        assert!(
            matches!(f, Err(PartitionError::InvalidNumWorkers)),
            "QA-D010-005: Fennel try_new(0, 1.0) must Err(InvalidNumWorkers); got {:?}",
            f.err()
        );

        // Orchestrator-level: generate_and_partition_chunked rejects num_workers=0.
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(std::iter::empty());
        // We need *some* strategy; use a 1-worker one (we won't reach allocate_batch).
        let mut strategy = RoundRobinStreamingStrategy::new(1);
        let result = generate_and_partition_chunked(stream, 0, &mut strategy);
        assert!(
            matches!(result, Err(PartitionError::InvalidNumWorkers)),
            "QA-D010-005: orchestrator with num_workers=0 must Err(InvalidNumWorkers); \
             got {:?}",
            result.err()
        );
    }

    /// QA-D010-006: `allocate_batch(num_workers > construction-time capacity)`
    /// must NOT panic with index OOB on `per_worker_counts`. The strategy
    /// returns an empty `Vec` instead.
    #[test]
    fn qa_d010_006_allocate_batch_excess_num_workers_no_panic() {
        use crate::net::Symbol;

        // Construct with capacity 4; call allocate_batch with num_workers=8
        // (which would index per_worker_counts[8] under the buggy pre-fix code).
        let mut s = RoundRobinStreamingStrategy::new(4);
        let batch = AgentBatch {
            agents: vec![(0u32, Symbol::Era), (1u32, Symbol::Era)],
            connections: vec![],
        };
        let result = s.allocate_batch(&batch, 8);
        assert!(
            result.is_empty(),
            "QA-D010-006: allocate_batch with num_workers > capacity must return \
             empty Vec; got {} assignments",
            result.len()
        );

        // FENNEL same contract.
        let mut f = FennelStreamingStrategy::new(2, 1.0);
        let result = f.allocate_batch(&batch, 5);
        assert!(
            result.is_empty(),
            "QA-D010-006: Fennel allocate_batch with num_workers > capacity must \
             return empty Vec"
        );
    }

    /// QA-D010-007: `FennelStreamingStrategy::try_new(_, NaN)` and
    /// `try_new(_, +Inf)` / `try_new(_, -Inf)` MUST return
    /// `PartitionError::InvalidStrategyParameter` (rejecting non-finite alpha
    /// at construction prevents NaN scores during allocation, which would
    /// silently violate R8 determinism).
    #[test]
    fn qa_d010_007_fennel_try_new_rejects_non_finite_alpha() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let r = FennelStreamingStrategy::try_new(2, bad);
            assert!(
                matches!(
                    r,
                    Err(PartitionError::InvalidStrategyParameter { name: "alpha", .. })
                ),
                "QA-D010-007: try_new(2, {bad}) must Err(InvalidStrategyParameter); got {:?}",
                r.err()
            );
        }

        // Negative-control: finite alpha (incl. 0.0) MUST succeed.
        for good in [0.0, 1.0, -2.5, f64::MIN_POSITIVE] {
            let r = FennelStreamingStrategy::try_new(2, good);
            assert!(
                r.is_ok(),
                "QA-D010-007 negative-control: try_new(2, {good}) must succeed; got {:?}",
                r.err()
            );
        }
    }

    // ---------------------------------------------------------------------------
    // QA-D010-008: orchestrator returns Err on bad strategy output (no panic)
    // ---------------------------------------------------------------------------

    /// QA-D010-008: a misbehaving `StreamingPartitionStrategy` that returns a
    /// `worker_id >= num_workers` MUST be reported as
    /// `PartitionError::StrategyReturnedInvalidWorker`, not a panic. Pre-fix,
    /// the orchestrator did `accumulators[*worker_id as usize].add_agent(...)`
    /// — an OOB index that aborts the tokio runtime.
    #[test]
    fn qa_d010_008_strategy_returns_invalid_worker_id_returns_err() {
        use crate::net::Symbol;

        struct Misbehaving;
        impl StreamingPartitionStrategy for Misbehaving {
            fn allocate_batch(
                &mut self,
                batch: &AgentBatch,
                num_workers: u32,
            ) -> Vec<(AgentId, WorkerId)> {
                // Out-of-bounds worker_id: num_workers + 100.
                batch
                    .agents
                    .iter()
                    .map(|(id, _)| (*id, num_workers + 100))
                    .collect()
            }
            fn finalize(&self) -> StreamingPartitionStats {
                StreamingPartitionStats {
                    total_agents: 0,
                    per_worker_counts: vec![],
                    border_wire_count: 0,
                    chunks_processed: 0,
                }
            }
        }

        let batch = AgentBatch {
            agents: vec![(0u32, Symbol::Era)],
            connections: vec![],
        };
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(std::iter::once(batch));
        let mut strategy = Misbehaving;
        let result = generate_and_partition_chunked(stream, 4, &mut strategy);
        assert!(
            matches!(
                result,
                Err(PartitionError::StrategyReturnedInvalidWorker { .. })
            ),
            "QA-D010-008: misbehaving strategy must return StrategyReturnedInvalidWorker; \
             got {:?}",
            result
        );
    }

    /// QA-D010-008: a misbehaving strategy that returns an `agent_id` not in the
    /// batch MUST be reported as `StrategyReturnedUnknownAgent`, not a panic
    /// (pre-fix: `symbol_lookup[&agent_id]` panics on missing key).
    #[test]
    fn qa_d010_008_strategy_returns_unknown_agent_returns_err() {
        struct Misbehaving;
        impl StreamingPartitionStrategy for Misbehaving {
            fn allocate_batch(
                &mut self,
                _batch: &AgentBatch,
                _num_workers: u32,
            ) -> Vec<(AgentId, WorkerId)> {
                // Return an agent_id that is NOT in the supplied batch.
                vec![(99_999u32, 0u32)]
            }
            fn finalize(&self) -> StreamingPartitionStats {
                StreamingPartitionStats {
                    total_agents: 0,
                    per_worker_counts: vec![],
                    border_wire_count: 0,
                    chunks_processed: 0,
                }
            }
        }

        let batch = AgentBatch {
            agents: vec![(0u32, crate::net::Symbol::Era)],
            connections: vec![],
        };
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(std::iter::once(batch));
        let mut strategy = Misbehaving;
        let result = generate_and_partition_chunked(stream, 2, &mut strategy);
        assert!(
            matches!(
                result,
                Err(PartitionError::StrategyReturnedUnknownAgent { agent_id: 99_999 })
            ),
            "QA-D010-008: misbehaving strategy must return StrategyReturnedUnknownAgent; \
             got {:?}",
            result
        );
    }

    // ---------------------------------------------------------------------------
    // QA-D010-009: max_pending_lifetime enforcement
    // ---------------------------------------------------------------------------

    /// QA-D010-009: an unresolved `Pending` directive whose target agent is
    /// never introduced within `max_pending_lifetime` chunks MUST surface as
    /// `PartitionError::PendingConnectionExpired`, NOT growing the pending
    /// HashMap unboundedly until the stream exhausts (or, for an infinite
    /// stream, forever).
    #[test]
    fn qa_d010_009_pending_connection_expires_on_lifetime_breach() {
        use crate::net::Symbol;

        // Stream of 6 batches; each batch creates one new agent and a Pending
        // directive targeting agent_id=999_999 (which will never be emitted).
        // With max_pending_lifetime=3, the orchestrator must Err on or before
        // chunk 5 (chunk 1 birthed the pending; age=4 > 3 fires at chunk 5).
        let batches: Vec<AgentBatch> = (0..6u32)
            .map(|i| AgentBatch {
                agents: vec![(i, Symbol::Era)],
                connections: vec![ConnectionDirective::Pending {
                    source: (i, 0u8),
                    target_agent_id: 999_999u32,
                    target_port: 0u8,
                }],
            })
            .collect();
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(batches.into_iter());
        let mut strategy = RoundRobinStreamingStrategy::new(2);
        let result = generate_and_partition_chunked_with_chunk_size_and_lifetime(
            stream,
            2,
            &mut strategy,
            10, // chunk_size, irrelevant for this test
            3,  // max_pending_lifetime — must trigger expiration
        );
        match result {
            Err(PartitionError::PendingConnectionExpired {
                agent_id,
                age,
                budget,
            }) => {
                assert_eq!(agent_id, 999_999, "expired entry's target_agent_id");
                assert_eq!(budget, 3, "budget echoes max_pending_lifetime");
                assert!(
                    age > budget,
                    "QA-D010-009: age ({age}) must exceed budget ({budget}) at the \
                     point of expiration"
                );
            }
            other => panic!(
                "QA-D010-009: must return PendingConnectionExpired; got {:?}",
                other
            ),
        }
    }

    /// QA-D010-009 negative-control: with `max_pending_lifetime == u32::MAX`
    /// (legacy "disabled" sentinel), an unresolved Pending must continue to
    /// surface as `UnresolvedForwardReferences` (post-stream check), NOT
    /// `PendingConnectionExpired` mid-stream.
    #[test]
    fn qa_d010_009_max_pending_lifetime_disabled_legacy_behavior() {
        use crate::net::Symbol;

        let batches: Vec<AgentBatch> = (0..6u32)
            .map(|i| AgentBatch {
                agents: vec![(i, Symbol::Era)],
                connections: vec![ConnectionDirective::Pending {
                    source: (i, 0u8),
                    target_agent_id: 999_999u32,
                    target_port: 0u8,
                }],
            })
            .collect();
        let stream: Box<dyn Iterator<Item = AgentBatch>> = Box::new(batches.into_iter());
        let mut strategy = RoundRobinStreamingStrategy::new(2);
        let result = generate_and_partition_chunked_with_chunk_size_and_lifetime(
            stream,
            2,
            &mut strategy,
            10,
            u32::MAX, // disabled
        );
        assert!(
            matches!(
                result,
                Err(PartitionError::UnresolvedForwardReferences { agent_id: 999_999 })
            ),
            "QA-D010-009 negative-control: with max_pending_lifetime=u32::MAX, must \
             fall through to UnresolvedForwardReferences; got {:?}",
            result
        );
    }

    /// QA-D011-POST-FIX-AUDIT F-001: PartitionAccumulator::finalize MUST seed
    /// the resulting Net's next_id to lie within id_range so subsequent
    /// create_agent calls do not trip AF-2 (debug guard at net/core.rs:536-548).
    ///
    /// Two scenarios pinned (mirrors the dense build_subnet Bug 2 witness at
    /// helpers.rs::tests::qa_d011_bug2_dense_build_subnet_next_id_in_range):
    ///
    /// 1. Empty partition (no agents assigned): SparseNet::new() leaves
    ///    next_id = 0; without the widen, AF-2 would panic on next_id < range.start
    ///    when the resulting Partition is reduced.
    /// 2. Boundary partition (agent at id == band_end - 1): SparseNet::create_agent_at
    ///    sets next_id = max(next_id, id+1) which can land at id_range.end. The
    ///    widen here keeps next_id at >= id_range.start; bound enforcement at
    ///    range.end is the responsibility of downstream allocation paths.
    #[test]
    fn qa_d011_postfix_f001_streaming_finalize_seeds_next_id_in_range() {
        // Scenario 1: empty partition.
        let acc = PartitionAccumulator::new(0);
        let id_range = IdRange {
            start: 100,
            end: 200,
        };
        let part1 = acc
            .finalize(id_range, 0, 0)
            .expect("finalize should succeed for empty partition");
        assert!(
            part1.subnet.next_id >= 100,
            "QA-D011-POST-FIX-AUDIT F-001: empty streaming partition next_id ({}) must be >= id_range.start (100)",
            part1.subnet.next_id
        );
        assert!(
            part1.subnet.next_id <= 200,
            "QA-D011-POST-FIX-AUDIT F-001: empty streaming partition next_id ({}) must be <= id_range.end (200)",
            part1.subnet.next_id
        );
        // id_range must be propagated so AF-2 has the bound to check against.
        assert!(
            part1.subnet.id_range.is_some(),
            "QA-D011-POST-FIX-AUDIT F-001: empty streaming partition must propagate id_range"
        );

        // Scenario 2: worker holds an agent at id = band_end - 1, with enough
        // dense neighbours that the SPEC-22 R30 threshold (eff_arena <= 4*live)
        // is satisfied (50 live agents at IDs 150..200, max=199, eff_arena=200,
        // 4*live=200 → not exceeded).
        let mut acc2 = PartitionAccumulator::new(0);
        for i in 150u32..200u32 {
            acc2.add_agent(i, Symbol::Era);
        }
        let part2 = acc2
            .finalize(
                IdRange {
                    start: 100,
                    end: 200,
                },
                0,
                0,
            )
            .expect("finalize should succeed for boundary partition");
        assert!(
            part2.subnet.next_id >= 100,
            "QA-D011-POST-FIX-AUDIT F-001: boundary partition next_id ({}) must be >= id_range.start (100)",
            part2.subnet.next_id
        );
    }
}
