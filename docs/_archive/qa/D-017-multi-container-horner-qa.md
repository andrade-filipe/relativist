# QA Review: D-017 — Multi-container Horner distribution demo

**Stage:** 5 (QA, adversarial)
**Date:** 2026-05-16
**Commits reviewed:** `35aaef4` → `ab38da9` (7 commits, +3682 / -10)
**Files in scope:**
- `relativist-core/src/config.rs` (ComputeArgs.encode_only, DecodeArgs)
- `relativist-core/src/commands.rs` (run_compute_command, run_decode_command, run_compute_with_encoder)
- `relativist-cli/src/main.rs` (decode dispatch)
- `relativist-core/tests/compute_encode_only.rs` (6 IT)
- `relativist-core/tests/decode_subcommand.rs` (7 IT)
- `relativist-core/tests/horner_encode_decode_roundtrip.rs` (8 IT + 1 PT + 2 ignored)
- `scripts/horner_distributed_demo.sh` (314 lines, bash)
- `docker-compose.yml` (env-var patch)
- `docs/demos/live_demo.md` (multi-container chapter)

**Bug verdict:** **CLEAN of CRITICAL bugs** — 0 P0, 1 P1, 6 P2, 3 P3.
**Test coverage:** ADEQUATE for the new surface (`compute --encode-only`, `decode`), GAPS on script-level paths and on a pre-existing silent-drop in the registry path that is now adjacent to new behavior.
**Recommendation:** **safe-to-merge** — proceed to Stage 6 REFACTOR. The P1 finding is pre-existing (not introduced by D-017) but the new `--encode-only` flag makes the asymmetry surprising; capture as a follow-up TASK in next-steps.md rather than blocking the bundle.

---

## Methodology

Ran release binary `target/release/relativist.exe` (built fresh) against an adversarial CLI matrix on Windows + PowerShell. JSON strings escaped via `\"` to defeat PowerShell quote-eating. All exit codes verbatim from `$LASTEXITCODE`.

---

## Bugs found

### BUG-D017-001 — `compute --codec X --input ... --output Y.bin` silently drops `--output` in the registry path
**Severity:** P1 (asymmetry surprise; not a regression but newly load-bearing for D-017)
**File:** `relativist-core/src/commands.rs:795-860` (`run_compute_with_encoder`)
**Category:** Silent failure / contract gap
**Reproduction:**
```powershell
$bin compute --codec horner --input '{"coeffs":[10000,500,1],"x":100}' --output C:\Temp\reduced.bin
# Exit: 0
# Prints: "Result: { ... "value": "70000" ... }"
# Post-run: Test-Path C:\Temp\reduced.bin → False  (file NOT created)
```
**Verbatim output:**
```
=== Relativist Compute (encoder: horner) ===
Encoding:    21443 agents, 4 redexes
Reduction:   1220 interactions in 0.00s (19.03 MIPS)
Result:      { "bit_length": 17, "value": "70000" }
Post-run exit: 0
Post-run exists: False
```
**Expected:** Either (a) save the reduced net to `--output` after the in-process pipeline finishes, mirroring the `local`/`reduce`/`coordinator` subcommands which all honour `--output`; or (b) warn at parse time that `--output` is meaningful only with `--encode-only` in the registry path.
**Actual:** Flag accepted silently, no file written. Operator has no signal the artifact wasn't produced unless they explicitly check.
**Why D-017 escalates this:** TASK-0728 introduces `--encode-only` with `requires = "output"`, so the operator's mental model becomes "in the registry path, `--output` writes the .bin." It writes only in the encode-only branch; in the reduce+decode branch the flag is dropped. The `horner_distributed_demo.sh` script avoids the pitfall by always pairing `--encode-only` with `--output`, but a hand-typed `compute --codec horner --input ... --output foo.bin` will mislead.
**Pre-existing scope:** Pre-D-017 commit `35aaef4^` already lacked a save-to-output in `run_compute_with_encoder`, so this is NOT a regression introduced by the bundle.
**Fix suggestion:** Add either (1) post-decode `if let Some(p) = output { save_bin(&net, p)?; }` after line 857 of commands.rs, or (2) a clap-level mutually-exclusive warning, or (3) docs/runtime note that `--output` is encode-only in the registry path. Recommend (1) for least surprise.

---

