# TASK-0717 — SPEC-27 v3 R21, R23: CLI `compute --encoder`/`--codec` with `conflicts_with`

**Spec:** SPEC-27 v3
**Requirements:** R21 (dual-form flag with `conflicts_with`, NOT `aliases`), R23 (`compute` pipeline `encode → validate → reduce_all → decode → print JSON`)
**Priority:** P0 (closes Phase 5; user-facing CLI surface for HornerCodec)
**Status:** TODO
**Depends on:** TASK-0716 (default_registry has `horner`; lacks `lambda`)
**Blocked by:** none
**Estimated complexity:** S–M (~60 LoC production + ~80 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

The current `ComputeArgs` (`relativist-core/src/config.rs:532`) exposes only
`--encoder` plus `--input`. SPEC-27 v3 R21 (closure of SC-008) requires **both**
`--encoder` and `--codec` flags with **mutual exclusion** via clap's
`conflicts_with` attribute, NOT the `aliases(...)` pattern (which silently
keeps the last value). Both flags MUST appear separately in `--help` output;
the application logic coalesces the two `Option<String>` fields into a single
codec name.

Behavior matrix (SPEC-27 v3 §3.6):

| Invocation | Result |
|------------|--------|
| `--encoder horner --input '<json>'` | OK — codec = `"horner"` |
| `--codec horner --input '<json>'` | OK — codec = `"horner"`, identical pipeline |
| `--encoder horner --codec horner` | clap conflict error (T20) |
| (neither flag, positional fallback) | Legacy positional `compute add 3 5` form preserved (R21 fallback) |

R23 mandates the pipeline order `encode → validate → reduce_all → decode → print JSON`.
The existing `commands::run_compute_command` already implements this when
`--encoder` is set — this task ensures that adding `--codec` does NOT change
the pipeline; it only changes the flag-parsing surface.

## Acceptance Criteria

- [ ] `ComputeArgs` adds a new `pub codec: Option<String>` field with `#[arg(long = "codec", value_name = "NAME", conflicts_with = "encoder")]`.
- [ ] Existing `pub encoder: Option<String>` field gets `conflicts_with = "codec"` added to its `#[arg]` attribute.
- [ ] `--help` lists `--encoder` and `--codec` as separate entries (verified by snapshot test).
- [ ] `commands::run_compute_command` coalesces `args.encoder.or(args.codec)` into a single codec name; pipeline behavior unchanged when only one flag is set.
- [ ] Passing both flags produces a clap conflict error with `ErrorKind::ArgumentConflict` (exit code 2 on most platforms); error message mentions both flag names.
- [ ] Backward-compatible positional fallback (`compute add 3 5`) preserved (R21 fallback / R7).

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/config.rs` | modify | Add `pub codec: Option<String>` field with `conflicts_with = "encoder"`; update `pub encoder` to include `conflicts_with = "codec"`. Update rustdoc to cite SPEC-27 v3 R21. ~10 LoC. |
| `relativist-core/src/commands.rs` | modify | In `run_compute_command`: coalesce `args.encoder.or(args.codec)` into a single `codec_name: Option<String>`. Update existing `--encoder` references in error messages to mention `--encoder/--codec`. ~10 LoC. |
| `relativist-core/src/config.rs` (tests) | modify | Add tests for the conflicts behavior; update existing `--encoder`-only tests to also exercise `--codec`. ~80 LoC. |

## Key Types / Signatures

```rust
#[derive(clap::Args, Debug)]
pub struct ComputeArgs {
    /// Arithmetic operation (legacy SPEC-14 path). Required when --encoder/--codec is omitted.
    pub op: Option<ArithmeticOp>,
    pub a: Option<u64>,
    pub b: Option<u64>,

    /// Codec name (preferred, matches SPEC-27 R1 trait name). Mutually exclusive with --codec.
    #[arg(long = "encoder", value_name = "NAME", conflicts_with = "codec")]
    pub encoder: Option<String>,

    /// Alternate spelling of --encoder; mutually exclusive.
    #[arg(long = "codec", value_name = "NAME", conflicts_with = "encoder")]
    pub codec: Option<String>,

    /// Encoder input as a JSON string. Required when --encoder/--codec is set. SPEC-27 R21.
    #[arg(long = "input", value_name = "JSON")]
    pub input: Option<String>,
}
```

Coalescing in `run_compute_command`:

```rust
let codec_name = args.encoder.clone().or(args.codec.clone());
match (codec_name, args.input.as_deref()) {
    (Some(name), Some(input)) => { /* registry path: encode → validate → reduce → decode → print */ }
    (Some(_), None) => { /* error: --input required when --encoder/--codec is set */ }
    (None, _) => { /* legacy positional path */ }
}
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.5 T17, T18, T19, T20, T21:

- `cli_legacy_positional_compute_add_3_5_unchanged` — `relativist compute add 3 5` produces the same output as before SPEC-27 v3 (T17).
- `cli_encoder_horner_input_json` — `relativist compute --encoder horner --input '{"coeffs":[3,2,5,1],"x":2}'` outputs `{"value":"35", "bit_length":6}` (T18).
- `cli_codec_horner_input_json_identical_to_encoder` — same input via `--codec` produces identical JSON output (T19).
- `cli_encoder_codec_both_set_returns_conflict_error` — `relativist compute --encoder horner --codec horner` returns `ErrorKind::ArgumentConflict`; stderr mentions both flag names (T20).
- `cli_codec_set_input_missing_returns_config_error` — `relativist compute --codec horner` (no `--input`) returns the `Config` error mentioning `--encoder/--codec` (parity with the existing `--encoder` rule).
- `clap_help_lists_encoder_and_codec_separately` — invoke with `--help` and assert both flag long-names appear as separate help entries (no aliasing).

## Dependencies Context

- `EncoderRegistry::encode_and_validate(name, input) -> Result<Net, RegistryError>` already exists.
- `EncoderRegistry::decode(name, net) -> Result<serde_json::Value, RegistryError>` already exists.
- `default_registry()` populated with `horner` after TASK-0716.
- `clap` already a dependency (`relativist-core/Cargo.toml`).
- Existing legacy positional path remains untouched — only the registry-name source changes.

## Notes

- The "Pattern note for SPEC-07" in SPEC-27 v3 §3.6 R21 trailing commentary
  recommends a separate task to register the dual-form flag pattern as a
  project-wide convention in SPEC-07. SPEC-27 v3 explicitly does NOT amend
  SPEC-07; that follow-up is **out of scope** for this bundle.
- The exit code for `ArgumentConflict` is platform-specific (typically 2 on
  Unix); the test asserts the `ErrorKind` programmatically, NOT the integer
  exit code.
- Existing tests at `config.rs:1186, 1210, 1240` reference SPEC-27 R21 for
  `--encoder`-only behavior — update those to cover the `--codec` path
  symmetrically (do NOT remove the `--encoder` cases).
- Snapshot of `--help` output: prefer a regex/contains assertion over a
  byte-exact snapshot to avoid spurious failures from clap version bumps.
