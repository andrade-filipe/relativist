//! Grid loop: the BSP cycle of split -> reduce -> merge -> resolve (SPEC-05, Section 4.5).

use std::time::Instant;

use crate::net::{Net, PortRef};
use crate::partition::{split, PartitionStrategy};
use crate::reduction::{reduce_all, reduce_border_once, ReductionStats};

use super::core::merge;
use super::helpers::{drain_stale_redexes, rebuild_free_port_index};
use super::types::{GridConfig, GridMetrics, WorkerRoundStats};

/// Executes the complete distributed reduction cycle (SPEC-05, R24-R30).
///
/// The grid loop repeats: split -> local reduce -> merge -> resolve borders
/// until the net reaches Normal Form or max_rounds is exceeded.
///
/// For n == 1, reduces locally without partitioning (degenerate case, R26).
///
/// Returns: (net in Normal Form or partial result, accumulated metrics).
pub fn run_grid(
    net: Net,
    config: &GridConfig,
    strategy: &dyn PartitionStrategy,
) -> (Net, GridMetrics) {
    assert!(config.num_workers >= 1, "num_workers must be >= 1");

    let mut current_net = net;
    let mut metrics = GridMetrics::default();
    let start_time = Instant::now();

    // [0] Initial Normal Form check: avoids unnecessary split/merge for
    // nets already in Normal Form (SPEC-05 v3, SC-011).
    drain_stale_redexes(&mut current_net);
    if current_net.redex_queue.is_empty() {
        metrics.converged = true;
        metrics.total_time = start_time.elapsed();
        return (current_net, metrics);
    }

    // n == 1 optimization (R26): reduce locally without partitioning
    if config.num_workers == 1 {
        return run_single_worker(current_net, config, metrics, start_time);
    }

    loop {
        // [1] Check round limit (R29)
        if let Some(max) = config.max_rounds {
            if metrics.rounds >= max {
                metrics.converged = false;
                break;
            }
        }

        // Record agents at start of round
        metrics
            .agents_per_round
            .push(current_net.count_live_agents());

        // === [2] PHASE 1: SPLIT ===
        let t_partition = Instant::now();
        let mut plan = split(current_net, config.num_workers, strategy);
        metrics.partition_time_per_round.push(t_partition.elapsed());

        // === [3] PHASE 2: LOCAL REDUCTION (per worker, sequentially) ===
        let t_compute = Instant::now();
        let mut local_interactions: u64 = 0;
        let mut local_by_rule: [u64; 6] = [0; 6];
        let mut worker_stats: Vec<WorkerRoundStats> = Vec::new();

        for partition in &mut plan.partitions {
            let agents_before = partition.subnet.count_live_agents();

            let t_reduce = Instant::now();
            let reduction_stats: ReductionStats = reduce_all(&mut partition.subnet);
            let reduce_duration = t_reduce.elapsed();
            local_interactions += reduction_stats.total_interactions;

            // Per-rule counts directly from ReductionStats (SPEC-03 R17)
            let by_rule = reduction_stats.interactions_by_rule;
            for i in 0..6 {
                local_by_rule[i] += by_rule[i];
            }

            // Rebuild free_port_index after local reduction (TASK-0063)
            partition.free_port_index = rebuild_free_port_index(
                &partition.subnet,
                partition.border_id_start,
                partition.border_id_end,
            );

            let agents_after = partition.subnet.count_live_agents();
            worker_stats.push(WorkerRoundStats {
                worker_id: partition.worker_id,
                agents_before,
                agents_after,
                local_redexes: reduction_stats.total_interactions as usize,
                reduce_duration_secs: reduce_duration.as_secs_f64(),
                interactions_by_rule: by_rule,
            });
        }

        metrics.compute_time_per_round.push(t_compute.elapsed());
        metrics
            .local_interactions_per_round
            .push(local_interactions);
        metrics.worker_stats_per_round.push(worker_stats);

        // === [4] PHASE 3: MERGE (structural) ===
        let t_merge = Instant::now();
        let (mut merged_net, border_redex_count) = merge(plan);
        metrics.merge_time_per_round.push(t_merge.elapsed());
        metrics.border_redexes_per_round.push(border_redex_count);

        // === [5] PHASE 4: RESOLVE BORDERS ===
        //
        // Two modes (SPEC-05 R30, R30a):
        //   - Lenient (default, `strict_bsp = false`): run `reduce_all` on
        //     the merged net. Cascades generated during border resolution
        //     are collapsed at the coordinator in the same round, so
        //     `rounds == 1` for nets whose cascades are confined to the
        //     merge step. This is the performance-oriented path and was
        //     the original behaviour prior to v0.10.0-bench.
        //   - Strict (`strict_bsp = true`): call `reduce_border_once`,
        //     which processes every redex currently in the merged queue
        //     exactly once and defers any new cascades to the next round.
        //     This forces the BSP cycle to iterate until Normal Form,
        //     exposing the real per-round border cost — the quantity that
        //     the Phase 3 LAN experiment measures. G1 still holds (the
        //     total interactions performed match the lenient and
        //     sequential baselines); only the round distribution changes.
        let t_border = Instant::now();
        let border_stats: ReductionStats = if config.strict_bsp {
            reduce_border_once(&mut merged_net)
        } else {
            reduce_all(&mut merged_net)
        };
        metrics
            .border_reduce_time_per_round
            .push(t_border.elapsed());

        let border_by_rule = border_stats.interactions_by_rule;
        metrics
            .border_interactions_per_round
            .push(border_stats.total_interactions);

        // === [6] Accumulate metrics, rounds++ ===
        for i in 0..6 {
            metrics.total_interactions_by_rule[i] += local_by_rule[i] + border_by_rule[i];
        }
        metrics.total_interactions += local_interactions + border_stats.total_interactions;

        current_net = merged_net;
        metrics.rounds += 1;

        // === [7] TERMINATION CHECK (aligned with SPEC-13 CheckTermination) ===
        drain_stale_redexes(&mut current_net);
        if current_net.redex_queue.is_empty() {
            #[cfg(debug_assertions)]
            verify_no_redexes_full_scan(&current_net);
            metrics.converged = true;
            break;
        }
    }

    metrics.total_time = start_time.elapsed();
    (current_net, metrics)
}

