# SPEC-REVIEW-19 §3.4 — D-005 Option A Amendment — Adversarial Review

**Date:** 2026-04-23
**Target:** Planned amendment to SPEC-19 §3.4 extending `PendingCommutation` wire encoding with `local_wiring: Vec<(u8, u8, u8, u8)>`.
**Reviewer:** spec-critic (adversarial, Stage 0 of D-005 SDD cycle)
**Predecessors consulted:** SPEC-19 §3.3 (R23, R26, R48, DC-B5, DC-B6), SPEC-19 §3.4 (current R31–R37 + wire structs), `border_resolver.rs` (`CommutationBatch`, `PendingPortRef`, `SLOT_MARKER_BASE`, `encode_request_id`, `emit_external_principal`, `emit_erasure_principal`), `protocol/types.rs` (`Message::RoundStart`, `RoundResult`, `PendingCommutation`), `worker.rs` (`handle_round_start` mint loop), `net/core.rs` (`Net::create_agent`, `Net::connect`), `pipeline-state.md`.

---

## Overall Assessment

The proposed wire schema `local_wiring: Vec<(u8, u8, u8, u8)>` is **fundamentally underspecified against the source-of-truth schema it is supposed to transport**. The resolver emits `Vec<(u8, u8, PortRef)>` — not four `u8`s. A `PortRef` in that vector can carry:

(a) a **slot-marker** reference (`AgentPort(SLOT_MARKER_BASE + sibling_slot, port)`) pointing at a sibling agent in the same `CommutationBatch` whose concrete `AgentId` is not yet assigned at resolver time;
(b) a **concrete local agent** reference (`AgentPort(live_agent_id, port)`) — an agent that already exists in the worker's partition before the commutation;
(c) (in the ERA path) the *target* is produced from `partition.subnet.get_target(...)`, which can in principle return a `FreePort(bid)` — the resolver's current `emit_erasure_principal` converts that to a `PendingNewBorder`, but nothing in the spec prevents a future resolver path from emitting a `FreePort` directly in `local_wiring`.

A `(u8, u8, u8, u8)` layout cannot express (b) at all (AgentId is `u32`, not `u8`), cannot distinguish (a) from (b), and silently loses any future (c). The amendment as sketched will cause CON-DUP and CON-ERA/DUP-ERA to mint agents whose principal port is wired to agent-id `< 256`, which is **an active-pair-producing bug** on any partition where `id_range.start <= 255` (i.e. worker 0 in every test fixture the canary relies on).

This is an architecturally wrong wire shape, not a typo in a payload. It is sign-off-blocking independent of every other finding.

Secondary findings surface around: the missing `Net::wire_agents` method (the D-005 row names it, but it does not exist in the codebase); absence of a slot-to-AgentId substitution rule at the worker; compatibility ordering with `pending_commutations` already in the same `RoundStart`; rkyv archived-schema re-derivation; the `R48` stray-token interaction; the DC-B6 preserve-existing-border interaction; and the missing protocol-version / wire-discriminant versioning statement.

**Verdict:** **BLOCK.** The amendment must be rewritten before Stage 1 task-splitter can consume it.

---

## Findings

### SC-001 — Wire schema cannot encode the resolver's local_wiring (dim a) [CRITICAL]

**Dimension:** (a) bit-width + (e) versioning interaction.
**Location of gap:** planned §3.4 `PendingCommutation { ... local_wiring: Vec<(u8, u8, u8, u8)> }` vs. `border_resolver.rs:358` `local_wiring: Vec<(u8, u8, PortRef)>`.
**Problem:** The third element of the internal tuple is a `PortRef`, not a pair of `u8`s. `PortRef` is `AgentPort(AgentId, u8) | FreePort(u32)`, where `AgentId` is `u32`. The resolver emits THREE distinct target categories inside `local_wiring`:
1. **Sibling slot placeholder** (`AgentPort(SLOT_MARKER_BASE + sibling_slot, port)`) — see `border_resolver.rs:820-821` (`con_wiring.push((SLOT_P, 1, PortRef::AgentPort(slot_marker(SLOT_R), 1)))`).
2. **Live local agent** (`AgentPort(existing_id, port)`) — see `emit_erasure_principal` at `border_resolver.rs:1037` (`local_wiring.push((era_slot, 0, target))` where `target` was just read via `partition_non_era.subnet.get_target(...)` and is an `AgentPort(some_live_agent_u32, u8)` case). Also `emit_external_principal` on the worker_con/worker_dup path (called at `border_resolver.rs:773-812`).
3. (Latent) **FreePort** — currently the resolver routes FreePorts to `pending_new_borders` instead of `local_wiring`, but nothing in the spec pins this invariant; a future resolver fix could legitimately push a `FreePort(bid)` into `local_wiring`, and the wire schema must either accept it or explicitly forbid it.

