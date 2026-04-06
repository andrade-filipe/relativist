# SPEC-07 -- Round 1: Spec Critic Review

**Date:** 2026-04-05
**Reviewer:** Spec Critic (adversarial)
**Target:** SPEC-07-deployment.md (status: Revised v2)
**Predecessors consulted:** SPEC-00, SPEC-01, SPEC-02, SPEC-03, SPEC-04, SPEC-05, SPEC-06
**Successors consulted:** SPEC-10, SPEC-11, SPEC-12, SPEC-13

---

## Overall Assessment

SPEC-07 was written as a comprehensive deployment and execution spec covering CLI, configuration, lifecycles, I/O formats, workload generators, Docker, and logging. At the time of writing it was self-consistent. However, four successor specs (SPEC-10, SPEC-11, SPEC-12, SPEC-13) have since been written at Revised v2 status and collectively introduced significant contradictions, supersessions, and extensions that SPEC-07 does not reflect. The result is that SPEC-07, if read in isolation by an implementer, would produce a binary with the wrong default bind address, missing CLI flags, missing subcommands, an incomplete input format story, and no security layer. SPEC-07 is now the most stale of the Revised v2 specs and requires a thorough reconciliation pass.

**Verdict:** NEEDS REVISION

---

## Issues

### SC-001: `--host` flag superseded by `--bind` (SPEC-10 R5, SPEC-13 R44)
**Severity:** CRITICAL
**Axis:** Consistency
**Section:** 3.1 (R3), 3.3 (R11, R12), 4.1
**Requirement:** R3, R11, R12
**Problem:** SPEC-07 R3 defines the coordinator flag `--host <HOST>` (default: `0.0.0.0`). SPEC-10 R5 explicitly states: "This requirement supersedes SPEC-07 R3 and R12 for the default bind address; SPEC-07's `--host 0.0.0.0` default is overridden to `--bind 127.0.0.1:9000`." SPEC-10 Section 4.5 confirms: "The CLI flag for bind address is `--bind` (matching SPEC-13 R44), not `--host` (SPEC-07 R3). SPEC-07 R3's `--host` flag is superseded by SPEC-13 R44's `--bind` flag." SPEC-13 R44 uses `--bind` with default `127.0.0.1:9000`.

The contradictions are threefold:
1. **Flag name:** SPEC-07 uses `--host`; SPEC-10/SPEC-13 use `--bind`.
2. **Default value:** SPEC-07 defaults to `0.0.0.0`; SPEC-10/SPEC-13 default to `127.0.0.1`.
3. **Semantics:** SPEC-07's `--host` is address-only (port is separate via `--port`); SPEC-13's `--bind` is a socket address (`HOST:PORT` combined).

Additionally, SPEC-07 R3 defines a separate `--port <PORT>` flag for the coordinator. With `--bind` combining host and port, `--port` becomes redundant. SPEC-07 R12 lists the default `host` as `0.0.0.0`, which is now incorrect.

The Design section (4.1) contains a full Rust struct `CoordinatorArgs` with `pub host: String` and `pub port: u16` fields, which would need to be replaced by a single `pub bind: SocketAddr` field.

**Impact if unresolved:** The implementer will see `--host 0.0.0.0` in SPEC-07 and `--bind 127.0.0.1:9000` in SPEC-10/SPEC-13. If they follow SPEC-07, the coordinator binds to all interfaces by default -- a security issue (PESQ-019 L1). If they follow SPEC-10, the SPEC-07 code examples are wrong.
**Suggested resolution:** Update SPEC-07 R3 to use `--bind <ADDR:PORT>` with default `127.0.0.1:9000`. Remove the separate `--port` flag. Update R11, R12, and the Design section (4.1) to match. Add a supersession note referencing SPEC-10 R5 and SPEC-13 R44.

---

