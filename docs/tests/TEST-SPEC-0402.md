# TEST-SPEC-0402: Worker-side mint-then-wire — R24.1.6a/b/c + R33c case dispatch + R24 ordering invariant

**See also:** [docs/backlog/TASK-0402.md](../backlog/TASK-0402.md); [TEST-SPEC-0400](./TEST-SPEC-0400.md); [TEST-SPEC-0401](./TEST-SPEC-0401.md).

**Task:** TASK-0402
**Parent spec:** SPEC-19 §3.3 R23a (5 substitution clauses + clause 6 HashSet pre-pass), R24.1.6a/b/c (mint-then-wire protocol), R24 ordering invariant, §3.4 R33b/R33c (cases 1-7), §3.6 R48a/R48b.
**Bundle:** D-005 Option A.
**Date:** 2026-04-23
**Baseline before this task:** 1158 lib default / 1201 lib `--features zero-copy` (post-TASK-0401).
**Cumulative target after this task:** **≥ 1169 / ≥ 1212** (+11 both configs).

---

## Scope

### Covers
1. R24.1.6a mint — correct symbol, correct count, monotonic AgentId from worker's IdRange.
2. R24.1.6b wire — R23a clauses 3 (slot-marker decode), 4 (concrete-id pass-through), 6 (HashSet pre-pass for duplicate detection).
3. R24.1.6c echo — slot-0 canonical `minted_agent_id` regardless of in-round reduction activity.
4. R24 ordering invariant — `reduce_all` NOT called inside the mint-then-wire loop.
5. R33c case dispatch — all 7 cases (6 reject paths + 1 warn path).
6. R48a stray slot-marker rejection.
7. R48b empty `local_wiring` legality.
8. Happy-path CON-DUP end-to-end preflight (parity against `Net::commute_con_dup` reference).

### Does NOT cover
- Wire-layer serialization (TEST-SPEC-0400).
- Resolver-side PC population (TEST-SPEC-0401).
- G1 parity through the full BSP loop (TEST-SPEC-0403).
- TCP `RegisterNack` transport — existing protocol-error tests cover that mapping; TEST-SPEC-0402 stops at the `Err(ProtocolError::MalformedLocalWiring)` return value.

---

## Test target file paths

- `relativist-core/src/worker.rs` — inline `#[cfg(test)] mod tests` block. New `#[test]` fns: UT-0402-01 through UT-0402-11.
- Optional: dedicated `worker_mint_then_wire_tests.rs` submodule if inline block exceeds current worker.rs test-block size — developer discretion.

All tests synchronous. No tokio, no async. The helper under test (`apply_pending_commutation`) is a pure function over `&mut Net` + `&PendingCommutation` + `&mut Vec<MintedAgent>`.

Tests that need to capture `tracing::warn!` use the `tracing_test::traced_test` attribute or an equivalent `tracing::subscriber::with_default` + buffer pattern.

---

## Unit Tests

### UT-0402-01: `apply_pending_commutation_rejects_src_slot_out_of_range`

**Purpose:** R33c case 1. A hint with `src_slot >= pc.target_symbols.len()` is rejected BEFORE any `Net::connect` call.

**Target:** `worker.rs::tests`.

**Given:**
```
let mut net = Net::new_with_range(IdRange { start: 0, end: 10_000 });
let pc = PendingCommutation {
    request_id: 1,
    target_symbols: vec![Symbol::Con],  // len == 1, only slot 0 is valid
    local_wiring: vec![
        LocalWiringHint { src_slot: 2, src_port: 1, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 1) },
    ],
};
let mut response = Vec::new();
```

**When:** `let result = apply_pending_commutation(&mut net, &pc, &mut response);`.

**Then:**
- `result == Err(ProtocolError::MalformedLocalWiring { request_id: 1, reason: MalformedLocalWiringReason::SrcSlotOutOfRange { src_slot: 2, symbol_count: 1 } })`.
- `response.is_empty()` — no MintedAgent echo on error.
- Note on net state: by R24 ordering §5 "atomic-batch", rejection AFTER mint but BEFORE connect means the minted CON agent may exist in `net.agents` when this error fires (src_slot validation runs inside the wire loop). Developer discretion: if implementation validates src_slot BEFORE the mint loop, `net.agents` is unchanged; either behavior is acceptable — the test MUST document which is the case and assert accordingly. **Default assumption (per TASK-0402 §3 flow):** mint loop runs first, validation happens in wire loop, so `net.agents.len() == 1`.

