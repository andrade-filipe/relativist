# TEST-SPEC-TASK-0717: Tests for TASK-0717 — CLI `compute --encoder`/`--codec` with `conflicts_with`

**Task:** TASK-0717
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R21 (dual-form flag with clap `conflicts_with`, NOT `aliases` — SC-008), R23 (`compute` pipeline `encode → validate → reduce_all → decode → print JSON`)
**Test IDs (from SPEC-27 v3 §7.5):** T17 (legacy positional preserved), T18 (`--encoder` flag), T19 (`--codec` alias), T20 (mutually exclusive flags — `ErrorKind::ArgumentConflict`).

---

## Scope

Per Round 2 closure SC-008: clap **`conflicts_with`** pattern, NOT `aliases(...)` (which silently keeps last value). Both `--encoder` and `--codec` MUST appear separately in `--help` output. Application logic coalesces `flags.encoder.or(flags.codec)`.

T20 specifically asserts `clap::ErrorKind::ArgumentConflict` (programmatic check), NOT the integer exit code (which is platform-specific).

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0717-01 | unit (in-module) | none | `relativist-core/src/config.rs` | ~20 |
| UT-0717-02 | unit (in-module) | none | same | ~25 |
| UT-0717-03 | unit (in-module) | none | same | ~25 |
| UT-0717-04 | unit (in-module) | none | same | ~25 |
| UT-0717-05 | unit (in-module) | none | same | ~20 |
| UT-0717-06 | unit (in-module) | none | same | ~25 |

## Test floor delta (from TASK-0717 acceptance criteria)

- default: **+6** → ≥ 1878
- zero-copy: **+6** → ≥ 1922
- streaming-no-recycle: **+6** → ≥ 1869
- release: **+6** → ≥ 1820

---

## Unit Tests

### UT-0717-01: `cli_legacy_positional_compute_add_3_5_unchanged` (T17)

**Purpose:** R21 fallback / R7 — legacy positional syntax `compute add 3 5` MUST still parse and produce the same JSON output as before SPEC-27 v3.

**Input:**
```rust
use clap::Parser;

let args = Cli::parse_from(["relativist", "compute", "add", "3", "5"]);
// Resolve to ComputeArgs.
let compute_args = args.command.expect_compute();
```

**Expected output:**
```rust
assert_eq!(compute_args.op, Some(ArithmeticOp::Add));
assert_eq!(compute_args.a, Some(3));
assert_eq!(compute_args.b, Some(5));
assert!(compute_args.encoder.is_none());
assert!(compute_args.codec.is_none());
assert!(compute_args.input.is_none());
```

**Edge cases:**
- (EC-1) `compute mul 4 7`, `compute exp 2 3`, `compute sum_of_squares 3` — all positional forms parse correctly.
- (EC-2) Pipeline-end check: actually invoke `run_compute_command(compute_args)` and assert stdout matches the pre-v3 `compute add 3 5` JSON output (`{"result": 8, "interactions": <some_u64>}`). This is an integration-level check; if too heavy for in-module test, factor into a separate integration test under `tests/cli_compute_legacy.rs`.

---

### UT-0717-02: `cli_encoder_flag_horner_input_json` (T18)

**Purpose:** R21 — `--encoder horner --input <json>` invokes the registry-based pipeline.

**Input:**
```rust
let args = Cli::parse_from([
    "relativist", "compute",
    "--encoder", "horner",
    "--input", r#"{"coeffs":[3,2,5,1],"x":2}"#,
]);
let compute_args = args.command.expect_compute();
```

**Expected output:**
```rust
assert_eq!(compute_args.encoder.as_deref(), Some("horner"));
assert!(compute_args.codec.is_none());
assert_eq!(compute_args.input.as_deref(), Some(r#"{"coeffs":[3,2,5,1],"x":2}"#));
```

**Edge cases:**
- (EC-1) End-to-end: invoke `run_compute_command` and assert stdout JSON contains `"value":"35"` and `"bit_length":6`.
- (EC-2) `--encoder church_add --input '{"op":"add","a":3,"b":5}'` — Church codec via the registry path; output `{"result":8, ...}`.

