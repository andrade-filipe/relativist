# REVIEW: SPEC-27 Phase 3 — LambdaCodec

**Stage:** 4 REVIEW
**Date:** 2026-04-16
**Reviewer:** sdd-pipeline (manual orchestration, Option B)
**Files reviewed:**
- `relativist-core/src/encoding/codec_lambda.rs` (~830 LoC inc. 26 tests)
- `relativist-core/src/encoding/mod.rs` (added module + re-exports)
- `relativist-core/src/reduction/rules.rs` (engine fix to `interact_comm`)

---

## Verdict: **APPROVE with no Must-Fix issues.**

All acceptance criteria met. Implementation is clean, well-commented, and within
the SPEC-27 R10-R16 scope. The engine fix to `interact_comm` is justified as
necessary for sound reduction of identity-typed arguments and mirrors the
existing self-loop guards in `interact_anni` / `interact_eras`.

---

## Code Quality

| Check | Status | Notes |
|-------|--------|-------|
| No `unwrap()` in production code | ✅ | Only in `#[cfg(test)]` |
| No `println!` / `eprintln!` | ✅ | None present |
| Errors via `thiserror`-derived types | ✅ | Uses existing `EncodeError` / `DecodeError` |
| `pub(crate)` unless truly public | ✅ | Public API: `LambdaCodec`, `Term`, `parse_term`, `print_term`, `encode_lambda`, `decode_lambda` |
| Newtype IDs / `#[derive(Debug, Clone, ...)]` | ✅ | `Term` derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` |
| IC concept comments | ✅ | Module-level comment maps Mackie/Pinto pipeline to ports; binder fan-out documented |
| `#[derive(Default)]` for codec | ✅ | `LambdaCodec` is `Copy + Default` |

## Architecture

- Placed in `encoding::codec_lambda` (consistent with `codec_church`).
- Depends only on `net::*` and `encoding::traits` (no cross-module reach).
- Object-safe (`Box<dyn Codec>` test passes).
- No async / no I/O — pure layer, unaffected by tokio (matches SPEC-13 dependency direction).

## SPEC-27 Compliance (R10-R16)

| Req | Status | Evidence |
|-----|--------|----------|
| R10 | ✅ | `LambdaCodec` struct + `Encoder` + `Decoder` + `Codec` impls |
| R11 | ✅ | `Term::{Var, Lam, App}` (minimal grammar) |
| R12 | ✅ | Dual input: `{"term": "..."}` (parser) + `{"ast": {...}}` (serde) |
| R13 | ✅ | REF-005 mapping: Lam→CON(p0=out, p1=binder, p2=body), App→CON, n-uses→DUP tree, 0-use→ERA |
| R14 | ✅ | Port-directed readback via `find_entry_port(0)` + recursive `readback_inner` |
| R15 | ✅ | Output `{"term": String, "agents": usize, "interactions": null}` |
| R16 | ✅ | T5-T9 round-trips + N1-N3 negatives all pass |

## Test Coverage

- 26 inline tests, +24 over baseline 726 → 752 (TEST-SPEC required ≥+5).
- Categories: parser (5), encoder (6), decoder (3), pretty-printer (3),
  edge cases T5-T9 (5), negatives N1-N3 (3), object safety (1).
- `cargo test --workspace`: 752 passing, 0 failed, 0 ignored.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.

## Engine Fix (rules.rs::interact_comm)

**Change:** added self-loop guards for both CON and DUP sides of CON-DUP commutation.

**Why required:**
- The encoder produces self-loop CON for the identity term `λx. x`.
- When such a CON appears as the argument of an application that triggers
  CON-DUP commutation (T7, T9), the previous implementation left the new
  agents' principal ports DISCONNECTED (T1 violation).
- This was a latent engine bug — `interact_anni` and `interact_eras` already
  carried analogous self-loop guards; `interact_comm` was the missing one.

**Semantics:**
- CON self-loop = identity. Duplicating identity yields two identities, so
  the fix creates two new self-loop CONs on the DUP's output wires.
- DUP self-loop = short-circuit. Symmetric handling on the CON's output wires.
- Both self-looped: nothing external to wire; both removed.

**Risk assessment:** none. The fix only triggers on a previously undefined
case (T1 violation); no existing test relied on the broken behavior.
Confirmed by 690 v1 tests + 36 v2 tests still passing.

## Suggestions (Should-Fix, deferred to Stage 6 REFACTOR or future work)

1. **Free-variable name preservation.** `encode_term` discards the name of free
   variables (assigns an unrelated `FreePort(fid)`); on decode they appear as
   `free_<fid>`. For round-tripping open terms, a free-var registry would
   help. Out of scope for T5-T9 (all closed terms).
2. **DUP cycles in pathological encodings.** `MAX_READBACK_DEPTH = 10_000`
   guards the loop; a longer-term solution is a proper port-walking algorithm
   that reuses var-map for shared sub-terms. Acceptable for the 5 round-trip
   targets in scope.
3. **Lambda-on-left parens in pretty-printer** were missing in initial draft;
   fixed during DEV (commit pending) — flag this as a regression-test target
   for QA.

## QA Hand-off

QA stage should focus on:
- Adversarial terms with repeated bindings (`λx. λx. x` shadowing).
- Deeply nested applications (`a b c d e ... z`) for parser correctness.
- Mixed Greek/ASCII/`lambda` syntax in same input.
- Terms that produce DUP cycles after reduction (Y-combinator, omega).
- Decoder on partially-reduced nets (intermediate state).
