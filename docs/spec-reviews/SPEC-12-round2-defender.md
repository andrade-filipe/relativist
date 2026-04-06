# SPEC-12 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-12-user-io.md
**Critic review:** SPEC-12-round1-critic.md
**Spec version:** Draft v1 -> Revised v2

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 17 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 0 |
| **Total issues** | **20** |

---

## Responses

### SC-001: The `io` module does not exist in SPEC-13's module structure
**Response:** ACCEPTED
**Action taken:** The spec now explicitly acknowledges that the `io` module is a 12th module not present in SPEC-13 R5. A cross-spec note in Section 1 documents the required amendment to SPEC-13 R5. Section 4.3 now clearly delineates the Core Layer parts (`text.rs`, `examples.rs` -- pure functions, no I/O) from the Infrastructure Layer parts (`mod.rs` load/save, `binary.rs`, `json.rs` -- file system I/O). R36 was updated to remove the incorrect reference to "SPEC-13, R5" and instead points to Section 4.3 with a note about the required SPEC-13 amendment.
**Spec sections modified:** Section 1 (Purpose -- added cross-spec note), Section 3.5 R36, Section 4.3

### SC-002: Input format contradicts SPEC-07 (bincode-only) and SPEC-13 R42
**Response:** ACCEPTED
**Action taken:** A format supersession note was added to Section 1 stating: "SPEC-12 R1-R50 supersede SPEC-07 R22-R25 and SPEC-13 R42 for all file format specifications." The note clarifies that three-format support applies to `reduce`, `inspect`, and `generate` subcommands. The `coordinator`, `worker`, and `local` subcommands accept only `.bin` (bincode) for performance. R18 was clarified to state that `--input-format` applies only to input detection and output format always follows the output file's extension.
**Spec sections modified:** Section 1 (Purpose -- added supersession note), Section 3.2 R18

### SC-003: `reduce` vs `local` subcommand semantics remain unresolved
**Response:** ACCEPTED
**Action taken:** A subcommand clarification note was added to Section 1 explicitly stating: `reduce` (SPEC-13 R41, R46) performs purely sequential reduction via `reduce_all()` with no partitioning and no grid loop; `local` (SPEC-07 R1, R5, R18) runs the full grid loop in-process with simulated workers and partitioning; both subcommands coexist. SPEC-12 specifies `reduce`, `inspect`, and `generate`; it does not redefine `local`, `coordinator`, or `worker`.
**Spec sections modified:** Section 1 (Purpose -- added subcommand clarification note)

### SC-004: IoError type duplicates and conflicts with SPEC-13 error enums
**Response:** ACCEPTED
**Action taken:** Renamed `IoError` to `FileIoError` throughout the spec to avoid name collision with `std::io::Error` and to prevent duplicate `#[from] std::io::Error` conversions when composed into SPEC-13 R17's `RelativistError`. Added a note to R52 stating that SPEC-13 R17's `Io(#[from] std::io::Error)` variant MUST be replaced by `FileIo(#[from] FileIoError)` when the `io` module is added.
**Spec sections modified:** Section 3.8 R51, R52

### SC-005: `parse_ic` returns `ParseError` but only `IoError` is defined
**Response:** ACCEPTED
**Action taken:** Changed `parse_ic` signature from `Result<Net, ParseError>` to `Result<Net, FileIoError>`. The `FileIoError::Parse { line, message }` variant carries sufficient information for all parse error cases. This avoids introducing a redundant error type. A note was added to R51 documenting this decision.
**Spec sections modified:** Section 3.8 R51 (signature and note)

### SC-006: Text DSL grammar does not handle `root` declaration semantics
**Response:** ACCEPTED
**Action taken:** Added new requirements R54-R57 in a new Section 3.10 ("Text DSL Root Declaration Semantics"): R54 -- at most one `root` declaration per file (duplicate = parse error); R55 -- no `root` declaration means `net.root = None`; R56 -- root port reference must be valid, both `AgentPort` and `FreePort` are permitted (FreePort is valid per SPEC-14 R9); R57 -- root is not a wire and not counted in wire count, but FreePort roots count toward free port count. Added tests T11 covering all three cases.
**Spec sections modified:** Section 3.10 (new), Section 7 (T11)