### SC-002: Missing security CLI flags (`--token`, `--token-file`, `--insecure`, `--tls-*`)
**Severity:** CRITICAL
**Axis:** Consistency | Completeness
**Section:** 3.1 (R3, R4), 3.3 (R10), 4.1
**Requirement:** R3, R4, R10
**Problem:** SPEC-10 Section 4.5 defines the following CLI flags not present anywhere in SPEC-07:

**Coordinator flags:**
| Flag | Type | Default | Source |
|------|------|---------|--------|
| `--token` | `Option<String>` | None | SPEC-10 R9 |
| `--token-file` | `Option<PathBuf>` | `./relativist-token` | SPEC-10 R12 |
| `--insecure` | `bool` | `false` | SPEC-10 Section 4.5 |
| `--tls-cert` | `PathBuf` | (required if TLS) | SPEC-10 R25 |
| `--tls-key` | `PathBuf` | (required if TLS) | SPEC-10 R25 |

**Worker flags:**
| Flag | Type | Default | Source |
|------|------|---------|--------|
| `--token` | `Option<String>` | None | SPEC-10 R13 |
| `--tls-ca` | `PathBuf` | (optional) | SPEC-10 R26 |

SPEC-07 R10 says "Relativist's configuration MUST be derived entirely from CLI arguments" and R44 (scope exclusions) explicitly says "Authentication or encryption. The environment is considered trusted." These two positions are now contradicted by SPEC-10, which introduces a full three-tier security model with token auth and optional TLS.

SPEC-07's `WorkerArgs` struct (Section 4.1) contains only `--coordinator <HOST:PORT>`. SPEC-10 and SPEC-13 R45 add `--token` and `--tls-ca`.

**Impact if unresolved:** The implementer following SPEC-07's `CoordinatorArgs` and `WorkerArgs` structs will produce a binary without any authentication or encryption capability. SPEC-10's security model depends on these CLI flags existing.
**Suggested resolution:** Add all security flags to SPEC-07 R3 and R4, or add a supersession note directing the implementer to SPEC-10 Section 4.5 for the complete coordinator and worker flag sets. Update the Design section (4.1) structs. Remove the "Authentication or encryption" exclusion from R44, replacing it with a reference to SPEC-10.

---

### SC-003: Missing observability CLI flags (`--log-format`, `--metrics-port`)
**Severity:** HIGH
**Axis:** Consistency | Completeness
**Section:** 3.1 (R3, R4), 3.11 (R35), 4.1
**Requirement:** R3, R4, R35
**Problem:** SPEC-11 introduces two CLI flags not present in SPEC-07:

| Flag | Type | Default | Applies to | Source |
|------|------|---------|-----------|--------|
| `--log-format` | `text` or `json` | TTY-dependent | Coordinator + Worker | SPEC-11 R3 (MUST) |
| `--metrics-port` | `u16` | `9090` | Coordinator only | SPEC-11 R20 (MUST, feature-gated) |

SPEC-11 OQ-2 explicitly acknowledges: "SPEC-11 introduces `--metrics-port` (R20) and `--log-format` (R3) CLI arguments. These SHOULD be added to SPEC-13 R44/R45 (and SPEC-07 R3) during their next revision cycle."

SPEC-07 R35 specifies `tracing` with `RUST_LOG` via `EnvFilter` for logging, but does not mention the `--log-format` flag for selecting between text and JSON output. SPEC-11 R3 makes `--log-format` a MUST requirement on both `CoordinatorArgs` and `WorkerArgs`.

**Impact if unresolved:** The implementer following SPEC-07 will not add `--log-format` to the CLI, making JSON log output inaccessible except through code changes. The `--metrics-port` flag is needed for the Prometheus HTTP server.
**Suggested resolution:** Add `--log-format` to both `CoordinatorArgs` and `WorkerArgs` in SPEC-07 R3/R4 and the Design section (4.1). Add `--metrics-port` to `CoordinatorArgs` (conditional on `metrics` feature). Reference SPEC-11 R3 and R20.

