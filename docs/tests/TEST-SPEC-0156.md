# TEST-SPEC-0156: Implement /metrics endpoint with Prometheus encoding

**Task:** TASK-0156
**Spec:** SPEC-11 R21, T8a
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: GET /metrics returns HTTP 200

**Type:** Integration (HTTP handler)
**Input:** Create a `Registry` with at least one registered metric, build router, send `GET /metrics`
**Expected:** Response status is `200 OK`
**Verifies:** R21 -- metrics endpoint is reachable and responds

### T2: GET /metrics Content-Type is application/openmetrics-text

**Type:** Integration (HTTP handler)
**Input:** Send `GET /metrics`, inspect `Content-Type` header
**Expected:** Header value contains `application/openmetrics-text`
**Verifies:** T8a -- explicit Content-Type for OpenMetrics format

### T3: GET /metrics Content-Type includes version and charset

**Type:** Integration (HTTP handler)
**Input:** Send `GET /metrics`, inspect `Content-Type` header
**Expected:** Header value is exactly `application/openmetrics-text; version=1.0.0; charset=utf-8`
**Verifies:** T8a -- full Content-Type string per OpenMetrics specification

### T4: GET /metrics body contains registered counter

**Type:** Integration (HTTP handler)
**Input:**
1. Create `Registry`, register a `Counter<u64>` named `"test_counter"` with help `"A test"`
2. Increment the counter: `counter.inc()`
3. Build router, send `GET /metrics`, read body as UTF-8
**Expected:** Body contains the string `test_counter`
**Verifies:** R21 -- all registered metrics appear in the scrape response

### T5: GET /metrics body contains relativist_ prefixed metric

**Type:** Integration (HTTP handler)
**Input:**
1. Create `Registry` with `prefix("relativist")`
2. Register a `Counter<u64>` named `"rounds_total"`
3. Build router, send `GET /metrics`
**Expected:** Body contains `relativist_rounds_total`
**Verifies:** TASK-0156 AC -- response includes all registered metrics with `relativist_` prefix

### T6: GET /metrics body is valid OpenMetrics text

**Type:** Integration (HTTP handler)
**Input:** Send `GET /metrics`, read body
**Expected:** Body contains `# EOF` as the last meaningful line (required by OpenMetrics), and each metric family has a `# TYPE` annotation
**Verifies:** TASK-0156 AC -- response is valid Prometheus text exposition format

### T7: Histogram metric appears in encoded output after observation

**Type:** Integration (HTTP handler)
**Input:**
1. Create `Registry`, register a `Histogram` named `"round_duration_seconds"` with buckets `[0.01, 0.1, 1.0]`
2. Observe a value: `histogram.observe(0.05)`
3. Build router, send `GET /metrics`
**Expected:** Body contains `round_duration_seconds_bucket`, `round_duration_seconds_sum`, `round_duration_seconds_count`
**Verifies:** TASK-0156 AC -- histogram values are observable after recording

### T8: GET /metrics with empty registry returns valid response

**Type:** Integration (HTTP handler)
**Input:** Create an empty `Registry` (no metrics registered), build router, send `GET /metrics`
**Expected:** Response status is `200 OK`, body is a valid (possibly minimal) OpenMetrics document ending with `# EOF`
**Verifies:** Graceful behavior when no metrics are registered

---

## Edge Cases

### E1: Encoding error produces HTTP 500

**Verify:** If `prometheus_client::encoding::text::encode` returns an error (unlikely but guarded), the handler returns `500 Internal Server Error` with a `text/plain` body describing the error.
**Why:** The implementation guards against encode failures, so the test documents that error path.

### E2: Multiple concurrent /metrics requests do not deadlock

**Verify:** Send 10 concurrent `GET /metrics` requests to the same router instance. All return 200 within a reasonable timeout (1 second).
**Why:** The `Arc<Registry>` is shared across all handler invocations; concurrent reads must not deadlock.

### E3: Metrics response is UTF-8

**Verify:** The response body can be decoded as valid UTF-8 with no replacement characters.
**Why:** OpenMetrics text format requires UTF-8 encoding.
