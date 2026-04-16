# TEST-SPEC-0339: encoders list subcommand

**Task:** TASK-0339
**Spec:** SPEC-27 R22
**Generated:** 2026-04-16

---

## E1: `encoders list` parses
```rust
let cli = Cli::try_parse_from(["relativist", "encoders", "list"]).unwrap();
assert!(matches!(cli.command, Command::Encoders(EncodersArgs {
    action: EncodersAction::List
})));
```

## E2: `encoders` without action prints help (returns Err with help text)
```rust
let res = Cli::try_parse_from(["relativist", "encoders"]);
assert!(res.is_err()); // clap's MissingSubcommand or DisplayHelp
```

## E3: handler enumerates 5 codecs
The handler is hard to test directly (writes to stdout). Instead, assert that
`default_registry().list().len() == 5` (already covered by TEST-SPEC-0337 D1)
and that the handler iterates that list.

## Acceptance Criteria

1. `cargo test --workspace` count: 781+ → 784+ (≥ +3 parsing tests).
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo fmt --check` clean.
4. Manual smoke: `relativist encoders list` prints 5 lines.
