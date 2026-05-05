# SPEC-23: Compact Memory Representation

**Status:** Draft
**Depends on:** SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-13 (System Architecture)
**Amends:** SPEC-02 (Net internal types), SPEC-22 (Arena Management)
**ROADMAP items:** 2.15 (Compact Memory Representation)
**References consumed:** REF-002 (Lafont 1997, p.70-73: net structure), AC-006 (HVM compact encoding), AC-015 CC-1 (bit-packed ports), AC-011 (per-worker arena slices)
**Briefings consumed:** BRIEF-20260415-v2-tier5-teorica (Section 2.15: invariant re-verification), BRIEF-20260415-v2-tier5-codebase (Section 2.15: 225 PortRef call sites, migration strategy)

---

## 1. Purpose

This spec defines the migration of Relativist's in-memory net representation from semantic Rust types (enum-based `PortRef`, struct-based `Agent`, `Vec<Option<Agent>>` arena) to a compact bit-packed encoding inspired by HVM2 (AC-006, AC-015). The goal is ~33% memory reduction per port reference (6 bytes → 4 bytes), improved cache locality, and faster serialization for the wire protocol, without changing the public API surface or violating any IC-theoretic invariant (T1-T7).

The migration is implemented through an accessor-based abstraction: all code that reads or writes `PortRef` and `Agent` values transitions to accessor methods, and then the internal representation changes from enum to bit-packed u32/u64 behind the same accessor interface.

---

## 2. Definitions

Terms defined in SPEC-00, SPEC-01, and SPEC-02 are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Compact PortRef** | A `u32` value encoding a port reference via bit-packing: `(value << TAG_BITS) \| tag`. Replaces the enum-based `PortRef` from SPEC-02. The accessor API (`agent_id()`, `port_id()`, `is_agent_port()`, `is_free_port()`) remains identical. |
| **Tag** | The low-order bits of a compact `PortRef` that distinguish the reference type. Minimum 2 bits (4 states: AgentPort-P0, AgentPort-P1, AgentPort-P2, FreePort). |
| **Compact Agent** | A `u64` value encoding an agent's metadata: symbol in high bits, agent ID in low bits. Replaces the struct-based `Agent` from SPEC-02. |
| **TAG_BITS** | The number of bits reserved for the tag in a compact `PortRef`. Constant, defined at compile time. |
| **AGENT_PORT_P0** | Tag value for a principal port (port_id = 0). |
| **AGENT_PORT_P1** | Tag value for a left auxiliary port (port_id = 1). |
| **AGENT_PORT_P2** | Tag value for a right auxiliary port (port_id = 2). |
| **FREE_PORT** | Tag value for a free port (Lafont interface or border sentinel). |
| **Accessor Method** | A method on `PortRef` or `Agent` that extracts semantic fields from the bit-packed value (e.g., `PortRef::agent_id() -> AgentId`). These methods are the abstraction boundary: all code outside `net/types.rs` uses accessors, never raw bit manipulation. |

---

## 3. Requirements

### 3.1 Compact PortRef

**R1.** `PortRef` MUST be represented as a newtype struct wrapping a single `u32` value: `pub struct PortRef(u32)`. The enum representation (`AgentPort(AgentId, PortId)` / `FreePort(u32)`) from SPEC-02 MUST be replaced. **(MUST)**

**R2.** The `u32` encoding MUST use bit-packing with a fixed tag in the low-order bits: `(value << TAG_BITS) | tag`. `TAG_BITS` MUST be 2, providing 4 tag values. **(MUST)**

**R3.** The 4 tag values MUST be assigned as:

| Tag | Value | Meaning | Decoded as |
|-----|-------|---------|------------|
| `AGENT_PORT_P0` | 0b00 | Principal port | `(value = agent_id)` |
| `AGENT_PORT_P1` | 0b01 | Left auxiliary port | `(value = agent_id)` |
| `AGENT_PORT_P2` | 0b10 | Right auxiliary port | `(value = agent_id)` |
| `FREE_PORT` | 0b11 | Free port | `(value = free_port_index)` |

**(MUST)**

