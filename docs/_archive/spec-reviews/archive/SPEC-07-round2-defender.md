# SPEC-07 -- Round 2: Defender Response

**Date:** 2026-04-05
**Defender:** Especialista em Specs
**Target:** SPEC-07-deployment.md
**Critic review:** SPEC-07-round1-critic.md
**Spec version:** Revised v2 -> Revised v3

---

## Summary

| Response | Count |
|----------|-------|
| ACCEPTED | 13 |
| PARTIALLY ACCEPTED | 3 |
| NOT ADDRESSED | 0 |
| **Total issues** | **16** |

---

## Responses

### SC-001: `--host` flag superseded by `--bind` (SPEC-10 R5, SPEC-13 R44)
**Response:** ACCEPTED
**Action taken:** Complete reconciliation of the bind address across the entire spec:
- R3: `--host <HOST>` and `--port <PORT>` replaced by `--bind <ADDR:PORT>` with default `127.0.0.1:9000`.
- R11: `NodeConfig` fields `host` and `port` replaced by single `bind: SocketAddr`.
- R12: Default `host: 0.0.0.0` replaced by `bind: 127.0.0.1:9000` with reference to SPEC-10 R5.
- Section 4.1: `CoordinatorArgs` struct updated -- `pub host: String` and `pub port: u16` replaced by `pub bind: SocketAddr` with `default_value = "127.0.0.1:9000"`.
- Section 4.2: `build_node_config_coordinator` uses `args.bind` directly. `build_node_config_worker` parses coordinator address into `SocketAddr`.
- Section 4.4: Worker log message updated to use `node_config.bind`.
- Section 4.11.2: Docker Compose `command` updated from `--port "9000"` and `--net` to `--bind "0.0.0.0:9000"` and `--input` (Docker containers must bind to `0.0.0.0` for inter-container communication).
- Section 4.12: Bare-metal example updated to `--bind 0.0.0.0:9000 --input workload.bin`.
- Section 4.13: Sequence diagram updated from `[bind TCP :9000]` to `[bind TCP (--bind)]`.
- Deploy script updated to use `--bind 0.0.0.0:$PORT --input`.
- Supersession note added to R3 explaining the three changes (flag name, default value, semantics).
**Spec sections modified:** R3, R11, R12, Section 4.1, 4.2, 4.3, 4.4, 4.11.2, 4.12, 4.13

### SC-002: Missing security CLI flags (`--token`, `--token-file`, `--insecure`, `--tls-*`)
**Response:** ACCEPTED
**Action taken:** Rather than duplicating the full security flag definitions (which would create a maintenance burden and risk divergence), SPEC-07 now adds supersession notes to R3, R4, and the Design section directing the implementer to SPEC-10 Section 4.5 for the complete security flag set. The `CoordinatorArgs` and `WorkerArgs` structs in Section 4.1 include comments indicating where security flags are to be added by the implementer per SPEC-10. R44 (scope exclusions) removes the "Authentication or encryption" exclusion and replaces it with a note about SPEC-10's three-tier model (see SC-008).
**Spec sections modified:** R3 (supersession note), R4 (supersession note), R44 (exclusion removed), Section 4.1 (comments in structs)

### SC-003: Missing observability CLI flags (`--log-format`, `--metrics-port`)
**Response:** ACCEPTED
**Action taken:** Both flags are now explicitly listed in the relevant requirements:
- `--log-format` added to R3 (coordinator), R4 (worker), and R5 (local), with reference to SPEC-11 R3.
- `--metrics-port` added to R3 (coordinator only, feature-gated on `metrics`), with reference to SPEC-11 R20.
- R12 (defaults) updated with `log_format: TTY-dependent` and `metrics_port: 9090`.
- Section 4.1: All three Args structs (`CoordinatorArgs`, `WorkerArgs`, `LocalArgs`) include `pub log_format: Option<LogFormat>`. `CoordinatorArgs` includes a comment for `--metrics-port` to be added per SPEC-11 R20.
- R35: Added supersession note referencing SPEC-11 as authoritative for all observability concerns.
**Spec sections modified:** R3, R4, R5, R12, R35, Section 4.1

