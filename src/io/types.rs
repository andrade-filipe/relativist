//! I/O types: NetFormat, NetSummary, ReductionSummary (SPEC-12 R1, R29, R35).

use crate::net::Symbol;

/// Supported net file formats (SPEC-12 R1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum NetFormat {
    /// Binary (serde + bincode). Extension: .bin
    Bin,
    /// Text DSL. Extension: .ic
    Ic,
    /// JSON. Extension: .json
    Json,
}

/// Detect format from file extension.
pub fn detect_format(path: &std::path::Path) -> Option<NetFormat> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("bin") => Some(NetFormat::Bin),
        Some("ic") => Some(NetFormat::Ic),
        Some("json") => Some(NetFormat::Json),
        _ => None,
    }
}

/// Summary statistics of an IC net (SPEC-12 R29).
#[derive(Debug, Clone, serde::Serialize)]
pub struct NetSummary {
    pub agents: usize,
    pub wires: usize,
    pub redexes: usize,
    pub con: usize,
    pub dup: usize,
    pub era: usize,
    pub free_ports: usize,
    pub normal_form: bool,
}

/// Reduction summary for human-readable output (SPEC-12 R35).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ReductionSummary {
    pub agents_before: usize,
    pub agents_after: usize,
    pub redexes_before: usize,
    pub redexes_after: usize,
    pub normal_form: bool,
    pub total_interactions: u64,
    pub duration_secs: f64,
    pub mips: f64,
}

/// Compute a NetSummary from a Net (SPEC-12 R29, R61).
///
/// Iterates only over ports `0..=arity(agent.symbol)` for each live agent,
/// skipping unused port slots beyond the agent's arity (R61).
pub fn net_summary(net: &crate::net::Net) -> NetSummary {
    let agents = net.count_live_agents();
    let con = net
        .live_agents()
        .filter(|a| a.symbol == Symbol::Con)
        .count();
    let dup = net
        .live_agents()
        .filter(|a| a.symbol == Symbol::Dup)
        .count();
    let era = net
        .live_agents()
        .filter(|a| a.symbol == Symbol::Era)
        .count();
    let redexes = net.redex_queue.len();

    // Count distinct wires (AgentPort-AgentPort pairs) and free ports
    // R61: iterate only over ports 0..=arity(symbol) per live agent
    let mut wires = 0usize;
    let mut free_ports = 0usize;
    for agent in net.live_agents() {
        let arity = crate::net::arity(agent.symbol);
        for p in 0..=arity {
            let idx = agent.id as usize * 3 + p as usize;
            if idx >= net.ports.len() {
                continue;
            }
            match &net.ports[idx] {
                crate::net::PortRef::AgentPort(target_id, target_port) => {
                    // Count only once per pair: when our index < target index
                    let target_idx = *target_id as usize * 3 + *target_port as usize;
                    if idx < target_idx {
                        wires += 1;
                    }
                }
                crate::net::PortRef::FreePort(_) => {
                    free_ports += 1;
                }
            }
        }
    }

    NetSummary {
        agents,
        wires,
        redexes,
        con,
        dup,
        era,
        free_ports,
        normal_form: redexes == 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};
    use std::path::Path;

    #[test]
    fn test_detect_format_bin() {
        assert_eq!(detect_format(Path::new("net.bin")), Some(NetFormat::Bin));
    }

    #[test]
    fn test_detect_format_ic() {
        assert_eq!(detect_format(Path::new("net.ic")), Some(NetFormat::Ic));
    }

    #[test]
    fn test_detect_format_json() {
        assert_eq!(detect_format(Path::new("net.json")), Some(NetFormat::Json));
    }

    #[test]
    fn test_detect_format_unknown() {
        assert_eq!(detect_format(Path::new("net.xyz")), None);
        assert_eq!(detect_format(Path::new("net")), None);
    }

    #[test]
    fn test_net_summary_empty() {
        let net = Net::new();
        let summary = net_summary(&net);
        assert_eq!(summary.agents, 0);
        assert_eq!(summary.wires, 0);
        assert_eq!(summary.redexes, 0);
        assert!(summary.normal_form);
    }

    #[test]
    fn test_net_summary_with_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(2));
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(3));

        let summary = net_summary(&net);
        assert_eq!(summary.agents, 2);
        assert_eq!(summary.con, 1);
        assert_eq!(summary.dup, 1);
        assert_eq!(summary.era, 0);
        assert_eq!(summary.wires, 1); // Only principal-principal
        assert_eq!(summary.free_ports, 4);
        assert_eq!(summary.redexes, 1);
        assert!(!summary.normal_form);
    }

    #[test]
    fn test_net_summary_serializes_to_json() {
        let net = Net::new();
        let summary = net_summary(&net);
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"agents\":0"));
        assert!(json.contains("\"normal_form\":true"));
    }
}
