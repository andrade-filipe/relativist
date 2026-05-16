//! Command entry points for each CLI subcommand.
//!
//! Each function takes the parsed Args struct and returns
//! Result<(), RelativistError>. Wired from main.rs.

use crate::config::{
    build_grid_config, build_grid_config_from_local, build_node_config_coordinator,
    build_node_config_worker, parse_strategy, BenchArgs, CompletionsArgs, CoordinatorArgs,
    EncodersAction, EncodersArgs, GenerateArgs, InspectArgs, LocalArgs, ReduceArgs, UpdateArgs,
    ValidateArgs, WorkerArgs,
};
use crate::error::RelativistError;
use crate::io::{load_net_from_file, print_summary, save_net_to_file, write_metrics};
use crate::merge::run_grid;
use crate::protocol::config::{TransportBackend, TransportConfig};
use crate::reduction::reduce_all;

/// Log an advisory when TCP is used on a loopback address (SPEC-17 R33).
///
/// Suggests `--transport=unix` for same-host deployments where Unix domain
/// sockets would avoid TCP overhead. MUST NOT auto-switch (R34).
fn log_loopback_advisory(bind: std::net::SocketAddr, config: &TransportConfig) {
    if config.backend == TransportBackend::Tcp && bind.ip().is_loopback() {
        tracing::info!(
            addr = %bind,
            "Same-host detected (loopback address with TCP backend). \
             Consider --transport=unix for lower overhead on same-host deployments."
        );
    }
}

/// Execute local mode: grid loop in-process, no TCP (SPEC-07 R18).
///
/// SPEC-19 R20 (TASK-0396): routes through `run_grid_entry` to observe
/// `grid_config.delta_mode`. When the flag is true on this CLI path we
/// cleanly refuse: the delta BSP loop requires a `WorkerDispatch`, and
/// no in-process dispatch has been wired to the local CLI yet (DC-C2
/// defers the async binding to `protocol/coordinator.rs`; TASK-0395
/// will land a test-support `LocalDeltaDispatch` usable from integration
/// tests but not from production CLI). When the flag is false, the
/// router delegates to the v1 full-partition path unchanged, preserving
/// SPEC-19 R42 byte-for-byte.
pub fn run_local_command(args: LocalArgs) -> Result<(), RelativistError> {
    let grid_config = build_grid_config_from_local(&args);
    let strategy = parse_strategy(&args.strategy)?;

    if grid_config.delta_mode {
        return Err(RelativistError::Config(
            "--delta-mode on `local` requires a coordinator runtime that is not yet \
             wired on the in-process CLI path (SPEC-19 R20, TASK-0396 placeholder). \
             Omit --delta-mode to use the v1 full-partition path, or run under \
             `coordinator` mode once TCP-backed WorkerDispatch ships."
                .into(),
        ));
    }

    let net = load_net_from_file(&args.input)?;
    let (mut reduced_net, metrics) =
        crate::merge::run_grid_entry(net, &grid_config, &*strategy, None);

    crate::encoding::discover_root(&mut reduced_net);
    print_summary(&reduced_net, &metrics);

    if let Some(ref path) = args.output {
        save_net_to_file(&reduced_net, path)?;
    }
    if let Some(ref path) = args.metrics {
        write_metrics(&metrics, path)?;
    }

    Ok(())
}

/// Execute reduce mode: purely local reduction, no partitioning (SPEC-13 R41).
pub fn run_reduce_command(args: ReduceArgs) -> Result<(), RelativistError> {
    let mut net = load_net_from_file(&args.input)?;
    let stats = reduce_all(&mut net);

    println!("=== Relativist Reduce Summary ===");
    println!("Interactions: {}", stats.total_interactions);
    println!("Final agents: {}", net.count_live_agents());

    if let Some(ref path) = args.output {
        save_net_to_file(&net, path)?;
    }

    Ok(())
}

/// Execute inspect mode: print summary statistics (SPEC-13 R47).
pub fn run_inspect_command(args: InspectArgs) -> Result<(), RelativistError> {
    let net = load_net_from_file(&args.input)?;

    println!("=== Relativist Inspect ===");
    println!("Agents:  {}", net.count_live_agents());
    println!(
        "  CON: {}",
        crate::io::count_agents_by_symbol(&net, crate::net::Symbol::Con)
    );
    println!(
        "  DUP: {}",
        crate::io::count_agents_by_symbol(&net, crate::net::Symbol::Dup)
    );
    println!(
        "  ERA: {}",
        crate::io::count_agents_by_symbol(&net, crate::net::Symbol::Era)
    );
    println!("Redexes: {}", net.redex_queue.len());
    println!(
        "Normal Form: {}",
        if net.redex_queue.is_empty() {
            "yes"
        } else {
            "no"
        }
    );

    Ok(())
}

/// Execute generate mode: create a workload network (SPEC-07 R8, SPEC-12 R35-R42a).
pub fn run_generate_command(args: GenerateArgs) -> Result<(), RelativistError> {
    use crate::io::generators;

    let net = generators::generate(args.example, args.size);

    println!("=== Relativist Generate ===");
    println!("Example: {:?}", args.example);
    println!("Size:    {}", args.size);
    println!("Agents:  {}", net.count_live_agents());
    println!("Redexes: {}", net.redex_queue.len());

    save_net_to_file(&net, &args.output)?;
    println!("Saved to: {}", args.output.display());

    Ok(())
}

