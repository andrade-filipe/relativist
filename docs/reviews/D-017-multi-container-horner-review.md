# Review: D-017 — Multi-container Horner distribution demo

**Files reviewed:**
- `relativist-core/src/config.rs` (+247 lines; `ComputeArgs.encode_only`, `DecodeArgs`, parser tests)
- `relativist-core/src/commands.rs` (+104 lines; `run_compute_with_encoder` short-circuit + `run_decode_command`)
- `relativist-cli/src/main.rs` (+1 line; dispatch wiring)
- `relativist-core/tests/compute_encode_only.rs` (6 default tests)
- `relativist-core/tests/decode_subcommand.rs` (7 default tests)
- `relativist-core/tests/horner_encode_decode_roundtrip.rs` (8 default + 1 proptest + 2 `#[ignore]`)
- `scripts/horner_distributed_demo.sh` (314 lines)
- `docker-compose.yml` (3-line env-var patch on `coordinator` command)
- `docs/demos/live_demo.md` (multi-container variation appended)

**Code quality verdict:** PASS WITH NOTES
**Architecture verdict:** ALIGNED
**Spec compliance:** SPEC-27 v3 R14'/R15'/R16'/R21 (decode subcommand mirrors `compute --codec` mutex + dual-form); SPEC-13 module boundaries respected (core stays pure; CLI dispatch in `relativist-cli/`); G1 invariant honored by both `discover_root` recovery (commands.rs:925) and the demo script's stage 6 cross-check.

---

## Must-Fix Issues

None. All changes are additive, parser-validated, and the legacy paths are preserved (verified by `IT-0728-10`, `IT-0728-09`).

---

## Should-Fix

### SF-001: `docker compose wait` is treated as a binary success signal but the fallback never triggers on a non-zero coordinator exit

**Category:** Code Quality / Demo Robustness
**File:** `scripts/horner_distributed_demo.sh:245-269`
**Problem:** Compose v2's `docker compose wait coordinator` returns the **service exit code**, not just a "did it finish" boolean. When the coordinator exits non-zero (e.g., worker handshake mismatch, encode validation error inside the container), the `if` branch is taken into the `else` path which then *re-polls* a container that has already exited — wasting `WAIT_TIMEOUT_SECS` seconds before the existence check on `$OUTPUT_HOST` (line 272) finally fires exit 4. The exit code from `wait` is also swallowed by `>/dev/null 2>&1`, so the operator never sees the real failure reason.
**Before:**
```bash
if MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        timeout "$WAIT_TIMEOUT_SECS" docker compose wait coordinator >/dev/null 2>&1; then
    :
else
    # Fallback: find the coordinator container ID and poll its state.
    ...
fi
```
**After (sketch):**
```bash
WAIT_RC=0
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
    timeout "$WAIT_TIMEOUT_SECS" docker compose wait coordinator > /tmp/coord_rc 2>/dev/null || WAIT_RC=$?
COORD_RC="$(cat /tmp/coord_rc 2>/dev/null || echo "$WAIT_RC")"
if [[ "$COORD_RC" != "0" ]]; then
    echo "ERROR: coordinator exited with code $COORD_RC; see \`docker logs relativist-coordinator-1\`." >&2
    exit 4
fi
```
**Why:** Surfaces the real failure mode before the file-existence heuristic obscures it; cuts the worst-case error path from `WAIT_TIMEOUT_SECS` (default 600 s) to immediate.

### SF-002: `COORD_CONTAINER=""` declared then unused on the happy path

**Category:** Code Quality (dead initialization)
**File:** `scripts/horner_distributed_demo.sh:243`
**Problem:** `COORD_CONTAINER=""` is assigned before the `if/else` but only ever reassigned and read inside the `else` block. Cleanly initialize where it is needed, or drop it. Minor, but `set -u` could trip on an unrelated future edit that reads the var on the happy path.
**Why:** Reduces noise; one variable, one purpose, one scope.

### SF-003: `docker inspect` call inside the fallback poll bypasses the MSYS path-conv guards used elsewhere

**Category:** Code Quality / Windows portability
**File:** `scripts/horner_distributed_demo.sh:258`
**Problem:** Every other Docker invocation in the script wraps with `MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'`; the polling `docker inspect -f '{{.State.Status}}' "$COORD_CONTAINER"` does not. The `{{.State.Status}}` format string contains characters Git-Bash sometimes mangles. Defensive consistency is cheap.
**Why:** Either the guards are necessary (then add it here) or they are not (then remove globally). Pick one.

