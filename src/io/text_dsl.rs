//! Text DSL parser and serializer for .ic format (SPEC-12 R6-R15).
//!
//! Grammar (pseudo-BNF):
//!   file        ::= line*
//!   line        ::= comment | agent_decl | wire_decl | blank
//!   comment     ::= '#' ...
//!   agent_decl  ::= 'agent' IDENT SYMBOL
//!   wire_decl   ::= 'wire' port_ref port_ref
//!   port_ref    ::= IDENT '.' PORT_NAME | 'free(' INT ')'
//!   PORT_NAME   ::= 'principal' | 'left' | 'right' | 'p0' | 'p1' | 'p2'
//!   SYMBOL      ::= 'CON' | 'DUP' | 'ERA'

use std::collections::HashMap;
use std::path::Path;

use crate::error::RelativistError;
use crate::net::{AgentId, Net, PortRef, Symbol};

// ---------------------------------------------------------------------------
// Parser (TASK-0164, TASK-0165)
// ---------------------------------------------------------------------------

/// Parse a .ic text file into a Net.
pub fn load_ic(path: &Path) -> Result<Net, RelativistError> {
    let text = std::fs::read_to_string(path)?;
    parse_ic(&text).map_err(|e| {
        RelativistError::Config(format!("parse error in {:?}: {}", path, e))
    })
}

/// Parse IC text DSL string into a Net (SPEC-12 R7-R11).
///
/// Two-pass: Pass 1 collects agent declarations, Pass 2 processes wires.
pub fn parse_ic(input: &str) -> Result<Net, String> {
    let mut net = Net::new();
    let mut name_to_id: HashMap<String, AgentId> = HashMap::new();

    // Pass 1: collect agent declarations
    for (line_num, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        if tokens[0] == "agent" {
            if tokens.len() != 3 {
                return Err(format!(
                    "line {}: 'agent' requires name and symbol (e.g., 'agent a CON')",
                    line_num + 1
                ));
            }
            let name = tokens[1].to_string();
            let symbol = parse_symbol(tokens[2], line_num + 1)?;
            if name_to_id.contains_key(&name) {
                return Err(format!("line {}: duplicate agent name '{}'", line_num + 1, name));
            }
            let id = net.create_agent(symbol);
            name_to_id.insert(name, id);
        }
    }

    // Pass 2: process wire and root declarations
    let mut root_set = false;
    for (line_num, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }
        match tokens[0] {
            "agent" => {} // Already processed
            "wire" => {
                if tokens.len() != 3 {
                    return Err(format!(
                        "line {}: 'wire' requires two port references",
                        line_num + 1
                    ));
                }
                let port_a = parse_port_ref(tokens[1], &name_to_id, &net, line_num + 1)?;
                let port_b = parse_port_ref(tokens[2], &name_to_id, &net, line_num + 1)?;

                // R58: Reject self-loop wires (port connected to itself)
                if port_a == port_b {
                    return Err(format!(
                        "port cannot be connected to itself at line {}",
                        line_num + 1
                    ));
                }

                // R59: Reject free-to-free wires
                if matches!(port_a, PortRef::FreePort(_))
                    && matches!(port_b, PortRef::FreePort(_))
                {
                    return Err(format!(
                        "free-to-free wires are not supported; at least one endpoint must be an agent port, at line {}",
                        line_num + 1
                    ));
                }

                net.connect(port_a, port_b);
            }
            "root" => {
                // R54: At most one root declaration
                if root_set {
                    return Err(format!(
                        "duplicate root declaration at line {}",
                        line_num + 1
                    ));
                }
                if tokens.len() != 2 {
                    return Err(format!(
                        "line {}: 'root' requires exactly one port reference",
                        line_num + 1
                    ));
                }
                let port_ref = parse_port_ref(tokens[1], &name_to_id, &net, line_num + 1)?;
                // R56: root port must refer to a valid port (already validated by parse_port_ref)
                net.root = Some(port_ref);
                root_set = true;
            }
            other => {
                return Err(format!(
                    "line {}: unknown keyword '{}' (expected 'agent', 'wire', or 'root')",
                    line_num + 1, other
                ));
            }
        }
    }

    // R55: If no root declaration is present, net.root remains None (default)

    Ok(net)
}

