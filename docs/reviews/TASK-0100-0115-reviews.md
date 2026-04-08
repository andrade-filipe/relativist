# TASK-0100 to TASK-0115: Combined Review (Phase 6 -- CLI & Config / Deployment)

**Spec:** SPEC-07 (Deployment and Execution), with extensions from SPEC-10, SPEC-11, SPEC-12, SPEC-13, SPEC-14
**Files reviewed:** `src/main.rs`, `src/commands.rs`, `src/config.rs`, `src/coordinator.rs`, `src/worker.rs`, `src/error.rs`
**Date:** 2026-04-08
**Reviewer:** Claude (automated pipeline)

---

## Stage 4: Code Cleaner

### MF-01: Duplicate `LogFormat` enum (config.rs vs observability/types.rs)
**File:** `src/config.rs` lines 64-70 vs `src/observability/types.rs` lines 7-12
**Issue:** Two independent `LogFormat` enums exist: `config::LogFormat` (with `ValueEnum` for clap) and `observability::types::LogFormat` (for tracing init). They are never bridged -- `--log-format` is parsed into `config::LogFormat` but `init_tracing()` always uses `ObservabilityConfig::default()` which hardcodes `observability::LogFormat::Text`. This means the CLI flag is dead code.
**Classification:** MF -- the `--log-format` flag is advertised but does nothing.
**Fix:** Wire `--log-format` from CLI args through to `ObservabilityConfig` in `main.rs`, converting `config::LogFormat` to `observability::LogFormat`.

### MF-02: Missing `Default` impl for `WorkerContext` (clippy)
**File:** `src/worker.rs` line 79
**Issue:** `WorkerContext::new()` exists without a corresponding `Default` impl. Clippy error: `new_without_default`.
**Classification:** MF -- blocks `cargo clippy -- -D warnings`.
**Fix:** Add `impl Default for WorkerContext`.

### MF-03: Large enum variant in `WorkerAction` (clippy)
**File:** `src/worker.rs` line 52
**Issue:** `WorkerAction::SendMessage(Message)` is 264+ bytes while other variants are tiny. Clippy error: `large_enum_variant`.
**Classification:** MF -- blocks `cargo clippy -- -D warnings`.
**Fix:** Box the `Message` payload: `SendMessage(Box<Message>)`.

### SF-01: `run_compute_command` is 90+ lines
**File:** `src/commands.rs` lines 121-215
**Issue:** This function handles encoding, reduction (two branches: local vs distributed), decoding, MIPS calculation, output, and metrics. It violates Single Responsibility and is the longest function in the file.
**Classification:** SF -- functional but hard to maintain. Could be split into `reduce_locally` and `reduce_distributed` helpers.
**Action:** Deferred -- splitting would be cosmetic and risks breaking the CLI interface flow.

### SF-02: Magic number 600 for timer duration
**File:** `src/coordinator.rs` line 229
**Issue:** `Duration::from_secs(600)` is hardcoded in the FSM transition. Should reference the config's `collect_timeout`.
**Classification:** SF -- works but couples the FSM to a magic number instead of config.
**Action:** Noted for future refactor; currently the FSM doesn't carry the config's timeout value in context.

### NTH-01: `RelError` type alias
**File:** `src/error.rs` line 146
**Issue:** `pub type RelError = RelativistError;` exists for "backwards-compatibility" but should be phased out.
**Classification:** NTH -- harmless alias, can be removed when convenient.

### NTH-02: Inline `crate::` paths in `run_inspect_command`
**File:** `src/commands.rs` lines 60-68
**Issue:** Uses `crate::io::count_agents_by_symbol` and `crate::net::Symbol::*` inline instead of imports at the top. Inconsistent with the rest of the file.
**Classification:** NTH -- readability only.

---

## Stage 5: Architecture Review

