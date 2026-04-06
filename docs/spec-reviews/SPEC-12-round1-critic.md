# SPEC-12 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-12-user-io.md (status: Draft v1)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-03, SPEC-07, SPEC-09, SPEC-13, SPEC-14

---

## Overall Assessment

SPEC-12 is a well-structured spec that fills a genuine usability gap by introducing text DSL support, the `inspect` subcommand, and shared generators. However, it introduces a new `io` module that does not exist in SPEC-13's canonical module structure (R5), creates contradictions with SPEC-07 and SPEC-13 on input formats and CLI subcommand semantics, defines a duplicate `IoError` type that overlaps with the per-module error enums already defined in SPEC-13 (R15-R16), and leaves several important parser edge cases unspecified. The generators are well-defined for the simple cases but lack pseudocode for the non-trivial generators (`dual_tree`, `tree_sum`, `tree_sum_balanced`, `erasure_propagation`, `mixed_rules`). The spec also references SPEC-09 benchmark IDs that do not match the SPEC-09 requirement numbers in several places.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: The `io` module does not exist in SPEC-13's module structure
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.8 (R51), 4.3
**Requirement:** R36, R51
**Problem:** SPEC-12 R36 states generators MUST be implemented in the "io module (SPEC-13, R5) or a shared examples submodule." SPEC-12 R51 defines a public API for the `io` module and Section 4.3 shows a module layout `io/ { mod.rs, binary.rs, text.rs, json.rs, examples.rs }`. However, SPEC-13 R5 defines exactly 11 modules for the crate and the `io` module is NOT among them. The 11 modules are: `net`, `reduction`, `partition`, `merge`, `encoding`, `protocol`, `coordinator`, `worker`, `config`, `observability`, `security`. There is no `io` module, no reference to it, and no placeholder for it.

Furthermore, SPEC-13 R6 defines the Core Layer as `net`, `reduction`, `partition`, `merge`, `encoding`. An `io` module that reads/writes files clearly involves I/O (file system access), so it cannot be part of the Core Layer per R6. Yet the generators (pure functions) and the text DSL parser/serializer (pure string functions) SHOULD be Core Layer operations. The `io` module conflates pure logic (parsing, formatting, generation) with impure I/O (file reads/writes), violating the Core/Infrastructure separation principle.

**Impact if unresolved:** The implementer cannot determine where to place 6+ source files (binary.rs, text.rs, json.rs, examples.rs, mod.rs). SPEC-13 R5 says "11 modules" with MUST -- adding a 12th module violates SPEC-13. If the implementer follows SPEC-13, there is no home for the I/O layer code.
**Suggested resolution:** Either (a) amend SPEC-13 R5 to add a 12th module `io/` and classify it: the pure parts (parser, serializer, generators) as Core Layer, and the file I/O parts (load_net, save_net) as a thin Infrastructure Layer wrapper; or (b) split the `io` module into two locations: generators in `encoding/examples.rs` or a new Core Layer submodule, and file I/O in `config/` or a new Infrastructure module. The SPEC-12 MUST explicitly state which spec takes precedence and document the required amendment to SPEC-13.

---

### SC-002: Input format contradicts SPEC-07 (bincode-only) and SPEC-13 R42
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.1 (R1), 3.7 (R48-R50)
**Requirement:** R1, R48-R50
**Problem:** SPEC-12 R1 mandates three input formats: Binary (.bin), Text DSL (.ic), and JSON (.json). SPEC-07 R22 states: "The input format for IC networks MUST be a binary file containing the Net serialized with serde + bincode." SPEC-07 mentions no .ic or .json format for ANY subcommand. SPEC-13 R42 says: "The input format for nets MUST be bincode (SPEC-02, SPEC-06). The system SHOULD also accept a human-readable JSON format for debugging." SPEC-13 does not mention a text DSL (.ic) format at all.

The contradictions:
- SPEC-07 R22: MUST be bincode only. SPEC-12 R1: MUST support three formats.
- SPEC-13 R42: MUST be bincode, SHOULD accept JSON. SPEC-12 R1: Text DSL (.ic) is MUST, not mentioned by SPEC-13.
- SPEC-07 R25: output MUST be bincode. SPEC-12 R18: output format inferred from extension (could be .ic or .json).

**Impact if unresolved:** The implementer does not know whether the `coordinator` and `local` subcommands must accept .ic files. If they only accept .bin (per SPEC-07), but `reduce` accepts .ic (per SPEC-12), the behavior is inconsistent. The `--output` behavior is also unclear: can the coordinator write .ic output?
**Suggested resolution:** SPEC-12 MUST explicitly state that it supersedes SPEC-07 R22-R25 and SPEC-13 R42 for the input/output format definition. Add a note: "SPEC-12 R1-R50 supersede SPEC-07 R22-R25 and SPEC-13 R42 for all file format specifications. The three-format support defined here applies to all subcommands that read or write net files." Alternatively, restrict .ic and .json to the `reduce`, `inspect`, and `generate` subcommands only, with `coordinator`/`worker`/`local` accepting only .bin for performance.