### SC-007: No pseudocode for non-trivial generators
**Response:** ACCEPTED
**Action taken:** Added full Rust pseudocode for: R38a (`ep_annihilation_dup`), R39 (`dual_tree` -- recursive tree construction with pseudocode), R40 (`con_dup_expansion`), R41 (`mixed_rules` -- full implementation with all 6 pair types showing exact wiring), R42a (`erasure_propagation` -- chain construction with ERA at one end). For `mixed_rules`, added a detailed note explaining that all pair types use fresh free port IDs for auxiliaries, ensuring no cross-pair interactions before initial redex resolution. For `tree_sum` and `tree_sum_balanced`, R42 defers to SPEC-09 R14/R15 which define the semantics; the exact implementation of these generators is tightly coupled to the Haskell prototype's `mkTree`/`mkTreeBalanced` functions (AC-004) and will require a separate analysis pass.
**Spec sections modified:** Section 3.5 (R38a, R39, R40, R41, R42a -- all with pseudocode)

### SC-008: `generate` subcommand size parameter is `u32` but SPEC-14 uses `u64`
**Response:** PARTIALLY ACCEPTED
**Action taken:** The generator function signature remains `fn generate_<name>(size: u32) -> Net` because: (a) the u32-to-u64 widening cast is safe and trivial, (b) all non-Church generators use u32 naturally (agent counts, tree depths), and (c) Church numerals above 10,000 already produce impractically large nets. Rather than changing the type system, the spec adds documented warnings on the `ChurchNat`, `ChurchAdd`, and `ChurchMul` enum variants recommending small size values (N <= 10,000 for ChurchNat/ChurchAdd, N <= 1,000 for ChurchMul). The `ExampleNet` comments were updated with these warnings. No validation check was added because the AgentId exhaustion concern is theoretical -- the reduction engine will already fail gracefully if IDs overflow u32, and a pre-generation check would duplicate reduction logic.
**Spec sections modified:** Section 3.5 R33 (ExampleNet enum -- ChurchNat, ChurchAdd, ChurchMul comments)

### SC-009: R13 example comment says "cross-wise" but wires show parallel connectivity
**Response:** ACCEPTED
**Action taken:** Rewrote the R13 comment to accurately describe the input topology ("auxiliary ports connected in parallel: left-left, right-right") and what happens after reduction ("both agents removed, auxiliary targets reconnected cross-wise via the CON-CON annihilation rule; since targets are ports of the removed agents, the result is an empty net").
**Spec sections modified:** Section 3.1.2 R13

### SC-010: `ep_annihilation` generator in R37 does not validate ERA port connectivity
**Response:** ACCEPTED
**Action taken:** Added new requirement R61 in Section 3.13 ("Generator Arity-Aware Validation") specifying that T1 validation MUST iterate only over ports `0..=arity(agent.symbol)` for each live agent. For ERA agents (arity 0), only port 0 is checked. Port array slots at indices `id*3+1` and `id*3+2` for ERA agents are unused and MUST NOT be validated against T1. This is consistent with SPEC-01 T1's formal statement.
**Spec sections modified:** Section 3.13 (new -- R61)

### SC-011: Reduction Summary MIPS calculation assumes wall-clock duration
**Response:** ACCEPTED
**Action taken:** Added requirements R44a and R44b. R44a defines `duration_secs` for the `reduce` subcommand as wall-clock time of `reduce_all` only (excluding file I/O). R44b states that `Speedup`, `Efficiency`, and `Overhead` in the coordinator summary are conditional on a `--baseline-secs <FLOAT>` flag; if not provided, those lines are omitted. This avoids requiring the coordinator to run a sequential baseline internally (which would be expensive) and delegates comprehensive baseline comparison to the benchmark suite (SPEC-09 R3). The R46 example output was updated to show the conditional lines.
**Spec sections modified:** Section 3.6 (R44a, R44b, R46 example)

### SC-012: Text DSL does not support self-wiring (port connected to itself)
**Response:** PARTIALLY ACCEPTED
**Action taken:** Added R58 (self-loop rejection: `wire a.left a.left` is a parse error) and R59 (free-to-free rejection: `wire free(0) free(1)` is a parse error). The critic also raised the question of `wire a.left a.right` (two different ports of the same agent). This IS permitted -- it is legitimate in IC nets (e.g., Church numeral 0 has `lambda_x.p1 <-> lambda_x.p2`). Only port-to-self (same port reference appearing twice) is rejected. Added tests T12 and T13 for these cases.

The critic's broader question about whether T1's "exactly one other port" formally prohibits self-loops is answered: SPEC-01 T1's formal statement says `ports[agent_port(a.id, p)] == q` where q is required to be a different port reference. Self-loops (`q == agent_port(a.id, p)`) violate this because the bidirectionality check `ports[port_index(q)] == agent_port(a.id, p)` is trivially satisfied but the connection is degenerate (the port is connected to itself, not to "one other port").
**Spec sections modified:** Section 3.11 (new -- R58, R59), Section 7 (T12, T13)

