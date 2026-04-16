# TEST-SPEC-0336: EncoderRegistry struct + ops

**Task:** TASK-0336
**Spec:** SPEC-27 R17, R18, R20
**Generated:** 2026-04-16

---

## R1: empty registry has zero codecs
```rust
let r = EncoderRegistry::new();
assert!(r.list().is_empty());
assert!(r.get("anything").is_none());
```

## R2: register + get round-trip
```rust
let mut r = EncoderRegistry::new();
r.register(Box::new(LambdaCodec::new())).unwrap();
let c = r.get("lambda").unwrap();
assert_eq!(c.name(), "lambda");
```

## R3: duplicate name rejected
```rust
let mut r = EncoderRegistry::new();
r.register(Box::new(LambdaCodec::new())).unwrap();
let err = r.register(Box::new(LambdaCodec::new())).unwrap_err();
assert!(matches!(err, RegistryError::DuplicateName(n) if n == "lambda"));
```

## R4: list() is sorted by name and reports descriptions
```rust
let mut r = EncoderRegistry::new();
r.register(Box::new(LambdaCodec::new())).unwrap();
r.register(Box::new(ChurchArithmeticCodec::new(ChurchOp::Add))).unwrap();
let l = r.list();
assert_eq!(l[0].0, "church_add");
assert_eq!(l[1].0, "lambda");
assert!(!l[0].1.is_empty()); // has a description
```

## R5: encode_and_validate returns Ok for valid input
```rust
let mut r = EncoderRegistry::new();
r.register(Box::new(LambdaCodec::new())).unwrap();
let net = r.encode_and_validate("lambda", r#"{"term":"λx. x"}"#.as_bytes()).unwrap();
assert!(net.count_live_agents() >= 1);
```

## R6: encode_and_validate rejects unknown encoder
```rust
let r = EncoderRegistry::new();
let err = r
    .encode_and_validate("nope", b"{}")
    .unwrap_err();
assert!(matches!(err, RegistryError::NotFound(n) if n == "nope"));
```

## R7: encode_and_validate propagates encoder errors
```rust
let mut r = EncoderRegistry::new();
r.register(Box::new(LambdaCodec::new())).unwrap();
let err = r.encode_and_validate("lambda", b"{not json}").unwrap_err();
assert!(matches!(err, RegistryError::Encode(EncodeError::InvalidInput(_))));
```

## R8: decode rejects unknown encoder
```rust
let r = EncoderRegistry::new();
let err = r.decode("nope", &Net::new()).unwrap_err();
assert!(matches!(err, RegistryError::NotFound(_)));
```

## Acceptance Criteria

1. `cargo test --workspace` count: 758 → 765+ (≥ +7).
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo fmt --check` clean.
