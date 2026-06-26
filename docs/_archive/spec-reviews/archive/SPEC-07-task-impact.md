# SPEC-07 Revised v3 -- Task Impact Report

**Date:** 2026-04-05
**Triggered by:** SPEC-07-round2-defender.md (Revised v2 -> Revised v3)
**Tasks affected:** 10 task files updated, 1 backlog header updated

---

## Summary of Spec Changes

SPEC-07 was revised from v2 to v3 incorporating 16 critic issues (13 ACCEPTED, 3 PARTIALLY ACCEPTED). The key changes that impact tasks are:

1. **`--host`/`--port` replaced by `--bind <ADDR:PORT>`** (SC-001): Single `SocketAddr` field, default `127.0.0.1:9000` (SPEC-10 R5)
2. **`--net` replaced by `--input`** across all subcommands (SC-006): Aligns with SPEC-13's naming
3. **Subcommands expanded from 4 to 7** (SC-004): `reduce`, `inspect`, `compute` added (defined by SPEC-12/SPEC-13/SPEC-14)
4. **`--log-format` and `--metrics-port` added** (SC-003): Cross-referenced to SPEC-11
5. **Security flags cross-referenced to SPEC-10** (SC-002): `--token`, `--token-file`, `--insecure` via supersession notes
6. **`GenerateArgs` superseded by SPEC-12 R33** (SC-009): Positional `example` (value_enum) + `-n` replaces `--workload`/`--size`
7. **`EraChain` renamed to `EpAnnihilation`** (SC-007): Aligns with SPEC-09/SPEC-12
8. **`NodeConfig` uses `bind: SocketAddr`** (SC-012): Replaces `host: String` + `port: u16`
9. **Bincode-only scoped to grid subcommands** (SC-005): SPEC-12 handles multi-format for utility subcommands

---

## Tasks Updated

### TASK-0084 (Define NodeConfig and NodeRole types)
**Changes:**
- `host: String` + `port: u16` fields replaced by `bind: SocketAddr`
- Constructor signatures updated: `default_coordinator(bind: SocketAddr, ...)`, `default_worker(coordinator_addr: SocketAddr)`
- Default bind changed from `0.0.0.0` to `127.0.0.1:9000`
- Added `std::net::SocketAddr` import to code sample
- Test expectations updated to verify `bind` field

**Rationale:** SC-001 consolidated two fields into one `SocketAddr`. All downstream tasks (TASK-0102, TASK-0113) that construct `NodeConfig` are updated accordingly.

### TASK-0100 (Refactor CLI to use Args structs)
**Changes:**
- `CoordinatorArgs`: `host: String` + `port: u16` replaced by `bind: SocketAddr` with default `127.0.0.1:9000`; `net: PathBuf` replaced by `input: PathBuf`; `log_format: Option<LogFormat>` added; comments for security and metrics-port flags
- `WorkerArgs`: `log_format: Option<LogFormat>` added; comment for security flags
- `LocalArgs`: `net: PathBuf` replaced by `input: PathBuf`; `log_format: Option<LogFormat>` added
- `GenerateArgs`: Replaced with comment referencing SPEC-12 R33 (positional `example` + `-n`)
- Short aliases updated: `-p` (port) and `-n` (net) removed; `-b` (bind) and `-i` (input) added
- Test expectations updated to use `--bind`, `--input` instead of `--port`, `--net`
- Added `std::net::SocketAddr` import

**Rationale:** SC-001 (bind), SC-006 (--input), SC-003 (--log-format, --metrics-port), SC-002 (security flags), SC-009 (GenerateArgs).

### TASK-0102 (Implement CLI-to-config mapping functions)
**Changes:**
- `build_node_config_coordinator`: `host`/`port` assignment replaced by `bind: args.bind`
- `build_node_config_worker`: `parse_host_port` replaced by `args.coordinator.parse::<SocketAddr>()`
- `parse_host_port` helper removed entirely (no longer needed with `SocketAddr`)
- Defaults section updated: `host=0.0.0.0` replaced by `bind=127.0.0.1:9000`
- Test expectations updated to match new signatures (no more `parse_host_port` tests)
- Context description updated for v3

**Rationale:** SC-001 makes `parse_host_port` obsolete since `SocketAddr` parsing is built into the standard library.

### TASK-0111 (Implement run_local_command)
**Changes:**
- All `args.net` references replaced by `args.input` (5 occurrences)
- Acceptance criterion for missing file updated to reference `--input` flag

**Rationale:** SC-006 (`--net` -> `--input`).

### TASK-0112 (Implement run_coordinator_command)
**Changes:**
- All `args.net` references replaced by `args.input` (2 occurrences in acceptance criteria and code sample)

**Rationale:** SC-006 (`--net` -> `--input`).

