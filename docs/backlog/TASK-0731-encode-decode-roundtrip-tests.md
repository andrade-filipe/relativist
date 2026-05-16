# TASK-0731: Integration tests — encode-only / decode roundtrip + multi-container smoke

**Spec:** SPEC-27 v3 R14'/R15'/R16' (decode contract)
**Bundle:** D-017
**Priority:** P1 (covers TASK-0728 + TASK-0729 + TASK-0730)
**Status:** TODO
**Depends on:** TASK-0728, TASK-0729 (compile-time); TASK-0730 (for the `#[ignore]` Docker smoke)
**Blocked by:** TASK-0728, TASK-0729
**Estimated complexity:** S–M (~80 LoC test code)

## Context

Cover the new CLI surfaces introduced by TASK-0728/0729 with integration tests in `relativist-core/tests/`, plus an opt-in (`#[ignore]`) end-to-end smoke that drives the multi-container demo script (TASK-0730). The non-Docker tests run in default CI; the Docker smoke is gated behind `--ignored` (CI runs it conditionally, locally it documents the contract).

## Acceptance Criteria

- [ ] New test file `relativist-core/tests/horner_encode_decode_roundtrip.rs` exists.
- [ ] Round-trip test: encode-only Horner input → `save_bin` → `load_bin` → `discover_root` → `registry.decode` → JSON equals the JSON from `run_compute_with_encoder` for the same input. Uses `tempfile::NamedTempFile` for the `.bin`.
- [ ] Encode-only short-circuits: assert that an encode-only run produces a non-empty `.bin` with `count_live_agents() > 0` AND does NOT call `reduce_all` (assert by checking that the loaded net still has `redex_queue.len() > 0` for inputs known to have redexes pre-reduction).
- [ ] Decode subcommand error paths: corrupt `.bin` → Config error; unknown codec → Config error.
- [ ] CLI parser tests for `--encode-only` (requires `--output`) and `Decode` subcommand (codec/encoder mutex).
- [ ] (Ignored) Docker smoke test `multi_container_horner_e2e` that shells out to `scripts/horner_distributed_demo.sh --workers 2` and asserts exit 0 + a non-empty JSON on stdout. Marked `#[ignore]` so default `cargo test` skips it.

## Files to Create/Modify

- `relativist-core/tests/horner_encode_decode_roundtrip.rs` — **new file**, ~80 LoC.
- Possibly extend `relativist-core/src/config.rs` `#[cfg(test)]` block with the new clap parse tests (some of these are unit tests close to the args; place there for cohesion).

## Key Test Skeletons

```rust
// tests/horner_encode_decode_roundtrip.rs
use relativist_core::encoding::{default_registry, discover_root};
use relativist_core::io::binary::{load_bin, save_bin};
use serde_json::Value;
use tempfile::NamedTempFile;

const INPUT: &[u8] = br#"{"coeffs":[10000,500,1],"x":100}"#;

#[test]
fn encode_then_load_then_decode_matches_inproc() {
    let registry = default_registry();
    let net = registry.encode_and_validate("horner", INPUT).unwrap();

    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();

    let mut loaded = load_bin(tmp.path()).unwrap();
    // The just-encoded net has unreduced redexes — to compare against the
    // run_compute_with_encoder happy-path we must reduce here too.
    let _ = relativist_core::reduction::reduce_all(&mut loaded);
    if loaded.root.is_none() {
        discover_root(&mut loaded);
    }
    let json_loaded = registry.decode("horner", &loaded).unwrap();

    // Reference: full in-process path.
    let mut reference = registry.encode_and_validate("horner", INPUT).unwrap();
    let _ = relativist_core::reduction::reduce_all(&mut reference);
    if reference.root.is_none() {
        discover_root(&mut reference);
    }
    let json_ref = registry.decode("horner", &reference).unwrap();

    assert_eq!(json_loaded, json_ref);
}

#[test]
fn encode_only_bin_preserves_redex_queue() {
    let registry = default_registry();
    let net = registry.encode_and_validate("horner", INPUT).unwrap();
    assert!(!net.redex_queue.is_empty(), "encoded net must have redexes pre-reduce");
    let tmp = NamedTempFile::new().unwrap();
    save_bin(&net, tmp.path()).unwrap();
    let loaded = load_bin(tmp.path()).unwrap();
    assert_eq!(loaded.redex_queue.len(), net.redex_queue.len());
    assert_eq!(loaded.count_live_agents(), net.count_live_agents());
}

#[test]
#[ignore = "requires Docker + built release binary; run with --ignored"]
fn multi_container_horner_e2e() {
    let output = std::process::Command::new("bash")
        .arg("scripts/horner_distributed_demo.sh")
        .arg("--workers").arg("2")
        .output()
        .expect("script invocation");
    assert!(output.status.success(),
        "demo script failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: Value = serde_json::from_str(
        stdout.lines().find(|l| l.trim_start().starts_with('{')).unwrap()
    ).expect("stdout has decoded JSON");
    assert!(json.get("value").is_some(), "decoded JSON has 'value' field");
}
```

(Test bodies above are illustrative; test-generator should produce the canonical version under D-017.)

## Test Expectations (for test-generator)

- Use `tempfile` (already a dev-dep — verify in `Cargo.toml`; if missing, request the developer to add it).
- Reference inputs MUST stay inside the working envelope (see `horner_live_demo.sh` lines 38–41 doc-comment).
- The Docker smoke MUST be `#[ignore]` to keep `cargo test` green on CI runners without Docker.

## Dependencies Context

- `default_registry`, `encode_and_validate`, `decode` come from TASK-0716 / TASK-0715.
- `discover_root` from `encoding` module (existing, used in `commands.rs:798`).
- `reduce_all` from `reduction` module.

## Notes

- The `encode_only_bin_preserves_redex_queue` test is the most important — it's the canary that `--encode-only` actually short-circuits before `reduce_all`.
- The Docker smoke duplicates what `horner_demo.sh` does today but for the multi-container path. Don't over-engineer; one positive-case assertion is enough.
- If `relativist-core/tests/horner_distributed_g1.rs` exists (mentioned in commands.rs comment line 795), audit it first — there may be a base helper to reuse.
