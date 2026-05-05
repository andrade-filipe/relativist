# TEST-SPEC-0104: Implement net serialization/deserialization helpers

**Task:** TASK-0104
**Spec:** SPEC-07
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Round-trip empty Net

**Type:** Unit test
**Input:**
```
let net = Net::new();
let bytes = serialize_net(&net).unwrap();
let decoded = deserialize_net(&bytes).unwrap();
```
**Expected:** `decoded.agents.len() == net.agents.len()`; `decoded.redex_queue.len() == 0`
**Verifies:** R22 -- `deserialize(serialize(net)) == net` for empty Net

### T2: Round-trip Net with agents and connections

**Type:** Unit test
**Input:**
```
let mut net = Net::new();
// Add 3 agents (CON, DUP, ERA) with connections
let bytes = serialize_net(&net).unwrap();
let decoded = deserialize_net(&bytes).unwrap();
```
**Expected:** Decoded net has same number of agents, same connections, same redex queue
**Verifies:** R22 -- round-trip for non-trivial nets

### T3: Corrupt data returns error

**Type:** Unit test
**Input:** `deserialize_net(&[0xFF, 0xFF, 0x00, 0x01])`
**Expected:** `Err(RelativistError::Config(_))` with "deserialization failed" in the message
**Verifies:** R24 -- corrupt data produces clear error

### T4: load_net_from_file on non-existent path

**Type:** Unit test
**Input:** `load_net_from_file(Path::new("/tmp/nonexistent_test_file_xyz.bin"))`
**Expected:** `Err(RelativistError::Io(_))` -- file not found
**Verifies:** R23 -- I/O errors propagated with clear context

### T5: save_net_to_file and load_net_from_file round-trip

**Type:** Integration test
**Input:**
```
let net = create_test_net_with_5_agents();
let path = tmp_dir.path().join("test_net.bin");
save_net_to_file(&net, &path).unwrap();
let loaded = load_net_from_file(&path).unwrap();
```
**Expected:** `loaded` matches original net (same agents, connections, redexes)
**Verifies:** Full file I/O round-trip

### T6: load_net_from_file logs agent and redex count

**Type:** Integration test
**Input:** Load a net file with 10 agents and 3 redexes
**Expected:** Info-level log message contains `agents = 10` and `redexes = 3`
**Verifies:** Acceptance criteria: info-level logging after load

---

## Edge Cases

### E1: Empty file produces deserialization error

**Verify:** `load_net_from_file` on a 0-byte file returns `Err` with descriptive message including the file path.
**Why:** Empty files should not silently produce a default Net.

### E2: Functions use synchronous std::fs, not tokio

**Verify:** Source code uses `std::fs::read` and `std::fs::write`, NOT `tokio::fs`.
**Why:** Local mode and generate are sync; async variants can be added later.