### BUG-D017-002 — Wrong-codec decode of a Horner .bin produces a misleading error
**Severity:** P2
**File:** `relativist-core/src/commands.rs:903-929` (`run_decode_command`); error originates inside `church_add` decoder.
**Category:** Poor diagnostic
**Reproduction:**
```powershell
$bin decode --codec church_add --input C:\Temp\horner_reduced.bin
# Exit: 3
# error: encoding error: unrecognized net structure:
#   Church numeral readback failed (DUP cycle or malformed net)
```
**Expected:** Mention that the codec may not match the encoder used to produce the .bin (e.g., "codec `church_add` failed on this net — verify that the file was produced by the same codec").
**Actual:** Operator sees "DUP cycle or malformed net" and may chase a phantom data-corruption bug.
**Fix suggestion:** No code-level fix required for D-017 closure; document in `docs/demos/live_demo.md` troubleshooting section ("If you see 'DUP cycle' on decode, verify `--codec` matches the producing encoder.").

---

### BUG-D017-003 — encode-only on a constant polynomial silently bypasses the distributed reducer
**Severity:** P2 (false-positive risk for the demo)
**File:** `scripts/horner_distributed_demo.sh` (no validation that the input has redexes)
**Category:** Demo can pass without proving the distributed path
**Reproduction:**
```powershell
$bin compute --codec horner --input '{"coeffs":[42],"x":99}' --encode-only --output const.bin
# Output: "Encoding: 85 agents, 0 redexes"   ← already normal form
$bin decode --codec horner --input const.bin
# Exit: 0  →  { "value": "42" }
```
**Why it matters:** The demo script will encode → ship `.bin` to coordinator → coordinator does nothing (0 redexes) → ship back → decode returns 42 → G1 check passes ✓. The operator will believe the distributed pipeline ran, but no interaction happened in any worker. This sabotages the "atrás das cortinas" pedagogical goal.
**Expected:** Either (a) the demo script refuses constant polynomials with a banner ("constant polynomials are trivial — coordinator will have no work; use degree ≥ 1 to see distribution"); or (b) the in-process reference branch verifies `total_interactions > 0` and warns.
**Fix suggestion:** Add an envelope guard to `horner_distributed_demo.sh` after stage 1 that loads the .bin via a `relativist inspect` call and exits with a warning if `Redexes: 0`. Defer to a follow-up — not blocking.

---

### BUG-D017-004 — encode-only path performs encoding work BEFORE write-permission check
**Severity:** P2 (wasted work on misconfig)
**File:** `relativist-core/src/commands.rs:643-657` and `relativist-core/src/io/binary.rs:29`
**Category:** Wasted work / late failure
**Reproduction:**
```powershell
# Target = pre-existing read-only file
$bin compute --codec horner --input '{"coeffs":[1,2,3],"x":10}' --encode-only --output $ro
# Output:
#   === Relativist Compute (encoder: horner) ===
#   Encoding:    93 agents, 4 redexes
#   error: I/O error: Acesso negado. (os error 5)
# Exit: 2
```
**Expected:** Pre-flight `OpenOptions::create_new` or write-permission probe BEFORE encoding 93 agents. For envelope-max inputs (~21k agents), this matters for fast-fail UX.
**Fix suggestion:** Defer — minor; the error message is clear and OS error 5 is unambiguous.

---

### BUG-D017-005 — `WAIT_TIMEOUT_SECS` env-var lacks type validation in the script
**Severity:** P2
**File:** `scripts/horner_distributed_demo.sh:57, 246, 262`
**Category:** Hostile input
**Reproduction:**
```bash
WAIT_TIMEOUT_SECS=abc bash scripts/horner_distributed_demo.sh
# `timeout abc docker compose wait coordinator` → timeout(1) errors out
# Fallback polling enters `(( waited >= WAIT_TIMEOUT_SECS ))` → bash arith
#   syntax error → script exits non-zero with cryptic message under `set -e`.
```
**Expected:** Same regex check as `WORKERS` (`^[0-9]+$`) applied at the top.
**Fix suggestion:** Add after the WORKERS check:
```bash
if ! [[ "$WAIT_TIMEOUT_SECS" =~ ^[0-9]+$ ]]; then
    echo "ERROR: WAIT_TIMEOUT_SECS must be a non-negative integer, got: $WAIT_TIMEOUT_SECS" >&2
    exit 1
fi
```

---

### BUG-D017-006 — `--workers` accepts arbitrarily large values without sanity cap
**Severity:** P2
**File:** `scripts/horner_distributed_demo.sh:88`
**Category:** Resource exhaustion
**Reproduction:**
```bash
bash scripts/horner_distributed_demo.sh --workers 1000
# Regex ^[1-9][0-9]*$ accepts 1000.
# `docker compose up --scale worker=1000` will hammer Docker Desktop.
```
**Expected:** Warn or cap at e.g. 32 (CPU count × 4), with a `--force-workers` override.
**Fix suggestion:** Defer to a follow-up; the live-demo doc says "tested with 2 and 4" so audience usage is bounded.

