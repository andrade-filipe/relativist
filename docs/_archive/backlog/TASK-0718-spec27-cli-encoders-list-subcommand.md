# TASK-0718 — SPEC-27 v3 R22: `encoders list` (and `codecs list` alias) CLI subcommand

**Spec:** SPEC-27 v3
**Requirements:** R22 (`encoders list` MUST; `codecs list` MAY as alias)
**Priority:** P1
**Status:** TODO
**Depends on:** TASK-0716 (default_registry contains the canonical 5 v3 codecs)
**Blocked by:** none
**Estimated complexity:** S (~30 LoC production + ~40 LoC tests)
**Bundle:** SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)

---

## Context

SPEC-27 v3 R22 requires a CLI subcommand that lists the available encoders
with their descriptions:

```
$ relativist encoders list
Available encoders:
  church_add            Church numeral addition (a + b)
  church_mul            Church numeral multiplication (a × b)
  church_exp            Church numeral exponentiation (a ^ b)
  church_sum_of_squares Sum of squares (1² + 2² + ... + n²)
  horner                Polynomial evaluation via Horner's method
```

The subcommand MAY also be invoked as `relativist codecs list` (clap alias for
terminological symmetry with R21). The existing CLI already has an `Encoders`
subcommand stub (`relativist-cli/src/main.rs:67` references
`commands::run_encoders_command`). This task verifies that the existing stub
matches v3 R22 output (post-TASK-0716 registry contents), adds the `codecs list`
alias, and adds tests.

## Acceptance Criteria

- [ ] `relativist encoders list` outputs exactly 5 lines (one per codec) in the canonical R19 order; each line has the codec name and description from `Codec::description()`.
- [ ] `relativist codecs list` is accepted as an alias and produces byte-identical output (R22 MAY).
- [ ] Output format: name padded to a column-aligned width (e.g., 22 chars), then description; matches the §3.6 R22 example.
- [ ] No empty lines or trailing whitespace beyond a single trailing newline.
- [ ] If `LambdaCodec` is registered (user-side), it shows up at the bottom — but the default registry (TASK-0716) excludes it, so the default output has 5 entries.

## Files to Create / Modify

| File | Kind | Change |
|------|------|--------|
| `relativist-core/src/config.rs` | modify | Add `Codecs(EncodersArgs)` variant to `Command` enum (or use clap subcommand alias on the existing `Encoders` variant). ~5 LoC. |
| `relativist-core/src/commands.rs` | modify | If a separate variant: route `Codecs` to `run_encoders_command` (same handler). If using clap subcommand alias: update rustdoc only. ~5 LoC + ~5 LoC tests for alias. |
| `relativist-cli/src/main.rs` | modify | Add `Command::Codecs(args) => commands::run_encoders_command(args),` if a separate variant. ~1 line. |

## Key Types / Signatures

Preferred approach (clap subcommand alias on the same variant):

```rust
#[derive(clap::Subcommand)]
pub enum Command {
    // ... existing variants ...

    /// List available encoders. Alias: `codecs list` (SPEC-27 v3 R22).
    #[command(alias = "codecs")]
    Encoders(EncodersArgs),
}
```

Output format (existing or new in `run_encoders_command`):

```
Available encoders:
  <name padded to 22 chars> <description>
```

## Test Expectations

For Stage 2 (test-generator) — maps to SPEC-27 v3 §7.5 T21:

- `cli_encoders_list_outputs_5_v3_codecs` — `relativist encoders list` stdout contains exactly 5 codec lines in R19 order; first line is `Available encoders:`.
- `cli_codecs_list_alias_produces_identical_output` — `relativist codecs list` stdout byte-identical to `relativist encoders list` (T21 alias).
- `cli_encoders_list_excludes_lambda` — output does NOT contain the substring `lambda` (T16-derived, post-TASK-0716).
- `cli_encoders_list_includes_horner_and_description` — output contains a line starting with `horner` followed by the description string from `HornerCodec::description()`.

## Dependencies Context

- `default_registry()` from TASK-0716 (5 codecs).
- `EncoderRegistry::list() -> Vec<(&str, &str)>` already exists.
- `clap` subcommand `alias` attribute (clap 4.x).

## Notes

- Two clap mechanisms can satisfy R22's "MAY also be invoked as `codecs list`":
  (a) `#[command(alias = "codecs")]` on the same variant (single handler;
  preferred — fewer code paths); (b) two separate variants both routing to
  `run_encoders_command` (more verbose). Choose (a) unless a Stage 4 reviewer
  finds it incompatible with existing CLI structure.
- Output formatting (column padding) is editorial. The R22 example uses ~22
  characters; the test verifies content, not exact whitespace.
- Test floor delta: +4 unit/integration tests.
