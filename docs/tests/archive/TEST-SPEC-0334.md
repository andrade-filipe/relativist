# TEST-SPEC-0334: LambdaCodec decoder

**Task:** TASK-0334
**Spec:** SPEC-27 R14-R15
**Generated:** 2026-04-16

---

## Unit Tests (decoder)

### D1: decode identity (no reduction needed)
```rust
let net = codec.encode(br#"{"term":"λx. x"}"#).unwrap();
let out = codec.decode(&net).unwrap();
let s = out["term"].as_str().unwrap();
assert!(alpha_equiv(s, "λx. x"));
assert!(out["agents"].as_u64().unwrap() >= 2);
```

### D2: decode after beta-reduction yields identity
```rust
let mut net = codec.encode(br#"{"term":"(λx. x) (λy. y)"}"#).unwrap();
reduce_all(&mut net);
let out = codec.decode(&net).unwrap();
assert!(alpha_equiv(out["term"].as_str().unwrap(), "λy. y"));
```

### D3: decoder rejects malformed nets
```rust
let mut empty = Net::new();
empty.create_agent(Symbol::Era);  // stray agent, no entry port
let err = codec.decode(&empty).unwrap_err();
assert!(matches!(err, DecodeError::UnrecognizedStructure(_)));
```

### D4: decoder is read-only
```rust
let net = codec.encode(br#"{"term":"λx. x"}"#).unwrap();
let snapshot = net.clone();
let _ = codec.decode(&net).unwrap();
assert_eq!(net, snapshot, "decode must not mutate input net");
```

### D5: output schema fields present
```rust
let net = codec.encode(br#"{"term":"λx. x"}"#).unwrap();
let out = codec.decode(&net).unwrap();
assert!(out.get("term").is_some());
assert!(out.get("agents").is_some());
assert!(out.get("interactions").is_some()); // null is acceptable here
```

## Pretty-printer Tests

### PP1: lambda body without redundant parens
```rust
let t = parse_term("λx. λy. x").unwrap();
let s = print_term(&t);
assert_eq!(s, "λx. λy. x");
```

### PP2: application parens when right operand is App
```rust
let t = parse_term("a (b c)").unwrap();
assert_eq!(print_term(&t), "a (b c)");
```

### PP3: erased binder rendered as `_`
```rust
let t = Term::Lam("_".into(), Box::new(Term::Var("y".into())));
assert_eq!(print_term(&t), "λ_. y");
```

## Codec object-safety

### O1: Box<dyn Codec> works
```rust
let boxed: Box<dyn Codec> = Box::new(LambdaCodec::new());
assert_eq!(boxed.name(), "lambda");
let _ = boxed.description();
```

## Helper

```rust
/// Alpha-equivalence: rename bound variables in canonical (de Bruijn-like)
/// order before string comparison. Sufficient for the small terms in T5-T9.
fn alpha_equiv(a: &str, b: &str) -> bool {
    canonicalise(parse_term(a).unwrap()) == canonicalise(parse_term(b).unwrap())
}
```
