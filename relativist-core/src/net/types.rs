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
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub enum PortRef {
    /// Port of an agent: `(agent_id, port_id)`.
    /// Port 0 = principal, 1 = left auxiliary, 2 = right auxiliary.
    AgentPort(AgentId, PortId),
    /// Free port identified by a unique integer index.
    /// Used for Lafont interface ports and boundary sentinels during partitioning.
    /// `FreePort(u32::MAX)` is reserved as the `DISCONNECTED` sentinel (see TASK-0007).
    FreePort(u32),
}

// PortRef serde encoding (SPEC-18 §3.2 / §4.3 — TASK-0344).
//
// PortRef is the most-frequent value on the wire (every port slot in every
// partition). The default bincode v2 enum encoding is `varint(discriminant)
// + payload`, which costs 6 bytes for the DISCONNECTED sentinel
// (`FreePort(u32::MAX)`) — the single hottest path in any partition.
//
// We override Serialize/Deserialize with a manual tagged-bytes encoding:
//
//   0xFF                    -> DISCONNECTED (1 byte)
//   0x00 + varint(id) + pid -> AgentPort(id, pid)  (3-7 bytes)
//   0x01 + varint(border)   -> FreePort(border)    (2-6 bytes; border != u32::MAX)
//
// The trick: bincode v2's `serialize_tuple(N)` does NOT prefix the byte
// stream with N — it just expects N elements to follow. So we model each
// PortRef variant as a tuple whose element count matches the wire shape.
// Deserialization peeks the tag byte and reads only the trailing elements
// the variant actually uses.
//
// NOTE: this encoding is wire-only. PortRef under serde_json now produces
// a JSON array of bytes, not a tagged map. No production code path uses
// JSON for PortRef (verified during TASK-0344).

impl serde::Serialize for PortRef {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeTuple;
        match *self {
            PortRef::FreePort(bid) if bid == u32::MAX => {
                // DISCONNECTED — single tag byte, exactly 1 wire byte.
                let mut t = ser.serialize_tuple(1)?;
                t.serialize_element(&PORTREF_TAG_DISCONNECTED)?;
                t.end()
            }
            PortRef::AgentPort(id, pid) => {
                // tag (u8) + id (varint u32) + pid (u8) = 3 elements
                let mut t = ser.serialize_tuple(3)?;
                t.serialize_element(&PORTREF_TAG_AGENT_PORT)?;
                t.serialize_element(&id)?;
                t.serialize_element(&pid)?;
                t.end()
            }
            PortRef::FreePort(bid) => {
                // tag (u8) + bid (varint u32) = 2 elements
                let mut t = ser.serialize_tuple(2)?;
                t.serialize_element(&PORTREF_TAG_FREE_PORT)?;
                t.serialize_element(&bid)?;
                t.end()
            }
        }
    }
}

impl<'de> serde::Deserialize<'de> for PortRef {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        // We request the maximum element count (3 for AgentPort) and let the
        // visitor consume only what the discovered tag requires. Bincode v2
        // does not pre-read the elements — `next_element` pulls bytes lazily.
        de.deserialize_tuple(3, PortRefVisitor)
    }
}

/// Tag byte for the `DISCONNECTED` (`FreePort(u32::MAX)`) sentinel.
pub const PORTREF_TAG_DISCONNECTED: u8 = 0xFF;
/// Tag byte for the `AgentPort(id, pid)` variant.
pub const PORTREF_TAG_AGENT_PORT: u8 = 0x00;
/// Tag byte for the `FreePort(bid)` variant where `bid != u32::MAX`.
pub const PORTREF_TAG_FREE_PORT: u8 = 0x01;

struct PortRefVisitor;

impl<'de> serde::de::Visitor<'de> for PortRefVisitor {
    type Value = PortRef;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "compact PortRef wire bytes (SPEC-18 §4.3)")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<PortRef, A::Error> {
        use serde::de::Error as _;
        let tag: u8 = seq
            .next_element()?
            .ok_or_else(|| A::Error::custom("PortRef: missing tag byte"))?;
        match tag {
            PORTREF_TAG_DISCONNECTED => Ok(PortRef::FreePort(u32::MAX)),
            PORTREF_TAG_AGENT_PORT => {
                let id: AgentId = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("PortRef::AgentPort: missing id"))?;
                let pid: PortId = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("PortRef::AgentPort: missing pid"))?;
                Ok(PortRef::AgentPort(id, pid))
            }
            PORTREF_TAG_FREE_PORT => {
                let bid: u32 = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("PortRef::FreePort: missing border id"))?;
                Ok(PortRef::FreePort(bid))
            }
            other => Err(A::Error::custom(format!(
                "PortRef: unknown tag byte 0x{:02X}",
                other
            ))),
        }
    }
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
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct Agent {
    /// The symbol determines arity and interaction rules.
    pub symbol: Symbol,
    /// Unique identifier, monotonically increasing, never reused (SPEC-01, I3).
    pub id: AgentId,
}

