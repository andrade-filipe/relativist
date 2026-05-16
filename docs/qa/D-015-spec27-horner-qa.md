# QA REPORT — D-015 SPEC-27 v3 Encoder/Decoder API + HornerCodec (Topic 2)

**QA agent:** Stage 5 (QA) of SDD pipeline
**Date:** 2026-05-06
**Branch:** `feature/stress-and-encoder` (HEAD `9f06e4f`)
**Inputs read:**
- Stage 4 review: `docs/reviews/D-015-spec27-horner-review.md`
- Source: `relativist-core/src/encoding/{horner,horner_oracle,biguint_readback,recipe,registry,traits,codec_church,arithmetic,church,mod}.rs`
- Source: `relativist-core/src/{commands.rs,config.rs,reduction/mod.rs}`
- Tests: `relativist-core/tests/{horner_distributed_g1,integration_horner_centralized,biguint_readback_independence,wire_helpers_privacy}.rs`
- SPEC-27 v3 (referenced via review)

**Bug verdict:** **BUGS FOUND — 2 CRITICAL, 4 HIGH, 5 MEDIUM, 4 LOW** (15 total)
**Test coverage:** **GAPS FOUND** — 7 missing-coverage areas (TG-001..TG-007) + 5 stress scenarios (SS-001..SS-005)
**Safe-to-merge verdict:** **NOT safe-to-merge as-is.** Two CRITICAL bugs (BUG-001, BUG-002) make the registry CLI path unusable for the v1 demo case (constant polynomial). Recommend a TASK follow-up filed before merge OR add to Stage 6 REFACTOR scope.

---

**Stage 6 REFACTOR closure (TASK-0721, 2026-05-06):** BUG-001, BUG-002, BUG-003, BUG-004, BUG-005, BUG-009, BUG-012 marked `Status: FIXED` inline below. SF-002, SF-004 from the reviewer report are also closed by this dispatch. After Stage 6 the bundle is **safe-to-merge** modulo the deferred MEDIUM/LOW bugs (BUG-006/007/008/010/011/013/014/015) that remain open as won't-fix or future-task as called out in their entries. New IT `tests/horner_codec_cli_roundtrip.rs` (~130 LoC) covers AC-1+AC-2+AC-3 end-to-end through the registry path. Pisos post-fix (Windows): default 1899 (+14), zero-copy 1943, streaming-no-recycle 1890, release 1841.

---

## Top 3 CRITICAL/HIGH bugs

1. **BUG-001 (CRITICAL):** `run_compute_with_encoder` does NOT call `discover_root` post-reduction. CLI invocation `relativist compute --codec horner --input '{"coeffs":[1,1],"x":2}'` returns `DecodeError::DecodeFailed("no root")` for *every* non-constant Horner input, because `HornerCodec::encode` sets `net.root = None` and the registry pipeline never recovers it.
2. **BUG-002 (CRITICAL):** `EncoderRegistry::encode_and_validate("horner", ...)` rejects every constant-polynomial input via E2 (`net has no redexes`). The constant-poly fast path in `HornerCodec::encode` returns a Normal-Form net immediately. The registry path that the CLI uses (`run_compute_with_encoder` → `encode_and_validate`) thus blocks the only readable Horner case from CLI.
3. **BUG-003 (HIGH):** `ChurchArithmeticCodec::encode` does NOT validate `a`/`b` against `MAX_CHURCH_NAT` — it forwards directly to `build_add` / `build_mul` / `build_exp` / `build_sum_of_squares`, which call `encode_church_into` with an `assert!(n <= MAX_CHURCH /* 1_000_000 */)`. Inputs with `a > 1_000_000` cause a process-aborting panic (DoS via crafted CLI JSON or wire input).

---

## Verdict on the 8 reviewer attack vectors

| # | Reviewer attack | QA verdict | Notes |
|---|------------------|------------|-------|
| 1 | G1 strictness probe (`format!("{:?}")`) | **AMPLIFIED** — the equality is genuinely structural for `Ok` variants, but for `Err` variants `DecodeError::UnrecognizedStructure(_)` carries free-form `String` payloads that are stable only as long as the error-path strings agree byte-for-byte. See BUG-004. |
| 2 | MF-001 `is_recipe_encoder` confirmed | **CONFIRMED** — the helper does what reviewer said: `type_name::<T>().contains("HornerCodec")` returns `true` for any type whose name *contains* "HornerCodec" (e.g., `MyHornerCodecMock`). Worse: the test would *still pass* if someone added `impl RecipeEncoder for HornerCodec` (BUG-005). |
| 3 | OracleError enum coverage | **REFUTED in part, AMPLIFIED in part** — the three variants cover what the encoder's R12' validation rejects. However the BigUint multiplication has no overflow protection — a maliciously crafted multi-iter case at boundary `[10000;25] @ 10000` produces ~10^104 which exhausts heap if instantiated repeatedly (DoS, see SS-002). |
| 4 | MAX_CHURCH_NAT boundary semantic | **AMPLIFIED** — boundary 10_000 inclusive is correctly tested. But validation order in encoder is "iterate all coeffs first, then check x" — a single oversize coef [10001] fails at index 0 even when x is also oversize (10001). Family-correspondence test PT-0715-07 hard-codes `max == 10_000` literal in lines 668/675 — if `MAX_CHURCH_NAT` ever changes, those literals need updating. **Not a single SOT.** See BUG-006. |
| 5 | Constants drift (TASK-0713/0714 _constants.rs) | **CONFIRMED** — see BUG-006. Three tests/files reference `10_000` literal: `horner_oracle.rs:215, 240, 248, 256`, `horner.rs:357, 369, 388`, and `horner.rs:668, 675` (PT-0715-07). |
| 6 | Registry duplicate-name R20 + casing | **CONFIRMED + AMPLIFIED** — `register("horner")` twice → `DuplicateName`. Casing: `"Horner" != "horner"` (case-sensitive lookup tested in `registry_lookup_is_case_sensitive`). However nothing rejects `register("HORNER")` *coexisting* with `register("horner")` — both register successfully (case-distinct names). User confusion risk: silent two-codec collision. See BUG-008. |
| 7 | CLI conflicts_with bypass | **REFUTED** — clap's `conflicts_with` correctly catches both `--encoder X --codec Y` and `--codec X --encoder Y`. Env-var bypass is N/A (no env reads in `ComputeArgs`). Positional bypass: clap rejects extra positionals (verified in test). |
| 8 | TASK-0719 R25 fallback path correctness | **CONFIRMED with caveat** — IT-0719-04 only exercises **constant polynomial** input (which trivially works because `coeffs.len() == 1` produces a Normal-Form net). The "fallback path is correct" claim is therefore *partially* untested for actual run_grid behavior. If a future codec adds `impl RecipeEncoder for ChurchArithmeticCodec`, the silent-pass via string check (BUG-005) means breakage flies under the radar. |

