# TASK-0050 Reviews: Build sub-net for one partition

## Stage 4-6: Combined Review — PASS

- `build_subnet` creates sub-net per SPEC-04 Section 4.5 Step 5
- Agents/ports sized to max_id+1, preserving ID indexing
- Internal wires copied directly, border wires replaced with FreePort(bid)
- Interface (pre-existing FreePort) wires copied as-is
- Unused slots initialized to None/DISCONNECTED
- Redex queue filtered to internal-only Active Pairs
- next_id and root left for caller (split orchestrator) to set
- 7 new tests (empty, single agent, internal wire, border wire, unused slots, redex queue, interface)
- 281 total tests. Clippy clean, fmt clean
