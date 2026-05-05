# SPEC-REVIEW-19 §3.4 D-005 — Re-Review (Round 2)

**Date:** 2026-04-23
**Target:** SPEC-19 §3.3 R23/R23a/R24, §3.4 R31–R37, §3.6 R48/R48a/R48b (post-redraft)
**Predecessors consulted:** Round 1 review + redraft in `specs/SPEC-19-delta-protocol.md`
**Source files cross-checked:** `border_resolver.rs:310-360, 760-872, 1020-1080`, `protocol/coordinator.rs:60-86`, `net/core.rs:177-200`

---

## 1. Status of Round-1 findings

| ID | Severity | Verdict | Evidence |
|---|---|---|---|
| **SC-001** wire schema | CRITICAL | **CLOSED** | §3.4 R33 defines `LocalWiringHint { src_slot: u8, src_port: u8, target: PortRef }` matching resolver `(u8,u8,PortRef)` 1-to-1. Docstring at lines 267-272 explicitly prohibits `(u8,u8,u8,u8)` with rationale. Three target categories (a)/(b)/(c) documented at lines 282-294. |
| **SC-002** `Net::wire_agents` | CRITICAL | **CLOSED** | R33b (line 309) names `Net::connect` as the sole sanctioned primitive and explicitly forbids introducing `Net::wire_agents` in Stage 1. R24.1.6b (line 160) repeats the prohibition. |
| **SC-003** slot→AgentId substitution | HIGH | **CLOSED** | R23a clauses 1-5 (lines 142-146) cover per-request scope, mint-then-wire order, slot-marker decoding with `(x - SLOT_MARKER_BASE) >= arity` guard, concrete-id pass-through, and in-round reducibility. |
| **SC-004** vector ordering | HIGH | **CLOSED** | "R23a determinism guarantee" paragraph (line 148) pins emitted-order application, cites `bincode` `Vec<T>` order preservation, and names R33c case 5 as the detection path. R24 ordering invariant (line 167) reinforces at the step level. |
| **SC-005** error path | HIGH | **CLOSED** | R33c (lines 311-344) introduces `ProtocolError::MalformedLocalWiring { request_id, reason }` with `MalformedLocalWiringReason` enum enumerating 6 cases (cases 1/2/3/5/6 hard-reject, case 4 SHOULD-warn). Line 342 pins NACK behavior and coordinator response. |
| **SC-006** rkyv compatibility | HIGH | **CLOSED** | R34 (line 346) requires `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, ...))]` on `PendingCommutation`, `LocalWiringHint`, `MintedAgent`; notes 4-byte alignment forced by `PortRef`'s `u32`; pins zero-copy baseline ≥ 1192 with enumerated minimum of 6 new UTs. |
| **SC-007** R48 stray slot-marker | MEDIUM | **CLOSED** | R48a (line 360) mirrors R48 at the wire-layer boundary and binds to R33c case 3. |
| **SC-008** FreePort in `local_wiring` | MEDIUM | **CLOSED** | Target category (c) in R33 docstring (line 290) is RESERVED; R33c case 6 `ReservedForFuture { border_id }` is a MUST-reject; R48a references SC-008. |
| **SC-009** PROTOCOL_VERSION bump | MEDIUM | **CLOSED** | R37 (line 352) mandates bump 2→3, cites existing `HandshakeAck` rejection path, justifies zero production cost. |
| **SC-010** echo semantics | MEDIUM | **CLOSED** | R24.1.6c (line 161) pins "echoed id is the mint-time id (pre-reduction)"; `MintedAgent.minted_agent_id` docstring (line 304) reinforces. |
| **SC-011** empty `local_wiring` | LOW | **CLOSED** | R48b (line 364) makes empty `local_wiring` explicitly legal. |
| **SC-012** zero-copy baseline | LOW | **CLOSED (spec-side)** | R34 pins ≥ 1192; procedural update of `pipeline-state.md` remains but is not a spec deliverable (see NF-005 below). |

**Summary:** 12/12 original findings CLOSED. No Round-1 finding remains OPEN.

---

## 2. New findings introduced by the redraft

### NF-001 — `target_symbols` cannot be reconstructed from `(symbol_type, arity)` on the wire [CRITICAL]