### SC-004: Subcommand count (4 vs 7) -- missing `reduce`, `inspect`, `compute`
**Response:** ACCEPTED
**Action taken:** R1 updated from "four subcommands" to "seven subcommands" with all seven listed explicitly. A note clarifies that the original 4 are defined in SPEC-07 while the additional 3 (`reduce`, `inspect`, `compute`) are defined in SPEC-13 R43 and detailed in SPEC-12 and SPEC-14. The `Command` enum in Section 4.1 now includes all seven variants with doc comments referencing their authoritative specs. The entrypoint (`main`) in Section 4.10 includes dispatch arms for all seven subcommands. R6 updated to reference "all 7 subcommands" in the help output. The Subcommand definition in Section 2 updated to list all seven.
**Spec sections modified:** R1, R6, Section 2 (Definitions), Section 4.1 (Command enum), Section 4.10 (main)

### SC-005: Input format: bincode-only (R22) superseded by 3-format support
**Response:** ACCEPTED
**Action taken:** R22 narrowed to explicitly apply only to `coordinator`, `worker`, and `local` subcommands. A supersession note added below R22 referencing SPEC-12 R1-R50 as authoritative for the `reduce`, `inspect`, and `generate` subcommands (three-format support: `.bin`, `.ic`, `.json`). R24 updated to clarify that the chaining property (output of one run is input of the next) applies to `.bin` format within the `coordinator`/`local` subcommands. Section 2 (Network Input definition) updated to describe the two-path format story. Rationale section 5.4 updated with an addendum acknowledging SPEC-12's multi-format extension.
**Spec sections modified:** R22 (scope narrowed + supersession note), R24 (clarified), Section 2 (Network Input), Section 5.4 (Rationale)

### SC-006: `--net` flag name inconsistent with SPEC-13's `--input`
**Response:** ACCEPTED
**Action taken:** Standardized on `--input` across all subcommands that accept a net file:
- R3 (coordinator): `--net` changed to `--input`.
- R5 (local): `--net` changed to `--input`, with supersession note explaining the change from v2 and noting that SPEC-13 R45a should be updated accordingly.
- R13 (coordinator lifecycle): "file specified by `--net`" changed to "`--input`".
- R14 (error handling): "`--net` file" changed to "`--input` file".
- Section 4.1: All Args structs updated (`pub net: PathBuf` -> `pub input: PathBuf` with short flag `-i`).
- Sections 4.3, 4.5: Pseudocode updated from `args.net` to `args.input`.
- Sequence diagrams: `[load net.bin]` changed to `[load input.bin]`.

Note: The `reduce` subcommand (SPEC-12 R24) already uses `--input`. The `inspect` subcommand (SPEC-12 R28) uses a positional path argument. Consistency is now achieved for all named-flag subcommands.
**Spec sections modified:** R3, R5, R13, R14, Section 4.1, 4.3, 4.5, 4.13, 4.14

### SC-007: Workload generator list diverges from SPEC-09/SPEC-12
**Response:** ACCEPTED
**Action taken:** R32 updated to list only the minimum 5 generators as a subset, with a supersession note directing the implementer to SPEC-12 R33 for the authoritative list of 12 generators via the `ExampleNet` enum. The v2 name "EraChain" renamed to "EpAnnihilation" for consistency with SPEC-09 and SPEC-12. Module location (`io/examples.rs`) referenced from SPEC-12 R36. Section 4.9 (workload generator code) updated with a supersession note and renamed function (`mk_era_chain` -> `mk_ep_annihilation`).
**Spec sections modified:** R32 (supersession note, rename), Section 4.9 (supersession note, code update)

### SC-008: Scope exclusion R44 contradicts SPEC-10 security model
**Response:** ACCEPTED
**Action taken:** The "Authentication or encryption. The environment is considered trusted (OBJETIVO_TCC.md)." line removed from R44. Replaced with a supersession note explaining that SPEC-10 defines a three-tier security model: Tier 1 (development, no auth) requires zero configuration, Tier 2 and Tier 3 are optional. The TCC evaluation uses Tier 1 (trusted environment), preserving the original intent, but the positive assertion that security is "out of scope" is eliminated. The implementer now knows to consult SPEC-10.
**Spec sections modified:** R44

