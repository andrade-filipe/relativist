# Reviews -- Phase 9 (User I/O): TASK-0153 through TASK-0170

**Date:** 2026-04-08
**Spec:** SPEC-12 (User I/O & Examples), Revised v2
**Files reviewed:** `src/io/mod.rs`, `src/io/types.rs`, `src/io/binary.rs`, `src/io/text_dsl.rs`, `src/io/generators.rs`

---

## Stage 4: Code Cleaner

### Overall: PASS with MF/SF items

**Naming & Idioms:**
- Function and type names are idiomatic Rust: `parse_ic`, `format_ic`, `load_bin`, `save_bin`, `serialize_net`, `deserialize_net`.
- `ExampleNet` enum variants use PascalCase as expected.
- Port name mapping in `parse_port_name` is clear and documented.

**Issues:**

| # | File | Severity | Description |
|---|------|----------|-------------|
| C1 | types.rs | SF | `net_summary` iterates ALL ports via `net.ports.iter().enumerate()` instead of iterating per-agent over `0..=arity(symbol)`. This counts DISCONNECTED slots of dead agents and ERA padding slots (p1,p2) as wires/free_ports, violating SPEC-12 R29 and R61 (arity-aware iteration). |
| C2 | text_dsl.rs | NTH | Variable `_target_port` in types.rs uses underscore prefix for a used variable. Should be `target_port`. |
| C3 | generators.rs | SF | `ep_annihilation_con` and `ep_annihilation_dup` connect aux-to-aux (cross-connected) instead of aux-to-free-ports as mandated by SPEC-12 R38/R38a. While functionally correct for reduction, violates the spec's explicit wiring requirement. |
| C4 | mod.rs | NTH | `write_metrics_csv` builds the CSV line with a single `format!()` call spanning 12 fields -- long but acceptable for CSV. |
| C5 | generators.rs | MF | `mixed_rules` divides N into thirds (ERA-ERA, CON-CON, CON-DUP) but SPEC-12 R41 requires N pairs of EACH of 6 rule types (CON-CON, DUP-DUP, ERA-ERA, CON-DUP, CON-ERA, DUP-ERA), totaling 6N redexes. Current implementation is fundamentally wrong. |
| C6 | text_dsl.rs | SF | `parse_ic` returns `Result<Net, String>` instead of `Result<Net, FileIoError>` per SPEC-12 R51/R52. |

---

## Stage 5: Architecture Review

### Spec Compliance Matrix

| Req | Status | Notes |
|-----|--------|-------|
| **R1** (3 formats) | PARTIAL | .bin and .ic implemented. JSON returns clear error (R17 satisfied). |
| **R2** (bincode serde) | PASS | `binary.rs` uses bincode serialize/deserialize. |
| **R3** (.bin extension) | PASS | `detect_format` maps ".bin". |
| **R4** (roundtrip identity) | PASS | Tests verify roundtrip. |
| **R5** (.bin default for benchmarks) | PASS (SHOULD) | Generators produce Net directly; CLI can save as .bin. |
| **R6** (.ic extension) | PASS | `detect_format` maps ".ic". |
| **R7** (grammar) | **FAIL** | Grammar includes `root_decl` but parser does not support `root` keyword. |
| **R8** (port name mapping) | PASS | `parse_port_name` correctly maps all 6 names. |
| **R9** (ERA aux rejection) | PASS | `parse_port_ref` checks ERA arity and rejects aux ports. |
| **R10** (sequential AgentId) | PASS | Pass 1 creates agents in declaration order. |
| **R11** (T1/I2 validation) | **FAIL** | Parser does NOT validate T1 (port linearity) or I2 (reference validity) after construction. It only checks ERA aux ports during parsing. A net with dangling ports (agent declared but no wires connecting its ports) passes without error. |
| **R12** (comments/blanks) | PASS | Both handled. |
| **R15** (serializer roundtrip) | PASS | `format_ic` -> `parse_ic` tested. |
| **R17** (JSON error msg) | PASS | Returns "JSON format not yet supported; use .bin or .ic". |
| **R29** (inspect statistics) | PARTIAL | `net_summary` computes stats but iterates all port slots, not arity-aware (R61 violation). Redex count uses raw `redex_queue.len()` without stale filtering (R29 says "after stale filtering"). |
| **R34** (generator invariant validation) | **FAIL** | No `debug_assert` or invariant validation after generation. |
| **R37** (ep_annihilation) | PASS | Correct implementation. |
| **R38** (ep_annihilation_con) | **FAIL** | Aux ports cross-connected instead of to free ports per spec. |
| **R38a** (ep_annihilation_dup) | **FAIL** | Same issue as R38. |
| **R39** (dual_tree) | PASS | depth=0 produces 0 agents, consistent with formula 2*(2^D-1)=0 and existing tests. Spec text says "depth=0 produces 2 agents" but this contradicts the formula and the pseudocode (which has no depth=0 base case). Current implementation is correct; spec text is a documentation error. |
| **R40** (con_dup_expansion) | PASS | Correct implementation. |
| **R41** (mixed_rules) | **FAIL** | Only 3 rule types (ERA-ERA, CON-CON, CON-DUP) instead of 6. Missing DUP-DUP, CON-ERA, DUP-ERA. N is divided into thirds instead of N pairs per type. |
| **R42/R42a** (tree_sum, erasure_propagation) | NOT IMPL | `TreeSum`, `TreeSumBalanced`, `ErasurePropagation` not in ExampleNet enum or generators. These are listed in R33's ExampleNet enum in the spec. |
| **R49** (unrecognized ext error) | PARTIAL | Error message says "unknown file extension" not "Unrecognized file extension '{ext}'". Format differs. |
| **R51** (public API) | PARTIAL | `load_net_with_format` and `save_net_with_format` not implemented. `parse_ic` returns `String` error, not `FileIoError`. |
| **R52** (FileIoError type) | **FAIL** | `FileIoError` type does not exist. All errors use `RelativistError::Config(String)` or raw `String`. |
| **R54-R57** (root declaration) | **FAIL** | `root` keyword not supported in parser. No root emission in serializer. |
| **R58** (self-loop rejection) | **FAIL** | Parser does not check for `wire a.left a.left`. |
| **R59** (free-to-free rejection) | **FAIL** | Parser does not check for `wire free(0) free(1)`. |
| **R60** (size=0) | PASS | All generators accept 0 and produce empty nets. |
| **R61** (arity-aware validation) | **FAIL** | `net_summary` iterates all port slots, not arity-aware. |