---

### BUG-D017-007 — IT-0731-11 Docker smoke uses relative path that fails when cargo runs from `relativist-core/`
**Severity:** P2 (test infrastructure; mitigated by `#[ignore]`)
**File:** `relativist-core/tests/horner_encode_decode_roundtrip.rs:238-244`
**Category:** Test gate / cwd assumption
**Reproduction:**
```
cargo test -p relativist-core --test horner_encode_decode_roundtrip -- --ignored
# `Command::new("bash").arg("scripts/horner_distributed_demo.sh")`
# Cargo runs the test with cwd = relativist-core/
# bash: scripts/horner_distributed_demo.sh: No such file or directory
```
**Expected:** Resolve the script path via `env!("CARGO_MANIFEST_DIR")` joined to `../scripts/horner_distributed_demo.sh`, or `$CARGO_WORKSPACE_DIR`.
**Fix suggestion:**
```rust
let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("..").join("scripts").join("horner_distributed_demo.sh");
let output = std::process::Command::new("bash").arg(script).arg("--workers").arg("2").output()...
```
**Risk if unfixed:** When a maintainer runs `cargo test --ignored` to exercise the Docker smoke, it will fail with "No such file" rather than actually exercising the demo. The test is silently non-functional.

---

### BUG-D017-008 — Out-of-envelope inputs surface as opaque encode errors with no envelope hint
**Severity:** P3
**File:** `relativist-core/src/commands.rs:656` (encode failure propagation)
**Category:** Poor diagnostic
**Reproduction:**
```bash
relativist compute --codec horner --input '{"coeffs":[1,2,3,1],"x":100}' --encode-only --output x.bin
# Falls into HornerCodec envelope check; error mentions the constraint
# but the demo script catches it with: "ERROR: local encode failed."
# (commands.rs line 167 of the script.) No envelope hint to the operator.
```
**Expected:** Script's error message at stage 1 should include a one-liner reminder ("Envelope: coeffs.len() in {1,2,3}; if len==3, c2 must be 1; c1 in [0,1025]; c0 in [0,10000]; x in [0,100]").
**Fix suggestion:** Append envelope reminder to the `ERROR: local encode failed.` branch in the script.

---

### BUG-D017-009 — Per-second timestamp leaves a 1-second collision window for parallel demo runs
**Severity:** P3
**File:** `scripts/horner_distributed_demo.sh:126`
**Category:** Race condition (low likelihood)
**Reproduction:** Two operators starting `bash scripts/horner_distributed_demo.sh` within the same second from different shells collide on `data/horner_input_${TS}.bin`. Encode of run 2 overwrites run 1 before run 1's coordinator finishes loading.
**Fix suggestion:** Append `$$` (PID) or `$(date +%N | cut -c1-3)` (ms) to `TS`.

---

### BUG-D017-010 — Cleanup trap omits the encode-only and reduced .bin artifacts in `data/`
**Severity:** P3 (operational hygiene)
**File:** `scripts/horner_distributed_demo.sh:142-156`
**Category:** Disk-space leak
**Reproduction:** Each demo invocation leaves `data/horner_input_<ts>.bin` (~426 KB for envelope-max) and `data/horner_output_<ts>.bin` (~426 KB) behind. After 100 demos in a workshop, ~85 MB residual.
**Fix suggestion:** Either leave as-is (the timestamped naming is intentional for post-talk inspection per the docstring) and document the cleanup convention, or accept a `--cleanup-artifacts` flag.

---

## Edge cases verified clean (no bug)