**Edge cases:**
- EC-1: `src_slot = pc.target_symbols.len()` (exactly one past the end). Same error.
- EC-2: `src_slot = u8::MAX` with `target_symbols.len() == 1`. Same error; no `u8` overflow.

**SPEC covered:** R33c case 1.

**Priority:** MANDATORY.

---

### UT-0402-02: `apply_pending_commutation_rejects_src_port_out_of_range`

**Purpose:** R33c case 2. `src_port` exceeds the port count of `target_symbols[src_slot]`. CON/DUP have 3 ports (0..=2); ERA has 1 port (0 only).

**Target:** `worker.rs::tests`.

**Given:** 3 sub-scenarios run as a table-driven test:
```
// Sub-a: CON with src_port == 3 (out-of-range; ports 0, 1, 2 are legal)
let pc_a = PendingCommutation {
    request_id: 1,
    target_symbols: vec![Symbol::Con],
    local_wiring: vec![LocalWiringHint { src_slot: 0, src_port: 3, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 0) }],
};

// Sub-b: ERA with src_port == 1 (out-of-range; only port 0 is legal for ERA)
let pc_b = PendingCommutation {
    request_id: 2,
    target_symbols: vec![Symbol::Era],
    local_wiring: vec![LocalWiringHint { src_slot: 0, src_port: 1, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 0) }],
};

// Sub-c: DUP with src_port == u8::MAX (boundary; same rejection)
let pc_c = PendingCommutation {
    request_id: 3,
    target_symbols: vec![Symbol::Dup],
    local_wiring: vec![LocalWiringHint { src_slot: 0, src_port: u8::MAX, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 0) }],
};
```

**When:** Call `apply_pending_commutation` on each.

**Then:**
- Each returns `Err(MalformedLocalWiring { reason: SrcPortOutOfRange { src_slot: 0, src_port: <per-case> } })`.
- `response.is_empty()`.

**Edge cases:**
- EC-1: valid boundary: CON with `src_port = 2` → should be `Ok(_)` (this is the upper legal bound). Include as a positive control within the same table-driven test.
- EC-2: ERA with `src_port = 0` → also `Ok(_)`, even with empty `local_wiring` being the more typical ERA case.

**SPEC covered:** R33c case 2.

**Priority:** MANDATORY.

---

### UT-0402-03: `apply_pending_commutation_rejects_target_sibling_out_of_range_r48a`

**Purpose:** R33c case 3 / R48a stray slot-marker guard. A hint with `target = AgentPort(x, p)` where `x >= SLOT_MARKER_BASE` and `(x - SLOT_MARKER_BASE) >= pc.target_symbols.len()` is rejected.

**Target:** `worker.rs::tests`.

**Given:**
```
let pc = PendingCommutation {
    request_id: 5,
    target_symbols: vec![Symbol::Con, Symbol::Dup],  // len == 2; valid slot_idx in [0, 2)
    local_wiring: vec![
        LocalWiringHint {
            src_slot: 0,
            src_port: 1,
            target: PortRef::AgentPort(SLOT_MARKER_BASE + 99, 0),  // slot_idx = 99, out of range
        },
    ],
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)`.

**Then:**
- `result == Err(MalformedLocalWiring { request_id: 5, reason: TargetSiblingOutOfRange { sibling_slot: 99, symbol_count: 2 } })`.
- `response.is_empty()`.

**Edge cases:**
- EC-1: boundary `x = SLOT_MARKER_BASE + 2` (exactly one past the end with `target_symbols.len() == 2`) — same rejection.
- EC-2: boundary `x = SLOT_MARKER_BASE + 1` with `target_symbols.len() == 2` — ACCEPTED (positive control).

**SPEC covered:** R33c case 3, R48a stray slot-marker guard.

**Priority:** MANDATORY.

---