### SC-013: Inspect wire count computation is ambiguous
**Response:** ACCEPTED
**Action taken:** Updated R29's wire count description to precisely define: "Number of distinct agent-to-agent connections: pairs (A, B) where both A and B are `AgentPort` entries, `ports[A] == B`, and `A < B` by canonical ordering `(id_a * 3 + port_a)`. FreePort connections are NOT counted as wires." Added pseudocode in Section 4.4 showing the actual wire count loop using the canonical ordering comparison.
**Spec sections modified:** Section 3.4 R29, Section 4.4 (wire count pseudocode)

### SC-014: Test T5 expected values may be wrong for `ep_annihilation(10)`
**Response:** ACCEPTED
**Action taken:** Added a note to T5: "For freshly generated nets (no reductions applied), the redex queue contains no stale entries, so the reported redex count equals the raw queue length." The test values themselves are confirmed correct.
**Spec sections modified:** Section 7 T5

### SC-015: Generator `ep_annihilation_dup` not defined
**Response:** ACCEPTED
**Action taken:** Added R38a defining `ep_annihilation_dup(n: u32) -> Net` with full Rust pseudocode. It is structurally identical to R38 but using `Symbol::Dup`.
**Spec sections modified:** Section 3.5 R38a (new)

### SC-016: Church numeral generators in ExampleNet overlap with SPEC-14 encoding module
**Response:** ACCEPTED
**Action taken:** Fixed all three cross-references from the non-existent "SPEC-14 R26" to the correct requirement numbers: `ChurchNat` -> SPEC-14 R4, `ChurchAdd` -> SPEC-14 R15, `ChurchMul` -> SPEC-14 R16. Added a note in Section 4.3 clarifying that Church generators in `io/examples.rs` are thin wrappers calling `encoding::encode_nat`, `encoding::build_add`, and `encoding::build_mul`.
**Spec sections modified:** Section 3.5 R33 (ExampleNet enum), Section 4.3

### SC-017: `format` flag on `ReduceArgs` is ambiguous -- input or output?
**Response:** ACCEPTED
**Action taken:** Renamed the field from `format` to `input_format` in the `ReduceArgs` struct (R24) and updated the doc comment to "Override input format detection (does not affect output format)." Clarified in R18 that `--input-format` applies only to input; output format ALWAYS follows the output file's extension. Updated R50 to refer to `--input-format` instead of `--format`.
**Spec sections modified:** Section 3.2 R18, Section 3.3 R24, Section 3.7 R50

### SC-018: No requirement for graceful handling of empty nets
**Response:** ACCEPTED
**Action taken:** Added R60 in Section 3.12 ("Empty Net and Size Zero") requiring all generators to accept `size = 0` and produce an empty net. Added test T10 (size zero for all generators), T14 (inspect and reduce on empty nets).
**Spec sections modified:** Section 3.12 (new -- R60), Section 7 (T10, T14)

### SC-019: CSV schema for per-round output (R22) lacks column for `round` timing
**Response:** PARTIALLY ACCEPTED
**Action taken:** Updated R22's CSV schema to align with SPEC-07 R29's more detailed schema, including phase-level timing columns (`partition_time_ms`, `compute_time_ms`, `merge_time_ms`, `network_send_time_ms`, `network_recv_time_ms`), network traffic columns (`bytes_sent`, `bytes_received`), and interaction breakdown (`local_interactions`, `border_interactions`). The critic also suggested removing R22 entirely and deferring to SPEC-09; this was rejected because the per-round CSV is a useful standalone feature for the coordinator subcommand, independent of the benchmark framework.
**Spec sections modified:** Section 3.2 R22

### SC-020: `ReductionSummary` struct not defined
**Response:** ACCEPTED
**Action taken:** Added the `ReductionSummary` struct definition to Section 4.4 with fields: `input: NetSummary`, `output: NetSummary`, `total_interactions: u64`, `duration_secs: f64`, `mips: f64`, `normal_form: bool`, `termination_reason: Option<String>`, and optional grid fields (`rounds`, `workers`, `speedup`, `efficiency`, `overhead_pct` -- all `Option` types). The grid-specific fields are `None` for local `reduce` and conditionally populated for coordinator mode based on `--baseline-secs` (R44b).
**Spec sections modified:** Section 4.4

---

## Changes Made to SPEC-12

### Header
- Status updated from "Draft v1" to "Revised v2"

### Section 1 (Purpose)
- Added cross-spec note documenting the required SPEC-13 R5 amendment for the `io` module
- Added format supersession note (SPEC-12 R1-R50 supersede SPEC-07 R22-R25 and SPEC-13 R42)
- Added subcommand clarification note (`reduce` vs `local` coexistence)