---

### SC-004: Subcommand count (4 vs 7) -- missing `reduce`, `inspect`, `compute`
**Severity:** HIGH
**Axis:** Consistency | Completeness
**Section:** 3.1 (R1), 4.1
**Requirement:** R1
**Problem:** SPEC-07 R1 defines exactly 4 subcommands: `coordinator`, `worker`, `local`, `generate`. SPEC-13 R43 defines 7 subcommands, explicitly adding 3 new ones:

| SPEC-07 | SPEC-13 | Status |
|---------|---------|--------|
| `coordinator` | `Coordinator` | Match |
| `worker` | `Worker` | Match |
| `local` | `Local` | Match (SPEC-13 preserves `local` per SC-005 resolution) |
| `generate` | `Generate` | Match |
| *(none)* | `Reduce` | New: purely sequential reduction via `reduce_all` (SPEC-13 R41, R46) |
| *(none)* | `Inspect` | New: net file inspection/statistics (SPEC-13 R47) |
| *(none)* | `Compute` | New: arithmetic encode/reduce/decode (SPEC-13 R48a, SPEC-14) |

SPEC-13 R43 explicitly states: "SPEC-13 adds `reduce`, `inspect`, and `compute` to the original 4 subcommands from SPEC-07 R1 (`coordinator`, `worker`, `local`, `generate`), without removing any."

SPEC-12 further specifies `reduce` (R23-R26), `inspect` (R27-R31), and `generate` (R32-R48) with detailed argument structures.

**Impact if unresolved:** SPEC-07 R1 says the CLI has "four subcommands" and R6 says "if no subcommand is provided" with exactly four listed. An implementer following SPEC-07 alone will miss three subcommands. The `reduce` subcommand is particularly important as the sequential baseline for G1 verification.
**Suggested resolution:** Update SPEC-07 R1 to list all 7 subcommands, or add a cross-reference note stating that SPEC-13 R43 extends R1. Add brief descriptions of `reduce`, `inspect`, and `compute` with references to their defining specs (SPEC-12, SPEC-13, SPEC-14).

---

### SC-005: Input format: bincode-only (R22) superseded by 3-format support
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.7 (R22-R24), 4.1
**Requirement:** R22, R23, R24
**Problem:** SPEC-07 R22 mandates: "The input format for IC networks MUST be a binary file containing the `Net` serialized with serde + bincode." SPEC-12 explicitly supersedes this for the `reduce`, `inspect`, and `generate` subcommands:

> "SPEC-12 R1-R50 supersede SPEC-07 R22-R25 and SPEC-13 R42 for all file format specifications."

SPEC-12 R1 requires three input formats: Binary (.bin), Text DSL (.ic), and JSON (.json). SPEC-12 introduces:
- A text DSL grammar with `.ic` extension (R6-R15)
- JSON format via serde JSON (R16-R17)
- Format auto-detection by file extension (R1)
- `--input-format` override flag (R24)

SPEC-12 notes that `coordinator`, `worker`, and `local` subcommands still accept only `.bin` for performance, which is consistent with SPEC-07 R22 for those subcommands. But for `reduce`, `inspect`, and `generate`, the bincode-only restriction is lifted.

SPEC-07 R24 states: "The input format MUST be the same format produced by the `generate` subcommand." This is no longer strictly true if `generate` can output `.ic` or `.json` (SPEC-12 R33 allows format inference from the output file extension).

**Impact if unresolved:** An implementer following SPEC-07 alone will hard-code bincode for all subcommands, making the `reduce` and `inspect` subcommands unusable with the text DSL (.ic files) that SPEC-12 defines for documentation and debugging.
**Suggested resolution:** Add a supersession note to SPEC-07 R22-R25 referencing SPEC-12 R1-R50. Clarify that R22's bincode-only requirement applies to `coordinator`, `worker`, and `local` subcommands, while `reduce`, `inspect`, and `generate` accept all three formats per SPEC-12.