### TASK-0113 (Implement run_worker_command)
**Changes:**
- Worker log message updated from `host = %node_config.host, port = node_config.port` to `bind = %node_config.bind`

**Rationale:** SC-001 (NodeConfig uses `bind: SocketAddr`).

### TASK-0114 (Implement run_generate_command)
**Changes:**
- Context updated to describe SPEC-12 R33 supersession of SPEC-07 R8
- "era-chain" renamed to "ep-annihilation" throughout
- `GenerateArgs` code sample updated to use positional `example: ExampleNet` + `-n` instead of `--workload`/`--size`
- Dispatch code updated from `get_workload(&args.workload)` to `args.example.get_workload()`
- `ExampleNet` enum listed with all 12 generators from SPEC-12 R33
- Notes section updated with v3 change summary

**Rationale:** SC-007 (EraChain -> EpAnnihilation), SC-009 (GenerateArgs superseded by SPEC-12 R33).

### TASK-0119 (Integration test: CLI end-to-end)
**Changes:**
- `GenerateArgs` in test updated: `workload: "era-chain"` -> `example: ExampleNet::EpAnnihilation`, `size: 10` -> `n: 10`
- `LocalArgs` in test updated: `net:` -> `input:`, added `log_format: None`

**Rationale:** SC-006 (--input), SC-007 (ep-annihilation), SC-009 (GenerateArgs), SC-003 (log_format field).

### TASK-0129 (Implement network binding security)
**Changes:**
- Acceptance criterion updated: no longer "superseding SPEC-07 R3" since SPEC-07 R3 v3 natively uses `--bind`
- Code comment updated from "Supersedes" to "Consistent with" SPEC-07 R3/R12 Revised v3
- Notes updated: `SocketAddr` parsing replaces string matching for `0.0.0.0` detection
- Supersession language removed since SPEC-07 and SPEC-10 are now aligned

**Rationale:** SC-001 made SPEC-07 natively consistent with SPEC-10/SPEC-13, eliminating the supersession relationship.

### TASK-0138 (Implement SecurityConfig builder from CLI flags)
**Changes:**
- Note updated: `--bind` flag is now "natively defined in SPEC-07 R3 Revised v3" instead of "comes from SPEC-13 R44 (not SPEC-07 `--host`)"

**Rationale:** SC-001 alignment.

---

## Backlog Updates

### BACKLOG.md
- Phase 6 header updated from "SPEC-07 + SPEC-13" to "SPEC-07 Revised v3 + SPEC-13"
- TASK-0100 title in table updated to include "Revised v3"

---

## Tasks NOT Updated (no changes needed)

| Task | Reason |
|------|--------|
| TASK-0101 | References R35/R36 (logging setup) -- unchanged in v3 |
| TASK-0103 | References R43 (exit codes) -- only minor clarification in v3 (exit code 2 scoped to grid subcommands), does not change the `exit_code()` implementation |
| TASK-0104 | References R22/R23/R24 (serialization) -- v3 narrows R22 scope but these helpers remain bincode-only regardless |
| TASK-0105 | References R27-R31 (metrics output) -- unchanged in v3 |
| TASK-0106 | References R15 (print_summary) -- unchanged in v3 |
| TASK-0116 | References R1/R6/R43 (main wiring) -- 7 subcommands already present, exit codes compatible |
| TASK-0117 | Layer boundary enforcement -- unaffected by CLI flag changes |

---

## Residual Cross-Spec Issues

### SPEC-13 R45a still references `--net`
SPEC-07 R5 now uses `--input`, but SPEC-13 R45a's inline argument list still says `--net`. Since R45a explicitly defers to "the arguments defined in SPEC-07 R5", the implementer should follow SPEC-07 R5's current `--input` naming. A future SPEC-13 revision should update R45a's inline list.

### SPEC-06 NodeConfig may also need revision
SPEC-06 Section 4.5 may still define `NodeConfig` with `host: String` + `port: u16`. If SPEC-06 has not been revised to v3 independently, there is a divergence between SPEC-07 v3's expectation of `bind: SocketAddr` and SPEC-06's definition. The implementer should follow SPEC-07 v3's `bind: SocketAddr` as the authoritative structure since it is the newer revision.

---

## Change Statistics

| Metric | Count |
|--------|-------|
| Task files updated | 10 |
| Backlog entries updated | 2 |
| `--host`/`--port` -> `--bind` replacements | 4 tasks (0084, 0100, 0102, 0113) |
| `--net` -> `--input` replacements | 4 tasks (0100, 0111, 0112, 0119) |
| `era-chain` -> `ep-annihilation` replacements | 2 tasks (0114, 0119) |
| `GenerateArgs` restructured | 2 tasks (0100, 0114) |
| `NodeConfig` field changes | 2 tasks (0084, 0102) |
| `log_format` field added | 2 tasks (0100, 0119) |
| Supersession language updated | 2 tasks (0129, 0138) |
