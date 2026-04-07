//! User I/O: net formats, generators, summary output (SPEC-12).
//!
//! - `binary`: .bin format (serde + bincode)
//! - `text_dsl`: .ic format (human-readable text DSL)
//! - `generators`: pre-built example net generators
//! - `types`: NetFormat, NetSummary, ReductionSummary

pub mod binary;
pub mod generators;
pub mod text_dsl;
pub mod types;

pub use binary::{deserialize_net, load_bin, save_bin, serialize_net};
pub use generators::{generate, ExampleNet};
pub use text_dsl::{format_ic, load_ic, parse_ic, save_ic};
pub use types::{detect_format, net_summary, NetFormat, NetSummary, ReductionSummary};

use std::path::Path;

use crate::error::RelativistError;
use crate::merge::GridMetrics;
use crate::net::{Net, Symbol};

// ---------------------------------------------------------------------------
// Format-dispatching load/save (TASK-0168, SPEC-12 R1, R18)
// ---------------------------------------------------------------------------

/// Load a Net from a file, auto-detecting format from extension (SPEC-12 R1).
///
/// Supports .bin (bincode), .ic (text DSL).
/// JSON format (.json) returns a descriptive error (SPEC-12 R17).
pub fn load_net_from_file(path: &Path) -> Result<Net, RelativistError> {
    let format = detect_format(path).ok_or_else(|| {
        RelativistError::Config(format!(
            "unknown file extension for {:?} (expected .bin, .ic, or .json)",
            path
        ))
    })?;

    let net = match format {
        NetFormat::Bin => load_bin(path)?,
        NetFormat::Ic => load_ic(path)?,
        NetFormat::Json => {
            return Err(RelativistError::Config(
                "JSON format not yet supported; use .bin or .ic".into(),
            ));
        }
    };

    tracing::info!(
        path = ?path,
        format = ?format,
        agents = net.count_live_agents(),
        redexes = net.redex_queue.len(),
        "loaded network"
    );
    Ok(net)
}

/// Save a Net to a file, format determined by extension.
pub fn save_net_to_file(net: &Net, path: &Path) -> Result<(), RelativistError> {
    let format = detect_format(path).ok_or_else(|| {
        RelativistError::Config(format!(
            "unknown file extension for {:?} (expected .bin, .ic, or .json)",
            path
        ))
    })?;

    match format {
        NetFormat::Bin => save_bin(net, path)?,
        NetFormat::Ic => save_ic(net, path)?,
        NetFormat::Json => {
            return Err(RelativistError::Config(
                "JSON format not yet supported; use .bin or .ic".into(),
            ));
        }
    }

    tracing::info!(path = ?path, format = ?format, "saved network");
    Ok(())
}

// ---------------------------------------------------------------------------
// Metrics output (SPEC-07 R27-R31)
// ---------------------------------------------------------------------------

/// Write metrics in JSON or CSV format, determined by file extension.
pub fn write_metrics(metrics: &GridMetrics, path: &Path) -> Result<(), RelativistError> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") => write_metrics_csv(metrics, path),
        _ => write_metrics_json(metrics, path),
    }
}

fn write_metrics_json(metrics: &GridMetrics, path: &Path) -> Result<(), RelativistError> {
    let json = serde_json::to_string_pretty(metrics)
        .map_err(|e| RelativistError::Config(format!("JSON serialization failed: {}", e)))?;
    std::fs::write(path, json)?;
    tracing::info!(path = ?path, "saved metrics (JSON)");
    Ok(())
}

fn write_metrics_csv(metrics: &GridMetrics, path: &Path) -> Result<(), RelativistError> {
    let mut csv = String::new();
    csv.push_str("round,agents,local_interactions,border_interactions,border_redexes,partition_time_ms,compute_time_ms,merge_time_ms,bytes_sent,bytes_received,network_send_time_ms,network_recv_time_ms\n");

    for r in 0..metrics.rounds as usize {
        let agents = metrics.agents_per_round.get(r).copied().unwrap_or(0);
        let local = metrics.local_interactions_per_round.get(r).copied().unwrap_or(0);
        let border = metrics.border_interactions_per_round.get(r).copied().unwrap_or(0);
        let border_redexes = metrics.border_redexes_per_round.get(r).copied().unwrap_or(0);
        let partition_ms = metrics.partition_time_per_round.get(r).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        let compute_ms = metrics.compute_time_per_round.get(r).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        let merge_ms = metrics.merge_time_per_round.get(r).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        let bytes_sent = metrics.bytes_sent_per_round.get(r).copied().unwrap_or(0);
        let bytes_recv = metrics.bytes_received_per_round.get(r).copied().unwrap_or(0);
        let net_send_ms = metrics.network_send_time_per_round.get(r).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
        let net_recv_ms = metrics.network_recv_time_per_round.get(r).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);

        csv.push_str(&format!(
            "{},{},{},{},{},{:.3},{:.3},{:.3},{},{},{:.3},{:.3}\n",
            r + 1, agents, local, border, border_redexes,
            partition_ms, compute_ms, merge_ms,
            bytes_sent, bytes_recv, net_send_ms, net_recv_ms
        ));
    }

    std::fs::write(path, csv)?;
    tracing::info!(path = ?path, "saved metrics (CSV)");
    Ok(())
}

// ---------------------------------------------------------------------------
// Print summary (SPEC-12 R35)
// ---------------------------------------------------------------------------

