# SPEC-16: Worker Daemon Mode

**Status:** Draft v1
**Depends on:** SPEC-06 (Wire Protocol), SPEC-07 (Deployment), SPEC-10 (Security), SPEC-13 (System Architecture)
**Gray zones resolved:** ---
**References consumed:** ---
**Discussions consumed:** ---
**Arguments consumed:** ---
**Code analyses consumed:** ---

---

## 1. Purpose

This spec defines a daemon mode for the Relativist worker process, enabling it to remain alive and automatically reconnect to the coordinator after each job completes. Without daemon mode, workers are single-shot: they connect, reduce one partition set, receive `Shutdown`, and exit. Daemon mode adds an outer loop around this existing behavior, allowing a single worker process to participate in multiple sequential benchmark runs without manual restart. This is motivated by the Phase 3 experimental campaign (~400 configurations), where restarting workers manually between each run is impractical.

The coordinator is unchanged. No new wire protocol messages are introduced. The `Shutdown` message semantics remain identical: from the coordinator's perspective, each job is a complete, independent grid cycle. The daemon worker simply interprets `Shutdown` as "this job is done; wait and reconnect" rather than "exit the process."

---

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Daemon Mode** | A worker execution mode where the process loops indefinitely, reconnecting to the coordinator after each job completes (or fails). Activated by the `--daemon` CLI flag. The worker exits only upon receiving an OS signal (SIGINT, SIGTERM) or a fatal unrecoverable error. |
| **Job** | One complete execution of the worker's inner logic: connect to the coordinator, register, receive a partition, reduce locally, return the result, and receive `Shutdown`. In daemon mode, the worker executes a sequence of jobs, one at a time. |
| **Single-Shot Mode** | The default worker execution mode (no `--daemon` flag). The worker executes exactly one job and exits. This is the existing behavior prior to SPEC-16 and remains the default. |
| **Post-Job Delay** | A fixed sleep interval between consecutive jobs in daemon mode. Prevents tight reconnection loops and allows the coordinator process to fully shut down before the worker attempts to reconnect. |

---

## 3. Requirements

### 3.1 CLI Interface

**R1.** The `worker` subcommand MUST accept an optional `--daemon` flag (boolean, default: `false`). When present, the worker runs in daemon mode. **(MUST)**

**R2.** Without `--daemon`, the worker MUST behave identically to the current single-shot implementation: connect, execute one job, and exit. No behavioral change for existing usage. **(MUST)**

**R3.** With `--daemon`, the worker MUST execute in a loop: run one job (via `run_worker_inner`), sleep for the post-job delay, and reconnect to the coordinator for the next job. The loop continues until interrupted by an OS signal. **(MUST)**

### 3.2 Reconnection Behavior

**R4.** In daemon mode, `connect_with_retry` MUST retry indefinitely (no maximum attempt count). The exponential backoff strategy (starting at 1 second, doubling up to a 16-second cap) defined in SPEC-06 R23 MUST still apply. This allows the worker to wait for a new coordinator instance to start between benchmark runs. **(MUST)**

**R5.** After a successful job (the inner worker function returns `Ok`), the worker MUST sleep for 2 seconds before attempting to reconnect. **(MUST)**

**R6.** After a failed job (the inner worker function returns `Err`), the worker MUST sleep for 5 seconds before attempting to reconnect. **(MUST)**

### 3.3 Signal Handling

**R7.** In daemon mode, the worker MUST exit cleanly upon receiving SIGINT (`Ctrl+C`) or SIGTERM. On Unix-like systems, both signals MUST be handled. On Windows, `Ctrl+C` (via `tokio::signal::ctrl_c()`) MUST be handled; SIGTERM handling is best-effort. **(MUST)**

**R8.** Upon receiving a termination signal, the worker MUST log an `INFO`-level message (e.g., `"Daemon received shutdown signal, exiting"`) and exit with code 0. **(MUST)**

**R9.** If a termination signal arrives while a job is actively running (i.e., the worker is connected and reducing), the worker SHOULD attempt best-effort cleanup (e.g., dropping the TCP connection gracefully). The worker MUST NOT block indefinitely waiting for the active job to complete; `tokio::select!` between the signal and the job future ensures prompt exit. **(SHOULD)**

### 3.4 Logging

**R10.** At the start of each job attempt in daemon mode, the worker MUST log an `INFO`-level message including the 1-indexed job number: `"Daemon job #N: connecting to coordinator at <addr>"`. **(MUST)**

**R11.** After each job completes, the worker MUST log the outcome:
- On success (`Ok`): `INFO`-level message, e.g., `"Daemon job #N: completed successfully"`.
- On failure (`Err`): `WARN`-level message including the error, e.g., `"Daemon job #N: failed: <error>"`.
**(MUST)**