**Axis:** Completeness + Consistency with resolver source-of-truth.
**Location:** §3.4 R33 `PendingCommutation.symbol_type: Symbol` / `arity: u8` (lines 247-253); §3.3 R24.1.6a (line 159).

**Problem.** The resolver emits `CommutationBatch.target_symbols: Vec<Symbol>` with heterogeneous contents. For every CON-DUP resolution, `target_symbols == vec![Symbol::Dup, Symbol::Con]` (verified at `border_resolver.rs:859` and `:865`). The wire-format `PendingCommutation` carries **only** `symbol_type: Symbol` (documented at lines 246-248 as "slot 0 of the batch") plus `arity: u8`. **The wire discards slot 1..N symbols.**

R24.1.6a (line 159) handwaves: *"the symbol of slot k is implicit in `pc.symbol_type` when `pc.arity == 1` and reconstructed by matching the sibling's `src_slot` against the batch layout for multi-sibling CON-DUP/CON-ERA/DUP-ERA."*

This is **not a reconstruction rule** — it is a pointer to rules that do not exist on the wire. The "batch layout" is defined in `border_resolver.rs` source code, not in the spec and not on the wire. Two concrete consequences:

1. **CON-DUP worker cannot know slot-1 symbol.** Given `(symbol_type=Dup, arity=2)`, the worker must mint slot 0 as Dup and slot 1 as… Dup? Con? The correct answer (`Con`) is only obtainable by reading resolver source. A Stage-1 task-splitter handed this spec will either (a) mint slot 1 as Dup (silent miswiring), (b) add a hardcoded rule `symbol_type == Dup → slot1 = Con`, which leaks resolver implementation into the wire contract.
2. **CON-ERA/DUP-ERA divergence.** For ERA mints, `target_symbols == vec![Symbol::Era]` with `arity == 1` (single sibling). For CON-DUP, `arity == 2` with heterogeneous symbols. The wire cannot distinguish whether `arity == 2` + `symbol_type == Con` means `[Con, Dup]` (CON-DUP path) or `[Con, Con]` (future symmetric mint path). This is exactly the kind of implicit-contract bug SC-001 was opened to prevent.

**Why SC-001's fix does not cover this:** SC-001 fixed the *target* format of `LocalWiringHint`. NF-001 is about the *source* symbols of the minted siblings, which is a separate payload field (`symbol_type`/`arity`) that was untouched by the redraft.

**Mandatory resolution.** One of:

**Shape A (preferred — mirror `target_symbols`):**
```rust
pub struct PendingCommutation {
    pub request_id: u32,
    pub target_symbols: Vec<Symbol>,  // replaces symbol_type + arity
    pub local_wiring: Vec<LocalWiringHint>,
}
```
`arity` becomes `target_symbols.len()`. The worker `create_agent`s from `target_symbols[k]` directly. Wire cost: `Vec<Symbol>` header is 8 bytes (bincode len prefix) + 1 byte per slot, vs. 1+1=2 bytes today for `(symbol_type, arity)`. For a typical CON-DUP batch (arity=2), this is +7 bytes per PC. Given PCs are already on the hot path and a typical round carries single-digit PCs, this is negligible.

**Shape B (preserve current fields, add `slot_symbols` side-channel):**
```rust
pub struct PendingCommutation {
    pub request_id: u32,
    pub symbol_type: Symbol,          // kept for back-compat / slot 0
    pub arity: u8,
    pub slot_symbols: Vec<Symbol>,    // NEW: symbols for slots 1..arity (slot 0 = symbol_type)
    pub local_wiring: Vec<LocalWiringHint>,
}
```
Worker reconstructs `target_symbols[0] = symbol_type`, `target_symbols[k] = slot_symbols[k-1]` for k ≥ 1.

Shape A is cleaner; Shape B is smaller in the `arity == 1` (ERA) case where `slot_symbols` is empty. Either way, **the rule that maps wire fields to per-slot Symbols MUST be written in R24.1.6a, not deferred to "the batch layout."**

**Impact if unresolved.** Exactly the G1-break class the canary flip is supposed to catch: worker mints `[Dup, Dup]` when resolver intended `[Dup, Con]`, wire's `local_wiring` then connects ports referring to a sibling-slot that holds the wrong symbol, canonicalize diverges from v1 on every CON-DUP-touching fixture. This is SC-001 severity and blocks Stage 1 identically.

---

