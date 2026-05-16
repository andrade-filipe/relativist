# TASK-0728: `compute --encode-only --output <path>` — emit `.bin` without reducing

**Spec:** SPEC-12 (formats), SPEC-27 (encoder API) — no new requirement; this is a CLI convenience splitting the existing `encode → reduce → decode` pipeline.
**Bundle:** D-017 (Multi-container Horner distribution demo)
**Priority:** P0 (critical path — coordinator needs a `.bin` on disk to load)
**Status:** TODO
**Depends on:** none (uses existing `registry.encode_and_validate` + `io::binary::save_bin`)
**Blocked by:** none
**Estimated complexity:** S (≈80–120 LoC prod + arg plumbing)

## Context

D-017 needs a way to materialise a HornerCodec input (JSON polynomial) into the bincode-v2 `.bin` format the existing `coordinator` subcommand consumes via `--input`. Today the only path that runs the encoder is `compute --encoder horner --input <json>`, which **also** reduces and decodes in-process. We need a "stop after encode" mode so the produced net can be handed to a coordinator + worker fleet for the actual reduction.

The encoder pipeline already exists end-to-end (`relativist-core/src/commands.rs` lines 763–812 in `run_compute_with_encoder`); this task only adds a CLI flag that short-circuits after `registry.encode_and_validate(name, input)?` and writes the net via `io::binary::save_bin` (`relativist-core/src/io/binary.rs` lines 28–33).

## Acceptance Criteria

- [ ] `relativist compute --codec horner --input '<JSON>' --encode-only --output ./out.bin` produces a valid bincode-v2 `.bin` and does **not** call `reduce_all`.
- [ ] `--encode-only` requires `--output` (clap `requires = "output"`); missing `--output` fails at parse time with a clear error.
- [ ] `--encode-only` is rejected (Config error) when used with positional legacy `compute add 3 5` (no encoder name to dispatch).
- [ ] The produced `.bin` round-trips: `relativist inspect <out.bin>` reports the same `count_live_agents()` as a no-encode-only run reports under `Encoding:` line.
- [ ] Backward compatibility: omitting `--encode-only` keeps the existing encode→reduce→decode behaviour byte-identical.

## Files to Create/Modify

- `relativist-core/src/config.rs` — modify `ComputeArgs`: add `#[arg(long, requires = "output")] pub encode_only: bool`. The `output: Option<PathBuf>` field already exists (used today as "save reduced net"); we reuse it.
- `relativist-core/src/commands.rs` — modify `run_compute_command` and `run_compute_with_encoder`:
  - Pass `encode_only` + `output` through.
  - Inside `run_compute_with_encoder`, after `encode_and_validate`, if `encode_only`, call `crate::io::binary::save_bin(&net, output_path)?`, print one summary line (`Encoded: N agents → <path>`), and return `Ok(())`.

## Key Types / Signatures

```rust
// config.rs — add to ComputeArgs (preserve existing field order):
/// D-017 / TASK-0728: stop after encode; write the un-reduced net to --output.
/// Requires --output. Mutually compatible with --encoder/--codec; rejected for
/// legacy positional `compute <op> <a> <b>` (no encoder dispatched).
#[arg(long, requires = "output")]
pub encode_only: bool,

// commands.rs — extend run_compute_with_encoder signature:
fn run_compute_with_encoder(
    name: &str,
    input: &[u8],
    encode_only_output: Option<&std::path::Path>,
) -> Result<(), RelativistError> {
    // ... existing encode_and_validate ...
    if let Some(path) = encode_only_output {
        crate::io::binary::save_bin(&net, path)?;
        println!("Encoded:     {} agents -> {}", net.count_live_agents(), path.display());
        return Ok(());
    }
    // ... existing reduce + decode path unchanged ...
}
```

Caller adapts:
```rust
let encode_only = if args.encode_only { args.output.as_deref() } else { None };
return run_compute_with_encoder(name, input.as_bytes(), encode_only);
```

## Test Expectations (for test-generator)

- Unit test (config): `--encode-only` without `--output` fails clap parsing.
- Unit test (config): `--encode-only --output x.bin` parses; field round-trips.
- Integration test (commands): encode-only Horner input writes a non-empty `.bin`; `load_bin` round-trip yields the same `count_live_agents()` as `encode_and_validate` directly.
- Integration test (commands): legacy `compute add 3 5 --encode-only --output x.bin` returns a Config error mentioning "encoder" or "codec".
- Smoke: `--encode-only` path does **not** print the `Reduction:` line.

## Dependencies Context

- `EncoderRegistry::encode_and_validate(name, input) -> Result<Net, RelativistError>` already exists (TASK-0716, `encoding/registry.rs`).
- `io::binary::save_bin(net, path) -> Result<(), RelativistError>` already exists.
- The `output: Option<PathBuf>` field of `ComputeArgs` is the same flag — semantic switches: today it saves the **reduced** net post-decode; with `--encode-only` it saves the **un-reduced** net pre-reduce. The mode flag disambiguates intent.

## Notes

- This task DOES NOT introduce a new `encode` subcommand; reusing `compute` keeps the CLI surface small and avoids duplicating arg plumbing. If a future bundle wants `encode`/`decode` as top-level verbs (TASK-0729 introduces `decode` as a subcommand because there is no analogous existing dispatcher), splitting is cheap.
- HornerCodec encoder is deterministic (TASK-0714); the `.bin` is bit-stable across runs for the same input — important for the wrapper script (TASK-0730) to detect drift.