### 3.5 Coordinator Compatibility

**R12.** The coordinator MUST NOT be modified to support daemon mode. The coordinator continues to orchestrate a single grid cycle and exit (or wait for the next invocation). From the coordinator's perspective, each connection from a daemon worker is indistinguishable from a fresh single-shot worker. **(MUST)**

**R13.** No new wire protocol messages are introduced by this spec. The existing `Shutdown` message (SPEC-06 R2) retains its current semantics: the coordinator sends `Shutdown` to signal end-of-job, and the worker closes the connection. In daemon mode, the worker then loops back to reconnect rather than exiting the process. **(MUST)**

---

## 4. Design

### 4.1 Refactoring `run_worker`

The existing `run_worker` function encapsulates the full worker lifecycle: parse config, connect with retry, register, receive partition, reduce, send result, receive shutdown, exit. To support daemon mode, this logic is split into two layers:

```rust
/// Inner worker function that executes a single job.
///
/// `max_connect_attempts`:
/// - `Some(n)`: retry connection up to `n` times (single-shot mode, backward compatible).
/// - `None`: retry indefinitely (daemon mode).
///
/// The cancellation_token parameter enables cooperative shutdown
/// when a signal is received during an active job.
async fn run_worker_inner(
    config: &WorkerConfig,
    cancellation_token: CancellationToken,
    max_connect_attempts: Option<u32>,
) -> Result<(), RelativistError>;
```

The public API provides two entry points:

```rust
/// Single-shot worker: connects, executes one job, exits.
/// Backward compatible with pre-SPEC-16 behavior.
/// Calls `run_worker_inner` with `max_connect_attempts = Some(10)`.
pub async fn run_worker(config: WorkerConfig) -> Result<(), RelativistError>;

/// Daemon worker: loops indefinitely, reconnecting after each job.
/// Calls `run_worker_inner` with `max_connect_attempts = None` in a loop.
/// Exits cleanly on SIGINT/SIGTERM.
pub async fn run_worker_daemon(config: WorkerConfig) -> Result<(), RelativistError>;
```

### 4.2 Daemon Loop Pseudocode

```rust
pub async fn run_worker_daemon(config: WorkerConfig) -> Result<(), RelativistError> {
    let mut job_number: u64 = 0;
    let token = CancellationToken::new();

    loop {
        job_number += 1;
        info!(job = job_number, addr = %config.coordinator_addr,
              "Daemon job #{job_number}: connecting to coordinator");

        tokio::select! {
            result = run_worker_inner(&config, token.clone(), None) => {
                match result {
                    Ok(()) => {
                        info!(job = job_number, "Daemon job #{job_number}: completed successfully");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    Err(e) => {
                        warn!(job = job_number, error = %e,
                              "Daemon job #{job_number}: failed: {e}");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Daemon received shutdown signal, exiting");
                break;
            }
        }
    }

    Ok(())
}
```

### 4.3 `connect_with_retry` Parameterization

The `connect_with_retry` function (SPEC-06 R23) is parameterized to support both finite and infinite retry:

```rust
/// Connect to the coordinator with exponential backoff.
///
/// - `max_attempts`: `Some(n)` for finite retry, `None` for infinite retry.
/// - Backoff: 1s, 2s, 4s, 8s, 16s (cap), 16s, 16s, ...
/// - Returns `Err` if max_attempts is exceeded.
/// - With `None`, retries indefinitely until connection succeeds or
///   the cancellation_token is cancelled.
pub async fn connect_with_retry(
    addr: &str,
    max_attempts: Option<u32>,
    cancellation_token: CancellationToken,
) -> Result<TcpStream, RelativistError>;
```

The existing call site in `run_worker` (single-shot) passes `Some(10)` to preserve backward compatibility with SPEC-06 R23's "at most 10 attempts" requirement.

### 4.4 CLI Integration

The `--daemon` flag is added to the `WorkerArgs` struct (SPEC-07 R4, SPEC-13 R44):

```rust
/// Worker subcommand arguments.
#[derive(Debug, Clone, clap::Args)]
pub struct WorkerArgs {
    /// Address of the coordinator (HOST:PORT).
    #[arg(long)]
    pub coordinator: String,

    /// Run in daemon mode: reconnect after each job completes.
    /// Without this flag, the worker exits after one job (single-shot).
    #[arg(long, default_value_t = false)]
    pub daemon: bool,

    // ... existing fields (--token, --tls-ca, --log-format) ...
}
```

The subcommand dispatch logic selects between `run_worker` and `run_worker_daemon` based on the flag:

```rust
Command::Worker(args) => {
    let config = WorkerConfig::from(args);
    if args.daemon {
        run_worker_daemon(config).await
    } else {
        run_worker(config).await
    }
}
```