/// Execute coordinator mode: distributed grid loop (SPEC-07 R13).
///
/// Creates a tokio runtime and runs the async coordinator protocol
/// (accept workers, distribute partitions, collect results, merge).
pub fn run_coordinator_command(args: CoordinatorArgs) -> Result<(), RelativistError> {
    use crate::protocol::coordinator::run_coordinator;
    use crate::protocol::transport::create_transport;
    use crate::security::{build_security_config, check_bind_warnings, write_token_file};

    let grid_config = build_grid_config(&args);
    let node_config = build_node_config_coordinator(&args)?;
    let strategy = parse_strategy(&args.strategy)?;
    let net = load_net_from_file(&args.input)?;

    // Build security config from --token flag
    #[cfg(feature = "tls")]
    let has_tls = args.tls_cert.is_some();
    #[cfg(not(feature = "tls"))]
    let has_tls = false;
    let security = build_security_config(args.token.as_deref(), has_tls)
        .map_err(|e| RelativistError::Config(format!("security: {}", e)))?;

    check_bind_warnings(&node_config.bind, security.token.is_some());

    // Write token file if a token was generated/provided
    if let Some(ref token) = security.token {
        write_token_file(token, &args.token_file)
            .map_err(|e| RelativistError::Config(format!("token file: {}", e)))?;
    }

    println!("=== Relativist Coordinator ===");
    println!("Workers:  {}", grid_config.num_workers);
    println!("Bind:     {}", node_config.bind);
    println!("Input:    {}", args.input.display());
    println!("Agents:   {}", net.count_live_agents());
    println!("Redexes:  {}", net.redex_queue.len());
    println!("Security: {:?}", security.tier);

    // Print copiable worker connect command
    if let Some(ref token) = security.token {
        println!();
        println!("Workers connect with:");
        println!(
            "  relativist worker --coordinator {}:{} --token \"{}\"",
            node_config.bind.ip(),
            node_config.bind.port(),
            token.to_base64()
        );
    }
    println!();

    // Run the async coordinator
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| RelativistError::Config(format!("tokio runtime: {}", e)))?;

    let mut transport = create_transport(node_config.bind, &node_config.transport)
        .map_err(crate::error::CoordinatorError::from)
        .map_err(RelativistError::from)?;

    // R33: Advisory when using TCP on loopback — suggest --transport=unix.
    // MUST NOT auto-switch (R34).
    log_loopback_advisory(node_config.bind, &node_config.transport);

    let (mut reduced_net, metrics) = rt
        .block_on(run_coordinator(
            net,
            &node_config,
            &grid_config,
            &*strategy,
            security.token.as_ref(),
            &mut *transport,
        ))
        .map_err(crate::error::CoordinatorError::from)
        .map_err(RelativistError::from)?;

    crate::encoding::discover_root(&mut reduced_net);
    print_summary(&reduced_net, &metrics);

    if let Some(ref path) = args.output {
        save_net_to_file(&reduced_net, path)?;
    }
    if let Some(ref path) = args.metrics {
        write_metrics(&metrics, path)?;
    }

    Ok(())
}

/// Execute worker mode: connect to coordinator, reduce partitions (SPEC-07 R16, SPEC-16).
///
/// Creates a tokio runtime and runs the async worker protocol.
/// In daemon mode (SPEC-16), the worker reconnects after each job.
pub fn run_worker_command(args: WorkerArgs) -> Result<(), RelativistError> {
    use crate::protocol::transport::create_transport;
    use crate::protocol::worker::{run_worker, run_worker_daemon};
    use crate::security::build_security_config;

    let node_config = build_node_config_worker(&args)?;

    // Build security config from --token flag
    #[cfg(feature = "tls")]
    let has_tls = args.tls_ca.is_some();
    #[cfg(not(feature = "tls"))]
    let has_tls = false;
    let security = build_security_config(args.token.as_deref(), has_tls)
        .map_err(|e| RelativistError::Config(format!("security: {}", e)))?;

    println!("=== Relativist Worker ===");
    println!("Coordinator: {}", args.coordinator);
    println!("Security:    {:?}", security.tier);
    if args.daemon {
        println!("Mode:        daemon (reconnecting)");
    }
    println!();

    // Run the async worker
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| RelativistError::Config(format!("tokio runtime: {}", e)))?;

    let mut transport = create_transport(node_config.bind, &node_config.transport)
        .map_err(crate::error::WorkerError::from)
        .map_err(RelativistError::from)?;

    // R33: Advisory when using TCP on loopback — suggest --transport=unix.
    log_loopback_advisory(node_config.bind, &node_config.transport);

    if args.daemon {
        rt.block_on(run_worker_daemon(
            &node_config,
            security.token.as_ref(),
            &mut *transport,
        ))
        .map_err(crate::error::WorkerError::from)
        .map_err(RelativistError::from)?;
    } else {
        rt.block_on(run_worker(
            &node_config,
            security.token.as_ref(),
            &mut *transport,
        ))
        .map_err(crate::error::WorkerError::from)
        .map_err(RelativistError::from)?;
        println!("Worker finished successfully.");
    }

    Ok(())
}