---

### SC-006: `--net` flag name inconsistent with SPEC-13's `--input`
**Severity:** HIGH
**Axis:** Consistency
**Section:** 3.1 (R3, R5), 4.1
**Requirement:** R3, R5
**Problem:** SPEC-07 uses `--net <PATH>` for the input network file in both the coordinator (R3) and local (R5) subcommands. SPEC-13 R44 uses `--input` for the coordinator: "The `coordinator` subcommand MUST accept at minimum: [...] `--input` (path to net file)." SPEC-12 R24 also uses `--input` for the `reduce` subcommand. SPEC-10 R2 uses `--input`: "A plain `relativist coordinator --workers 2 --input net.bin`..."

However, SPEC-13 R45a preserves `--net` for the `local` subcommand: "The `local` subcommand MUST accept the arguments defined in SPEC-07 R5: `--workers`, `--net`, `--max-rounds`, `--output`, `--metrics`, `--strategy`."

This creates an inconsistency: the coordinator uses `--input` but the `local` subcommand (which runs the same grid loop) uses `--net`. Users switching between `coordinator` and `local` modes would need to remember different flag names for the same concept.

**Impact if unresolved:** Confusing CLI ergonomics. The implementer must decide between the names and may accidentally break one spec's tests while satisfying another.
**Suggested resolution:** Standardize on `--input` for all subcommands that accept a net file (`coordinator`, `local`, `reduce`). Update SPEC-07 R3 and R5 to use `--input`. Add `--net` as a deprecated alias for backward compatibility if desired. Update SPEC-13 R45a accordingly.

---

### SC-007: Workload generator list diverges from SPEC-09/SPEC-12
**Severity:** MEDIUM
**Axis:** Consistency | Completeness
**Section:** 3.10 (R32)
**Requirement:** R32
**Problem:** SPEC-07 R32 defines 5 workload generators: `TreeSum`, `TreeSumBalanced`, `EraChain`, `ConDupExpansion`, `DualTree`. SPEC-12 R33 defines 12 generators via the `ExampleNet` enum:

| SPEC-07 Name | SPEC-12 Equivalent | Status |
|-------------|-------------------|--------|
| TreeSum | `TreeSum` | Match |
| TreeSumBalanced | `TreeSumBalanced` | Match |
| EraChain | `EpAnnihilation` | Renamed (ERA-ERA pairs is what EraChain was) |
| ConDupExpansion | `ConDupExpansion` | Match |
| DualTree | `DualTree` | Match |
| *(none)* | `EpAnnihilationCon` | New: N CON-CON pairs (Profile A) |
| *(none)* | `EpAnnihilationDup` | New: N DUP-DUP pairs (Profile A) |
| *(none)* | `MixedRules` | New: N pairs of each rule type |
| *(none)* | `ErasurePropagation` | New: ERA propagation chain (Profile C) |
| *(none)* | `ChurchNat` | New: Church numeral encoding (SPEC-14) |
| *(none)* | `ChurchAdd` | New: Church addition (SPEC-14) |
| *(none)* | `ChurchMul` | New: Church multiplication (SPEC-14) |

SPEC-12 R36 mandates generators be in `io/examples.rs`, not duplicated. SPEC-07 R32 does not specify a module location.

**Impact if unresolved:** An implementer following SPEC-07 will implement only 5 generators. The SPEC-09 benchmark suite and SPEC-12 `generate` subcommand require all 12. Missing generators means incomplete benchmark coverage.
**Suggested resolution:** Update SPEC-07 R32 to reference SPEC-12 R33 for the complete generator list. Remove the inline generator list from SPEC-07 or mark it as a subset with a cross-reference.

---