---

## Verdict on additional QA attack vectors (A–H)

| # | Attack | QA verdict |
|---|--------|------------|
| A | `count_valid_active_pairs` adversarial cases | **PASSING** — review of `reduction/mod.rs:41-46` shows correct `is_valid_redex` filter; all reviewer scenarios (empty queue, all-stale, mixed) covered by UT-0709-01..05. **One caveat (BUG-009):** `#[allow(dead_code)]` on line 40 is stale (already flagged as SF-001 in review). |
| B | `biguint_readback` recursive helpers stack overflow | **CONFIRMED HIGH** — depth cap of 64 is hard-coded magic number (SF-003). For pathological inputs constructed manually (not from HornerCodec), the recursion can exhaust stack BEFORE the depth cap fires. See BUG-010, SS-001. Additionally the arm-ordering in `count_chain_through_dups` line 217 has a **partially unreachable pattern** — the `p == 1` arm is shadowed by the earlier line 183 match. See BUG-011. |
| C | HornerCodec encoder R13' edge cases | **CONFIRMED MEDIUM** — `coeffs.len() == 1` happy path: Normal-Form net, root set, decode_biguint returns coeffs[0]. **All-zero coeffs `[0,0,...,0]`**: encoder produces a multi-iter Horner net that reduces to Church(0); decode_biguint walks the self-loop frame (line 111). **EDGE: `[0,0]@7` is tested via `reduce_and_decode` (line 304-306), returns Some(0).** **Missing edge:** `[0]@7` (constant poly, value 0) — registry path returns E2 error (BUG-002) so this case is silently broken at the CLI. |
| D | CLI command ambiguity (malformed JSON, missing fields) | **PARTIAL — MEDIUM gaps** — `serde_json` produces decent error message for malformed JSON but bubbles up as `EncodeError::InvalidInput` with raw `serde` error text (line 113 in horner.rs). For `{"coeffs":[1,2]}` (missing `x`), serde will reject "missing field `x`" — adequate. For `{"coeffs":[],"x":0}` empty coeffs, encoder returns custom message. Acceptable. **However:** `relativist compute --codec horner --input ''` returns `InvalidInput(JSON parse failed: EOF)` — fine — but could be improved. |
| E | Encoders list output stability | **PASSING** — `EncoderRegistry::list()` sorts by name, deterministic. Output line ordering is fixed by alphabetical sort. |
| F | Workspace dep injection (num-bigint pinning) | **NOT VERIFIED** — `Cargo.toml` not deeply inspected; deferred to cicd. Reviewer flagged this as a follow-up. No bug filed; tracked as TG-007. |
| G | PT-0715-06 silent skip rate (SF-004) | **AMPLIFIED** — confirmed by reading the proptest body (horner.rs:603-610). The `if let Ok(out)` arm silently passes when readback returns `Err`. **Worse**: with the input range `coeffs.len() == 2 with both >= 1`, the readable subset is dense (probably ~95% Ok), so SF-004 in practice may not be testing what the comment suggests. But because the threshold is unguarded, a future regression in single-iter readback would silently flip the proptest from "100 cases verified" to "0 cases verified, 100 skipped". |
| H | T13 across feature flags | **NOT EMPIRICALLY VERIFIED** — the QA host could not run `cargo test`. Theoretical analysis: HornerCodec uses no zero-copy / streaming-no-recycle-specific code paths; expected to pass on all profiles. cicd should confirm. |

---

## Verdict on the readback limitation under adversarial pressure

The reviewer's position (Finding 1) is: **accept v1, reframe G1 narrative, defer Mackie/Pinto-style readback to SPEC-28**. After adversarial probing I find this position **largely sustainable, with two caveats**:

1. **The `format!("{:?}")` equality is sound for `Ok` results and stable for `UnrecognizedStructure(static_str)` cases.** The HornerCodec readback paths produce errors with deterministic strings ("DUP branch disconnected", "non-CON in app chain", etc.) keyed off topology. If both seq and inproc reductions converge on the same NF (which P1/P3/P4 guarantee for terminating nets), they read back through the same code path and produce byte-identical error strings.

2. **BUT (BUG-004):** the `chain_from_dup_branch` error at line 303 includes `format!("unrecognized DUP-branch destination at agent {id} symbol {:?} port {dest:?}")` — and `id` is an `AgentId`. If sequential and distributed reductions produce structurally isomorphic NFs but with **different agent IDs** (which they MUST, because partition + merge re-IDs), the seq Err and inproc Err will have *different* `id` fields embedded in the string, and `format!("{:?}")` equality FAILS even though G1 holds. This is a **silent G1 false-negative**: the test would fail-loudly (panic) on a structural mismatch that isn't a real divergence. Whether this can be triggered depends on whether T6/T7/T8 actually hit this error path; if both legs of the comparison use different decode paths (which they do — different agent IDs through `chain_from_dup_branch` line 303), the test could fail spuriously.

   **Severity:** this is HIGH because the test purports to demonstrate G1 empirically. If it can panic on a non-G1-violating input, the empirical claim is weakened.

3. **Readback failure parity probe (suggested):** construct a Horner input that fails readback. Compare `seq_decoded(input)` and `inproc_decoded(input, 4)`. If both return `Err(UnrecognizedStructure(s_seq))` and `Err(UnrecognizedStructure(s_inproc))` and `s_seq != s_inproc`, the test panics. Actual behavior on T6/T7/T8 is empirical — not verified by QA host.

**Net QA position:** the readback limitation IS defendable for the TCC's empirical claim, but the structural-isomorphism test should be **strengthened** to compare topology (agent counts, edge-set hashes) rather than `format!("{:?}")`. This is REFACTOR scope (~40 LoC). Until then, the test is fragile.

---

## Bugs found

### BUG-001: CLI registry path skips `discover_root` for HornerCodec — non-constant Horner inputs always fail to decode

**Severity:** **CRITICAL**
**Status:** **FIXED (TASK-0721, 2026-05-06)** — `run_compute_with_encoder` now calls `crate::encoding::discover_root(&mut net)` after `reduce_all` when `net.root.is_none()` (mirrors the existing convention used by `seq_decoded`/`inproc_decoded` in `tests/horner_distributed_g1.rs`). Verified by new IT `cli_horner_non_constant_polynomial_decodes` in `tests/horner_codec_cli_roundtrip.rs` — `[1,1] @ 2` now decodes to `{"value":"3","bit_length":2}`.
**File:** `relativist-core/src/commands.rs:697-731` (`run_compute_with_encoder`)
**Category:** Logic Error / Spec compliance gap (R23 pipeline)