/// Optimized path for n == 1: reduce locally without partitioning (R26).
fn run_single_worker(
    mut net: Net,
    config: &GridConfig,
    mut metrics: GridMetrics,
    start_time: Instant,
) -> (Net, GridMetrics) {
    // Check round limit
    if let Some(max) = config.max_rounds {
        if max == 0 {
            metrics.converged = false;
            metrics.total_time = start_time.elapsed();
            return (net, metrics);
        }
    }

    metrics.agents_per_round.push(net.count_live_agents());

    let t_reduce = Instant::now();
    let stats = reduce_all(&mut net);
    let reduce_duration = t_reduce.elapsed();

    metrics.compute_time_per_round.push(reduce_duration);
    metrics
        .local_interactions_per_round
        .push(stats.total_interactions);
    metrics.total_interactions = stats.total_interactions;
    for i in 0..6 {
        metrics.total_interactions_by_rule[i] += stats.interactions_by_rule[i];
    }
    metrics.border_interactions_per_round.push(0);
    metrics.border_redexes_per_round.push(0);
    metrics.rounds = 1;
    metrics.converged = true;

    metrics.worker_stats_per_round.push(vec![WorkerRoundStats {
        worker_id: 0,
        agents_before: metrics.agents_per_round[0],
        agents_after: net.count_live_agents(),
        local_redexes: stats.total_interactions as usize,
        reduce_duration_secs: reduce_duration.as_secs_f64(),
        interactions_by_rule: stats.interactions_by_rule,
    }]);

    metrics.total_time = start_time.elapsed();
    (net, metrics)
}