### SC-008: Scope exclusion R44 contradicts SPEC-10 security model
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.15 (R44)
**Requirement:** R44
**Problem:** SPEC-07 R44 (Scope Exclusions) explicitly states: "Authentication or encryption. The environment is considered trusted (OBJETIVO_TCC.md)." This is directly contradicted by SPEC-10, which defines a complete three-tier security model including token authentication (R9-R18), optional TLS 1.3 encryption (R20-R28), and connection limits (R31-R33).

The contradiction is not merely an omission -- it is a positive assertion that security is out of scope, which an implementer might rely on to skip the entire SPEC-10.

**Impact if unresolved:** An implementer reading SPEC-07 R44 may skip SPEC-10 entirely, believing security is explicitly excluded. SPEC-10 is a Revised v2 spec with MUST requirements.
**Suggested resolution:** Remove the "Authentication or encryption" line from R44. Replace with: "Authentication and encryption are defined in SPEC-10 (three-tier security model). Tier 1 (development) requires no security configuration." Optionally move the remaining exclusions to a cross-spec exclusions document.

---

### SC-009: `generate` subcommand argument naming divergence
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.2 (R8), 4.1
**Requirement:** R8
**Problem:** SPEC-07 R8 defines the `generate` subcommand arguments as:
```
relativist generate --workload <NAME> --size <N> --output <PATH>
```
With the `GenerateArgs` struct using `--workload` (short: `-W`), `--size` (short: `-s`), and `--output` (short: `-o`).

SPEC-12 R33 defines different argument names:
```rust
pub struct GenerateArgs {
    #[arg(value_enum)]
    pub example: ExampleNet,   // positional, not --workload
    #[arg(long, short = 'n')]
    pub size: u32,             // short -n, not -s
    #[arg(long, short = 'o')]
    pub output: PathBuf,       // matches
}
```

The key differences:
1. SPEC-07 uses `--workload <NAME>` (named flag); SPEC-12 uses a positional `example` argument with `value_enum`.
2. SPEC-07 short flag for size is `-s`; SPEC-12 uses `-n`.
3. SPEC-07 calls them "workloads"; SPEC-12 calls them "examples" (`ExampleNet` enum).

**Impact if unresolved:** CLI ergonomics disagreement. The implementer must choose one interface. Users consulting SPEC-07 documentation will use the wrong syntax.
**Suggested resolution:** Defer to SPEC-12 R33 as the authoritative definition for `generate` arguments, since SPEC-12 is the User I/O spec and its definition is more detailed. Update SPEC-07 R8 to reference SPEC-12 R33 or update the argument names to match.

---

### SC-010: SPEC-07 logging (R35-R36) lacks SPEC-11's instrumentation requirements
**Severity:** MEDIUM
**Axis:** Completeness
**Section:** 3.11 (R35, R36)
**Requirement:** R35, R36
**Problem:** SPEC-07 R35-R36 define basic logging: use `tracing`, configure via `RUST_LOG`, and follow standard log levels. SPEC-11 significantly extends this with MUST requirements that SPEC-07 does not mention:

| SPEC-11 Requirement | Description | In SPEC-07? |
|---------------------|-------------|-------------|
| R1 | `tracing` as sole API; no `println!`, `eprintln!`, `dbg!`, `log` | No |
| R3 | `--log-format` CLI flag (text/json) | No |
| R5 | Default per-component log levels | No |
| R6 | `#[instrument]` on key functions with structured fields | No |
| R7 | FSM state transition logging at INFO | No |
| R9 | Target, thread ID, timestamp required in output | No |
| R9a | ERROR-level events for invariant violations, protocol errors, FSM errors | No |

SPEC-07 R35 says "MUST use the `tracing` crate" which is consistent with SPEC-11 R1, but SPEC-07 R36 is merely a SHOULD recommendation on log levels, while SPEC-11 makes many of these MUST requirements.

