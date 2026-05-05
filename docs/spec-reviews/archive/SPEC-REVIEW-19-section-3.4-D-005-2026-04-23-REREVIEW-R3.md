# SPEC-REVIEW-19 §3.4 D-005 — Re-Review (Round 3)

**Date:** 2026-04-23
**Target:** SPEC-19 §3.3 R23/R23a/R24, §3.4 R31–R37, §3.6 R48/R48a/R48b (post-Round-2 redraft)
**Predecessors consulted:** Round 1 review (`SPEC-REVIEW-19-section-3.4-D-005-2026-04-23.md`), Round 2 re-review (`-REREVIEW.md`), SPEC-19 spec at 1245 lines
**Source-of-truth cross-check:** `relativist-core/src/merge/border_resolver.rs:312-358` (`CommutationBatch.target_symbols: Vec<Symbol>`, `SLOT_MARKER_BASE = u32::MAX - 10_000`), `relativist-core/src/net/types.rs:34-38` (`Symbol` rkyv derive)

---

## 1. Status of Round-2 NF findings

| ID | Severity | Verdict | Evidence |
|---|---|---|---|
| **NF-001** target_symbols reconstruction gap | CRITICAL | **CLOSED** | Shape A adopted. `PendingCommutation.target_symbols: Vec<Symbol>` at L256 replaces `symbol_type: Symbol` + `arity: u8`. R24.1.6a at L160 explicitly reads slot `k` from `pc.target_symbols[k]` and forbids "reconstruct[ing] slot symbols from resolver-side batch layouts." Propagated through R23 payload prose (L135), R23a clause 3 (L144), R33c cases 1/3/7 (L324/330/349), R48a (L378). |
| **NF-002** version-check bidirectionality | HIGH | **CLOSED** | R37 L368 adds: "The version check fires symmetrically (NF-002) … Both sides validate against their own `PROTOCOL_VERSION` constant … implementers MUST NOT add defensive decode-retry paths." Reads symmetric + prescriptive as requested. |
| **NF-003** duplicate-key detection site | MEDIUM | **CLOSED** | R23a clause 6 at L147: `HashSet<(u8, u8)>` pre-pass "BEFORE the first call to `Net::connect`", with detection site pinned explicitly and re-linked from R33c case 5 (L342). R23a determinism guarantee (L149) cross-references clause 6. |
| **NF-004** `arity == 0` edge case | MEDIUM | **CLOSED** | R33 post-struct paragraph (L265) rejects empty `target_symbols`; R33c case 7 `ZeroArity` (L349-354); R33 docstring L255 pins `target_symbols.len() … always >= 1`. |
| **NF-005** pipeline-state.md stale | LOW (procedural) | **DEFERRED** (correct) | Changelog L1245 flags "NF-005 deferred to sdd-pipeline." Out of spec-critic scope as originally noted. |

**Summary:** 4/4 spec-side NFs CLOSED. NF-005 correctly deferred. No Round-2 NF remains OPEN at spec-scope.

---

## 2. NF-001 propagation audit (the critical item)

Per the Round-3 prompt criterion 2: after Shape A adoption, no legacy `pc.symbol_type` or `pc.arity` struct-field reference may remain, and every downstream requirement that previously used `arity` must route through `pc.target_symbols.len()` (count) or `pc.target_symbols[k]` (per-slot symbol).

### 2.1 Legacy field-reference scan

| Ripgrep of `specs/SPEC-19-delta-protocol.md` for `symbol_type`, `pc.arity`, `pc.symbol_type` | Expected | Found |
|---|---|---|
| `pc.symbol_type` (field access) | 0 | 0 |
| `pc.arity` (field access) | 0 | 0 |
| `symbol_type:` (struct-field decl) | 0 | 0 |
| `arity: u8` (struct-field decl) | 0 | 0 |
| `symbol_type` (prose) | 0 active + explicit retrospective | 1 occurrence at L250 (docstring narrating the NF-001 migration: *"Replaces the pre-NF-001 `(symbol_type, arity)` pair…"*) — **not a legacy reference; it is the migration note.** |

`arity` token still present 4x in the spec; all four inspected:

