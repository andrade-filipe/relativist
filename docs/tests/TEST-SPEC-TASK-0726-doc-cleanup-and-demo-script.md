# TEST-SPEC-TASK-0726: Tests for TASK-0726 — doc cleanup + `scripts/horner_demo.sh` smoke gate

**Task:** TASK-0726
**Spec:** SPEC-27 v3 — public-facing narrative
**Bundle:** D-016 — HornerCodec decoder extension
**Requirements covered:** N/A — doc + tooling housekeeping. TEST-SPEC exists for the OPTIONAL smoke-gate test only.
**Test IDs (from SPEC-27 v3 §7.3):** none directly; IT-0726-01 is a CI smoke gate over the end-to-end demo set.
**Production code under test:** `scripts/horner_demo.sh` (NEW, Bash) — invoked as a subprocess.

---

## Scope

TASK-0726 is doc + Bash housekeeping. Per TASK-0726 acceptance criteria, the test-generator MAY skip this TASK entirely OR optionally add ONE `#[ignore]`-by-default integration smoke test that invokes `scripts/horner_demo.sh` from `std::process::Command`. This TEST-SPEC emits **only IT-0726-01**, marked `#[ignore]` so it does not run in the default `cargo test` and does not affect the test floor.

Manual verification by the operator running `bash scripts/horner_demo.sh` directly is the primary acceptance for TASK-0726. IT-0726-01 is a secondary safety net for CI use (`cargo test -- --ignored`).

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| IT-0726-01 | integration (subprocess smoke) | `#[ignore]` (default), `cfg(unix)` | `relativist-core/tests/horner_demo_script_smoke.rs` **(CREATE)** | ~30 |

## Test floor delta (from TASK-0726 acceptance criteria)

- default: **0 net delta** — IT-0726-01 is `#[ignore]` and does NOT count toward the default test floor.
- `cargo test -- --ignored` count: +1.

---

## Integration Tests

### IT-0726-01: `horner_demo_script_smoke_full_run` (`#[ignore]` by default)

**Purpose:** Run `scripts/horner_demo.sh` as a subprocess; assert exit code 0 and that the stdout contains all 10 expected `"value"` strings.

**Preconditions:**
- `target/release/relativist` binary built (`cargo build --release`).
- `bash` available on PATH (script uses `set -euo pipefail`).
- The CI runner is Unix-like (gate via `cfg(unix)`). Windows runners SKIP this test (the script is Bash-only per TASK-0726 design).
- TASK-0726's `scripts/horner_demo.sh` exists at the repo root.

**Input:**
```rust
#![cfg(unix)]
use std::process::Command;
use std::time::Duration;

#[test]
#[ignore = "subprocess smoke test, run via `cargo test -- --ignored` (wall-time ~30-60s)"]
fn horner_demo_script_smoke_full_run() {
    // Locate the repo root from CARGO_MANIFEST_DIR (`relativist-core/`).
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("relativist-core has a parent directory (repo root)");
    let script = repo_root.join("scripts").join("horner_demo.sh");
    assert!(
        script.exists(),
        "scripts/horner_demo.sh must exist (TASK-0726): {}",
        script.display()
    );

    let output = Command::new("bash")
        .arg(script)
        .current_dir(repo_root)
        .output()
        .expect("failed to spawn `bash scripts/horner_demo.sh`");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "horner_demo.sh exited non-zero ({:?}); stdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );

    // Spot-check: stdout MUST mention each of the 10 expected output values
    // (3 newly-fixed cases from D-016 + 7 pre-existing Demo entries).
    // NOTE: substring match is intentional — script formatting may evolve.
    let expected_values = [
        "23",                          // Demo 8: [3,5]@4
        "7",                           // Demo 9: [1,1,1]@2
        "10",                          // Demo 10: [1,0,1]@3
        "35",                          // Demo 3: [3,2,5,1]@2 canonical
        "100001",                      // Demo 4: [1,0,0,0,0,1]@10 sparse
        "1111111111111111111111111",   // Demo 5: [1; 25]@10 T9 witness
    ];
    for expected in &expected_values {
        assert!(
            stdout.contains(expected),
            "horner_demo.sh stdout MUST contain {expected} — got:\n{stdout}"
        );
    }
}
```