### Module Boundaries
- **config.rs**: Pure CLI types + mapping functions. No business logic. Clean.
- **commands.rs**: Thin dispatch layer. Each `run_*_command` follows the pattern: build config -> load net -> process -> output. Clean.
- **coordinator.rs / worker.rs**: Pure FSM with stimulus-response pattern. No I/O. Testable. Excellent separation.
- **error.rs**: Centralized error hierarchy with `thiserror`. Clean `exit_code()` mapping.
- **main.rs**: Minimal -- parse CLI, init tracing, dispatch, handle error. Clean (except MF-01 dead log-format).

### Dependency Direction
All dependencies flow inward (main -> commands -> {config, io, merge, reduction, encoding}). No circular deps. The FSMs in coordinator.rs and worker.rs depend on net/partition/protocol types but NOT on I/O or runtime.

### Spec Compliance Matrix (SPEC-07 MUST Requirements)

| Req | Description | Status | Notes |
|-----|-------------|--------|-------|
| R1  | Single binary, 7 subcommands | PASS | All 7 implemented in `Command` enum |
| R2  | clap with derive macros | PASS | `#[derive(Parser)]`, `#[derive(Subcommand)]` |
| R3  | coordinator args | PASS | All flags present including SPEC-10 security flags |
| R4  | worker args | PASS | `--coordinator`, `--log-format`, SPEC-10 flags |
| R5  | local args | PASS | All flags present |
| R6  | No subcommand shows help, exit 1 | PARTIAL | clap shows help on missing subcommand but exit code is 2 (clap default), not 1 |
| R7  | Help text on all args | PASS | All args have doc comments (clap derives help from them) |
| R8  | generate subcommand | PASS (stub) | Returns Config error "not implemented yet (Phase 9)" |
| R9  | generate uses bincode | N/A | Stub only |
| R10 | Config from CLI only | PASS | No config files anywhere |
| R11 | CLI -> GridConfig, NodeConfig | PASS | `build_grid_config`, `build_node_config_*` functions |
| R12 | Sensible defaults | PASS | bind=127.0.0.1:9000, max_rounds=None, strategy=round-robin |
| R13 | Coordinator lifecycle | PARTIAL | Placeholder -- validates config and loads net, returns error |
| R14 | Input file error -> stderr, exit 1 | PASS | `load_net_from_file` returns IO/deserialization errors -> exit 1 |
| R15 | Print summary | PASS | `print_summary()` called for local and compute modes |
| R16 | Worker lifecycle | PARTIAL | Placeholder -- validates config, returns error |
| R17 | Worker connection failure -> exit 1 | N/A | Worker is placeholder |
| R18 | Local mode uses run_grid, no TCP | PASS | `run_grid()` called directly, no TCP imports |
| R19 | Local == distributed results | PASS (by design) | Same `run_grid` function used |
| R22 | Input format bincode .bin | PASS | Via `load_net_from_file` |
| R23 | Self-contained .bin | PASS | Full `Net` serialized |
| R24 | Output format == input format | PASS | Same bincode via `save_net_to_file` |
| R25 | --output writes bincode | PASS | Implemented in all commands that accept --output |
| R26 | No --output means no file | PASS | Conditional on `args.output.is_some()` |
| R27 | --metrics writes metrics | PASS | `write_metrics` called conditionally |
| R35 | tracing crate with RUST_LOG | PASS | `init_tracing` uses EnvFilter |
| R41 | Bare-metal deployment | PASS (by design) | Single binary, no runtime deps |
| R43 | Exit codes 0/1/2/3 | PASS | `exit_code()` in error.rs |

### Compliance Gaps (MUST):
1. **R6 (exit code on no subcommand):** clap exits with code 2, not 1. This is a clap behavior that would require overriding `Error::exit()`. Minor spec deviation.
2. **R13/R16 (coordinator/worker lifecycle):** Placeholders returning errors. Expected -- async runtime wiring is Phase 7+.

---

## Stage 6: QA Bug Hunt

