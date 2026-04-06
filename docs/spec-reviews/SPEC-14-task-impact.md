# SPEC-14 Task Impact Report

**Date:** 2026-04-05
**Trigger:** SPEC-14 revision from Draft v1 to Revised v2 (adversarial review)
**Defender response:** `SPEC-14-round2-defender.md`
**Revised spec:** `specs/SPEC-14-encoding.md`

---

## 1. Summary Table

| Category | Count | Task IDs |
|----------|-------|----------|
| **Created** | 12 | TASK-0200 through TASK-0211 |
| **Updated** | 2 | TASK-0117, TASK-0177 |
| **Obsoleted** | 0 | -- |
| **Unchanged** | 1 | TASK-0171 |
| **Total affected** | 15 | -- |

**Backlog total:** 180 -> 192 tasks

---

## 2. New Phase Created

**Phase 11: Encoding (SPEC-14)** was added to BACKLOG.md. No SPEC-14 tasks existed prior to this update -- the entire encoding module needed task decomposition.

---

## 3. Details for Each New Task

| Task ID | Title | Requirements Covered | Rationale |
|---------|-------|---------------------|-----------|
| TASK-0200 | Scaffold encoding module directory structure | R1, R2, R3 | Module structure (`src/encoding/` with `mod.rs`, `church.rs`, `arithmetic.rs`) is a MUST prerequisite |
| TASK-0201 | Implement encode_church_into | R4b, R5, R6, R7, R8, R10 | Core builder function (NEW in Revised v2 via SC-010). Foundation for all encoding |
| TASK-0202 | Implement encode_nat | R4, R8, R9, R10 | Public convenience wrapper. R9 was rewritten (SC-001) to use `net.root = Some(...)` |
| TASK-0203 | Implement decode_nat | R11, R12, R13, R14, R21 | Readback algorithm. Completely rewritten in Revised v2 (SC-002, SC-003) to use SPEC-02 API |
| TASK-0204 | Implement build_add | R4b, R15, R18, R20 | Addition combinator. Construction steps rewritten (SC-007, SC-010, SC-018) |
| TASK-0205 | Implement build_mul | R4b, R16, R18, R20 | Multiplication combinator. Major expansion from 2 sentences to full spec (SC-006) |
| TASK-0206 | Implement build_exp | R4b, R17, R18, R20 | Exponentiation combinator. R17 promoted from SHOULD to MUST (SC-017) |
| TASK-0207 | Encoding unit tests (ET-1 to ET-5, ET-9, ET-12) | ET-1 to ET-5, ET-9, ET-12 | Tests renamed from T-* to ET-* (SC-005). No reduction engine dependency |
| TASK-0208 | Arithmetic correctness tests (ET-6 to ET-8, ET-10) | ET-6, ET-7, ET-8, ET-10 | Tests rewritten to two-step pattern (SC-004). Requires Phase 2 |
| TASK-0209 | Distributed correctness test (ET-11) | ET-11 | Rewritten as multi-step code (SC-015). Requires Phase 4 |
| TASK-0210 | Compute CLI subcommand | R22, R23, R24, R25 | User-facing workflow for TCC defense demonstration |
| TASK-0211 | Arithmetic benchmark scenarios (ARITH-*) | R18, R20, Section 8 | 12 benchmark points from Section 8.2 for distributed evaluation |

---

## 4. Details for Each Updated Task

### TASK-0117: Enforce Core/Infrastructure layer boundary

**What changed:** Updated note in the Notes section. The reference to SPEC-14 was outdated ("still in Draft v1"). Changed to reflect Revised v2 status and reference to TASK-0200.

**Why:** SPEC-14 is now Revised v2. The note about `pub mod encoding;` placeholder should reference the scaffolding task (TASK-0200) that creates the module.

**Specific changes:**
- Notes section: "SPEC-14 (Church numerals)" -> "SPEC-14 (Revised v2)"; added conditional reference to TASK-0200

### TASK-0177: Implement generators - erasure_propagation and Church encodings