### NF-002 — HandshakeAck rejection is unidirectional; worker-side mismatch handling undefined [HIGH]

**Axis:** Consistency + Completeness.
**Location:** §3.4 R37 (line 352).

**Problem.** R37 cites `HandshakeAck` rejection at `protocol/coordinator.rs:66-81` — verified: the coordinator NACKs a worker whose `payload.protocol_version != PROTOCOL_VERSION`. However:

1. **Worker-side is not spec'd.** What happens when a coordinator running `PROTOCOL_VERSION=3` sends a `PendingCommutation` (with `local_wiring`) to a worker running `PROTOCOL_VERSION=2`? The v2 worker's bincode decode will fail on the new trailing `Vec<LocalWiringHint>` field (as R37 itself notes: "bincode does NOT tolerate missing trailing fields"). But bincode decode failure on a mid-session message is NOT the same as the handshake NACK path — it surfaces as a generic `ProtocolError::DeserializationFailed` mid-round, which the worker's FSM does not have a well-defined recovery for.
2. **Register handshake mismatch direction.** The `PROTOCOL_VERSION` check fires on `Message::Register` sent by the worker. If worker v2 (version 2) connects to coordinator v3 (version 3), coordinator NACKs — good. If worker v3 (version 3) connects to coordinator v2 (version 2) — coordinator v2's `PROTOCOL_VERSION` constant is 2, worker sends 3, NACK fires — good. So the handshake IS bidirectional in practice. **But R37 does not state this**; a Stage-1 reader could over-index on "the existing rejection path handles version mismatch" and miss that this hinges on both sides checking against their own `PROTOCOL_VERSION` constant.

**Mandatory resolution.** Add a sentence to R37: "The version check fires symmetrically: a worker connecting with `protocol_version == N` to a coordinator whose `PROTOCOL_VERSION == M != N` is rejected during `Register` regardless of which side is older. No v2 coordinator can reach `PendingCommutation` decode with a v3 worker (or vice versa) because `Register` is the first message of every session." This is prose-only, ~2 lines.

**Why HIGH not CRITICAL.** In practice the handshake blocks the bad path; the risk is only that an implementer misreads R37 and adds defensive decode-retry code mid-round. Catchable in review.

---

### NF-003 — R33c case 5 detection site unspecified [MEDIUM]

**Axis:** Completeness.
**Location:** §3.4 R33c case 5 (line 330-333); §3.3 R23a determinism guarantee (line 148).

**Problem.** R33c case 5 says *"duplicate `(src_slot, src_port)` keys in the same request … `Net::connect`'s last-write-wins semantics would silently mask this."* Correct diagnosis, but the spec does not say **where** to detect. Options:

1. Before the mint-then-wire loop: build a `HashSet<(u8, u8)>` over `pc.local_wiring` and reject if insertion collides.
2. Inside the wire loop: maintain the `HashSet` incrementally during R24.1.6b.
3. At decode time: extend bincode validation to reject.

Option 3 is wrong (bincode layer is symbol-unaware). Option 1 or 2 are both valid; spec should name one so test expectations are deterministic. UT for case 5 cannot be written without this.

**Suggested resolution.** Add to R23a clause 2 or as a new clause 6: "The worker MUST maintain a `HashSet<(u8, u8)>` of `(hint.src_slot, hint.src_port)` tuples across the entire `pc.local_wiring` vector; on insert collision, the worker MUST reject with `MalformedLocalWiringReason::DuplicateSourcePort` BEFORE the first call to `Net::connect`. This prevents partial application of a malformed batch." One paragraph.

---

### NF-004 — `arity == 0` edge case not covered [MEDIUM]

**Axis:** Completeness + Testability.
**Location:** §3.4 R33 (line 253 `arity: u8`); R33c cases 1-3 (lines 316-324).

**Problem.** All R33c guards assume `arity > 0`. Concretely:

- Case 1 (`SrcSlotOutOfRange { src_slot, arity }`): `src_slot < arity`. If `arity == 0`, every `src_slot` (u8) is ≥ arity — so a `PendingCommutation { arity: 0, local_wiring: [<any entry>] }` triggers case 1. But is `arity == 0` itself legal? It would correspond to "allocate 0 siblings" which is a degenerate emission the resolver never produces today but the spec does not forbid.
- R48b makes empty `local_wiring` explicitly legal. It does NOT make empty `target_symbols`/`arity == 0` explicitly legal or illegal.
- `MintedAgent` echo references `minted_ids_per_pc[0]` (R24.1.6c, line 161). If `arity == 0`, `minted_ids_per_pc` is empty and the echo indexes out of bounds.