/// Execute bench mode: run the benchmark suite (SPEC-09 R1, R6).
pub fn run_bench_command(args: BenchArgs) -> Result<(), RelativistError> {
    use crate::bench::csv::{
        write_csv_detail, write_csv_rounds, write_csv_sparse_construction, write_csv_summary,
    };
    use crate::bench::suite::run_benchmark_suite;
    use crate::bench::{BenchmarkId, BenchmarkSuiteConfig, Mode};

    // -----------------------------------------------------------------------
    // D-014 Stress-curve dispatch (TASK-0702)
    // -----------------------------------------------------------------------
    // When `--campaign stress-curve` is set, route to the descriptor
    // path; the existing matrix-runner code below is untouched.
    if let Some(crate::config::CampaignKind::StressCurve) = args.campaign {
        let workload = args
            .workload
            .ok_or_else(|| {
                RelativistError::Config("--campaign stress-curve requires --workload".to_string())
            })?
            .into_workload();
        let env = args
            .env
            .ok_or_else(|| {
                RelativistError::Config("--campaign stress-curve requires --env".to_string())
            })?
            .into_env();
        // Reuse the existing `--workers Vec<u32>` flag's first value.
        let workers = args.workers.first().copied().unwrap_or(1) as usize;
        let reps = args.reps.max(1) as usize;
        let n_seq_owned = args.n_seq.clone();

        // TASK-0720 BUG-003 (Fix-B): when `reps > 1`, the in-process
        // `run_one_sequence` does NOT reset `MemoryProbe::peak_bytes` (VmHWM)
        // between reps. Reps 2..N inherit rep 1's high-water-mark. The bash
        // orchestrator works around this by fork-execing a fresh child per
        // rep; in-process Rust callers see a monotonic peak. Document the
        // limitation loudly so a Rust user does not silently consume bad
        // numbers from rep 2 onward. See `docs/benchmarks/campaigns/stress-curve.md`
        // ("Known limitations" section).
        if reps > 1 {
            tracing::warn!(
                reps = reps,
                "VmHWM not reset between reps in the in-process Rust path; \
                 rep 1's peak is inherited by reps 2..N (bash orchestrator \
                 fork-execs a fresh child per rep to bypass this). See \
                 docs/benchmarks/campaigns/stress-curve.md."
            );
        }

        let outcome = crate::bench::suite::StressCurveDescriptor::run_one_sequence(
            workload,
            env,
            workers,
            reps,
            n_seq_owned.as_deref(),
            None,
        )
        .map_err(|e| RelativistError::Config(format!("stress-curve campaign: {}", e)))?;

        // TASK-0722 BUG-B fix: the `--campaign stress-curve` dispatch path
        // emits the REAL per-rep `BenchmarkResult` rows produced by the
        // underlying `run_benchmark_suite` invocation, via the canonical
        // `write_csv_detail` writer. TASK-0720's interim implementation
        // (Stage 6 REFACTOR) synthesised rows with hardcoded zeros for
        // every counter (interactions, MIPS, rounds, agent counts, bytes,
        // per-rule breakdowns), invalidating every sanity check downstream.
        //
        // Telemetry plumbing: `RepResult.bench_results` (BUG-B step 1)
        // carries the suite output up out of `run_one_sequence` (BUG-B
        // step 2); we annotate the LAST emitted row of the LAST rep with
        // the StopRule's `stop_reason` (when any) so the plot script and
        // the bash orchestrator can recover the campaign's halting cause.
        //
        // The closing summary line goes to `tracing::info!` (stderr by
        // default under the project's tracing-subscriber config) — the
        // operator can still see it on the console, but it does NOT
        // contaminate stdout.
        let mut rows: Vec<crate::bench::BenchmarkResult> = Vec::new();
        for rep in outcome.completed_reps.iter() {
            for bench_result in rep.bench_results.iter() {
                rows.push(bench_result.clone());
            }
        }

        // Annotate the final row of the last rep with the sequence-level
        // stop_reason (if any). This row is the right anchor because:
        //   (a) `outcome.last_attempted_n` corresponds to the rep that
        //       triggered the stop (per `StopRule::run_sequence` contract);
        //   (b) all rows for that rep share the same N — taking the last
        //       gives the highest `repetition` index inside that N.
        if let (Some(last_n), Some(reason)) = (outcome.last_attempted_n, outcome.stop_reason) {
            let reason_str = match reason {
                crate::bench::stop_rule::StopReason::WallTimeExceeded => "WallTimeExceeded",
                crate::bench::stop_rule::StopReason::MemoryExceeded => "MemoryExceeded",
                crate::bench::stop_rule::StopReason::Oom => "Oom",
            };
            // Walk in reverse to find the last row for `last_n`.
            for row in rows.iter_mut().rev() {
                if row.input_size as usize == last_n {
                    row.stop_reason = Some(reason_str.to_string());
                    break;
                }
            }
        }

        // Emit the CSV (header + rows) on stdout — this is what the bash
        // orchestrator captures via `>>"$RAW_CSV"`.
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        write_csv_detail(&mut handle, &rows)?;

        tracing::info!(
            completed_reps = outcome.completed_reps.len(),
            ?outcome.stop_reason,
            ?outcome.last_attempted_n,
            "stress-curve outcome"
        );
        return Ok(());
    }

    // Parse mode
    let mode = match args.mode.as_str() {
        "sequential" => Mode::Sequential,
        "local" => Mode::Local,
        "tcp-localhost" | "tcp_localhost" => Mode::TcpLocalhost,
        "tcp-network" | "tcp_network" => Mode::TcpNetwork,
        other => {
            return Err(RelativistError::Config(format!(
                "unknown mode '{}' (supported: sequential, local, tcp-localhost, tcp-network)",
                other
            )));
        }
    };

    // Parse benchmark IDs
    let benchmarks = if let Some(ref names) = args.benchmark {
        let mut ids = Vec::new();
        for name in names {
            let id = match name.as_str() {
                "ep_annihilation" => BenchmarkId::EPAnnihilation,
                "ep_annihilation_con" => BenchmarkId::EPAnnihilationCon,
                "ep_annihilation_dup" => BenchmarkId::EPAnnihilationDup,
                "condup_expansion" => BenchmarkId::ConDupExpansion,
                "dual_tree" => BenchmarkId::DualTree,
                "tree_sum" => BenchmarkId::TreeSum,
                "tree_sum_balanced" => BenchmarkId::TreeSumBalanced,
                "mixed_net" => BenchmarkId::MixedNet,
                "erasure_propagation" => BenchmarkId::ErasurePropagation,
                "church_add" => BenchmarkId::ChurchAdd,
                "church_mul" => BenchmarkId::ChurchMul,
                "cascade_cross" => BenchmarkId::CascadeCross,
                "church_sum_of_squares" => BenchmarkId::ChurchSumOfSquares,
                "all" => {
                    ids = vec![
                        BenchmarkId::EPAnnihilation,
                        BenchmarkId::EPAnnihilationCon,
                        BenchmarkId::EPAnnihilationDup,
                        BenchmarkId::ConDupExpansion,
                        BenchmarkId::DualTree,
                        BenchmarkId::TreeSum,
                        BenchmarkId::TreeSumBalanced,
                        BenchmarkId::MixedNet,
                        BenchmarkId::ErasurePropagation,
                        BenchmarkId::ChurchAdd,
                        BenchmarkId::ChurchMul,
                        BenchmarkId::CascadeCross,
                        BenchmarkId::ChurchSumOfSquares,
                    ];
                    break;
                }
                other => {
                    return Err(RelativistError::Config(format!(
                        "unknown benchmark '{}'",
                        other
                    )));
                }
            };
            ids.push(id);
        }
        ids
    } else {
        // Default: all benchmarks
        vec![
            BenchmarkId::EPAnnihilation,
            BenchmarkId::EPAnnihilationCon,
            BenchmarkId::EPAnnihilationDup,
            BenchmarkId::ConDupExpansion,
            BenchmarkId::DualTree,
            BenchmarkId::TreeSum,
            BenchmarkId::TreeSumBalanced,
            BenchmarkId::MixedNet,
            BenchmarkId::ErasurePropagation,
            BenchmarkId::ChurchAdd,
            BenchmarkId::ChurchMul,
            BenchmarkId::CascadeCross,
            BenchmarkId::ChurchSumOfSquares,
        ]
    };

    let config = BenchmarkSuiteConfig {
        benchmarks,
        sizes: args.sizes,
        workers: args.workers,
        mode,
        warmup_runs: args.warmup,
        repetitions: args.repetitions,
        csv_detail_path: args.csv_detail.as_ref().map(|p| p.display().to_string()),
        csv_rounds_path: args.csv_rounds.as_ref().map(|p| p.display().to_string()),
        csv_summary_path: args.csv_summary.as_ref().map(|p| p.display().to_string()),
        max_rounds: args.max_rounds,
        strict_bsp: args.strict_bsp,
        skip_g1: args.skip_g1,
        // Tier 3 fields wired from BenchArgs CLI flags (TASK-0603).
        // Defaults match the spec-mandated eager-path status quo when
        // flags are omitted, preserving backward compatibility for
        // existing bench scripts.
        chunk_size: args.chunk_size,
        max_pending_lifetime: args.max_pending_lifetime,
        recycle_policy: args.recycle_policy,
        representation: args.representation,
        // TASK-0607: optional sub-CSV for sparse-construction-memory rows.
        sparse_construction_memory_csv_path: args
            .csv_sparse
            .as_ref()
            .map(|p| p.display().to_string()),
    };

    println!("=== Relativist Benchmark Suite ===");
    println!(
        "Benchmarks:  {}",
        config
            .benchmarks
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("Mode:        {}", config.mode);
    println!("Workers:     {:?}", config.workers);
    println!("Warmup:      {}", config.warmup_runs);
    println!("Repetitions: {}", config.repetitions);
    println!();

    let suite_result = run_benchmark_suite(&config)
        .map_err(|e| RelativistError::Config(format!("Benchmark suite failed: {}", e)))?;

    // Print summary table (R41)
    println!("=== Results ===");
    println!(
        "{:<25} {:>6} {:>7} {:>10} {:>8} {:>8} {:>10}",
        "Benchmark", "Size", "Workers", "Time(s)", "MIPS", "Speedup", "Efficiency"
    );
    println!("{}", "-".repeat(80));
    for s in &suite_result.summaries {
        println!(
            "{:<25} {:>6} {:>7} {:>10.6} {:>8.1} {:>8.4} {:>10.4}",
            s.benchmark,
            s.input_size,
            s.workers,
            s.wall_clock_median,
            s.mips_mean,
            s.speedup_mean,
            s.efficiency_mean,
        );
        if s.cv > 0.10 {
            println!(
                "  WARNING: high variance (CV={:.2}%) for {} size={} workers={}",
                s.cv * 100.0,
                s.benchmark,
                s.input_size,
                s.workers
            );
        }
    }
    println!();
    println!(
        "Total datapoints: {}  |  All correct: {}",
        suite_result.results.len(),
        suite_result.all_correct
    );

    // CSV output (R39-R40)
    if let Some(ref path) = args.csv_detail {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_detail(&mut f, &suite_result.results)
            .map_err(|e| RelativistError::Config(format!("CSV detail write error: {}", e)))?;
        println!("Detail CSV written to: {}", path.display());
    }

    if let Some(ref path) = args.csv_rounds {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_rounds(&mut f, &suite_result.results)
            .map_err(|e| RelativistError::Config(format!("CSV rounds write error: {}", e)))?;
        println!("Rounds CSV written to: {}", path.display());
    }

    if let Some(ref path) = args.csv_summary {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_summary(&mut f, &suite_result.summaries)
            .map_err(|e| RelativistError::Config(format!("CSV summary write error: {}", e)))?;
        println!("Summary CSV written to: {}", path.display());
    }

    // TASK-0607: sparse-construction-memory sub-CSV (SPEC-09 §3.4.5).
    if let Some(ref path) = args.csv_sparse {
        let mut f = std::fs::File::create(path).map_err(|e| {
            RelativistError::Config(format!("cannot create {}: {}", path.display(), e))
        })?;
        write_csv_sparse_construction(&mut f, &suite_result.sparse_construction_rows).map_err(
            |e| RelativistError::Config(format!("CSV sparse-construction write error: {}", e)),
        )?;
        println!(
            "Sparse construction CSV written to: {} ({} row(s))",
            path.display(),
            suite_result.sparse_construction_rows.len()
        );
    }

    if !suite_result.all_correct {
        return Err(RelativistError::Config(
            "One or more benchmarks had correctness failures!".into(),
        ));
    }

    Ok(())
}

/// Execute compute mode: encode arithmetic, reduce, decode result
/// (SPEC-14 R22-R25; SPEC-27 v3 R21, R23).
///
/// Two paths:
/// - **Registry (SPEC-27 v3 R21, R23):** when `--encoder` OR `--codec` is
///   set (mutually exclusive at the clap layer via `conflicts_with`),
///   runs the `encode → validate → reduce_all → decode → print JSON`
///   pipeline. The two flags are coalesced via
///   `args.encoder.or(args.codec)`.
/// - **Legacy (SPEC-14):** positional `<op> <a> <b>` Church arithmetic.
pub fn run_compute_command(args: crate::config::ComputeArgs) -> Result<(), RelativistError> {
    use crate::config::ArithmeticOp;
    use crate::encoding::{build_add, build_exp, build_mul, decode_nat, discover_root};

    // SPEC-27 v3 R21, R23: registry path. `--encoder` and `--codec` are
    // mutually exclusive at the clap layer; coalesce here into one name.
    let codec_name = args.encoder.as_deref().or(args.codec.as_deref());
    if let Some(name) = codec_name {
        let input = args.input.as_ref().ok_or_else(|| {
            RelativistError::Config("--input is required when --encoder/--codec is set".to_string())
        })?;
        // TASK-0728 / D-017: when `--encode-only` is set, short-circuit after
        // `encode_and_validate` and write the un-reduced net to `--output`.
        let encode_only_out = if args.encode_only {
            Some(args.output.as_deref().ok_or_else(|| {
                // clap `requires = "output"` should already prevent this; the
                // defensive runtime check keeps the contract intact when the
                // command is constructed in code (integration tests).
                RelativistError::Config(
                    "--encode-only requires --output (path to write the un-reduced net)"
                        .to_string(),
                )
            })?)
        } else {
            None
        };
        return run_compute_with_encoder(name, input.as_bytes(), encode_only_out);
    }

    // SPEC-27 v3 R21: orphan `--input` without --encoder/--codec is a
    // configuration error.
    if args.input.is_some() {
        return Err(RelativistError::Config(
            "--input requires --encoder or --codec".to_string(),
        ));
    }

    // TASK-0728 / D-017 AC3: `--encode-only` requires `--encoder` / `--codec`.
    // Legacy positional `compute add 3 5 --encode-only` has no encoder to
    // dispatch — reject early with a clear Config error (not a panic).
    if args.encode_only {
        return Err(RelativistError::Config(
            "--encode-only requires --encoder or --codec (the legacy positional \
             `compute <op> <a> <b>` path has no encoder to dispatch)"
                .to_string(),
        ));
    }

    // Legacy SPEC-14 path: positional operation/a/b are required.
    let operation = args.operation.ok_or_else(|| {
        RelativistError::Config(
            "compute requires either positional <op> <a> <b> or --encoder/--input".to_string(),
        )
    })?;
    let a = args.a.ok_or_else(|| {
        RelativistError::Config("compute legacy mode requires positional <a>".to_string())
    })?;
    let b = args.b.ok_or_else(|| {
        RelativistError::Config("compute legacy mode requires positional <b>".to_string())
    })?;

    let op_name = match operation {
        ArithmeticOp::Add => "add",
        ArithmeticOp::Mul => "mul",
        ArithmeticOp::Exp => "exp",
    };

    // Build the arithmetic net
    let mut net = match operation {
        ArithmeticOp::Add => build_add(a, b),
        ArithmeticOp::Mul => build_mul(a, b),
        ArithmeticOp::Exp => build_exp(a, b),
    };

    let initial_agents = net.count_live_agents();
    let initial_redexes = net.redex_queue.len();

    println!("=== Relativist Compute ===");
    println!("Expression:  {}({}, {})", op_name, a, b);
    println!(
        "Encoding:    {} agents, {} redexes",
        initial_agents, initial_redexes
    );

    // Reduce (local or distributed)
    if let Some(workers) = args.workers {
        // Distributed mode via run_grid
        let grid_config = crate::merge::GridConfig {
            num_workers: workers,
            max_rounds: None,
            ..crate::merge::GridConfig::default()
        };
        let strategy = crate::partition::ContiguousIdStrategy;
        let (reduced_net, metrics) = run_grid(net, &grid_config, &strategy);
        net = reduced_net;

        let mips = if metrics.total_time.as_secs_f64() > 0.0 {
            metrics.total_interactions as f64 / metrics.total_time.as_secs_f64() / 1_000_000.0
        } else {
            0.0
        };
        println!(
            "Reduction:   {} interactions in {:.2}s ({:.2} MIPS)",
            metrics.total_interactions,
            metrics.total_time.as_secs_f64(),
            mips
        );
        println!("Workers:     {}", workers);
        println!("Rounds:      {}", metrics.rounds);

        if let Some(ref path) = args.metrics {
            write_metrics(&metrics, path)?;
        }
    } else {
        // Local reduction
        let start = std::time::Instant::now();
        let stats = reduce_all(&mut net);
        let elapsed = start.elapsed();

        let mips = if elapsed.as_secs_f64() > 0.0 {
            stats.total_interactions as f64 / elapsed.as_secs_f64() / 1_000_000.0
        } else {
            0.0
        };
        println!(
            "Reduction:   {} interactions in {:.2}s ({:.2} MIPS)",
            stats.total_interactions,
            elapsed.as_secs_f64(),
            mips
        );
    }

    // Discover root of the resulting Church numeral
    discover_root(&mut net);

    // Decode result: try canonical decode, then shared-chain fallback for mul
    let result =
        decode_nat(&net).or_else(|| crate::encoding::arithmetic::decode_shared_chain(&net));
    match result {
        Some(n) => println!("Result:      {}", n),
        None => {
            println!("Result:      (non-decodable normal form)");
            println!("  The net is in normal form but uses a non-canonical Church encoding");
            println!("  (e.g., cyclic DUP sharing from exponentiation).");
            println!("  Final agents: {}", net.count_live_agents());
        }
    }

    // Save output if requested
    if let Some(ref path) = args.output {
        save_net_to_file(&net, path)?;
    }

    Ok(())
}

/// SPEC-27 R21, R23: registry-driven compute path.
///
/// Pipeline: `encode → validate (E1+E2) → reduce_all → decode → print JSON`.
///
/// **TASK-0728 / D-017:** when `encode_only_output` is `Some(path)`, the
/// pipeline short-circuits after `encode_and_validate` and writes the
/// un-reduced net to `path` via `io::binary::save_bin`. The `reduce_all`
/// and decode stages are skipped; the produced `.bin` is the input format
/// the coordinator subcommand consumes via `--input`.
fn run_compute_with_encoder(
    name: &str,
    input: &[u8],
    encode_only_output: Option<&std::path::Path>,
) -> Result<(), RelativistError> {
    let registry = crate::encoding::default_registry();

    println!("=== Relativist Compute (encoder: {}) ===", name);

    let mut net = registry.encode_and_validate(name, input)?;
    let initial_agents = net.count_live_agents();
    let initial_redexes = net.redex_queue.len();
    println!(
        "Encoding:    {} agents, {} redexes",
        initial_agents, initial_redexes
    );

    // TASK-0728 / D-017 short-circuit: skip reduce_all + decode and persist
    // the un-reduced net for downstream coordinator consumption.
    if let Some(path) = encode_only_output {
        crate::io::binary::save_bin(&net, path)?;
        println!(
            "Encoded:     {} agents -> {}",
            net.count_live_agents(),
            path.display()
        );
        return Ok(());
    }

    let start = std::time::Instant::now();
    let stats = reduce_all(&mut net);
    let elapsed = start.elapsed();
    let mips = if elapsed.as_secs_f64() > 0.0 {
        stats.total_interactions as f64 / elapsed.as_secs_f64() / 1_000_000.0
    } else {
        0.0
    };
    println!(
        "Reduction:   {} interactions in {:.2}s ({:.2} MIPS)",
        stats.total_interactions,
        elapsed.as_secs_f64(),
        mips
    );

    // SPEC-27 v3 R23 pipeline (TASK-0721 BUG-001): codecs that compose Church
    // arithmetic via `wire_*_into` (HornerCodec, build_add/build_mul/...) emit
    // nets with `root = None` — the result wire is connected to `FreePort(0)`
    // and the post-reduction root must be discovered before decoding. The unit
    // tests `seq_decoded` / `inproc_decoded` (tests/horner_distributed_g1.rs)
    // already do this; the CLI path must mirror that contract.
    if net.root.is_none() {
        let recovered = crate::encoding::discover_root(&mut net);
        tracing::debug!(
            recovered_root = recovered,
            agents = net.count_live_agents(),
            "post-reduce root discovery"
        );
    }

    let json_out = registry.decode(name, &net)?;
    let pretty = serde_json::to_string_pretty(&json_out)
        .map_err(|e| RelativistError::Encoding(format!("serialize result: {}", e)))?;
    println!("Result:      {}", pretty);

    Ok(())
}

/// Render the `encoders list` output to a `String` (testable variant).
///
/// SPEC-27 v3 R22 — formatted as:
///
/// ```text
/// Available encoders:
///   <name padded to column>  <description>
///   ...
/// ```
///
/// The `list()` order matches `EncoderRegistry::list` (alphabetical on name).
fn format_encoders_list() -> String {
    let r = crate::encoding::default_registry();
    let pairs = r.list();
    let max_name = pairs.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
    let mut out = String::from("Available encoders:\n");
    for (name, desc) in pairs {
        out.push_str(&format!("  {:<width$}  {}\n", name, desc, width = max_name));
    }
    out
}

/// D-017 / TASK-0729: `decode` subcommand handler.
///
/// Loads a bincode-v2 `.bin` from `args.input`, runs the named codec's
/// decoder, and emits the result as pretty JSON to either `args.output`
/// (when set) or stdout. Errors propagate as:
///
/// - `RelativistError::Config` for missing/unknown codec, invalid `--codec`/
///   `--encoder` combination, or unparseable `.bin` (the load path returns
///   `Config` for any deserialization failure).
/// - `RelativistError::Encoding` for codec-level decode failures (e.g.,
///   `NotNormalForm` when the input net still has redexes — the operator
///   forgot to run the coordinator).
///
/// **Root recovery contract:** codecs that compose Church arithmetic via
/// `wire_*_into` (HornerCodec, `build_add`/`build_mul`/...) emit nets with
/// `root = None` — the result wire is connected to `FreePort(0)` and the
/// post-reduce root must be discovered before decoding. This mirrors the
/// in-process pipeline in `run_compute_with_encoder` (post-`reduce_all`
/// branch) and the test convention in `tests/horner_distributed_g1.rs`.
pub fn run_decode_command(
    args: crate::config::DecodeArgs,
) -> Result<(), RelativistError> {
    let name = args
        .codec
        .as_deref()
        .or(args.encoder.as_deref())
        .ok_or_else(|| {
            RelativistError::Config("decode requires --codec or --encoder".to_string())
        })?;
    let mut net = crate::io::binary::load_bin(&args.input)?;
    if net.root.is_none() {
        let recovered = crate::encoding::discover_root(&mut net);
        tracing::debug!(
            recovered_root = recovered,
            agents = net.count_live_agents(),
            "decode subcommand: post-load root discovery"
        );
    }
    let registry = crate::encoding::default_registry();
    let json_out = registry.decode(name, &net)?;
    let pretty = serde_json::to_string_pretty(&json_out)
        .map_err(|e| RelativistError::Encoding(format!("serialize result: {}", e)))?;
    match args.output {
        Some(ref path) => std::fs::write(path, &pretty)?,
        None => println!("{}", pretty),
    }
    Ok(())
}

/// SPEC-27 v3 R22: list registered encoders with descriptions.
///
/// The `codecs` subcommand is a clap alias for `encoders` (R22 MAY) — both
/// dispatch through this handler.
pub fn run_encoders_command(args: EncodersArgs) -> Result<(), RelativistError> {
    match args.action {
        EncodersAction::List => {
            print!("{}", format_encoders_list());
        }
    }
    Ok(())
}

/// Execute validate: run data quality checks on benchmark CSVs (DATA-COLLECTION-PLAN Section 10).
pub fn run_validate_command(args: ValidateArgs) -> Result<(), RelativistError> {
    use crate::bench::validate::validate_campaign;

    let report = validate_campaign(&args.detail, &args.summary, &args.rounds);
    report.print();

    if report.all_hard_passed {
        Ok(())
    } else {
        Err(RelativistError::Config(
            "Data quality validation failed: one or more hard checks did not pass".into(),
        ))
    }
}

/// Execute update: check for and install the latest release (SPEC-15 R19).
pub fn run_update_command(args: UpdateArgs) -> Result<(), RelativistError> {
    let current = env!("CARGO_PKG_VERSION");
    let repo = "andrade-filipe/relativist";
    let api_path = format!("repos/{}/releases/latest", repo);

    // Try `gh api` first (works with private repos), fall back to `curl`
    let body = fetch_release_json(&api_path, repo)?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
        RelativistError::Config(format!("Failed to parse GitHub API response: {}", e))
    })?;

    let tag = json["tag_name"]
        .as_str()
        .ok_or_else(|| RelativistError::Config("No tag_name in release".into()))?;
    let latest = tag.strip_prefix('v').unwrap_or(tag);

    println!("Current version: {}", current);
    println!("Latest version:  {}", latest);

    if current == latest {
        println!("\nAlready up to date!");
        return Ok(());
    }

    println!("\nUpdate available: {} -> {}", current, latest);

    if args.check {
        return Ok(());
    }

    // Determine platform artifact name
    let (target, ext) = if cfg!(target_os = "windows") {
        ("x86_64-pc-windows-msvc", "exe")
    } else if cfg!(target_os = "linux") {
        ("x86_64-unknown-linux-gnu", "tar.gz")
    } else if cfg!(target_os = "macos") {
        ("x86_64-apple-darwin", "tar.gz")
    } else {
        return Err(RelativistError::Config(
            "Unsupported platform for self-update".into(),
        ));
    };

    let artifact_name = if ext == "exe" {
        format!("relativist-{}-{}.exe", tag, target)
    } else {
        format!("relativist-{}-{}.tar.gz", tag, target)
    };

    let tmp_dir = std::env::temp_dir().join("relativist-update");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| RelativistError::Config(format!("Failed to create temp dir: {}", e)))?;

    let artifact_path = tmp_dir.join(&artifact_name);
    let checksums_path = tmp_dir.join("SHA256SUMS");

    // Download artifact
    println!("\nDownloading {}...", artifact_name);
    download_release_asset(repo, tag, &artifact_name, &artifact_path)?;

    // Download and verify checksum
    println!("Verifying checksum...");
    let checksums_ok = download_release_asset(repo, tag, "SHA256SUMS", &checksums_path).is_ok();

    if checksums_ok {
        let checksums_content = std::fs::read_to_string(&checksums_path).unwrap_or_default();
        if let Some(expected_line) = checksums_content
            .lines()
            .find(|l| l.contains(&artifact_name))
        {
            let expected_hash = expected_line.split_whitespace().next().unwrap_or("");
            // Compute SHA256 of downloaded file
            let hash = compute_sha256(&artifact_path)?;
            if hash != expected_hash {
                return Err(RelativistError::Config(format!(
                    "Checksum mismatch!\n  Expected: {}\n  Got:      {}",
                    expected_hash, hash
                )));
            }
            println!("Checksum OK: {}", &hash[..16]);
        } else {
            eprintln!("Warning: artifact not found in SHA256SUMS, skipping verification.");
        }
    } else {
        eprintln!("Warning: could not download SHA256SUMS, skipping verification.");
    }

    // Determine canonical install directory and install there
    let install_dir = canonical_install_dir()?;
    std::fs::create_dir_all(&install_dir).map_err(|e| {
        RelativistError::Config(format!(
            "Cannot create install dir {:?}: {}",
            install_dir, e
        ))
    })?;

    let bin_name = if cfg!(target_os = "windows") {
        "relativist.exe"
    } else {
        "relativist"
    };
    let install_path = install_dir.join(bin_name);

    if cfg!(target_os = "windows") {
        // Windows: copy new binary to canonical location
        if install_path.exists() {
            let old_path = install_dir.join("relativist.exe.old");
            let _ = std::fs::remove_file(&old_path);
            std::fs::rename(&install_path, &old_path)
                .map_err(|e| RelativistError::Config(format!("Cannot rename old binary: {}", e)))?;
            std::fs::copy(&artifact_path, &install_path).map_err(|e| {
                let _ = std::fs::rename(&old_path, &install_path);
                RelativistError::Config(format!("Cannot install new binary: {}", e))
            })?;
            let _ = std::fs::remove_file(&old_path);
        } else {
            std::fs::copy(&artifact_path, &install_path)
                .map_err(|e| RelativistError::Config(format!("Cannot install binary: {}", e)))?;
        }
    } else {
        // Unix: extract from tar.gz, copy to canonical location
        let status = std::process::Command::new("tar")
            .args([
                "xzf",
                &artifact_path.display().to_string(),
                "-C",
                &tmp_dir.display().to_string(),
            ])
            .status()
            .map_err(|e| RelativistError::Config(format!("tar failed: {}", e)))?;

        if !status.success() {
            return Err(RelativistError::Config("Failed to extract archive".into()));
        }

        let new_binary = tmp_dir.join("relativist");
        std::fs::copy(&new_binary, &install_path)
            .map_err(|e| RelativistError::Config(format!("Cannot install binary: {}", e)))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&install_path, std::fs::Permissions::from_mode(0o755));
        }
    }

    // Ensure canonical install dir is in PATH
    let path_added = ensure_in_path(&install_dir)?;

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp_dir);

    println!("\nrelativist updated: {} -> {}", current, latest);
    println!("Installed to: {}", install_path.display());
    if path_added {
        println!("\nAdded {} to your PATH.", install_dir.display());
        println!("Please restart your terminal for the change to take effect.");
    }
    println!("Verify: relativist --version");

    Ok(())
}

