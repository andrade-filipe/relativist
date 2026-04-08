# TEST-SPEC-0066: Implement merge function - restore boundary connections

**Task:** TASK-0066
**Spec:** SPEC-05 (R4, R5, R6, R7, R9b, R12, R13, R14)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Single border wire restored
Create two partitions: partition A has agent 0 (CON) with port 1 -> `FreePort(100)`, partition B has agent 2 (CON) with port 1 -> `FreePort(100)`. Border map: `{100: (AgentPort(0, 1), AgentPort(2, 1))}`. After merge, assert `get_target(AgentPort(0, 1)) == AgentPort(2, 1)` and vice versa.

### T2: One side erased - border discarded (R6)
Create two partitions: partition A has agent 0 with `free_port_index` containing `{100: AgentPort(0, 1)}`. Partition B's `free_port_index` does NOT contain border_id 100 (agent was erased). Border map has entry for 100. After merge, assert no panic and agent 0's port 1 remains DISCONNECTED.

### T3: Both sides erased - border discarded (R7)
Border map has entry for border_id 200. Neither partition's `free_port_index` contains 200. After merge, assert no panic. The border is silently discarded.

### T4: Border with two principal ports - redex detected
Create partition A with agent 0 (CON) whose principal port (port 0) -> `FreePort(100)`. Partition B with agent 2 (DUP) whose principal port (port 0) -> `FreePort(100)`. After merge, assert `border_redex_count == 1` and the redex queue contains the pair.

### T5: Border with principal + auxiliary - no redex
Create partition A with agent 0 at principal port (port 0) -> `FreePort(100)`. Partition B with agent 2 at auxiliary port (port 1) -> `FreePort(100)`. After merge, assert `border_redex_count == 0`.

### T6: Multiple borders restored
Create two partitions with 3 border wires (border_ids 100, 101, 102). All have valid endpoints in both partitions. After merge, assert all 3 wires are correctly restored and `border_redex_count` reflects the actual principal-principal pairs.

## Edge Cases

### E1: Empty border map
Create two partitions with no borders. After merge, assert `border_redex_count == 0` and no connections changed.

### E2: All borders erased on one side
Create a border map with 5 entries. Partition A has endpoints for all 5. Partition B has endpoints for none (all erased). After merge, assert `border_redex_count == 0` and no panic.

### E3: Round-trip identity (D1)
Build a net with 2 CON agents connected at principal ports (active pair). Split into 2 partitions (no local reduction). Merge back. Assert the result net is structurally isomorphic to the original.
