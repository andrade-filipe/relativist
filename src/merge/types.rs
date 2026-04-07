//! Types for merge and grid cycle (SPEC-05, Section 4.1).
//!
//! GridMetrics, WorkerRoundStats, and GridConfig are pure data structures
//! with no logic. They are used by run_grid and the merge function.

use std::time::Duration;

use crate::partition::WorkerId;

/// Metrics collected during grid loop execution (SPEC-05, R34-R35, R35a).
///
/// Accumulates per-round data for experimental analysis (SPEC-09).
/// Inspired by GridMetrics from the Haskell prototype (AC-004).
#[derive(Debug, Clone, Default)]
pub struct GridMetrics {
    /// Total number of rounds executed.
    pub rounds: u32,

    /// Sum of all interactions (local + border) across all rounds.
    pub total_interactions: u64,

    /// Per-rule interaction totals across all rounds and all sources
    /// (workers + border resolution):
    /// [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    /// Required by SPEC-11 R12 for Prometheus `interactions_by_rule_total`.
    pub total_interactions_by_rule: [u64; 6],

    /// Local worker interactions per round.
    pub local_interactions_per_round: Vec<u64>,

    /// Border interactions (coordinator, after merge) per round.
    pub border_interactions_per_round: Vec<u64>,

    /// Border redexes detected by merge per round.
    pub border_redexes_per_round: Vec<u32>,

    /// Number of live agents at the start of each round.
    pub agents_per_round: Vec<usize>,

    /// Partitioning time per round.
    pub partition_time_per_round: Vec<Duration>,

    /// Local reduction time (all workers) per round.
    /// In local simulation, includes rebuild_free_port_index unless
    /// index_rebuild_time_per_round is separately tracked.
    pub compute_time_per_round: Vec<Duration>,

    /// Structural merge time per round (excludes border resolution).
    pub merge_time_per_round: Vec<Duration>,

    /// Time for reduce_all after merge per round (border resolution).
    pub border_reduce_time_per_round: Vec<Duration>,

    /// Time for rebuild_free_port_index per round (SHOULD, R35a).
    /// Enables accurate overhead decomposition for SPEC-09 benchmarks.
    pub index_rebuild_time_per_round: Vec<Duration>,

    // --- Network metrics (SPEC-06 R33-R34, populated in distributed mode) ---
    /// Total bytes sent by the coordinator per round (headers + payloads).
    pub bytes_sent_per_round: Vec<usize>,

    /// Total bytes received by the coordinator per round (headers + payloads).
    pub bytes_received_per_round: Vec<usize>,

    /// Wall-clock time to send all partitions per round.
    pub network_send_time_per_round: Vec<Duration>,

    /// Wall-clock time to collect all results per round.
    pub network_recv_time_per_round: Vec<Duration>,

    /// Wall-clock total execution time.
    pub total_time: Duration,

    /// Did the grid converge to Normal Form?
    /// false if max_rounds was reached before convergence.
    pub converged: bool,

    /// Per-worker statistics, per round (populated in distributed context).
    pub worker_stats_per_round: Vec<Vec<WorkerRoundStats>>,
}

impl GridMetrics {
    /// Returns the total bytes transferred across all rounds (sent + received).
    /// SPEC-06, Section 4.10.
    pub fn total_network_bytes(&self) -> usize {
        self.bytes_sent_per_round.iter().sum::<usize>()
            + self.bytes_received_per_round.iter().sum::<usize>()
    }

    /// Returns the communication overhead as a fraction of total time.
    /// Formula: (sum(send_time) + sum(recv_time)) / total_time
    /// Cf. DISC-006 v2, Section 1.1; SPEC-06 R35.
    pub fn network_overhead_fraction(&self) -> f64 {
        let send_total: Duration = self.network_send_time_per_round.iter().sum();
        let recv_total: Duration = self.network_recv_time_per_round.iter().sum();
        let network_total = send_total + recv_total;
        if self.total_time.is_zero() {
            0.0
        } else {
            network_total.as_secs_f64() / self.total_time.as_secs_f64()
        }
    }
}