1. **L135** (R23 payload prose): `"slot k minted as a target_symbols[k] agent of its native arity"` — "native arity" = arity of the `Symbol` itself (CON=2, DUP=2, ERA=0), not `pc.arity`. Clean.
2. **L143** (R23a clause 2): `"the worker MUST allocate all arity sibling AgentId values (via Net::create_agent, consuming from the worker's IdRange monotonically) BEFORE applying any entry of that request's local_wiring"` — **soft gap**: the word `arity` is used as if it were still a struct-field count. A Stage-1 implementer would read this and ask "where does `arity` come from now?" The resolution is obvious (`pc.target_symbols.len()`), and L143 immediately afterward says `"minted_ids_per_pc[k] holds the AgentId allocated for slot k"`, which anchors the count to the `target_symbols` indexing. Ambiguity is low but the prose is not maximally tight. **Flagged as new finding NR3-001 (LOW).**
3. **L232** (`LocalReconnection` docstring): `"Port of that agent (0 = principal, 1..arity = aux)"` — unrelated to `pc.arity`; this is the general comment on agent-port ports, applies across the spec. Clean.
4. **L254** (`PendingCommutation` docstring): `"the effective arity of the request is target_symbols.len() and is always >= 1"` — **explicitly defines** that `arity` is now a derived quantity. Clean.

### 2.2 Downstream rule rewrites

| Location | Round-2 shape | Round-3 shape | Verdict |
|---|---|---|---|
| R23 payload prose (L135) | `symbol_type` + `arity` | `target_symbols: Vec<Symbol>` with slot `k` minted as `target_symbols[k]` | **OK** |
| R23a clause 3 slot-marker decoding (L144) | guard `(x - SLOT_MARKER_BASE) >= arity` | guard `(x - SLOT_MARKER_BASE) >= pc.target_symbols.len()` | **OK** |
| R23a clause 6 pre-pass (L147) | did not exist | `HashSet<(u8, u8)>` built across `pc.local_wiring` before any `Net::connect` | **OK** |
| R24.1.6a mint clause (L160) | implicit reconstruction | `pc.target_symbols.len()` allocations, slot `k` minted with `pc.target_symbols[k]` taken directly from wire, explicit prohibition on reconstruction | **OK** |
| R33 struct (L244-262) | fields `symbol_type`, `arity` | single field `target_symbols: Vec<Symbol>` | **OK** |
| R33 post-struct NF-004 note (L265) | did not exist | `target_symbols.len() == 0` rejected via R33c case 7 | **OK** |
| R33c case 1 `SrcSlotOutOfRange` (L324) | `{ src_slot, arity }` | `{ src_slot: u8, symbol_count: u8 }` with doc `src_slot >= pc.target_symbols.len()` | **OK** — enum variant renamed `arity` → `symbol_count` consistently |
| R33c case 3 `TargetSiblingOutOfRange` (L330) | `{ sibling_slot, arity }` | `{ sibling_slot: u8, symbol_count: u8 }` with doc `sibling_slot >= pc.target_symbols.len()` | **OK** — renamed consistently |
| R33c case 7 `ZeroArity` (L349-354) | did not exist | new MUST-reject variant for empty `target_symbols` | **OK** |
| R33c introductory sentence (L319) | did not reference `target_symbols` | "`symbol_count` below is a shorthand for `pc.target_symbols.len()`" | **OK** |
| R33c MUST list (L360) | "cases 1, 2, 3, 5, 6" | "cases 1, 2, 3, 5, 6, 7" | **OK** — case 7 included in hard-reject set |
| R34 rkyv audit (L362) | `PendingCommutation`, `LocalWiringHint`, `MintedAgent` rkyv derives | adds explicit coverage of `Vec<Symbol>` element type and notes `Symbol` is `#[repr(u8)]` (1-byte aligned), so enclosing `Vec<Symbol>` does not perturb the 4-byte alignment imposed by `PortRef`; zero-copy baseline still ≥ 1192; adds +1 ZeroArity rejection UT | **OK** |
| R48a (L378) | `arity` in rejection condition | `(x - SLOT_MARKER_BASE) >= pc.target_symbols.len()`, variant args now `{ sibling_slot, symbol_count }` | **OK** — matches R33c case 3 signature |

**Verdict — NF-001 propagation: 10 call-sites updated, 1 soft gap (L143 prose, NR3-001 LOW).** The critical audit is clean; no `pc.symbol_type` / `pc.arity` legacy reference survives at any call site where the count or the per-slot symbol matters for behaviour.

### 2.3 rkyv coverage of `Vec<Symbol>`