**Impact if unresolved:** An implementer following only SPEC-07 will produce a logging system that lacks structured spans, FSM transition logging, per-component default levels, and the `--log-format` flag.
**Suggested resolution:** Add a cross-reference from SPEC-07 R35-R36 to SPEC-11 as the authoritative observability spec: "SPEC-11 supersedes and extends these requirements with detailed instrumentation, log format selection, and metrics."

---

### SC-011: Coordinator `--net` flag uses `--net <PATH>` but SPEC-12 R2 uses `--input <PATH>` with format detection
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.1 (R3), 3.7 (R22)
**Requirement:** R3, R22
**Problem:** SPEC-07 R3 specifies `--net <PATH>` for the coordinator and describes it as "path to the input network file (`.bin`)". SPEC-12 introduces format auto-detection by file extension. While SPEC-12 limits `coordinator` and `local` to `.bin` only, the naming convention of `--net` vs `--input` is inconsistent across subcommands. More critically, SPEC-07's Design section (4.1) hardcodes the description as ".bin, bincode-serialized Net" in the `#[arg]` help text, which would be misleading if format detection is ever extended to the coordinator.

**Impact if unresolved:** Minor naming inconsistency that creates friction when the user moves between subcommands.
**Suggested resolution:** Covered by SC-006. Standardize on `--input` across all subcommands.

---

### SC-012: `NodeConfig` mapping (R11) references `host` and `port` separately, but SPEC-13 uses `bind: SocketAddr`
**Severity:** MEDIUM
**Axis:** Consistency
**Section:** 3.3 (R11)
**Requirement:** R11
**Problem:** SPEC-07 R11 states CLI arguments MUST be mapped to `NodeConfig` which includes `host` and `port` as separate fields (from SPEC-06 Section 4.5). SPEC-13 R44 uses `--bind` as a single socket address (`127.0.0.1:9000`). The mapping in SPEC-07 Section 4.2 (`build_node_config_coordinator`) constructs `NodeConfig` with separate `host: args.host.clone()` and `port: args.port` fields, which is inconsistent with a `--bind <ADDR:PORT>` flag that provides a combined socket address.

**Impact if unresolved:** The NodeConfig struct definition and the CLI-to-config mapping code in SPEC-07 Section 4.2 will not compile against the SPEC-13 CLI design.
**Suggested resolution:** Update `NodeConfig` to use a single `bind: SocketAddr` field, or update the mapping to parse `--bind` into separate host and port fields.

---

### SC-013: Exit code R43 is underspecified for new subcommands
**Severity:** LOW
**Axis:** Completeness
**Section:** 3.14 (R43)
**Requirement:** R43
**Problem:** SPEC-07 R43 defines exit codes 0-3 with generic descriptions. The new subcommands (`reduce`, `inspect`, `compute`) may have different error conditions. For example, `inspect` has no communication errors (exit code 2 is irrelevant), and `compute` may have encoding errors (not covered by any exit code). The exit codes are a SHOULD requirement, so this is not critical, but completeness suffers.

**Impact if unresolved:** The implementer must invent exit code semantics for new subcommands.
**Suggested resolution:** Generalize exit code descriptions or add per-subcommand exit code notes. Alternatively, keep the existing codes as a superset and note that not all codes apply to all subcommands.

---

### SC-014: R19 (local mode equivalence) references `run_coordinator` but SPEC-13 distinguishes `local` from `reduce`
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.6 (R19)
**Requirement:** R19
**Problem:** SPEC-07 R19 states: "Local mode MUST produce results identical to distributed mode for the same network and number of workers. Formally: `run_grid(net, n) == extract_result(run_coordinator(net, n))`." SPEC-13 clarifies that `local` runs the full grid loop in-process (R41a), while `reduce` calls `reduce_all` directly (R41). The Fundamental Property G1 (SPEC-01) is:

```
reduce_all(net) ~ extract_result(run_grid(net, n))
```