/// Return the canonical install directory for the relativist binary.
///
/// - Windows: `%LOCALAPPDATA%\relativist\bin\`
/// - Linux/macOS: `~/.relativist/bin/`
fn canonical_install_dir() -> Result<std::path::PathBuf, RelativistError> {
    if cfg!(target_os = "windows") {
        // %LOCALAPPDATA%\relativist\bin
        let local_app = std::env::var("LOCALAPPDATA").map_err(|_| {
            RelativistError::Config("LOCALAPPDATA environment variable not set".into())
        })?;
        Ok(std::path::PathBuf::from(local_app)
            .join("relativist")
            .join("bin"))
    } else {
        // ~/.relativist/bin
        let home = std::env::var("HOME")
            .map_err(|_| RelativistError::Config("HOME environment variable not set".into()))?;
        Ok(std::path::PathBuf::from(home)
            .join(".relativist")
            .join("bin"))
    }
}

/// Ensure the given directory is in the user's PATH. Returns true if it was added.
fn ensure_in_path(dir: &std::path::Path) -> Result<bool, RelativistError> {
    let dir_str = dir.to_string_lossy();

    // Check if already in PATH
    if let Ok(path_var) = std::env::var("PATH") {
        let sep = if cfg!(target_os = "windows") {
            ';'
        } else {
            ':'
        };
        for entry in path_var.split(sep) {
            if std::path::Path::new(entry) == dir {
                return Ok(false); // already present
            }
        }
    }

    if cfg!(target_os = "windows") {
        // Windows: use setx to permanently add to user PATH
        // Read current user PATH from registry via powershell
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Environment]::GetEnvironmentVariable('Path', 'User')",
            ])
            .output()
            .map_err(|e| RelativistError::Config(format!("Cannot read user PATH: {}", e)))?;

        let current_user_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Check if already in user PATH (may differ from session PATH)
        let already = current_user_path
            .split(';')
            .any(|e| std::path::Path::new(e.trim()) == dir);
        if already {
            return Ok(false);
        }

        let new_path = if current_user_path.is_empty() {
            dir_str.to_string()
        } else {
            format!("{};{}", current_user_path, dir_str)
        };

        let ps_cmd = format!(
            "[Environment]::SetEnvironmentVariable('Path', '{}', 'User')",
            new_path.replace('\'', "''")
        );
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .status()
            .map_err(|e| RelativistError::Config(format!("Cannot set user PATH: {}", e)))?;

        if !status.success() {
            eprintln!("Warning: could not add {} to PATH automatically.", dir_str);
            eprintln!("Add it manually: setx PATH \"%PATH%;{}\"", dir_str);
            return Ok(false);
        }
    } else {
        // Unix: append to ~/.bashrc (most common shell)
        let home = std::env::var("HOME").unwrap_or_default();
        let bashrc = std::path::PathBuf::from(&home).join(".bashrc");
        let export_line = format!("\nexport PATH=\"{}:$PATH\"\n", dir_str);

        // Check if already in .bashrc
        if let Ok(contents) = std::fs::read_to_string(&bashrc) {
            if contents.contains(&format!("{}:", dir_str))
                || contents.contains(&format!("\"{}\"", dir_str))
            {
                return Ok(false);
            }
        }

        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&bashrc)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(export_line.as_bytes())
            })
            .map_err(|e| RelativistError::Config(format!("Cannot update ~/.bashrc: {}", e)))?;
    }

    Ok(true)
}