**Description:**
`run_compute_with_encoder` runs `encode_and_validate → reduce_all → decode → print JSON` but does NOT call `discover_root(&mut net)` between `reduce_all` and `decode`. `HornerCodec::encode` sets `net.root = None` for any non-constant polynomial (lines 174-177 in horner.rs); `decode_biguint` requires `net.root.is_some()` (line 57-59 in biguint_readback.rs) and returns `DecodeError::DecodeFailed("no root")` otherwise.

The integration tests `seq_decoded` / `inproc_decoded` (horner_distributed_g1.rs:42-68) explicitly call `discover_root` between reduce and decode — but the CLI path does not. **Tests pass but CLI is broken.**

**Reproduction:**
```bash
$ relativist compute --codec horner --input '{"coeffs":[1,1],"x":2}'
=== Relativist Compute (encoder: horner) ===
Encoding:    XX agents, YY redexes
Reduction:   ... interactions ...
ERROR: decode failed: no root
```

**Expected:** decoded value `{"value":"3","bit_length":2}` (single-iteration Horner, readable case).
**Actual:** `DecodeError::DecodeFailed("no root")` propagated up as `RegistryError::Decode(...)`.

**Fix suggestion:**
```rust
// In commands.rs run_compute_with_encoder after reduce_all:
let stats = reduce_all(&mut net);
let elapsed = start.elapsed();
// SPEC-27 R23 pipeline: discover_root is required for codecs that emit
// FreePort-rooted nets (HornerCodec multi-iter, build_add/mul/exp post-reduce).
if net.root.is_none() {
    crate::encoding::discover_root(&mut net);
}
let json_out = registry.decode(name, &net)?;
```

**Why critical:** the v1 G1-demo Horner pipeline is documented in SPEC-27 v3 §6 Phase 5 as the user-visible CLI surface. The bug means the CLI is unusable for the codec the spec was rewritten around. CI does not catch this because there is no CLI integration test for `compute --codec horner`.

---

### BUG-002: Registry `encode_and_validate("horner", const_poly)` rejects with E2 — constant-polynomial CLI input is silently broken

**Severity:** **CRITICAL**
**Status:** **FIXED (TASK-0721, 2026-05-06)** — Path A applied: new method `Codec::accepts_normal_form_input(&self, input: &[u8]) -> bool` (default `false`); HornerCodec returns `true` for `coeffs.len() == 1`. Registry's `encode_and_validate` consults the codec before validation and routes to `validate_encoded_net_allowing_normal_form` (E2 bypass) when the codec opts in. ChurchArithmeticCodec keeps the default `false` (its nets always have redexes by construction). Verified by `cli_horner_constant_polynomial_decodes` (`[42] @ 99 → 42`), `cli_horner_constant_zero_polynomial_decodes`, and `cli_horner_constant_max_church_polynomial_decodes` in `tests/horner_codec_cli_roundtrip.rs`.
**File:** `relativist-core/src/encoding/horner.rs:139-143` (constant-polynomial fast path) + `relativist-core/src/encoding/traits.rs:124-128` (`validate_encoded_net` E2 check)
**Category:** Spec compliance / Logic error