/// Number of port slots per agent in the port array.
///
/// Every agent occupies exactly 3 slots: 1 principal + 2 auxiliary.
/// ERA agents waste 2 slots (they have only 1 port), but uniform
/// indexing simplifies the port array layout to O(1) lookups.
pub const PORTS_PER_SLOT: usize = 3;

/// Computes the flat index in the port array for `(agent_id, port_id)`.
///
/// The port array is a `Vec<PortRef>` where slot `agent_id * 3 + port_id`
/// stores the `PortRef` to which port `(agent_id, port_id)` is connected.
#[inline]
pub fn port_index(agent_id: AgentId, port_id: PortId) -> usize {
    (agent_id as usize) * PORTS_PER_SLOT + (port_id as usize)
}

/// Sentinel for disconnected or invalid port slots.
///
/// Used internally during reduction operations as a temporary marker.
/// A slot containing `DISCONNECTED` **violates invariant T1** (linearity)
/// if it persists after a reduction rule completes. Debug assertions
/// check for this (see `debug.rs`).
pub const DISCONNECTED: PortRef = PortRef::FreePort(u32::MAX);

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

/// Maps `FreePort(bid)` identifiers to the `AgentPort` they are connected to.
///
/// Used by the partitioner (SPEC-04) and merger (SPEC-05) to resolve
/// boundary connections. `FreePort` references have no slot in the port
/// array, so the `BorderMap` provides the reverse lookup.
///
/// Maintained externally to the `Net` struct — it is NOT a field of `Net`.
pub type BorderMap = std::collections::HashMap<u32, PortRef>;

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
    #[allow(clippy::clone_on_copy)]
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
            let bytes = crate::protocol::bincode_v2::encode(&sym).unwrap();
            let deserialized: Symbol = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
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
            let bytes = crate::protocol::bincode_v2::encode(&pr).unwrap();
            let des: PortRef = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
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

    // -----------------------------------------------------------------------
    // TASK-0344 — compact PortRef wire encoding (SPEC-18 §3.2 / §4.3)
    // -----------------------------------------------------------------------

    // R1: round-trip identity for every variant in the SPEC-18 §7.1 T2 list.
    #[test]
    fn test_portref_compact_round_trip_t2_set() {
        let cases = [
            PortRef::AgentPort(0, 0),
            PortRef::AgentPort(16383, 2),
            PortRef::AgentPort(16384, 1),
            PortRef::AgentPort(u32::MAX - 1, 2),
            PortRef::FreePort(0),
            PortRef::FreePort(1000),
            PortRef::FreePort(u32::MAX), // DISCONNECTED sentinel
        ];
        for p in cases {
            let bytes = crate::protocol::bincode_v2::encode(&p).unwrap();
            let (back, n): (PortRef, usize) = crate::protocol::bincode_v2::decode(&bytes).unwrap();
            assert_eq!(n, bytes.len(), "{:?}: bytes not fully consumed", p);
            assert_eq!(back, p, "{:?}: round-trip mismatch", p);
        }
    }

    // R2: DISCONNECTED collapses to exactly 1 wire byte (R8 hot path).
    #[test]
    fn test_portref_compact_disconnected_is_one_byte() {
        let bytes = crate::protocol::bincode_v2::encode(&PortRef::FreePort(u32::MAX)).unwrap();
        assert_eq!(bytes.len(), 1, "DISCONNECTED must collapse to 1 byte");
        assert_eq!(bytes[0], PORTREF_TAG_DISCONNECTED);
    }

    // R3: small AgentPort fits in <= 4 bytes.
    #[test]
    fn test_portref_compact_small_agent_port_le_four_bytes() {
        let bytes = crate::protocol::bincode_v2::encode(&PortRef::AgentPort(100, 0)).unwrap();
        assert!(
            bytes.len() <= 4,
            "AgentPort(100, 0) compact encoding was {} bytes (expected <= 4)",
            bytes.len(),
        );
    }

    // R4: AgentPort(0, 0) is exactly 3 bytes (tag + varint(0) + pid).
    #[test]
    fn test_portref_compact_zero_agent_port_is_three_bytes() {
        let bytes = crate::protocol::bincode_v2::encode(&PortRef::AgentPort(0, 0)).unwrap();
        assert_eq!(bytes.len(), 3, "AgentPort(0,0) tag+varint+pid = 3 bytes");
        assert_eq!(bytes[0], PORTREF_TAG_AGENT_PORT, "tag for AgentPort");
    }

    // R5: malformed payload — unknown tag is rejected.
    #[test]
    fn test_portref_compact_unknown_tag_is_error() {
        let res: Result<(PortRef, usize), _> =
            crate::protocol::bincode_v2::decode(&[0x42, 0x00, 0x00]);
        assert!(res.is_err(), "tag 0x42 must be rejected");
    }

    // R6: malformed payload — truncated stream is rejected.
    // 0x00 tag = AgentPort, but no following bytes for id/pid.
    #[test]
    fn test_portref_compact_truncated_payload_is_error() {
        let res: Result<(PortRef, usize), _> = crate::protocol::bincode_v2::decode(&[0x00]);
        assert!(
            res.is_err(),
            "AgentPort tag without id/pid must produce a serde error"
        );
    }

    // R7: composition with `CompactSubnet` — round-trip a Partition that
    // contains every PortRef variant inside a Net.
    // (Lives in `partition::compact` tests; this is a sanity-only smoke.)
    #[test]
    fn test_portref_compact_freeport_zero_two_bytes() {
        let bytes = crate::protocol::bincode_v2::encode(&PortRef::FreePort(0)).unwrap();
        // tag(1) + varint(0)(1) = 2 bytes
        assert_eq!(bytes.len(), 2, "FreePort(0) = tag + varint(0) = 2 bytes");
        assert_eq!(bytes[0], PORTREF_TAG_FREE_PORT);
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
        let bytes = crate::protocol::bincode_v2::encode(&a).unwrap();
        let des: Agent = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
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

    // --- PORTS_PER_SLOT / port_index / DISCONNECTED tests (TASK-0007) ---

    // T1: PORTS_PER_SLOT value
    #[test]
    fn test_ports_per_slot() {
        assert_eq!(PORTS_PER_SLOT, 3);
    }

    // T2: port_index correctness
    #[test]
    fn test_port_index() {
        assert_eq!(port_index(0, 0), 0);
        assert_eq!(port_index(0, 1), 1);
        assert_eq!(port_index(0, 2), 2);
        assert_eq!(port_index(1, 0), 3);
        assert_eq!(port_index(1, 1), 4);
        assert_eq!(port_index(5, 1), 16);
        assert_eq!(port_index(1000, 0), 3000);
    }

    // T3: DISCONNECTED is FreePort(u32::MAX)
    #[test]
    fn test_disconnected_value() {
        assert_eq!(DISCONNECTED, PortRef::FreePort(u32::MAX));
    }

    // T4: DISCONNECTED distinguishable from valid AgentPort
    #[test]
    fn test_disconnected_not_agent_port() {
        assert_ne!(DISCONNECTED, PortRef::AgentPort(0, 0));
        assert_ne!(DISCONNECTED, PortRef::AgentPort(u32::MAX, 0));
    }

    // -----------------------------------------------------------------------
    // TASK-0353 — rkyv round-trip tests for hot-path types (SPEC-18 §3.5).
    //
    // The `zero-copy` feature derives `rkyv::{Archive, Serialize, Deserialize}`
    // on Symbol, PortRef, Agent (this file), Net, IdRange, Partition,
    // CompactSubnet, and WorkerRoundStats. These tests confirm each type
    // round-trips byte-for-byte through `rkyv::to_bytes` -> `access`
    // -> `deserialize` (the validating API mandated by R24 step 3).
    //
    // The bincode v2 path MUST remain unaffected by the rkyv derives —
    // see `serde_bincode_v2_path_unaffected_by_rkyv_derives` in
    // `protocol::frame` for the cross-build coexistence guarantee.
    // -----------------------------------------------------------------------

    /// UT-0353-01: Symbol round-trips for every variant.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_symbol_exhaustive() {
        for sym in [Symbol::Con, Symbol::Dup, Symbol::Era] {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&sym).expect("serialize");
            let archived =
                rkyv::access::<rkyv::Archived<Symbol>, rkyv::rancor::Error>(bytes.as_ref())
                    .expect("access");
            let back: Symbol =
                rkyv::deserialize::<Symbol, rkyv::rancor::Error>(archived).expect("deserialize");
            assert_eq!(sym, back, "Symbol::{:?} did not round-trip", sym);
        }
    }

    /// UT-0353-02: Agent round-trips with each Symbol value.
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_agent_each_symbol() {
        for (sym, id) in [
            (Symbol::Con, 0u32),
            (Symbol::Dup, 1_234u32),
            (Symbol::Era, u32::MAX),
        ] {
            let agent = Agent { symbol: sym, id };
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&agent).expect("serialize");
            let archived =
                rkyv::access::<rkyv::Archived<Agent>, rkyv::rancor::Error>(bytes.as_ref())
                    .expect("access");
            let back: Agent =
                rkyv::deserialize::<Agent, rkyv::rancor::Error>(archived).expect("deserialize");
            assert_eq!(
                agent, back,
                "Agent {{ {:?}, {} }} did not round-trip",
                sym, id
            );
        }
    }

    /// UT-0353-03: PortRef round-trips both variants and the DISCONNECTED
    /// sentinel (which is `FreePort(u32::MAX)`).
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_portref_all_variants() {
        let cases = [
            PortRef::AgentPort(0, 0),
            PortRef::AgentPort(42, 1),
            PortRef::AgentPort(u32::MAX - 1, 2),
            PortRef::FreePort(0),
            PortRef::FreePort(1_000),
            PortRef::FreePort(u32::MAX), // DISCONNECTED sentinel
        ];
        for p in cases {
            let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&p).expect("serialize");
            let archived =
                rkyv::access::<rkyv::Archived<PortRef>, rkyv::rancor::Error>(bytes.as_ref())
                    .expect("access");
            let back: PortRef =
                rkyv::deserialize::<PortRef, rkyv::rancor::Error>(archived).expect("deserialize");
            assert_eq!(p, back, "PortRef {:?} did not round-trip", p);
        }
    }
}