With a `(u8, u8, u8, u8)` encoding, the two "agent-id-like" fields are one byte each, which means:
- For case (1), `sibling_slot` fits in one byte (hard cap 16 per `encode_request_id` assertion at `border_resolver.rs:319`), but the ENCODING mechanism (slot placeholder vs. concrete id) is lost.
- For case (2), an AgentId `>= 256` CANNOT BE TRANSMITTED. Every real integration fixture has `id_range.start` in the hundreds already (UT-0394-05 uses `minted_agent_id = 100`; UT-0394-06 uses 100, 101, 102). The `live_agent_id` on the same worker is typically in the same numeric bracket. Truncation of a `u32` AgentId to `u8` would **silently re-route the new agent's port onto agent id `live_id & 0xFF`**, introducing exactly the kind of invisible cross-wiring that canonicalization would catch too late.

**Impact if unresolved:** The canary (`SKIP_ASYMMETRIC = false` on all 6 fixtures × 2 strict modes) would fail on the first asymmetric case with `live_agent_id > 255`, and in the worst case could pass on a toy test (say, AgentId `== 100` fits in `u8`) but fail catastrophically in Phase 3 LAN where partitions carry millions of agents. This is the exact "G1 claim apoiado em código quebrado" risk the context message warns about.

**Mandatory resolution:** The wire encoding MUST mirror the resolver's schema, not invent a narrower one. Two acceptable shapes:

**Shape A (preferred — symmetrical):**
```rust
pub struct PendingCommutation {
    pub request_id: u32,
    pub symbol_type: Symbol,
    pub arity: u8,
    pub local_wiring: Vec<LocalWiringHint>,
}
pub struct LocalWiringHint {
    /// Which minted slot inside this PendingCommutation's sibling group.
    /// Range [0, 16) — enforced by DC-B5 CommutationBatch.target_symbols cap.
    pub src_slot: u8,
    /// Port on the minted agent: 0 = principal, 1..=arity = aux.
    pub src_port: u8,
    /// The target port. Sibling placeholders use AgentPort(SLOT_MARKER_BASE + sibling_slot, port);
    /// concrete local agents use AgentPort(live_agent_id, port); FreePort is reserved and MUST be
    /// rejected with a ProtocolError until a future amendment extends the semantics.
    pub target: PortRef,
}
```
This keeps the resolver-to-wire transport a pure `From<(u8, u8, PortRef)>` conversion, preserves the SLOT_MARKER_BASE mechanism already reserved by R48, and survives AgentIds of any `u32` magnitude.

**Shape B (tagged-union, more verbose but no slot-marker aliasing):**
```rust
pub struct LocalWiringHint {
    pub src_slot: u8,
    pub src_port: u8,
    pub target: LocalWiringTarget,
}
pub enum LocalWiringTarget {
    Sibling { slot: u8, port: u8 },
    ConcreteAgent { agent_id: AgentId, port: u8 },
    // FreePort variant deliberately omitted — add in a future revision if required.
}
```
Shape B is more defensive (removes the SLOT_MARKER_BASE aliasing trick from the wire), at the cost of a discriminator byte per entry.

Both shapes MUST be paired with a spec-level note pinning which of (1)/(2)/(3) the resolver is currently allowed to emit, and an explicit R-requirement that extension to (3) is a future wire break.

---

### SC-002 — `Net::wire_agents` method named in the amendment does not exist [CRITICAL]