### Section 3.1.2 (Text DSL)
- R13: Rewrote CON-CON example comment to accurately describe parallel input wiring and cross reconnection during reduction

### Section 3.2 (Output Formats)
- R18: Clarified `--input-format` applies only to input; output format follows output extension
- R22: Aligned per-round CSV schema with SPEC-07 R29

### Section 3.3 (reduce subcommand)
- R24: Renamed `format` field to `input_format` with updated doc comment

### Section 3.5 (generate subcommand)
- R33: Fixed Church numeral cross-references (SPEC-14 R26 -> R4, R15, R16); added size warnings
- R36: Removed incorrect "SPEC-13, R5" reference; pointed to Section 4.3 with amendment note
- R38a: New requirement with full pseudocode for `ep_annihilation_dup`
- R39: Added recursive tree construction pseudocode for `dual_tree`
- R40: Added full pseudocode for `con_dup_expansion`
- R41: Added full implementation pseudocode for `mixed_rules` with all 6 pair types; added note on pair independence
- R42a: New requirement with full pseudocode for `erasure_propagation`

### Section 3.6 (Reduction Summary)
- R44a: New requirement defining `duration_secs` scope for `reduce` (excludes file I/O)
- R44b: New requirement making Speedup/Efficiency/Overhead conditional on `--baseline-secs`
- R46: Updated example to show conditional speedup/efficiency/overhead lines

### Section 3.7 (File Format Detection)
- R50: Renamed from `--format` to `--input-format`

### Section 3.8 (I/O Module API)
- R51: Changed `parse_ic` return type from `ParseError` to `FileIoError`; added explanatory note
- R52: Renamed `IoError` to `FileIoError`; added composition note for SPEC-13 R17

### Section 3.10 (new -- Root Declaration Semantics)
- R54: At most one root declaration per file
- R55: No root -> `net.root = None`
- R56: Root must reference a valid port; both AgentPort and FreePort permitted
- R57: Root is not a wire; clarified counting rules

### Section 3.11 (new -- Text DSL Edge Cases)
- R58: Self-loop rejection (port-to-self is a parse error)
- R59: Free-to-free wire rejection (parse error)

### Section 3.12 (new -- Empty Net and Size Zero)
- R60: All generators must accept size=0 and produce empty nets

### Section 3.13 (new -- Generator Arity-Aware Validation)
- R61: T1 validation must iterate only over ports 0..=arity(agent.symbol)

### Section 4.3 (Generator Sharing)
- Updated module layout with Core/Infrastructure annotations
- Added note about required SPEC-13 R5 amendment
- Added note about Church generator wrappers calling encoding module

### Section 4.4 (Net Summary Computation)
- Added wire count and free port count pseudocode with canonical ordering
- Added `ReductionSummary` struct definition

### Section 7 (Test Requirements)
- T5: Added note about stale filtering on fresh nets
- T10: New test for size=0 generators
- T11: New test for root declaration edge cases
- T12: New test for self-loop rejection
- T13: New test for free-to-free rejection
- T14: New test for empty net handling

---

## Residual Risks

### SC-008 (PARTIALLY ACCEPTED): Church numeral size limits are advisory, not enforced

The size warnings on `ChurchNat`, `ChurchAdd`, and `ChurchMul` are documented in comments (SHOULD-level) but not enforced by validation code (no MUST). This means a user could call `relativist generate --example church-mul --size 4000000000` and encounter an AgentId overflow during reduction. This is an acceptable risk because:

1. The reduction engine must handle u32 overflow regardless (it is a general concern, not specific to generators).
2. Adding a pre-generation size check would require duplicating complexity analysis from the reduction engine.
3. The TCC benchmarks use small sizes (N <= 1000 for Church numerals), so the risk does not affect experimental results.

### SC-012 (PARTIALLY ACCEPTED): Free-to-free wires rejected rather than stored

The decision to reject `wire free(0) free(1)` (rather than storing it in a separate data structure) means that certain valid Lafont nets with free-to-free connections cannot be expressed in the text DSL. This is an acceptable limitation because:

1. The port array (SPEC-02 R8) has no slots for free ports, so storing free-to-free connections would require a separate data structure not defined in SPEC-02.
2. Free-to-free connections are rare in practice and irrelevant for the TCC benchmarks.
3. The binary and JSON formats can represent any `Net` struct directly; the text DSL is for small human-authored examples where free-to-free connections are unlikely.

### SC-019 (PARTIALLY ACCEPTED): Per-round CSV is a SHOULD, not a MUST

The per-round CSV (R22) is SHOULD-level. If implementation time is constrained, it may be deferred. The benchmark suite (SPEC-09) produces its own detailed output and does not depend on the coordinator's CSV feature.