---

### SC-003: `reduce` vs `local` subcommand semantics remain unresolved
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.3 (R23)
**Requirement:** R23
**Problem:** SPEC-12 R23 defines `reduce` as calling `reduce_all()` directly without network communication. SPEC-07 R1 defines `local` as running "the grid loop entirely in-process." SPEC-13 R41 defines `reduce` as bypassing coordinator/worker/protocol entirely and calling `reduce_all` directly. The SPEC-13 Round 1 review (SC-005) already flagged this: `local` (grid simulation with partitioning) and `reduce` (direct sequential reduction) are semantically different operations.

SPEC-12 defines `reduce` but makes no mention of `local`. The implementer does not know:
1. Does `reduce` replace `local`, or do both exist?
2. If both exist, does `local` also support .ic input?
3. SPEC-09 benchmarks reference `Local` mode (in-memory grid) -- which subcommand runs those?

**Impact if unresolved:** The benchmark suite (SPEC-09) requires `Local` mode for in-memory grid benchmarks with partitioning. If only `reduce` exists (no partitioning), in-memory grid benchmarks have no CLI entry point.
**Suggested resolution:** SPEC-12 MUST clarify: (a) `reduce` is for purely sequential reduction (no partitioning), (b) `local` (from SPEC-07) is for in-memory grid simulation (with partitioning), and (c) both subcommands coexist. Update the CLI enum to include both. This aligns with the resolution suggested in SPEC-13 SC-005.

---

### SC-004: IoError type duplicates and conflicts with SPEC-13 error enums
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.8 (R52)
**Requirement:** R52
**Problem:** SPEC-12 R52 defines an `IoError` enum with variants `Io`, `Parse`, `Serialization`, `UnrecognizedFormat`, `UnsupportedFormat`. SPEC-13 R15-R17 mandates per-module error enums and a top-level `RelativistError` that unifies them via `#[from]` conversions. The `RelativistError` in SPEC-13 R17 has an `Io(#[from] std::io::Error)` variant -- which would conflict with `IoError::Io(#[from] std::io::Error)` if both are composed via `#[from]`.

Additionally, SPEC-12's `IoError::Parse { line, message }` overlaps with potential parsing errors in the `encoding` module (SPEC-14) for Church numeral readback failures. And the `IoError::Serialization` variant overlaps with bincode serialization errors that SPEC-06 (wire protocol) also handles.

More fundamentally, SPEC-13 R15 says "Each module MUST define its own error enum." If the `io` module does not exist in SPEC-13, there is no module to own `IoError`.

**Impact if unresolved:** Two competing `#[from] std::io::Error` conversions would cause a Rust compilation error when composing into `RelativistError`. The implementer must resolve this manually.
**Suggested resolution:** (a) Add `IoError` to SPEC-13 R17's `RelativistError` enum once the `io` module is added to the module structure (per SC-001). (b) Remove the `Io(#[from] std::io::Error)` variant from `RelativistError` since `IoError` already wraps it. (c) Alternatively, rename `IoError` to `FileIoError` or `NetIoError` to avoid name collision with the std trait.

---

### SC-005: `parse_ic` returns `ParseError` but only `IoError` is defined
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.8 (R51, R52)
**Requirement:** R51
**Problem:** The `parse_ic` function signature in R51 returns `Result<Net, ParseError>`, but SPEC-12 never defines a `ParseError` type. R52 defines `IoError` with a `Parse { line, message }` variant, but `ParseError` and `IoError` are different types. The spec either needs to define `ParseError` as a separate type (with its own fields), or change the `parse_ic` signature to return `Result<Net, IoError>`.

**Impact if unresolved:** The implementer must invent the `ParseError` type or guess that it should be `IoError`.
**Suggested resolution:** Either (a) define `ParseError` as a dedicated struct: `pub struct ParseError { pub line: usize, pub message: String }` with an `impl From<ParseError> for IoError`, or (b) change the `parse_ic` signature to return `Result<Net, IoError>` since `IoError` already has a `Parse` variant. Option (b) is simpler and avoids introducing yet another error type.

---

### SC-006: Text DSL grammar does not handle `root` declaration semantics
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.1.2 (R7, R8)
**Requirement:** R7
**Problem:** The grammar includes `root_decl ::= 'root' port_ref`, and Section 4.2 (serializer) says "Emit `root <port_ref>` if the net has a root port." However, SPEC-02 R6 defines `root: Option<PortRef>` on the `Net` struct. The SPEC-12 grammar and design sections do not specify:

1. What happens if multiple `root` declarations appear? Is only the last one used? Is it an error?
2. What happens if no `root` declaration appears? Is `net.root` set to `None`?
3. If `root free(0)` is specified, is the root a FreePort? Or must it be an AgentPort? SPEC-02 Section 4.9 says `net.root = Some(PortRef::AgentPort(root_agent, 0))` (only AgentPort example), but the grammar allows `free_port` as a `port_ref` for `root`.
4. The `root` declaration is not a wire -- it sets the `net.root` field. But R11 only validates T1 (port linearity) and I2 (reference validity). Does the root port need to be a valid port? Is the root port counted in the "free port count" reported by `inspect`?

**Impact if unresolved:** Ambiguous parser behavior for the `root` declaration. Test writers cannot predict expected behavior.
**Suggested resolution:** Add the following requirements: (a) At most one `root` declaration is allowed per file; duplicate `root` declarations MUST produce a parse error. (b) If no `root` declaration is present, `net.root` MUST be `None`. (c) The `root` port reference MUST be a valid port in the constructed net (validated alongside T1 and I2). (d) Clarify whether `root free(N)` sets `net.root = Some(FreePort(N))` (valid use case for Lafont interface ports) or is restricted to `AgentPort` references.

---

### SC-007: No pseudocode for non-trivial generators
**Severity:** HIGH
**Axis:** Completeness
**Section:** 3.5 (R39-R42)
**Requirement:** R39, R40, R41, R42
**Problem:** SPEC-12 provides full Rust pseudocode for `ep_annihilation` (R37) and `ep_annihilation_con` (R38), but the remaining 6 generators have only prose descriptions:

- R39 (`dual_tree`): "MUST produce two perfect binary trees of CON agents with the given depth, connected at the roots via principal ports." No pseudocode.
- R40 (`con_dup_expansion`): "MUST produce N CON-DUP pairs connected at principal ports." No pseudocode.
- R41 (`mixed_rules`): "MUST produce a net containing N pairs of each of the 6 interaction rule types." No pseudocode. How are the erasure pairs (CON-ERA, DUP-ERA) connected? The CON/DUP needs auxiliary ports connected somewhere -- to free ports? To each other?
- R42 (`tree_sum`, `tree_sum_balanced`, `erasure_propagation`): "MUST follow the benchmark definitions in SPEC-09." But SPEC-09 R14, R15, R17 also lack pseudocode for these generators.

The `mixed_rules` generator is particularly underspecified: it must create CON-ERA and DUP-ERA pairs, but these require the CON/DUP agent to have its auxiliary ports connected to something. If they connect to free ports, the net grows; if they connect to each other, the topology is unclear. Additionally, the CON-DUP pairs in `mixed_rules` will create new agents during reduction that may interact with other pairs, making the generator's post-reduction behavior difficult to predict and verify.

**Impact if unresolved:** The implementer must invent the topology for 6 generators. Different implementations could produce structurally different nets, making benchmark results non-reproducible across implementations.
**Suggested resolution:** Add Rust pseudocode (or at minimum, precise topology descriptions with port connectivity tables) for all generators. For `dual_tree`, show the recursive tree construction. For `mixed_rules`, specify exactly how the 6 pair types are wired (especially auxiliary ports of erasure pairs). For `tree_sum` and `erasure_propagation`, provide the chain construction logic.

---

### SC-008: `generate` subcommand size parameter is `u32` but SPEC-09 uses `u32` and SPEC-14 uses `u64`
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.5 (R33, R35)
**Requirement:** R33, R35
**Problem:** The `GenerateArgs` struct (R33) defines `size: u32`. The generator function signatures (R35) use `fn generate_<name>(size: u32) -> Net`. The `ExampleNet` enum (R33) includes `ChurchNat`, `ChurchAdd`, and `ChurchMul` which are defined in SPEC-14. But SPEC-14's encoding functions use `u64`: `encode_nat(n: u64)`, `build_add(a: u64, b: u64)`, `build_mul(a: u64, b: u64)`.

If the `generate` subcommand passes a `u32` size to a Church numeral generator that calls `encode_nat(n: u64)`, the cast is safe (u32 -> u64 widening). But the function signature `fn generate_<name>(size: u32) -> Net` in R35 means the Church generators cannot expose the full u64 range of SPEC-14.

More critically, the `ChurchMul` variant says: "church(floor(sqrt(N))) * church(floor(sqrt(N)))." If N is the size parameter (u32), then the operands are `sqrt(N)`. For N = 4_000_000_000 (max u32), `sqrt(N)` is ~63,245. `church(63245) * church(63245)` would produce a net with ~126,490 agents before reduction and ~4 billion interactions. The `AgentId` space is `u32` (~4 billion), so the reduction could exhaust the ID space.