**Dimension:** (f) order of application + completeness axis.
**Location of gap:** DEFERRED-WORK.md D-005 row line 107: "`partition.subnet.wire_agents(PortRef::AgentPort(minted_id_a, port_a), PortRef::AgentPort(minted_id_b, port_b))`". Repeated in pipeline-state.md active-bundle description.
**Problem:** `Net` has `connect(&mut self, a: PortRef, b: PortRef)` (at `net/core.rs:177`). There is no `wire_agents` method on `Net`, `Subnet`, or `Partition`. A Stage-1 task-splitter reading the amendment will either (i) invent a new method (blowing the surgical scope), or (ii) fall through to `connect`, which has a `debug_assert_ne!(a, b, ...)` that will fire on any degenerate case.

Further: `connect` has two semantics that `wire_agents` is NOT documented to inherit:
- It unconditionally registers principal-pair redexes (`net/core.rs:198-200`). If a minted CON-DUP agent's principal port `p.0` is wired via `local_wiring` to another minted agent's principal port (e.g. a self-loop residue from a malformed resolver emission), `connect` will push a new redex into the local queue, which `reduce_all` (called immediately after the mint loop in `worker.rs:452`) will process — potentially violating R24.1.6's contract that minted agents "MUST NOT treat the minted agents as reducible in the round in which they are created if their principal port is a `FreePort(new_bid)`" (R23 last sentence). The R23 clause gates on FreePort, but the `local_wiring` path can wire principal-to-principal between siblings, and no spec text says what happens then.
- It has a FreePort-redirect bookkeeping branch (`net/core.rs:186-194`). If a hypothetical future (3) FreePort target arrives via `local_wiring`, this path silently activates.

