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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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
}