SPEC-07 R19's formulation is essentially correct for the `local` subcommand, but it does not mention the `reduce` subcommand's relationship. SPEC-13 R41 makes the three-way equivalence explicit: `reduce_all(net) ~ run_grid_local(net, n) ~ run_coordinator(net, n)`.

**Impact if unresolved:** Minor conceptual gap. The implementer may not realize that `reduce` must also be equivalent.
**Suggested resolution:** Update R19 to reference the full G1 equivalence chain including the `reduce` subcommand.

---

### SC-015: Docker Compose example uses outdated CLI syntax
**Severity:** LOW
**Axis:** Consistency
**Section:** 3.12 (R40)
**Requirement:** R40
**Problem:** SPEC-07 R40 shows Docker Compose usage examples:
```bash
docker compose run coordinator relativist generate --workload tree-sum --size 10000 --output /data/net.bin
```
This uses `--workload` (SPEC-07 naming) rather than the positional `example` argument defined in SPEC-12 R33. The Docker Compose YAML itself would also need to reference `--bind` instead of `--host`/`--port`.

**Impact if unresolved:** Docker examples in documentation will use outdated CLI syntax that does not match the implemented binary.
**Suggested resolution:** Update examples to use SPEC-12/SPEC-13 CLI syntax.

---

### SC-016: R12 default timeouts not verifiable via CLI
**Severity:** LOW
**Axis:** Testability
**Section:** 3.3 (R12)
**Requirement:** R12
**Problem:** SPEC-07 R12 specifies sensible defaults for `worker_connect_timeout` (120s), `distribute_timeout` (60s), and `collect_timeout` (600s). These defaults are hardcoded and cannot be overridden via CLI (SPEC-07 defines no flags for them). SPEC-10 and SPEC-13 also do not introduce CLI flags for these timeouts. While the defaults are reasonable for the TCC scope, they are not testable by end users -- an integration test that wants to verify timeout behavior must wait the full default duration or modify source code.

**Impact if unresolved:** No practical issue for the TCC. Integration tests for timeout behavior will be slow.
**Suggested resolution:** Consider adding `--connect-timeout`, `--distribute-timeout`, and `--collect-timeout` flags to the coordinator, or document that these are configurable only at compile time in v1.

---

## Invariant Preservation Analysis

### I1 (Bidirectional Consistency)
No direct impact from SPEC-07. CLI/config does not touch port arrays.

### I2 (Reference Validity)
SPEC-07 R22's bincode format must preserve reference validity. SPEC-12's text DSL parser (R11) adds a validation step for T1 and I2. SPEC-07 does not specify input validation for `.bin` files -- if a corrupt file is loaded, I2 may be violated. SPEC-07 R14 covers the error case ("cannot be deserialized") but does not mandate post-deserialization invariant validation.

**Recommendation:** Add a SHOULD requirement: "After deserializing a net from a `.bin` file, the coordinator/local mode SHOULD validate invariants T1 and I2 in debug mode."

### I3 (Monotonicity of AgentIds)
No direct impact from SPEC-07. The `generate` subcommand generators (R32-R34) must produce nets with valid `next_id`. SPEC-07 R33 ("deterministic network") is consistent.

### I4 (Redex Queue Validity)
No direct impact. Serialized nets include the redex queue; after deserialization, stale entries may exist. SPEC-07 does not mention this.

### I5 (Termination)
SPEC-07 R19 (local mode equivalence) and R13 (coordinator lifecycle) both reference Normal Form as the termination condition, consistent with I5. SPEC-07 R5's `--max-rounds` provides the safety valve for non-terminating nets.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 2 |
| HIGH | 4 |
| MEDIUM | 5 |
| LOW | 5 |

## Mandatory (must fix before implementation)