---

### UT-0717-03: `cli_codec_flag_horner_input_json_identical_to_encoder` (T19)

**Purpose:** R21 — `--codec horner --input <json>` produces the **same** result as `--encoder horner --input <json>`.

**Input:**
```rust
let args_a = Cli::parse_from([
    "relativist", "compute",
    "--encoder", "horner",
    "--input", r#"{"coeffs":[1,1],"x":3}"#,
]);
let args_b = Cli::parse_from([
    "relativist", "compute",
    "--codec", "horner",
    "--input", r#"{"coeffs":[1,1],"x":3}"#,
]);

let coalesced_a = args_a.encoder.clone().or(args_a.codec.clone());
let coalesced_b = args_b.encoder.clone().or(args_b.codec.clone());
```

**Expected output:**
```rust
assert_eq!(coalesced_a, Some("horner".to_string()));
assert_eq!(coalesced_b, Some("horner".to_string()));
// End-to-end: both produce the same stdout JSON.
let out_a = run_compute_command_capturing_stdout(args_a);
let out_b = run_compute_command_capturing_stdout(args_b);
assert_eq!(out_a, out_b);
// Specifically: 1 + 1*3 = 4
assert!(out_a.contains(r#""value":"4""#));
```

**Edge cases:**
- (EC-1) `--codec horner` alone (no `--encoder`) MUST NOT trigger the conflict (T20 case requires BOTH flags set).
- (EC-2) Whitespace / case in flag names: `--Encoder` (capitalized) MUST fail with unknown-arg error (clap is case-sensitive on long names).

---

### UT-0717-04: `cli_encoder_codec_both_set_returns_conflict_error` (T20 — SC-008)

**Purpose:** SC-008 closure. clap MUST reject `--encoder horner --codec horner` with `ErrorKind::ArgumentConflict`. Programmatic check (NOT integer exit code).

**Input:**
```rust
let result = Cli::try_parse_from([
    "relativist", "compute",
    "--encoder", "horner",
    "--codec",   "horner",
    "--input",   r#"{"coeffs":[1],"x":0}"#,
]);
```

**Expected output:**
```rust
let err = result.expect_err("clap MUST reject conflicting --encoder + --codec");
assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);

// Error message MUST mention both flag names (per TASK-0717 acceptance).
let rendered = err.to_string();
assert!(rendered.contains("--encoder"), "error MUST mention --encoder");
assert!(rendered.contains("--codec"),   "error MUST mention --codec");
```

**Edge cases:**
- (EC-1) Reverse order: `--codec horner --encoder horner` — same `ArgumentConflict`.
- (EC-2) Different values: `--encoder horner --codec church_add` — STILL conflict (mutual exclusion is on the flag names, not on equal values).
- (EC-3) `--encoder` AND `--codec` AND `--input`: all three present, two flags conflict — error fires before `--input` is even validated.
- (EC-4) Anti-regression on `aliases(...)` semantics: a test that programmatically checks the clap argument tree does NOT use `aliases` for these two flags. Implementation-dependent; OPTIONAL.

---

### UT-0717-05: `cli_codec_set_input_missing_returns_config_error`

**Purpose:** Parity with the existing `--encoder` rule — `--codec horner` without `--input` MUST return a `Config` error mentioning `--encoder/--codec` (NOT just `--encoder`).

**Input:**
```rust
let args = Cli::parse_from([
    "relativist", "compute",
    "--codec", "horner",
    // no --input
]);
let compute_args = args.command.expect_compute();
let result = run_compute_command(compute_args);
```

**Expected output:**
```rust
match result {
    Err(e) if e.is_config_error() => {
        let msg = e.to_string();
        assert!(msg.contains("--encoder") || msg.contains("--codec"));
        assert!(msg.contains("input") || msg.contains("--input"));
    }
    other => panic!("expected config error mentioning --input, got {:?}", other),
}
```