### SC-009: `generate` subcommand argument naming divergence
**Response:** ACCEPTED
**Action taken:** R8 updated to reference SPEC-12 R33 as the authoritative definition for `generate` arguments. A supersession note documents the specific changes from v2: `--workload` -> positional `example` (value_enum), `-s` -> `-n`. The v2 `GenerateArgs` struct in Section 4.1 replaced with a comment referencing SPEC-12 R33. Docker Compose usage examples updated from `--workload tree-sum --size 10000` to `tree-sum -n 10000`.
**Spec sections modified:** R8 (supersession note), Section 4.1 (GenerateArgs replaced with reference), Section 4.11.2 (examples), usage examples in Section 4.12

### SC-010: SPEC-07 logging (R35-R36) lacks SPEC-11's instrumentation requirements
**Response:** ACCEPTED
**Action taken:** A supersession note added below R35 explicitly referencing SPEC-11 as authoritative for all observability concerns, with a summary of what SPEC-11 adds beyond R35-R36: `--log-format` CLI flag (R3), `#[instrument]` on key functions with structured fields (R6), FSM state transition logging at INFO (R7), per-component default log levels (R5), and ERROR-level events for invariant violations (R9a). R35-R36 are preserved as the baseline; the supersession note makes clear that SPEC-11 layers MUST requirements on top.
**Spec sections modified:** R35 (supersession note)

### SC-011: `--net` help text hardcodes `.bin` description
**Response:** ACCEPTED (subsumed by SC-006)
**Action taken:** The `--net` flag no longer exists. The `--input` flag in `CoordinatorArgs` and `LocalArgs` retains the `.bin` description because those subcommands only accept `.bin` (R22). For `reduce` and `inspect`, which accept multiple formats per SPEC-12, the Args definitions live in SPEC-12 where the help text appropriately describes all formats.
**Spec sections modified:** Section 4.1 (covered by SC-006 changes)

### SC-012: `NodeConfig` mapping uses separate host/port vs SPEC-13's bind socket address
**Response:** ACCEPTED (subsumed by SC-001)
**Action taken:** Fully resolved by SC-001. `NodeConfig` now uses `bind: SocketAddr`. The mapping functions in Section 4.2 use `args.bind` directly for the coordinator and `args.coordinator.parse()` for the worker.
**Spec sections modified:** R11, Section 4.2 (covered by SC-001 changes)

### SC-013: Exit code R43 underspecified for new subcommands
**Response:** PARTIALLY ACCEPTED
**Action taken:** R43 updated to note that exit code 2 (communication error) is "Not applicable to `reduce`, `inspect`, `generate`, or `compute` subcommands." Exit code 0 updated to include `max_interactions` (used by `reduce`). Exit code 1 updated to include "encoding error" (used by `compute`). Per-subcommand exit code tables were considered but rejected as over-specification -- the existing 4-code scheme is a superset where some codes simply do not apply to local-only subcommands, which is a natural and understood pattern.
**Spec sections modified:** R43

### SC-014: R19 references `run_coordinator` but SPEC-13 distinguishes `local` from `reduce`
**Response:** ACCEPTED
**Action taken:** R19 updated to reference the full G1 equivalence chain from SPEC-13 R41: `reduce_all(net) ~ run_grid_local(net, n) ~ run_coordinator(net, n)`, where `~` denotes isomorphism of graphs. The `reduce` subcommand (SPEC-13 R41) is now explicitly mentioned as providing the sequential baseline `reduce_all(net)` against which both `local` and `coordinator` results are verified.
**Spec sections modified:** R19

### SC-015: Docker Compose example uses outdated CLI syntax
**Response:** ACCEPTED
**Action taken:** All CLI examples updated throughout the spec:
- Docker Compose YAML: `--port "9000"` -> `--bind "0.0.0.0:9000"`, `--net` -> `--input`.
- Docker Compose usage: `--workload tree-sum --size 10000` -> `tree-sum -n 10000` (positional + SPEC-12 short flag).
- Bare-metal cargo run: `generate --workload tree-sum --size 10000` -> `generate tree-sum -n 10000`.
- Deploy script: `--port $PORT --net` -> `--bind 0.0.0.0:$PORT --input`.
**Spec sections modified:** Sections 4.11.2, 4.12 (examples and deploy script)

