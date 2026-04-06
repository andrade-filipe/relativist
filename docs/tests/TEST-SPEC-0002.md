# TEST-SPEC-0002: Define Symbol enum

**Task:** TASK-0002
**Spec:** SPEC-02 R1
**Generated:** 2026-04-06

---

## Test Summary

Verify that the `Symbol` enum has exactly 3 variants with correct discriminants, derives, and documentation. Leaf type with no dependencies.

---

## Unit Tests

### T1: Symbol has exactly 3 variants

**Type:** Compile-time + runtime
**Input:** Instantiate all 3 variants: `Symbol::Con`, `Symbol::Dup`, `Symbol::Era`
**Expected:** All compile and are distinct values
**Verifies:** R1 — exactly 3 variants

### T2: Discriminant values match repr(u8)

**Type:** Runtime assertion
**Input:**
```rust
Symbol::Con as u8  // expected: 0
Symbol::Dup as u8  // expected: 1
Symbol::Era as u8  // expected: 2
```
**Expected:** Con=0, Dup=1, Era=2
**Verifies:** `#[repr(u8)]` with explicit discriminants

### T3: Symbol implements Copy and Clone

**Type:** Compile-time
**Input:**
```rust
let s = Symbol::Con;
let s2 = s;      // Copy
let s3 = s.clone(); // Clone
assert_eq!(s, s2);
assert_eq!(s, s3);
```
**Expected:** Compiles and all equal
**Verifies:** Derive Copy, Clone

### T4: Symbol implements PartialEq and Eq

**Type:** Runtime assertion
**Input:**
```rust
assert_eq!(Symbol::Con, Symbol::Con);
assert_ne!(Symbol::Con, Symbol::Dup);
assert_ne!(Symbol::Con, Symbol::Era);
assert_ne!(Symbol::Dup, Symbol::Era);
```
**Expected:** All pass
**Verifies:** Derive PartialEq, Eq

### T5: Symbol implements Hash (usable in HashMap)

**Type:** Compile-time + runtime
**Input:**
```rust
use std::collections::HashMap;
let mut map = HashMap::new();
map.insert(Symbol::Con, "constructor");
map.insert(Symbol::Dup, "duplicator");
map.insert(Symbol::Era, "eraser");
assert_eq!(map.len(), 3);
```
**Expected:** Compiles and map has 3 entries
**Verifies:** Derive Hash

### T6: Symbol serialization round-trip (serde + bincode)

**Type:** Runtime
**Input:**
```rust
for sym in [Symbol::Con, Symbol::Dup, Symbol::Era] {
    let bytes = bincode::serialize(&sym).unwrap();
    let deserialized: Symbol = bincode::deserialize(&bytes).unwrap();
    assert_eq!(sym, deserialized);
}
```
**Expected:** Round-trip identity for all 3 variants
**Verifies:** Derive Serialize, Deserialize

### T7: Symbol Debug formatting

**Type:** Runtime
**Input:**
```rust
assert_eq!(format!("{:?}", Symbol::Con), "Con");
assert_eq!(format!("{:?}", Symbol::Dup), "Dup");
assert_eq!(format!("{:?}", Symbol::Era), "Era");
```
**Expected:** Debug output matches variant names
**Verifies:** Derive Debug

---

## Edge Cases

### E1: Exhaustive match compiles

**Input:**
```rust
fn name(s: Symbol) -> &'static str {
    match s {
        Symbol::Con => "constructor",
        Symbol::Dup => "duplicator",
        Symbol::Era => "eraser",
    }
}
```
**Expected:** Compiles without a `_ =>` arm (proof that enum is exhaustive with 3 variants)

### E2: Size of Symbol is 1 byte

**Input:**
```rust
assert_eq!(std::mem::size_of::<Symbol>(), 1);
```
**Expected:** Symbol is 1 byte due to `#[repr(u8)]`