### UT-0402-04: `apply_pending_commutation_warns_on_dangling_concrete_agent_case_4_should_warn`

**Purpose:** R33c case 4 — SHOULD-warn, NOT reject. Concrete-id target where the id is absent from the partition. `Net::connect` is still called; a `tracing::warn!` is emitted.

**Target:** `worker.rs::tests` with `#[traced_test]` attribute.

**Given:**
```
let mut net = Net::new_with_range(IdRange { start: 0, end: 10_000 });
// Do NOT insert agent id 999 into the partition.
let pc = PendingCommutation {
    request_id: 7,
    target_symbols: vec![Symbol::Con],
    local_wiring: vec![
        LocalWiringHint {
            src_slot: 0,
            src_port: 1,
            target: PortRef::AgentPort(999, 0),  // 999 < SLOT_MARKER_BASE → concrete-id path; id absent from net
        },
    ],
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)` under a tracing subscriber that buffers events.

**Then:**
- `result.is_ok()` — the edge is applied despite the dangling target.
- `response.len() == 1` and `response[0].request_id == 7`.
- The tracing buffer captures at least one `Level::WARN` event whose message contains "R33c case 4" or "dangling concrete agent" and the agent id `999`.
- `net.get_agent(minted_id).unwrap().ports[1] == PortRef::AgentPort(999, 0)` — connect edge applied even with dangling peer.

