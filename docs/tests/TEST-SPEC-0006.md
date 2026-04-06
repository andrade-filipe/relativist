# TEST-SPEC-0006: Define arity and total_ports functions

**Task:** TASK-0006
**Spec:** SPEC-02 R1 (implicit)
**Generated:** 2026-04-06

---

## Unit Tests

### T1: arity values
```rust
assert_eq!(arity(Symbol::Con), 2);
assert_eq!(arity(Symbol::Dup), 2);
assert_eq!(arity(Symbol::Era), 0);
```

### T2: total_ports values
```rust
assert_eq!(total_ports(Symbol::Con), 3);
assert_eq!(total_ports(Symbol::Dup), 3);
assert_eq!(total_ports(Symbol::Era), 1);
```

### T3: const fn usable at compile time
```rust
const CON_ARITY: u8 = arity(Symbol::Con);
assert_eq!(CON_ARITY, 2);
```

## Edge Cases

### E1: total_ports = arity + 1 identity
For all symbols, verify total_ports(s) == arity(s) + 1.
