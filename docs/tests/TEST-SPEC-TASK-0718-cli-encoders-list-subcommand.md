# TEST-SPEC-TASK-0718: Tests for TASK-0718 — `encoders list` (and `codecs list` alias) CLI subcommand

**Task:** TASK-0718
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R22 (`encoders list` MUST; `codecs list` MAY as alias)
**Test IDs (from SPEC-27 v3 §7.5):** T21 (encoder list outputs 5 v1 codecs; alias produces identical output).

---

## Scope

This task verifies that the existing `relativist encoders list` subcommand (whose stub already exists at `relativist-cli/src/main.rs:67` per TASK-0718 context) emits the post-TASK-0716 5-codec layout, and that `relativist codecs list` (clap subcommand alias) produces byte-identical output.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0718-01 | unit (in-module or integration) | none | `relativist-core/src/commands.rs` (or `relativist-cli/tests/`) | ~30 |
| UT-0718-02 | unit (in-module or integration) | none | same | ~25 |
| UT-0718-03 | unit | none | same | ~15 |
| UT-0718-04 | unit | none | same | ~20 |

## Test floor delta (from TASK-0718 acceptance criteria)

- default: **+4** → ≥ 1882
- zero-copy: **+4** → ≥ 1926
- streaming-no-recycle: **+4** → ≥ 1873
- release: **+4** → ≥ 1824

---

## Unit Tests

### UT-0718-01: `cli_encoders_list_outputs_5_v3_codecs` (T21)

**Purpose:** R22 — `relativist encoders list` outputs exactly 5 codec lines in canonical R19 order; first stdout line is `Available encoders:`.

**Approach:** Capture stdout from `run_encoders_command(args)` and assert content. Avoid spawning a real subprocess to keep the test fast and deterministic.

**Input:**
```rust
let registry = default_registry();
let stdout = capture_stdout(|| run_encoders_command(EncodersArgs { /* list */ }, &registry));
let lines: Vec<&str> = stdout.lines().collect();
```

**Expected output:**
```rust
assert_eq!(lines[0], "Available encoders:");

// 5 codec lines follow.
let codec_lines = &lines[1..];
assert_eq!(codec_lines.len(), 5);

// Each line begins with the codec name (post-leading-whitespace).
let names: Vec<String> = codec_lines.iter().map(|l| l.trim().split_whitespace().next().unwrap().to_string()).collect();
assert_eq!(
    names,
    vec!["church_add", "church_mul", "church_exp", "church_sum_of_squares", "horner"]
);
```

**Edge cases:**
- (EC-1) Output ends with a single newline (no trailing whitespace beyond `\n`).
- (EC-2) No empty lines within the codec list (output is a tight block).
- (EC-3) Each codec line contains its description (per TASK-0718 acceptance: "name padded to a column-aligned width, then description"). Concretely, each codec line MUST contain the description string from the codec's `description()` (asserted in UT-0718-04).

---

### UT-0718-02: `cli_codecs_list_alias_produces_identical_output` (T21 alias)

**Purpose:** R22 MAY — `relativist codecs list` produces byte-identical output to `relativist encoders list`.

**Approach:** Both invocations route to `run_encoders_command` per TASK-0718 NOTE preferred approach (clap subcommand alias on the same variant: `#[command(alias = "codecs")]`).

**Input:**
```rust
// Parse CLI both ways.
let cli_encoders = Cli::parse_from(["relativist", "encoders", "list"]);
let cli_codecs   = Cli::parse_from(["relativist", "codecs",   "list"]);

let stdout_encoders = capture_stdout(|| dispatch(&cli_encoders));
let stdout_codecs   = capture_stdout(|| dispatch(&cli_codecs));
```

**Expected output:**
```rust
assert_eq!(stdout_encoders, stdout_codecs, "encoders list and codecs list MUST be byte-identical");
```

**Edge cases:**
- (EC-1) Subcommand alias is implemented via `#[command(alias = "codecs")]` (preferred) — both routes hit the same handler. The alternative (two separate variants both routing to `run_encoders_command`) is acceptable but redundant.
- (EC-2) `relativist codec list` (singular) MAY also be accepted as a clap alias if convenient, but this is NOT asserted.

---

### UT-0718-03: `cli_encoders_list_excludes_lambda` (T16-derived)

**Purpose:** Post-TASK-0716, `lambda` MUST NOT appear in the listing.

**Input:**
```rust
let stdout = capture_stdout(|| run_encoders_command(EncodersArgs { /* list */ }, &default_registry()));
```

**Expected output:**
```rust
assert!(!stdout.to_lowercase().contains("lambda"), "encoders list MUST NOT contain 'lambda' post-v3");
```

**Edge cases:**
- (EC-1) The substring "lambda" might appear in another description in the future; this test enforces exact absence at the v3 baseline.
- (EC-2) Case-insensitive check: `LAMBDA`, `Lambda`, `lambda` all forbidden.

---

### UT-0718-04: `cli_encoders_list_includes_horner_and_description`

**Purpose:** R22 — output contains a line starting with `horner` followed by `HornerCodec::description()`.

**Input:**
```rust
let stdout = capture_stdout(|| run_encoders_command(EncodersArgs { /* list */ }, &default_registry()));
let horner_line = stdout
    .lines()
    .find(|l| l.trim().starts_with("horner"))
    .expect("horner line MUST exist");
```

**Expected output:**
```rust
let horner_codec = HornerCodec::new();
let desc = horner_codec.description();
assert!(horner_line.contains(desc), "horner line MUST include its description");
```

**Edge cases:**
- (EC-1) `description()` text is single-line (no embedded newlines — UT-0716-05 EC-3 dependency).
- (EC-2) Whitespace between name and description: padding is editorial (TASK-0718 NOTE: ~22 chars); test asserts content presence, NOT exact column alignment.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | `encoders list` outputs 5 codecs | exactly 5 lines after header | UT-0718-01 |
| EC-002 | First line is `Available encoders:` | exact match | UT-0718-01 |
| EC-003 | Codec names in canonical R19 order | matches `["church_add", ..., "horner"]` | UT-0718-01 |
| EC-004 | Single trailing newline | no double-newlines or extra whitespace | UT-0718-01 EC-1 |
| EC-005 | `codecs list` alias byte-identical | strict equality | UT-0718-02 |
| EC-006 | "lambda" absent from output | post-v3 exclusion | UT-0718-03 |
| EC-007 | "horner" line includes description | includes substring | UT-0718-04 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T21 (encoders list, codecs list alias) | UT-0718-01 + UT-0718-02 |

## Dependencies Context

- `default_registry()` from TASK-0716 (5 codecs).
- `EncoderRegistry::list() -> Vec<(&str, &str)>` (existing).
- `clap` subcommand alias mechanism (clap 4.x: `#[command(alias = "...")]`).
- `HornerCodec::description()` from TASK-0715.

## Notes

- The two clap mechanisms (subcommand alias on same variant vs. two separate variants) are interchangeable. Per TASK-0718 NOTE, prefer `#[command(alias = "codecs")]` for fewer code paths.
- Output formatting (column padding) is editorial; tests verify content, not exact whitespace beyond the trailing newline rule.
- The tests can live either as unit tests in `commands.rs` (with stdout-capture helper) OR as integration tests in `relativist-cli/tests/`. Choose the location that minimizes test fragility against clap version bumps.
- Test floor delta: **+4** unit/integration tests.