**Edge cases:**
- EC-1: target id is inside the coordinator-reserved range (`u32::MAX - 10_000 .. u32::MAX`) — treated as a concrete-id (since it's < `SLOT_MARKER_BASE`); warn+apply path the same.
- EC-2: target id IS present in the partition (non-dangling) — no warn captured; `result.is_ok()` unchanged.

**SPEC covered:** R33c case 4 (SHOULD-warn semantic).

**Priority:** MANDATORY. Requires `tracing-test` or equivalent dev-dependency; developer confirms availability in `Cargo.toml` before shipping this test. If unavailable, fallback: assert only `result.is_ok()` and the edge is applied; document the warn-capture as deferred.

---

### UT-0402-05: `apply_pending_commutation_rejects_duplicate_source_port_case_5_via_hashset_pre_pass`

**Purpose:** R33c case 5 / R23a clause 6 NF-003. Duplicate `(src_slot, src_port)` keys in `local_wiring` are detected by the HashSet pre-pass BEFORE any `Net::connect` call. Rejection is atomic — the net is unmodified (except for mints which happened before wire step).

**Target:** `worker.rs::tests`.

**Given:**
```
let mut net = Net::new_with_range(IdRange { start: 0, end: 10_000 });
let initial_agent_count = net.agents.len();
let initial_connects = 0;  // if tracked via an internal counter, or use a spy wrapper

let pc = PendingCommutation {
    request_id: 8,
    target_symbols: vec![Symbol::Con],
    local_wiring: vec![
        LocalWiringHint { src_slot: 0, src_port: 1, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 2) },
        LocalWiringHint { src_slot: 0, src_port: 1, target: PortRef::AgentPort(SLOT_MARKER_BASE + 0, 2) },  // duplicate
    ],
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)`.

**Then:**
- `result == Err(MalformedLocalWiring { request_id: 8, reason: DuplicateSourcePort { src_slot: 0, src_port: 1 } })`.
- `response.is_empty()`.
- **Atomic-batch assertion:** the minted CON agent exists in `net.agents` (mint ran before pre-pass per TASK-0402 flow), BUT none of its aux ports were connected. Specifically: `net.agents.len() == initial_agent_count + 1` and for `aid = minted_id`, `net.agents[aid as usize].ports[1] == PortRef::DISCONNECTED` and `.ports[2] == PortRef::DISCONNECTED`.
- **Ordering of pre-pass vs range-validation:** the HashSet pre-pass (clause 6) MUST run BEFORE per-hint range validation (clauses 1-3). Include a second sub-case where the duplicates also violate `SrcSlotOutOfRange`: verify the error is `DuplicateSourcePort`, NOT `SrcSlotOutOfRange`. This confirms ordering.

**Edge cases:**
- EC-1: 3 entries with the same `(src_slot, src_port)` — the HashSet detects the second one; the third is never reached.
- EC-2: duplicates on different slots/ports pair: e.g. `(0, 1)` and `(1, 1)` are NOT duplicates (different slot); accepted.

**SPEC covered:** R33c case 5, R23a clause 6, NF-003.

**Priority:** MANDATORY.

---

### UT-0402-06: `apply_pending_commutation_rejects_reserved_for_future_freeport_target_case_6`

**Purpose:** R33c case 6 / R48a. `target = PortRef::FreePort(_)` is reserved for future cross-partition wire breaks; the worker rejects any PC that attempts to route a minted port to a FreePort. Cross-partition edges MUST go through `PendingNewBorder`, never `local_wiring`.

**Target:** `worker.rs::tests`.

**Given:**
```
let pc = PendingCommutation {
    request_id: 9,
    target_symbols: vec![Symbol::Con],
    local_wiring: vec![
        LocalWiringHint {
            src_slot: 0,
            src_port: 1,
            target: PortRef::FreePort(42),  // RESERVED
        },
    ],
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)`.

**Then:**
- `result == Err(MalformedLocalWiring { request_id: 9, reason: ReservedForFuture { border_id: 42 } })`.
- `response.is_empty()`.

**Edge cases:**
- EC-1: `FreePort(0)` — boundary; same rejection.
- EC-2: `FreePort(u32::MAX)` — boundary; same rejection.
- EC-3: confirm a FreePort target NEVER accidentally coerces to a slot-marker (the slot-marker path is keyed off `PortRef::AgentPort(x, _)` with `x >= SLOT_MARKER_BASE`, NOT `PortRef::FreePort`).

**SPEC covered:** R33c case 6, R48a, SC-008.

**Priority:** MANDATORY.

---

### UT-0402-07: `apply_pending_commutation_rejects_zero_arity_case_7_before_minting`

**Purpose:** R33c case 7 / NF-004. Empty `target_symbols` is rejected BEFORE any `create_agent` call — this guarantees `minted_ids_per_pc[0]` (consumed by R24.1.6c echo) is always well-defined for non-rejected PCs.

**Target:** `worker.rs::tests`.

**Given:**
```
let mut net = Net::new_with_range(IdRange { start: 0, end: 10_000 });
let initial_agent_count = net.agents.len();
let pc = PendingCommutation {
    request_id: 10,
    target_symbols: vec![],  // ZeroArity
    local_wiring: vec![],
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)`.

**Then:**
- `result == Err(MalformedLocalWiring { request_id: 10, reason: ZeroArity })`.
- `response.is_empty()`.
- **Mint guard:** `net.agents.len() == initial_agent_count` — NO mint attempted (verify NF-004 ordering: rejection BEFORE mint loop).

**Edge cases:**
- EC-1: `local_wiring` non-empty BUT `target_symbols` empty → still ZeroArity (rejection happens first; wire loop never runs).

**SPEC covered:** R33c case 7, NF-004.

**Priority:** MANDATORY.

---

### UT-0402-08: `apply_pending_commutation_empty_local_wiring_is_legal_r48b`

**Purpose:** R48b — `local_wiring: vec![]` is legal. The minted agent(s) stay in the partition with all aux ports DISCONNECTED. The echo still fires with the slot-0 minted id.

**Target:** `worker.rs::tests`.

**Given:**
```
let mut net = Net::new_with_range(IdRange { start: 0, end: 10_000 });
let pc = PendingCommutation {
    request_id: 11,
    target_symbols: vec![Symbol::Con],
    local_wiring: vec![],  // R48b empty legal
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)`.

**Then:**
- `result.is_ok()`.
- `response.len() == 1` and `response[0] == MintedAgent { request_id: 11, minted_agent_id: minted_id_slot_0 }`.
- `net.agents.iter().any(|a| a.symbol == Symbol::Con)` — CON minted.
- All aux ports of the minted CON equal `PortRef::DISCONNECTED` (no edges applied).

**Edge cases:**
- EC-1: `target_symbols = vec![Symbol::Era]` + `local_wiring = vec![]` → ERA minted, no aux ports to disconnect (arity 0).
- EC-2: `target_symbols = vec![Symbol::Con, Symbol::Dup]` + `local_wiring = vec![]` → both minted, both disconnected, echo is the CON's id (slot-0).

**SPEC covered:** R48b.

**Priority:** MANDATORY.

---

### UT-0402-09: `apply_pending_commutation_r24_ordering_invariant_no_reduce_all_inside_loop`

**Purpose:** R24 ordering invariant — `Net::connect` auto-enqueues principal-principal redexes; those MUST remain QUEUED and NOT drained inside the mint-then-wire loop. Reduction happens at R24.2, after all PCs have been applied.

**Target:** `worker.rs::tests`.

**Given:** A PC whose `local_wiring` connects the principals of two minted siblings (creating an in-round redex):
```
let mut net = Net::new_with_range(IdRange { start: 0, end: 10_000 });
let initial_queue_len = net.redex_queue.len();  // typically 0
let initial_reductions = net.reductions_performed;  // existing counter, typically 0

let pc = PendingCommutation {
    request_id: 12,
    target_symbols: vec![Symbol::Con, Symbol::Con],
    local_wiring: vec![
        // Wire slot-0 principal to slot-1 principal → creates a CON-CON redex in-round
        LocalWiringHint {
            src_slot: 0,
            src_port: 0,  // principal
            target: PortRef::AgentPort(SLOT_MARKER_BASE + 1, 0),  // sibling-slot principal
        },
    ],
};
```

**When:** `apply_pending_commutation(&mut net, &pc, &mut response)`.

**Then:**
- `result.is_ok()`.
- `net.redex_queue.len() >= initial_queue_len + 1` — the principal-principal pair is queued.
- `net.reductions_performed == initial_reductions` — NO reduction fired inside the loop.
- Both minted CON agents still exist in `net.agents` (the CON-CON redex has NOT yet been consumed).
- `response.len() == 1` and `response[0].minted_agent_id == minted_ids[0]` (slot-0, even though this id may soon be consumed by the redex).

**Edge cases:**
- EC-1: multi-PC scenario — apply 2 PCs in sequence, each producing in-round redexes. Verify `reductions_performed` stays 0 across both calls.
- EC-2: connect aux-to-aux (NOT principal-principal) — queue SHOULD NOT gain an entry, but reductions stay 0 regardless.

**SPEC covered:** R24 ordering invariant; R23a clause 5 (in-round reducibility of minted pairs, deferred to R24.2).

**Priority:** MANDATORY (critical invariant; a silent violation would double-count interactions at G1 parity).

---

### UT-0402-10: `apply_pending_commutation_echo_slot_zero_semantics_r24_1_6c`

**Purpose:** R24.1.6c echo — `MintedAgent.minted_agent_id == minted_ids_per_pc[0]`, UNCONDITIONALLY, regardless of whether the agent is consumed by in-round reduction, whether `local_wiring` is empty, or whether additional siblings exist (slots 1+).

**Target:** `worker.rs::tests`.

**Given:** 2 sub-scenarios in one test:
```
// Sub-a: PC with 3 siblings, empty local_wiring — no reduction fires.
let pc_a = PendingCommutation {
    request_id: 20,
    target_symbols: vec![Symbol::Con, Symbol::Dup, Symbol::Era],
    local_wiring: vec![],
};

// Sub-b: PC with 2 siblings + local_wiring that WILL produce an in-round redex
// consuming slot-0's principal (simulated: connect slot-0 principal to slot-1 principal,
// then manually invoke reduce_all in the test after apply_pending_commutation returns).
let pc_b = PendingCommutation {
    request_id: 21,
    target_symbols: vec![Symbol::Con, Symbol::Con],
    local_wiring: vec![
        LocalWiringHint { src_slot: 0, src_port: 0, target: PortRef::AgentPort(SLOT_MARKER_BASE + 1, 0) },
    ],
};
```

**When:**
- Sub-a: `apply_pending_commutation(...)` only.
- Sub-b: `apply_pending_commutation(...)` then `net.reduce_all()`.

**Then:**
- Sub-a:
  - `response[0].request_id == 20`.
  - `response[0].minted_agent_id == minted_ids_a[0]` (the CON — slot 0).
  - NOT the DUP's id (slot 1) and NOT the ERA's id (slot 2).
- Sub-b:
  - `response[1].request_id == 21`.
  - `response[1].minted_agent_id == minted_ids_b[0]` — the pre-reduction slot-0 CON id, EVEN THOUGH that CON was consumed by the CON-CON redex during `reduce_all`.
  - After `reduce_all`, `net.get_agent(response[1].minted_agent_id)` may return `None` (consumed) — this is expected; the echo captures the ORIGINAL mint-time id, not a post-reduction survivor.

**Edge cases:**
- EC-1: 1-slot PC — trivial; echo id == the only minted id.
- EC-2: 16-slot PC (max cap) — echo id == slot 0 (the first minted id), NOT slot 15.

**SPEC covered:** R24.1.6c echo semantics.

**Priority:** MANDATORY.

---

### UT-0402-11: `apply_pending_commutation_happy_path_con_dup_matches_reference_net`

**Purpose:** End-to-end parity preflight. Construct a full CON-DUP `PendingCommutation` mirroring what `border_resolver.rs:820-866` would emit for a border-crossing CON-DUP redex; apply it via `apply_pending_commutation`; run `reduce_all`; compare the resulting partition state against a reference net built via `Net::commute_con_dup` directly on the equivalent inputs.

**Target:** `worker.rs::tests`.

**Given:**
- **Reference net (pre-worker):** a minimal 2-agent net with a CON at id 0 and a DUP at id 1, principal-connected (no partition split). The expected `commute_con_dup` outcome: 4 new agents, 4 edges, CON-DUP consumed.
- **Worker net (under test):**
  - Start with an empty net in partition 0.
  - Pre-populate with any "border-side" agents the CON's aux ports route to — for a minimal test, use two sink ERA agents at ids 10 and 11.
  - Construct the PC:
    ```
    let pc = PendingCommutation {
        request_id: 30,
        target_symbols: vec![Symbol::Dup, Symbol::Con],  // or [Con, Dup] — match resolver emission order
        local_wiring: vec![
            // 4 hints: each minted agent's aux ports routed.
            // Exact hints per border_resolver.rs:820-866 structure.
            LocalWiringHint { src_slot: 0, src_port: 1, target: PortRef::AgentPort(SLOT_MARKER_BASE + 1, 1) },
            LocalWiringHint { src_slot: 0, src_port: 2, target: PortRef::AgentPort(SLOT_MARKER_BASE + 1, 2) },
            LocalWiringHint { src_slot: 1, src_port: 1, target: PortRef::AgentPort(10, 0) },  // concrete ERA
            LocalWiringHint { src_slot: 1, src_port: 2, target: PortRef::AgentPort(11, 0) },  // concrete ERA
        ],
    };
    ```

**When:** `apply_pending_commutation(&mut worker_net, &pc, &mut response)?; worker_net.reduce_all();`.

**Then:**
- `response[0] == MintedAgent { request_id: 30, minted_agent_id: minted_ids[0] }`.
- The post-reduce worker net is structurally equivalent to the reference net under canonical node-renaming (apply the same `canonicalize` helper used by UT-0385-08).
- `worker_net.reductions_performed >= 1` (the in-round redex, if any, was consumed at `reduce_all`).

**Edge cases:**
- EC-1: CON-DUP with both aux ports on each side routed to placeholders (4-placeholder emission pattern) — verify symmetry.
- EC-2: CON-DUP with one aux port concrete + one placeholder — mixed pattern.

**SPEC covered:** R24.1.6a/b/c end-to-end at worker level; G1 parity preflight (the integration-level gate is TEST-SPEC-0403).

**Priority:** MANDATORY (this is the closest thing to an integration test at the TEST-SPEC-0402 scope; catches gross wiring bugs before TEST-SPEC-0403 hits them via UT-0385-08).

---

## Property Tests

### PT-0402-01 (optional): `prop_apply_pending_commutation_preserves_r24_ordering_invariant`

**Property:** For all arbitrary-but-valid PCs, after `apply_pending_commutation` returns `Ok(_)`, `net.reductions_performed` equals its pre-call value.

**Generator strategy:**
- `arb_target_symbols()` — length 1..=4 (keep small for tractability).
- `arb_local_wiring(target_symbols, arity_map)` — respect clauses 1-4 (no case 5-7 violations, valid slot/port ranges).
- Include targets across all three categories (placeholder / concrete-existing / concrete-absent).

**Assertion:**
```
let before = net.reductions_performed;
let result = apply_pending_commutation(&mut net, &pc, &mut response);
prop_assume!(result.is_ok());
prop_assert_eq!(net.reductions_performed, before);
```

**Shrinking note:** counterexamples should isolate the specific hint that caused (spurious) reduction.

**Priority:** OPTIONAL. UT-0402-09 covers the single-case invariant; property coverage is belt-and-braces. INCLUDE if proptest is in-scope for the crate.

---

## Integration Tests

None at this task level. Integration-level coverage is TEST-SPEC-0403.

However, UT-0402-11 is a mini-integration (end-to-end within one partition). It is the bridge between unit-level and full-loop integration.

---

## Negative Tests (Catalog)

Already covered by UT-0402-01 through UT-0402-07. Summary:

| Case | UT |
|------|-----|
| R33c case 1 SrcSlotOutOfRange | UT-0402-01 |
| R33c case 2 SrcPortOutOfRange | UT-0402-02 |
| R33c case 3 TargetSiblingOutOfRange / R48a | UT-0402-03 |
| R33c case 4 DanglingConcreteAgent (SHOULD-warn) | UT-0402-04 |
| R33c case 5 DuplicateSourcePort | UT-0402-05 |
| R33c case 6 ReservedForFuture | UT-0402-06 |
| R33c case 7 ZeroArity | UT-0402-07 |

**Boundary conditions (documented as ECs within each UT):**
- Lower bound of each numeric field (0).
- Upper bound (u8::MAX for slot/port; u32::MAX for agent ids).
- `target_symbols.len() == 16` (cap).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-0402-01 | src_slot at upper boundary (exactly `len()`) | Rejected (off-by-one check) | UT-0402-01 EC-1 |
| EC-0402-02 | src_port at ERA upper bound (port 0 OK, port 1 rejected) | Rejected at 1 | UT-0402-02 Sub-b |
| EC-0402-03 | src_port at CON/DUP upper bound (port 2 OK, port 3 rejected) | Rejected at 3 | UT-0402-02 Sub-a |
| EC-0402-04 | target slot-marker at upper boundary | Rejected off-by-one | UT-0402-03 EC-1 |
| EC-0402-05 | target id in coordinator-reserved range | Warn + apply (case 4 semantics) | UT-0402-04 EC-1 |
| EC-0402-06 | 3 duplicate hints (same slot+port) | Detect on 2nd via HashSet; 3rd never reached | UT-0402-05 EC-1 |
| EC-0402-07 | FreePort(0) and FreePort(u32::MAX) | Both rejected as case 6 | UT-0402-06 EC-1/2 |
| EC-0402-08 | PC with empty target_symbols + non-empty local_wiring | Rejected as case 7 (wire loop never runs) | UT-0402-07 EC-1 |
| EC-0402-09 | PC with target_symbols non-empty + empty local_wiring | Ok (R48b) | UT-0402-08 |
| EC-0402-10 | Multi-PC scenario: reductions stay 0 across both | R24 ordering preserved | UT-0402-09 EC-1 |
| EC-0402-11 | Echo with consumed slot-0 agent post-reduce_all | MintedAgent.id = original mint-time id | UT-0402-10 Sub-b |
| EC-0402-12 | CON-DUP 4-placeholder pattern | Mirror of UT-0402-11 happy path | UT-0402-11 EC-1 |

---

## Coverage mapping

| Requirement / contract | Covered by |
|------------------------|-----------|
| SPEC-19 R23a clause 1 (per-PC slot namespace) | UT-0402-01, 02 structural (each PC's slots are independent) |
| SPEC-19 R23a clause 2 (mint count = target_symbols.len()) | UT-0402-08, 10, 11 |
| SPEC-19 R23a clause 3 (slot-marker decode) | UT-0402-03 (reject path) + UT-0402-11 (accept path) |
| SPEC-19 R23a clause 4 (concrete-id pass-through) | UT-0402-04 + UT-0402-11 |
| SPEC-19 R23a clause 5 (in-round reducibility deferred) | UT-0402-09 |
| SPEC-19 R23a clause 6 / NF-003 (HashSet pre-pass) | UT-0402-05 |
| SPEC-19 R24.1.6a mint | UT-0402-08, 10, 11 (positive); UT-0402-07 (no-mint on reject) |
| SPEC-19 R24.1.6b wire | UT-0402-01 through 06, 11 |
| SPEC-19 R24.1.6c echo | UT-0402-10 |
| SPEC-19 R24 ordering invariant | UT-0402-09 |
| SPEC-19 R33b `Net::connect` sanctioned primitive | UT-0402-11 (exercised); all others indirectly |
| SPEC-19 R33c case 1 | UT-0402-01 |
| SPEC-19 R33c case 2 | UT-0402-02 |
| SPEC-19 R33c case 3 | UT-0402-03 |
| SPEC-19 R33c case 4 | UT-0402-04 |
| SPEC-19 R33c case 5 | UT-0402-05 |
| SPEC-19 R33c case 6 | UT-0402-06 |
| SPEC-19 R33c case 7 | UT-0402-07 |
| SPEC-19 R48a stray slot-marker guard | UT-0402-03 |
| SPEC-19 R48b empty local_wiring legal | UT-0402-08 |
| NF-003 atomicity (pre-pass detects before connect) | UT-0402-05 (atomic-batch assertion) |
| NF-004 ZeroArity rejected before mint | UT-0402-07 (mint guard) |

---

## Test count estimate

| Config | Baseline | UTs added | Target |
|--------|----------|-----------|--------|
| `cargo test --workspace --lib` | 1158 | +11 (UT-0402-01..11) | **≥ 1169** |
| `cargo test --workspace --lib --features zero-copy` | 1201 | +11 | **≥ 1212** |

PT-0402-01 is OPTIONAL; if included, both targets +1.

---

## Acceptance gate

1. `cargo test --workspace --lib` count: 1158 → ≥ 1169.
2. `cargo test --workspace --lib --features zero-copy` count: 1201 → ≥ 1212.
3. `cargo clippy --workspace --all-targets -- -D warnings` clean (default + zero-copy).
4. `cargo fmt --check` clean.
5. UT-0385-06/07 still green (symmetric-rule G1 parity regression canary).
6. `SKIP_ASYMMETRIC` remains `true` — flip is TASK-0403.
7. `tracing-test` dev-dependency available (or fallback path documented for UT-0402-04).
8. No regression on existing tests; baseline 1158 / 1201 lower bound holds.

---

## Notes on interaction with other TEST-SPECs

- **TEST-SPEC-0400:** consumes error enum; a failing UT-0400-04 implies all of TEST-SPEC-0402's R33c reject tests fail sympathetically.
- **TEST-SPEC-0401:** supplies the producer-side PCs; TEST-SPEC-0402 verifies the consumer-side. UT-0402-11 (happy-path) implicitly validates the full transport chain.
- **TEST-SPEC-0403:** bundle acceptance gate; depends on every UT here being green.
- **TEST-SPEC-0398 UT-0398-03/04/05:** coordinator-side consumption of `MintedAgent`; this TEST-SPEC produces the `MintedAgent` that the coordinator-side tests consume.

---

## Out of scope

- **TCP session NACK mapping** — downstream of `Err(ProtocolError::MalformedLocalWiring)` return; existing protocol-error routing tests cover it.
- **Performance of the HashSet pre-pass** — `O(local_wiring.len())`; negligible.
- **Fuzz of arbitrary `PortRef` values** — PT-0402-01 covers proptest-level coverage.
- **IdRange exhaustion behavior** — SPEC-04 owns; existing UTs cover.
- **Mutex/concurrency around `Net::connect`** — the worker's `handle_round_start` is single-threaded per partition; irrelevant.
- **NR3-003 `TargetSymbolsTooLong` case** — if TASK-0400 shipped the 8th case, add an analogous UT here mirroring UT-0402-01. **Default plan: DEFERRED at TASK-0400, so no test here.**
