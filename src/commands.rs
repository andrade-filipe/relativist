//! Command entry points for each CLI subcommand.
//!
//! Each function takes the parsed Args struct and returns
//! Result<(), RelativistError>. Wired from main.rs.

use crate::config::{
    build_grid_config, build_grid_config_from_local, build_node_config_coordinator,
    build_node_config_worker, parse_strategy, BenchArgs, CompletionsArgs, CoordinatorArgs,
    GenerateArgs, InspectArgs, LocalArgs, ReduceArgs, UpdateArgs, WorkerArgs,
};
use crate::error::RelativistError;
use crate::io::{load_net_from_file, print_summary, save_net_to_file, write_metrics};
use crate::merge::run_grid;
use crate::reduction::reduce_all;

/// Execute local mode: grid loop in-process, no TCP (SPEC-07 R18).
pub fn run_local_command(args: LocalArgs) -> Result<(), RelativistError> {
    let grid_config = build_grid_config_from_local(&args);
    let strategy = parse_strategy(&args.strategy)?;

    let net = load_net_from_file(&args.input)?;
    let (reduced_net, metrics) = run_grid(net, &grid_config, &*strategy);

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
/// Full implementation depends on the async runtime wiring (TASK-0112).
/// This is a placeholder that validates config and loads the net.
pub fn run_coordinator_command(args: CoordinatorArgs) -> Result<(), RelativistError> {
    let _grid_config = build_grid_config(&args);
    let _node_config = build_node_config_coordinator(&args);
    let _strategy = parse_strategy(&args.strategy)?;
    let _net = load_net_from_file(&args.input)?;

    Err(RelativistError::Config(
        "coordinator: distributed mode not yet wired (needs async runtime)".into(),
    ))
}

/// Execute worker mode: connect to coordinator (SPEC-07 R16).
///
/// Full implementation depends on the async runtime wiring (TASK-0113).
/// This is a placeholder that validates config.
pub fn run_worker_command(args: WorkerArgs) -> Result<(), RelativistError> {
    let _node_config = build_node_config_worker(&args)?;

    Err(RelativistError::Config(
        "worker: distributed mode not yet wired (needs async runtime)".into(),
    ))
}

/// Execute bench mode: run the benchmark suite (SPEC-09 R1, R6).
pub fn run_bench_command(args: BenchArgs) -> Result<(), RelativistError> {
    use crate::bench::csv::{write_csv_detail, write_csv_rounds, write_csv_summary};
    use crate::bench::suite::run_benchmark_suite;
    use crate::bench::{BenchmarkId, BenchmarkSuiteConfig, Mode};

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

    if !suite_result.all_correct {
        return Err(RelativistError::Config(
            "One or more benchmarks had correctness failures!".into(),
        ));
    }

    Ok(())
}

/// Execute compute mode: encode arithmetic, reduce, decode result (SPEC-14 R22-R25).
pub fn run_compute_command(args: crate::config::ComputeArgs) -> Result<(), RelativistError> {
    use crate::config::ArithmeticOp;
    use crate::encoding::{build_add, build_exp, build_mul, decode_nat, discover_root};

    let op_name = match args.operation {
        ArithmeticOp::Add => "add",
        ArithmeticOp::Mul => "mul",
        ArithmeticOp::Exp => "exp",
    };

    // Build the arithmetic net
    let mut net = match args.operation {
        ArithmeticOp::Add => build_add(args.a, args.b),
        ArithmeticOp::Mul => build_mul(args.a, args.b),
        ArithmeticOp::Exp => build_exp(args.a, args.b),
    };

    let initial_agents = net.count_live_agents();
    let initial_redexes = net.redex_queue.len();

    println!("=== Relativist Compute ===");
    println!("Expression:  {}({}, {})", op_name, args.a, args.b);
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

    // Extract/install
    let current_exe = std::env::current_exe().map_err(|e| {
        RelativistError::Config(format!("Cannot determine current executable path: {}", e))
    })?;

    if cfg!(target_os = "windows") {
        // Windows: rename current .exe to .old, copy new
        let old_path = current_exe.with_extension("exe.old");
        let _ = std::fs::remove_file(&old_path);
        std::fs::rename(&current_exe, &old_path).map_err(|e| {
            RelativistError::Config(format!(
                "Cannot rename current executable: {}. Try running as administrator.",
                e
            ))
        })?;
        std::fs::copy(&artifact_path, &current_exe).map_err(|e| {
            // Try to restore old binary
            let _ = std::fs::rename(&old_path, &current_exe);
            RelativistError::Config(format!("Cannot install new binary: {}", e))
        })?;
        let _ = std::fs::remove_file(&old_path);
    } else {
        // Unix: extract from tar.gz, then replace
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
        std::fs::copy(&new_binary, &current_exe).map_err(|e| {
            RelativistError::Config(format!(
                "Cannot replace binary at {}: {}. Try: sudo relativist update",
                current_exe.display(),
                e
            ))
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&current_exe, std::fs::Permissions::from_mode(0o755));
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp_dir);

    println!("\nrelativist updated: {} -> {}", current, latest);
    println!("Verify: relativist --version");

    Ok(())
}

/// Fetch latest release JSON from GitHub API.
/// Tries `gh api` first (handles private repos via auth), falls back to `curl`.
fn fetch_release_json(api_path: &str, _repo: &str) -> Result<String, RelativistError> {
    // Try gh first
    if let Ok(output) = std::process::Command::new("gh")
        .args(["api", api_path])
        .output()
    {
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
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
    // Try gh first
    if let Ok(status) = std::process::Command::new("gh")
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
