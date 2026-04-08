# TEST-SPEC-0102: Implement CLI-to-config mapping functions

**Task:** TASK-0102
**Spec:** SPEC-07, SPEC-13
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: build_grid_config maps correctly

**Type:** Unit test
**Input:**
```
let args = CoordinatorArgs { workers: 4, max_rounds: Some(10), bind: default, input: "x.bin".into(), .. };
let config = build_grid_config(&args);
```
**Expected:** `config.num_workers == 4`, `config.max_rounds == Some(10)`
**Verifies:** R10 -- CLI to GridConfig mapping

### T2: build_node_config_coordinator uses correct defaults

**Type:** Unit test
**Input:**
```
let args = CoordinatorArgs { workers: 2, bind: "127.0.0.1:9000".parse().unwrap(), .. };
let config = build_node_config_coordinator(&args);
```
**Expected:**
- `config.bind == "127.0.0.1:9000".parse::<SocketAddr>()`
- `config.num_workers == 2`
- `config.max_payload_size == 268_435_456`
- `config.worker_connect_timeout == Duration::from_secs(120)`
- `config.distribute_timeout == Duration::from_secs(60)`
- `config.collect_timeout == Duration::from_secs(600)`
**Verifies:** R12 -- default timeout values from SPEC-06 v3

### T3: build_node_config_worker parses valid address

**Type:** Unit test
**Input:** `build_node_config_worker(&WorkerArgs { coordinator: "127.0.0.1:9000".to_string(), .. })`
**Expected:** `Ok(NodeConfig)` with `config.bind == "127.0.0.1:9000".parse()`
**Verifies:** R11 -- coordinator address parsing

### T4: build_node_config_worker rejects invalid address

**Type:** Unit test
**Input:** `build_node_config_worker(&WorkerArgs { coordinator: "not-valid".to_string(), .. })`
**Expected:** `Err(RelError::Config(_))` with error message containing "not-valid"
**Verifies:** Error handling for malformed addresses

### T5: parse_strategy recognizes round-robin

**Type:** Unit test
**Input:** `parse_strategy("round-robin")`
**Expected:** `Ok(Box<dyn PartitionStrategy>)` -- returns successfully
**Verifies:** R10 -- strategy name to implementation mapping

### T6: parse_strategy rejects unknown strategy

**Type:** Unit test
**Input:** `parse_strategy("unknown-strategy")`
**Expected:** `Err(RelError::Config(_))` with message listing available strategies
**Verifies:** Clear error for invalid strategy names

### T7: build_grid_config_from_local maps identically to coordinator version

**Type:** Unit test
**Input:**
```
let args = LocalArgs { workers: 3, max_rounds: None, .. };
let config = build_grid_config_from_local(&args);
```
**Expected:** `config.num_workers == 3`, `config.max_rounds == None`
**Verifies:** Local and coordinator modes produce compatible GridConfig

---

## Edge Cases

### E1: Worker NodeConfig has zero/ignored fields for coordinator-only settings

**Verify:** `build_node_config_worker` sets `num_workers` to 0 and timeouts to zero or defaults (they are not meaningful for a worker).
**Why:** Worker does not use coordinator-specific fields.

### E2: IPv6 coordinator address

**Verify:** `build_node_config_worker(&WorkerArgs { coordinator: "[::1]:9000".to_string(), .. })` returns `Ok` with correct IPv6 SocketAddr.
**Why:** IPv6 support should work via standard SocketAddr parsing.
