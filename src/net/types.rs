//! Fundamental types for Interaction Combinator networks.
//!
//! Defines Symbol, AgentId, PortId, PortRef, Agent, and related
//! constants. These are the building blocks of the Net struct.

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
}