/// Statistics of a single worker in a specific round.
/// Canonical definition: SPEC-05 R37. Resolves SPEC-11 OQ-1.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerRoundStats {
    /// Identifier of the worker.
    pub worker_id: WorkerId,
    /// Number of live agents in the partition before local reduction.
    pub agents_before: usize,
    /// Number of live agents in the partition after local reduction.
    pub agents_after: usize,
    /// Number of local redexes reduced by this worker.
    pub local_redexes: usize,
    /// Wall-clock duration of reduce_all for this worker (seconds).
    pub reduce_duration_secs: f64,
    /// Per-rule interaction counts:
    /// [CON-CON, CON-DUP, CON-ERA, DUP-DUP, DUP-ERA, ERA-ERA].
    pub interactions_by_rule: [u64; 6],
}

/// Configuration for the grid loop (SPEC-05, R25, R29).
///
/// The partition strategy is NOT stored here because trait objects
/// are not Clone. It is passed as a separate parameter to `run_grid`.
#[derive(Debug, Clone)]
pub struct GridConfig {
    /// Number of workers for parallel reduction.
    /// Must be >= 1.
    pub num_workers: u32,

    /// Maximum number of rounds before forced termination.
    /// None = no limit (loop until Normal Form).
    /// Some(limit) = terminate after `limit` rounds even if not converged (R29).
    pub max_rounds: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // === GridMetrics tests (TASK-0060) ===

    // T1: GridMetrics::default() initializes all fields correctly
    #[test]
    fn test_grid_metrics_default() {
        let m = GridMetrics::default();
        assert_eq!(m.rounds, 0);
        assert_eq!(m.total_interactions, 0);
        assert_eq!(m.total_interactions_by_rule, [0; 6]);
        assert!(m.local_interactions_per_round.is_empty());
        assert!(m.border_interactions_per_round.is_empty());
        assert!(m.border_redexes_per_round.is_empty());
        assert!(m.agents_per_round.is_empty());
        assert!(m.partition_time_per_round.is_empty());
        assert!(m.compute_time_per_round.is_empty());
        assert!(m.merge_time_per_round.is_empty());
        assert!(m.border_reduce_time_per_round.is_empty());
        assert!(m.index_rebuild_time_per_round.is_empty());
        assert!(m.bytes_sent_per_round.is_empty());
        assert!(m.bytes_received_per_round.is_empty());
        assert!(m.network_send_time_per_round.is_empty());
        assert!(m.network_recv_time_per_round.is_empty());
        assert_eq!(m.total_time, Duration::ZERO);
        assert!(!m.converged);
        assert!(m.worker_stats_per_round.is_empty());
    }

    // T2: GridMetrics fields are writable and accessible
    #[test]
    fn test_grid_metrics_fields_writable() {
        let mut m = GridMetrics::default();
        m.rounds = 5;
        m.total_interactions = 1000;
        m.total_interactions_by_rule = [10, 20, 30, 40, 50, 60];
        m.local_interactions_per_round.push(100);
        m.border_redexes_per_round.push(3);
        m.converged = true;
        m.total_time = Duration::from_secs(42);

        assert_eq!(m.rounds, 5);
        assert_eq!(m.total_interactions, 1000);
        assert_eq!(m.total_interactions_by_rule[1], 20);
        assert_eq!(m.local_interactions_per_round[0], 100);
        assert_eq!(m.border_redexes_per_round[0], 3);
        assert!(m.converged);
        assert_eq!(m.total_time, Duration::from_secs(42));
    }

    // === WorkerRoundStats tests (TASK-0061) ===

    // T1: WorkerRoundStats construction and field access
    #[test]
    fn test_worker_round_stats_construction() {
        let stats = WorkerRoundStats {
            worker_id: 2,
            agents_before: 100,
            agents_after: 50,
            local_redexes: 25,
            reduce_duration_secs: 0.042,
            interactions_by_rule: [5, 3, 7, 2, 4, 4],
        };
        assert_eq!(stats.worker_id, 2);
        assert_eq!(stats.agents_before, 100);
        assert_eq!(stats.agents_after, 50);
        assert_eq!(stats.local_redexes, 25);
        assert!((stats.reduce_duration_secs - 0.042).abs() < f64::EPSILON);
        assert_eq!(stats.interactions_by_rule, [5, 3, 7, 2, 4, 4]);
    }