/// Count agents of a specific symbol in a net.
pub fn count_agents_by_symbol(net: &Net, symbol: Symbol) -> usize {
    net.live_agents().filter(|a| a.symbol == symbol).count()
}

/// Print a human-readable execution summary to stdout.
pub fn print_summary(net: &Net, metrics: &GridMetrics) {
    println!("=== Relativist Execution Summary ===");
    println!("Converged:          {}", if metrics.converged { "yes" } else { "no" });
    println!("Rounds:             {}", metrics.rounds);
    println!("Total interactions: {}", metrics.total_interactions);
    println!("Total time:         {:.3}s", metrics.total_time.as_secs_f64());
    println!("Final agents:       {}", net.count_live_agents());
    println!("  CON: {}", count_agents_by_symbol(net, Symbol::Con));
    println!("  DUP: {}", count_agents_by_symbol(net, Symbol::Dup));
    println!("  ERA: {}", count_agents_by_symbol(net, Symbol::Era));

    if metrics.rounds > 0 {
        let avg_round = metrics.total_time.as_secs_f64() / metrics.rounds as f64;
        let total_local: u64 = metrics.local_interactions_per_round.iter().sum();
        let total_border: u64 = metrics.border_interactions_per_round.iter().sum();
        println!("Avg round time:     {:.3}s", avg_round);
        println!("Local interactions: {}", total_local);
        println!("Border interactions:{}", total_border);
    }

    let total_bytes = metrics.total_network_bytes();
    if total_bytes > 0 {
        let sent: usize = metrics.bytes_sent_per_round.iter().sum();
        let recv: usize = metrics.bytes_received_per_round.iter().sum();
        println!("Bytes sent:         {}", sent);
        println!("Bytes received:     {}", recv);
        println!("Network overhead:   {:.1}%", metrics.network_overhead_fraction() * 100.0);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};
    use std::path::PathBuf;

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_net_from_file(&PathBuf::from("nonexistent_file_xyz.bin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_unknown_extension() {
        let result = load_net_from_file(&PathBuf::from("net.xyz"));
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("unknown file extension"));
    }

    #[test]
    fn test_load_json_not_supported() {
        // Create a temporary file with .json extension
        let path = std::env::temp_dir().join("relativist_test_unsupported.json");
        std::fs::write(&path, "{}").unwrap();
        let result = load_net_from_file(&path);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("JSON format not yet supported"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_bin_file_roundtrip() {
        let mut net = Net::new();
        net.create_agent(Symbol::Era);
        let path = std::env::temp_dir().join("relativist_test_dispatch.bin");
        save_net_to_file(&net, &path).unwrap();
        let restored = load_net_from_file(&path).unwrap();
        assert_eq!(restored.count_live_agents(), 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_ic_file_roundtrip() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let path = std::env::temp_dir().join("relativist_test_dispatch.ic");
        save_net_to_file(&net, &path).unwrap();
        let restored = load_net_from_file(&path).unwrap();
        assert_eq!(restored.count_live_agents(), 2);
        let _ = std::fs::remove_file(&path);
    }

    // === Metrics output tests ===

    #[test]
    fn test_write_metrics_json() {
        let mut metrics = GridMetrics::default();
        metrics.rounds = 2;
        metrics.total_interactions = 100;
        metrics.converged = true;
        metrics.agents_per_round = vec![50, 30];
        metrics.local_interactions_per_round = vec![40, 60];
        metrics.border_interactions_per_round = vec![0, 0];
        metrics.border_redexes_per_round = vec![0, 0];
        metrics.partition_time_per_round = vec![std::time::Duration::from_millis(1), std::time::Duration::from_millis(2)];
        metrics.compute_time_per_round = vec![std::time::Duration::from_millis(10), std::time::Duration::from_millis(20)];
        metrics.merge_time_per_round = vec![std::time::Duration::from_millis(5), std::time::Duration::from_millis(5)];

        let path = std::env::temp_dir().join("relativist_test_metrics_p9.json");
        write_metrics(&metrics, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"rounds\": 2"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_write_metrics_csv() {
        let mut metrics = GridMetrics::default();
        metrics.rounds = 2;
        metrics.agents_per_round = vec![50, 30];
        metrics.local_interactions_per_round = vec![40, 60];
        metrics.border_interactions_per_round = vec![0, 0];
        metrics.border_redexes_per_round = vec![1, 2];
        metrics.partition_time_per_round = vec![std::time::Duration::from_millis(1), std::time::Duration::from_millis(2)];
        metrics.compute_time_per_round = vec![std::time::Duration::from_millis(10), std::time::Duration::from_millis(20)];
        metrics.merge_time_per_round = vec![std::time::Duration::from_millis(5), std::time::Duration::from_millis(5)];

        let path = std::env::temp_dir().join("relativist_test_metrics_p9.csv");
        write_metrics(&metrics, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].starts_with("round,agents,"));
        assert_eq!(lines.len(), 3);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_count_agents_by_symbol() {
        let mut net = Net::new();
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Con);
        net.create_agent(Symbol::Dup);
        net.create_agent(Symbol::Era);
        assert_eq!(count_agents_by_symbol(&net, Symbol::Con), 2);
        assert_eq!(count_agents_by_symbol(&net, Symbol::Dup), 1);
        assert_eq!(count_agents_by_symbol(&net, Symbol::Era), 1);
    }

    #[test]
    fn test_print_summary_no_panic() {
        let net = Net::new();
        let metrics = GridMetrics::default();
        print_summary(&net, &metrics);
    }
}