### SF-004: `PT-0731-09` cap of 12 cases hides shrinking power when the demo regresses

**Category:** Testing quality
**File:** `relativist-core/tests/horner_encode_decode_roundtrip.rs:198-231`
**Problem:** The property test is configured with `cases: 12` (justified in the comment by ~60 s debug-build budget). That is fine as the default. But the test silently skips encoder rejections via `let Ok(mut a) = a else { return Ok(()); };` — with only 12 attempts, an envelope-tightening regression that rejects 100% of generated inputs would still pass green. Consider asserting at least one case actually decoded, or splitting into a `cfg(release)` variant with `cases: 256`.
**Before:**
```rust
let Ok(mut a) = a else { return Ok(()); };
```
**After:**
```rust
let Ok(mut a) = a else {
    // Telemetry: count rejections so a 100% rejection rate is visible.
    use std::sync::atomic::{AtomicUsize, Ordering};
    static REJECTED: AtomicUsize = AtomicUsize::new(0);
    REJECTED.fetch_add(1, Ordering::Relaxed);
    return Ok(());
};
```
…and a separate `#[test] fn proptest_actually_exercised_at_least_one_case()` reading the atomic at end of the run. Optional; the current shape is defensible for the bundle.
**Why:** Property tests with low case counts and silent skip-arms are a common source of "test passes because nothing was tested."

### SF-005: Magic exit codes in `horner_distributed_demo.sh` repeated across two surfaces without a single source of truth

**Category:** Code Quality (DRY)
**File:** `scripts/horner_distributed_demo.sh:34-41` (banner) and the `exit 2/3/4/5/6` literals scattered through the file.
**Problem:** If a future maintainer renumbers an exit code in the body but forgets the header (or vice versa), `IT-0731-11` would not catch it — the Docker smoke only checks `status.success()`. Consider `readonly EX_ENCODE=2 EX_DOCKER_UP=3 EX_COORD=4 EX_DECODE=5 EX_G1=6` at the top.
**Why:** The exit-code contract is part of the script's public surface (the header documents it); a single source of truth keeps it honest.

---

## Nice-to-Have

### NTH-001: `run_decode_command` uses `println!` while the rest of `commands.rs` mixes `println!` and `tracing::*`

**File:** `relativist-core/src/commands.rs:927-928`
**Suggestion:** The handler uses `println!` for the stdout JSON sink, which is correct (CLI output, not telemetry). The accompanying `tracing::debug!` on root-recovery is good. No change needed; flag only because the project CLAUDE.md says "no `println!` — use `tracing` macros only", and this is one of the explicit exceptions (user-facing CLI output). Worth noting in a comment so a future reviewer doesn't try to "fix" it.

### NTH-002: `DecodeArgs` lacks an `Args` builder helper analogous to the test fixture in `compute_encode_only.rs`

**File:** `relativist-core/tests/decode_subcommand.rs:43-48` (and duplicated several times)
**Suggestion:** The `DecodeArgs { codec: Some(...), encoder: None, input: ..., output: None }` literal recurs in 7 test bodies. A small `fn horner_decode_args(input: &Path, output: Option<&Path>) -> DecodeArgs` helper would tighten the file. Defer to test-generator preference.

### NTH-003: Backlog "Stage 3 closure record" commit conflates docs metadata with code

**File:** `docs/backlog/BACKLOG.md` (commit `ab38da9`)
**Suggestion:** Pure metadata bumps in the same series as code commits are easier to revert when isolated. The current bundle is fine, but flagging as a pattern note.

---

## Passed Checks

