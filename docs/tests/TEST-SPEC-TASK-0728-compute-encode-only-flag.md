# TEST-SPEC-TASK-0728: Tests for TASK-0728 — `compute --encode-only --output <path>`

**Task:** TASK-0728
**Spec:** SPEC-12 (formats), SPEC-27 v3 (encoder API, R21/R23)
**Bundle:** D-017 (Multi-container Horner distribution demo)
**Requirements covered:** AC1..AC5 of TASK-0728
**Invariants asserted:** none new — guards SPEC-27 R23 pipeline boundary (encode short-circuit MUST NOT call `reduce_all`); guards SPEC-12 bincode-v2 round-trip stability.

---

## Scope

Cover the `--encode-only` flag added to `ComputeArgs`:

1. CLI parser semantics (clap `requires = "output"`; legacy positional rejection).
2. Encode short-circuit: writes `.bin`, does NOT reduce.
3. `.bin` reload round-trip is bit-for-bit stable (HornerCodec is deterministic per TASK-0714).
4. Backward compatibility for the existing `compute --encoder horner --input <json>` (no `--encode-only`) path.

### Test category & location

| #            | Category                  | File                                                          | LoC |
|--------------|---------------------------|---------------------------------------------------------------|-----|
| UT-0728-01   | unit (in-module clap)     | `relativist-core/src/config.rs` (`#[cfg(test)]`)              | ~20 |
| UT-0728-02   | unit (in-module clap)     | same                                                           | ~15 |
| UT-0728-03   | unit (in-module clap)     | same                                                           | ~15 |
| UT-0728-04   | unit (in-module clap)     | same                                                           | ~15 |
| IT-0728-05   | integration (commands)    | `relativist-core/tests/compute_encode_only.rs` (new)          | ~25 |
| IT-0728-06   | integration (commands)    | same                                                           | ~25 |
| IT-0728-07   | integration (commands)    | same                                                           | ~25 |
| IT-0728-08   | integration (commands)    | same                                                           | ~20 |
| IT-0728-09   | integration (commands)    | same                                                           | ~25 |
| IT-0728-10   | integration (commands)    | same                                                           | ~20 |

### Test floor delta

- default: **+10**
- zero-copy: **+10**
- streaming-no-recycle: **+10**
- release: **+10** (all tests are sync, no async runtime needed)

---

## Unit Tests (CLI parser)

### UT-0728-01: `encode_only_requires_output_flag`

**Function under test:** clap derivation on `ComputeArgs` (`relativist-core/src/config.rs`, the `ComputeArgs` struct + `Cli::try_parse_from` path).

**Purpose:** Verify that the `requires = "output"` attribute on `encode_only` causes clap to reject parses missing `--output`.

**Input:**
```rust
let res = Cli::try_parse_from([
    "relativist", "compute",
    "--codec", "horner",
    "--input", r#"{"coeffs":[1,2,3],"x":10}"#,
    "--encode-only",
]);
```

**Expected output:**
```rust
let err = res.unwrap_err();
assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
let msg = err.to_string();
assert!(msg.contains("--output") || msg.contains("output"),
        "missing-output error must mention --output, got: {msg}");
```

**Edge cases:**
- Long alias `--encode-only` only (no short form). MUST fail without `--output`.

---

### UT-0728-02: `encode_only_with_output_parses_successfully`

**Function under test:** `Cli::try_parse_from` on `ComputeArgs`.

**Purpose:** Happy path — `--encode-only --output x.bin` parses and field is set.

**Input:**
```rust
let cli = Cli::try_parse_from([
    "relativist", "compute",
    "--codec", "horner",
    "--input", r#"{"coeffs":[1,2,3],"x":10}"#,
    "--encode-only",
    "--output", "/tmp/out.bin",
]).expect("parse must succeed");
```

**Expected output:**
```rust
match cli.command {
    Command::Compute(args) => {
        assert!(args.encode_only, "encode_only must be true");
        assert_eq!(args.output.as_deref(), Some(std::path::Path::new("/tmp/out.bin")));
        assert_eq!(args.codec.as_deref(), Some("horner"));
    }
    other => panic!("expected Compute, got {other:?}"),
}
```

---

### UT-0728-03: `encode_only_default_is_false_when_omitted`

**Function under test:** `Cli::try_parse_from`.

**Purpose:** Backward compatibility — without `--encode-only`, the bool defaults to false and existing `--output` semantics (save reduced net) are unaffected.

**Input:**
```rust
let cli = Cli::try_parse_from([
    "relativist", "compute",
    "--codec", "horner",
    "--input", r#"{"coeffs":[1,2,3],"x":10}"#,
    "--output", "/tmp/reduced.bin",
]).unwrap();
```

**Expected output:**
```rust
match cli.command {
    Command::Compute(args) => assert!(!args.encode_only),
    _ => unreachable!(),
}
```

---

### UT-0728-04: `encode_only_does_not_require_output_when_absent_from_args`

**Function under test:** `Cli::try_parse_from`.

