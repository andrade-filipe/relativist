# TEST-SPEC-0168: Implement load_net/save_net dispatch with format detection

**Task:** TASK-0168
**Spec:** SPEC-12 R1, R18, R48, R49, R50, R51
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: load_net_from_file dispatches .bin correctly (roundtrip)

**Type:** Integration (filesystem)
**Input:**
```rust
let mut net = Net::new();
net.create_agent(Symbol::Era);
let path = temp_dir().join("dispatch_test.bin");
save_net_to_file(&net, &path).unwrap();
let restored = load_net_from_file(&path).unwrap();
```
**Expected:** `restored.count_live_agents() == 1`
**Verifies:** R1 -- .bin format dispatches to binary module

### T2: load_net_from_file dispatches .ic correctly (roundtrip)

**Type:** Integration (filesystem)
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Era);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(0));
net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(1));
let path = temp_dir().join("dispatch_test.ic");
save_net_to_file(&net, &path).unwrap();
let restored = load_net_from_file(&path).unwrap();
```
**Expected:** `restored.count_live_agents() == 2`
**Verifies:** R1 -- .ic format dispatches to text DSL parser

### T3: load_net_from_file on .json returns descriptive error

**Type:** Integration (filesystem)
**Input:**
```rust
let path = temp_dir().join("dispatch_test.json");
std::fs::write(&path, "{}").unwrap();
let result = load_net_from_file(&path);
```
**Expected:** Returns `Err(...)` containing `"JSON format not yet supported"`
**Verifies:** R1, R17 -- JSON format recognized but not yet implemented

### T4: load_net_from_file on unknown extension returns error

**Type:** Unit
**Input:** `load_net_from_file(Path::new("net.xyz"))`
**Expected:** Returns `Err(...)` containing `"unknown file extension"`
**Verifies:** R49 -- unrecognized extension produces error

### T5: save_net_to_file + load_net_from_file roundtrip for .bin

**Type:** Integration (filesystem)
**Input:**
```rust
let mut net = Net::new();
let a = net.create_agent(Symbol::Con);
let b = net.create_agent(Symbol::Dup);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
let path = temp_dir().join("roundtrip_dispatch.bin");
save_net_to_file(&net, &path).unwrap();
let restored = load_net_from_file(&path).unwrap();
```
**Expected:** `restored.count_live_agents() == 2`, all connections preserved
**Verifies:** R51 -- end-to-end roundtrip through dispatch layer

### T6: save_net_to_file + load_net_from_file roundtrip for .ic

**Type:** Integration (filesystem)
**Input:**
```rust
let mut net = Net::new();
net.create_agent(Symbol::Era);
net.create_agent(Symbol::Era);
let path = temp_dir().join("roundtrip_dispatch.ic");
save_net_to_file(&net, &path).unwrap();
let restored = load_net_from_file(&path).unwrap();
```
**Expected:** `restored.count_live_agents() == 2`
**Verifies:** R51 -- text DSL roundtrip through dispatch

### T7: load_net_from_file on non-existent file returns error

**Type:** Integration (filesystem)
**Input:** `load_net_from_file(Path::new("nonexistent_file_xyz.bin"))`
**Expected:** Returns `Err(...)` (I/O error, file not found)
**Verifies:** Error propagation from underlying format module

### T8: save_net_to_file on .json returns error

**Type:** Integration (filesystem)
**Input:**
```rust
let net = Net::new();
let path = temp_dir().join("dispatch_save.json");
let result = save_net_to_file(&net, &path);
```
**Expected:** Returns `Err(...)` containing `"JSON format not yet supported"`
**Verifies:** R17 -- JSON save is not supported in v1

---

## Edge Cases

### E1: File extension detection is case-sensitive

**Verify:** `load_net_from_file(Path::new("net.BIN"))` returns an error (unknown extension), not dispatching to binary.
**Why:** Extension detection via `Path::extension()` preserves case on case-sensitive systems.

### E2: Format detection handles paths with dots in directory names

**Verify:** `load_net_from_file(Path::new("/path.dir/net.bin"))` correctly detects `.bin` extension (not `.dir`).
**Why:** `Path::extension()` returns only the file's own extension, ignoring directory components.

### E3: Concurrent load/save to different files does not interfere

**Verify:** Save two different nets to two different files simultaneously (from parallel tests), then load both. Each loads the correct net.
**Why:** File I/O operations are independent; the test documents that no shared mutable state exists in the dispatch layer.
