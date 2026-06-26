# TEST-SPEC-TASK-0731: Tests for TASK-0731 — encode/decode roundtrip + multi-container smoke

**Task:** TASK-0731
**Spec:** SPEC-27 v3 R14'/R15'/R16' (decode contract); ARG-001 G1 (Fundamental Property — informally referenced by the Docker smoke)
**Bundle:** D-017
**Requirements covered:** AC1..AC6 of TASK-0731
**Invariants asserted:**
- SPEC-27 v3 R14'/R15' (decode schema stable across save/load).
- SPEC-12 bincode-v2 round-trip determinism.
- ARG-001 G1 (informal): distributed (TCP localhost smoke + Docker smoke) result equals in-process result.

---

## Scope

End-to-end tests covering the **combined behaviour** of TASK-0728 (`--encode-only`) and TASK-0729 (`decode` subcommand):

1. Roundtrip 1: `encode → reduce_all → decode` produces correct value (in-process baseline).
2. Roundtrip 2: `encode → save → load → reduce_all → discover_root → decode` produces the **same** value (validates that bincode persistence is semantically transparent).
3. Encode-only canary: `--encode-only` short-circuits before `reduce_all` (redex_queue still populated post-load).
4. Negative paths: corrupt `.bin`, unknown codec (these duplicate TASK-0729 IT-09/10 in part — keep them here as well to make the D-017 contract self-contained for the demo script).
5. CLI parser parity tests for `--encode-only` (requires `--output`) and `Decode` subcommand mutex.
6. **Distributed smokes:**
   - `#[ignore]` TCP-localhost smoke: spawn coordinator + 2 workers on `127.0.0.1` (no Docker), run reduction, assert decoded value matches in-process.
   - `#[ignore]` Docker smoke: shell out to `scripts/horner_distributed_demo.sh --workers 2`, assert exit 0 and a JSON line on stdout.

### Test category & location

| #            | Category                          | Cfg gating                | File                                                            | LoC |
|--------------|-----------------------------------|---------------------------|-----------------------------------------------------------------|-----|
| IT-0731-01   | integration (roundtrip)           | none                      | `relativist-core/tests/horner_encode_decode_roundtrip.rs` (new) | ~30 |
| IT-0731-02   | integration (roundtrip via .bin)  | none                      | same                                                             | ~35 |
| IT-0731-03   | integration (encode-only canary)  | none                      | same                                                             | ~25 |
| IT-0731-04   | integration (constant polynomial) | none                      | same                                                             | ~25 |
| IT-0731-05   | integration (degree-2 envelope)   | none                      | same                                                             | ~25 |
| IT-0731-06   | integration (corrupt .bin)        | none                      | same                                                             | ~20 |
| IT-0731-07   | integration (unknown codec)       | none                      | same                                                             | ~20 |
| UT-0731-08   | unit (clap parity)                | none                      | `relativist-core/src/config.rs` (`#[cfg(test)]`)                | ~20 |
| PT-0731-09   | property (optional)               | none                      | `relativist-core/tests/horner_encode_decode_roundtrip.rs`       | ~40 |
| IT-0731-10   | integration (TCP localhost smoke) | `#[ignore]` (manual/CI)   | same                                                             | ~80 |
| IT-0731-11   | integration (Docker smoke)        | `#[ignore]` (CI w/ Docker)| same                                                             | ~35 |

### Test floor delta

- default: **+9** (IT-0731-01..07 + UT-0731-08 + PT-0731-09; IT-0731-10/11 are `#[ignore]` and do NOT count). Adjust to **+8** if PT-0731-09 is dropped (optional per task scope).
- zero-copy: **+9**
- streaming-no-recycle: **+9**
- release: **+9**

---

## Shared test setup

```rust
// relativist-core/tests/horner_encode_decode_roundtrip.rs
use relativist_core::encoding::{default_registry, discover_root};
use relativist_core::io::binary::{load_bin, save_bin};
use relativist_core::reduction::reduce_all;
use serde_json::Value;
use tempfile::NamedTempFile;

// Reference input — matches `horner_live_demo.sh` working envelope (degree 2,
// coeffs and x small enough to reduce in <1s, large enough to exercise R13'
// composition non-trivially).
const HORNER_INPUT: &[u8] = br#"{"coeffs":[10000,500,1],"x":100}"#;

fn inproc_reference_json() -> Value {
    let reg = default_registry();
    let mut net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
    let _ = reduce_all(&mut net);
    if net.root.is_none() { discover_root(&mut net); }
    reg.decode("horner", &net).unwrap()
}
```

---

## Integration Tests

