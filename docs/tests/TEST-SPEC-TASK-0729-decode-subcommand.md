# TEST-SPEC-TASK-0729: Tests for TASK-0729 — `decode` subcommand

**Task:** TASK-0729
**Spec:** SPEC-12 (formats), SPEC-27 v3 R14'/R15'/R16'/R21 (decode contract, codec/encoder mutex)
**Bundle:** D-017
**Requirements covered:** AC1..AC5 of TASK-0729
**Invariants asserted:**
- SPEC-27 v3 R14' (decoder honors `discover_root` post-merge contract — see `run_compute_with_encoder` lines 791–804).
- SPEC-27 v3 R4 NotNormalForm semantics (decode on un-reduced net surfaces a clear error, no panic).
- SPEC-27 v3 R21 codec/encoder alias mutex.

---

## Scope

Cover the new `Decode(DecodeArgs)` Command variant and `run_decode_command` handler:

1. CLI parser: codec/encoder mutex (clap `conflicts_with`), required `--input`, optional `--output`.
2. Happy-path decode against a previously-saved reduced `.bin` produces the same JSON as the in-process compute pipeline.
3. Root recovery: nets with `root = None` post-load must trigger `discover_root` automatically.
4. Error paths: corrupt `.bin`, unknown codec, un-reduced net, codec/encoder mismatch.
5. Output sink: stdout vs `--output <path>`.

### Test category & location

| #            | Category                  | File                                                          | LoC |
|--------------|---------------------------|---------------------------------------------------------------|-----|
| UT-0729-01   | unit (in-module clap)     | `relativist-core/src/config.rs` (`#[cfg(test)]`)              | ~15 |
| UT-0729-02   | unit (in-module clap)     | same                                                           | ~15 |
| UT-0729-03   | unit (in-module clap)     | same                                                           | ~15 |
| UT-0729-04   | unit (in-module clap)     | same                                                           | ~15 |
| IT-0729-05   | integration (commands)    | `relativist-core/tests/decode_subcommand.rs` (new)            | ~30 |
| IT-0729-06   | integration (commands)    | same                                                           | ~25 |
| IT-0729-07   | integration (commands)    | same                                                           | ~30 |
| IT-0729-08   | integration (commands)    | same                                                           | ~20 |
| IT-0729-09   | integration (commands)    | same                                                           | ~20 |
| IT-0729-10   | integration (commands)    | same                                                           | ~25 |
| IT-0729-11   | integration (commands)    | same                                                           | ~25 |

### Test floor delta

- default: **+11**
- zero-copy: **+11**
- streaming-no-recycle: **+11**
- release: **+11**

---

## Unit Tests (CLI parser)

### UT-0729-01: `decode_parses_with_codec_and_input`

**Function under test:** clap derivation on `DecodeArgs` (`relativist-core/src/config.rs`).

**Purpose:** Happy-path parse — `--codec horner --input x.bin` parses; output is `None`.

**Input:**
```rust
let cli = Cli::try_parse_from([
    "relativist", "decode",
    "--codec", "horner",
    "--input", "/tmp/x.bin",
]).expect("parse");
```

**Expected output:**
```rust
match cli.command {
    Command::Decode(args) => {
        assert_eq!(args.codec.as_deref(), Some("horner"));
        assert!(args.encoder.is_none());
        assert_eq!(args.input, std::path::PathBuf::from("/tmp/x.bin"));
        assert!(args.output.is_none());
    }
    _ => panic!("expected Decode"),
}
```

---

### UT-0729-02: `decode_codec_and_encoder_together_fails`

**Function under test:** clap `conflicts_with` on `DecodeArgs::encoder`.

**Purpose:** SPEC-27 R21 — passing both `--codec` and `--encoder` MUST fail at parse time.

**Input:**
```rust
let res = Cli::try_parse_from([
    "relativist", "decode",
    "--codec", "horner",
    "--encoder", "horner",
    "--input", "/tmp/x.bin",
]);
```

**Expected output:**
```rust
let err = res.unwrap_err();
assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
```

---

### UT-0729-03: `decode_encoder_alias_parses`

**Function under test:** clap derivation.

**Purpose:** AC2 — `--encoder` accepted as alias for `--codec`.

