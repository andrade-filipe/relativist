# TEST-SPEC-0084: Define NodeConfig type

**Task:** TASK-0084
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Default values are correct

**Type:** Unit test
**Input:** `let config = NodeConfig::default();`
**Expected:**
- `config.bind` == `"127.0.0.1:9000".parse::<SocketAddr>().unwrap()`
- `config.num_workers` == 1
- `config.max_payload_size` == 268_435_456 (256 MiB)
- `config.worker_connect_timeout` == Duration::from_secs(120)
- `config.distribute_timeout` == Duration::from_secs(60)
- `config.collect_timeout` == Duration::from_secs(600)
**Verifies:** SPEC-06 v3 Section 4.5 default values

### T2: Fields can be overridden after construction

**Type:** Unit test
**Input:**
```
let mut config = NodeConfig::default();
config.bind = "0.0.0.0:8080".parse().unwrap();
config.num_workers = 4;
config.worker_connect_timeout = Duration::from_secs(30);
```
**Expected:** `config.bind` == `0.0.0.0:8080`, `config.num_workers` == 4, `config.worker_connect_timeout` == 30s
**Verifies:** All fields are `pub` and mutable

### T3: NodeConfig derives Debug and Clone

**Type:** Unit test
**Input:**
```
let config = NodeConfig::default();
let _ = format!("{:?}", config);
let _ = config.clone();
```
**Expected:** Both operations succeed
**Verifies:** Derive attributes

### T4: NodeConfig has exactly 6 fields

**Type:** Source verification
**Input:** Construct NodeConfig with all 6 fields explicitly
**Expected:** Compiles; no missing or extra fields
**Verifies:** SPEC-06 v3 Section 4.5 field list

---

## Edge Cases

### E1: NodeRole does NOT exist

**Verify:** There is no `NodeRole` enum defined in the protocol module.
**How:** `use relativist::protocol::NodeRole;` should fail to compile.
**Why:** NodeRole was removed in SPEC-06 v3 (SC-005); role determined by CLI subcommand per SPEC-13 R43.

### E2: bind field uses SocketAddr, not separate host+port

**Verify:** `NodeConfig` has a single `bind: SocketAddr` field, NOT separate `host: String` and `port: u16` fields.
**How:** `config.bind.port()` returns 9000; `config.bind.ip()` returns `127.0.0.1`.
**Why:** SPEC-06 v3 consolidated host+port into `bind: SocketAddr`.
