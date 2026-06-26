# TASK-0729: `decode` subcommand — read reduced `.bin`, run codec decoder, print JSON

**Spec:** SPEC-12 (formats), SPEC-27 v3 R14'/R15'/R16' (decode contract)
**Bundle:** D-017
**Priority:** P0 (final stage of multi-container demo — without it the reduced `.bin` from the coordinator is opaque)
**Status:** TODO
**Depends on:** TASK-0728 (paired half; useful to land them together) — but no code dependency
**Blocked by:** none
**Estimated complexity:** S (≈50–80 LoC prod)

## Context

Once the coordinator writes the reduced `.bin` to its volume (`/data/horner_reduced.bin`), we need a host-side step to convert it back to the user-visible numeric value. The decode logic already exists (`EncoderRegistry::decode(name, &net) -> Result<serde_json::Value, RelativistError>`, called today from `run_compute_with_encoder` at `commands.rs:806`); this task lifts it into a standalone `decode` subcommand so the multi-container demo (TASK-0730) can stitch the pieces.

Critical invariant — replicate the post-reduce root recovery from `run_compute_with_encoder` (commands.rs:797–804): codecs that compose Church arithmetic via `wire_*_into` (HornerCodec) emit nets with `root = None`; `discover_root(&mut net)` must run before `registry.decode` or decoding errors out with "no root agent".

## Acceptance Criteria

- [ ] `relativist decode --codec horner --input ./reduced.bin` loads the net, recovers root if missing, decodes, and prints pretty JSON to stdout (same format as `run_compute_with_encoder` line 807–809).
- [ ] `--encoder` accepted as alias for `--codec` (symmetric with `compute --encoder/--codec`, SPEC-27 R21); mutually exclusive via clap `conflicts_with`.
- [ ] `--output <path>` (optional) writes the JSON to a file instead of stdout.
- [ ] Unknown codec name returns a Config error listing available codecs (mirrors `run_compute_with_encoder` error path).
- [ ] Reading a corrupt `.bin` returns a Config error mentioning the path and the underlying bincode error.

## Files to Create/Modify

- `relativist-core/src/config.rs` — add to `Command` enum: `Decode(DecodeArgs)` variant; define `DecodeArgs` (codec/encoder/input/output fields, mirror `ComputeArgs` flag rules).
- `relativist-core/src/commands.rs` — add `pub fn run_decode_command(args: DecodeArgs) -> Result<(), RelativistError>`. Body: load via `io::binary::load_bin`, `discover_root` if `net.root.is_none()`, `registry.decode(name, &net)`, serialize pretty, write/print.
- `relativist-core/src/main.rs` (or wherever `Cli::run` dispatches) — add the `Command::Decode(args) => run_decode_command(args)` arm.

## Key Types / Signatures

```rust
// config.rs
#[derive(clap::Args, Debug)]
pub struct DecodeArgs {
    /// Codec name from the registry (e.g., "horner").
    #[arg(long, value_name = "NAME")]
    pub codec: Option<String>,

    /// Alias of --codec. Mutually exclusive.
    #[arg(long, value_name = "NAME", conflicts_with = "codec")]
    pub encoder: Option<String>,

    /// Path to a bincode-v2 `.bin` (typically the reduced net output of a coordinator).
    #[arg(short = 'i', long)]
    pub input: std::path::PathBuf,

    /// Optional path to write the JSON result. If absent, prints to stdout.
    #[arg(short = 'o', long)]
    pub output: Option<std::path::PathBuf>,
}

// commands.rs
pub fn run_decode_command(args: DecodeArgs) -> Result<(), RelativistError> {
    let name = args.codec.as_deref().or(args.encoder.as_deref())
        .ok_or_else(|| RelativistError::Config(
            "decode requires --codec or --encoder".to_string()))?;
    let mut net = crate::io::binary::load_bin(&args.input)?;
    if net.root.is_none() {
        crate::encoding::discover_root(&mut net);
    }
    let registry = crate::encoding::default_registry();
    let json_out = registry.decode(name, &net)?;
    let pretty = serde_json::to_string_pretty(&json_out)
        .map_err(|e| RelativistError::Encoding(format!("serialize result: {}", e)))?;
    match args.output {
        Some(ref path) => std::fs::write(path, &pretty)?,
        None => println!("{}", pretty),
    }
    Ok(())
}
```

## Test Expectations (for test-generator)

- Unit test (config): `decode --codec horner --input x.bin` parses; both `codec` and `encoder` together fails.
- Integration test: round-trip — encode Horner input → save → load → decode → JSON matches `run_compute_with_encoder` output for the same input (post-reduce). Use `tempfile`.
- Integration test: missing root recovery — construct a Net with `root = None` that decodes correctly only after `discover_root`; confirm `run_decode_command` succeeds without manual recovery.
- Integration test: corrupt `.bin` (random bytes) returns Config error.
- Integration test: unknown codec name returns Config error.

## Dependencies Context

- `io::binary::load_bin` exists (`io/binary.rs:22`).
- `discover_root` is `pub` from `encoding` module (used at `commands.rs:798`).
- `EncoderRegistry::decode` returns `serde_json::Value` (TASK-0715 contract).
- Pipeline contract for `root = None` is documented in `run_compute_with_encoder` comments — copy the rationale into a doc comment on `run_decode_command`.

## Notes

- Output to a file (`--output`) is what the multi-container demo (TASK-0730) will pipe into a teardown summary; printing to stdout is what an operator does interactively.
- This subcommand intentionally does NOT reduce. If the user passes a non-reduced net, `registry.decode` will likely return an "not normal form" error (SPEC-27 v3 R4 `NotNormalForm.redexes`). That's the right behaviour — it tells the operator their pipeline didn't actually run the coordinator.
