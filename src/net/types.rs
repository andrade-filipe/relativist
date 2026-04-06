//! Fundamental types for Interaction Combinator networks.
//!
//! Defines Symbol, AgentId, PortId, PortRef, Agent, and related
//! constants. These are the building blocks of the Net struct.

/// Unique identifier for an agent in the net.
///
/// Monotonically increasing, never reused within an execution (SPEC-01, I3).
/// `u32` allows ~4 billion agents over the lifetime of a single reduction,
/// which is sufficient for the TCC's experimental workloads.
pub type AgentId = u32;

/// Port index within an agent: 0 = principal, 1 = left auxiliary, 2 = right auxiliary.
///
/// Values beyond 2 are invalid for any Interaction Combinator agent.
/// CON and DUP have 3 ports (0, 1, 2); ERA has only port 0 (principal).
/// Validation is done at the call site via `arity()`, not in the type itself.
pub type PortId = u8;

/// The 3 universal symbols of Lafont's Interaction Combinators (REF-002, p.71-72).
///
/// Every agent in an interaction net has exactly one symbol, which determines
/// its arity (number of auxiliary ports) and which interaction rule applies
/// when two agents form an active pair via their principal ports.
///
/// - **Con** (γ, constructor): arity 2. The "glue" that builds structure.
/// - **Dup** (δ, duplicator): arity 2. Copies structure when it commutes with Con.
/// - **Era** (ε, eraser): arity 0. Destroys structure on contact.
///
/// These 3 symbols with their 6 interaction rules form a universal basis:
/// any interaction net system can be encoded using only Con, Dup, and Era
/// (Lafont 1997, Theorem 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Symbol {
    /// γ (gamma) — Constructor. Arity 2 (principal + 2 auxiliary ports).
    Con = 0,
    /// δ (delta) — Duplicator. Arity 2 (principal + 2 auxiliary ports).
    Dup = 1,
    /// ε (epsilon) — Eraser. Arity 0 (principal port only, no auxiliary ports).
    Era = 2,
}

/// A reference to a specific port in the net.
///
/// This is the most-used type in Relativist — it appears in the port array,
/// in connect/disconnect arguments, in debug assertions, and in serialized messages.
///
/// **`AgentPort(id, port)`**: a port belonging to agent `id` at slot `port`.
/// Port 0 is the principal port; ports 1 and 2 are auxiliary (left and right).
///
/// **`FreePort(index)`**: serves dual purpose (SPEC-00 Sections 6.1, 6.2):
/// - *Lafont free ports*: the external interface of the net (inputs/outputs).
/// - *Boundary free ports*: synthetic markers inserted by the partitioner,
///   carrying a `borderId` for merge resolution (DISC-004 v2 Section 1.4).
///
/// Both are structurally identical at the type level; the distinction is
/// semantic and resolved by context (the border map in SPEC-04/SPEC-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PortRef {
    /// Port of an agent: `(agent_id, port_id)`.
    /// Port 0 = principal, 1 = left auxiliary, 2 = right auxiliary.
    AgentPort(AgentId, PortId),
    /// Free port identified by a unique integer index.
    /// Used for Lafont interface ports and boundary sentinels during partitioning.
    /// `FreePort(u32::MAX)` is reserved as the `DISCONNECTED` sentinel (see TASK-0007).
    FreePort(u32),
}

/// An agent (node) in the interaction net.
///
/// Each agent has a symbol (which determines its arity and applicable
/// interaction rules) and a unique ID. Agents do NOT store their port
/// connections directly — connections live in the Net's flat port array,
/// indexed by `(agent_id * PORTS_PER_SLOT + port_id)`.
///
/// This separation simplifies removal: marking a slot as `None` in the
/// agent arena is O(1), without updating a complex adjacency structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Agent {
    /// The symbol determines arity and interaction rules.
    pub symbol: Symbol,
    /// Unique identifier, monotonically increasing, never reused (SPEC-01, I3).
    pub id: AgentId,
}

