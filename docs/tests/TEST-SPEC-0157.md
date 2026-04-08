# TEST-SPEC-0157: Implement axum HTTP server spawn as background tokio task

**Task:** TASK-0157
**Spec:** SPEC-11 R20, R23, R24, R24a, R33a
**Generated:** 2026-04-08 (retroactive)

---

## Unit Tests

### T1: spawn_metrics_server binds and returns Ok

**Type:** Integration (async)
**Input:**
```rust
let registry = Arc::new(Registry::default());
let is_ready = Arc::new(AtomicBool::new(true));
let (tx, rx) = tokio::sync::oneshot::channel();
let handle = spawn_metrics_server(
    "127.0.0.1:0".parse().unwrap(), // port 0 = OS-assigned
    registry,
    is_ready,
    rx,
).await;
```
**Expected:** `handle` is `Ok(JoinHandle<()>)` -- server started successfully
**Verifies:** R23 -- server spawns as background tokio task

### T2: Spawned server responds to GET /health

**Type:** Integration (async + HTTP client)
**Input:**
1. Spawn server on `127.0.0.1:0`
2. Retrieve the actual bound address from the `TcpListener`
3. Send `GET http://{addr}/health` using an HTTP client (e.g., `reqwest` or `hyper`)
**Expected:** Response status is `200 OK`, body is `"ok"`
**Verifies:** R23, R21 -- the background server actually serves requests

### T3: Spawned server does not block the calling async task

**Type:** Integration (async)
**Input:**
1. Spawn server
2. Immediately after `spawn_metrics_server` returns, execute another async operation (e.g., `tokio::time::sleep(Duration::from_millis(1))`)
**Expected:** The second async operation completes promptly (< 100ms), proving the server did not block the caller
**Verifies:** R23 -- server runs in a separate tokio task

### T4: Shutdown signal causes server to stop

**Type:** Integration (async)
**Input:**
1. Spawn server with `oneshot::channel`
2. Confirm server is responding (send a health check)
3. Send `()` on the `oneshot::Sender`
4. Await the `JoinHandle`
**Expected:** `JoinHandle` completes without error (server exited gracefully)
**Verifies:** R24a -- graceful shutdown on signal

### T5: Server stops accepting connections after shutdown

**Type:** Integration (async + HTTP client)
**Input:**
1. Spawn server, note the bind address
2. Send shutdown signal
3. Await the `JoinHandle`
4. Attempt to send `GET /health` to the same address
**Expected:** The HTTP request fails (connection refused or similar error)
**Verifies:** R24a -- after shutdown, port is released

### T6: Binding to an already-used port returns Err

**Type:** Integration (async)
**Input:**
1. Bind a `TcpListener` to `127.0.0.1:0` to claim a port
2. Get the port number
3. Call `spawn_metrics_server` with that same port
**Expected:** Returns `Err(std::io::Error)` with kind `AddrInUse`
**Verifies:** TASK-0157 AC -- bind failure is handled gracefully, does not panic

### T7: Default metrics port is 9090

**Type:** Source verification
**Input:** Verify that the default configuration or constant for the metrics port is `9090`
**Expected:** The default port value is `9090`
**Verifies:** R20 -- default metrics port

---

## Edge Cases

### E1: Spawn with port 0 binds to random available port

**Verify:** When calling `spawn_metrics_server` with `127.0.0.1:0`, the actual bound address has a non-zero port. The test must verify this by reading the `local_addr()` from the listener.
**Why:** Port 0 is used in tests to avoid port conflicts; the OS assigns a free port.

### E2: JoinHandle can be dropped without blocking

**Verify:** After spawning the server, dropping the `JoinHandle` without awaiting it does not cause a panic or resource leak. The server continues running until the shutdown signal is sent.
**Why:** In production, the coordinator may not need to await the handle; it may just cancel the token.

### E3: Server handles rapid shutdown (immediate cancel)

**Verify:** Send the shutdown signal immediately after spawning (within microseconds), then await the handle. It completes without error.
**Why:** Race condition between server startup and shutdown must be handled gracefully.
