# TEST-SPEC-0100: Refactor CLI to use Args structs

**Task:** TASK-0100
**Spec:** SPEC-07, SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Parse coordinator subcommand with required args

**Type:** Unit test
**Input:** `Cli::try_parse_from(["relativist", "coordinator", "--workers", "4", "--input", "input.bin"])`
**Expected:** Parses successfully; `args.workers == 4`, `args.input == PathBuf::from("input.bin")`, `args.bind == "127.0.0.1:9000".parse::<SocketAddr>()`
**Verifies:** R3 -- CoordinatorArgs with default bind address

### T2: Parse coordinator with explicit bind

**Type:** Unit test
**Input:** `Cli::try_parse_from(["relativist", "coordinator", "--workers", "4", "--bind", "0.0.0.0:8080", "--input", "net.bin"])`
**Expected:** `args.bind == "0.0.0.0:8080".parse::<SocketAddr>()`
**Verifies:** R3 -- bind overrides default

### T3: Parse worker subcommand

**Type:** Unit test
**Input:** `Cli::try_parse_from(["relativist", "worker", "--coordinator", "127.0.0.1:9000"])`
**Expected:** Parses successfully; `args.coordinator == "127.0.0.1:9000"`
**Verifies:** R4 -- WorkerArgs with coordinator address

### T4: No subcommand returns error

**Type:** Unit test
**Input:** `Cli::try_parse_from(["relativist"])`
**Expected:** Returns `Err` (clap error for missing subcommand)
**Verifies:** R6 -- no subcommand shows help and exits with code 1

### T5: Parse local subcommand with defaults

**Type:** Unit test
**Input:** `Cli::try_parse_from(["relativist", "local", "--workers", "2", "--input", "test.bin"])`
**Expected:** `args.workers == 2`, `args.strategy == "round-robin"` (default), `args.max_rounds == None`
**Verifies:** R5 -- LocalArgs with default strategy

### T6: Short aliases work

**Type:** Unit test
**Input:** `Cli::try_parse_from(["relativist", "coordinator", "-w", "4", "-b", "0.0.0.0:9000", "-i", "net.bin"])`
**Expected:** Parses identically to long form: `workers == 4`, `bind == 0.0.0.0:9000`, `input == "net.bin"`
**Verifies:** Short aliases -w, -b, -i are registered

### T7: All 7 subcommands are recognized

**Type:** Unit test
**Input:** Parse each of: `coordinator`, `worker`, `local`, `reduce`, `inspect`, `generate`, `compute` with minimal required args
**Expected:** All 7 parse successfully into the correct `Command` variant
**Verifies:** SPEC-13 R43 -- all subcommands present

---

## Edge Cases

### E1: Invalid bind address format

**Verify:** `Cli::try_parse_from(["relativist", "coordinator", "-w", "4", "-b", "not-an-address", "-i", "x.bin"])` returns `Err`.
**Why:** clap should reject invalid SocketAddr values at parse time.

### E2: Args structs are in config module, not main.rs

**Verify:** `use relativist::config::{Cli, Command, CoordinatorArgs, WorkerArgs, LocalArgs};` compiles.
**Why:** SPEC-07 Section 4.1 requires structs to be in the config module for reuse and testability.
