# TEST-SPEC-0138: Implement SecurityConfig builder from CLI flags

**Task:** TASK-0138
**Spec:** SPEC-10
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Empty args produces Tier 1 config

**Input:** `SecurityConfig::for_coordinator(&SecurityArgs::default())`
**Expected:** `Ok(config)` where `config.tier == SecurityTier::Development` and `config.token == None`
**Verifies:** T9 -- no flags = Development

### T2: token=auto generates new token, Tier 2

**Input:** `SecurityConfig::for_coordinator(&SecurityArgs { token: Some("auto".into()), ..Default::default() })`
**Expected:** `Ok(config)` where `config.tier == SecurityTier::PrivateNetwork` and `config.token.is_some()`
**Verifies:** R9 -- auto-generation

### T3: token=base64 decodes token, Tier 2

**Input:** Generate a token, get its base64. `SecurityConfig::for_coordinator(&SecurityArgs { token: Some(base64_str), ..Default::default() })`
**Expected:** `Ok(config)` where `config.tier == SecurityTier::PrivateNetwork`
**Verifies:** R10 -- base64 decoding

### T4: Invalid base64 token returns error

**Input:** `SecurityConfig::for_coordinator(&SecurityArgs { token: Some("invalid!@#".into()), ..Default::default() })`
**Expected:** `Err(SecurityError::Token(TokenError::InvalidBase64(...)))`
**Verifies:** Error propagation from from_base64

### T5: TLS cert + key + token produces Tier 3

**Input:** `SecurityArgs { token: Some("auto".into()), tls_cert: Some("cert.pem".into()), tls_key: Some("key.pem".into()), ..Default::default() }` (with `tls` feature)
**Expected:** `Ok(config)` where `config.tier == SecurityTier::Production`
**Verifies:** Tier 3 detection

### T6: TLS cert without key returns Config error

**Input:** `SecurityArgs { token: Some("auto".into()), tls_cert: Some("cert.pem".into()), ..Default::default() }`
**Expected:** `Err(SecurityError::Config(...))`
**Verifies:** R25 -- incomplete TLS config

### T7: TLS without token returns Config error

**Input:** `SecurityArgs { tls_cert: Some("cert.pem".into()), tls_key: Some("key.pem".into()), ..Default::default() }`
**Expected:** `Err(SecurityError::Config(...))` per R4
**Verifies:** R4 -- TLS requires token

### T8: Worker RELATIVIST_TOKEN env var

**Input:** Set `RELATIVIST_TOKEN` env var to a valid base64 token; call `SecurityConfig::for_worker(&SecurityArgs::default())`
**Expected:** `Ok(config)` with the token loaded from env var
**Verifies:** R13 -- env var token loading

### T9: Worker CLI token takes precedence over env var

**Input:** Set `RELATIVIST_TOKEN` env var; also provide `SecurityArgs { token: Some(different_base64), ... }`
**Expected:** Config uses the CLI token, not the env var (R13)
**Verifies:** CLI precedence over env var

---

## Edge Cases

### E1: No --max-message-size flag

**Verify:** `SecurityArgs` does NOT have a `max_message_size` field.
**Why:** SC-004 -- removed in Revised v2.

### E2: TLS flags without tls feature compiled

**Input:** Provide TLS cert/key flags when compiled without `--features tls`
**Expected:** `Err(SecurityError::Config("TLS flags provided but the 'tls' feature is not enabled..."))` with helpful rebuild message
**Why:** User-friendly error for misconfiguration.