fn parse_symbol(s: &str, line: usize) -> Result<Symbol, String> {
    match s {
        "CON" => Ok(Symbol::Con),
        "DUP" => Ok(Symbol::Dup),
        "ERA" => Ok(Symbol::Era),
        other => Err(format!(
            "line {}: unknown symbol '{}' (expected CON, DUP, or ERA)",
            line, other
        )),
    }
}

fn parse_port_ref(
    s: &str,
    names: &HashMap<String, AgentId>,
    net: &Net,
    line: usize,
) -> Result<PortRef, String> {
    // Check for free(N) syntax
    if let Some(inner) = s.strip_prefix("free(").and_then(|s| s.strip_suffix(')')) {
        let id: u32 = inner.parse().map_err(|_| {
            format!("line {}: invalid free port id '{}'", line, inner)
        })?;
        return Ok(PortRef::FreePort(id));
    }

    // agent.port syntax
    let parts: Vec<&str> = s.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(format!(
            "line {}: invalid port ref '{}' (expected 'name.port' or 'free(N)')",
            line, s
        ));
    }

    let agent_name = parts[0];
    let port_name = parts[1];

    let &agent_id = names.get(agent_name).ok_or_else(|| {
        format!("line {}: unknown agent '{}'", line, agent_name)
    })?;

    let port_id = parse_port_name(port_name, line)?;

    // Validate ERA agents don't have auxiliary ports (SPEC-12 R9)
    if let Some(agent) = net.get_agent(agent_id) {
        if agent.symbol == Symbol::Era && port_id > 0 {
            return Err(format!(
                "line {}: ERA agent '{}' has no auxiliary ports",
                line, agent_name
            ));
        }
    }

    Ok(PortRef::AgentPort(agent_id, port_id))
}

fn parse_port_name(name: &str, line: usize) -> Result<u8, String> {
    match name {
        "principal" | "p0" => Ok(0),
        "left" | "p1" => Ok(1),
        "right" | "p2" => Ok(2),
        other => Err(format!(
            "line {}: unknown port name '{}' (expected principal/left/right/p0/p1/p2)",
            line, other
        )),
    }
}

// ---------------------------------------------------------------------------
// Serializer (TASK-0166)
// ---------------------------------------------------------------------------

/// Serialize a Net to .ic text DSL format (SPEC-12 R15).
pub fn format_ic(net: &Net) -> String {
    let mut out = String::new();

    // Agent declarations
    for agent in net.live_agents() {
        let sym = match agent.symbol {
            Symbol::Con => "CON",
            Symbol::Dup => "DUP",
            Symbol::Era => "ERA",
        };
        out.push_str(&format!("agent a{} {}\n", agent.id, sym));
    }

    // Wire declarations — emit each pair once (lower index first)
    let mut emitted = std::collections::HashSet::new();
    for agent in net.live_agents() {
        let arity = crate::net::arity(agent.symbol);
        for port in 0..=arity {
            let src = PortRef::AgentPort(agent.id, port);
            let src_idx = agent.id as usize * 3 + port as usize;
            if src_idx >= net.ports.len() {
                continue;
            }
            let target = &net.ports[src_idx];
            let target_idx = match target {
                PortRef::AgentPort(id, p) => *id as usize * 3 + *p as usize,
                PortRef::FreePort(_) => usize::MAX,
            };

            let key = if src_idx < target_idx {
                (src_idx, target_idx)
            } else {
                (target_idx, src_idx)
            };
            if emitted.contains(&key) {
                continue;
            }
            emitted.insert(key);

            let src_str = format_port_ref(&src);
            let tgt_str = format_port_ref(target);
            out.push_str(&format!("wire {} {}\n", src_str, tgt_str));
        }
    }

    // Root declaration (R54-R57)
    if let Some(ref root_ref) = net.root {
        out.push_str(&format!("root {}\n", format_port_ref(root_ref)));
    }

    out
}

/// Save a Net to a .ic text file.
pub fn save_ic(net: &Net, path: &Path) -> Result<(), RelativistError> {
    let text = format_ic(net);
    std::fs::write(path, text)?;
    Ok(())
}