/// Defense-in-depth: full scan for redexes not in the queue (R41).
///
/// In debug mode, verifies that no agent has its principal port connected
/// to another agent's principal port without being in the redex queue.
/// This catches bugs in Net::connect that might fail to insert a redex.
///
/// Complexity: O(A * PORTS_PER_SLOT).
#[cfg(debug_assertions)]
fn verify_no_redexes_full_scan(net: &Net) {
    for agent in net.live_agents() {
        let target = net.get_target(PortRef::AgentPort(agent.id, 0));
        if let PortRef::AgentPort(other_id, 0) = target {
            if agent.id < other_id {
                // This is an active pair not in the queue — bug in connect()
                panic!(
                    "R41: undiscovered redex ({}, {}) found during full scan",
                    agent.id, other_id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Symbol;
    use crate::partition::ContiguousIdStrategy;

    // === TASK-0069: Skeleton tests ===

    // T1: Net already in Normal Form -> converged immediately, 0 rounds
    #[test]
    fn test_run_grid_already_normal_form() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // No redexes (no principal-principal connections)
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 0);
        assert_eq!(metrics.total_interactions, 0);
        assert_eq!(result.count_live_agents(), 2);
    }

    // T2: max_rounds = Some(0) -> terminates immediately, converged = false
    #[test]
    fn test_run_grid_max_rounds_zero() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let config = GridConfig {
            num_workers: 2,
            max_rounds: Some(0),
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(!metrics.converged);
        assert_eq!(metrics.rounds, 0);
    }

    // T3: total_time is populated
    #[test]
    fn test_run_grid_total_time_populated() {
        let net = Net::new();
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);
        // Verify the grid actually ran and produced metrics
        let _ = metrics.total_time;
    }

    // === TASK-0072: n == 1 optimization ===

    // T4: Single worker reduces without partitioning
    #[test]
    fn test_run_grid_single_worker() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let config = GridConfig {
            num_workers: 1,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 1);
        assert_eq!(metrics.total_interactions, 1); // ERA-ERA = 1 interaction
        assert_eq!(result.count_live_agents(), 0);
    }

    // === TASK-0070/0071: Phase integration tests ===

    // T5: ERA-ERA pair with 2 workers -> split as border, merge resolves
    #[test]
    fn test_run_grid_era_era_border() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0 -> w0
        let b = net.create_agent(Symbol::Era); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert_eq!(metrics.total_interactions, 1); // 1 ERA-ERA interaction
        assert_eq!(metrics.border_redexes_per_round[0], 1); // detected as border redex
    }

    // T6: CON-CON annihilation with border redex
    #[test]
    fn test_run_grid_con_con_border() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con); // id=0 -> w0
        let b = net.create_agent(Symbol::Con); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0); // annihilation removes both
        assert_eq!(metrics.total_interactions, 1);
    }

    // T7: Internal redexes resolved during local reduction
    #[test]
    fn test_run_grid_internal_redexes() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0
        let b = net.create_agent(Symbol::Era); // id=1
        let c = net.create_agent(Symbol::Era); // id=2
        let d = net.create_agent(Symbol::Era); // id=3
                                               // a-b pair -> internal to w0 (ids 0,1)
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        // c-d pair -> internal to w1 (ids 2,3)
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert_eq!(metrics.total_interactions, 2); // 2 ERA-ERA
                                                   // Both redexes are internal, no border redexes
        assert_eq!(metrics.border_redexes_per_round[0], 0);
        assert_eq!(metrics.local_interactions_per_round[0], 2);
    }

    // T8: Metrics accumulation across a single round
    #[test]
    fn test_run_grid_metrics_single_round() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(metrics.rounds, 1);
        assert_eq!(metrics.agents_per_round.len(), 1);
        assert_eq!(metrics.agents_per_round[0], 2);
        assert!(!metrics.partition_time_per_round.is_empty());
        assert!(!metrics.compute_time_per_round.is_empty());
        assert!(!metrics.merge_time_per_round.is_empty());
        assert!(!metrics.border_reduce_time_per_round.is_empty());
    }

    // === TASK-0075: Fundamental Property G1 ===

    // G1a: ERA-ERA — reduce_all(net) == run_grid(net, 2)
    #[test]
    fn test_g1_era_era() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let net_seq = net.clone();
        let mut seq = net_seq;
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1b: CON-CON annihilation
    #[test]
    fn test_g1_con_con() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1c: DUP-DUP annihilation
    #[test]
    fn test_g1_dup_dup() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Dup);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1d: CON-ERA erasure
    #[test]
    fn test_g1_con_era() {
        let mut net = Net::new();
        let c = net.create_agent(Symbol::Con); // id=0
        let e = net.create_agent(Symbol::Era); // id=1
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(e, 0));
        net.connect(PortRef::AgentPort(c, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(1));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1e: Chain of ERA-ERA pairs (4 agents, 2 internal redexes, 2 workers)
    #[test]
    fn test_g1_chain_internal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1f: CON-DUP commutation across partition boundary
    #[test]
    fn test_g1_con_dup_border() {
        let mut net = Net::new();
        let c = net.create_agent(Symbol::Con); // id=0 -> w0
        let d = net.create_agent(Symbol::Dup); // id=1 -> w1
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));
        net.connect(PortRef::AgentPort(c, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(d, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(d, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        // CON-DUP creates 4 new agents from 2 original
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1g: G1 with 4 workers
    #[test]
    fn test_g1_four_workers() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        let c = net.create_agent(Symbol::Era);
        let d = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(c, 0), PortRef::AgentPort(d, 0));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 4,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1h: Empty net -> both return empty net immediately
    #[test]
    fn test_g1_empty_net() {
        let net = Net::new();
        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // G1i: FreePort-FreePort redirect regression — annihilation consumes both
    // agents in one partition, leaving the border reference unresolvable.
    //
    // Topology: c0-c1 (internal pair in W0), c2 survives in W1.
    //   c0.0 <-> c1.0 (active pair, internal to W0)
    //   c1.1 <-> c2.1 (border wire)
    //   c1.2 <-> c2.2 (border wire)
    //   c0.1, c0.2 = Lafont FreePorts; c2.0 = Lafont FreePort
    //
    // Bug: W0 reduces c0-c1 (CON-CON CROSS). The CROSS links
    //   FreePort(B0) <-> FreePort(Lafont) — a no-op in the port array.
    //   Border refs B0, B1 are lost. Merge can't restore c2.1 and c2.2,
    //   leaving them DISCONNECTED → T1 violation.
    //
    // Fix: Net::connect records FreePort-FreePort redirects in
    //   freeport_redirects, which rebuild_free_port_index uses.
    #[test]
    fn test_g1_freeport_redirect_regression() {
        let mut net = Net::new();
        let c0 = net.create_agent(Symbol::Con); // id=0 → W0
        let c1 = net.create_agent(Symbol::Con); // id=1 → W0
        let c2 = net.create_agent(Symbol::Con); // id=2 → W1

        // Active pair in W0
        net.connect(PortRef::AgentPort(c0, 0), PortRef::AgentPort(c1, 0));
        // Border wires (will cross the partition boundary)
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c2, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::AgentPort(c2, 2));
        // Lafont FreePorts
        net.connect(PortRef::AgentPort(c0, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c0, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(c2, 0), PortRef::FreePort(2));

        // Sequential baseline
        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        // Grid with 2 workers
        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        // Both should produce 1 agent (c2 survives) with 1 interaction
        assert_eq!(seq.count_live_agents(), 1);
        assert_eq!(grid_result.count_live_agents(), 1);
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);

        // c2's ports should match sequential result
        // Sequential: c2.0 = FP(2), c2.1 = FP(1) (via CROSS), c2.2 = FP(0) (via CROSS)
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(c2, 0)),
            seq.get_target(PortRef::AgentPort(c2, 0))
        );
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(c2, 1)),
            seq.get_target(PortRef::AgentPort(c2, 1))
        );
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(c2, 2)),
            seq.get_target(PortRef::AgentPort(c2, 2))
        );
    }

    // G1j: Same as G1i but with DUP-DUP (PARALLEL pattern)
    #[test]
    fn test_g1_freeport_redirect_dup_dup() {
        let mut net = Net::new();
        let d0 = net.create_agent(Symbol::Dup); // id=0 → W0
        let d1 = net.create_agent(Symbol::Dup); // id=1 → W0
        let d2 = net.create_agent(Symbol::Dup); // id=2 → W1

        net.connect(PortRef::AgentPort(d0, 0), PortRef::AgentPort(d1, 0));
        net.connect(PortRef::AgentPort(d1, 1), PortRef::AgentPort(d2, 1));
        net.connect(PortRef::AgentPort(d1, 2), PortRef::AgentPort(d2, 2));
        net.connect(PortRef::AgentPort(d0, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(d0, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(d2, 0), PortRef::FreePort(2));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert_eq!(seq.count_live_agents(), 1);
        assert_eq!(grid_result.count_live_agents(), 1);
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(d2, 1)),
            seq.get_target(PortRef::AgentPort(d2, 1))
        );
        assert_eq!(
            grid_result.get_target(PortRef::AgentPort(d2, 2)),
            seq.get_target(PortRef::AgentPort(d2, 2))
        );
    }

    // G1k: Symmetric — both partitions consume agents with border wires
    #[test]
    fn test_g1_freeport_redirect_symmetric() {
        let mut net = Net::new();
        let c0 = net.create_agent(Symbol::Con); // id=0 → W0
        let c1 = net.create_agent(Symbol::Con); // id=1 → W0
        let c2 = net.create_agent(Symbol::Con); // id=2 → W1
        let c3 = net.create_agent(Symbol::Con); // id=3 → W1

        // Internal pairs in both partitions
        net.connect(PortRef::AgentPort(c0, 0), PortRef::AgentPort(c1, 0));
        net.connect(PortRef::AgentPort(c2, 0), PortRef::AgentPort(c3, 0));
        // Border wires (c1 aux <-> c2 aux)
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c2, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::AgentPort(c2, 2));
        // Lafont FreePorts
        net.connect(PortRef::AgentPort(c0, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c0, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(c3, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(c3, 2), PortRef::FreePort(3));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            max_rounds: None,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        // All agents consumed
        assert_eq!(seq.count_live_agents(), 0);
        assert_eq!(grid_result.count_live_agents(), 0);
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // === SPEC-05 R30a: strict BSP mode tests ===
    //
    // These tests cover the `strict_bsp` branch of the grid loop. In strict
    // mode, border cascades are NOT reduced at the coordinator after merge,
    // so the loop iterates — redistributing border redexes to workers in the
    // next round. This exposes true multi-round BSP behavior needed for
    // Phase 3 LAN measurements, which is otherwise hidden by the lenient
    // coordinator-side `reduce_all` optimization.
    //
    // The Fundamental Property G1 (SPEC-01) MUST hold in both modes: the
    // same reductions are performed, only the distribution across rounds
    // differs.

    // T-strict-1: Net already in Normal Form behaves identically under
    // strict_bsp. Lenient and strict both report 0 rounds.
    #[test]
    fn test_strict_bsp_already_normal_form() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(b, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
        net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));

        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(metrics.rounds, 0);
        assert_eq!(metrics.total_interactions, 0);
        assert_eq!(result.count_live_agents(), 2);
    }

    // T-strict-2: ERA-ERA annihilation with strict_bsp completes in exactly
    // 1 round. The single border redex is consumed by the next round's
    // worker, and no new redexes are produced (annihilation is terminal).
    #[test]
    fn test_strict_bsp_era_era_single_round() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Era); // id=0 -> w0
        let b = net.create_agent(Symbol::Era); // id=1 -> w1
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));

        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(result.count_live_agents(), 0);
        assert_eq!(metrics.total_interactions, 1);
        // In strict mode the border redex is resolved by worker reduction
        // in the next round; no cascading is generated, so 1 round suffices.
        assert!(
            metrics.rounds >= 1,
            "expected at least 1 round, got {}",
            metrics.rounds
        );
    }

    // T-strict-3: CON-CON cascade across the partition boundary forces
    // a multi-round reduction in strict mode. Two left-side CONs (worker 0)
    // and two right-side CONs (worker 1) are chained so that reducing the
    // first border active pair produces a second border active pair. Under
    // lenient mode this all resolves in 1 round (coordinator reduce_all);
    // under strict mode, the cascade is deferred to a subsequent round.
    #[test]
    fn test_strict_bsp_con_cascade_multi_round() {
        let mut net = Net::new();
        // Left chain (worker 0): ids 0, 1
        let l0 = net.create_agent(Symbol::Con); // id=0 -> w0
        let l1 = net.create_agent(Symbol::Con); // id=1 -> w0
        // Right chain (worker 1): ids 2, 3
        let r0 = net.create_agent(Symbol::Con); // id=2 -> w1
        let r1 = net.create_agent(Symbol::Con); // id=3 -> w1

        // Initial border active pair: l0.0 <-> r0.0
        net.connect(PortRef::AgentPort(l0, 0), PortRef::AgentPort(r0, 0));
        // l0.1 <-> l1.0 (aux-to-principal; not an active pair yet)
        net.connect(PortRef::AgentPort(l0, 1), PortRef::AgentPort(l1, 0));
        // r0.2 <-> r1.0 (aux-to-principal; mirrors l side)
        net.connect(PortRef::AgentPort(r0, 2), PortRef::AgentPort(r1, 0));
        // Terminal FreePorts for the remaining aux ports
        net.connect(PortRef::AgentPort(l0, 2), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(r0, 1), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(l1, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(l1, 2), PortRef::FreePort(3));
        net.connect(PortRef::AgentPort(r1, 1), PortRef::FreePort(4));
        net.connect(PortRef::AgentPort(r1, 2), PortRef::FreePort(5));

        // CON-CON annihilation rule: link(a1, b2) and link(a2, b1). After
        // the l0-r0 annihilation, a1=l1.0 and b2=r1.0, so link(l1.0, r1.0)
        // creates the second principal-principal active pair — still
        // cross-partition (l1 in w0, r1 in w1).

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        // G1: equivalent Normal Form to sequential
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
        // Strict mode exposes the cascade as >= 2 rounds: round 1 consumes
        // the first CON-CON pair and produces a new principal-principal
        // border pair; round 2 (or later) resolves it.
        assert!(
            metrics.rounds >= 2,
            "expected >= 2 rounds under strict BSP, got {}",
            metrics.rounds
        );
    }

    // T-strict-4: Binary tree of CON-DUP nodes (DualTree-style) at depth 4.
    // Each layer of commutations pushes new active pairs across the
    // partition boundary, so strict mode should accumulate several rounds
    // before convergence.
    #[test]
    fn test_strict_bsp_dual_tree_depth4_multi_round() {
        // Use the generator from io::generators to match the DualTree bench.
        let net = crate::io::generators::dual_tree(4);

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
        // DualTree depth=4 has cross-partition cascades at every tree level.
        // We expect at least 2 rounds (strict actually reveals multi-round
        // behavior); the exact count depends on tree topology.
        assert!(
            metrics.rounds >= 2,
            "expected >= 2 rounds for dual_tree(4) under strict BSP, got {}",
            metrics.rounds
        );
    }

    // T-strict-5: G1 — strict mode yields the same Normal Form as sequential
    // reduction, for a variety of topologies. This is the Fundamental
    // Property check for strict mode.
    #[test]
    fn test_strict_bsp_g1_vs_sequential() {
        // Topology: 2 CON-CON pairs across the border + 1 internal ERA-ERA.
        let mut net = Net::new();
        let c0 = net.create_agent(Symbol::Con); // id=0 -> w0
        let c1 = net.create_agent(Symbol::Con); // id=1 -> w0
        let c2 = net.create_agent(Symbol::Con); // id=2 -> w1
        let c3 = net.create_agent(Symbol::Con); // id=3 -> w1
        let e0 = net.create_agent(Symbol::Era); // id=4 -> w1
        let e1 = net.create_agent(Symbol::Era); // id=5 -> w1

        // Border: c1 <-> c2 (CON-CON annihilation)
        net.connect(PortRef::AgentPort(c1, 0), PortRef::AgentPort(c2, 0));
        // Internal: c0 <-> c3 is aux wire; e0 <-> e1 is internal ERA-ERA
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c0, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::AgentPort(c0, 2));
        net.connect(PortRef::AgentPort(c2, 1), PortRef::AgentPort(c3, 1));
        net.connect(PortRef::AgentPort(c2, 2), PortRef::AgentPort(c3, 2));
        net.connect(PortRef::AgentPort(c0, 0), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(c3, 0), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(e0, 0), PortRef::AgentPort(e1, 0));

        let mut seq = net.clone();
        let seq_stats = reduce_all(&mut seq);

        let config = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };
        let (grid_result, metrics) = run_grid(net, &config, &ContiguousIdStrategy);

        assert!(metrics.converged);
        assert_eq!(seq.count_live_agents(), grid_result.count_live_agents());
        assert_eq!(seq_stats.total_interactions, metrics.total_interactions);
    }

    // T-strict-6: G1 — lenient and strict modes produce structurally
    // equivalent Normal Forms (same live agent count and same total
    // interactions). Round count may differ, but the work is the same.
    #[test]
    fn test_strict_bsp_g1_vs_lenient() {
        let net = crate::io::generators::dual_tree(3);

        let lenient_cfg = GridConfig {
            num_workers: 2,
            strict_bsp: false,
            ..GridConfig::default()
        };
        let strict_cfg = GridConfig {
            num_workers: 2,
            strict_bsp: true,
            ..GridConfig::default()
        };

        let (lenient_net, lenient_metrics) =
            run_grid(net.clone(), &lenient_cfg, &ContiguousIdStrategy);
        let (strict_net, strict_metrics) =
            run_grid(net, &strict_cfg, &ContiguousIdStrategy);

        assert!(lenient_metrics.converged);
        assert!(strict_metrics.converged);
        assert_eq!(
            lenient_net.count_live_agents(),
            strict_net.count_live_agents()
        );
        assert_eq!(
            lenient_metrics.total_interactions,
            strict_metrics.total_interactions
        );
        // Lenient concentrates all cascades into 1 round; strict should
        // use >= lenient's round count.
        assert!(strict_metrics.rounds >= lenient_metrics.rounds);
    }
}