**Suggested resolution.** Add to R33: *"`arity == 0` is a protocol violation; worker MUST reject with a new `MalformedLocalWiringReason::ZeroArity` (or reuse case 1 with `arity=0, src_slot=0`). Every `PendingCommutation` MUST mint ≥ 1 sibling."* Or, if future resolver paths might want `arity == 0` as a no-op, make it explicitly legal AND redefine `MintedAgent` echo to skip zero-arity PCs AND amend R48b symmetrically to R26.

Either choice is fine; the spec must pick one. Current redraft leaves the reader to guess.

---

### NF-005 — `pipeline-state.md` still reflects pre-redraft narrative [LOW — procedural]

**Axis:** Procedural (not a spec defect).
**Location:** `docs/pipeline-state.md:3` and `:11`.

**Observation.** The file still says "Entering **Stage 0 (spec-critic)** on §3.4 amendment before Stage 1 SPLITTING" and "Stage: 0 — SPEC-CRITIC (adversarial review of SPEC-19 §3.4 amendment, pre-SPLITTING)". The redraft has landed; this review closes Stage 0. The sdd-pipeline agent owns that file per the in-file header — **not the spec-critic** — but flag this because a task-splitter invoked next with stale state may either re-run Stage 0 or misread the bundle scope.

**Suggested resolution.** On sign-off, the sdd-pipeline agent updates `pipeline-state.md` to Stage 1 (task-splitter) with the redraft + this review as inputs. Not a spec change.

---

## 3. Severity tally (new findings only)

| Severity | Count |
|---|---|
| CRITICAL | 1 (NF-001) |
| HIGH | 1 (NF-002) |
| MEDIUM | 2 (NF-003, NF-004) |
| LOW | 1 (NF-005, procedural) |
| **Total new** | **5** |

Combined with Round 1 closure (12/12 CLOSED), the net open findings are the 5 above.

---

## 4. Verdict: **BLOCK**

**Gate check:**
- ≥1 CRITICAL new (NF-001) → **BLOCK** condition met.
- ≥2 HIGH new (only 1 present) → not triggered.
- ≥1 original STILL OPEN at HIGH+ → not triggered (all 12 CLOSED).

**Summary.** The redraft resolves every Round-1 finding cleanly and precisely; the author fixed the wire-shape, ordering, substitution, error-path, rkyv, versioning, and nit-level items exactly as requested. However, the redraft introduces a **structurally symmetric problem on the mint-source side**: where Round 1 caught that `LocalWiringHint.target` could not carry a `u32` AgentId through `u8`, Round 2 must catch that `target_symbols` cannot be reconstructed from `(symbol_type, arity)` when slots hold heterogeneous symbols (Dup/Con in every CON-DUP batch). Both are the same class of bug (wire payload loses information the resolver emits) and NF-001 inherits the same blast radius — a wrong slot-1 symbol wires the minted agent's port to an agent of the wrong type, canonicalize diverges on the canary flip, and the G1-asymmetric parity claim supporting the TCC thesis fails on broken code.

**Required next step.** Return to especialista-em-specs (root-layer TCC agent, separate session) with these 5 NFs. Expected redraft size:

- **NF-001:** ~15-20 lines — extend `PendingCommutation` (Shape A: replace `symbol_type`/`arity` with `target_symbols: Vec<Symbol>`; or Shape B: add `slot_symbols`). Rewrite R24.1.6a reconstruction clause to reference the wire field, not resolver source. Amend R33 docstrings. Add 1 line to R34 zero-copy audit (tiny alignment impact, `Symbol` is 1 byte).
- **NF-002:** ~2 lines in R37.
- **NF-003:** ~3 lines (new clause in R23a or new R33d).
- **NF-004:** ~3 lines (either R33 prose rejecting `arity == 0` or R48b extension legalizing it).
- **NF-005:** sdd-pipeline housekeeping, out of spec-critic scope.

Estimated re-review time after next redraft: ~30 minutes. NF-001 is the only structural change; NF-002/003/004 are prose-only.