**What changed:** Three modifications:
1. **Dependencies:** Added TASK-0202, TASK-0204, TASK-0205 as dependencies (Church generators now delegate to the encoding module)
2. **Dependencies Context:** Added references to `encode_nat`, `build_add`, `build_mul` from the encoding module
3. **Notes section:** Rewrote SPEC-14-related notes to reflect Revised v2:
   - Removed "still in Draft v1" language
   - Added note that generators are thin wrappers delegating to `src/encoding/` functions
   - Added note about R17 promotion (SHOULD -> MUST) for `build_exp`
   - Removed "implemented as `todo!()` stubs initially" -- encoding module must be implemented first
   - Added note that encoding module tasks must precede this task

**Why:** The Church generators in `src/io/examples.rs` (SPEC-12 R33) should delegate to the canonical encoding module functions (SPEC-14 R4, R15, R16) rather than reimplementing the encoding logic. This avoids duplication and ensures consistency.

---

## 5. Unchanged Tasks

### TASK-0171: Implement generator - ep_annihilation (ERA-ERA pairs)

**Status:** No change needed. TASK-0171 mentions SPEC-14 R26 only in the `ExampleNet` enum definition (ChurchNat, ChurchAdd, ChurchMul variants listed as future dispatch targets). The variant names in R26 did not change between Draft v1 and Revised v2. The `todo!()` stubs in the dispatcher are correct -- they will be filled in by TASK-0177.

---

## 6. Requirement Coverage Verification

Every MUST requirement in SPEC-14 Revised v2 is mapped to at least one task below.

### 6.1 Core Requirements

| Requirement | Description | Task(s) | Status |
|-------------|-------------|---------|--------|
| R1 | Encoding module in `src/encoding/`, Core Layer, pure sync | TASK-0200 | Covered |
| R2 | Depends only on `net` module types | TASK-0200 | Covered |
| R3 | Module organization: mod.rs, church.rs, arithmetic.rs | TASK-0200 | Covered |
| R4 | `encode_nat(n: u64) -> Net`, panics if n > 10_000 | TASK-0202 | Covered |
| R4b | `encode_church_into(net: &mut Net, n: u64) -> AgentId` | TASK-0201 | Covered |
| R5 | Church(0): 2 CON + 1 ERA, self-loop on inner lambda | TASK-0201 | Covered |
| R6 | Church(1): 3 CON, single application | TASK-0201 | Covered |
| R7 | Church(n >= 2): (n+2) CON + (n-1) DUP, DUP chain | TASK-0201 | Covered |
| R8 | Output satisfies T1-T7, debug validation | TASK-0201, TASK-0202 | Covered |
| R9 | `net.root = Some(AgentPort(lam_f, 0))` | TASK-0202 | Covered |
| R10 | Output in Normal Form (0 redexes) | TASK-0201, TASK-0202 | Covered |
| R11 | `decode_nat(net: &Net) -> Option<u64>` | TASK-0203 | Covered |
| R12 | Structural traversal algorithm | TASK-0203 | Covered |
| R13 | `&Net` (no modification) | TASK-0203 | Covered |
| R14 | Returns None for non-Church nets | TASK-0203 | Covered |
| R15 | `build_add(a: u64, b: u64) -> Net` | TASK-0204 | Covered |
| R16 | `build_mul(a: u64, b: u64) -> Net` | TASK-0205 | Covered |
| R17 | `build_exp(base: u64, exp: u64) -> Net` (MUST -- promoted) | TASK-0206 | Covered |
| R18 | Arithmetic nets reduce to correct Church numeral | TASK-0204, 0205, 0206, 0208 | Covered |
| R20 | Complexity bounds table | TASK-0204, 0205, 0206 | Covered |
| R21 | `decode_nat` O(n) time | TASK-0203 | Covered |
| R22 | `compute` CLI subcommand with `ComputeArgs`, `ArithmeticOp` | TASK-0210 | Covered |
| R23 | Local vs distributed mode selection | TASK-0210 | Covered |
| R24 | Human-readable output format | TASK-0210 | Covered |
| R25 | Warning on decode failure | TASK-0210 | Covered |
| R26 | ChurchNat, ChurchAdd, ChurchMul ExampleNet variants | TASK-0171, TASK-0177 | Covered |
| R27 | Generators usable from both generate and benchmark suite | TASK-0177 | Covered |

### 6.2 Test Requirements