**Input:**
```rust
let cli = Cli::try_parse_from([
    "relativist", "decode",
    "--encoder", "horner",
    "--input", "/tmp/x.bin",
]).expect("parse");
```

**Expected output:** `args.encoder == Some("horner")`, `args.codec.is_none()`.

---

### UT-0729-04: `decode_requires_input_path`

**Function under test:** clap derivation — `input: PathBuf` is required (not `Option`).

**Purpose:** Missing `--input` fails parsing.

**Input:**
```rust
let res = Cli::try_parse_from([
    "relativist", "decode",
    "--codec", "horner",
]);
```

**Expected output:**
```rust
let err = res.unwrap_err();
assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
```

---

## Integration Tests

Shared helpers in `relativist-core/tests/decode_subcommand.rs`:
```rust
use relativist_core::config::DecodeArgs;
use relativist_core::commands::run_decode_command;
use relativist_core::encoding::{default_registry, discover_root};
use relativist_core::io::binary::{save_bin, load_bin};
use relativist_core::reduction::reduce_all;
use tempfile::NamedTempFile;
use serde_json::Value;

const HORNER_INPUT: &[u8] = br#"{"coeffs":[10000,500,1],"x":100}"#;

fn make_reduced_horner_bin() -> NamedTempFile {
    let reg = default_registry();
    let mut net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut net);
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    tmp
}

fn make_unreduced_horner_bin() -> NamedTempFile {
    let reg = default_registry();
    let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    tmp
}
```

### IT-0729-05: `decode_reduced_horner_bin_matches_inproc_pipeline`

**Function under test:** `run_decode_command` (new in `relativist-core/src/commands.rs`).

**Purpose:** Happy path — decode a reduced `.bin` and confirm the JSON matches what `run_compute_with_encoder` produces in-process.

**Input:**
```rust
let bin = make_reduced_horner_bin();
let out = NamedTempFile::new().unwrap();
let args = DecodeArgs {
    codec: Some("horner".to_string()),
    encoder: None,
    input: bin.path().to_path_buf(),
    output: Some(out.path().to_path_buf()),
};
run_decode_command(args).expect("decode must succeed");
let written = std::fs::read_to_string(out.path()).unwrap();
let decoded: Value = serde_json::from_str(&written).unwrap();
```

**Expected output:**
```rust
// Cross-check via in-process pipeline
let reg = default_registry();
let mut ref_net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
let _ = reduce_all(&mut ref_net);
if ref_net.root.is_none() { discover_root(&mut ref_net); }
let ref_json = reg.decode("horner", &ref_net).unwrap();

assert_eq!(decoded, ref_json);
// SPEC-27 v3 R15' schema:
assert!(decoded.get("value").is_some(), "JSON must have 'value' field");
assert!(decoded.get("bit_length").is_some(), "JSON must have 'bit_length' field");
```

---

### IT-0729-06: `decode_recovers_root_when_missing_post_load`

**Function under test:** `run_decode_command` — specifically the `if net.root.is_none() { discover_root(&mut net); }` block (mirrors `commands.rs:797–804`).

**Purpose:** AC critical — HornerCodec composes via `wire_*_into`, leaving `root = None` after reduction. After save/load, `root` is still `None`. The handler MUST `discover_root` automatically; otherwise `registry.decode` returns "no root agent".

**Input:**
```rust
let reg = default_registry();
let mut net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
let _ = reduce_all(&mut net);
// Force root=None to simulate the post-merge state (HornerCodec already does this naturally).
net.root = None;
let tmp = NamedTempFile::new().unwrap();
save_bin(&net, tmp.path()).unwrap();

// Sanity: confirm load preserves root=None
let loaded_check = load_bin(tmp.path()).unwrap();
assert!(loaded_check.root.is_none(), "saved net must round-trip root=None");

let args = DecodeArgs {
    codec: Some("horner".to_string()),
    encoder: None,
    input: tmp.path().to_path_buf(),
    output: None,
};
let res = run_decode_command(args);
```

**Expected output:**
```rust
res.expect("decode must succeed even when root=None on disk");
```

**Edge cases:** Without root recovery, this would error with `RelativistError::Encoding("no root agent")` or similar. The test would fail loudly — exactly the regression we want to catch.

---

