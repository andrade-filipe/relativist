# TEST-SPEC-0125: Define SecurityConfig struct

**Task:** TASK-0125
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: development() returns Tier 1 config

**Input:** `SecurityConfig::development().tier`
**Expected:** `SecurityTier::Development`
**Verifies:** R2 -- zero-config development mode

### T2: development() has no token

**Input:** `SecurityConfig::development().token`
**Expected:** `None`
**Verifies:** Tier 1 has no authentication

### T3: development() has default max_connections

**Input:** `SecurityConfig::development().max_connections`
**Expected:** `1024`
**Verifies:** R31 default

### T4: development() has default idle_timeout

**Input:** `SecurityConfig::development().idle_timeout`
**Expected:** `Duration::from_secs(30)`
**Verifies:** R32 default

### T5: SecurityConfig derives Debug

**Input:** `format!("{:?}", SecurityConfig::development())`
**Expected:** Does not panic; output contains "Development" (from the tier field)
**Verifies:** Debug derive works; token is redacted via AuthToken's custom Debug

### T6: SecurityConfig derives Clone

**Input:** `let c = SecurityConfig::development(); let c2 = c.clone();`
**Expected:** Compiles and `c2.tier == c.tier`
**Verifies:** Clone derive

---

## Edge Cases

### E1: No max_message_size field

**Verify:** `SecurityConfig` does NOT have a `max_message_size` field. Attempting to access `config.max_message_size` fails to compile.
**Why:** SC-004 -- message size is owned by SPEC-06 `NodeConfig.max_payload_size`.

### E2: TLS fields absent without feature

**Verify:** When compiled without `--features tls`, `SecurityConfig` does not have `tls_server` or `tls_client` fields.
**Why:** Feature-gated fields must not exist when feature is disabled.