**R4.** `PortRef` MUST provide the following accessor methods, each returning the same types as the SPEC-02 enum destructuring:
- `fn agent_id(&self) -> AgentId` — extracts the agent ID. MUST panic if `is_free_port()`.
- `fn port_id(&self) -> PortId` — extracts the port ID (0, 1, or 2). MUST panic if `is_free_port()`.
- `fn free_port_index(&self) -> u32` — extracts the free port index. MUST panic if `is_agent_port()`.
- `fn is_agent_port(&self) -> bool` — returns `true` if tag != FREE_PORT.
- `fn is_free_port(&self) -> bool` — returns `true` if tag == FREE_PORT.
**(MUST)**

**R5.** `PortRef` MUST provide constructor methods:
- `fn agent_port(id: AgentId, port: PortId) -> Self` — creates a compact agent port reference.
- `fn free_port(index: u32) -> Self` — creates a compact free port reference.
**(MUST)**

**R6.** The `DISCONNECTED` sentinel MUST remain `PortRef::free_port(u32::MAX >> TAG_BITS)`. This is the maximum representable free port index in the compact encoding. **(MUST)**

**R7.** `PortRef` MUST implement `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`. The `Debug` output MUST show the semantic fields (e.g., `AgentPort(42, 0)`) rather than the raw u32 value, preserving readability. **(MUST)**

**R8.** `PortRef` MUST implement custom `Serialize` and `Deserialize` that encode the u32 value directly (4 bytes LE), not via the `serde` derive macro. This enables zero-copy wire encoding when combined with SPEC-18 wire format v2. **(MUST)**

**R9.** The maximum representable `AgentId` in a compact `PortRef` is `(u32::MAX >> TAG_BITS) = 1,073,741,823` (~1 billion agents). This MUST be documented. If `AgentId` exceeds this limit, `agent_port()` MUST panic with a clear error message. **(MUST)**

### 3.2 Compact Agent

**R10.** `Agent` MUST be represented as a newtype struct wrapping a single `u64` value: `pub struct Agent(u64)`. The struct representation (`Agent { symbol: Symbol, id: AgentId }`) from SPEC-02 MUST be replaced. **(MUST)**

**R11.** The `u64` encoding MUST pack the symbol in the high bits and the agent ID in the low bits: `((symbol as u64) << 32) | (id as u64)`. **(MUST)**

**R12.** `Agent` MUST provide accessor methods:
- `fn symbol(&self) -> Symbol` — extracts the symbol from high bits.
- `fn id(&self) -> AgentId` — extracts the agent ID from low bits.
**(MUST)**

**R13.** `Agent` MUST provide a constructor: `fn new(symbol: Symbol, id: AgentId) -> Self`. **(MUST)**

**R14.** `Agent` MUST implement `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`. `Debug` output MUST show semantic fields: `Agent(Con, 42)`. **(MUST)**

### 3.3 Net Representation

**R15.** The `Net` struct MUST retain its current field layout (`agents: Vec<Option<Agent>>`, `ports: Vec<PortRef>`, `redex_queue`, `next_id`, `root`, `freeport_redirects`) but with `Agent` and `PortRef` using the compact types defined above. **(MUST)**

**R16.** The dense port array indexing formula `port_index(id, port) = id * PORTS_PER_SLOT + port` (SPEC-02) MUST remain unchanged. `PORTS_PER_SLOT` remains 3. **(MUST)**

**R17.** `Net::create_agent()` MUST use `Agent::new(symbol, id)` instead of `Agent { symbol, id }`. **(MUST)**

**R18.** `Net::connect()` MUST use `PortRef::agent_port(id, port)` and `PortRef::free_port(index)` constructors instead of enum constructors. **(MUST)**

**R19.** `Net::get_target()` MUST use `PortRef` accessor methods to decode the target port reference. **(MUST)**

### 3.4 Migration Path

**R20.** The migration MUST proceed in two phases that can be shipped as separate commits:
- **Phase A (accessor migration):** Introduce accessor methods on the existing enum-based `PortRef` and struct-based `Agent`. Migrate all ~225 pattern-match sites to use accessors. No representation change. All 690 tests pass unchanged.
- **Phase B (representation swap):** Replace enum with `struct PortRef(u32)` and struct with `struct Agent(u64)`. Accessor methods change implementation from pattern-match to bit-mask. All tests pass with the new representation.
**(MUST)**

**R21.** Phase A MUST be completed and merged before Phase B begins. This ensures that any accessor site that fails can be debugged against the known-correct enum representation. **(MUST)**

### 3.5 Invariant Preservation

