# TEST-SPEC-0338: compute --encoder dispatch

**Task:** TASK-0338
**Spec:** SPEC-27 R21, R23
**Generated:** 2026-04-16

---

## C1: legacy positional path still parses
```rust
let cli = Cli::try_parse_from(["relativist", "compute", "add", "3", "5"]).unwrap();
match cli.command {
    Command::Compute(args) => {
        assert!(args.encoder.is_none());
        assert!(matches!(args.operation, Some(ArithmeticOp::Add)));
        assert_eq!(args.a, Some(3));
        assert_eq!(args.b, Some(5));
    }
    _ => panic!("expected Compute"),
}
```

## C2: --encoder + --input parses without positional
```rust
let cli = Cli::try_parse_from([
    "relativist", "compute", "--encoder", "lambda", "--input", r#"{"term":"λx. x"}"#,
]).unwrap();
match cli.command {
    Command::Compute(args) => {
        assert_eq!(args.encoder.as_deref(), Some("lambda"));
        assert!(args.input.is_some());
        assert!(args.operation.is_none());
    }
    _ => panic!(),
}
```

## C3: --input without --encoder rejected by clap
```rust
let res = Cli::try_parse_from([
    "relativist", "compute", "--input", "{}",
]);
assert!(res.is_err());
```

## C4: church_add round-trip via registry path
Integration test (or test in commands.rs):
```rust
// Use the registry directly to verify the pipeline match (cannot easily call
// run_compute_command in a test because it prints to stdout).
let r = default_registry();
let mut net = r.encode_and_validate("church_add", br#"{"a":3,"b":5}"#).unwrap();
reduce_all(&mut net);
let out = r.decode("church_add", &net).unwrap();
assert_eq!(out["result"].as_u64().unwrap(), 8);
```
*(This case overlaps with TEST-SPEC-0337 D3; counted there. Acceptance is that
the dispatch wiring in `commands.rs` calls these exact functions.)*

## C5: unknown encoder produces error
```rust
let r = default_registry();
let err = r.encode_and_validate("nope", b"{}").unwrap_err();
assert!(matches!(err, RegistryError::NotFound(_)));
```
*(Wiring test: `run_compute_command` must surface this as a `RelativistError`.)*

## Acceptance Criteria

1. `cargo test --workspace` count: 777 → 781+ (≥ +4 unit tests for arg parsing).
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo fmt --check` clean.
4. Manual smoke (run by developer): `relativist compute add 3 5` and
   `relativist compute --encoder church_add --input '{"a":3,"b":5}'` both succeed.
