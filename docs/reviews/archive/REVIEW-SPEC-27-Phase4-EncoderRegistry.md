# REVIEW: SPEC-27 Phase 4 — EncoderRegistry

**Date:** 2026-04-16
**Tasks:** TASK-0336, TASK-0337
**Spec requirements:** R17, R18, R19, R20
**Files reviewed:** `relativist-core/src/encoding/registry.rs`, `relativist-core/src/encoding/mod.rs`
**Test count:** 758 → 771 (+13)
**Verdict:** APPROVE — no Must-Fix items.

---

## Spec compliance

| Req | Requirement | Status | Evidence |
|-----|-------------|--------|----------|
| R17 | `EncoderRegistry` keyed by `Codec::name()` | ✅ | `HashMap<String, Box<dyn Codec>>`, registry.rs:18 |
| R18 | `encode_and_validate` calls codec then `validate_encoded_net` | ✅ | registry.rs:68-75 |
| R19 | `default_registry()` returns 5 built-ins | ✅ | registry.rs:92-106; sorted names verified at registry.rs:206-216 |
| R20 | `RegistryError` with DuplicateName / NotFound / Encode / Decode | ✅ | registry.rs:22-32 |

## Code quality

- Trait-object dispatch via `Box<dyn Codec>` — required for runtime extensibility.
- `register()` checks for duplicate before insert (not `entry().or_insert()` swap),
  so the new codec is dropped on collision rather than replacing — correct
  R20 semantics.
- `list()` sorts by name → deterministic CLI output (acceptance criterion).
- `encode_and_validate` is the only path into `validate_encoded_net` from CLI;
  centralises the contract check so callers can't forget it (R6 by construction).
- `decode()` propagates `DecodeError` via `#[from]` — symmetric with `Encode`.
- `Default` impl delegates to `new()` (clippy-friendly).
- `default_registry()` uses `.expect()` with explanatory messages; safe because
  each codec has a distinct, hardcoded `name()` and the registry starts empty.

## Architecture

- Module placement: `encoding::registry` sits beside `traits`, `codec_church`,
  `codec_lambda` — natural home; no cross-layer leakage.
- Pure module: no I/O, no async, no global state. Coordinator/CLI will own a
  registry instance (Phase 5).
- Object-safety: `Codec: Encoder + Decoder` requires `Send + Sync`, so registry
  is trivially `Send + Sync` and can be shared across worker threads if needed.
- No premature abstraction over registration source (file, plugin, etc.) —
  matches DISC-012 v2 scope (Layer 2 only).

## Test coverage (13 inline tests)

TEST-SPEC-0336 (R1-R8):
- R1: empty registry — `empty_registry_has_no_codecs`
- R2: register + get — `register_and_get_round_trip`
- R3: duplicate rejected — `register_duplicate_name_rejected`
- R4: list sorted with descriptions — `list_is_sorted_with_descriptions`
- R5: encode_and_validate happy path — `encode_and_validate_returns_net_on_valid_input`
  (uses `(λx. x) (λy. y)` so net has CON-CON redex and passes E2)
- R6: unknown encoder — `encode_and_validate_rejects_unknown_encoder`
- R7: encoder errors propagate — `encode_and_validate_propagates_encoder_errors`
- R8: decode unknown encoder — `decode_rejects_unknown_encoder`

TEST-SPEC-0337 (D1-D5):
- D1: 5 codecs — `default_registry_has_five_codecs`
- D2: names sorted — `default_registry_names_match_spec`
- D3: Church round-trip — `default_registry_church_codecs_round_trip`
  (covers add=8, mul=12, sum_of_squares=14; see Should-Fix below for exp)
- D4: lambda round-trip — `default_registry_lambda_round_trips_identity`
- D5: descriptions non-empty — `default_registry_descriptions_non_empty`

All 13 pass; no flakiness observed across 3 consecutive `cargo test --workspace`
runs.

## Should-Fix (deferred, not blocking)

1. **`church_exp` decode skipped in round-trip test.** Pre-existing SPEC-14
   limitation: `decode_nat_or_shared` does not handle the DUP-cycle structure
   left by `build_exp` after reduction (memory: project_phase11_encoding_status).
   The Phase 4 test verifies `church_exp` encodes + validates but skips decode.
   Fix belongs in SPEC-14 (recursive readback for DUP cycles), not SPEC-27.
   When fixed, restore exp to the round-trip case set in registry.rs:218-243.

2. **No test for `register()` insert ordering after collision.** Implicitly
   covered (the original codec stays because we early-return on collision),
   but a dedicated assertion would document the invariant. ~5 LoC if desired.

3. **`get()` returns `Option<&dyn Codec>` but no `get_mut`.** Out of scope for
   v2 (codecs are immutable post-registration), but worth noting if a future
   spec wants codec configuration.

## Out of scope (explicitly)

- CLI `encoders list` subcommand — Phase 5 (TASK-0339).
- CLI `compute --encoder <name>` flag — Phase 5 (TASK-0338).
- `RecipeEncoder` extension — Phase 6.
- Plugin loading from disk — not in DISC-012 v2 scope.

## Verification

```
cargo build --workspace          → clean
cargo test --workspace           → 771 passed, 0 failed (was 758, +13)
cargo clippy --workspace --all-targets -- -D warnings → clean
cargo fmt --check                → clean
```

## Verdict

**APPROVE.** Phase 4 is implementation-complete and ready for QA stage.
