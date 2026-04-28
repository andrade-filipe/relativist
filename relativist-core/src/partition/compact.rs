//! Compact wire representation of a partition's `Net` (L6 mitigation).
//!
//! The in-memory `Net` layout is dense: `agents: Vec<Option<Agent>>` and
//! `ports: Vec<PortRef>` are sized to `max_id + 1` and `(max_id + 1) * 3`
//! respectively, even when most slots are `None` or `DISCONNECTED`. Under
//! `ContiguousIdStrategy`, the last worker's subnet always spans the full
//! `max_id + 1` range regardless of how many live agents it actually owns,
//! which pushes large partitions past the 256 MiB protocol frame cap (see
//! `PHASE2-FINDINGS.md` L6).
//!
//! `CompactSubnet` stores only live agents and non-`DISCONNECTED` ports,
//! along with the arena sizes needed to rebuild the original dense layout
//! on the receiving side. It is used via serde's `serialize_with` /
//! `deserialize_with` on the `Partition::subnet` field — in-memory the
//! subnet is still a `Net`; only the wire form is compressed.
//!
//! Round-trip invariant: `Net -> CompactSubnet -> Net` preserves
//! `agents`, `ports`, `redex_queue`, `next_id`, and `root` byte-for-byte.
//! `freeport_redirects` is `#[serde(skip)]` on `Net` and is not carried.

use std::collections::VecDeque;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::net::{port_index, Agent, AgentId, Net, PortRef, DISCONNECTED, PORTS_PER_SLOT};

/// Sparse wire-only representation of a partition sub-`Net`.
///
/// Per live agent we carry `(id, agent, [p0, p1, p2])`. The three-port array
/// is serialised verbatim — `DISCONNECTED` entries pay only the enum tag, so
/// densely connected agents cost no more than the dense encoding while the
/// whole triple disappears for tombstoned slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(
    feature = "zero-copy",
    derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)
)]
pub struct CompactSubnet {
    /// Target length of `Net::agents` after inflation (equals the original
    /// `agents.len()`). Stored explicitly so round-trip preserves the arena
    /// size, not just the live agent IDs.
    pub agent_arena_len: u32,

    /// Live slots, one entry per non-`None` agent:
    /// `(agent_id, agent, [port0, port1, port2])`. The receiver rebuilds
    /// the dense arenas by sizing to `agent_arena_len` and scattering each
    /// entry into its slot.
    pub live: Vec<(AgentId, Agent, [PortRef; PORTS_PER_SLOT])>,

    /// Full redex queue (small compared to agents/ports).
    pub redex_queue: VecDeque<(AgentId, AgentId)>,

    /// Next free agent id.
    pub next_id: AgentId,

    /// Optional root port.
    pub root: Option<PortRef>,
}

impl CompactSubnet {
    /// Builds a compact view of `net` by filtering out `None` agent slots.
    /// Live agents keep all three port entries inline; `DISCONNECTED` ports
    /// on a live agent are preserved verbatim so full round-trip is exact.
    pub fn from_net(net: &Net) -> Self {
        let arena_len = net.agents.len();

        let mut live: Vec<(AgentId, Agent, [PortRef; PORTS_PER_SLOT])> = Vec::new();
        for (idx, slot) in net.agents.iter().enumerate() {
            if let Some(agent) = slot {
                let base = port_index(idx as AgentId, 0);
                let ports = [net.ports[base], net.ports[base + 1], net.ports[base + 2]];
                live.push((idx as AgentId, *agent, ports));
            }
        }

        Self {
            agent_arena_len: arena_len as u32,
            live,
            redex_queue: net.redex_queue.clone(),
            next_id: net.next_id,
            root: net.root,
        }
    }

    /// Inflates back into a dense `Net`, re-creating `agents` and `ports`
    /// arenas sized to `agent_arena_len` and filled with `None` /
    /// `DISCONNECTED` sentinels before applying the live entries.
    pub fn into_net(self) -> Net {
        let arena_len = self.agent_arena_len as usize;
        let mut agents: Vec<Option<Agent>> = vec![None; arena_len];
        let mut ports: Vec<PortRef> = vec![DISCONNECTED; arena_len * PORTS_PER_SLOT];

        for (id, agent, slot_ports) in self.live {
            let idx = id as usize;
            if idx < agents.len() {
                agents[idx] = Some(agent);
                let base = port_index(id, 0);
                if base + PORTS_PER_SLOT <= ports.len() {
                    ports[base] = slot_ports[0];
                    ports[base + 1] = slot_ports[1];
                    ports[base + 2] = slot_ports[2];
                }
            }
        }

        Net {
            agents,
            ports,
            redex_queue: self.redex_queue,
            next_id: self.next_id,
            root: self.root,
            freeport_redirects: std::collections::HashMap::new(),
            free_list: Vec::new(),
            id_range: None,
            border_entries_shadow: None,
            recycle_policy: crate::net::core::RecyclePolicy::DisableUnderDelta,
            is_in_delta_round: false,
            #[cfg(debug_assertions)]
            protected_tombstones: None,
            #[cfg(debug_assertions)]
            free_list_pops: 0,
        }
    }
}

/// serde `serialize_with` adapter used on `Partition::subnet`.
pub fn serialize_subnet_compact<S: Serializer>(
    net: &Net,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    CompactSubnet::from_net(net).serialize(serializer)
}