### IT-0731-01: `encode_reduce_decode_inproc_returns_expected_value`

**Function under test:** baseline — `EncoderRegistry::encode_and_validate` + `reduce_all` + `discover_root` + `EncoderRegistry::decode` (`relativist-core/src/encoding/registry.rs`, `reduction/mod.rs`, `encoding/horner.rs`).

**Purpose:** Establish the in-process JSON reference for all subsequent comparisons.

**Input:** `HORNER_INPUT` per shared setup.

**Expected output:**
```rust
let json = inproc_reference_json();
assert!(json.get("value").is_some(), "schema R15': missing 'value'");
assert!(json.get("bit_length").is_some(), "schema R15': missing 'bit_length'");
let value_str = json["value"].as_str().expect("value is decimal string");
// Horner eval of [10000,500,1] at x=100 = 1 + 500*100 + 10000*100^2 = 100050001? — actually
// the Horner encoding here is sum_i coeffs[i] * x^i, so:
//   coeffs[0] + coeffs[1]*x + coeffs[2]*x^2 = 10000 + 500*100 + 1*100^2 = 10000 + 50000 + 10000 = 70000
// The exact value depends on HornerCodec's coefficient ordering (TASK-0714);
// the test should NOT hardcode the expected integer — it should only assert
// `value_str.parse::<u64>().is_ok()` and use it as the cross-check reference
// for IT-0731-02.
assert!(value_str.parse::<u128>().is_ok(),
        "value must be a parseable decimal integer, got: {value_str}");
```

**Edge cases:** Test deliberately does NOT hardcode the numeric value — it asserts schema and parseability, then uses the same value as the oracle for IT-0731-02. This shields against the test ossifying around a particular Horner coefficient convention.

---

### IT-0731-02: `encode_save_load_reduce_decode_matches_inproc`

**Function under test:** the full D-017 pipeline — proves that `save_bin` + `load_bin` is semantically transparent across the reduction boundary.

**Purpose:** AC2 — round-trip through the file system produces identical JSON to the pure in-process path.

**Input:**
```rust
let reg = default_registry();
let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();

let tmp = NamedTempFile::new().unwrap();
save_bin(&net, tmp.path()).unwrap();

let mut loaded = load_bin(tmp.path()).unwrap();
let _ = reduce_all(&mut loaded);
if loaded.root.is_none() { discover_root(&mut loaded); }
let json_loaded = reg.decode("horner", &loaded).unwrap();

let json_ref = inproc_reference_json();
```

**Expected output:**
```rust
assert_eq!(json_loaded, json_ref,
           "save/load roundtrip must produce identical JSON to in-process pipeline");
```

**Edge cases:** This is the **single most important test** of D-017 — it certifies the coordinator pipeline is correct because `save_bin → coordinator → load_bin → reduce → decode` is exactly this test with a TCP hop in the middle. If this passes and TCP framing is correct (already covered by SPEC-06 tests), G1 follows.

---

### IT-0731-03: `encode_only_bin_preserves_redex_queue`

**Function under test:** `EncoderRegistry::encode_and_validate` followed by `save_bin`/`load_bin`. Canary that the `--encode-only` CLI path (TASK-0728) really skips `reduce_all`.

**Purpose:** AC3 — verifies that a freshly-encoded (un-reduced) Horner net retains its redex queue across `save_bin → load_bin`.

**Input:**
```rust
let reg = default_registry();
let net = reg.encode_and_validate("horner", HORNER_INPUT).unwrap();
assert!(!net.redex_queue.is_empty(),
        "encoded net must have redexes pre-reduce (canary precondition)");

let tmp = NamedTempFile::new().unwrap();
save_bin(&net, tmp.path()).unwrap();
let loaded = load_bin(tmp.path()).unwrap();
```

**Expected output:**
```rust
assert_eq!(loaded.redex_queue.len(), net.redex_queue.len(),
           "save/load must preserve redex_queue length");
assert_eq!(loaded.count_live_agents(), net.count_live_agents(),
           "save/load must preserve live agent count");
```

**Edge cases:** If TASK-0728's developer accidentally calls `reduce_all` somewhere along the encode-only path, this would fail with `loaded.redex_queue.len() == 0` — clear and direct signal.

---

### IT-0731-04: `constant_polynomial_roundtrip_does_not_panic`

**Function under test:** full encode→save→load→reduce→decode pipeline with edge-case input `coeffs:[42]`.

**Purpose:** Boundary — degree-0 polynomial. HornerCodec must produce a valid net even when there are no Horner steps (just a Church-numeral encoding of 42). Save/load/decode must complete without panic.

