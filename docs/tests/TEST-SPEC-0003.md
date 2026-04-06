# TEST-SPEC-0003: Define AgentId and PortId type aliases

**Task:** TASK-0003
**Spec:** SPEC-02 R2, R3
**Generated:** 2026-04-06

---

## Test Summary

Type aliases have no runtime behavior to test. Verification is primarily compile-time (correct usage patterns compile) and size assertions.

---

## Unit Tests

### T1: AgentId is u32

**Type:** Compile-time + runtime
**Input:**
```rust
let id: AgentId = 42u32;
assert_eq!(std::mem::size_of::<AgentId>(), 4);
assert_eq!(id, 42);
```
**Expected:** Compiles; AgentId is 4 bytes

### T2: PortId is u8

**Type:** Compile-time + runtime
**Input:**
```rust
let p: PortId = 0u8;
assert_eq!(std::mem::size_of::<PortId>(), 1);
assert_eq!(p, 0);
```
**Expected:** Compiles; PortId is 1 byte

### T3: PortId valid range

**Type:** Runtime
**Input:**
```rust
let principal: PortId = 0;
let aux1: PortId = 1;
let aux2: PortId = 2;
```
**Expected:** All compile and hold correct values
**Note:** Values > 2 are invalid by convention, but not enforced by the type (by design)

### T4: AgentId max value

**Type:** Runtime
**Input:**
```rust
let max: AgentId = u32::MAX;
assert_eq!(max, 4_294_967_295);
```
**Expected:** AgentId supports full u32 range

---

## Edge Cases

### E1: AgentId arithmetic

**Input:**
```rust
let id: AgentId = 0;
let next = id + 1;
assert_eq!(next, 1);
```
**Expected:** Standard u32 arithmetic works (important for next_id increment)

### E2: PortId as array index

**Input:**
```rust
let ports = [10, 20, 30];
let p: PortId = 1;
assert_eq!(ports[p as usize], 20);
```
**Expected:** PortId can be used as array index via `as usize`
