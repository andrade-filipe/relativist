# TEST-SPEC-0337: default_registry() with 5 codecs

**Task:** TASK-0337
**Spec:** SPEC-27 R19
**Generated:** 2026-04-16

---

## D1: default_registry has exactly 5 codecs
```rust
let r = default_registry();
assert_eq!(r.list().len(), 5);
```

## D2: all 5 expected names are present and sorted
```rust
let r = default_registry();
let names: Vec<&str> = r.list().iter().map(|(n, _)| *n).collect();
assert_eq!(
    names,
    vec![
        "church_add",
        "church_exp",
        "church_mul",
        "church_sum_of_squares",
        "lambda",
    ]
);
```

## D3: each Church codec round-trips a valid input
```rust
let r = default_registry();
for (name, expected) in [
    ("church_add", 8u64),
    ("church_mul", 12u64),
    ("church_exp", 8u64),
] {
    let json = match name {
        "church_exp" => r#"{"base":2,"exponent":3}"#.as_bytes(),
        _ => r#"{"a":3,"b":5}"#.as_bytes(),
    };
    let mut net = r.encode_and_validate(name, json).unwrap();
    reduce_all(&mut net);
    let out = r.decode(name, &net).unwrap();
    assert_eq!(out["result"].as_u64().unwrap(), expected);
}
```

## D4: lambda codec round-trips the identity
```rust
let r = default_registry();
let net = r.encode_and_validate("lambda", r#"{"term":"λx. x"}"#.as_bytes()).unwrap();
let out = r.decode("lambda", &net).unwrap();
assert!(out["term"].as_str().unwrap().contains("λ"));
```

## D5: descriptions are non-empty
```rust
let r = default_registry();
for (_, desc) in r.list() {
    assert!(!desc.is_empty());
}
```

## Acceptance Criteria

1. `cargo test --workspace` count: 765+ → 770+ (≥ +5).
2. `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. `cargo fmt --check` clean.