**Expected output:**
- Exit code 0.
- Stdout contains every spot-checked decimal value.

**Edge cases:**
- (EC-1) On Windows runners (no `cfg(unix)`), the test is excluded at compile time. Documented in the `#![cfg(unix)]` attribute.
- (EC-2) If `cargo build --release` was not run, the script will fail when invoking `target/release/relativist`. The test will fail with a clear stderr message; the developer reading the failure understands they must build first. (Optional: add a `Command::new("cargo").args(["build", "--release"]).status()` invocation at the start, but this slows the test by ~30-90 s; not required.)
- (EC-3) The full demo set runs ~30-60 s wall-time. `#[ignore]` keeps this out of the default `cargo test` loop; only `cargo test -- --ignored` triggers it.
- (EC-4) The spot-check list intentionally covers only 6 of the 10 expected values — the remaining 4 (`Demo 1`, `Demo 2`, `Demo 6`, `Demo 7` per the existing doc) have shared / overlapping value strings that could false-positive the substring check. The 6 chosen here have unique values that uniquely identify their demos.

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Default `cargo test` | IT-0726-01 SKIPPED (`#[ignore]`) | IT-0726-01 |
| EC-002 | `cargo test -- --ignored` | IT-0726-01 RUNS; exit 0; all spot-checks pass | IT-0726-01 |
| EC-003 | Windows runner | IT-0726-01 EXCLUDED at compile time (`cfg(unix)`) | IT-0726-01 |
| EC-004 | `target/release/relativist` missing | IT-0726-01 FAILS with clear stderr | IT-0726-01 |
| EC-005 | One demo decode regresses (e.g., `[3,5]@4` returns wrong value) | IT-0726-01 FAILS with the missing-substring assertion | IT-0726-01 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| N/A | TASK-0726 has no SPEC-27 test obligations; IT-0726-01 is a CI smoke gate added at the test-generator's discretion. |

## Dependencies Context

- `scripts/horner_demo.sh` (NEW, created by TASK-0726 itself).
- `target/release/relativist` binary.
- Bash + `grep` (or `jq` if the script uses it — script must fall back per TASK-0726 Notes).
- TASK-0723 + TASK-0724 + TASK-0725 production code in HEAD (otherwise the demos would fail).

## Notes

- This TEST-SPEC is intentionally minimal. TASK-0726 is doc + Bash housekeeping; the regression safety nets for the decoder fixes themselves live in TEST-SPEC-0723 / -0724 / -0725. IT-0726-01 catches **packaging / script-level** regressions only (e.g., the script's grep pattern silently matches the wrong value).
- The `#[ignore]` attribute is REQUIRED — the test invokes a 30-60 s subprocess and would dominate `cargo test` wall-time if run by default.
- The `cfg(unix)` gate is REQUIRED — the script is Bash and uses `set -euo pipefail`, `timeout`, and POSIX tools. The (future) Docker arm tracked as TASK-0727 (per TASK-0726 BACKLOG entry) is the Windows-compatible path.
- Test floor delta: **0** for default `cargo test`. `cargo test -- --ignored` gains 1.
- Surprising edge case for the developer: **the script's `grep` for value strings may need to handle JSON-quoted values** (e.g., `"value":"23"` vs `"value": "23"`). The test's substring check on the raw stdout sidesteps this, but if the script formats its OWN output (separate from the relativist binary's), the developer should ensure each decoded value is echoed plainly to stdout for the assertion to match.
- If the developer chooses NOT to author this test (per TASK-0726 AC "MAY SKIP entirely"), they should document the deferral in the TASK-0726 commit message and reference TEST-SPEC-0723/0724/0725 as the primary safety nets. In that case the file `relativist-core/tests/horner_demo_script_smoke.rs` is NOT created.