fn format_port_ref(pr: &PortRef) -> String {
    match pr {
        PortRef::AgentPort(id, port) => {
            let port_name = match port {
                0 => "principal",
                1 => "left",
                2 => "right",
                _ => "p?",
            };
            format!("a{}.{}", id, port_name)
        }
        PortRef::FreePort(id) => format!("free({})", id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};

    #[test]
    fn test_parse_empty() {
        let net = parse_ic("").unwrap();
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_parse_comments_and_blanks() {
        let net = parse_ic("# comment\n\n# another comment\n").unwrap();
        assert_eq!(net.count_live_agents(), 0);
    }

    #[test]
    fn test_parse_single_agent() {
        let net = parse_ic("agent x CON").unwrap();
        assert_eq!(net.count_live_agents(), 1);
    }

    #[test]
    fn test_parse_con_con_annihilation() {
        let input = "\
agent a CON
agent b CON
wire a.principal b.principal
wire a.left b.left
wire a.right b.right
";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.count_live_agents(), 2);
        assert_eq!(net.redex_queue.len(), 1);
    }

    #[test]
    fn test_parse_free_ports() {
        let input = "\
agent c CON
agent d DUP
wire c.principal d.principal
wire c.left free(0)
wire c.right free(1)
wire d.left free(2)
wire d.right free(3)
";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.count_live_agents(), 2);
    }

    #[test]
    fn test_parse_era_no_aux() {
        let input = "\
agent e ERA
agent f ERA
wire e.principal f.principal
";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.count_live_agents(), 2);
    }

    #[test]
    fn test_parse_era_aux_rejected() {
        let input = "agent e ERA\nwire e.left free(0)\n";
        let result = parse_ic(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no auxiliary ports"));
    }

    #[test]
    fn test_parse_duplicate_name_rejected() {
        let input = "agent x CON\nagent x DUP\n";
        let result = parse_ic(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("duplicate"));
    }

    #[test]
    fn test_parse_unknown_symbol_rejected() {
        let result = parse_ic("agent x FOO");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unknown_agent_rejected() {
        let result = parse_ic("agent a CON\nwire a.principal b.principal\n");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown agent"));
    }

    #[test]
    fn test_parse_p_aliases() {
        let input = "\
agent a CON
agent b CON
wire a.p0 b.p0
wire a.p1 b.p1
wire a.p2 b.p2
";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.count_live_agents(), 2);
    }

    // R58: Self-loop wire rejected
    #[test]
    fn test_parse_self_loop_rejected() {
        let input = "agent a CON\nwire a.left a.left\n";
        let result = parse_ic(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("port cannot be connected to itself"));
    }

    // R59: Free-to-free wire rejected
    #[test]
    fn test_parse_free_to_free_rejected() {
        let input = "agent a CON\nwire free(0) free(1)\n";
        let result = parse_ic(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("free-to-free wires are not supported"));
    }

    // R54: Root declaration support
    #[test]
    fn test_parse_root_declaration() {
        let input = "\
agent a CON
wire a.left free(0)
wire a.right free(1)
root a.principal
";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.root, Some(PortRef::AgentPort(0, 0)));
    }

    // R54: Duplicate root rejected
    #[test]
    fn test_parse_duplicate_root_rejected() {
        let input = "\
agent a CON
root a.principal
root a.left
";
        let result = parse_ic(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("duplicate root declaration"));
    }

    // R55: No root declaration -> net.root is None
    #[test]
    fn test_parse_no_root_is_none() {
        let input = "agent a CON\n";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.root, None);
    }

    // R56: Root with free port reference
    #[test]
    fn test_parse_root_free_port() {
        let input = "root free(0)\n";
        let net = parse_ic(input).unwrap();
        assert_eq!(net.root, Some(PortRef::FreePort(0)));
    }

    #[test]
    fn test_format_empty_net() {
        let net = Net::new();
        let text = format_ic(&net);
        assert!(text.is_empty());
    }

    #[test]
    fn test_format_emits_root() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
        net.root = Some(PortRef::AgentPort(a, 0));
        let text = format_ic(&net);
        assert!(text.contains("root a0.principal"));
    }

    #[test]
    fn test_format_roundtrip() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let text = format_ic(&net);
        assert!(text.contains("agent a0 CON"));
        assert!(text.contains("agent a1 ERA"));
        assert!(text.contains("wire"));

        // Re-parse should succeed
        let reparsed = parse_ic(&text).unwrap();
        assert_eq!(reparsed.count_live_agents(), 2);
    }
}