### SC-016: R12 default timeouts not verifiable via CLI
**Response:** PARTIALLY ACCEPTED
**Action taken:** The timeout values (120s connect, 60s distribute, 600s collect) are retained as compile-time defaults per SPEC-06 R24/R30. Adding CLI flags for these timeouts was considered but rejected for v1: the number of parameters is already growing with security and observability flags, and timeout tuning is not needed for the TCC evaluation (trusted network, stable environment). However, the concern about integration test speed is valid. The resolution is that integration tests can use the `NodeConfig` struct directly (setting custom timeout values programmatically) without needing CLI exposure. This is already the standard pattern: tests construct configs directly rather than parsing CLI strings. No spec change was made for this item beyond documenting the rationale here.

### Invariant Preservation Recommendation: Post-deserialization validation
**Response:** PARTIALLY ACCEPTED
**Action taken:** The reviewer's recommendation to add post-deserialization invariant validation (T1, I2) was noted. This is a valid concern but belongs in SPEC-08 (Test Strategy) or SPEC-12 (which already specifies input validation for `.ic` files in R11). Adding a SHOULD requirement here would duplicate concerns. The implementer is advised via the existing `AppError::Deserialize` error path that corrupt files produce errors; invariant validation in debug mode is an implementation-level decision that the ENGINEER can make. No spec change was made, but the recommendation is acknowledged.

---

## Changes Made to SPEC-07

### Header
- Status changed from "Revised v2" to "Revised v3"
- Added "Extended by: SPEC-10, SPEC-11, SPEC-12, SPEC-13" line
- Added global supersession note explaining the relationship between SPEC-07 and its successors

### Section 1 (Purpose)
- Updated subcommand list from 4 to 7
- Added mention of successor specs and their areas of extension

### Section 2 (Definitions)
- **Subcommand:** Updated to list all 7 subcommands and reference SPEC-13 R43
- **Network Input:** Updated to describe two-path format story (bincode for grid path, three formats for utility path per SPEC-12)

### Section 3.1 (Single Binary with CLI Subcommands)
- **R1:** Updated from "four subcommands" to "seven subcommands" with full list
- **R3:** `--host`/`--port` replaced by `--bind <ADDR:PORT>` (default `127.0.0.1:9000`); `--net` replaced by `--input`; `--log-format` and `--metrics-port` added; supersession note for security flags
- **R4:** `--log-format` added; supersession note for security flags
- **R5:** `--net` replaced by `--input`; `--log-format` added; supersession note
- **R6:** Updated to reference "all 7 subcommands"

### Section 3.2 (Workload Generator Subcommand)
- **R8:** Changed from SHOULD to MUST; supersession note for SPEC-12 R33

### Section 3.3 (Configuration)
- **R11:** `NodeConfig` fields updated to `bind: SocketAddr`; supersession note
- **R12:** Defaults updated: `bind: 127.0.0.1:9000`, `log_format`, `metrics_port` added; supersession note

### Section 3.4 (Coordinator Lifecycle)
- **R13:** `--net` reference changed to `--input`
- **R14:** `--net` reference changed to `--input`

### Section 3.6 (Local Mode)
- **R19:** Updated to full G1 equivalence chain including `reduce` subcommand

### Section 3.7 (Input Format)
- **R22:** Scope narrowed to `coordinator`/`worker`/`local` only; supersession note for SPEC-12
- **R24:** Clarified chaining applies to `.bin` within grid subcommands

### Section 3.10 (Pre-defined Workload Generators)
- **R32:** Supersession note for SPEC-12 R33 (12 generators); "EraChain" renamed to "EpAnnihilation"

### Section 3.11 (Logging)
- **R35:** Supersession note referencing SPEC-11 as authoritative

### Section 3.14 (Exit Codes)
- **R43:** Exit code 2 marked as not applicable to local-only subcommands; encoding errors added to code 1

### Section 3.15 (Scope Exclusions)
- **R44:** "Authentication or encryption" exclusion removed; supersession note for SPEC-10 three-tier model