### IT-0729-07: `decode_on_unreduced_bin_returns_clear_error_no_panic`

**Function under test:** `run_decode_command` — error propagation from `registry.decode`.

**Purpose:** Per TASK-0729 Notes: "If the user passes a non-reduced net, `registry.decode` will likely return an NotNormalForm error. That's the right behaviour." Must NOT panic.

**Input:**
```rust
let bin = make_unreduced_horner_bin();
let args = DecodeArgs {
    codec: Some("horner".to_string()),
    encoder: None,
    input: bin.path().to_path_buf(),
    output: None,
};
let res = run_decode_command(args);
```

**Expected output:**
```rust
match res {
    Err(e) => {
        let msg = e.to_string().to_lowercase();
        assert!(
            msg.contains("normal form") || msg.contains("redex") || msg.contains("not reduced"),
            "expected NotNormalForm-style error, got: {}", e
        );
    }
    Ok(()) => panic!("decoding an un-reduced net MUST error"),
}
```

**Edge cases:** Important boundary — the operator's pipeline forgot to run the coordinator. The error message must point at the right cause (redexes remaining), not a cryptic decoder failure.

---

### IT-0729-08: `decode_with_wrong_codec_returns_clear_error`

**Function under test:** `run_decode_command` — codec dispatch error path.

**Purpose:** Boundary — Horner-encoded `.bin` decoded with the wrong codec (e.g., `church_add` if registered, or any non-horner registered codec) must fail loudly. If only `horner` is registered after TASK-0716, use an obviously-unknown name to exercise the same path (covered separately by IT-0729-09; this test specifically targets the case where the codec exists but mismatches the net topology).

**Input:**
```rust
// If a second codec is registered, use it. Otherwise this test becomes IT-0729-09.
// Inspect default_registry().list() in setup; pick a non-horner codec if available.
let names: Vec<String> = default_registry().list()
    .iter().map(|(n,_)| n.to_string()).collect();
let alt = names.iter().find(|n| n.as_str() != "horner").cloned();

if let Some(other) = alt {
    let bin = make_reduced_horner_bin();
    let args = DecodeArgs {
        codec: Some(other),
        encoder: None,
        input: bin.path().to_path_buf(),
        output: None,
    };
    let res = run_decode_command(args);
    assert!(res.is_err(), "decoding horner bytes as a different codec must error");
}
// else: skip (becomes UT covered by IT-0729-09)
```

**Expected output:** `Err(_)` — error variant unspecified (Encoding or Config both acceptable). The MUST is "no panic".

**Edge cases:** If `default_registry()` exposes only `horner` post-D-017, this test degenerates and is a no-op; document that explicitly in the test body.

---

### IT-0729-09: `decode_with_unknown_codec_name_returns_config_error`

**Function under test:** codec lookup path in `run_decode_command` (`registry.decode` returns Config error for missing codec, per `EncoderRegistry::decode` contract).

**Purpose:** AC4 — unknown codec name produces a Config error listing available codecs.

**Input:**
```rust
let bin = make_reduced_horner_bin();
let args = DecodeArgs {
    codec: Some("nonexistent_codec_xyz".to_string()),
    encoder: None,
    input: bin.path().to_path_buf(),
    output: None,
};
let res = run_decode_command(args);
```

**Expected output:**
```rust
match res {
    Err(RelativistError::Config(msg)) => {
        let low = msg.to_lowercase();
        assert!(low.contains("nonexistent_codec_xyz") || low.contains("codec") || low.contains("not found") || low.contains("unknown"),
                "config error must reference the unknown name or 'codec', got: {msg}");
    }
    Ok(()) => panic!("unknown codec name MUST error"),
    Err(other) => panic!("expected Config error, got {other:?}"),
}
```

---

### IT-0729-10: `decode_corrupt_bin_returns_config_error_with_path`

**Function under test:** `run_decode_command` -> `io::binary::load_bin` error propagation.

**Purpose:** AC5 — feeding random bytes to `--input` must error cleanly, not panic, and the error should mention the path or the underlying bincode failure.

**Input:**
```rust
let tmp = NamedTempFile::new().unwrap();
std::fs::write(tmp.path(), b"this is not bincode at all, just garbage bytes").unwrap();
let args = DecodeArgs {
    codec: Some("horner".to_string()),
    encoder: None,
    input: tmp.path().to_path_buf(),
    output: None,
};
let res = run_decode_command(args);
```