    // T2: WorkerRoundStats serde round-trip
    #[test]
    fn test_worker_round_stats_serde() {
        let stats = WorkerRoundStats {
            worker_id: 1,
            agents_before: 200,
            agents_after: 100,
            local_redexes: 50,
            reduce_duration_secs: 1.5,
            interactions_by_rule: [10, 20, 5, 8, 3, 4],
        };
        let bytes = bincode::serialize(&stats).unwrap();
        let deserialized: WorkerRoundStats = bincode::deserialize(&bytes).unwrap();
        assert_eq!(deserialized.worker_id, stats.worker_id);
        assert_eq!(deserialized.agents_before, stats.agents_before);
        assert_eq!(deserialized.agents_after, stats.agents_after);
        assert_eq!(deserialized.local_redexes, stats.local_redexes);
        assert!(
            (deserialized.reduce_duration_secs - stats.reduce_duration_secs).abs() < f64::EPSILON
        );
        assert_eq!(
            deserialized.interactions_by_rule,
            stats.interactions_by_rule
        );
    }

    // T3: interactions_by_rule has exactly 6 elements
    #[test]
    fn test_worker_round_stats_by_rule_size() {
        let stats = WorkerRoundStats {
            worker_id: 0,
            agents_before: 0,
            agents_after: 0,
            local_redexes: 0,
            reduce_duration_secs: 0.0,
            interactions_by_rule: [0; 6],
        };
        assert_eq!(stats.interactions_by_rule.len(), 6);
    }

    // === GridConfig tests (TASK-0062) ===

    // T1: GridConfig with max_rounds
    #[test]
    fn test_grid_config_with_max_rounds() {
        let config = GridConfig {
            num_workers: 4,
            max_rounds: Some(100),
        };
        assert_eq!(config.num_workers, 4);
        assert_eq!(config.max_rounds, Some(100));
    }

    // T2: GridConfig with no round limit
    #[test]
    fn test_grid_config_no_limit() {
        let config = GridConfig {
            num_workers: 8,
            max_rounds: None,
        };
        assert_eq!(config.num_workers, 8);
        assert_eq!(config.max_rounds, None);
    }

    // === GridMetrics network extensions (TASK-0094) ===

    // T1: total_network_bytes with known values
    #[test]
    fn test_total_network_bytes() {
        let mut m = GridMetrics::default();
        m.bytes_sent_per_round = vec![100, 200, 300];
        m.bytes_received_per_round = vec![50, 150, 250];
        assert_eq!(m.total_network_bytes(), 1050);
    }

    // T2: total_network_bytes with no rounds
    #[test]
    fn test_total_network_bytes_empty() {
        let m = GridMetrics::default();
        assert_eq!(m.total_network_bytes(), 0);
    }

    // T3: network_overhead_fraction with known durations
    #[test]
    fn test_network_overhead_fraction() {
        let mut m = GridMetrics::default();
        m.network_send_time_per_round = vec![Duration::from_secs(1), Duration::from_secs(2)];
        m.network_recv_time_per_round = vec![Duration::from_secs(3), Duration::from_secs(4)];
        m.total_time = Duration::from_secs(20);
        // (1+2+3+4) / 20 = 10/20 = 0.5
        let fraction = m.network_overhead_fraction();
        assert!((fraction - 0.5).abs() < f64::EPSILON);
    }

    // T4: network_overhead_fraction returns 0.0 when total_time is zero
    #[test]
    fn test_network_overhead_fraction_zero_time() {
        let m = GridMetrics::default();
        assert_eq!(m.network_overhead_fraction(), 0.0);
    }

    // T5: network_overhead_fraction with empty Vecs
    #[test]
    fn test_network_overhead_fraction_empty() {
        let mut m = GridMetrics::default();
        m.total_time = Duration::from_secs(10);
        assert_eq!(m.network_overhead_fraction(), 0.0);
    }
}