**Purpose:** clap `requires` is one-directional — omitting both `--encode-only` and `--output` is valid (legacy mode).

**Input:**
```rust
let cli = Cli::try_parse_from([
    "relativist", "compute",
    "--codec", "horner",
    "--input", r#"{"coeffs":[1,2,3],"x":10}"#,
]);
```

**Expected output:** `cli.is_ok()` AND `args.encode_only == false` AND `args.output.is_none()`.

---

## Integration Tests (command execution)

All integration tests live in `relativist-core/tests/compute_encode_only.rs` (new). They invoke the public command entrypoint (call `run_compute_command` directly with a constructed `ComputeArgs`, OR shell out via `assert_cmd` if `run_compute_command` is not `pub` — developer's choice; prefer the in-process call for speed).

Shared helpers:
```rust
use relativist_core::config::ComputeArgs;
use relativist_core::commands::run_compute_command;
use relativist_core::encoding::default_registry;
use relativist_core::io::binary::load_bin;
use tempfile::NamedTempFile;
```

### IT-0728-05: `encode_only_writes_nonempty_bin_for_horner_degree_2`

**Function under test:** `run_compute_command` (`relativist-core/src/commands.rs`) — specifically the new short-circuit branch in `run_compute_with_encoder`.

**Purpose:** Happy path — degree-2 Horner polynomial encodes to a non-empty `.bin`, file exists, and `load_bin` succeeds (bincode is structurally valid).

**Input:**
```rust
let tmp = NamedTempFile::new().unwrap();
let args = ComputeArgs {
    op: None,
    a: None,
    b: None,
    codec: Some("horner".to_string()),
    encoder: None,
    input: Some(r#"{"coeffs":[10000,500,1],"x":100}"#.to_string()),
    encode_only: true,
    output: Some(tmp.path().to_path_buf()),
    // ...other fields default per existing ComputeArgs construction
    ..Default::default() // or full field list — developer chooses
};
run_compute_command(args).expect("encode-only must succeed");
```

**Expected output:**
```rust
let meta = std::fs::metadata(tmp.path()).unwrap();
assert!(meta.len() > 0, "encoded .bin must be non-empty");
let loaded = load_bin(tmp.path()).expect("bincode must be valid");
assert!(loaded.count_live_agents() > 0, "loaded net must have agents");
```

**Edge cases:** Verifies non-empty + bincode-valid + reload roundtrip (per user mandate: do NOT inspect IC content).

---

### IT-0728-06: `encode_only_constant_polynomial_smallest_net`

**Function under test:** same as IT-0728-05.

**Purpose:** Boundary — constant polynomial `coeffs:[42]` (degree 0) produces the smallest valid Horner net. Must not panic and must produce a loadable `.bin`.

**Input:** `input = r#"{"coeffs":[42],"x":7}"#`, `encode_only = true`, `output = Some(tmp.path())`.

**Expected output:**
```rust
run_compute_command(args).expect("constant polynomial must encode");
let loaded = load_bin(tmp.path()).unwrap();
assert!(loaded.count_live_agents() > 0);
// Constant polynomial encodes to a Church numeral '42' — single chain,
// no Horner steps. We don't inspect structure; just confirm valid bincode.
```

**Edge cases:** confirms the codec gracefully handles `coeffs.len() == 1` without slicing-off-end errors.

---

### IT-0728-07: `encode_only_degree_2_max_envelope_loads_cleanly`

**Function under test:** same as IT-0728-05.

**Purpose:** Boundary at the high end of the working envelope — degree-2 with reasonable coefficients (per `horner_live_demo.sh` envelope). Confirms the encode-only path does not OOM, panic, or truncate.

**Input:** `input = r#"{"coeffs":[10000,500,1],"x":100}"#` (matches TASK-0731 reference input). `encode_only = true`.

**Expected output:**
```rust
run_compute_command(args).expect("envelope-max input must encode");
let loaded = load_bin(tmp.path()).unwrap();
let n_agents = loaded.count_live_agents();
assert!(n_agents > 0);
// Determinism cross-check: a second encode of the same input must yield
// the same agent count (HornerCodec deterministic per TASK-0714).
let tmp2 = NamedTempFile::new().unwrap();
let args2 = /* same as args, but output = tmp2 */;
run_compute_command(args2).unwrap();
let loaded2 = load_bin(tmp2.path()).unwrap();
assert_eq!(loaded.count_live_agents(), loaded2.count_live_agents(),
           "HornerCodec must be deterministic");
```

---

### IT-0728-08: `encode_only_does_not_reduce_redexes_remain_in_queue`

**Function under test:** `run_compute_with_encoder` short-circuit branch (per acceptance criterion AC1 — does NOT call `reduce_all`).

**Purpose:** **Canary test** — proves the short-circuit really skips `reduce_all`. After encode-only, the loaded net MUST still contain unreduced redexes.

**Input:**
```rust
let args = /* encode_only=true, output=tmp, codec=horner,
              input='{"coeffs":[10000,500,1],"x":100}' */;
run_compute_command(args).unwrap();
let loaded = load_bin(tmp.path()).unwrap();
```

**Expected output:**
```rust
assert!(!loaded.redex_queue.is_empty(),
        "encode-only must NOT reduce; redex_queue must be non-empty for non-trivial input");
```

**Edge cases:** If the chosen input has zero redexes pre-reduction (rare for Horner with non-trivial coeffs), the test would be vacuous — input choice is intentional (matches TASK-0731 reference and is known to produce redexes per `EncoderRegistry::encode_and_validate` flow).

---

### IT-0728-09: `legacy_positional_compute_rejects_encode_only`

**Function under test:** dispatch logic at the top of `run_compute_command` (the branch that handles positional `op a b` vs `--codec`).

**Purpose:** AC3 — `compute add 3 5 --encode-only --output x.bin` returns a `RelativistError::Config` because there is no encoder to dispatch.

**Input:**
```rust
let args = ComputeArgs {
    op: Some("add".to_string()),
    a: Some("3".to_string()),
    b: Some("5".to_string()),
    codec: None,
    encoder: None,
    input: None,
    encode_only: true,
    output: Some(tmp.path().to_path_buf()),
    ..Default::default()
};
let res = run_compute_command(args);
```

**Expected output:**
```rust
match res {
    Err(RelativistError::Config(msg)) => {
        let low = msg.to_lowercase();
        assert!(low.contains("encoder") || low.contains("codec") || low.contains("encode-only"),
                "config error must mention encoder/codec/encode-only, got: {msg}");
    }
    Ok(()) => panic!("legacy positional + --encode-only must error"),
    Err(other) => panic!("expected Config error, got {other:?}"),
}
```

**Edge cases:** Also covers the scenario where the user mistakenly thinks `--encode-only` works for legacy mode. The error MUST be clear, not a panic.

---

### IT-0728-10: `backward_compat_no_encode_only_still_reduces_and_decodes`

**Function under test:** `run_compute_with_encoder` non-short-circuit branch (AC5).

**Purpose:** Without `--encode-only`, the existing encode→reduce→decode path is byte-identical to pre-task behaviour.

**Input:**
```rust
let args = ComputeArgs {
    codec: Some("horner".to_string()),
    input: Some(r#"{"coeffs":[10000,500,1],"x":100}"#.to_string()),
    encode_only: false,
    output: None, // no save
    ..Default::default()
};
run_compute_command(args).expect("legacy encoder path must still work");
```

**Expected output:** `Ok(())` — no panics, no errors. (Stdout printing of `Result:` line is implicit; not asserted to avoid coupling to formatting. The `compute_encoder.rs` test suite for TASK-0716/0717 already covers stdout contract.)

**Edge cases:** This is the canary that we did not accidentally break the existing pipeline while threading `encode_only` through.

---

## Property Tests

None for TASK-0728. The encode-only path is a thin wrapper around existing functions whose properties are covered by TASK-0714 (HornerCodec encoder PT) and TASK-0715 (Codec impl PT). Adding PT here would duplicate.

---

## Edge Cases Catalog

| #     | Scenario                                                  | Expected Behavior                                              | Test       |
|-------|-----------------------------------------------------------|----------------------------------------------------------------|------------|
| EC-01 | `--encode-only` without `--output`                        | clap `MissingRequiredArgument` error mentioning `--output`     | UT-0728-01 |
| EC-02 | Constant polynomial `coeffs:[42]`                         | Encode succeeds; smallest valid net; `.bin` non-empty           | IT-0728-06 |
| EC-03 | Degree-2 polynomial (max envelope)                        | Encode succeeds; deterministic across two runs                  | IT-0728-07 |
| EC-04 | Legacy positional `compute add 3 5 --encode-only`         | `RelativistError::Config` (no panic)                            | IT-0728-09 |
| EC-05 | Default mode (no `--encode-only`) with `--output` present | Behaviour unchanged — saves reduced net post-decode (existing) | IT-0728-10 |
| EC-06 | Re-encoding same input twice                              | Two `.bin` files have same `count_live_agents()` (determinism)  | IT-0728-07 |
| EC-07 | After encode-only, redex_queue still populated            | Confirms `reduce_all` was NOT invoked                           | IT-0728-08 |

---

## Quality Checklist

- [x] Every acceptance criterion (AC1..AC5) has at least one test.
- [x] CLI parser tested for both happy path and missing-output rejection.
- [x] Boundary cases (constant polynomial, max-envelope) explicit.
- [x] Canary test for "does NOT reduce" present (IT-0728-08).
- [x] Backward compat covered (IT-0728-10).
- [x] No panics: every error path returns `Err(RelativistError::Config(_))` or clap `Err`.
- [x] No dependence on stdout text formatting (avoids coupling to print statements).
- [x] No test depends on another test's state.

## Suggested next agent

`developer` — start with config.rs clap derivation (UT-0728-01..04), then commands.rs short-circuit (IT-0728-05..10).
