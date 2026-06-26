# TEST-SPEC-0333: LambdaCodec encoder

**Task:** TASK-0333
**Spec:** SPEC-27 R10-R13
**Generated:** 2026-04-16

---

## Unit Tests (parser)

### P1: parse identity
```rust
let t = parse_term("λx. x").unwrap();
assert!(matches!(t, Term::Lam(name, body) if name == "x" && matches!(*body, Term::Var(ref v) if v == "x")));
```

### P2: parse application left-associative
```rust
let t = parse_term("a b c").unwrap();
// (a b) c — App(App(Var a, Var b), Var c)
assert!(matches!(t, Term::App(ref f, _) if matches!(**f, Term::App(_, _))));
```

### P3: backslash and "lambda" alternative syntaxes accepted
```rust
assert_eq!(format!("{:?}", parse_term("\\x. x").unwrap()), format!("{:?}", parse_term("λx. x").unwrap()));
assert_eq!(format!("{:?}", parse_term("lambda x. x").unwrap()), format!("{:?}", parse_term("λx. x").unwrap()));
```

### P4: parens override application precedence
```rust
let t = parse_term("a (b c)").unwrap();
// App(Var a, App(Var b, Var c))
assert!(matches!(t, Term::App(_, ref a) if matches!(**a, Term::App(_, _))));
```

### P5: parse error returns InvalidInput
```rust
let codec = LambdaCodec::new();
let err = codec.encode(br#"{"term":"(λx."}"#).unwrap_err();
assert!(matches!(err, EncodeError::InvalidInput(msg) if msg.contains("parse")));
```

## Unit Tests (encoder)

### E1: identity yields exactly 2 CON, 0 redex
```rust
let net = codec.encode(br#"{"term":"λx. x"}"#).unwrap();
assert_eq!(count_con(&net), 2);
assert_eq!(count_dup(&net), 0);
assert_eq!(count_era(&net), 0);
assert!(net.redex_queue.is_empty());
```

### E2: beta-redex creates principal-principal pair
```rust
let net = codec.encode(br#"{"term":"(λx. x) (λy. y)"}"#).unwrap();
assert!(!net.redex_queue.is_empty(), "(λx.x)(λy.y) must contain a CON-CON redex");
```

### E3: unused binder produces ERA
```rust
let net = codec.encode(br#"{"term":"λx. λy. y"}"#).unwrap();
assert!(count_era(&net) >= 1, "unused x must be erased");
```

### E4: shared variable (n>1 uses) produces DUP
```rust
let net = codec.encode(br#"{"term":"λx. x x"}"#).unwrap();
assert!(count_dup(&net) >= 1, "x used twice must introduce a DUP");
```

### E5: JSON AST input is accepted (alternative to "term")
```rust
let json = br#"{"ast":{"Lam":["x",{"Var":"x"}]}}"#;
let net = codec.encode(json).unwrap();
assert_eq!(count_con(&net), 2);
```

### E6: encoder name and description
```rust
assert_eq!(codec.name(), "lambda");
assert!(codec.description().to_lowercase().contains("lambda"));
```

## Edge Cases

### EC1: Object-safe Box<dyn Encoder>
```rust
let boxed: Box<dyn Encoder> = Box::new(LambdaCodec::new());
assert_eq!(boxed.name(), "lambda");
```

### EC2: Pretty-print round-trip is structural
```rust
let t1 = parse_term("(λf. λx. f (f x)) (λy. y)").unwrap();
let s = print_term(&t1);
let t2 = parse_term(&s).unwrap();
assert_eq!(format!("{:?}", t1), format!("{:?}", t2));
```

### EC3: Free variable wires to FreePort
```rust
let net = codec.encode(br#"{"term":"f x"}"#).unwrap();
// f and x are free. Both should be tracked via FreePort.
let free_count = net.live_agents()
    .filter_map(|a| {
        let t0 = net.get_target(PortRef::AgentPort(a.id, 0));
        let t2 = net.get_target(PortRef::AgentPort(a.id, 2));
        if matches!(t0, PortRef::FreePort(_)) || matches!(t2, PortRef::FreePort(_)) { Some(()) } else { None }
    })
    .count();
assert!(free_count >= 1);
```

## Helpers

```rust
fn count_con(net: &Net) -> usize { net.live_agents().filter(|a| a.symbol == Symbol::Con).count() }
fn count_dup(net: &Net) -> usize { net.live_agents().filter(|a| a.symbol == Symbol::Dup).count() }
fn count_era(net: &Net) -> usize { net.live_agents().filter(|a| a.symbol == Symbol::Era).count() }
```
