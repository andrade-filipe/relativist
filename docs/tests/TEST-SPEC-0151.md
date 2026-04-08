# TEST-SPEC-0151: Define protocol metrics

**Task:** TASK-0151
**Spec:** SPEC-11
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: ProtocolMetrics::new() does not panic

**Input:** `ProtocolMetrics::new()` (with `--features metrics`)
**Expected:** Returns a valid instance
**Verifies:** Constructor works

### T2: All 4 protocol metrics registered with relativist_ prefix

**Input:** Create metrics, register, encode
**Expected:** Encoded output contains: `relativist_messages_sent_total`, `relativist_messages_received_total`, `relativist_message_size_bytes`, `relativist_heartbeat_latency_seconds`
**Verifies:** R14, R17

### T3: messages_sent_total can be incremented with type label

**Input:** `metrics.messages_sent_total.get_or_create(&MessageTypeLabel { r#type: "AssignPartition".into() }).inc();`
**Expected:** Counter increments for the "AssignPartition" type
**Verifies:** R14 -- labeled counter

### T4: Histograms use IC_HISTOGRAM_BUCKETS

**Input:** Observe a value in `message_size_bytes`, encode
**Expected:** Bucket boundaries match `IC_HISTOGRAM_BUCKETS`
**Verifies:** R15 -- custom buckets

### T5: Build without metrics feature compiles

**Input:** `cargo check` without `--features metrics`
**Expected:** Compilation succeeds with no metrics code
**Verifies:** R10 -- feature gate

---

## Edge Cases

### E1: heartbeat_latency_seconds has no labels

**Verify:** `heartbeat_latency_seconds` is a plain `Histogram`, not a `Family<_, Histogram>`.
**Why:** Only one heartbeat mechanism per coordinator.

### E2: MessageTypeLabel uses raw identifier for type

**Verify:** `MessageTypeLabel` field is `r#type` (raw identifier) because `type` is a Rust keyword.
**Why:** Correct Rust syntax for keyword-named fields.