**Impact if unresolved:** Type mismatch at the boundary between SPEC-12 generators and SPEC-14 encoding functions. Large Church numeral sizes could exhaust the AgentId space.
**Suggested resolution:** (a) Document that Church numeral generators SHOULD use small size values (N <= 10,000 for ChurchNat, proportionally smaller for ChurchMul). (b) Add a validation check in the Church generators: if the expected agent count exceeds a configurable threshold (e.g., 1,000,000), print a warning. (c) Consider changing the generator signature to accept `u64` for forward-compatibility, or add a `--large-size` flag with u64 for Church-specific generators.

---

### SC-009: R13 example comment says "cross-wise" but wires show parallel connectivity
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.1.2 (R13)
**Requirement:** R13
**Problem:** The R13 informative example says in its comment: "Auxiliary ports are connected cross-wise." But the wire declarations show:
```
wire a.left b.left
wire a.right b.right
```
This is parallel connectivity (left-to-left, right-to-right), not cross-wise. Cross-wise would be `wire a.left b.right` and `wire a.right b.left`.

The comment appears to describe what happens *after reduction* (CON-CON annihilation cross-reconnects), but the wires describe the *input* topology. The comment also says "After reduction: 0 agents, free ports reconnected" -- but there are no free ports in this net; all ports connect to other agents. The reduction removes both agents and the cross-reconnection targets are the auxiliary ports of the removed agents themselves, yielding an empty net.

**Impact if unresolved:** A reader following the comment would think the input wires are cross-connected (which they are not). A test writer implementing this example might wire cross-wise instead of parallel, producing a different net.
**Suggested resolution:** Rewrite the R13 comment to clarify: "Input: two CON agents with principal ports connected and auxiliary ports connected in parallel (left-left, right-right). After CON-CON annihilation (cross reconnection): both agents removed, auxiliary targets are reconnected cross-wise. Since targets are the other agent's auxiliary ports (which are also removed), the result is an empty net (0 agents, 0 wires)."

---

### SC-010: `ep_annihilation` generator in R37 does not validate ERA port connectivity
**Severity:** MEDIUM
**Axis:** Invariant Preservation
**Section:** 3.5 (R37)
**Requirement:** R37
**Problem:** The pseudocode for `ep_annihilation` (R37) creates ERA-ERA pairs and connects their principal ports. ERA agents have arity 0, meaning they have only port 0 (the principal port). After `connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0))`, both ERA agents satisfy T1 -- their only port is connected. However, the port array is sized `agents.len() * 3` (SPEC-02 R8), meaning slots `a*3+1`, `a*3+2` exist even for ERA agents but are unused.

The question is: what values do these unused port array slots contain? SPEC-02 does not define a sentinel for "unused slot of an ERA agent" vs "disconnected port of a CON/DUP agent." If they contain `DISCONNECTED` (same sentinel used for removed agents), the assertion in T1 verification might incorrectly flag ERA slots as "dangling ports."

SPEC-12 R34 says generators MUST produce valid nets satisfying T1-T7. But T1 says "Every port of every live agent MUST be connected to exactly one other port." For ERA agents with arity 0, ports 1 and 2 are not valid ports. The T1 verification ("traverses all live agents and verifies that each port has exactly one entry in the port array") must know to skip ports beyond the agent's arity. This is a SPEC-01/SPEC-02 concern, but SPEC-12's generators are the first place it becomes concrete.

**Impact if unresolved:** If the T1 debug assertion iterates over all 3 slots for ERA agents and checks connectivity, every ERA-only net will fail the assertion. The generator would violate R34.
**Suggested resolution:** Clarify in SPEC-12 R34 (or add a note referencing SPEC-01 T1) that the T1 verification for generators MUST iterate only over ports `0..=arity(agent.symbol)` for each agent, skipping slots beyond the agent's arity. This is consistent with SPEC-01 T1's formal statement which says "every port index `p` in `0..=arity(a.symbol)`."

---

### SC-011: Reduction Summary MIPS calculation assumes wall-clock duration
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.6 (R45)
**Requirement:** R45
**Problem:** The Reduction Summary includes `MIPS: 0.776`. SPEC-09 defines MIPS as `total_interactions / wall_clock_seconds / 1_000_000`. The metrics JSON (R21) includes `duration_secs: 1.234` and `mips: 0.776`. But SPEC-12 never defines how `duration_secs` is measured for the `reduce` subcommand. Is it:
- Wall-clock time from the start of `reduce_all` to completion (excludes I/O)?
- Wall-clock time from file load to file write (includes I/O)?
- Wall-clock time from process start to exit?

