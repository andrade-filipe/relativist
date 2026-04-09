//! Integration tests for CLI end-to-end (TASK-0119).
//!
//! Tests the local mode round-trip: generate a net, save it,
//! run local mode, verify output.

use std::path::PathBuf;

use relativist::commands::run_local_command;
use relativist::config::LocalArgs;
use relativist::io::{load_net_from_file, save_net_to_file};
use relativist::net::{Net, PortRef, Symbol};

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("relativist_test_{}", name))
}

/// Create a small net with 2 CON agents connected via principal ports
/// (one active pair = one annihilation).
fn make_annihilation_net() -> Net {
    let mut net = Net::new();
    let a = net.create_agent(Symbol::Con);
    let b = net.create_agent(Symbol::Con);
    // Principal-principal connection → active pair
    net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
    // Aux ports connected to each other
    net.connect(PortRef::AgentPort(a, 1), PortRef::AgentPort(b, 1));
    net.connect(PortRef::AgentPort(a, 2), PortRef::AgentPort(b, 2));
    net
}

#[test]
fn test_local_mode_roundtrip() {
    let net = make_annihilation_net();
    assert_eq!(net.count_live_agents(), 2);
    assert_eq!(net.redex_queue.len(), 1);

    let input_path = temp_path("local_input.bin");
    let output_path = temp_path("local_output.bin");
    let metrics_path = temp_path("local_metrics.json");

    save_net_to_file(&net, &input_path).unwrap();

    let args = LocalArgs {
        workers: 2,
        input: input_path.clone(),
        max_rounds: None,
        output: Some(output_path.clone()),
        metrics: Some(metrics_path.clone()),
        strategy: "round-robin".to_string(),
        log_format: None,
    };

    run_local_command(args).unwrap();

    // Verify output net exists and is in normal form
    let reduced = load_net_from_file(&output_path).unwrap();
    assert_eq!(
        reduced.count_live_agents(),
        0,
        "annihilation should remove both agents"
    );
    assert!(reduced.redex_queue.is_empty(), "should be in normal form");

    // Verify metrics file exists and is valid JSON
    let metrics_content = std::fs::read_to_string(&metrics_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&metrics_content).unwrap();
    assert_eq!(json["converged"], true);

    // Cleanup
    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);
    let _ = std::fs::remove_file(&metrics_path);
}

#[test]
fn test_local_mode_single_worker() {
    let net = make_annihilation_net();

    let input_path = temp_path("local_single_input.bin");
    let output_path = temp_path("local_single_output.bin");

    save_net_to_file(&net, &input_path).unwrap();

    let args = LocalArgs {
        workers: 1,
        input: input_path.clone(),
        max_rounds: None,
        output: Some(output_path.clone()),
        metrics: None,
        strategy: "round-robin".to_string(),
        log_format: None,
    };

    run_local_command(args).unwrap();

    let reduced = load_net_from_file(&output_path).unwrap();
    assert_eq!(reduced.count_live_agents(), 0);

    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&output_path);
}

#[test]
fn test_local_mode_nonexistent_file() {
    let args = LocalArgs {
        workers: 1,
        input: PathBuf::from("nonexistent_xyzzy.bin"),
        max_rounds: None,
        output: None,
        metrics: None,
        strategy: "round-robin".to_string(),
        log_format: None,
    };

    let result = run_local_command(args);
    assert!(result.is_err());
}

#[test]
fn test_local_mode_csv_metrics() {
    let net = make_annihilation_net();
    let input_path = temp_path("local_csv_input.bin");
    let metrics_path = temp_path("local_metrics.csv");

    save_net_to_file(&net, &input_path).unwrap();

    let args = LocalArgs {
        workers: 2,
        input: input_path.clone(),
        max_rounds: None,
        output: None,
        metrics: Some(metrics_path.clone()),
        strategy: "round-robin".to_string(),
        log_format: None,
    };

    run_local_command(args).unwrap();

    let csv = std::fs::read_to_string(&metrics_path).unwrap();
    assert!(
        csv.starts_with("round,agents,"),
        "CSV should start with header"
    );

    let _ = std::fs::remove_file(&input_path);
    let _ = std::fs::remove_file(&metrics_path);
}
