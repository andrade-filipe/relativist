# TEST-SPEC-0553: install_connection helper (internal vs border classification + border ID allocation)

**SPEC-21 §7 ID:** plumbing (T5 partial — C3 bijectivity verified post-finalize via TASK-0570).
**Owning task:** TASK-0553.
**Parent spec:** SPEC-21 §4.6 install_connection (border classification at connection-time, AC-007 pattern); §3.3 R20.
**Type:** unit + integration.
**Theory anchor:** AC-007 (HVM2 Reduction Engine — connection-time atomic-link with ownership informs the no-separate-scan-pass discipline); ARG-002 Q3 (bidirectional FreePort).

---

## Inputs / Fixtures

- 4 accumulators, one per worker (`WorkerId(0..4)`).
- An `agent_owner: HashMap<AgentId, WorkerId>` mapping that places agents 0, 1 in worker 0; agents 2, 3 in worker 1; etc.
- A fresh `border_id_counter` starting at 0.
- A `border_map: HashMap<u32, (PortRef, PortRef)>` initially empty.

## Unit Tests

| ID | Test name | Given | When | Then |
|----|-----------|-------|------|------|
| UT-0553-01 | `internal_wire_inserts_only_in_owner_accumulator` | agents 0 and 1 both owned by worker 0; connect `(0, 0) ↔ (1, 1)` | call `install_connection` | `accumulators[0]` has the wire; `accumulators[1..4]` unchanged; `border_map` unchanged. |
| UT-0553-02 | `border_wire_allocates_bid_zero_first` | agent 0 owned by worker 0, agent 1 owned by worker 1; connect `(0,0) ↔ (1,1)`; `border_id_counter = 0` initially | call `install_connection` | `border_id_counter == 1` after; `border_map[0] == ((0,0), (1,1))` (or its canonical orientation per R20). |
| UT-0553-03 | `border_wire_calls_connect_on_both_accumulators` | UT-0553-02 fixture | post-call: inspect both accumulators | both have a `connect((agent, port), FreePort(0))` registered (`accumulators[0]` connects (0,0) to FreePort(0); `accumulators[1]` connects (1,1) to FreePort(0)). |
| UT-0553-04 | `sequential_border_allocation_zero_one_two` | 5 cross-partition wires installed sequentially: `(0,0)↔(1,1), (2,0)↔(3,1), ...` | call `install_connection` 5× | post: `border_map.keys()` = {0, 1, 2, 3, 4}; `border_id_counter == 5`. |
| UT-0553-05 | `mixed_internal_and_border_sequence` | sequence of 4 wires: 2 internal, 2 border | call install_connection 4× | only 2 border IDs allocated; `border_id_counter == 2`; correct accumulators received the internal wires. |
| UT-0553-06 | `connection_time_classification_no_scan_pass` | the impl source file | grep for `for ... in net.wires` (post-construction full-net iteration) inside `install_connection` or its callers | NONE (AC-007 pattern: classification is connection-time, not scan-time). |
| UT-0553-07 | `border_map_canonical_orientation` | UT-0553-02 fixture; the connect direction `(0,0) ↔ (1,1)` and the reversed `(1,1) ↔ (0,0)` | both calls | both produce the SAME `border_map[0]` value (orientation canonicalized; e.g., always lower-WorkerId first). |

## Edge cases

| EC | Scenario | Expected |
|----|----------|----------|
| EC-1 | One endpoint references an agent NOT in `agent_owner` (orphan / forward reference) | install_connection MUST treat this as Pending (caller forgot to filter); behavior delegated to the orchestrator (TEST-SPEC-0554). |
| EC-2 | Both endpoints are FreePorts (border-to-border) | a degenerate but allowed case; the helper allocates one border id per FreePort pair (per R20 spec); test asserts the documented behavior. |
| EC-3 | A self-loop connect `(0, 0) ↔ (0, 1)` (agent connected to itself) | succeeds (this is a valid IC structure: a wire between two ports of the same agent). Installed in the owner's accumulator only. |
| EC-4 | `border_id_counter` near `u32::MAX` | overflow MUST be detected; install_connection returns Err / panics. |

## Invariants asserted

- T1 (Port Linearity) — preserved via delegated `connect`.
- C2 (Complete Wire Coverage) — every connection becomes either an internal wire or a border-wire pair.
- C3 (FreePort Bijectivity) — established at the install_connection layer; full bijectivity verified post-finalize in TEST-SPEC-T5.
- §4.6 install_connection contract (connection-time classification).
- R20 (border id range; canonical orientation in border_map).

## ARG/DISC/REF citation

- AC-007 (HVM2 Reduction Engine atomic-link with ownership pattern).
- ARG-002 Q3 (bidirectional FreePort).

## Determinism notes

UT-0553-04 sequential allocation depends on call order. The orchestrator (TEST-SPEC-0554) MUST invoke install_connection in deterministic order (by chunk-then-by-directive-index). The test fixtures use explicit ordering.

UT-0553-07 canonical orientation: the spec MUST document the canonicalization rule (e.g., "always insert with lower WorkerId first"); this test grep-asserts the documented rule against the actual behavior.

UT-0553-06 (connection-time vs scan-time discipline) is a code-review gate plus a structural grep. Implementer MUST NOT introduce a post-construction wire-iteration pass; this is a regression against AC-007 pattern.

## Cross-test dependencies

- TEST-SPEC-0550, TEST-SPEC-0551 (accumulator constructed + connect available) — prerequisites.
- TEST-SPEC-0552 (finalize) — runs AFTER install_connection populates accumulators.
- TEST-SPEC-T5 (streaming pipeline produces valid partitions) — full C3 bijectivity coverage post-finalize.
- TEST-SPEC-0554 (orchestrator) — calls install_connection per directive; UT-0554 verifies the call sequence.
- TEST-SPEC-T3 (forward reference resolution) — Pending → Resolved transitions invoke install_connection.