- **EC-1 (A1.1):** `--encode-only` without `--output` → clap rejects with `MissingRequiredArgument` mentioning `--output`. Exit 2. PASS.
- **EC-2 (A1.4):** Constant polynomial `[42]` encodes to 85 agents, 0 redexes, 1029-byte .bin. Loads cleanly. PASS.
- **EC-3 (A1.6 + A1.7):** Full roundtrip `encode-only` → `reduce` → `decode` returns `{ "value": "70000", "bit_length": 17 }`, matching the in-process `compute` reference. PASS.
- **EC-4 (A2.1+A2.2):** Decoding an unreduced .bin returns `error: encoding error: net is not in normal form (has 4 valid active pair(s))`. Exit 3, no panic. PASS.
- **EC-5 (A2.3):** Decoding 15 bytes of garbage returns `error: configuration error: failed to deserialize <path>: ... UnexpectedVariant { type_name: "Option<T>" ...`. Exit 1, no panic. PASS.
- **EC-6 (A2.4):** Decoding an empty file returns `... UnexpectedEnd { additional: 1 }`. Exit 1, no panic. PASS.
- **EC-7 (A2.5):** Decoding `Z:\does_not_exist.bin` returns `error: I/O error: O sistema não pode encontrar o caminho especificado. (os error 3)`. Exit 2, no panic. PASS.
- **EC-8 (A2.8):** `decode --input X` without `--codec`/`--encoder` returns `error: configuration error: decode requires --codec or --encoder`. PASS.
- **EC-9 (A2.9):** `decode --codec horner` without `--input` → clap rejects. PASS.
- **EC-10 (UT-0729-02):** Passing both `--codec` and `--encoder` to `decode` → clap `ArgumentConflict`. PASS (covered by existing unit test).
- **EC-11 (A1.9):** `--output` pointing at a directory → I/O `Acesso negado. (os error 5)`. Exit 2. PASS.
- **EC-12 docker-compose env-var defaults:** Env vars `INPUT_PATH`/`OUTPUT_PATH`/`METRICS_PATH` default to the prior literal paths (`/data/input.bin`, etc.) via `${VAR:-default}`. Pre-D-017 `docker compose up coordinator worker` invocations preserve their semantics. PASS.

---

## Test coverage gaps

### TG-D017-01 — No automated coverage for the registry-path `--output` silent drop (BUG-D017-001)
**Suggested test:** Add to `compute_encode_only.rs`:
```rust
#[test]
fn compute_with_encoder_and_output_without_encode_only_writes_or_warns() {
    // Either save the reduced net OR document the no-op explicitly.
    let tmp = NamedTempFile::new().unwrap();
    let args = ComputeArgs { codec: Some("horner".into()), input: Some(...),
        output: Some(tmp.path().to_path_buf()), encode_only: false, ..};
    run_compute_command(args).unwrap();
    // Currently FAILS: file is not created. Decide on contract.
    assert!(tmp.path().metadata().unwrap().len() > 0,
        "operator expectation: --output must produce a file");
}
```

### TG-D017-02 — No coverage of `--workers 0` regex-rejection in the script
**Suggested test:** A bash-level smoke that invokes the script with `--workers 0`, `--workers -1`, `--workers abc` and asserts exit 1. Belongs in a future `tests/scripts_smoke.sh`.

### TG-D017-03 — IT-0731-11 silently broken under `#[ignore]` (BUG-D017-007)
**Suggested test:** Either fix the cwd (per BUG-D017-007 fix suggestion) or add a non-ignored unit test that asserts the script path resolves: `assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join("../scripts/horner_distributed_demo.sh").exists());`.

### TG-D017-04 — PT-0731-09 capped at 12 cases (style commit `1d53692`)
The cap is documented as a debug-build budget. Adequate for local dev. Recommend that CI runs the same property test with `--release` and an env-var override (e.g., `PROPTEST_CASES=256`) at least nightly to catch regressions the 12-case sample would miss.

---

## Stress scenarios (documented, not exercised)

- **SS-1: SIGINT mid-`compose up`** — The trap `cleanup() ... exit $rc` runs `docker compose stop || true`, but if `compose up` is itself mid-flight when Ctrl-C arrives, partial container creation may leave orphaned containers that `compose stop` cannot reach by service name. **Recommendation:** test manually before public demo.
- **SS-2: Port 9000 already bound** — Pre-existing process on host port 9000 causes `compose up coordinator` to fail with exit 1 from compose. Script's stage-3 branch catches it and exits 3 with `"ERROR: docker compose up coordinator failed."`. Operator gets a clear signal. **Adequate.**
- **SS-3: Worker connect-timeout** — If `--workers 4` is requested but no worker container ever ConnAccept's, coordinator hangs waiting. Script's `WAIT_TIMEOUT_SECS=600` covers this; on timeout the trap runs and `compose stop` brings everything down. **Adequate.**

---

## Closure

Reviewed all `unwrap()`/`expect()`/`panic!()` paths in new code: `relativist-core/src/commands.rs` adds 0 unwraps in `run_decode_command` and `run_compute_with_encoder` short-circuit. Test files use `unwrap()` extensively which is idiomatic and expected. The script uses `set -euo pipefail` with explicit `|| true` only on the teardown — correct posture.

**Verdict: safe-to-merge.** No P0/P1 newly introduced. Stage 6 REFACTOR can proceed; BUG-D017-001 (registry-path `--output` silent drop) and BUG-D017-007 (IT-0731-11 cwd) should be captured as TASK entries in `docs/next-steps.md` for a follow-up bundle.