### 4.5 Post-Job Delay Constants

The post-job delays are defined as module-level constants:

```rust
/// Delay after a successful job before reconnecting (daemon mode).
const DAEMON_SUCCESS_DELAY: Duration = Duration::from_secs(2);

/// Delay after a failed job before reconnecting (daemon mode).
const DAEMON_FAILURE_DELAY: Duration = Duration::from_secs(5);
```

These are hardcoded constants, not configurable via CLI. See Q2 in Open Questions.

---

## 5. Rationale

### 5.1 Why Daemon Mode Instead of External Process Managers

External tools (systemd, supervisord, shell loops like `while true; do relativist worker ...; done`) could achieve similar behavior. However:

1. **Integrated signal handling:** The daemon loop can cleanly abort a running job on Ctrl+C, whereas a shell loop would kill the process abruptly mid-reduction.
2. **Job-aware logging:** The worker logs job numbers and outcomes with structured fields, enabling correlation across a multi-hundred-run campaign.
3. **Connection retry semantics:** In daemon mode, `connect_with_retry` retries indefinitely with backoff, whereas a shell-loop restart would re-parse CLI args and reinitialize state on each iteration.
4. **Single deployment artifact:** No additional scripts or systemd units to distribute to test machines.

### 5.2 Why the Coordinator Does Not Change

The coordinator already has a well-defined lifecycle: accept workers, run one grid cycle, send `Shutdown`, exit. Making the coordinator aware of daemon workers would require session management, worker re-identification across jobs, and state cleanup between rounds -- all of which add complexity with no benefit for the TCC benchmark campaign. The simpler model is: each coordinator invocation is one benchmark run; daemon workers connect to whatever coordinator appears next.

### 5.3 Why Fixed Delays Instead of Configurable

The 2-second and 5-second delays are sufficient for the TCC benchmark campaign. Making them configurable via CLI adds flags that will never be tuned in practice. If future use cases require different delays, they can be promoted to CLI flags in v2 with backward-compatible defaults.

### 5.4 Why `CancellationToken` Over Direct Signal Handling

Using `tokio_util::sync::CancellationToken` (or equivalent) decouples signal handling from the inner worker logic. The daemon loop owns the signal listener and cancels the token on signal receipt. The inner worker and `connect_with_retry` check the token at natural yield points (between connection attempts, between BSP phases). This avoids threading signal-handling logic through every layer and makes the inner worker testable without OS signals.

---

## 6. Haskell Prototype Reference

The Haskell prototype (`grid_computing_interaction_combinators_prototype_v1`) does not implement daemon mode. Workers in the prototype are single-shot processes launched by the benchmark driver script. The daemon mode concept is entirely new to the Relativist implementation, motivated by the scale of the Phase 3 experimental campaign.

The `workerLoop` function in the Haskell prototype (AC-003) processes a single grid cycle and exits. The `connectWithRetry` function has a finite retry count (10 attempts). Relativist's refactoring of `run_worker` into `run_worker_inner` preserves the Haskell prototype's single-job logic while adding the outer daemon loop as a new layer.

---

## 7. Test Requirements

**T1.** `connect_with_retry` with `max_attempts = Some(1)` MUST fail after exactly 1 attempt when no listener is available on the target address. The error MUST be a connection error, not a timeout. **(MUST)**

**T2.** `connect_with_retry` with `max_attempts = None` MUST successfully connect when the listener becomes available after a delay. Test setup: spawn a task that starts listening after 3 seconds; `connect_with_retry(None)` MUST eventually connect. **(MUST)**

**T3.** In daemon mode, the worker MUST complete at least 2 sequential jobs against mock coordinators. Test setup: start a mock coordinator that sends `AssignPartition` + `Shutdown`, verify the worker reconnects and completes a second job with a second mock coordinator instance. **(MUST)**

**T4.** The `--daemon` CLI flag MUST parse correctly: present yields `daemon = true`, absent yields `daemon = false`. This MUST be tested via clap's `try_parse_from` with both flag-present and flag-absent argument vectors. **(MUST)**

---

## 8. Open Questions

**Q1.** `--max-idle <DURATION>`: Should the daemon exit automatically after being idle (unable to connect) for a configurable duration? **Deferred to v2.** For the TCC campaign, operators will manually Ctrl+C workers when the campaign is complete.

**Q2.** Should post-job delays (2s success, 5s failure) be configurable via CLI flags (`--success-delay`, `--failure-delay`)? **Deferred.** The hardcoded defaults are sufficient for the TCC experimental campaign. If future benchmarking scenarios require different timing, these can be promoted to CLI flags with the current values as defaults, maintaining backward compatibility.