- **SC-001:** `--host` superseded by `--bind` with different default -- reconcile flag name, default value, and semantics with SPEC-10/SPEC-13
- **SC-002:** Missing security CLI flags (`--token`, `--token-file`, `--insecure`, `--tls-*`) -- add or cross-reference SPEC-10
- **SC-003:** Missing observability CLI flags (`--log-format`, `--metrics-port`) -- add or cross-reference SPEC-11
- **SC-004:** Subcommand count (4 vs 7) -- add `reduce`, `inspect`, `compute` or reference SPEC-13 R43
- **SC-005:** Input format bincode-only superseded by 3-format support -- add supersession note referencing SPEC-12
- **SC-006:** `--net` vs `--input` flag name inconsistency -- standardize across subcommands

## Recommended (should fix)

- **SC-007:** Workload generator list diverges from SPEC-12 (5 vs 12)
- **SC-008:** Scope exclusion R44 contradicts SPEC-10 security model
- **SC-009:** `generate` argument naming divergence with SPEC-12
- **SC-010:** Logging requirements (R35-R36) lack SPEC-11 instrumentation detail
- **SC-011:** `--net` help text hardcodes `.bin` description
- **SC-012:** NodeConfig mapping uses separate host/port vs SPEC-13's bind socket address

---

## Checklist

### Consistency
- [ ] **FAIL:** `--host 0.0.0.0` default contradicts SPEC-10 R5 / SPEC-13 R44 `--bind 127.0.0.1:9000` (SC-001)
- [ ] **FAIL:** Missing `--token`, `--token-file`, `--insecure`, `--tls-*` flags from SPEC-10 (SC-002)
- [ ] **FAIL:** Missing `--log-format`, `--metrics-port` flags from SPEC-11 (SC-003)
- [ ] **FAIL:** 4 subcommands vs 7 in SPEC-13 R43 (SC-004)
- [ ] **FAIL:** Bincode-only input (R22) superseded by SPEC-12 3-format support (SC-005)
- [ ] **FAIL:** `--net` flag name vs `--input` in SPEC-13/SPEC-12 (SC-006)
- [ ] **FAIL:** 5 generators vs 12 in SPEC-12 R33 (SC-007)
- [ ] **FAIL:** R44 "no authentication" contradicts SPEC-10 (SC-008)
- [ ] **FAIL:** `--workload` vs positional `example` argument in SPEC-12 (SC-009)
- [ ] **FAIL:** NodeConfig host/port vs bind SocketAddr (SC-012)
- [x] Coordinator lifecycle (R13) consistent with SPEC-06 Section 4.6
- [x] Worker lifecycle (R16) consistent with SPEC-06 Section 4.7
- [x] Exit codes (R43) consistent with error categories
- [x] Docker deployment (R37-R40) consistent with binary design

### Testability
- [x] R1 (single binary): trivially verifiable
- [x] R6 (no subcommand -> help + exit 1): testable
- [x] R14 (invalid file -> exit 1): testable
- [x] R18 (local mode no TCP): testable by port scanning
- [x] R19 (local == distributed): testable via G1 verification
- [x] R33 (deterministic generators): testable by double invocation
- [ ] **PARTIAL:** R12 (default timeouts): not overridable via CLI, hard to test (SC-016)
- [ ] **PARTIAL:** R39 (Docker == bare-metal): testable but requires Docker environment

### Completeness
- [ ] **FAIL:** CLI args incomplete -- missing security, observability, and new subcommand args
- [x] Coordinator lifecycle fully specified (R13, steps 1-6)
- [x] Worker lifecycle fully specified (R16, steps 1-4)
- [x] Local mode fully specified (R18-R21)
- [ ] **FAIL:** Workload generator list incomplete vs SPEC-12 (SC-007)
- [x] Output formats (R25-R31) fully specified
- [x] Docker deployment (R37-R40) sufficiently specified

### Invariant Preservation
- [x] R19 preserves G1 (Fundamental Property)
- [x] R22-R24 round-trip format supports D1 (split/merge identity) testing
- [ ] **PARTIAL:** No post-deserialization invariant validation (T1, I2) recommended
