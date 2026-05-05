# TEST-SPEC-0094: Implement GridMetrics network extensions

**Task:** TASK-0094
**Spec:** SPEC-06
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: total_network_bytes sums correctly

**Type:** Unit test
**Input:**
```
let mut metrics = GridMetrics::default();
metrics.bytes_sent_per_round = vec![100, 200, 300];
metrics.bytes_received_per_round = vec![150, 250, 350];
```
**Expected:** `metrics.total_network_bytes() == 100 + 200 + 300 + 150 + 250 + 350 == 1350`
**Verifies:** R33 -- total bytes calculation

### T2: network_overhead_fraction with known durations

**Type:** Unit test
**Input:**
```
let mut metrics = GridMetrics::default();
metrics.network_send_time_per_round = vec![Duration::from_millis(100), Duration::from_millis(200)];
metrics.network_recv_time_per_round = vec![Duration::from_millis(300), Duration::from_millis(400)];
metrics.total_time = Duration::from_secs(10);
```
**Expected:** `metrics.network_overhead_fraction() == 1.0 / 10.0 == 0.1` (1000ms network / 10000ms total)
**Verifies:** R35 -- overhead fraction formula from DISC-006 v2

### T3: network_overhead_fraction returns 0.0 when total_time is zero

**Type:** Unit test
**Input:**
```
let mut metrics = GridMetrics::default();
metrics.total_time = Duration::ZERO;
metrics.network_send_time_per_round = vec![Duration::from_millis(100)];
```
**Expected:** `metrics.network_overhead_fraction() == 0.0`
**Verifies:** Division-by-zero protection

### T4: Empty vectors return 0 and 0.0

**Type:** Unit test
**Input:**
```
let metrics = GridMetrics::default();  // all vecs empty, total_time = 0
```
**Expected:** `metrics.total_network_bytes() == 0`; `metrics.network_overhead_fraction() == 0.0`
**Verifies:** Local mode (no network activity) produces zero metrics

---

## Edge Cases

### E1: Single round with zero bytes sent

**Verify:** `bytes_sent_per_round = vec![0]; bytes_received_per_round = vec![0]` produces `total_network_bytes() == 0`.
**Why:** A round where no data was transferred (edge case in testing scenarios).

### E2: Very large byte counts do not overflow

**Verify:** `bytes_sent_per_round = vec![usize::MAX / 2]; bytes_received_per_round = vec![usize::MAX / 2]` produces a valid result without panic.
**Why:** Large networks could produce very high byte counts; usize should not overflow in realistic scenarios but the method should be safe.