/// Returns the arity (number of auxiliary ports) of a symbol.
///
/// - CON (γ): 2 auxiliary ports (left, right) — REF-002 p.71
/// - DUP (δ): 2 auxiliary ports (left, right) — REF-002 p.71
/// - ERA (ε): 0 auxiliary ports — REF-002 p.72
///
/// Every agent also has a principal port (port 0), not counted in arity.
pub const fn arity(symbol: Symbol) -> u8 {
    match symbol {
        Symbol::Con => 2,
        Symbol::Dup => 2,
        Symbol::Era => 0,
    }
}

/// Returns the total number of ports of a symbol: `arity + 1`.
///
/// Includes the principal port (port 0). Used for port array indexing
/// and iteration over all ports of an agent.
/// - CON/DUP: 3 ports (principal + 2 auxiliary)
/// - ERA: 1 port (principal only)
pub const fn total_ports(symbol: Symbol) -> u8 {
    arity(symbol) + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    // T1: Symbol has exactly 3 variants (exhaustive match compiles)
    #[test]
    fn test_symbol_exhaustive_match() {
        fn name(s: Symbol) -> &'static str {
            match s {
                Symbol::Con => "constructor",
                Symbol::Dup => "duplicator",
                Symbol::Era => "eraser",
            }
        }
        assert_eq!(name(Symbol::Con), "constructor");
        assert_eq!(name(Symbol::Dup), "duplicator");
        assert_eq!(name(Symbol::Era), "eraser");
    }

    // T2: Discriminant values match repr(u8)
    #[test]
    fn test_symbol_discriminants() {
        assert_eq!(Symbol::Con as u8, 0);
        assert_eq!(Symbol::Dup as u8, 1);
        assert_eq!(Symbol::Era as u8, 2);
    }

    // T3: Symbol implements Copy and Clone
    #[test]
    fn test_symbol_copy_clone() {
        let s = Symbol::Con;
        let s2 = s; // Copy
        let s3 = s.clone(); // Clone
        assert_eq!(s, s2);
        assert_eq!(s, s3);
    }

    // T4: Symbol implements PartialEq and Eq
    #[test]
    fn test_symbol_equality() {
        assert_eq!(Symbol::Con, Symbol::Con);
        assert_eq!(Symbol::Dup, Symbol::Dup);
        assert_eq!(Symbol::Era, Symbol::Era);
        assert_ne!(Symbol::Con, Symbol::Dup);
        assert_ne!(Symbol::Con, Symbol::Era);
        assert_ne!(Symbol::Dup, Symbol::Era);
    }

    // T5: Symbol implements Hash (usable in HashMap)
    #[test]
    fn test_symbol_hash() {
        let mut map = HashMap::new();
        map.insert(Symbol::Con, "constructor");
        map.insert(Symbol::Dup, "duplicator");
        map.insert(Symbol::Era, "eraser");
        assert_eq!(map.len(), 3);
        assert_eq!(map[&Symbol::Con], "constructor");
    }

    // T6: Symbol serialization round-trip
    #[test]
    fn test_symbol_serde_roundtrip() {
        for sym in [Symbol::Con, Symbol::Dup, Symbol::Era] {
            let bytes = bincode::serialize(&sym).unwrap();
            let deserialized: Symbol = bincode::deserialize(&bytes).unwrap();
            assert_eq!(sym, deserialized);
        }
    }

    // T7: Symbol Debug formatting
    #[test]
    fn test_symbol_debug() {
        assert_eq!(format!("{:?}", Symbol::Con), "Con");
        assert_eq!(format!("{:?}", Symbol::Dup), "Dup");
        assert_eq!(format!("{:?}", Symbol::Era), "Era");
    }

    // E2: Size of Symbol is 1 byte
    #[test]
    fn test_symbol_size() {
        assert_eq!(std::mem::size_of::<Symbol>(), 1);
    }

    // --- AgentId tests (TASK-0003) ---

    // T1: AgentId is u32 (4 bytes)
    #[test]
    fn test_agent_id_is_u32() {
        let id: AgentId = 42u32;
        assert_eq!(std::mem::size_of::<AgentId>(), 4);
        assert_eq!(id, 42);
    }

    // T4: AgentId supports full u32 range
    #[test]
    fn test_agent_id_max_value() {
        let max: AgentId = u32::MAX;
        assert_eq!(max, 4_294_967_295);
    }

    // E1: AgentId arithmetic (important for next_id increment)
    #[test]
    fn test_agent_id_arithmetic() {
        let id: AgentId = 0;
        let next = id + 1;
        assert_eq!(next, 1);
    }

    // --- PortId tests (TASK-0003) ---

    // T2: PortId is u8 (1 byte)
    #[test]
    fn test_port_id_is_u8() {
        let p: PortId = 0u8;
        assert_eq!(std::mem::size_of::<PortId>(), 1);
        assert_eq!(p, 0);
    }

    // T3: PortId valid range (0, 1, 2)
    #[test]
    fn test_port_id_valid_range() {
        let principal: PortId = 0;
        let aux1: PortId = 1;
        let aux2: PortId = 2;
        assert_eq!(principal, 0);
        assert_eq!(aux1, 1);
        assert_eq!(aux2, 2);
    }

    // E2: PortId as array index
    #[test]
    fn test_port_id_as_index() {
        let ports = [10, 20, 30];
        let p: PortId = 1;
        assert_eq!(ports[p as usize], 20);
    }

    // --- PortRef tests (TASK-0004) ---

    // T1: Variant discrimination
    #[test]
    fn test_portref_variant_discrimination() {
        assert_ne!(PortRef::AgentPort(0, 0), PortRef::FreePort(0));
    }

    // T2: AgentPort equality
    #[test]
    fn test_portref_agent_port_equality() {
        assert_eq!(PortRef::AgentPort(5, 1), PortRef::AgentPort(5, 1));
        assert_ne!(PortRef::AgentPort(5, 1), PortRef::AgentPort(5, 2));
        assert_ne!(PortRef::AgentPort(5, 1), PortRef::AgentPort(6, 1));
    }

    // T3: FreePort equality
    #[test]
    fn test_portref_free_port_equality() {
        assert_eq!(PortRef::FreePort(42), PortRef::FreePort(42));
        assert_ne!(PortRef::FreePort(42), PortRef::FreePort(43));
    }

    // T4: Copy semantics
    #[test]
    fn test_portref_copy() {
        let p = PortRef::AgentPort(1, 0);
        let p2 = p; // Copy
        assert_eq!(p, p2); // original still usable after copy
    }

    // T5: Pattern matching extracts correct fields
    #[test]
    fn test_portref_pattern_matching() {
        match PortRef::AgentPort(7, 2) {
            PortRef::AgentPort(id, port) => {
                assert_eq!(id, 7);
                assert_eq!(port, 2);
            }
            _ => panic!("wrong variant"),
        }
        match PortRef::FreePort(99) {
            PortRef::FreePort(bid) => assert_eq!(bid, 99),
            _ => panic!("wrong variant"),
        }
    }

    // T6: Serde round-trip
    #[test]
    fn test_portref_serde_roundtrip() {
        for pr in [PortRef::AgentPort(100, 0), PortRef::FreePort(55)] {
            let bytes = bincode::serialize(&pr).unwrap();
            let des: PortRef = bincode::deserialize(&bytes).unwrap();
            assert_eq!(pr, des);
        }
    }

    // T7: Hash (usable in HashSet)
    #[test]
    fn test_portref_hash() {
        let mut set = HashSet::new();
        set.insert(PortRef::AgentPort(1, 0));
        set.insert(PortRef::FreePort(1));
        assert_eq!(set.len(), 2);
    }

    // T8: Debug formatting
    #[test]
    fn test_portref_debug() {
        assert!(format!("{:?}", PortRef::AgentPort(3, 1)).contains("AgentPort"));
        assert!(format!("{:?}", PortRef::FreePort(7)).contains("FreePort"));
    }

    // E1: FreePort(u32::MAX) is structurally valid
    #[test]
    fn test_portref_freeport_max() {
        let p = PortRef::FreePort(u32::MAX);
        assert_eq!(p, PortRef::FreePort(u32::MAX));
    }

    // --- Agent tests (TASK-0005) ---

    // T1: Agent construction and field access
    #[test]
    fn test_agent_construction() {
        let a = Agent {
            symbol: Symbol::Con,
            id: 42,
        };
        assert_eq!(a.symbol, Symbol::Con);
        assert_eq!(a.id, 42);
    }

    // T2: Agent equality (same symbol + id)
    #[test]
    fn test_agent_equality() {
        let a1 = Agent {
            symbol: Symbol::Dup,
            id: 7,
        };
        let a2 = Agent {
            symbol: Symbol::Dup,
            id: 7,
        };
        assert_eq!(a1, a2);
    }

    // T3: Agent inequality (different id)
    #[test]
    fn test_agent_inequality_id() {
        let a1 = Agent {
            symbol: Symbol::Con,
            id: 1,
        };
        let a2 = Agent {
            symbol: Symbol::Con,
            id: 2,
        };
        assert_ne!(a1, a2);
    }

    // T4: Agent inequality (different symbol)
    #[test]
    fn test_agent_inequality_symbol() {
        let a1 = Agent {
            symbol: Symbol::Con,
            id: 1,
        };
        let a2 = Agent {
            symbol: Symbol::Dup,
            id: 1,
        };
        assert_ne!(a1, a2);
    }

    // T5: Agent is Copy
    #[test]
    fn test_agent_copy() {
        let a = Agent {
            symbol: Symbol::Era,
            id: 0,
        };
        let b = a; // Copy
        assert_eq!(a, b); // original still usable
    }

    // T6: Agent serde round-trip
    #[test]
    fn test_agent_serde_roundtrip() {
        let a = Agent {
            symbol: Symbol::Con,
            id: 100,
        };
        let bytes = bincode::serialize(&a).unwrap();
        let des: Agent = bincode::deserialize(&bytes).unwrap();
        assert_eq!(a, des);
    }

    // T7: Agent size is compact (u8 symbol + padding + u32 id)
    #[test]
    fn test_agent_size() {
        // Symbol is 1 byte, AgentId is 4 bytes. With alignment, Agent is 8 bytes.
        assert!(std::mem::size_of::<Agent>() <= 8);
    }

    // E1: Agent with id 0
    #[test]
    fn test_agent_id_zero() {
        let a = Agent {
            symbol: Symbol::Era,
            id: 0,
        };
        assert_eq!(a.id, 0);
    }

    // E2: Agent with max id
    #[test]
    fn test_agent_max_id() {
        let a = Agent {
            symbol: Symbol::Con,
            id: u32::MAX,
        };
        assert_eq!(a.id, u32::MAX);
    }

    // --- arity / total_ports tests (TASK-0006) ---

    // T1: arity values
    #[test]
    fn test_arity() {
        assert_eq!(arity(Symbol::Con), 2);
        assert_eq!(arity(Symbol::Dup), 2);
        assert_eq!(arity(Symbol::Era), 0);
    }

    // T2: total_ports values
    #[test]
    fn test_total_ports() {
        assert_eq!(total_ports(Symbol::Con), 3);
        assert_eq!(total_ports(Symbol::Dup), 3);
        assert_eq!(total_ports(Symbol::Era), 1);
    }

    // T3: const fn usable at compile time
    #[test]
    fn test_arity_const() {
        const CON_ARITY: u8 = arity(Symbol::Con);
        const ERA_TOTAL: u8 = total_ports(Symbol::Era);
        assert_eq!(CON_ARITY, 2);
        assert_eq!(ERA_TOTAL, 1);
    }

    // E1: total_ports = arity + 1 identity for all symbols
    #[test]
    fn test_total_ports_identity() {
        for sym in [Symbol::Con, Symbol::Dup, Symbol::Era] {
            assert_eq!(total_ports(sym), arity(sym) + 1);
        }
    }
}