/// Return candidate paths for the `gh` CLI binary.
/// On Windows, the default installer puts it in `C:\Program Files\GitHub CLI\`
/// which may not be in the process PATH (e.g., when launched from PowerShell
/// or from a cargo-built binary).
fn gh_candidates() -> Vec<String> {
    let mut candidates = vec!["gh".to_string()];
    if cfg!(target_os = "windows") {
        candidates.push(r"C:\Program Files\GitHub CLI\gh.exe".to_string());
    }
    candidates
}

/// Fetch latest release JSON from GitHub API.
/// Tries `gh api` first (handles private repos via auth), falls back to `curl`.
fn fetch_release_json(api_path: &str, _repo: &str) -> Result<String, RelativistError> {
    // Try gh first (check multiple paths on Windows)
    for gh_cmd in gh_candidates() {
        if let Ok(output) = std::process::Command::new(&gh_cmd)
            .args(["api", api_path])
            .output()
        {
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
        }
    }

    // Fallback: curl (works for public repos)
    let url = format!("https://api.github.com/{}", api_path);
    let output = std::process::Command::new("curl")
        .args(["-sSfL", "-H", "Accept: application/json", &url])
        .output()
        .map_err(|e| {
            RelativistError::Config(format!(
                "Neither gh nor curl available: {}. Install gh (https://cli.github.com) or curl.",
                e
            ))
        })?;

    if !output.status.success() {
        return Err(RelativistError::Config(format!(
            "Failed to fetch release info. If the repo is private, install and authenticate gh:\n  gh auth login\nError: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Download a file from GitHub Release assets.
/// Tries `gh release download` first (handles private repos), falls back to `curl`.
fn download_release_asset(
    repo: &str,
    tag: &str,
    asset_name: &str,
    dest: &std::path::Path,
) -> Result<(), RelativistError> {
    // Try gh first (check multiple paths on Windows)
    for gh_cmd in gh_candidates() {
        if let Ok(status) = std::process::Command::new(&gh_cmd)
            .args([
                "release",
                "download",
                tag,
                "--repo",
                repo,
                "--pattern",
                asset_name,
                "--dir",
                &dest.parent().unwrap_or(dest).display().to_string(),
            ])
            .status()
        {
            if status.success() {
                return Ok(());
            }
        }
    }

    // Fallback: curl (public repos)
    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        repo, tag, asset_name
    );
    let status = std::process::Command::new("curl")
        .args(["-sSfL", "-o", &dest.display().to_string(), &url])
        .status()
        .map_err(|e| RelativistError::Config(format!("curl failed: {}", e)))?;

    if !status.success() {
        return Err(RelativistError::Config(format!(
            "Failed to download {}",
            asset_name
        )));
    }
    Ok(())
}

/// Compute SHA256 hash of a file (for update checksum verification).
fn compute_sha256(path: &std::path::Path) -> Result<String, RelativistError> {
    // Shell out to a platform tool to avoid adding a SHA256 dependency
    if cfg!(target_os = "windows") {
        let output = std::process::Command::new("certutil")
            .args(["-hashfile", &path.display().to_string(), "SHA256"])
            .output()
            .map_err(|e| RelativistError::Config(format!("certutil failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        // certutil output: line 0 = header, line 1 = hash, line 2 = footer
        let hash = stdout
            .lines()
            .nth(1)
            .unwrap_or("")
            .trim()
            .replace(' ', "")
            .to_lowercase();
        Ok(hash)
    } else {
        let output = std::process::Command::new("sha256sum")
            .arg(path.as_os_str())
            .output()
            .map_err(|e| RelativistError::Config(format!("sha256sum failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let hash = stdout.split_whitespace().next().unwrap_or("").to_string();
        Ok(hash)
    }
}

/// Execute completions: generate shell completion scripts (SPEC-15 R20).
pub fn run_completions_command(args: CompletionsArgs) -> Result<(), RelativistError> {
    use crate::config::{Cli, ShellType};
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    let shell = match args.shell {
        ShellType::Bash => Shell::Bash,
        ShellType::Zsh => Shell::Zsh,
        ShellType::Fish => Shell::Fish,
        ShellType::PowerShell => Shell::PowerShell,
    };

    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "relativist", &mut std::io::stdout());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ArithmeticOp, ComputeArgs};

    fn empty_compute_args() -> ComputeArgs {
        ComputeArgs {
            operation: None,
            a: None,
            b: None,
            encoder: None,
            codec: None,
            input: None,
            workers: None,
            output: None,
            metrics: None,
            encode_only: false,
        }
    }

    // SPEC-27 R21: legacy mode with no positional args → Config error.
    #[test]
    fn run_compute_legacy_missing_operation_errors() {
        let args = empty_compute_args();
        let err = run_compute_command(args).unwrap_err();
        assert!(matches!(err, RelativistError::Config(_)));
    }

    // SPEC-27 v3 R21: --encoder/--codec set but --input missing → Config error.
    #[test]
    fn run_compute_encoder_without_input_errors() {
        let mut args = empty_compute_args();
        args.encoder = Some("horner".to_string());
        let err = run_compute_command(args).unwrap_err();
        match err {
            RelativistError::Config(msg) => assert!(msg.contains("--input")),
            other => panic!("expected Config, got {:?}", other),
        }

        // Same path through --codec.
        let mut args = empty_compute_args();
        args.codec = Some("horner".to_string());
        let err = run_compute_command(args).unwrap_err();
        match err {
            RelativistError::Config(msg) => assert!(msg.contains("--input")),
            other => panic!("expected Config, got {:?}", other),
        }
    }

    // SPEC-27 R21: legacy mode with operation but missing operand → Config error.
    #[test]
    fn run_compute_legacy_missing_b_errors() {
        let mut args = empty_compute_args();
        args.operation = Some(ArithmeticOp::Add);
        args.a = Some(3);
        // b missing
        let err = run_compute_command(args).unwrap_err();
        assert!(matches!(err, RelativistError::Config(_)));
    }

    // SPEC-27 R21: unknown encoder bubbles up as Encoding error.
    #[test]
    fn run_compute_unknown_encoder_errors() {
        let err = run_compute_with_encoder("nope_does_not_exist", b"{}", None).unwrap_err();
        match err {
            RelativistError::Encoding(msg) => assert!(msg.contains("not found")),
            other => panic!("expected Encoding, got {:?}", other),
        }
    }

    // SPEC-27 v3 R23: invalid JSON for a real encoder → Encoding error
    // (propagated from the codec via RegistryError::Encode). Uses
    // `horner` since `lambda` is no longer in the v3 default registry
    // (TASK-0716).
    #[test]
    fn run_compute_with_encoder_invalid_input_errors() {
        let err = run_compute_with_encoder("horner", b"{not json}", None).unwrap_err();
        assert!(matches!(err, RelativistError::Encoding(_)));
    }

    // SPEC-27 R22: encoders list returns Ok and prints (smoke).
    #[test]
    fn run_encoders_list_succeeds() {
        let args = EncodersArgs {
            action: EncodersAction::List,
        };
        assert!(run_encoders_command(args).is_ok());
    }

    // --- TASK-0718 / SPEC-27 v3 R22 ---

    // UT-0718-03 / T21: rendered output contains the 5 v3 codecs in
    // canonical R19 order; first line is "Available encoders:".
    #[test]
    fn cli_encoders_list_outputs_5_v3_codecs() {
        let out = format_encoders_list();
        assert!(out.starts_with("Available encoders:"));

        // Canonical R19 order is alphabetical on name (matches
        // `EncoderRegistry::list`).
        let expected_order = [
            "church_add",
            "church_exp",
            "church_mul",
            "church_sum_of_squares",
            "horner",
        ];
        let mut last_pos = 0usize;
        for name in &expected_order {
            let pos = out
                .find(name)
                .unwrap_or_else(|| panic!("missing codec in output: {name}\n{out}"));
            assert!(pos >= last_pos, "codec {name} appears out of R19 order");
            last_pos = pos;
        }

        // Exactly 5 lines following the header (one per codec).
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines.len(),
            6,
            "expected 1 header + 5 codecs, got {lines:?}"
        );
    }

    // UT-0718-04 / T16-derived: lambda is NOT in the default output.
    #[test]
    fn cli_encoders_list_excludes_lambda() {
        let out = format_encoders_list();
        assert!(
            !out.contains("lambda"),
            "default output must not list lambda: {out}"
        );
    }

    // UT-0718-05 / R22: output includes a horner line whose description
    // matches HornerCodec::description().
    #[test]
    fn cli_encoders_list_includes_horner_and_description() {
        let out = format_encoders_list();
        let horner_line = out
            .lines()
            .find(|l| l.trim_start().starts_with("horner"))
            .expect("horner line missing in encoders list");
        assert!(
            horner_line.contains("Polynomial evaluation via Horner's method"),
            "horner description mismatch: {horner_line}"
        );
    }
}
