# TEST-SPEC-0335: LambdaCodec edge cases (T5-T9)

**Task:** TASK-0335
**Spec:** SPEC-27 R16, T5-T9
**Generated:** 2026-04-16

---

## Round-trip Tests

### T5: identity (no reduction)
```rust
let codec = LambdaCodec::new();
let net = codec.encode(br#"{"term":"λx. x"}"#).unwrap();
let out = codec.decode(&net).unwrap();
assert!(alpha_equiv(out["term"].as_str().unwrap(), "λx. x"));
```

### T6: single beta-reduction
```rust
let mut net = codec.encode(br#"{"term":"(λx. x) (λy. y)"}"#).unwrap();
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
assert!(alpha_equiv(out["term"].as_str().unwrap(), "λy. y"));
```

### T7: nested application (double-identity collapse)
```rust
let mut net = codec.encode(br#"{"term":"(λf. λx. f (f x)) (λy. y)"}"#).unwrap();
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
assert!(alpha_equiv(out["term"].as_str().unwrap(), "λx. x"));
```

### T8: erasure of unused binder
```rust
let mut net = codec.encode(br#"{"term":"(λx. λy. y) (λz. z)"}"#).unwrap();
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
assert!(alpha_equiv(out["term"].as_str().unwrap(), "λy. y"));
```

### T9: duplication via DUP-CON commutation
```rust
let mut net = codec.encode(br#"{"term":"(λx. x x) (λy. y)"}"#).unwrap();
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
assert!(alpha_equiv(out["term"].as_str().unwrap(), "λy. y"));
```

## Negative Tests

### N1: malformed JSON
```rust
let err = codec.encode(b"{not json}").unwrap_err();
assert!(matches!(err, EncodeError::InvalidInput(_)));
```

### N2: missing body after dot
```rust
let err = codec.encode(br#"{"term":"(λx."}"#).unwrap_err();
assert!(matches!(err, EncodeError::InvalidInput(msg) if msg.contains("parse")));
```

### N3: decode of foreign net structure
```rust
let mut foreign = Net::new();
foreign.create_agent(Symbol::Era); // stray, no FreePort(0) target
let err = codec.decode(&foreign).unwrap_err();
assert!(matches!(err, DecodeError::UnrecognizedStructure(_)));
```

## Acceptance Criteria

1. Total inline tests in `codec_lambda.rs`: ≥ 8 (T5-T9 + N1-N3).
2. `cargo test --workspace` count: 726 → 731+ (≥ +5).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean.
4. `cargo fmt --check` clean.