For benchmarks (SPEC-09), MIPS uses `wall_clock_secs` which is total execution time. For the `reduce` subcommand, the user likely cares about reduction time only (excluding file parsing/serialization). The spec should be explicit.

Additionally, for distributed mode (R46), the Reduction Summary includes `Speedup: 3.2x (vs sequential baseline)`. But the `coordinator` subcommand does not run a sequential baseline -- it only runs the distributed reduction. Where does the sequential baseline timing come from? Is the user expected to run `reduce` separately and compare manually?

**Impact if unresolved:** MIPS values will vary depending on what the implementer includes in the timing, making comparisons unreliable. Speedup calculation for `coordinator` is undefined.
**Suggested resolution:** (a) Define `duration_secs` for the `reduce` subcommand as the wall-clock time of the `reduce_all` call only (excluding file I/O). (b) For the `coordinator` subcommand, either: require the coordinator to also run a sequential baseline internally before distribution (expensive but self-contained), or omit `Speedup` and `Efficiency` from the coordinator summary (defer to the benchmark suite for those metrics), or accept a `--baseline-secs` flag for manual comparison.

---

### SC-012: Text DSL does not support self-wiring (port connected to itself)
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.1.2 (R7)
**Requirement:** R7
**Problem:** The grammar allows `wire port_ref port_ref`, where both port_refs could be the same agent port (e.g., `wire a.left a.right`). SPEC-02 and SPEC-01 T1 do not prohibit self-wiring (a port of an agent connected to another port of the same agent). Self-wiring is legitimate in IC nets -- for example, Church numeral 0 has `lambda_x.p1 <-> lambda_x.p2` (SPEC-14, R5).

However, what about `wire a.left a.left`? A port connected to itself. T1 says each port is connected to "exactly one other port" -- "other" suggests a different port. But the formal statement says `ports[agent_port(a.id, p)] == q` where q could equal `agent_port(a.id, p)`. The spec should clarify whether self-loops (port-to-self) are permitted or rejected by the parser.

Additionally, what about `wire free(0) free(1)`? Two free ports connected to each other, with no agents involved. This is a valid wire in Lafont's theory but would not be stored in the port array (SPEC-02 R8: the port array only has slots for agent ports). The parser must handle or reject this case.

**Impact if unresolved:** Edge cases in the parser that may cause subtle bugs or inconsistencies with the formal model.
**Suggested resolution:** Add requirements: (a) `wire port_ref port_ref` where both port_refs are identical MUST be rejected with a parse error ("port cannot be connected to itself"). (b) `wire free(N) free(M)` MUST be handled: either stored in a separate structure (since the port array has no slots for free-to-free wires) or rejected with an appropriate error message explaining that free-to-free wires require agent intermediaries.

---

### SC-013: Inspect wire count computation is ambiguous
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 3.4 (R29)
**Requirement:** R29
**Problem:** R29 says wire count is "Bidirectional connections / 2." The port array stores connections bidirectionally: if port A connects to port B, then `ports[A] == B` and `ports[B] == A`. So the wire count is `(number of occupied port array entries) / 2`. But:

1. What about FreePort connections? If `ports[agent_port(a, 1)] == FreePort(5)`, does that count as a wire? The FreePort has no reverse entry in the port array (SPEC-02 R8: "port array of size `agents.len() * 3`" -- no slots for free ports). So `FreePort -> AgentPort` entries are not bidirectional in the port array. These would not be counted in "bidirectional connections / 2."
2. What about DISCONNECTED slots? How are they distinguished from FreePort connections?
3. The pseudocode in Section 4.4 (`net_summary`) elides the wire count computation with `// ... wire count, redex count, free port count ...`. No actual algorithm is given.

**Impact if unresolved:** Different implementers may compute different wire counts for the same net, making `inspect` output non-reproducible.
**Suggested resolution:** Define wire count precisely: "Wire count = the number of distinct pairs (A, B) where A < B (by canonical ordering), `ports[A]` is defined, `ports[A] == B`, and both A and B are `AgentPort` entries. FreePort connections are NOT counted as wires; they are counted separately as 'free port count.'" Provide pseudocode for the wire count computation.

---

### SC-014: Test T5 expected values may be wrong for `ep_annihilation(10)`
**Severity:** MEDIUM
**Axis:** Testability
**Section:** 7 (T5)
**Requirement:** T5
**Problem:** Test T5 says: "for `ep_annihilation(10)`, verify that all reported statistics match expected values (20 agents, 10 redexes, 0 CON, 0 DUP, 20 ERA, 0 free ports, not normal form)."

