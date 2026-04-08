# TEST-SPEC-0144: Implement init_tracing with fmt::Layer and EnvFilter

**Task:** TASK-0144
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: init_tracing with default config does not panic

**Input:** `init_tracing(ObservabilityConfig::default_coordinator())`
**Expected:** Returns without panic; tracing subscriber is set
**Verifies:** T1 -- basic initialization (must run in isolated test process)

### T2: Calling init_tracing twice panics

**Input:** Call `init_tracing(...)` twice in the same process
**Expected:** Second call panics (inherent from `set_global_default`)
**Verifies:** R32 -- exactly-once initialization

### T3: JSON format produces valid JSON output

**Input:** `init_tracing` with `log_format: LogFormat::Json`; emit a `tracing::info!("test")`; capture output
**Expected:** Output line is parseable as JSON
**Verifies:** R3 -- JSON log format

### T4: RUST_LOG override is respected

**Input:** Set `RUST_LOG=relativist::reduction=trace`; init tracing; emit a trace-level event from the reduction target
**Expected:** Event appears in output (overriding the default WARN level)
**Verifies:** R4 -- env var override

### T5: fmt::Layer includes target and thread ID

**Input:** Emit a log event after init_tracing with Text format
**Expected:** Output contains the module target path and a thread ID
**Verifies:** R9 -- structured fields in output

### T6: File name and line number are disabled

**Input:** Emit a log event after init_tracing
**Expected:** Output does NOT contain source file name or line number
**Verifies:** R9 -- file/line disabled by default

---

## Edge Cases

### E1: Local role is accepted

**Verify:** `init_tracing(ObservabilityConfig::default_local())` does not panic and structured logging works.
**Why:** R33a -- local mode uses the same tracing initialization.

### E2: No println/eprintln/dbg usage

**Verify:** Source code of `tracing_init.rs` does not contain `println!`, `eprintln!`, `dbg!`, or `log::` macros.
**Why:** R1 -- all output must go through `tracing`.