/// serde `deserialize_with` adapter used on `Partition::subnet`.
pub fn deserialize_subnet_compact<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Net, D::Error> {
    let compact = CompactSubnet::deserialize(deserializer)?;
    Ok(compact.into_net())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};

    fn nets_equivalent(a: &Net, b: &Net) -> bool {
        a.agents == b.agents
            && a.ports == b.ports
            && a.redex_queue == b.redex_queue
            && a.next_id == b.next_id
            && a.root == b.root
    }

    // T1: Empty net round-trip preserves everything.
    #[test]
    fn test_empty_net_roundtrip() {
        let net = Net::new();
        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert!(nets_equivalent(&net, &back));
    }

    // T2: Single agent round-trip.
    #[test]
    fn test_single_agent_roundtrip() {
        let mut net = Net::new();
        let _ = net.create_agent(Symbol::Era);
        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert!(nets_equivalent(&net, &back));
    }

    // T3: Connected pair round-trip preserves port targets.
    #[test]
    fn test_connected_pair_roundtrip() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert!(nets_equivalent(&net, &back));
    }

    // T4: Tombstone slot (remove_agent leaves agents[id] = None) is preserved.
    #[test]
    fn test_tombstone_slot_roundtrip() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.remove_agent(a);

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();

        // arena length must be preserved so that later create_agent continues
        // to allocate at the same next_id.
        assert_eq!(net.agents.len(), back.agents.len());
        assert_eq!(net.next_id, back.next_id);
        assert!(back.agents[a as usize].is_none());
        assert_eq!(back.agents[b as usize].map(|ag| ag.id), Some(b));
    }

    // T5: Bincode round-trip via the serde adapters matches the direct conversion.
    #[test]
    fn test_bincode_roundtrip_via_adapters() {
        let mut net = Net::new();
        let c1 = net.create_agent(Symbol::Con);
        let c2 = net.create_agent(Symbol::Con);
        let d = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(c1, 0), PortRef::AgentPort(d, 0));
        net.connect(PortRef::AgentPort(c1, 1), PortRef::AgentPort(c2, 1));
        net.connect(PortRef::AgentPort(c1, 2), PortRef::FreePort(42));

        let compact = CompactSubnet::from_net(&net);
        let bytes = crate::protocol::bincode_v2::encode(&compact).unwrap();
        let restored: CompactSubnet = crate::protocol::bincode_v2::decode_value(&bytes).unwrap();
        let back = restored.into_net();

        assert!(nets_equivalent(&net, &back));
    }

    // T6: Redex queue is preserved.
    #[test]
    fn test_redex_queue_preserved() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        // Connecting two principal ports enqueues the pair.
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        assert!(!net.redex_queue.is_empty());

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert_eq!(net.redex_queue, back.redex_queue);
    }

    // T7: Root is preserved.
    #[test]
    fn test_root_preserved() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        net.root = Some(PortRef::AgentPort(a, 0));

        let compact = CompactSubnet::from_net(&net);
        let back = compact.into_net();
        assert_eq!(net.root, back.root);
    }

    // T8: Compact form is actually smaller for sparse arenas.
    #[test]
    fn test_compact_smaller_for_sparse() {
        // Create a net, grow the arena, then remove most agents to simulate
        // a last-worker subnet under ContiguousIdStrategy.
        let mut net = Net::new();
        let mut ids = Vec::new();
        for _ in 0..1000 {
            ids.push(net.create_agent(Symbol::Era));
        }
        // Keep only the last 10 live.
        for &id in &ids[..990] {
            net.remove_agent(id);
        }

        let dense_bytes = crate::protocol::bincode_v2::encode(&net).unwrap();
        let compact = CompactSubnet::from_net(&net);
        let compact_bytes = crate::protocol::bincode_v2::encode(&compact).unwrap();

        assert!(
            compact_bytes.len() * 3 < dense_bytes.len(),
            "compact ({}) should be <<< dense ({}) for sparse net",
            compact_bytes.len(),
            dense_bytes.len()
        );
    }

    // -----------------------------------------------------------------------
    // TASK-0353 — rkyv round-trip for CompactSubnet (SPEC-18 §3.5).
    //
    // CompactSubnet has no PartialEq derive — we compare via inflation
    // back to a Net (which DOES implement PartialEq) plus the structural
    // fields that survive the wire form (`agent_arena_len`, `next_id`,
    // `root`, `redex_queue`, and the `live` count).
    //
    // CompactSubnet is the bincode v2 wire form of Partition.subnet but
    // its rkyv derive is wired up so the type can be archived directly
    // by anything that needs to (no production caller does today; the
    // derive guards against future regressions and exercises the test
    // cube fully).
    // -----------------------------------------------------------------------

    /// UT-0353-07: CompactSubnet round-trips via rkyv. Equality is checked
    /// by inflating both sides back to Net (which implements PartialEq).
    #[cfg(feature = "zero-copy")]
    #[test]
    fn rkyv_round_trip_compact_subnet_minimal() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Era);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(42));
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(u32::MAX));

        let compact = CompactSubnet::from_net(&net);
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&compact).expect("serialize");
        let archived =
            rkyv::access::<rkyv::Archived<CompactSubnet>, rkyv::rancor::Error>(bytes.as_ref())
                .expect("access");
        let back: CompactSubnet =
            rkyv::deserialize::<CompactSubnet, rkyv::rancor::Error>(archived).expect("deserialize");

        assert_eq!(back.agent_arena_len, compact.agent_arena_len);
        assert_eq!(back.next_id, compact.next_id);
        assert_eq!(back.root, compact.root);
        assert_eq!(back.redex_queue, compact.redex_queue);
        assert_eq!(back.live.len(), compact.live.len());

        // Inflated nets must compare equal via the shared PartialEq impl.
        let original_inflated = compact.clone().into_net();
        let round_inflated = back.into_net();
        assert_eq!(
            original_inflated, round_inflated,
            "CompactSubnet -> rkyv -> CompactSubnet must inflate to an equal Net"
        );
    }
}