The expected value "not normal form" implies 10 redexes exist. This is correct: `ep_annihilation(10)` creates 10 ERA-ERA pairs connected at principal ports, producing 10 redexes. However, the test says "0 free ports." ERA agents have arity 0 (only port 0, the principal port). All principal ports are connected to other principal ports (forming redexes). So there are indeed 0 free ports. But the claim "not normal form" is correct only if we trust the inspect command to correctly report the redex queue as non-empty.

The issue is that T5 does not specify whether "redex count" uses the raw queue length or the stale-filtered count. R29 says "Current entries in the redex queue (after stale filtering)." For a freshly generated net with no reductions applied, there are no stale entries, so both counts are 10. But the spec should note this assumption.

**Impact if unresolved:** Minor -- the test values are correct. But the test should note that stale filtering is irrelevant for freshly generated nets.
**Suggested resolution:** Add a note to T5: "For freshly generated nets (no reductions applied), the redex queue contains no stale entries, so the reported redex count equals the raw queue length."

---

### SC-015: Generator `ep_annihilation_dup` not defined
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.5 (R33)
**Requirement:** R33
**Problem:** The `ExampleNet` enum (R33) includes `EpAnnihilationDup` with description "N DUP-DUP annihilation pairs (Profile A). SPEC-09 R11." But SPEC-12 provides no generator requirement or pseudocode for `ep_annihilation_dup`. R38 defines `ep_annihilation_con` with full pseudocode, but there is no corresponding R-number for `ep_annihilation_dup`. The implementation is trivially analogous to R38 (replace `Symbol::Con` with `Symbol::Dup`), but it is not formally specified.

**Impact if unresolved:** The implementer must infer the generator implementation. Since DUP-DUP annihilation uses parallel reconnection (not cross), the generator itself is identical to CON-CON in structure (the reconnection pattern only matters during reduction, not generation). But it should still be formally specified.
**Suggested resolution:** Add an R-number (e.g., R38a) defining `ep_annihilation_dup(n: u32) -> Net` with the same structure as R38 but using `Symbol::Dup`. Or add a note to R38 stating that the DUP variant is structurally identical.

---

### SC-016: Church numeral generators in ExampleNet overlap with SPEC-14 encoding module
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.5 (R33)
**Requirement:** R33
**Problem:** The `ExampleNet` enum includes `ChurchNat`, `ChurchAdd`, and `ChurchMul`, referencing "SPEC-14 R26." But SPEC-14 defines `encode_nat`, `build_add`, `build_mul` as functions in the `encoding` module (Core Layer, `src/encoding/`). SPEC-12 says generators reside in the `io` module or a shared `examples` submodule (R36). This creates a layering question: are the Church generators in the `io` module (calling into `encoding`) or directly in `encoding`?

Additionally, SPEC-14 R26 does not exist -- SPEC-14's requirements go up to R25 (ComputeArgs). The reference "SPEC-14 R26" appears three times in the ExampleNet enum comments.

**Impact if unresolved:** Incorrect cross-reference (R26 does not exist in SPEC-14). Minor layering confusion.
**Suggested resolution:** (a) Fix the cross-reference to the correct SPEC-14 requirement numbers (R4 for encode_nat, R15 for build_add, R16 for build_mul). (b) Clarify that Church generators in the `io/examples.rs` module are thin wrappers calling `encoding::encode_nat`, `encoding::build_add`, etc.

---

### SC-017: `format` flag on `ReduceArgs` is ambiguous -- input or output?
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.3 (R24)
**Requirement:** R24
**Problem:** The `ReduceArgs` struct (R24) has a `--format` flag with doc comment "Override input format detection." But R18 says the output format is "determined by the output file's extension (or the `--format` flag)." So `--format` overrides BOTH input and output format detection? Or is there a separate `--output-format`?

If `--format bin` is specified and the input is `net.ic`, then: (a) input is read as .bin (overriding .ic extension), and (b) if `--output net.ic` is specified, output is written as .bin (overriding .ic extension)?

This is confusing. A single `--format` flag serving dual purpose (input and output) creates ambiguity.

**Impact if unresolved:** User confusion. The behavior when `--format bin --input net.ic --output result.ic` is specified is unclear.
**Suggested resolution:** Either (a) split into `--input-format` and `--output-format` flags, each overriding the respective extension detection, or (b) clarify that `--format` applies ONLY to input detection and output format always follows the output extension, or (c) document the precedence rules explicitly: `--format` overrides input; output follows its own extension unless `--format` is the only format specified.

---

### SC-018: No requirement for graceful handling of empty nets
**Severity:** LOW
**Axis:** Testability
**Section:** 3.3, 3.4, 3.5
**Requirement:** R23, R27, R32
**Problem:** What happens when:
- `reduce` is called on an empty net (0 agents, 0 redexes)? It is already in Normal Form. The spec says `reduce_all()` is called; it should return immediately with 0 interactions.
- `inspect` is called on an empty net? All statistics should be 0, normal_form should be true.
- `generate` is called with `size=0`? For `ep_annihilation(0)`: the loop runs 0 times, producing an empty net. Is this valid?