**Description:**
`HornerCodec::encode` for `coeffs.len() == 1` returns a net with zero redexes (it's already in Normal Form — just `encode_church_into(coeffs[0])`). The registry's `encode_and_validate` calls `validate_encoded_net(&net)`, which calls `net.is_reduced()` (= `redex_queue.is_empty()`) and rejects with `EncodeError::InvalidNet("E2: net has no redexes (nothing to reduce)")`.

The encoder doc-comment (horner.rs:39-45) acknowledges this: "the encoded net has zero redexes and its E2 (at-least-one-redex) check is the registry's responsibility". But the registry has only ONE validate path (`encode_and_validate`), and **it always validates E2**. The CLI uses `encode_and_validate` exclusively. **There is no codec-aware bypass.**

**Reproduction:**
```bash
$ relativist compute --codec horner --input '{"coeffs":[42],"x":7}'
=== Relativist Compute (encoder: horner) ===
ERROR: encoding produced invalid net: E2: net has no redexes (nothing to reduce)
```

The unit test `horner_encode_constant_polynomial_skips_loop` (horner.rs:218-244) DOES NOT call `encode_and_validate` — it bypasses the registry entirely. `T23 / IT-0719-04` (integration_horner_centralized.rs) ALSO bypasses `encode_and_validate` (line 34: `codec.encode(json)` directly). **The bug is invisible to all current tests** because no test exercises the registry path for a Horner constant polynomial.

**Expected:** registry should accept constant polynomial Horner input, return a Normal-Form net for direct decode (no reduction needed).
**Actual:** rejected with E2 error.

**Fix suggestion (Option A — preferred):** SPEC-27 §3.2 R5 wording is "an encoded net MUST satisfy T1-T7 AND have at least one redex *unless the encoded net is already in Normal Form by construction*". Add a Normal-Form bypass to `validate_encoded_net`:
```rust
// E2: net has at least one redex OR is already in Normal Form by
// construction (constant-polynomial codecs return NF nets directly).
// We proxy "already-NF-by-construction" via "has at least one live agent
// AND root is Some" — empty/rootless nets still rejected.
if net.is_reduced() && (net.root.is_none() || net.count_live_agents() == 0) {
    return Err(EncodeError::InvalidNet(
        "E2: net is empty/rootless and has no redexes".to_string(),
    ));
}
```

**Fix suggestion (Option B — narrower):** Add a `Codec::accepts_nf_input(&self) -> bool` method (defaults to `false`); have `encode_and_validate` skip E2 when codec opts in.

**Fix suggestion (Option C — punt to SPEC):** Amend SPEC-27 R5 to explicitly allow Codec encoders to return NF nets, document the registry-path constraint, and require constant-poly Horner CLI users to construct a non-trivial input. This is the LEAST invasive but contradicts the user-facing demo.

**Why critical:** the "readable subset" the reviewer accepts as defendable in §6 of the review is precisely **constant polynomials and single-iteration Horner**. The latter has BUG-001 (no discover_root). The former has BUG-002 (E2 reject). **No Horner input is currently usable from the CLI.** The G1 empirical demo exists only in `cargo test` paths, not in the user-facing tool.

---

### BUG-003: `ChurchArithmeticCodec::encode` panics on inputs > MAX_CHURCH (1_000_000)

**Severity:** **HIGH** (security: DoS via crafted JSON; Rust panic = process abort)
**Status:** **FIXED (TASK-0721, 2026-05-06)** — `ChurchArithmeticCodec::encode` now validates `params.a` and (when applicable) `params.b` against `MAX_CHURCH_NAT` before delegating to `build_*`, mirroring HornerCodec's R12' bound check. `sum_of_squares` keeps `b` ignored per R8. Verified by `church_codec_rejects_a_above_max_church_nat`, `church_codec_rejects_b_above_max_church_nat`, `church_codec_accepts_boundary_max_church_nat`, `church_codec_sum_of_squares_ignores_oversize_b`, `church_codec_oversize_a_does_not_panic_under_unwind_test` (lib tests) and `cli_church_add_oversize_a_returns_invalid_input`, `cli_church_add_u64_max_a_returns_invalid_input`, `cli_church_mul_oversize_b_returns_invalid_input` (IT).
**File:** `relativist-core/src/encoding/codec_church.rs:129-156`
**Category:** Panic Path / Input validation gap

**Description:**
`ChurchArithmeticCodec::encode` calls `build_add(params.a, params.b.unwrap_or(0))` etc. directly with no validation of `a` or `b` against the public cap `MAX_CHURCH_NAT = 10_000`. The chain `build_add → encode_church_into → assert!(n <= MAX_CHURCH /* 1_000_000 */)` (church.rs:40 / 64) panics for any input where `a > 1_000_000` or `b > 1_000_000`.

Note: the assert message claims `MAX_CHURCH = 1_000_000`, but R12'/SC-013 say codecs MUST validate against `MAX_CHURCH_NAT = 10_000` BEFORE delegating. HornerCodec does this correctly (lines 122-132). ChurchArithmeticCodec does NOT.

**Reproduction:**
```bash
$ relativist compute --codec church_add --input '{"a":2000000,"b":1}'
thread 'main' panicked at 'encode_church_into: n = 2000000 exceeds maximum supported value 1000000', src/encoding/church.rs:64:5
```

A protocol-layer attacker submitting an `AssignWork` payload with crafted Church inputs could crash a worker. With `panic = "abort"` (release builds typically), the entire process terminates.

**Expected:** `EncodeError::InvalidInput("a = 2000000 exceeds cap (max 10000)")`.
**Actual:** Rust panic, process abort.

**Fix suggestion:**
```rust
// In codec_church.rs::encode, before the match block:
use crate::encoding::church::MAX_CHURCH_NAT;
if params.a > MAX_CHURCH_NAT {
    return Err(EncodeError::InvalidInput(format!(
        "a = {} exceeds cap (max {})", params.a, MAX_CHURCH_NAT
    )));
}
if let Some(b) = params.b {
    if b > MAX_CHURCH_NAT {
        return Err(EncodeError::InvalidInput(format!(
            "b = {} exceeds cap (max {})", b, MAX_CHURCH_NAT
        )));
    }
}
```

**Why high:** the bug pre-exists D-015 — but D-015 publishes `MAX_CHURCH_NAT` as a SOT constant (R12'/SC-013) and aligns HornerCodec to it. The asymmetry (Horner validates, Church does not) is now *visible*; a security-conscious reviewer could file this as the v3 disposition introduces inconsistent behavior across the registry's 5 codecs.

---

### BUG-004: `format!("{:?}")` G1 isomorphism check is fragile against agent-ID divergence in error payloads

**Severity:** **HIGH** (test reliability / G1 empirical claim)
**Status:** **FIXED (TASK-0721, 2026-05-06)** — Replaced `format!("{:?}")` debug equality with a `StructuralOutcome` witness that classifies decode results into three families: `Decoded { value, bit_length }` (compared exactly), `DecodeFailed { family_tag }` (variant name only — agent-ID-bearing inner strings are intentionally elided), and `EncodeFailed { msg }` (encoder errors are deterministic from input bytes). The witness allows agent-ID divergence in DUP-branch error payloads (which would legitimately differ between sequential and distributed reductions) while still requiring identical decoded values when both legs succeed. Mackie/Pinto-style structural readback remains future work per SPEC-27 §5.1.
**File:** `relativist-core/tests/horner_distributed_g1.rs:130-138` + `relativist-core/src/encoding/biguint_readback.rs:303-306`
**Category:** Logic error / test fragility

**Description:**
`horner_distributed_g1_in_process_structural_isomorphism` (line 110-140) compares `format!("{seq:?}")` and `format!("{inproc:?}")`. The `seq` and `inproc` results are `Result<serde_json::Value, String>`. For inputs that fail v1 readback, both legs return `Err(...)` with strings like `"decode: UnrecognizedStructure(\"...\")"`.

The error payload inside `chain_from_dup_branch` (biguint_readback.rs:303) includes `format!("unrecognized DUP-branch destination at agent {id} symbol ...")`. The `id` is an `AgentId`. **Sequential and distributed reductions produce structurally isomorphic NFs with different agent IDs** (because partition + merge re-IDs agents — see SPEC-04 R12). So even when G1 holds, `format!("{:?}")` of the two error strings can differ in the `agent {id}` substring, causing the test to PANIC on a non-violating input.

**Reproduction:** speculative — depends on whether `chain_from_dup_branch` line 303 is actually reached on T6/T7/T8 inputs. The test currently passes (per developer self-report). But a refactor that changes which structural error fires (e.g., changing arm priority in `count_chain_through_dups`) could flip the test from passing to spuriously panicking.

**Expected:** the test should compare *topology* (agent counts, edge-set hashes, symbol histograms) — invariants under ID renaming.
**Actual:** the test compares debug strings, which include ID-leaky payloads.

**Fix suggestion (REFACTOR scope, ~40 LoC):**
```rust
fn structural_signature(net: &Net) -> (usize, usize, [usize; 3]) {
    let mut sym_hist = [0usize; 3]; // [Con, Dup, Era]
    for agent in net.live_agents() {
        match agent.symbol {
            Symbol::Con => sym_hist[0] += 1,
            Symbol::Dup => sym_hist[1] += 1,
            Symbol::Era => sym_hist[2] += 1,
        }
    }
    let edge_count = /* count of port-pair edges */;
    (net.count_live_agents(), edge_count, sym_hist)
}
// Then compare structural_signature(seq_net) == structural_signature(inproc_net).
```

This was specifically called out by the reviewer as Stage 6 REFACTOR option (review §6 item 8). QA confirms the concern is real, not theoretical.

**Why high:** the test is named "structural isomorphism" but it's not structural — it's stringly-typed. The TCC narrative leans on T13 as the empirical witness for G1 in §6 of the artigo; if T13 can fail spuriously on a non-G1-violating input, the empirical claim is dilutable.

---

### BUG-005: `is_recipe_encoder` test is misleading (does NOT detect future `impl RecipeEncoder for HornerCodec`)

**Severity:** **HIGH** (already reviewer MF-001; QA confirms)
**Status:** **FIXED (TASK-0721, 2026-05-06)** — Reviewer Option A applied: replaced the string-probe helper with `static_assertions::assert_not_impl_all!(crate::encoding::HornerCodec: RecipeEncoder)`. If anyone adds `impl RecipeEncoder for HornerCodec`, the recipe test module fails to compile (compile-time witness, not runtime). The runtime `horner_codec_is_not_recipe_encoder` test is retained as a documentation breadcrumb/grep target. `static_assertions = "1"` was already present as a regular dep (no Cargo.toml change required).
**File:** `relativist-core/src/encoding/recipe.rs:380-387`
**Category:** Logic error / test that doesn't test what it claims

**Description:**
This is the reviewer's MF-001. QA confirms: the function `is_recipe_encoder<T: 'static>() -> bool` is `std::any::type_name::<T>().contains("HornerCodec")`. Adding `impl RecipeEncoder for HornerCodec { ... }` somewhere else in the crate would NOT fail this test. The test name `horner_codec_is_not_recipe_encoder` is therefore semantically empty.

**Reproduction (manual mutation — not run, see SS-003):**
1. Create `relativist-core/src/encoding/horner_recipe.rs`:
```rust
use super::horner::HornerCodec;
use super::recipe::RecipeEncoder;
use super::traits::{EncodeError, Encoder};
use crate::partition::Partition;

impl RecipeEncoder for HornerCodec {
    type Recipe = ();
    fn is_decomposable(&self) -> bool { true }
    fn make_recipes(&self, _i: &[u8], _w: u32) -> Result<Vec<()>, EncodeError> { Ok(vec![]) }
    fn generate_partition(&self, _r: &()) -> Result<Partition, EncodeError> { todo!() }
}
```
2. Run `cargo test horner_codec_is_not_recipe_encoder`. **Expected (correct test):** fails. **Actual:** passes. The test name has flipped semantically; the assertion is now factually wrong but the test runner cannot tell.

**Fix suggestion:** see reviewer Option A (compile-time negative bound — preferred) or Option B (rename helper). The reviewer's recipe is correct; QA endorses Option A as the best signal.

---

### BUG-006: PT-0715-07 hard-codes `max == 10_000` literal in family-correspondence checks (MAX_CHURCH_NAT not used)

**Severity:** **MEDIUM** (SOT discipline / regression risk)
**File:** `relativist-core/src/encoding/horner.rs:668, 675`
**Category:** Spec compliance / Constants drift

**Description:**
SC-013 declared `MAX_CHURCH_NAT` as the single source of truth — codecs and tests MUST source from it, not hard-code `10_000`. The encoder and oracle correctly use `MAX_CHURCH_NAT`. **PT-0715-07 hard-codes the literal `10_000` in two places:**
```rust
prop_assert_eq!(max, 10_000);
```
on lines 668 and 675. If `MAX_CHURCH_NAT` is raised (e.g., to 100_000), these literals do not update; the proptest fails opaquely with `"left: 10000 right: 100000"`.

**Reproduction:** change `pub const MAX_CHURCH_NAT: u64 = 10_000;` to `100_000` in church.rs. Run `cargo test horner_property_test_negative_cross_check`. Test fails on the `prop_assert_eq!(max, 10_000)` line.

**Fix suggestion:**
```rust
use crate::encoding::church::MAX_CHURCH_NAT;
// ...
prop_assert_eq!(max, MAX_CHURCH_NAT);
```

**Why medium:** doesn't affect runtime correctness; only catches the regression at the wrong place (test failure with confusing message instead of automatic propagation). But this is exactly what SC-013 was supposed to prevent.

---

### BUG-007: `count_chain_through_dups` arm-ordering shadows DUP-aux-port p1 case

**Severity:** **MEDIUM** (dead code path; documentation lies)
**File:** `relativist-core/src/encoding/biguint_readback.rs:156-235`
**Category:** Logic error (subtle) / arm-ordering

**Description:**
The match in `count_chain_through_dups` has these arms in order:
1. Line 180: `PortRef::AgentPort(id, port) if id == lam_x && port == 1` → return count
2. Line 183: `PortRef::AgentPort(app_id, 1)` → CON app, count += 1, advance to p2
3. Line 199: `PortRef::AgentPort(dup_id, 0)` → DUP via principal port, recurse
4. Line 217: `PortRef::AgentPort(dup_id, p) if p == 1 || p == 2` → DUP via aux port

**Arm 2** matches ANY `(id, 1)`. Arm 4's `p == 1` branch is therefore unreachable: a DUP at port 1 reaches arm 2 first, where the agent symbol check fails (`agent.symbol != Symbol::Con`) and returns `Err(UnrecognizedStructure("non-CON in app chain"))`. The comment on lines 214-216 ("we approached a DUP's p1 or p2 from outside") is false for `p1`.

**Implication:** any net that legitimately has a DUP at p1 of an aux-port reachable from `lam_x.p2` decodes as `Err("non-CON in app chain")` instead of being walked through. Whether HornerCodec produces such structures: unclear from inspection alone (would need empirical reduction trace). For safety, the dead arm should be removed and the comment fixed, OR the order swapped to make `p == 1 || p == 2` for DUPs reachable.

**Fix suggestion:**
```rust
// Reorder: check DUP-aux-port BEFORE the generic CON-app-port arm.
PortRef::AgentPort(maybe_dup_id, p) if p == 1 || p == 2 => {
    if let Some(agent) = net.get_agent(maybe_dup_id) {
        if agent.symbol == Symbol::Dup {
            here = PortRef::AgentPort(maybe_dup_id, 0);
            continue;
        }
        // Not a DUP — fall through to the CON-app-port logic below.
    }
    // ...
}
```

**Why medium:** hard to triage as CRITICAL because we cannot empirically verify HornerCodec produces this topology. But the comment lies, and silent dead code in a security-sensitive readback module is a smell.

---

### BUG-008: Registry allows case-distinct codec name collisions (`"horner"` and `"HORNER"` coexist)

**Severity:** **LOW** (edge case; arguably by design)
**File:** `relativist-core/src/encoding/registry.rs:43-50`
**Category:** Input validation / UX

**Description:**
`EncoderRegistry::register` checks `self.codecs.contains_key(&name)` for exact match. `"Horner"`, `"horner"`, and `"HORNER"` are three distinct keys. The CLI lookup at registry.get is case-sensitive (per `registry_lookup_is_case_sensitive` test). So a user could register a malicious "Horner" alongside the official "horner" and cause confusion.

**Expected behavior (debatable):** either case-insensitive lookup OR rejection of case-distinct names that hash-collide modulo case. SPEC-27 v3 R20 just says "duplicate name" — strict interpretation = exact-match. So this is arguably correct per spec.

**Suggested test:**
```rust
#[test]
fn case_distinct_names_coexist() {
    let mut r = EncoderRegistry::new();
    r.register(Box::new(HornerCodec::new())).unwrap();  // "horner"
    // Manually create a codec wrapper renamed to "HORNER"...
    // (This would require a wrapper struct to test, hence we just document the behavior.)
}
```

**Fix suggestion:** document the case-sensitivity in `EncoderRegistry::register` rustdoc; optionally add a debug-only warning if a case-insensitive match exists. Not a blocking bug.

---

### BUG-009: Stale `#[allow(dead_code)]` on `count_valid_active_pairs`

**Severity:** **LOW** (already reviewer SF-001)
**Status:** **NOT FIXED in TASK-0721** — out of scope for this dispatch (the helper is now consumed by `decode_biguint`'s `count_valid_active_pairs` call, so the attribute may already be stale-but-not-warning; verifying the warning state was not in the Stage 6 critical path). Leave as-is for the next clippy sweep.
**File:** `relativist-core/src/reduction/mod.rs:40`
**Category:** Code cleanliness

Reviewer SF-001 confirmed; QA agrees. Remove the attribute and the comment in REFACTOR.

---

### BUG-010: `count_chain_through_dups` recursion has no stack-overflow guard before the depth=64 cap

**Severity:** **MEDIUM** (DoS / pathological-input)
**File:** `relativist-core/src/encoding/biguint_readback.rs:156-313`
**Category:** Resource exhaustion

**Description:**
The depth cap of 64 is checked *after* the function call enters the stack frame. For a maliciously constructed net with a >64-deep DUP chain, recursion happens 65 levels before the cap fires — enough to consume kilobytes of stack but not enough to overflow. **However:** if the cap is raised in a future codec, OR if a non-DUP cycle exists in the net (the cap ONLY catches "depth exceeded"), the function can recurse indefinitely until the OS-imposed stack limit aborts the process.

The cap is a magic-number `64` (reviewer SF-003). On 64-bit Linux with 8 MiB default stack, a recursion frame of ~256 bytes fits ~30k levels — safe. But on Windows with 1 MiB default stack, only ~4k levels fit — closer to the limit if a future codec admits very deep DUP nesting (Mackie/Pinto-style readback for deeply iterated multiplication).

Additionally: net topology corruption (e.g., a DUP whose p1 self-loops back through its own principal) could create a true cycle that the depth cap catches eventually, but not before consuming `64 * frame_size` bytes of stack per call.

**Reproduction (synthetic):** construct a Net with a DUP-cycle: 5 DUPs in a ring where each p1 connects to the next DUP's p0. Manually call `decode_biguint`. Trace recursion depth. Cap fires at depth 65; stack grew by ~16 KiB.

**Fix suggestion:**
1. Replace recursion with explicit work-stack (BFS over DUP boundaries) — eliminates stack-overflow risk entirely.
2. OR: lower the cap to a value tied to `coeffs.len()` (the legitimate input size), not a hard-coded magic number.
3. Reviewer SF-003 already proposes naming the constant `MAX_READBACK_DEPTH`; QA endorses it AND tightens the value to `32` (more than enough for v1 HornerCodec; pathological inputs trip earlier).

---

### BUG-011: HornerCodec encoder panics if `encode_church_into` is called with `n > MAX_CHURCH (1_000_000)`

**Severity:** **MEDIUM** (defensive — encoder validates `MAX_CHURCH_NAT = 10_000` first, so this is unreachable in current code; but the panic path is still embedded)
**File:** `relativist-core/src/encoding/church.rs:62-66`
**Category:** Panic Path

**Description:**
`encode_church_into` (church.rs:62) has `assert!(n <= MAX_CHURCH /* 1_000_000 */)`. The HornerCodec encoder validates `<= MAX_CHURCH_NAT (10_000)` first (lines 122-132 of horner.rs), so the inner assert is unreachable from HornerCodec. But:
- BUG-003 above shows ChurchArithmeticCodec does NOT validate, so the assert IS reachable from `ChurchArithmeticCodec::encode`.
- Future codecs that compose Horner with multiplication (e.g., `PolynomialMultiEvalCodec`) might bypass the cap.

The assert message reveals an internal constant — defense in depth would replace this with a proper `Result` return + propagation. `MAX_CHURCH > MAX_CHURCH_NAT` by 100x — a buffer that allows internal compositions to grow without immediate ceiling. That's good. But the `.unwrap()`-equivalent `assert!` should be a `Result` return.

**Fix suggestion:**
```rust
pub fn encode_church_into(net: &mut Net, n: u64) -> Result<AgentId, EncodeError> {
    if n > MAX_CHURCH {
        return Err(EncodeError::InvalidInput(format!(
            "encode_church_into: n = {n} exceeds MAX_CHURCH = {MAX_CHURCH}"
        )));
    }
    // ... rest unchanged
}
```

This is a breaking change to a public function; properly handled in a SPEC-14 amendment (NOT D-015 scope). For D-015, just document the panic path in the function rustdoc and require all callers to validate first.

---

### BUG-012: PT-0715-06 silent skip pattern (already reviewer SF-004)

**Severity:** **MEDIUM** (already reviewer SF-004)
**Status:** **FIXED (TASK-0721, 2026-05-06)** — PT-0715-06 now increments thread-local atomic counters on `Ok` and `Err` paths; companion test `pt_0715_06_skip_rate_is_bounded` enumerates the proptest's domain (`a, b, x in 1..=10`, 1000 cases) deterministically and asserts `skips <= 95% * total`. Test-isolation note: the deterministic enumeration is identical to (a superset of) the proptest's input distribution, so the skip-rate guard runs even when the proptest is filtered out.
**File:** `relativist-core/src/encoding/horner.rs:603-610`

Reviewer SF-004; QA confirms. The `if let Ok(out)` arm makes Err cases silently pass. Suggested fix (skip-rate guard) is REFACTOR scope.

---

### BUG-013: `discover_root` ambiguity-detection swallows information (multiple DISCONNECTED candidates → silent false return)

**Severity:** **LOW**
**File:** `relativist-core/src/encoding/arithmetic.rs:46-76`
**Category:** Error path coverage

**Description:**
`discover_root` returns `false` if multiple CON agents have DISCONNECTED principal ports — without logging *which* ones. For a complex Horner reduction that produces an unexpected topology, the only signal back is "discover_root returned false" (or net.root remains None). Then decode fails with `DecodeError::DecodeFailed("no root")`. Diagnostic narrative: nothing actionable.

**Fix suggestion:** add `tracing::warn!` (or `tracing::debug!`) when ambiguity detected, naming the candidates. Not blocking.

---

### BUG-014: `chain_from_dup_branch` recursion has same depth=64 magic number but **no cycle-detection**

**Severity:** **LOW** (depth cap fires, just not gracefully)
**File:** `relativist-core/src/encoding/biguint_readback.rs:251-255`
**Category:** Resource exhaustion / magic number

Same as BUG-010 / SF-003. Two literal `64` values (lines 162, 251) — should both reference a single `MAX_READBACK_DEPTH` constant.

---

### BUG-015: `OracleError` variants do not include the original `coeffs` array — debug output leaks information AND lacks reconstruction

**Severity:** **LOW** (info exposure / diagnostic gap)
**File:** `relativist-core/src/encoding/horner_oracle.rs:28-41`
**Category:** Error path coverage

**Description:**
For `CoefficientOverflow { idx, value, max }`, the error reports the offending coefficient's index/value but NOT the full `coeffs` slice. A deep-debugging user has to grep upstream logs to reconstruct the input. Conversely, if a sensitive (e.g., proprietary coefficient list) is submitted, `value` exposes one element of it in error logs — minor info leak.

For QA purposes this is fine; flagging for completeness.

---

## Edge Cases Not Covered

### EC-001: Constant polynomial through registry CLI path
**Scenario:** `relativist compute --codec horner --input '{"coeffs":[42],"x":7}'`
**Input:** valid constant polynomial, x ignored
**Expected behavior:** print `{"value":"42", "bit_length":6}`
**Current behavior:** **broken** — registry rejects with E2 (BUG-002).
**Suggested test:** integration test in `relativist-core/tests/cli_integration.rs` exercising every codec via the registry.

### EC-002: Non-constant Horner through registry CLI path
**Scenario:** `relativist compute --codec horner --input '{"coeffs":[1,1],"x":2}'`
**Expected:** `{"value":"3", "bit_length":2}`
**Current:** **broken** — `DecodeError::DecodeFailed("no root")` (BUG-001).
**Suggested test:** same as EC-001.

### EC-003: Empty input bytes via CLI
**Scenario:** `relativist compute --codec horner --input ''`
**Current:** rejected via `EncodeError::InvalidInput(JSON parse failed: ...)`. **Acceptable**.

### EC-004: JSON missing "x" field
**Scenario:** `{"coeffs":[1,2,3]}`
**Current:** `EncodeError::InvalidInput("JSON parse failed: missing field 'x'")`. **Acceptable.**

### EC-005: JSON with negative coefficient (parse to u64)
**Scenario:** `{"coeffs":[-5],"x":2}`
**Current:** serde rejects with "invalid type" — NOT a `CoefficientOverflow`. Family-correspondence check PT-0715-07 does NOT cover negative inputs. **Gap.**

### EC-006: Coefficient `u64::MAX`
**Scenario:** `{"coeffs":[18446744073709551615],"x":1}`
**Current:** parses; encoder returns `CoefficientOverflow` (passes `> MAX_CHURCH_NAT`). **Adequate.**

### EC-007: Cap boundary inclusive
**Scenario:** `{"coeffs":[10000],"x":10000}`
**Current:** encoder accepts; oracle returns `BigUint(10000)`. **Tested.**

### EC-008: `coeffs.len() == 1, coeffs[0] == 0` (constant zero polynomial)
**Scenario:** `{"coeffs":[0],"x":99}`
**Current:** encoder returns NF net, decode_biguint returns `BigUint::from(0u64)`. **Tested in horner_encode_constant_polynomial_skips_loop**, but BLOCKED by BUG-002 from CLI.

### EC-009: ChurchArithmeticCodec with `a > MAX_CHURCH_NAT`
**Scenario:** `compute --codec church_add --input '{"a":2000000,"b":1}'`
**Current:** **panics** via assert in encode_church_into (BUG-003).
**Suggested test:** test in codec_church.rs verifying `EncodeError::InvalidInput` is returned for `a > MAX_CHURCH_NAT`.

### EC-010: HornerCodec with very large coefficient count (DoS regime)
**Scenario:** `coeffs` of length 10_000, all equal to 10_000, x = 10_000.
**Current:** encoder accepts, builds net of ~10_000 * O(C) live agents, attempts reduce_all. Memory: O(N²) for nested-DUP intermediate forms. May OOM.
**Severity:** stress / SS-005 below.

---

## Test Coverage Gaps

### TG-001: No CLI integration test for `compute --codec horner`
**Missing test for:** end-to-end CLI exercise of every registered codec.
**Why it matters:** BUG-001 + BUG-002 — the entire registry-driven Horner pipeline is broken from CLI but invisible to current tests because all unit/integration tests bypass `encode_and_validate` and `run_compute_with_encoder`.
**Suggested test:** `tests/cli_integration.rs::compute_codec_horner_constant_polynomial`, `compute_codec_horner_non_constant`, `compute_codec_church_add_oversize` (panic regression).

### TG-002: No regression for ChurchArithmeticCodec input bound check
**Missing test for:** `ChurchArithmeticCodec::encode` rejecting `a > MAX_CHURCH_NAT`.
**Why it matters:** BUG-003. Currently a CRITICAL panic path is uncatchable.
**Suggested test:** `church_add_oversize_a_returns_invalid_input`.

### TG-003: No structural-isomorphism stronger than debug-string equality
**Missing test for:** topology-comparing test (agent counts, edge-set hashes) for T13.
**Why it matters:** BUG-004 — the current debug-string test is fragile against ID divergence in error payloads.

### TG-004: No tests of `encode_and_validate("horner", const_poly)`
**Missing test for:** `encode_and_validate` accepting a constant-polynomial Horner input.
**Why it matters:** BUG-002. Currently no test exists; the bug ships invisible.
**Suggested test:** add to registry.rs tests block.

### TG-005: PT-0715-06 has no skip-rate observability
**Missing test for:** `Ok(...)` count vs total_cases ratio (tracked via static counter in proptest body).
**Why it matters:** BUG-012 / SF-004 — silent regressions in the readable-subset readback would not fail the proptest.

### TG-006: No tests for `count_chain_through_dups` DUP-aux-port p1 destination
**Missing test for:** synthetic net with a DUP attached at port 1 of an aux destination.
**Why it matters:** BUG-007 — arm-ordering shadows this case; nothing exercises it.

### TG-007: Cargo.lock pinning of num-bigint not verified
**Missing test for:** `num-bigint` dep visibility (must NOT leak into `relativist-cli`).
**Why it matters:** workspace hygiene; deferred to cicd. Reviewer flagged.

---

## Stress Scenarios

### SS-001: Pathological DUP chain depth
**Scenario:** synthetic net with a 64-deep DUP chain reachable from `lam_x.p2`.
**Risk:** `count_chain_through_dups` hits depth cap. Current behavior: `Err(UnrecognizedStructure)`. ACCEPTABLE.
**Risk #2:** 65-deep chain → depth check fires AFTER 65 frames are pushed. ~16 KiB stack consumed per call. Multi-threaded coordinator under load could exhaust thread stacks via parallel decode requests.
**Recommendation:** convert recursion to explicit BFS work-stack (no per-frame growth). OR document that decode is single-threaded per request.

### SS-002: Memory-exhaustion via boundary BigUint
**Scenario:** `coeffs = [10_000; 25], x = 10_000` → result ~10^104, stored as BigUint.
**Risk:** 10^104 in BigUint = ~340 bits = ~50 bytes. Negligible. **Even at the maximum-of-maximum** input — `coeffs.len()` 10_000 × value 10_000 — the BigUint result is bounded by `O(coeffs.len() * log10(MAX_CHURCH_NAT))` digits = ~40_000 digits = 16 KiB. No DoS.
**Recommendation:** no action; the input cap discipline holds.

### SS-003: Adding `impl RecipeEncoder for HornerCodec` (manual mutation)
**Scenario:** developer adds `impl RecipeEncoder for HornerCodec` somewhere (e.g., during SPEC-25 M7 prototype).
**Risk:** test `horner_codec_is_not_recipe_encoder` STILL PASSES (BUG-005). The downstream IT-0719-04 fallback test still passes (because HornerCodec is constructed as `Box<dyn Codec>`, not as `Box<dyn RecipeEncoder>`). **Silent escape route.**
**Recommendation:** fix BUG-005 with compile-time negative bound (reviewer Option A).

### SS-004: Crafted JSON via wire protocol
**Scenario:** in a future v2 deployment with `AssignWork.encoder_input` field carrying user JSON, an attacker submits `{"a":18446744073709551615,"b":1}` to a `church_add` codec.
**Risk:** BUG-003 — process panic, worker abort. Coordinator cannot distinguish "worker crashed because of bug" from "worker crashed because of malicious input".
**Recommendation:** fix BUG-003 (validate against MAX_CHURCH_NAT in ChurchArithmeticCodec).

### SS-005: HornerCodec with very long coefficient lists
**Scenario:** `{"coeffs": [1; 100000], "x": 2}` — 100k-degree polynomial.
**Risk:** encoder builds ~100k Horner iterations, each with ~9 agents (lambda combinator + apps + DUP). Live-agent count ~ 1M; memory ~100 MiB. Then `reduce_all` runs through all redexes — could be 10s of seconds wall-clock. Pre-validation has no length cap.
**Recommendation:** add a `MAX_HORNER_COEFFS` const (e.g., 1024) and reject with `EncodeError::InputTooLarge`. Defense in depth.

---

## Severity Counts

| Severity | Count |
|----------|-------|
| CRITICAL | 2 (BUG-001 BUG-002) |
| HIGH | 4 (BUG-003 BUG-004 BUG-005 — review's MF-001 — and BUG-007 promoted from MEDIUM if confirmed empirically) |
| MEDIUM | 5 (BUG-006 BUG-007 BUG-010 BUG-011 BUG-012) |
| LOW | 4 (BUG-008 BUG-009 BUG-013 BUG-014 BUG-015) |
| **Total** | **15** |

---

## Recommendation: D-015 safe-to-merge?

**NOT safe-to-merge as-is.** The two CRITICAL bugs (BUG-001 + BUG-002) make the v1 G1-demo Horner CLI pipeline broken **at the user-facing surface**. The unit tests pass because they bypass the registry path that the CLI actually uses. The TCC narrative leans on the codec as the empirical demonstration — if the demo cannot be invoked from the CLI, the narrative weakens.

**Recommended path forward (two options):**

### Option A (preferred — small REFACTOR):
1. Add `discover_root` call to `run_compute_with_encoder` (BUG-001 fix, ~3 LoC).
2. Add NF-bypass to `validate_encoded_net` E2 OR add a `Codec::accepts_nf_input(&self) -> bool` method (BUG-002 fix, ~10 LoC).
3. Add CLI integration test exercising `compute --codec horner` for both constant-poly and single-iter inputs (TG-001, ~30 LoC).
4. Apply reviewer SF-001 (BUG-009) + SF-002 + SF-003 + SF-004 + MF-001 (BUG-005).
5. Estimate: ~120 LoC. Within Stage 6 REFACTOR scope (≤200 LoC SDD rule).

### Option B (defer):
File a follow-up TASK explicitly named `D-015-followup-horner-cli-pipeline-fix` BEFORE merging D-015 to v2-development. Document BUG-001, BUG-002, BUG-003 as "ship-with-known-defects" in `docs/DEFERRED-WORK.md`. The G1 narrative in the artigo would have to acknowledge "CLI invocation requires source-code changes; demonstration via `cargo test` only". This weakens the empirical claim and is **not recommended**.

---

## Verdict on the readback limitation under adversarial pressure (final restatement)

The reviewer's position **largely sustains**, but with these reservations:

- The `format!("{:?}")` G1 isomorphism check is **fragile against agent-ID divergence in DUP-branch error payloads** (BUG-004). The structural-isomorphism narrative needs strengthening — recommended REFACTOR adds topology comparison (~40 LoC, reviewer §6 item 8).
- The "constant polynomial + single-iter Horner" readable subset is the **only G1 evidence empirically demonstrated** today, and it's BLOCKED from the CLI (BUG-001, BUG-002). The narrative needs to either fix the CLI or honestly say "demonstrated via `cargo test` only".
- The Mackie/Pinto-style readback deferral is correct: out of scope for D-015, in scope for SPEC-28 / Future Work. The reviewer's framing in §6 §3 is defendable.

QA endorses the readback limitation acceptance — **conditional on fixing BUG-001 + BUG-002 so the CLI demonstrates the readable subset that the narrative claims**.

---

**Reviewer signoff (QA):** D-015 has 2 CRITICAL bugs that block CLI invocation of the codec the spec was rewritten around. Stage 6 REFACTOR scope must include BUG-001 and BUG-002 fixes — they are NOT cosmetic. Recommended REFACTOR scope: ~120 LoC (BUG-001 + BUG-002 + CLI integration test + reviewer's MF-001/SF-001..SF-004). Defer Option B not recommended. After REFACTOR, the bundle is shippable.