**Edge cases:**
- (EC-1) `--encoder horner` (without --input): same Config error.
- (EC-2) Empty `--input ""`: behavior depends on encoder; likely `EncodeError::InvalidInput` (JSON parse error). Acceptable; this test asserts the **missing** input case, not malformed input.

---

### UT-0717-06: `clap_help_lists_encoder_and_codec_separately`

**Purpose:** SC-008 — both flags MUST appear separately in `--help` output (NOT as a single flag with aliases). Help text for `--codec` SHOULD reference `--encoder` as primary.

**Input:**
```rust
let mut cmd = Cli::command();
let help_text = cmd.find_subcommand_mut("compute").unwrap().render_help().to_string();
```

**Expected output:**
```rust
assert!(help_text.contains("--encoder"), "--encoder MUST appear in help");
assert!(help_text.contains("--codec"),   "--codec MUST appear in help");
// Both flags MUST appear as separate entries (not e.g., "--encoder, --codec" on one line as in alias formatting).
let encoder_line_count = help_text.lines().filter(|l| l.trim_start().starts_with("--encoder")).count();
let codec_line_count = help_text.lines().filter(|l| l.trim_start().starts_with("--codec")).count();
assert_eq!(encoder_line_count, 1);
assert_eq!(codec_line_count, 1);
```

**Edge cases:**
- (EC-1) clap may render help differently across versions; use a contains/regex assertion (NOT byte-exact snapshot) to avoid spurious failures (per TASK-0717 NOTE).
- (EC-2) `--codec` help text mentions `--encoder` as primary (informative; SHOULD per TASK-0717 — relax to `assert!(help_text.contains("--encoder") && help_text.contains("--codec"))` if exact phrasing is too brittle).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Legacy positional `compute add 3 5` | parses; encoder/codec/input all None | UT-0717-01 |
| EC-002 | `--encoder horner --input <json>` | encoder = Some("horner"); pipeline runs | UT-0717-02 |
| EC-003 | `--codec horner --input <json>` | codec = Some("horner"); identical output to --encoder | UT-0717-03 |
| EC-004 | Both `--encoder` AND `--codec` set | `ErrorKind::ArgumentConflict`; error mentions both flags | UT-0717-04 |
| EC-005 | `--codec horner` no `--input` | Config error mentioning input + codec | UT-0717-05 |
| EC-006 | `--help` lists both flags separately | each flag on its own line | UT-0717-06 |
| EC-007 | Reverse-order conflict `--codec X --encoder Y` | same ArgumentConflict | UT-0717-04 EC-1 |
| EC-008 | Different values `--encoder X --codec Y` | still ArgumentConflict (mutual exclusion on flag names) | UT-0717-04 EC-2 |
| EC-009 | `--Encoder` (capitalized) | unknown-arg error (case-sensitive) | UT-0717-03 EC-2 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T17 (legacy positional preserved) | UT-0717-01 |
| T18 (`--encoder` flag pipeline) | UT-0717-02 |
| T19 (`--codec` alias identical to `--encoder`) | UT-0717-03 |
| T20 (mutually exclusive — `ErrorKind::ArgumentConflict`) | UT-0717-04 |

## Dependencies Context

- `clap` crate (existing).
- `EncoderRegistry::encode_and_validate(name, input)` and `decode(name, net)` (existing).
- `default_registry()` populated with `horner` (TASK-0716).
- `HornerCodec` (TASK-0715).

## Notes

- T20 asserts `ErrorKind::ArgumentConflict` programmatically; the integer exit code is platform-specific (typically 2 on Unix) and is NOT asserted. Per TASK-0717 NOTE.
- The "Pattern note for SPEC-07" in TASK-0717 (and SPEC-27 v3 §3.6 R21 trailing commentary) recommending a separate task to register the dual-form flag pattern in SPEC-07 is **out of scope** for this bundle.
- Existing tests at `config.rs:1186, 1210, 1240` reference SPEC-27 R21 for `--encoder`-only behavior — those MUST be updated to cover `--codec` symmetrically (do NOT remove `--encoder` cases). Stage 4 reviewer confirms.
- Test floor delta: **+6** (6 unit tests in-module).