### Section 4.1 (CLI Structure)
- `Command` enum: 4 variants -> 7 variants (added `Reduce`, `Inspect`, `Compute` with doc comments)
- `CoordinatorArgs`: `host: String` + `port: u16` -> `bind: SocketAddr`; `net: PathBuf` -> `input: PathBuf`; `log_format` added; comments for security/metrics flags
- `WorkerArgs`: `log_format` added; comments for security flags
- `LocalArgs`: `net: PathBuf` -> `input: PathBuf`; `log_format` added
- `GenerateArgs`: Replaced with comment referencing SPEC-12 R33

### Section 4.2 (CLI-to-Config Mapping)
- `build_node_config_coordinator`: `host`/`port` -> `bind: args.bind`
- `build_node_config_worker`: `parse_host_port` -> `args.coordinator.parse::<SocketAddr>()`

### Section 4.3 (Coordinator Lifecycle)
- `args.net` -> `args.input` throughout

### Section 4.4 (Worker Lifecycle)
- Log message: `node_config.host:node_config.port` -> `node_config.bind`

### Section 4.5 (Local Mode)
- `args.net` -> `args.input` throughout

### Section 4.9 (Workload Generator)
- Added supersession note referencing SPEC-12 R33-R42a
- `mk_era_chain` -> `mk_ep_annihilation`; `"era-chain"` -> `"ep-annihilation"`

### Section 4.10 (Entrypoint)
- Added dispatch arms for `Reduce`, `Inspect`, `Compute` with spec references

### Section 4.11.2 (Docker Compose)
- Coordinator command: `--port "9000"` and `--net` -> `--bind "0.0.0.0:9000"` and `--input`
- Usage examples: `--workload tree-sum --size 10000` -> `tree-sum -n 10000`

### Section 4.12 (Manual Deployment)
- Bare-metal example: `--port 9000 --net` -> `--bind 0.0.0.0:9000 --input`
- Usage example: `generate --workload tree-sum --size 10000` -> `generate tree-sum -n 10000`
- Deploy script: `--port $PORT --net` -> `--bind 0.0.0.0:$PORT --input`

### Section 4.13 (Sequence Diagram: Distributed Mode)
- `[load net.bin]` -> `[load input.bin]`; `[bind TCP :9000]` -> `[bind TCP (--bind)]`

### Section 4.14 (Sequence Diagram: Local Mode)
- `[load net.bin]` -> `[load input.bin]`

### Section 5.4 (Rationale: Bincode Format)
- Added v3 addendum acknowledging SPEC-12's multi-format extension

### Section 6.6 (Deployment Comparison)
- Table updated: "Single binary with subcommands" -> "7 subcommands"; input row updated for two-path format story

### Section 7 (Resolved Questions)
- Question 3 (inspect subcommand): Updated to note elevation to MUST by SPEC-13/SPEC-12

---

## Residual Risks

### SC-016 (PARTIALLY ACCEPTED -- design decision, not a gap)
Timeout values remain compile-time defaults. This is intentional for v1: integration tests construct `NodeConfig` directly with custom timeout values, bypassing the CLI. Adding CLI flags for timeouts would be appropriate for a production release but is over-specification for the TCC scope.

### Cross-spec consistency: SPEC-13 R45a still references `--net`
SPEC-07 R5 now uses `--input`, but SPEC-13 R45a still says "the `local` subcommand MUST accept the arguments defined in SPEC-07 R5: `--workers`, `--net`, `--max-rounds`, `--output`, `--metrics`, `--strategy`." Since SPEC-07 R5 is the authoritative definition for `local` arguments (and SPEC-13 R45a explicitly defers to "the arguments defined in SPEC-07 R5"), the implementer should follow SPEC-07 R5's current `--input` naming. A future SPEC-13 revision should update R45a's inline list to match, but this is not blocking since the cross-reference to R5 is clear.

### No duplication of security/observability flag details
SPEC-07 does not inline the full security (SPEC-10) or observability (SPEC-11) flag definitions. Instead, it uses supersession notes with cross-references. This avoids duplication but requires the implementer to consult multiple specs for the complete flag set. This is an acceptable tradeoff given the alternative risk of divergent definitions across specs.