**R22.** T1 (Port Linearity) MUST be verified for the compact representation: the u32→u32 port array mapping MUST be injective for live ports. The `assert_all_invariants()` function in `net/debug.rs` MUST be updated to work with compact PortRef accessors. **(MUST)**

**R23.** I1 (Bidirectional Consistency) MUST be re-verified: `connect(a, b)` MUST set `ports[port_index(a)] = b` and `ports[port_index(b)] = a` using compact encoding. A dedicated test MUST exercise all 4 tag combinations (P0↔P0, P0↔P1, P0↔P2, P0↔FreePort). **(MUST)**

**R24.** I2 (Reference Validity) MUST be re-verified: every non-DISCONNECTED entry in the port array MUST reference a live agent (for agent ports) or a valid free port index (for free ports). The tag bits MUST correctly distinguish the two cases. **(MUST)**

**R25.** I6 (ERA Auxiliary Slot Cleanliness) MUST be preserved: ERA agents occupy 3 slots in the port array (same as CON/DUP) but ports 1 and 2 MUST be DISCONNECTED. The compact encoding does not change this — ERA still occupies `PORTS_PER_SLOT` slots. **(MUST)**

### 3.6 Wire Protocol Interaction

**R26.** The custom `Serialize`/`Deserialize` for compact `PortRef` (R8) MUST be compatible with SPEC-18 wire format v2 (if implemented) or with the current bincode v1 format (if SPEC-18 is not yet implemented). For bincode v1 compatibility, the custom serde MUST encode the same byte pattern as the enum-based PortRef would produce under bincode v1. **(MUST)**

**R27.** If SPEC-18 is implemented first, compact `PortRef` serialization MAY use the varint encoding from bincode v2. If SPEC-23 is implemented first, the wire format MUST remain compatible with bincode v1 until SPEC-18 bumps the protocol version. **(MAY / MUST)**

---

## 4. Invariant Amendments

### 4.1 SPEC-02 Amendments

**A1.** SPEC-02 Section 5.2 (Port Array) is amended: `PortRef` is a `u32` newtype with bit-packed encoding, not an enum. The accessor API replaces pattern matching. All call sites use accessors.

**A2.** SPEC-02 Section 5.4 (PortRef Type) is amended: the migration path from enum to compact encoding described in SPEC-02 RQ1 is now executed. The "anticipation" becomes a "requirement."

### 4.2 SPEC-01 Amendments

**A3.** I1, I2, and I6 are NOT amended in statement — they remain structurally identical. However, their verification procedures in `assert_all_invariants()` MUST be updated to use accessor methods instead of enum pattern matching.

### 4.3 SPEC-22 Interaction

**A4.** If SPEC-22 (Arena Management) is implemented before SPEC-23, the free-list mechanism (R1-R5 of SPEC-22) operates on `Option<Agent>` where `Agent` is the struct type. When SPEC-23 converts `Agent` to a u64 newtype, the free-list's `None` representation MUST be preserved (the `Option<Agent(u64)>` niche optimization MAY be exploited, where `Agent(0)` could serve as the sentinel if symbol=0 + id=0 is reserved).

**A5.** If SPEC-23 is implemented before SPEC-22, the free-list in SPEC-22 MUST be designed for the compact `Agent(u64)` representation from the start.

---

## 5. Non-Goals

**NG1.** Dual-buffer layout (HVM2-style `nodes: Vec<u64>` + `vars: Vec<u32>`). This is a deeper architectural change that would alter the port array indexing formula and the entire `Net` struct layout. It is deferred to a potential v3. SPEC-23 keeps the single flat port array.

**NG2.** Bit-packed symbol variants beyond Con/Dup/Era. If SPEC-15 (hypothetical: extended symbols) adds new symbols, the 2-bit tag space would be insufficient for encoding port_id + symbol. SPEC-23's encoding is designed for exactly 3 port types + 1 free port type.

**NG3.** Zero-copy serialization (rkyv). Compact PortRef improves serialization efficiency but does not enable true zero-copy deserialization. That is SPEC-18 R31-R41 (rkyv archive format).

---

## 6. Memory Budget Analysis

### 6.1 Per-Agent Memory