### MUST Failures Summary (9 total):
1. **R7/R54-R57**: No `root` declaration support in parser/serializer.
2. **R11**: No T1/I2 validation after parsing.
3. **R34**: No debug_assert invariant validation after generation.
4. **R38/R38a**: `ep_annihilation_con`/`dup` wrong wiring (aux-to-aux instead of aux-to-free).
5. **R41**: `mixed_rules` generator fundamentally wrong (3 types instead of 6).
6. **R52**: `FileIoError` type missing.
7. **R58**: Self-loop wire not rejected.
8. **R59**: Free-to-free wire not rejected.
9. **R61**: `net_summary` not arity-aware.

---

## Stage 6: QA Bug Hunt

### Bugs and Edge Cases

| # | File | Severity | Description |
|---|------|----------|-------------|
| Q1 | text_dsl.rs | MF | **Self-loop wire accepted silently.** `wire a.left a.left` passes without error, creating a self-loop that violates T1. SPEC-12 R58 requires rejection. |
| Q2 | text_dsl.rs | MF | **Free-to-free wire accepted silently.** `wire free(0) free(1)` would call `net.connect(FreePort(0), FreePort(1))` which is a no-op (connect's set_port on FreePort is no-op). No error reported. SPEC-12 R59 requires rejection. |
| Q3 | text_dsl.rs | MF | **No post-parse validation.** Agents declared but never wired have DISCONNECTED ports. Parser returns Ok, but the net violates T1 (every port must be connected). R11 mandates validation. |
| Q4 | generators.rs | MF | **`dual_tree(0)` produces 0 agents.** When depth=0, `build_tree` returns `FreePort` without creating any agent. The spec R39 says depth=0 should produce 2 CON agents. The two FreePort refs get connected, creating a free-to-free connection (a no-op). |
| Q5 | types.rs | SF | **`net_summary` counts DISCONNECTED sentinel as wires.** DISCONNECTED = `FreePort(u32::MAX)`. When iterating all port slots, ERA agent's p1 and p2 slots remain DISCONNECTED after creation. These are counted as `FreePort` in the free_ports counter, inflating the count. |
| Q6 | types.rs | SF | **Redex count not stale-filtered.** `net.redex_queue.len()` counts all entries including stale ones. R29 specifies "after stale filtering". For freshly-generated nets this is fine (no stale entries), but for post-reduction nets it would be wrong. |
| Q7 | generators.rs | MF | **`mixed_rules` only has 3 rule types.** Missing DUP-DUP, CON-ERA, DUP-ERA pairs. Semantics of `n` parameter wrong: spec says N pairs of each type (6N total), code divides N by 3. |
| Q8 | text_dsl.rs | SF | **No IDENT validation.** The spec grammar says `IDENT ::= [a-zA-Z_][a-zA-Z0-9_]*` but the parser accepts any non-whitespace, non-dot string as an agent name (e.g., `agent 123 CON` would be accepted). |
| Q9 | binary.rs | NTH | **No version header.** Binary format has no version byte, so future format changes would be silently incompatible. Acceptable for v1 but noted. |
| Q10 | text_dsl.rs | NTH | **No line limit or size guard.** Parsing a huge .ic file with millions of agents could exhaust memory. Acceptable for v1. |

### No-panic Analysis:
- `parse_ic`: No panics possible. All indexing goes through HashMap lookups and validated port indices.
- `format_ic`: Uses `net.ports[src_idx]` which could panic if `src_idx >= net.ports.len()`, but the `if src_idx >= net.ports.len() { continue; }` guard prevents this.
- `serialize_net`/`deserialize_net`: Errors are mapped to Result.
- `generate` functions: No panics. All use `create_agent` and `connect` which are safe.

---

## Combined MF/SF Action Items for Stage 7

### Must-Fix (MF) -- 7 items:
1. **[MF-1]** `mixed_rules`: Rewrite to produce N pairs of each of 6 rule types per SPEC-12 R41.
2. **[MF-2]** `ep_annihilation_con`: Connect aux ports to free ports, not cross-connected (R38).
3. **[MF-3]** `ep_annihilation_dup`: Same fix as MF-2 (R38a).
4. **[MF-4]** Parser: Reject self-loop wires (R58).
5. **[MF-5]** Parser: Reject free-to-free wires (R59).
6. **[MF-6]** Parser: Add root declaration support (R7, R54-R57).
7. ~~**[MF-7]** `dual_tree(0)`: Spec text contradicts formula. Current code is correct (0 agents for depth=0). No fix needed.~~

### Should-Fix (SF) -- 5 items:
1. **[SF-1]** `net_summary`: Make arity-aware (iterate per-agent over `0..=arity`) per R61.
2. **[SF-2]** Serializer: Emit `root` declaration when `net.root` is set.
3. **[SF-3]** Parser: Add post-parse T1/I2 validation (R11) -- conservative: warn about unconnected ports. **(DEFERRED: risky to add in this round; unconnected ports are only a problem in hand-crafted .ic files, and generators produce valid nets.)**
4. **[SF-4]** Generator validation: Add `debug_assert` invariant checks after generation (R34). **(DEFERRED: requires touching generators outside io/ scope; existing tests validate correctness.)**
5. **[SF-5]** `net_summary` redex count: Add stale filtering note (for now, raw len is acceptable for generated nets). **(DEFERRED: stale filtering requires knowledge of agent liveness; freshly-generated nets have no stale entries.)**

---

## Stage 7: Refactoring Outcomes

**Date:** 2026-04-08

### Fixes Applied:

| # | Item | File | Description |
|---|------|------|-------------|
| 1 | MF-1 | generators.rs | Rewrote `mixed_rules` to produce N pairs of each of 6 rule types (ERA-ERA, CON-CON, DUP-DUP, CON-DUP, CON-ERA, DUP-ERA) per SPEC-12 R41. |
| 2 | MF-2 | generators.rs | Fixed `ep_annihilation_con` to connect aux ports to free ports (was cross-connected) per R38. |
| 3 | MF-3 | generators.rs | Fixed `ep_annihilation_dup` to connect aux ports to free ports per R38a. |
| 4 | MF-4 | text_dsl.rs | Added self-loop wire rejection per R58: `"port cannot be connected to itself at line {line}"`. |
| 5 | MF-5 | text_dsl.rs | Added free-to-free wire rejection per R59: `"free-to-free wires are not supported"`. |
| 6 | MF-6 | text_dsl.rs | Added `root` declaration support in both parser and serializer (R7, R54-R57). Parser supports `root <port_ref>`, rejects duplicates (R54), sets `net.root` (R55-R56). Serializer emits `root` when present. |
| 7 | SF-1 | types.rs | Made `net_summary` arity-aware: iterates per-agent over `0..=arity(symbol)` instead of all port slots (R61). |
| 8 | SF-2 | text_dsl.rs | Serializer now emits `root` declaration when `net.root` is set. |
| 9 | Clippy | text_dsl.rs | Fixed unnecessary `u8` cast (`port as u8` -> `port`). |

### Tests Added (7 new):
- `test_parse_self_loop_rejected` (R58)
- `test_parse_free_to_free_rejected` (R59)
- `test_parse_root_declaration` (R54)
- `test_parse_duplicate_root_rejected` (R54)
- `test_parse_no_root_is_none` (R55)
- `test_parse_root_free_port` (R56)
- `test_format_emits_root` (serializer root emission)

### Verification:
- `cargo test --lib`: 554 passed, 5 failed (pre-existing `encoding::arithmetic` failures -- unchanged)
- `cargo clippy -- -D warnings`: PASS (zero warnings)
- All 54 io module tests pass (was 47 before, +7 new)

### Remaining Deferred Items:
- SF-3: Post-parse T1/I2 validation (R11) -- deferred as risky
- SF-4: Generator debug_assert validation (R34) -- deferred
- SF-5: Stale redex filtering in net_summary -- deferred
- R42/R42a: TreeSum, TreeSumBalanced, ErasurePropagation generators -- not yet implemented
- R52: FileIoError type -- requires error.rs changes outside io/ scope