| Check | Evidence | Verdict |
|---|---|---|
| `Symbol` derives rkyv::Archive under `zero-copy` | `relativist-core/src/net/types.rs:34-38` shows `#[cfg_attr(feature = "zero-copy", derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize))]` over `Symbol`, which is `#[repr(u8)]`. | **OK** — build does NOT break with `--features zero-copy` after Shape A adoption. |
| Spec acknowledges `Symbol` rkyv coverage | R34 L362: "the same derive MUST be applied to `PendingCommutation` (already present) and the per-slot `Symbol` element type carried inside `target_symbols: Vec<Symbol>` (NF-001 Shape A) — `Symbol` is a `#[repr(u8)]` enum, so its archived form is 1-byte-aligned and the enclosing `Vec<Symbol>` does not perturb the existing 4-byte alignment imposed by `PortRef` inside `LocalWiringHint`." | **OK** |
| Alignment audit | R34 (b): re-baselines `Partition` archived size "because `(u8, u8, PortRef)` forces 4-byte alignment via the `u32` payload inside `PortRef`" — explicitly calls out that the new `Vec<Symbol>` element size (1 byte, 1-byte aligned) does not introduce new alignment pressure. | **OK** — alignment concern addressed. |

---

## 3. New findings introduced by Round 3 redraft

### NR3-001 — R23a clause 2 still uses bare word `arity` as if it were a count field [LOW]

**Axis:** Completeness (prose hygiene).
**Location:** §3.3 R23a clause 2 (L143).

**Problem.** Line 143 reads: *"the worker MUST allocate all `arity` sibling `AgentId` values (via `Net::create_agent`, consuming from the worker's `IdRange` monotonically) BEFORE applying any entry of that request's `local_wiring`."*

After Shape A, there is no `arity` field on `PendingCommutation`. The sentence is a leftover from the Round-1 redraft. A strict Stage-1 reader would pattern-match `arity` to the (non-existent) field. The nearest struct field now is `target_symbols`, with `target_symbols.len()` as the count.

**Impact if unresolved.** Minimal. The next sentence of the same clause (*"`minted_ids_per_pc[k]` holds the `AgentId` allocated for slot `k`"*) pins the iteration to `k ∈ [0, target_symbols.len())` implicitly, and R24.1.6a is explicit (*"allocate `pc.target_symbols.len()` fresh `AgentId` values"*). The spec as-a-whole is unambiguous, but this one line reads as a forgotten rename.

**Suggested resolution.** One-word edit:
> "the worker MUST allocate all `pc.target_symbols.len()` sibling `AgentId` values (via …)"

Or: "the worker MUST allocate one fresh sibling `AgentId` for every entry of `pc.target_symbols` (via …)".

This is LOW because: (i) R24.1.6a is the normative mint clause and it is fully rewritten; (ii) R23a clause 2 is pre-amble prose whose only role is to pin mint-before-wire ordering, which is preserved; (iii) no test or implementation decision hinges on which sentence is the authoritative count. Recommend fix on the next spec touch; does not block Stage 1.

---

### NR3-002 — R37 "MUST NOT add defensive decode-retry paths" is imperative over implementation tactics [LOW]

**Axis:** Consistency (spec vs. downstream SPEC-06/SPEC-11 error-handling policies).
**Location:** §3.4 R37 (L368, last sentence).

**Problem.** R37 now reads *"implementers MUST NOT add defensive decode-retry paths for that case."* This is a prescription about **what implementers are forbidden from writing**, not about what the protocol allows on the wire. Two concerns:

1. **Over-reach.** SPEC-06 may already license transport-level retry (TCP reconnection, handshake retry on transient NACK) and SPEC-11 may document observability-triggered decode-failure traces. A naive reading of "MUST NOT add defensive decode-retry paths" could be quoted by a reviewer to block a SPEC-06-legal retry loop that happens to sit near the `PendingCommutation` decode path. The sentence needs scoping.
2. **Spec genre mismatch.** Negatives about absent code are unusual in formal specs; they are usually encoded as invariants on observable state ("no mid-session session MAY continue after a decode failure on a new-schema trailing field — the worker MUST close the connection and emit `ProtocolError::DeserializationFailed`"). The current wording is closer to a coding-review note than a testable requirement.

**Impact if unresolved.** Minimal. The surrounding prose is clear enough that the intent ("don't silently swallow decode failures hoping the next frame lines up") is recoverable. But a pedantic Stage-1 reader may flag the sentence at task-splitter time.

**Suggested resolution.** Rewrite the last clause of R37 as a testable invariant:

> "Mid-session bincode-decode failures on the new trailing `Vec<LocalWiringHint>` field MUST surface as `ProtocolError::DeserializationFailed` and immediately terminate the session; implementations MUST NOT implement a decode-retry loop that re-parses the same frame under a different schema assumption."

Keeps the intent, roots it in an observable state transition, and does not conflict with SPEC-06 transport-level retry. LOW severity because the surrounding text already gives enough context.

---

### NR3-003 — `target_symbols` max length bound not explicit in R33 signature [LOW]

**Axis:** Completeness (range specification).
**Location:** §3.4 R33 `PendingCommutation.target_symbols` (L256), L252-253 docstring.

**Problem.** The Round-3 prompt asked whether `target_symbols.len()` has an explicit max. The docstring at L252 says *"Length is bounded by 16 via the `encode_request_id` assertion at `border_resolver.rs:318-322` (slot-marker namespace reservation)"*. Verified against source — `debug_assert!(agent_slot < 16, …)`.

Two issues with this:
1. **`debug_assert!` is release-stripped.** The cap is enforced only in debug builds of the resolver. A release-build resolver bug that emits `target_symbols.len() > 16` produces a `CommutationBatch` with un-encodable slot markers; the downstream `encode_request_id` silently truncates (the `& 0xF` mask in `encode_request_id`'s final expression). A Round-3 adversarial reader could argue the worker has no defensive guard at decode time: R33c case 3 rejects `sibling_slot >= pc.target_symbols.len()`, but does NOT reject `target_symbols.len() > 16` as a protocol violation. A malicious / bugged coordinator sending `target_symbols: [Con; 255]` would pass R33c entirely.
2. **Bincode `Vec<T>` has no length cap by default.** `target_symbols: Vec<Symbol>` on the wire is `u64 len prefix + 1 byte/elem`; a coordinator could send `len = u64::MAX` and the worker's bincode decode would allocate (or OOM). The existing `Message` size-cap at SPEC-06 R3/R8 frame-size limits do bound this at the frame level, but no R33-local guard mentions it.

Worker then does `pc.target_symbols.len()` allocations via `Net::create_agent`. If that number is legitimately large (by spec), fine; if it is a protocol-encoding violation (because `encode_request_id` requires slot < 16), the worker has no detection site.

**Impact if unresolved.** Low in practice: resolver bugs that emit `len > 16` would already crash CI via `debug_assert!`; production wire tampering is out of the documented threat model (no authenticated-coordinator attacker). But the symmetry with NF-003's "detect malformed payload BEFORE first `Net::connect`" argument applies here too: if the coordinator-side invariant is "slot < 16", the worker-side invariant should mirror it and reject `pc.target_symbols.len() > 16` at R33c.

**Suggested resolution.** Add an eighth case to `MalformedLocalWiringReason`:

```rust
/// Case 8 (MUST reject): `pc.target_symbols.len() > 16` -- exceeds the
/// slot-marker namespace cap enforced by the resolver's
/// `encode_request_id` assertion (`border_resolver.rs:318-322`). Worker
/// rejects before mint allocation to avoid unbounded `create_agent`
/// loops on malformed input.
TargetSymbolsTooLong { symbol_count: u8 },
```

And update R34's zero-copy baseline to ≥ 1193 (+1 UT for TargetSymbolsTooLong rejection). LOW severity because this is hardening against a non-production threat (tampered wire from an authenticated coordinator) and because the real-build `debug_assert!` cap on the resolver side already catches the bug path.

This is a true R3 finding (not inherited from prior rounds) because it was introduced by the Shape A transition: pre-NF-001, `PendingCommutation.arity: u8` capped the count structurally at 255 with no ambiguity, and the resolver's `agent_slot < 16` assertion governed the `u8 → u32` transform. Post-Shape A, `Vec<Symbol>` has no structural cap at all, and the relationship between `target_symbols.len()` and the 16-slot resolver invariant is documented in prose only.

---

## 4. Severity tally (Round 3 new findings)

| Severity | Count |
|---|---|
| CRITICAL | 0 |
| HIGH | 0 |
| MEDIUM | 0 |
| LOW | 3 (NR3-001, NR3-002, NR3-003) |
| **Total new** | **3, all LOW** |

Combined Round-2 NF closure: **4/4 spec-scope CLOSED** + NF-005 correctly deferred to sdd-pipeline.

---

## 5. Verdict: **SIGN-OFF**

**Gate check:**
- ≥1 CRITICAL new → not triggered (0 CRITICAL).
- ≥2 HIGH new → not triggered (0 HIGH).
- Any NF STILL OPEN at ≥HIGH → not triggered (NF-001..004 CLOSED, NF-005 deferred per scope).
- NF-001 propagation incomplete → not triggered (10/10 call-sites updated; NR3-001 is a prose leftover, not a call-site propagation failure).

**Summary.** The Round-2 redraft lands cleanly. Shape A has been propagated through R23, R23a, R24, R33 (struct + post-struct note), R33c (enum variants renamed `arity` → `symbol_count`, plus new case 7 ZeroArity), R34 (alignment audit + +1 UT), and R48a. `Symbol`'s pre-existing `rkyv::Archive` derive at `relativist-core/src/net/types.rs:34-38` confirms the `--features zero-copy` build continues to work after the `Vec<Symbol>` substitution. The changelog entry at L1245 is extended (not duplicated) and covers both rounds of history. NF-002's symmetric version check is prescriptive-but-correct; NF-003's detection-site pin is in R23a clause 6 BEFORE any `Net::connect`; NF-004's zero-arity rejection is tight.

The three Round-3 new findings are all LOW:
- **NR3-001** is a leftover `arity` word at L143 — a single-word prose edit.
- **NR3-002** is R37's prescriptive last sentence — recommend a reword to a testable invariant on the next spec touch.
- **NR3-003** is a Shape-A side-effect on max-length bounding — worth adding as an eighth `MalformedLocalWiringReason` case the next time the enum is opened, but not blocking.

None of them blocks Stage 1. The spec is implementable as-written by a developer who has never seen the codebase, with no ambiguity about what to build at any critical call site.

**Required next step.** sdd-pipeline updates `pipeline-state.md` to Stage 1 (TASK-SPLITTER) with the Round-2 redraft + this Round-3 review as inputs. The three NR3 LOW findings may be folded into a housekeeping pass on the next spec touch (not Stage 1 blocker).

Estimated task-splitter runtime on the Round-2 redraft: bundle scope is unchanged from Round 1 (NF-001's Shape A is structurally 1 field swap + downstream prose; no new atomic tasks arise from the NF series). Task-splitter should target the same ~200 LoC-per-task envelope; the enum-variant renames (`arity` → `symbol_count`) and the `Vec<Symbol>` field swap are both single-sitting edits.

---

## 6. Checklist

### Consistency
- [x] All terms match SPEC-00 / SPEC-19 conventions
- [x] Type signatures compatible with resolver source-of-truth (`CommutationBatch.target_symbols: Vec<Symbol>` mirrored 1-to-1)
- [x] No contradictions with predecessor requirements (R48 `SLOT_MARKER_BASE = u32::MAX - 10_000` reservation still holds)
- [x] Data flow assumptions match resolver outputs at `border_resolver.rs:312-358`

### Testability
- [x] Every MUST requirement has a testable criterion (R33c cases 1/2/3/5/6/7 all pinned with enum variants; R34 +1 UT for ZeroArity)
- [x] Boundary conditions defined (empty `local_wiring` at R48b; empty `target_symbols` at R33 post-struct note + R33c case 7; max length implicitly via docstring, NR3-003 suggests explicit UT)
- [x] Error conditions specified (7 enum variants in `MalformedLocalWiringReason`)

### Completeness
- [x] Per-slot mint rule fully specified at R24.1.6a with explicit prohibition on reconstruction
- [x] Duplicate-key detection site pinned to R23a clause 6 with `HashSet<(u8, u8)>` pre-pass
- [x] `ZeroArity` edge case covered in R33 + R33c case 7 + R34 UT
- [x] rkyv coverage extends to `Vec<Symbol>` (R34 (a)) and `Symbol`'s existing derive confirmed in source
- [x] Changelog L1245 correctly extended (both Round 1 + Round 2 history preserved)

### Invariant Preservation
- [x] G1 (R38) unchanged; Shape A does not alter the recoverability argument
- [x] D3 (R39) unchanged; slot-marker-to-AgentId substitution is worker-local
- [x] D6 (R40) unchanged; in-round reducibility of minted sibling pairs preserved at R23a clause 5
- [x] T4 (strong confluence) unchanged; the mint-then-wire ordering at R24 step 1 is isomorphic to the resolver's sequential emission