### BUG-01 (MF): `workers=0` causes panic via `run_grid` assertion
**File:** `src/commands.rs` -> `src/merge/grid.rs:26`
**Path:** `relativist local --workers 0 --input x.bin` or `relativist compute add 3 5 --workers 0`
**Impact:** The CLI accepts `--workers 0` (u32 allows it), passes it to `run_grid()`, which calls `assert!(config.num_workers >= 1)` and panics. User gets an ugly stack trace instead of a friendly error message.
**Classification:** MF -- user-facing panic.
**Fix:** Validate `workers >= 1` at the CLI layer in `config.rs` using `value_parser!(clap::value_parser!(u32).range(1..))` or validate in the `build_grid_config*` functions, returning `RelativistError::Config`.

### BUG-02 (MF): `--log-format` is dead code (never wired to tracing init)
**File:** `src/main.rs` line 13 + `src/config.rs` lines 64-70 + all `log_format` fields
**Path:** `relativist local --workers 2 --input x.bin --log-format json` -- the flag is parsed and stored but `init_tracing` is called with `ObservabilityConfig::default()` before CLI parsing (line 13 vs line 14).
**Impact:** User expects JSON log output, gets text. The flag is silently ignored.
**Classification:** MF -- advertised feature that does nothing.
**Fix:** Move `init_tracing` after CLI parsing, extract `log_format` from the matched subcommand, and convert `config::LogFormat` -> `observability::LogFormat`.

### BUG-03 (SF): `LogTransition` does not log `Done` or `Error` in some paths
**File:** `src/coordinator.rs`, `src/worker.rs`
**Path:** In coordinator, when transitioning `CheckTermination -> Done`, the `LogTransition` action uses `from: CheckTermination, to: Done` which is correct. But the `WaitingForWorkers` self-transition when a worker connects but minimum not met uses `from: from.clone(), to: from` -- both from and to are the same state. This is semantically questionable (logging a "transition" to the same state) but not a bug.
**Classification:** SF -- the self-transition log is noisy but technically correct as a "progress" log.

### BUG-04 (SF): `WorkerRoundStats` fields filled with zeros in worker FSM
**File:** `src/worker.rs` lines 120-126
**Issue:** `agents_before: 0`, `local_redexes: 0`, `reduce_duration_secs: 0.0`, `interactions_by_rule: [0; 6]` are commented "filled by the runtime" but the FSM creates the Message with zeros. The async runtime must patch these before actually sending.
**Classification:** SF -- by design (the FSM is pure, runtime fills actual stats), but fragile. If the runtime forgets, zeros propagate to metrics.

### BUG-05 (NTH): No validation of `CoordinatorArgs.workers >= 1`
**File:** `src/config.rs`
**Issue:** Same as BUG-01 but for coordinator mode. Currently the coordinator is a placeholder that doesn't call `run_grid`, but when it's wired up, `workers=0` would panic there too.
**Classification:** NTH -- coordinator is a stub, but should be fixed proactively.

### BUG-06 (NTH): `compute --workers 0` causes panic
**File:** `src/commands.rs` line 149-156
**Path:** `relativist compute add 3 5 --workers 0` hits `run_grid` with `num_workers: 0`.
**Classification:** Same root cause as BUG-01. Fix in config validation.

---

## Summary of Actions

| ID | Severity | File(s) | Action |
|----|----------|---------|--------|
| MF-01 / BUG-02 | MF | main.rs, config.rs | Wire --log-format to init_tracing |
| MF-02 | MF | worker.rs | Add Default impl for WorkerContext |
| MF-03 | MF | worker.rs | Box Message in WorkerAction::SendMessage |
| BUG-01 / BUG-05 / BUG-06 | MF | config.rs, commands.rs | Validate workers >= 1 at CLI layer |
| SF-01 | SF | commands.rs | Deferred (function length) |
| SF-02 | SF | coordinator.rs | Deferred (magic number) |
| BUG-03 | SF | coordinator.rs | Deferred (self-transition logging) |
| BUG-04 | SF | worker.rs | Deferred (zero-filled stats by design) |
| NTH-01 | NTH | error.rs | Deferred (RelError alias) |
| NTH-02 | NTH | commands.rs | Deferred (inline paths) |