**Input:** `input = br#"{"coeffs":[42],"x":7}"#`

**Expected output:**
```rust
let reg = default_registry();
let net = reg.encode_and_validate("horner", input).unwrap();
let tmp = NamedTempFile::new().unwrap();
save_bin(&net, tmp.path()).unwrap();
let mut loaded = load_bin(tmp.path()).unwrap();
let _ = reduce_all(&mut loaded);
if loaded.root.is_none() { discover_root(&mut loaded); }
let json = reg.decode("horner", &loaded).expect("constant polynomial must decode");
let v: u64 = json["value"].as_str().unwrap().parse().unwrap();
assert_eq!(v, 42, "constant polynomial coeffs=[42] must decode to 42 regardless of x");
```

**Edge cases:** This is **the** test that catches "off-by-one in `coeffs[1..]`" bugs in the encoder. The value is unambiguous (must be 42, independent of x) — safe to hardcode.

---

### IT-0731-05: `degree_2_envelope_roundtrip_matches_inproc`

**Function under test:** same as IT-0731-02 but with the maximum-envelope input.

**Purpose:** Boundary at the working envelope ceiling. Demonstrates that save/load is robust at the largest-allowed inputs in `horner_live_demo.sh`.

**Input:** `HORNER_INPUT` (same as IT-0731-02 — already the envelope max per task notes). If a larger envelope is desired here, use `r#"{"coeffs":[1,1,1],"x":1000}"#` (still within reduce-time budget).

**Expected output:** identical assertion to IT-0731-02 (`json_loaded == json_ref`).

**Edge cases:** Mainly defends against memory-pressure regressions in `save_bin`/`load_bin` on larger nets.

---

### IT-0731-06: `decode_corrupt_bin_returns_error_no_panic`

**Function under test:** `run_decode_command` (TASK-0729) error path on garbage input.

**Purpose:** Duplicates IT-0729-10 in the D-017 contract suite so that the demo script can rely on a stable error contract.

**Input:**
```rust
let tmp = NamedTempFile::new().unwrap();
std::fs::write(tmp.path(), b"corrupt").unwrap();
let args = relativist_core::config::DecodeArgs {
    codec: Some("horner".to_string()),
    encoder: None,
    input: tmp.path().to_path_buf(),
    output: None,
};
let res = relativist_core::commands::run_decode_command(args);
```

**Expected output:** `assert!(res.is_err(), "corrupt .bin must error, not panic");`

---

### IT-0731-07: `decode_unknown_codec_returns_config_error`

**Function under test:** `run_decode_command` codec dispatch error path.

**Purpose:** Duplicates IT-0729-09 in the D-017 contract suite (same rationale as IT-0731-06).

**Input:**
```rust
let bin = /* same as IT-0731-02 setup, reduced .bin */;
let args = relativist_core::config::DecodeArgs {
    codec: Some("does_not_exist".to_string()),
    encoder: None,
    input: bin.path().to_path_buf(),
    output: None,
};
let res = relativist_core::commands::run_decode_command(args);
```

**Expected output:**
```rust
match res {
    Err(relativist_core::error::RelativistError::Config(_)) => {} // ok
    other => panic!("expected Config error for unknown codec, got {other:?}"),
}
```

---

## Unit Tests (CLI parity)

### UT-0731-08: `cli_parity_encode_only_and_decode_subcommand`

**Function under test:** `Cli::try_parse_from` for both `compute --encode-only ...` and `decode ...`. Single combined parity test (the per-flag granular tests live in TEST-SPEC-0728 and TEST-SPEC-0729).

**Purpose:** AC5 — assert that the two new CLI surfaces co-exist and parse correctly back-to-back. Guards against accidental clap conflict between the new flag and the new subcommand.

**Input:**
```rust
let a = Cli::try_parse_from([
    "relativist", "compute", "--codec", "horner",
    "--input", r#"{"coeffs":[1,2,3],"x":10}"#,
    "--encode-only", "--output", "/tmp/x.bin",
]).expect("encode-only path parses");

let b = Cli::try_parse_from([
    "relativist", "decode", "--codec", "horner", "--input", "/tmp/x.bin",
]).expect("decode subcommand parses");
```

**Expected output:**
```rust
assert!(matches!(a.command, Command::Compute(ref args) if args.encode_only));
assert!(matches!(b.command, Command::Decode(_)));
```

---

## Property Tests (optional)

### PT-0731-09: `horner_save_load_roundtrip_property` (optional)

**Function under test:** `save_bin` + `load_bin` semantic transparency for any HornerCodec input within the envelope.