| Component | SPEC-02 (v1) | SPEC-23 (compact) | Savings |
|-----------|--------------|-------------------|---------|
| `Agent` struct/value | 8 bytes (Symbol:1 + pad:3 + AgentId:4) | 8 bytes (u64) | 0% |
| `Option<Agent>` arena slot | 16 bytes (discriminant + Agent + pad) | 16 bytes (Option<u64> with niche = 8, but Vec alignment may round) | ~0% |
| `PortRef` per port slot | 8 bytes (enum discriminant:4 + payload:4) | 4 bytes (u32) | **50%** |
| 3 port slots per agent | 24 bytes | 12 bytes | **50%** |
| **Total per live agent** | **40 bytes** | **~24 bytes** | **~40%** |

### 6.2 Net-Level Memory

For `ep_annihilation_con(20M)` (40M agents):
- v1: 40M × 40 bytes = ~1.6 GB
- Compact: 40M × 24 bytes = ~0.96 GB
- **Savings: ~640 MB (40%)**

For `ep_annihilation_con(50M)` (100M agents):
- v1: 100M × 40 bytes = ~4.0 GB
- Compact: 100M × 24 bytes = ~2.4 GB
- **Savings: ~1.6 GB (40%)**

---

## 7. Test Strategy

### 7.1 Phase A Tests (Accessor Migration)

**T1. Accessor round-trip for agent ports.**
- For each (id, port) in [(0,0), (0,1), (0,2), (1000,0), (u32::MAX/4, 2)]:
  Create `PortRef::AgentPort(id, port)`, verify `agent_id() == id`, `port_id() == port`, `is_agent_port() == true`.

**T2. Accessor round-trip for free ports.**
- For each index in [0, 1, 1000, u32::MAX]:
  Create `PortRef::FreePort(index)`, verify `free_port_index() == index`, `is_free_port() == true`.

**T3. DISCONNECTED sentinel.**
- Verify `DISCONNECTED.is_free_port() == true`.
- Verify `DISCONNECTED.free_port_index()` returns the expected sentinel value.

### 7.2 Phase B Tests (Representation Swap)

**T4. Compact encoding round-trip.**
- For each (id, port) in [(0,0), (1,1), (1073741823, 2)]:
  Create via `PortRef::agent_port(id, port)`, verify raw u32 bits match expected encoding, verify accessor round-trip.

**T5. Compact free port round-trip.**
- For each index in [0, 1, 1073741823]:
  Create via `PortRef::free_port(index)`, verify raw u32, verify accessor.

**T6. Agent compact encoding.**
- For each (symbol, id) in [(Con, 0), (Dup, 42), (Era, 1073741823)]:
  Create `Agent::new(symbol, id)`, verify `symbol() == symbol`, `id() == id`.

**T7. AgentId overflow.**
- Verify `PortRef::agent_port(1073741824, 0)` panics (exceeds 30-bit max).

**T8. All 690 existing tests pass.**
- The full test suite MUST pass with zero modifications to test logic (only type construction may change via `impl From` or constructor methods).

### 7.3 Invariant Tests

**T9. Bidirectional consistency under compact encoding.**
- Create a net with 3 agents (Con, Dup, Era), connect them, verify `assert_all_invariants()` passes.
- Verify `get_target(connect_source) == connect_target` and vice versa.

**T10. ERA slot cleanliness.**
- Create an ERA agent, verify ports 1 and 2 are DISCONNECTED in compact encoding.

---

## 8. Open Questions

**Q1. Option<Agent(u64)> niche optimization.** Rust's `Option<NonZeroU64>` is 8 bytes (niche optimization). If `Agent(u64)` uses a `NonZeroU64` inner type (agent ID 0 is reserved as sentinel), `Option<Agent>` shrinks from 16 bytes to 8 bytes, saving an additional 8 bytes per arena slot. This would require AgentId 0 to be permanently reserved (never allocated). The v1 convention starts `next_id` at 0, so the first agent gets ID 0. This convention would need to change (start at 1). Impact: ~2 billion agents representable vs ~1 billion. Worth investigating but not required for SPEC-23 MVP.

**Q2. SIMD-friendly alignment.** The compact port array (`Vec<u32>`) could be aligned to 16-byte boundaries for SIMD operations on port scans. This is a micro-optimization deferred to benchmarking.

**Q3. Interaction with CompactSubnet wire format.** The `CompactSubnet` serde (SPEC-02, used in wire protocol) already skips dead slots and DISCONNECTED ports. The compact encoding makes this more efficient (smaller per-port footprint), but the `CompactSubnet` logic must be updated to use accessor methods.