**Mandatory resolution:**
1. The amendment MUST name `Net::connect` (or define a new primitive and add it as part of this ticket's scope with explicit pre-/post-conditions).
2. The amendment MUST add a requirement that the worker applies `local_wiring` AFTER `alloc_agent × N` so every slot-marker can be resolved, AND BEFORE `reduce_all` in the same round, so D6 progress arguments are preserved.
3. The amendment MUST state whether principal-pair redexes introduced by `local_wiring` are (i) processed in-round (consistent with CON-DUP being the only agent-count-increasing rule), or (ii) deferred (for parity with the FreePort-pinned case in R23). **Pick one and write the test.**

---

### SC-003 — No slot-to-AgentId substitution rule specified [HIGH]

**Dimension:** (c) R48 interaction + (f) order of application + completeness axis.
**Location of gap:** entire amendment sketch — no prose at all on how a worker converts `AgentPort(SLOT_MARKER_BASE + sibling_slot, port)` to `AgentPort(concrete_minted_id, port)`.
**Problem:** The resolver emits slot-marker placeholders (SC-001 case (1)) precisely because it does not know the worker's `IdRange`. The spec must specify the substitution algorithm:
1. Order-dependence: the worker allocates agents in `pending_commutations` order (`worker.rs:441` — `create_agent` is sequential, incrementing `next_id`). So `minted_agents[k].minted_agent_id = id_range.start + k` (modulo exhaustion). But `local_wiring` slots are **intra-batch** (one `PendingCommutation` = one batch = one `CommutationBatch`), not across all `pending_commutations` in the round. Nothing in the current amendment sketch says slots are local to THIS `PendingCommutation`; a Stage-1 reader could reasonably interpret them as round-global indices, which would break on any round with two batches.
2. Cross-batch referencing: is `local_wiring` allowed to reference a slot in a DIFFERENT `PendingCommutation` within the same `RoundStart`? The resolver's `SLOT_MARKER_BASE + slot` encoding is SHARED across batches (same constant) — if the worker sees `AgentPort(SLOT_MARKER_BASE + 1, 2)` in batch A's `local_wiring`, is slot 1 in batch A or batch B?  Source-of-truth code at `border_resolver.rs:767-866` shows the resolver treats it as local-to-THIS-batch (`con_wiring` and `dup_wiring` are disjoint `Vec`s each with their own slots). The spec must pin this explicitly.
3. Validation path: if `src_slot >= target_symbols.len()` or the decoded sibling slot references a slot beyond the declared mint count, the worker MUST reject with `ProtocolError::MalformedLocalWiring` (or similar, see SC-007).

**Mandatory resolution:** Add an R-requirement (suggest R23a or equivalent) stating:
- `local_wiring` slots are local to the containing `PendingCommutation` (not the round).
- Worker MUST allocate all `arity`-many agents for a given `PendingCommutation` before applying ANY entry of that PC's `local_wiring`. (Fixes the forward-reference case in SC-002 item 2.)
- Substitution rule: for every `PortRef::AgentPort(x, p)` in `local_wiring` where `x >= SLOT_MARKER_BASE`, replace `x` with `minted_agents[x - SLOT_MARKER_BASE].minted_agent_id` on the per-PC mint vector (NOT on the round-global vector).
- For every `PortRef::AgentPort(x, p)` where `x < SLOT_MARKER_BASE`, pass-through as a concrete live AgentId; the worker MUST verify the agent exists in its partition (optional: debug-only assert) to catch resolver bugs.

---

### SC-004 — Vector ordering / determinism unspecified [HIGH]

**Dimension:** (b) ordering preserves G1 determinism.
**Location of gap:** amendment sketch has no ordering requirement on `local_wiring`.
**Problem:** G1 under delta mode (R38) requires `canonicalize(out_delta) == canonicalize(out_v1)`. Canonicalization runs on the final merged net, so *intermediate* order-of-wiring inside `Net::connect` calls does not change final structure — **as long as `connect` is commutative-modulo-order for all emitted tuples**. It is, because `connect` just stores bidirectional port-array entries (`net/core.rs:180-181`). However:
- The principal-pair redex-queue insertion at `net/core.rs:198-200` uses `push_back` in the insertion order. If `reduce_all` later picks off redexes in queue order, different `local_wiring` orderings would traverse the local reduction in different orders, and while T4 (strong confluence) guarantees the same final net, the `interactions_by_rule` and `local_redexes` STATS would diverge — and the canary gate checks **`metrics.total_interactions` parity** on every case. Two runs with the same input but different `local_wiring` ordering could produce parity mismatches if they disagree on *which* reductions happened locally vs. at the coordinator.
- The `freeport_redirects` HashMap insert at `net/core.rs:186-194` is stable under ordering for FreePort–FreePort connects (currently not emitted in `local_wiring`, but see SC-001 case (3)).

**Mandatory resolution:** Add an R-requirement: "the order of entries in `local_wiring` MUST be the order the resolver emitted; the worker MUST apply them sequentially in that order; the wire encoding MUST preserve order (bincode `Vec` already does — this is just a spec pin). Reordering is a protocol violation."

Alternatively, the spec can *relax* to unordered if the canary evidence confirms zero metric drift across permutations — but that requires a generative test and is not in the current scope.

---

### SC-005 — Error path not defined [HIGH]

**Dimension:** (g) error path.
**Location of gap:** no `ProtocolError` variant named for malformed `local_wiring`.
**Problem:** The amendment must specify what the worker does when `local_wiring` is internally inconsistent. Cases:
1. `src_slot >= arity` within the owning PC (the slot refers to an unminted agent).
2. `src_port > arity_of_symbol_at_slot` (port index out of range for the symbol).
3. Sibling-slot placeholder target `sibling_slot >= arity`.
4. Target `AgentPort(live_id, p)` where `live_id` is NOT in the worker's partition (dangling reference).
5. Duplicate `(src_slot, src_port)` keys — two entries trying to write the same port (connect would last-write-wins silently).

Without a documented error path, the Stage-1 developer will `unwrap` or silently skip, both of which are unacceptable in a paper-backing codepath. Precedent: R48 explicitly handles stray-token duplicates with a `debug_assert` + lenient mint (see UT-0394-10).

**Mandatory resolution:** Add an R-requirement naming a new error variant (suggest: `ProtocolError::MalformedLocalWiring { request_id, reason }`), and enumerate the 5 cases above as either MUST-reject (1, 2, 3, 5) or SHOULD-warn (4, for symmetry with the resolver's own "dangling FreePort" lenient path at `border_resolver.rs:1061-1077`).

---

### SC-006 — rkyv archived-schema regeneration not addressed [HIGH]

**Dimension:** (d) rkyv zero-copy compatibility.
**Location of gap:** amendment sketch has no mention of `Archive`/`Serialize`/`Deserialize` derive macros or the `--features zero-copy` test lane.
**Problem:** `RoundStart` and `RoundResult` are (or will be, per SPEC-18 R20–R27 which shipped as D-002) on the rkyv hot-path. `Vec<LocalWiringHint>` (or `Vec<(u8,u8,u8,u8)>` under the misspecified sketch) needs:
1. `#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]` on the new struct (and, if Shape B, on the tagged enum).
2. Alignment audit: `(u8, u8, u8, u8)` is trivially 1-byte-aligned; `(u8, u8, PortRef)` carries a `PortRef` with a `u32` inside, forcing 4-byte alignment and padding that WILL change `Partition` archive sizes. Q5 (`zero_copy_tests.rs:656`) pins byte-count parity.
3. `bytecheck` / validation path: `rkyv::access` runs `CheckBytes` — new types must pass validation on malformed input. Q1 (`zero_copy_tests.rs:524`) requires `ArchiveValidationFailed` on corrupt root pointers.
4. Test count: pipeline-state.md line 15 demands `--features zero-copy` ≥ 1186 on close. New hot-path fields add at least 2 tests (round-trip + corrupt-bytes-rejected).

**Mandatory resolution:** The amendment MUST explicitly state that `LocalWiringHint` (or the chosen shape) implements `Archive + Serialize<…> + Deserialize<…, bytecheck::Strategy<…>>`, that R34 (serde/bincode + CRC32) extends to cover the new struct, and that R35's "wire optimization" exemption for `InitialPartition`/`FinalStateResult` does NOT apply to `local_wiring` (which lives on the HOT path inside `RoundStart.pending_commutations`).

---

### SC-007 — R48 interaction with slot-marker targets is underspecified [MEDIUM]

**Dimension:** (c) R48 stray-token guard.
**Location of gap:** R48 governs stray `request_id` echoes on `MintedAgent`. It says nothing about stray slot-marker references inside `local_wiring`.
**Problem:** If the resolver emits `local_wiring: [(0, 1, AgentPort(SLOT_MARKER_BASE + 5, 2))]` on a batch with `target_symbols.len() == 2` (slot 5 does not exist), the worker currently has no defined behavior. The symmetric `encode_request_id` assertion (`border_resolver.rs:318-322`) guards at the resolver-output boundary, but the WIRE layer has no such guard.

Interaction with SC-003 #3 (cross-batch referencing): if the spec allows cross-batch refs (it shouldn't), R48 must be amended to say so; otherwise R48's current wording ("Each worker MUST allocate from its own reserved `id_range`") gives no hint about slot-marker validity.

**Suggested resolution:** Add a clause to R48 (or a new R48a): "Worker MUST reject a `PendingCommutation` whose `local_wiring` contains an `AgentPort(SLOT_MARKER_BASE + s, _)` with `s >= target_symbols.len()` (SC-005 case 3) as a `ProtocolError::MalformedLocalWiring`; this is symmetric to the coordinator's R48 stray-token guard on `MintedAgent.request_id`."

---

### SC-008 — DC-B6 preserve-existing-border path not crossed with local_wiring [MEDIUM]

**Dimension:** (h) DC-B6 interaction.
**Location of gap:** the CON-ERA / DUP-ERA `emit_erasure_principal` (`border_resolver.rs:1023-1080`) puts non-AgentPort targets into `pending_new_borders`, but the spec needs to explicitly say "a minted agent's port is NEVER wired directly to a `FreePort(bid)` via `local_wiring`; all FreePort hops surface as `PendingNewBorder`." This is currently implicit in the resolver code; the wire contract should pin it.
**Problem:** If a future resolver change (or a test-only resolver mock) emits `(slot, port, FreePort(bid))` inside `local_wiring`, the worker would call `connect(AgentPort(minted, p), FreePort(bid))`, which is a legitimate `connect` path (`net/core.rs:186-194` even has bookkeeping for it), but the BorderGraph at the coordinator would NOT know about this new reference — the `FreePort` would be dangling from the coordinator's perspective. D-004's `register_minted_agents` + `PendingNewBorder` promotion logic handles only the `PendingPortRef::Pending` case.

**Suggested resolution:** Add an R-requirement: "`local_wiring` entries where the target is `PortRef::FreePort(_)` are a protocol violation. All border-crossing wires MUST be emitted as `PendingNewBorder` on the containing `BorderResolution`, never as `local_wiring`." Accept via `ProtocolError::MalformedLocalWiring { reason: ReservedForFuture }`.

This also pins the SC-001 case (3) answer cleanly: "reserved; reject."

---

### SC-009 — No protocol version / discriminant stability statement [MEDIUM]

**Dimension:** (e) versioning.
**Location of gap:** amendment sketch in DEFERRED-WORK.md D-005 row has no version-bump note. The repo has `PROTOCOL_VERSION` (2 per `protocol/coordinator.rs:570`) and SPEC-19 R37 says "MUST coordinate with SPEC-18" but does not require a version bump on enum-payload growth.
**Problem:** Adding `local_wiring: Vec<LocalWiringHint>` to `PendingCommutation` is a bincode-compatible ADDITION only if `Vec` is the trailing field and bincode reads it as the remainder of the payload. With serde+bincode the struct DOES decode old-format (pre-amendment) `PendingCommutation` bytes to yield the new-format struct with `local_wiring: Vec::new()` **ONLY IF** a migration strategy is explicit. Bincode's default behavior on a missing field is to return a deserialization error.

The relevant tests are TASK-0368 T6 (existing PC round-trip at `protocol/types.rs:779`) and the spec clause at R34.

**Suggested resolution:** The amendment MUST state one of:
(a) Bump `PROTOCOL_VERSION` from 2 to 3 (cleanest; HandshakeAck rejection at `protocol/coordinator.rs:66-81` already handles mismatch).
(b) Frame the new field as a trailing `Vec`, and add an R-requirement that bincode-v2 handles trailing-optional via a versioned prefix byte (if SPEC-18 already reserved one — verify).
(c) Freeze v2 and introduce `Message::PendingCommutationV2` on a new discriminant. (Overkill for scope.)

Option (a) is simplest; no prior coordinator or worker in production (v1 frozen; v2 is dev-branch); cost is bumping one constant. The spec should name it.

---

### SC-010 — Pairing with `MintedAgent` echo not restated [MEDIUM]

**Dimension:** consistency axis (cross-reference within §3.4).
**Location of gap:** R26 describes `minted_agents` as the echo response; the amendment's `local_wiring` application changes the semantics of what "echo" means (post-wiring, not post-mint-only).
**Problem:** A coordinator reading `MintedAgent { request_id, minted_agent_id }` receives the id BEFORE `reduce_all` runs on the worker. If `local_wiring` wires a minted agent's principal port to another minted agent's principal port, a redex exists and may consume the minted agent within the same round. The `MintedAgent` echo then references an agent id that may already be dead (garbage-collected or marked consumed). D-004 TASK-0399's `register_minted_agents` assumes the id is live when it receives it.

**Suggested resolution:** Add an R-requirement stating the order inside R24's step list (currently R24.1.6 → R24.2 = reduce_all):
- R24.1.6a: mint all agents in pending_commutations (existing).
- R24.1.6b (NEW): apply `local_wiring` for every PC, in order, for every PC (sibling-slot substitution per SC-003).
- R24.1.6c (NEW): build `minted_agents` vector **from the ids allocated in 1.6a**, regardless of whether reduction in 1.6d-equivalent consumes them. This pins the semantics: "echo the *minted* id, not the *surviving* id."
- R24.2: reduce_all (existing).

The alternative — reduce before emitting the echo — would require D-004's `register_minted_agents` to tolerate "id was minted then immediately consumed," which is currently untested and adds failure modes.

---

### SC-011 — Empty-`local_wiring` semantics not pinned [LOW]

**Dimension:** completeness.
**Problem:** R26 pins `minted_agents.is_empty()` to `pending_commutations.is_empty()`. There's no corresponding rule for `local_wiring.is_empty()`. An ERA mint on a CON-ERA/DUP-ERA resolution may legitimately emit an empty `local_wiring` (if both of the consumed agent's aux targets were FreePorts — see `emit_erasure_principal` `FreePort(_)` branches). This should be spec-text, not implicit.
**Suggested resolution:** Add a one-line note: "`local_wiring` MAY be empty; an empty `local_wiring` means the minted agent(s) have no intra-worker edges to apply, and the worker MUST NOT treat empty as an error."

---

### SC-012 — Test-count baseline in pipeline-state.md not yet updated for new UTs [LOW]

**Dimension:** completeness (procedural).
**Problem:** pipeline-state.md line 15 sets the canary at 1146/1186. The amendment will add: (a) wire round-trip (+1 / +1 zero-copy), (b) rkyv validation test (+1 zero-copy only), (c) slot-substitution UT (+1), (d) malformed-wiring rejection UTs (+2–3), (e) canary flip `SKIP_ASYMMETRIC=false` (no count change). The spec doesn't set a new target, but Stage 1 will need one.
**Suggested resolution:** When the amendment lands, bump the pipeline-state baseline to 1146+5 = 1151 default / 1186+6 = 1192 zero-copy as a minimum. Not a spec change — just a task-splitter input.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH     | 4 |
| MEDIUM   | 4 |
| LOW      | 2 |
| **Total** | **12** |

### Mandatory (must fix before Stage 1 task-splitter):
- **SC-001** — Rewrite `local_wiring` wire shape to preserve the resolver's `(u8, u8, PortRef)` triple (Shape A preferred). Drop `(u8, u8, u8, u8)`.
- **SC-002** — Name `Net::connect` (or introduce and spec a new primitive explicitly); specify worker-side ordering: alloc all → apply `local_wiring` → `reduce_all`.
- **SC-003** — Specify slot-to-AgentId substitution: local to each PC, placeholder decode rule, validation.
- **SC-004** — Pin ordering: entries MUST be applied in emitted order; wire MUST preserve order.
- **SC-005** — Define `ProtocolError::MalformedLocalWiring` (or equivalent) and enumerate rejection cases.
- **SC-006** — State rkyv `Archive`/`Serialize`/`Deserialize` requirements; align with SPEC-18 Q1/Q5; bump `--features zero-copy` test baseline.

### Recommended (should fix):
- **SC-007** — R48 amendment for stray slot-marker tokens.
- **SC-008** — Explicitly forbid `FreePort` targets in `local_wiring`.
- **SC-009** — Bump `PROTOCOL_VERSION` to 3.
- **SC-010** — Pin echo semantics: `minted_agents` carries minted-time ids, not post-reduction ids.

### Nit (optional):
- **SC-011** — Pin empty-`local_wiring` as legal.
- **SC-012** — Update pipeline-state baseline on next revision.

---

## Verdict: **BLOCK**

The amendment as sketched is structurally incorrect (SC-001) and incomplete against its own source-of-truth. Gate condition (0 CRITICAL + 0 HIGH) is NOT met (2 CRITICAL + 4 HIGH).

**Required next step:** especialista-em-specs (root-layer TCC agent) redrafts the amendment addressing all six MANDATORY items above, then this review is re-run. Expected redraft size: ~60 lines of spec prose + 1 new wire struct + 1 new error variant + 1 R-requirement revision (R23 or new R23a) + 1 R-requirement new (R48a) + protocol-version bump note. Estimated spec-critic re-review time: ~1 hour after redraft.

The canary flip (`SKIP_ASYMMETRIC = false`) and the G1-asymmetric parity claim that backs the TCC thesis depend on this wire contract being right the first time. Precedent (SPEC-06 revision 2026-04-05: 2 CRITICAL + 3 HIGH on similarly "surgical" wire edits) strongly validates the sdd-pipeline decision to make Stage 0 mandatory here.