These are boundary conditions that MUST be testable. The spec does not explicitly address them.

**Impact if unresolved:** Edge case bugs. The `generate` subcommand might reject size=0 or produce invalid nets.
**Suggested resolution:** Add a note that size=0 MUST be a valid input for all generators, producing an empty net (or the minimal valid net for that generator). Add test cases for size=0 to Section 7.

---

### SC-019: CSV schema for per-round output (R22) lacks column for `round` timing
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.2 (R22)
**Requirement:** R22
**Problem:** R22 specifies a per-round CSV schema for distributed execution: `round,duration_ms,local_redexes,border_redexes,interactions,agents_before,agents_after`. But SPEC-09 R18 requires phase-level timing: `partition_time_secs`, `reduce_time_secs`, `merge_time_secs`, `network_send_time_secs`, `network_recv_time_secs`. The CSV schema in R22 has only a single `duration_ms` column per round, not the phase breakdown.

Also, SPEC-07 R29 defines a different CSV schema: `round,agents,local_interactions,border_interactions,border_redexes,partition_time_ms,compute_time_ms,merge_time_ms,bytes_sent,bytes_received,network_send_time_ms,network_recv_time_ms`. This is significantly more detailed than SPEC-12 R22's schema.

**Impact if unresolved:** The per-round CSV from SPEC-12 R22 does not contain enough data for the experimental analysis required by SPEC-09. If the implementer follows SPEC-12, the CSV will be missing phase breakdowns.
**Suggested resolution:** Align SPEC-12 R22's CSV schema with SPEC-07 R29 (which is more complete). Or explicitly defer per-round CSV to the benchmark suite (SPEC-09) and remove R22 from SPEC-12.

---

### SC-020: `ReductionSummary` struct not defined
**Severity:** LOW
**Axis:** Completeness
**Section:** 2, 3.6, 4.4
**Requirement:** R44-R47
**Problem:** The Definitions table (Section 2) defines "Reduction Summary" as a concept, and R44-R47 define the output format. Section 4.4 defines `NetSummary` as a Rust struct. But there is no `ReductionSummary` struct defined anywhere in SPEC-12. The reduction summary includes timing, MIPS, interaction count, and grid metrics -- none of which are fields on `NetSummary`. The BACKLOG (TASK-0162) references "Define NetSummary and ReductionSummary structs," but the spec only defines the former.

**Impact if unresolved:** The implementer must invent the `ReductionSummary` struct fields.
**Suggested resolution:** Add a `ReductionSummary` struct definition to Section 4.4 with fields: `input_summary: NetSummary`, `output_summary: NetSummary`, `total_interactions: u64`, `duration_secs: f64`, `mips: f64`, `normal_form: bool`, `termination_reason: Option<String>`, and optional grid fields (`rounds: Option<u32>`, `workers: Option<u32>`, `speedup: Option<f64>`, `efficiency: Option<f64>`, `overhead: Option<f64>`).

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 5 |
| MEDIUM | 7 |
| LOW | 6 |

## Mandatory (must fix before implementation)

- **SC-001:** The `io` module does not exist in SPEC-13's module structure -- add it to SPEC-13 R5 or refactor placement
- **SC-002:** Input format (3 formats) contradicts SPEC-07 (bincode-only) and SPEC-13 R42 -- establish supersession
- **SC-003:** `reduce` vs `local` subcommand semantics unresolved -- clarify coexistence
- **SC-004:** IoError type duplicates and conflicts with SPEC-13 error enums -- resolve `#[from]` composition
- **SC-005:** `parse_ic` returns undefined `ParseError` type -- define it or unify with `IoError`
- **SC-006:** Text DSL `root` declaration semantics unspecified -- define multiplicity, absence, and valid types
- **SC-007:** No pseudocode for 6 of 8 non-trivial generators -- add pseudocode or precise topology descriptions

## Recommended (should fix)

- **SC-008:** Generator size `u32` vs SPEC-14 encoding `u64` -- document constraints
- **SC-009:** R13 example comment misleadingly says "cross-wise" -- rewrite comment
- **SC-010:** ERA port array slot semantics for T1 validation -- clarify arity-aware iteration
- **SC-011:** MIPS duration measurement and speedup baseline undefined -- define timing scope
- **SC-012:** Self-wiring and free-to-free wiring edge cases in DSL parser -- add requirements
- **SC-013:** Wire count computation ambiguous (FreePort handling) -- define precisely
- **SC-014:** Test T5 should note stale filtering assumption
- **SC-015:** `ep_annihilation_dup` generator not formally defined -- add R-number

## Optional (may fix)