**Purpose:** For all `coeffs ∈ [0, 10000]^k`, `k ∈ {1, 2, 3}`, `x ∈ [0, 100]`: `encode(input) → save → load → reduce → decode == encode(input) → reduce → decode`.

**Generator strategy (proptest):**
```rust
fn arb_horner_input() -> impl Strategy<Value = String> {
    (1usize..=3, 0u64..=100).prop_flat_map(|(k, x)| {
        prop::collection::vec(0u64..=10_000, k..=k)
            .prop_map(move |coeffs| {
                format!(r#"{{"coeffs":{:?},"x":{}}}"#, coeffs, x)
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 50, .. ProptestConfig::default() })]
    #[test]
    fn save_load_roundtrip_preserves_decoded_value(input in arb_horner_input()) {
        let reg = default_registry();
        let mut a = reg.encode_and_validate("horner", input.as_bytes()).unwrap();
        let _ = reduce_all(&mut a);
        if a.root.is_none() { discover_root(&mut a); }
        let json_a = reg.decode("horner", &a).unwrap();

        let net = reg.encode_and_validate("horner", input.as_bytes()).unwrap();
        let tmp = NamedTempFile::new().unwrap();
        save_bin(&net, tmp.path()).unwrap();
        let mut b = load_bin(tmp.path()).unwrap();
        let _ = reduce_all(&mut b);
        if b.root.is_none() { discover_root(&mut b); }
        let json_b = reg.decode("horner", &b).unwrap();

        prop_assert_eq!(json_a, json_b);
    }
}
```

**Shrinking note:** Counter-example would be a `coeffs`/`x` pair where save/load alters reduction semantics — minimal shrink would isolate the smallest such input. None expected.

**Optional:** Skip if developer judges PT to be redundant with IT-0731-02 over the canonical envelope input.

---

## Distributed smokes (ignored by default)

### IT-0731-10: `tcp_localhost_distributed_matches_inproc` (`#[ignore]`)

**Function under test:** end-to-end TCP path on `127.0.0.1` — spawn coordinator + 2 worker tasks in the same process, run the reduction, decode, compare to in-process baseline. **No Docker, no shell-out.**

**Purpose:** Demonstrates ARG-001 G1 for the simplest distributed configuration before adding the Docker layer. Catches regressions in `protocol/` framing without requiring a Docker daemon.

**Cfg:** `#[ignore = "TCP localhost smoke; run with --ignored"]`.

**Input:** `HORNER_INPUT` again.

**Skeleton (developer adapts to actual coordinator/worker entrypoints in `protocol/` and `coordinator/`):**
```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "TCP localhost smoke; run with --ignored"]
async fn tcp_localhost_distributed_matches_inproc() {
    // 1. Spawn 2 worker tasks bound to 127.0.0.1:<ephemeral>.
    // 2. Spawn coordinator task bound to 127.0.0.1:<ephemeral>, configured
    //    with the worker addrs.
    // 3. Coordinator loads the encode-only .bin we produce via save_bin().
    // 4. Drive the BSP cycle to completion via the existing run_grid path.
    // 5. After coordinator finishes, save_bin the reduced net to a tmp file.
    // 6. load_bin + discover_root + decode -> JSON.
    // 7. assert_eq!(json_tcp, inproc_reference_json()).

    // If the in-process distributed harness from
    // `tests/horner_distributed_g1.rs` (TASK-0715) already provides this,
    // reuse it and just thread the encode-only .bin through.
}
```

**Expected output:** `assert_eq!(json_tcp, inproc_reference_json())`.

**Edge cases:** Port-binding failures must surface as test failures, not panics that crash the runner. Use `0` for ephemeral ports and read back the actual bound port.

**Note:** If implementation is non-trivial, developer MAY mark this `#[ignore = "deferred to D-018"]` and document the gap in `docs/next-steps.md`. The Docker smoke (IT-0731-11) covers the same contract more thoroughly, so this test is a nice-to-have for fast local iteration.

---

### IT-0731-11: `multi_container_horner_e2e_docker` (`#[ignore]`)

**Function under test:** the bash script `scripts/horner_distributed_demo.sh` produced by TASK-0730. Asserts exit 0 and a JSON line on stdout with a `value` field.

**Purpose:** AC6 — end-to-end Docker smoke. Confirms TASK-0728 (`--encode-only`) + TASK-0729 (`decode`) + TASK-0730 (script + compose) compose correctly across a real Docker network.

**Cfg:** `#[ignore = "requires Docker + built release binary; run with --ignored"]`.