**Expected output:**
```rust
match res {
    Err(e) => {
        // Acceptable variants: RelativistError::Config, ::Io, ::Encoding —
        // the MUST is "error, not panic", and the message references either
        // the path or the failure reason.
        let s = e.to_string().to_lowercase();
        assert!(
            s.contains("bincode") || s.contains("deserialize") || s.contains("corrupt")
              || s.contains("load") || s.contains(&tmp.path().display().to_string().to_lowercase()),
            "corrupt .bin error must mention parse failure or path, got: {}", e
        );
    }
    Ok(()) => panic!("corrupt .bin must not decode silently"),
}
```

---

### IT-0729-11: `decode_stdout_path_when_output_absent`

**Function under test:** `run_decode_command` output sink branch.

**Purpose:** AC1 — when `--output` is absent, JSON is printed to stdout. Verify by capturing the entrypoint's return (`Ok(())`) and asserting no file is created. (Stdout capture in unit tests is awkward; the contract test is "completes Ok and writes nothing to disk".)

**Input:**
```rust
let bin = make_reduced_horner_bin();
let args = DecodeArgs {
    codec: Some("horner".to_string()),
    encoder: None,
    input: bin.path().to_path_buf(),
    output: None, // <-- the case under test
};
run_decode_command(args).expect("decode to stdout must succeed");
```

**Expected output:** `Ok(())` and the test doesn't observe stdout (deliberately decoupled from formatting). If the developer prefers, use `assert_cmd` + `assert().stdout(predicates::str::contains("value"))` against the compiled binary — both approaches acceptable.

---

## Property Tests

None for TASK-0729 directly — decoder properties (encode→reduce→decode roundtrip for any valid input) are covered by TASK-0731 (PT optional) and TASK-0715 T11 (≥100 valid + ≥30 negative).

---

## Edge Cases Catalog

| #     | Scenario                                          | Expected Behavior                                                 | Test       |
|-------|---------------------------------------------------|-------------------------------------------------------------------|------------|
| EC-01 | Both `--codec` and `--encoder` passed             | clap `ArgumentConflict`                                            | UT-0729-02 |
| EC-02 | `--encoder horner` alias                          | parses identically to `--codec horner`                             | UT-0729-03 |
| EC-03 | Missing `--input`                                 | clap `MissingRequiredArgument`                                     | UT-0729-04 |
| EC-04 | Reduced `.bin` with `root = None`                 | `run_decode_command` invokes `discover_root` automatically         | IT-0729-06 |
| EC-05 | Un-reduced `.bin`                                 | `Err(NotNormalForm-style)`; **no panic**                           | IT-0729-07 |
| EC-06 | Unknown codec name                                | `Err(Config)` with helpful message                                 | IT-0729-09 |
| EC-07 | Corrupt `.bin` (random bytes)                     | `Err(_)` mentioning parse failure or path; **no panic**            | IT-0729-10 |
| EC-08 | Wrong codec for given net topology                | `Err(_)` (Encoding or Config); **no panic**                        | IT-0729-08 |
| EC-09 | `--output` absent                                 | stdout sink; no file created                                       | IT-0729-11 |
| EC-10 | `--output <path>` present                         | file written, contents = pretty JSON                               | IT-0729-05 |

---

## Quality Checklist

- [x] Every acceptance criterion (AC1..AC5) has at least one test.
- [x] CLI parser tests cover all 4 parser invariants (happy path, mutex, alias, required input).
- [x] Root recovery boundary (most critical for D-017 coordinator flow) covered.
- [x] All 4 error paths return `Err(_)` — no panic anywhere.
- [x] Boundary inputs explicit (corrupt bytes, unknown codec, un-reduced net, root=None).
- [x] Stdout vs file sink covered without coupling to formatting.
- [x] No test depends on another test's state.

## Suggested next agent

`developer` — start with config.rs (`DecodeArgs` + `Command::Decode`), then commands.rs (`run_decode_command`), then the dispatch arm in main/lib entrypoint. The clap unit tests guide the field layout; the integration tests validate the runtime contract.