- **SC-016:** Church numeral generator cross-reference "SPEC-14 R26" does not exist -- fix reference
- **SC-017:** `--format` flag serves dual purpose (input/output) ambiguously -- split or clarify
- **SC-018:** No requirement for graceful handling of size=0 or empty nets -- add boundary tests
- **SC-019:** Per-round CSV schema less detailed than SPEC-07 R29 -- align or defer
- **SC-020:** `ReductionSummary` struct not defined -- add Rust struct definition

---

## Checklist

### Consistency
- [x] Fundamental types (Symbol, AgentId, PortRef, Net, Agent) match SPEC-00/SPEC-02
- [x] Terms from SPEC-00 used without redefinition where appropriate
- [x] FreePort variant consistent with SPEC-02 R4
- [x] Generator names match SPEC-09 benchmark names
- [ ] **FAIL:** `io` module not in SPEC-13 R5 module structure (SC-001)
- [ ] **FAIL:** Three input formats contradict SPEC-07 R22 bincode-only (SC-002)
- [ ] **FAIL:** Three input formats contradict SPEC-13 R42 (SC-002)
- [ ] **FAIL:** `reduce` subcommand vs SPEC-07 `local` subcommand unresolved (SC-003)
- [ ] **FAIL:** IoError conflicts with SPEC-13 R17 RelativistError::Io (SC-004)
- [ ] **FAIL:** parse_ic returns undefined ParseError type (SC-005)
- [ ] **FAIL:** Church generator references "SPEC-14 R26" which does not exist (SC-016)
- [x] Generator signatures align with SPEC-09 Benchmark trait (`make_net(size: u32) -> Net`)
- [x] Redex queue semantics (stale filtering) consistent with SPEC-02 R17 and SPEC-01 I4
- [x] Normal Form definition consistent with SPEC-02 R16 and SPEC-00 Section 5.5
- [x] `reduce` subcommand calls `reduce_all()` consistent with SPEC-03 R13

### Testability
- [x] R4 (binary roundtrip): testable via serialize/deserialize identity
- [x] R11 (parser validation): testable via malformed input test cases
- [x] R15 (text DSL serializer roundtrip): testable via parse(format(net)) == net
- [x] T1-T9 test requirements are concrete with expected values
- [ ] **PARTIAL:** T5 expected values correct but stale filtering assumption not noted (SC-014)
- [x] T6 reduce correctness verifiable
- [x] T7 max-interactions limit verifiable
- [x] T8 format detection verifiable
- [x] T9 generator consistency with benchmark suite verifiable
- [ ] **FAIL:** Wire count computation method unspecified -- not reproducibly testable (SC-013)
- [ ] **FAIL:** MIPS timing scope undefined -- not reproducibly testable (SC-011)
- [ ] **FAIL:** No test for size=0 edge case (SC-018)

### Completeness
- [x] Binary format fully specified (serde + bincode v2, .bin extension, roundtrip identity)
- [x] Text DSL grammar provided in pseudo-BNF
- [x] JSON format specified with MAY deferral and MUST error message
- [x] File format detection table complete (.bin, .ic, .json)
- [x] IoError enum defined with thiserror
- [x] NetSummary struct defined with fields and serialization
- [ ] **FAIL:** ReductionSummary struct not defined (SC-020)
- [ ] **FAIL:** ParseError type not defined (SC-005)
- [ ] **FAIL:** `root` declaration semantics incomplete (SC-006)
- [ ] **FAIL:** No pseudocode for 6 non-trivial generators (SC-007)
- [ ] **FAIL:** `ep_annihilation_dup` not formally specified (SC-015)
- [ ] **FAIL:** Self-wiring and free-to-free wiring edge cases unspecified (SC-012)
- [x] I/O module API (load_net, save_net, parse_ic, format_ic) well-defined
- [x] CLI argument structs provided as Rust code
- [x] Informative examples for text DSL (R13, R14) present
- [x] Rationale section covers major design decisions

### Invariant Preservation
- [x] T1 (port linearity): R11 requires parser validation; R34 requires generator validation
- [x] T5 (rule correctness): generators do not apply rules, so not directly relevant
- [x] I1 (bidirectionality): R4 binary roundtrip preserves; R15 text DSL roundtrip preserves
- [x] I2 (reference validity): R11 requires parser validation
- [x] I3 (monotonic IDs): R10 mandates sequential ID assignment in parser
- [ ] **PARTIAL:** T1 verification for ERA agents unclear (arity-aware iteration needed) (SC-010)
- [x] I4 (redex queue validity): parser correctly inserts redexes only for principal port connections (R9 prevents ERA auxiliary references)
- [x] G1 (fundamental property): generators shared between CLI and benchmarks (R35-R36) ensures consistency