**Skeleton:**
```rust
#[test]
#[ignore = "requires Docker + built release binary; run with --ignored"]
fn multi_container_horner_e2e_docker() {
    let output = std::process::Command::new("bash")
        .arg("scripts/horner_distributed_demo.sh")
        .arg("--workers").arg("2")
        .output()
        .expect("invoke demo script");
    assert!(
        output.status.success(),
        "demo script must exit 0; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_line = stdout
        .lines()
        .find(|l| l.trim_start().starts_with('{'))
        .expect("stdout must contain a JSON line from `decode`");
    let json: Value = serde_json::from_str(json_line)
        .expect("decoded JSON must parse");
    assert!(json.get("value").is_some(), "decoded JSON must have 'value' field");
}
```

**Expected output:** `output.status.success() && json.value.is_some()`.

**Edge cases:**
- If `docker` is not installed, the test should error fast (`output()` returns `Err`). Acceptable to let that surface as a test failure with `--ignored` — the operator opted in.
- If `--workers 2` is not yet supported by the script, this test must be added when TASK-0730 completes.

---

## Edge Cases Catalog

| #     | Scenario                                              | Expected Behavior                                                  | Test         |
|-------|-------------------------------------------------------|--------------------------------------------------------------------|--------------|
| EC-01 | In-process baseline                                   | JSON has `value` (parseable int) + `bit_length` fields              | IT-0731-01   |
| EC-02 | encode → save → load → reduce → decode                | JSON identical to in-process baseline                               | IT-0731-02   |
| EC-03 | encode-only short-circuit                             | Loaded net has non-empty `redex_queue` (NOT reduced)                | IT-0731-03   |
| EC-04 | Constant polynomial `coeffs:[42]`                     | Decodes to exactly 42 regardless of x                                | IT-0731-04   |
| EC-05 | Degree-2 envelope-max input                           | JSON identical to in-process baseline                               | IT-0731-05   |
| EC-06 | Corrupt `.bin` to `decode`                            | `Err(_)`, no panic                                                  | IT-0731-06   |
| EC-07 | Unknown codec name                                    | `Err(Config)`, no panic                                              | IT-0731-07   |
| EC-08 | CLI parity (compute --encode-only + decode coexist)   | Both parse cleanly                                                   | UT-0731-08   |
| EC-09 | Property: any envelope input roundtrips losslessly    | `prop_assert_eq!(json_a, json_b)` for 50 cases                       | PT-0731-09   |
| EC-10 | TCP localhost coordinator + 2 workers                 | Decoded JSON equals in-process baseline                              | IT-0731-10   |
| EC-11 | Docker multi-container demo                           | Script exits 0, stdout has JSON with `value`                         | IT-0731-11   |

---

## Quality Checklist

- [x] Every acceptance criterion (AC1..AC6) has at least one test.
- [x] Constant-polynomial boundary (EC-04) hardcodes the unambiguous expected value (42).
- [x] All other tests cross-check against in-process baseline (no ossified numeric oracles).
- [x] Encode-only canary (IT-0731-03) precondition asserted (`redex_queue` non-empty pre-save).
- [x] Both negative paths (corrupt, unknown codec) covered without panic.
- [x] Distributed smokes correctly `#[ignore]`-gated; default `cargo test` stays green without Docker.
- [x] Property test optional and clearly scoped within the working envelope.
- [x] No test depends on another test's state (each tmp file is local).

## Notes for the developer

- **Reuse `tests/horner_distributed_g1.rs` helpers if available** (per TASK-0731 Notes line 118). The G1 in-process harness from TASK-0715 IT-0715-08 likely has the partition/coordinator/worker setup boilerplate the TCP smoke (IT-0731-10) needs.
- **Cargo.toml audit:** `tempfile` and `proptest` should already be dev-deps (used by TASK-0715/0725). Verify before starting; if missing, add to `[dev-dependencies]` per the existing pattern.
- **HORNER_INPUT determinism:** the chosen input has been stable in the codebase since D-015. If a later refactor changes the working envelope, update `HORNER_INPUT` here and in `horner_live_demo.sh` in lockstep.
- **Constant polynomial expected value (42):** the assertion `v == 42` for `coeffs:[42]` IS load-bearing — it is the only test in D-017 that hardcodes a specific decoded integer. If this fails, HornerCodec's degree-0 handling is broken regardless of what other roundtrip tests say.

## Suggested next agent

`developer` — start once TASK-0728 + TASK-0729 are GREEN, since IT-0731-06/07 + UT-0731-08 + IT-0731-10/11 depend on the new CLI surfaces compiling. IT-0731-01..05 can land standalone (they use the library API directly, not the CLI surfaces).