| Test | Description | Task(s) | Status |
|------|-------------|---------|--------|
| ET-1 | Church(0) structure | TASK-0207 | Covered |
| ET-2 | Church(1) structure | TASK-0207 | Covered |
| ET-3 | Church(2) structure | TASK-0207 | Covered |
| ET-4 | Normal Form property | TASK-0207 | Covered |
| ET-5 | Roundtrip encode/decode | TASK-0207 | Covered |
| ET-6 | Addition correctness | TASK-0208 | Covered |
| ET-7 | Multiplication correctness | TASK-0208 | Covered |
| ET-8 | Exponentiation correctness | TASK-0208 | Covered |
| ET-9 | Invariant preservation (T1-T7) | TASK-0207 | Covered |
| ET-10 | Property test (proptest, SHOULD) | TASK-0208 | Covered |
| ET-11 | Distributed correctness (G1) | TASK-0209 | Covered |
| ET-12 | Decode rejection | TASK-0207 | Covered |

### 6.3 Non-MUST Requirements (for completeness)

| Requirement | Level | Description | Task(s) | Notes |
|-------------|-------|-------------|---------|-------|
| R19 | MAY | Factorial encoding (stretch goal) | -- | Not tasked; spec says MAY |
| ET-10 | SHOULD | Proptest for addition | TASK-0208 | Included in TASK-0208 as optional |
| R15 note | SHOULD | Direct construction approach | TASK-0204 | Implementation choice, noted in task |

### 6.4 Benchmark Scenarios (Section 8)

| Benchmark ID | Task | Status |
|--------------|------|--------|
| ARITH-ADD-S/M/L/XL | TASK-0211 | Covered |
| ARITH-MUL-S/M/L/XL | TASK-0211 | Covered |
| ARITH-EXP-S/M/L/XL | TASK-0211 | Covered |

---

## 7. Key Changes from Adversarial Review Reflected in Tasks

| SC-ID | Change | Impact on Tasks |
|-------|--------|----------------|
| SC-001 | R9 rewritten: `net.root = Some(AgentPort(lam_f, 0))` | TASK-0202 uses direct field assignment, not `set_root()` |
| SC-002 | Local `get_agent` helper for decode | TASK-0203 specifies the helper pattern |
| SC-003 | `get_target` returns `PortRef`, check `DISCONNECTED` | TASK-0203 uses `DISCONNECTED` checks, not `Option` |
| SC-004 | Two-step pattern: `reduce_all` then `decode_nat` | TASK-0208 uses explicit two-step pattern |
| SC-005 | Test labels renamed T-* to ET-* | TASK-0207, TASK-0208, TASK-0209 use ET- prefix |
| SC-006 | Multiplication/exponentiation fully specified | TASK-0205, TASK-0206 have complete construction steps |
| SC-007 | Single-net construction via `encode_church_into` | TASK-0204, 0205, 0206 use `encode_church_into` on shared net |
| SC-010 | New R4b: `encode_church_into` | TASK-0201 is dedicated to this new function |
| SC-012 | Panic for n > 10_000 | TASK-0201, TASK-0202 include assert |
| SC-017 | R17 promoted SHOULD -> MUST | TASK-0206 is P0 (critical path), not optional |
| SC-015 | ET-11 rewritten as multi-step code | TASK-0209 uses explicit local + distributed pattern |

---

## 8. Dependency Graph (Phase 11)

```
Phase 1 ─────────────────────────────────┐
                                         │
TASK-0200 (scaffold) ◄───────────────────┘
    │
    ├── TASK-0201 (encode_church_into)
    │       │
    │       ├── TASK-0202 (encode_nat)
    │       │       │
    │       │       └── TASK-0207 (unit tests ET-1..5, ET-9, ET-12)
    │       │
    │       ├── TASK-0204 (build_add)
    │       ├── TASK-0205 (build_mul)
    │       └── TASK-0206 (build_exp)
    │               │
    │               └── TASK-0208 (arithmetic tests ET-6..8, ET-10) ◄── Phase 2
    │                       │
    │                       └── TASK-0209 (distributed test ET-11) ◄── Phase 4
    │
    └── TASK-0203 (decode_nat) ◄── (used by 0207, 0208, 0209, 0210, 0211)

TASK-0210 (compute CLI) ◄── TASK-0200, 0204-0206, 0203, Phase 2, TASK-0100
TASK-0211 (ARITH-* benchmarks) ◄── 0204-0206, 0203, TASK-0182, Phase 2, Phase 4
```