- [x] No `unwrap()` / `expect()` / `unreachable!()` in production paths under `relativist-core/src/`
- [x] Errors flow through `RelativistError::{Config,Encoding}` with operator-actionable messages (e.g., `commands.rs:649-657` and `:670-677`)
- [x] No `unsafe` introduced
- [x] No `println!` in non-CLI code (`run_decode_command` is the CLI handler; `println!` is appropriate there)
- [x] `thiserror`-based error variants; no `anyhow`
- [x] CLI flags use `conflicts_with` / `requires` to push validation into clap (`config.rs:622, 628` and `:589`)
- [x] Backwards compat: legacy `compute add 3 5` path still works (`IT-0728-09` exercises rejection of `--encode-only` on that path; nothing else changed in the legacy branch); `compute --codec X --input Y` without `--encode-only` still runs encode→reduce→decode (`IT-0728-10`)
- [x] `docker-compose.yml` env-var defaults preserve prior literals (`${INPUT_PATH:-/data/input.bin}` etc.), so `bench-tcp` / `bench-tcp-eager` profiles and existing stress-curve invocations are untouched
- [x] `run_decode_command` mirrors the `discover_root` recovery contract from `run_compute_with_encoder` (commands.rs:912-919 vs. :845-852) — single source of truth at the `discover_root` function, both call sites cite the contract in comments
- [x] SPEC-27 v3 R21 dual-form (`--codec` / `--encoder` mutex) replicated on `decode` (`config.rs:609-613`)
- [x] Module boundaries (SPEC-13): `decode` handler lives in `relativist-core/src/commands.rs`, calls into `crate::io::binary` and `crate::encoding`; no async/tokio leakage into the core
- [x] Test naming follows `IT-NNNN-NN` / `UT-NNNN-NN` / `PT-NNNN-NN` convention from `rust-tests-guidelines`
- [x] `#[ignore]` tests are documented in the file header and inline (`IT-0731-10`, `IT-0731-11`)
- [x] Script `horner_distributed_demo.sh` uses `set -euo pipefail`, lockfile-free but trap-based cleanup on `EXIT INT TERM`, MSYS path-conv guards on every Docker call (except SF-003), explicit exit codes
- [x] Docker logic in `IT-0731-11` is consistent with the script: invokes `scripts/horner_distributed_demo.sh --workers 2` and parses the first JSON line from stdout — matches the script's `[5/6] Decoding ...` stanza (`scripts/horner_distributed_demo.sh:281-286`)

---

## Cross-check: `IT-0731-11` ↔ `horner_distributed_demo.sh`

| Concern | Test expectation | Script behavior | Verdict |
|---|---|---|---|
| Exit code | `status.success()` (rc=0) | `exit 0` only on G1 OK + decode OK | OK |
| JSON line on stdout | `lines().find(\|l\| l.starts_with('{'))` | Stage 5 prints `$DIST_JSON` raw (line 286), which `decode` emits as pretty JSON; first line is `{` | OK |
| `value` field | `json.get("value").is_some()` | HornerCodec decode schema (SPEC-27 v3 R15') always emits `value` | OK |
| Side effects | Test does not clean up `data/horner_*_*` files | Script never deletes; intentional (operator inspects `data/`) | OK — known leak, documented in script header |

The `#[ignore]` rationale (no Docker daemon in CI) is sound; the local validation note in the script commit (`439c7c6` body: "Validated end-to-end with workers=2 ... decoded JSON `{\"bit_length\": 17, \"value\": \"70000\"}` matches the in-process reference") covers the gap.

---

## Notes on the proptest cap (`PT-0731-09`)

The TASK-0731 spec acknowledged a debug-build budget; capping at 12 cases is a reasonable trade-off documented inline (`horner_encode_decode_roundtrip.rs:194-197`). The shrink-noise concern in SF-004 is the only meaningful follow-up; defer to QA / a future hardening pass.

---

## Recommendation for parallel QA dispatch

**PROCEED in parallel.** The bundle is VERDE / GREEN at the code level: no production-side bugs, no spec drift, no broken backwards compat, all tests are designed to fail loudly. QA's adversarial pass should focus on:

1. Concurrent invocations of `horner_distributed_demo.sh` on the same host — the per-run `TS` filename suffix should isolate `.bin` files but `docker compose` uses the project name as singleton (two parallel runs will fight over the `coordinator` container).
2. `--encode-only --output /dev/null` (or platform equivalent) — does `save_bin` survive a "device or special file" output path, or does it silently truncate?
3. `decode --input <pipe>` — what happens when `--input` is a FIFO / non-seekable bincode source?
4. The `wait` ↔ fallback handoff in the demo script under a deliberately-killed coordinator container.

None of these are blocking for merge; they are QA fodder, not reviewer findings.
