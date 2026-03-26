---
pesq_id: PESQ-001
title: "BOINC: Volunteer Computing Architecture"
category: Grid Computing Architectures
date_created: 2026-03-25
status: Complete
---

# PESQ-001: BOINC -- Volunteer Computing Architecture

**Category:** Grid Computing Architectures
**Status:** Complete
**Cross-references:**
- Specs: SPEC-07 (deployment), SPEC-06 (wire protocol), SPEC-05 (merge and grid cycle)
- References: REF-007 (Casanova 2002), REF-017 (Foster 2001), REF-011 (DDGrid)
- Discussions: DISC-007 v2 (fault tolerance), DISC-005 v2 (cross-boundary protocol)

---

## 1. Subject Overview

BOINC (Berkeley Open Infrastructure for Network Computing) is an open-source middleware system for volunteer computing, developed at UC Berkeley by David P. Anderson starting in 2002. It evolved from the SETI@home project and became the dominant platform for large-scale volunteer computing. BOINC enables scientific projects to harness computing resources from millions of volunteered personal computers worldwide.

**Scale:** As of 2021, BOINC connected approximately 136,000 active hosts with a combined throughput of ~20 PetaFLOPS. Over 2.5 million people have installed the BOINC client software across the project's lifetime. As of February 2026, 26 projects are listed on the official BOINC project directory.

**Computation model:** BOINC implements a strict **bag-of-tasks** model. Each job (called a "workunit") is an independent unit of computation with well-defined input files and output files. There is no inter-job communication during execution. Clients pull work from the server, execute it locally, and upload results. This is fundamentally different from Relativist's **iterative synchronous grid loop** (SPEC-05), where workers receive partitions of a single shared graph and must synchronize at every round.

**Key design constraints that shaped BOINC:**
1. **Untrusted, anonymous clients:** Volunteers are not authenticated. Results may be incorrect (malicious or faulty hardware).
2. **Heterogeneous platforms:** Windows, macOS, Linux, Android, with varying CPU/GPU architectures.
3. **Intermittent availability:** Volunteer computers can disconnect at any time.
4. **Asymmetric bandwidth:** Home connections often have limited upload bandwidth.
5. **No inter-client communication:** Clients communicate only with the server, never with each other.

**Primary reference:** Anderson, D.P. (2019). "BOINC: A Platform for Volunteer Computing." *Journal of Grid Computing*, 18, 99-122. [arXiv:1903.01699](https://arxiv.org/pdf/1903.01699)

---

## 2. Architecture / Design

### 2.1 High-Level Architecture

BOINC follows a strict **client-server model** with a centralized project server and distributed volunteer clients. A single project server can manage hundreds of thousands of clients.

```
+--------------------------------------------------+
|                  PROJECT SERVER                   |
|                                                   |
|  +----------+   +---------+   +---------------+  |
|  | Work Gen |-->| MySQL   |<--| Transitioner  |  |
|  +----------+   | Database|   +---------------+  |
|                  |         |           |          |
|  +----------+   |         |   +---------------+  |
|  | Feeder   |-->| (shared |   | Validator     |  |
|  +----------+   |  memory)|   +---------------+  |
|       |         |         |           |          |
|  +----------+   |         |   +---------------+  |
|  | Scheduler|<->|         |   | Assimilator   |  |
|  | (CGI)    |   |         |   +---------------+  |
|  +----------+   |         |           |          |
|       ^         |         |   +---------------+  |
|       |         +---------+   | File Deleter  |  |
|       |              ^        +---------------+  |
|  +----------+        |                           |
|  | Apache   |   +----------+                     |
|  | HTTP     |   | Upload   |                     |
|  | Server   |   | Handler  |                     |
|  +----------+   | (CGI)    |                     |
|       ^         +----------+                     |
+-------|--------------|---------------------------+
        |              |
     HTTP GET       HTTP POST
   (scheduling)   (file upload)
        |              |
+-------v--------------v--------------------------+
|              INTERNET (HTTP/HTTPS)                |
+-------^--------------^--------------------------+
        |              |
+-------|--------------|---------------------------+
|       v              v       VOLUNTEER CLIENT    |
|  +-----------+  +----------+                     |
|  | Core      |  | File     |                     |
|  | Client    |  | Transfers|                     |
|  +-----------+  +----------+                     |
|       |                                          |
|  +-----------+                                   |
|  | Science   |  (sandboxed execution)            |
|  | App(s)    |                                   |
|  +-----------+                                   |
|       |                                          |
|  +-----------+                                   |
|  | BOINC     |  (GUI, optional)                  |
|  | Manager   |                                   |
|  +-----------+                                   |
+--------------------------------------------------+
```

### 2.2 Server Components

The BOINC server consists of two CGI programs and five (or more) backend daemons, all written in C++. They share state through a MySQL/MariaDB database and, for the scheduler/feeder pair, through a shared-memory segment.

#### 2.2.1 MySQL Database

The central state store. Contains tables for:
- **Workunits:** Job definitions with input file references, deadlines, replication parameters.
- **Results (instances):** Individual instances of workunits sent to specific hosts. Each workunit may have multiple results (replicas).
- **Hosts:** Registered volunteer computers with hardware profiles.
- **Apps / App versions:** Application binaries for different platforms.

All server daemons read and write to this database. It is the single source of truth for all job state.

#### 2.2.2 Feeder Daemon

The feeder is a continuously running daemon that maintains a **shared-memory segment** (typically 32+ MB) containing a cache of unsent result records and their associated workunits. The feeder reads from the database and fills vacant slots in the cache. This decouples the scheduler from direct database access for the hot path (dispatching work), which is the key performance optimization.

**Performance impact:** Without the feeder, every scheduler RPC would require a database query to find available work. The shared-memory cache allows the scheduler (which may have hundreds of concurrent CGI instances) to read candidate jobs without contention on the database.

#### 2.2.3 Scheduler (CGI)

The scheduler is a CGI (or FastCGI) program invoked by the Apache HTTP server for each client RPC. Many scheduler instances run concurrently. The scheduler:

1. Reads the client's request (XML over HTTP POST), which contains: platform info, available resources, completed results to report.
2. Processes reported results: marks them as completed in the database.
3. Selects new work from the shared-memory cache, considering: platform compatibility, resource requirements, host reliability, homogeneous redundancy class, deadline feasibility.
4. Writes the reply (XML): new jobs to execute, file download URLs, messages to display.

**Throughput:** A single server machine can dispatch ~8.8 million tasks per day (~100/second). With multi-host deployment, this scales to 23.6 million tasks/day (Anderson, Korpela, Walton 2005).

#### 2.2.4 Transitioner Daemon

The transitioner manages **state transitions** for workunits and results. It is the state machine driver for the backend. Key responsibilities:

- **Generate initial results:** When a workunit is created, the transitioner generates the initial set of result records (one per replica, based on the `target_nresults` parameter).
- **Timeout handling:** If a result's report deadline expires and no reply has arrived, set outcome to `NO_REPLY` and generate a replacement result.
- **Error propagation:** If a workunit enters an error state, cancel all unsent results.
- **Trigger assimilation:** When all results for a workunit reach `server_state = OVER`, set the workunit's `assimilate_state = READY`.

**Result `server_state` values:**
| State | Value | Meaning |
|-------|-------|---------|
| `UNSENT` | 1 | Created, waiting in queue |
| `IN_PROGRESS` | 2 | Sent to a client |
| `OVER` | 5 | Terminal: success, error, or timeout |

**Result `outcome` values (when `OVER`):**
| Outcome | Meaning |
|---------|---------|
| `SUCCESS` | Client returned a result |
| `COULDNT_SEND` | Scheduling failed |
| `CLIENT_ERROR` | Client reported an error |
| `NO_REPLY` | Deadline expired with no response |
| `DIDNT_NEED` | Cancelled (workunit already has enough results) |
| `VALIDATE_ERROR` | Validation failed |

#### 2.2.5 Validator Daemon

The validator compares results from redundant computation to find consensus. It is per-application (different apps may have different comparison logic). Two phases:

1. **Check phase:** Determine if a result is usable at all (e.g., output files exist, not corrupted). Project-supplied function: `check_pair(result1, result2) -> {match, no_match, inconclusive}`.
2. **Consensus phase:** When enough results pass the check, determine if a quorum agrees. If a quorum of `min_quorum` results match, the first matching result becomes the "canonical result" for that workunit.

**Validation states:**
| State | Meaning |
|-------|---------|
| `INIT` | Not yet validated |
| `VALID` | Matches canonical result |
| `INVALID` | Does not match canonical result |

BOINC provides generic validators (e.g., `sample_bitwise_validator` for exact byte comparison) and allows projects to supply custom comparison functions for fuzzy matching (e.g., floating-point results within epsilon).

#### 2.2.6 Assimilator Daemon

The assimilator handles completed, validated workunits. It is per-application and is linked with a project-supplied function that processes the canonical result. Typical actions:
- Move output files to a results directory.
- Parse output files and insert results into a project-specific database.
- Trigger downstream analysis pipelines.

The assimilator only runs after the validator has selected a canonical result (or determined that the workunit failed permanently).

#### 2.2.7 File Deleter Daemon

The file deleter reclaims disk space by removing input and output files that are no longer needed. It examines workunit and result state: once a workunit is fully assimilated and all its results are `OVER`, the associated files can be deleted. Files are stored in a hierarchical directory structure with 1024 subdirectories (hashed by filename) under the `upload/` and `download/` directories.

### 2.3 Client Components

#### 2.3.1 Core Client

The core client is a daemon (background process) that:
- Manages attachments to one or more projects.
- Periodically issues **Scheduler RPCs** to each project server to report completed work and request new work.
- Downloads input files and application binaries from the project's download server (HTTP GET).
- Uploads output files to the project's upload server (HTTP POST to a CGI handler).
- Schedules local CPU/GPU resources among multiple projects and applications.
- Checkpoints running applications (the app itself writes checkpoint files; the client manages their lifecycle).
- Manages persistent file transfers with resume capability (partial transfers restart from the last successful offset).

**RPC scheduling policy:** The client does not poll continuously. It uses an exponential backoff policy for server contact, with a default minimum interval. The server can also request a delay via `request_delay` in the scheduler reply.

#### 2.3.2 Science Applications

Science applications are the actual computational programs. They can be:
- **Native BOINC apps:** Linked against the BOINC API library. The API provides: progress reporting, checkpointing, communication with the core client (heartbeat).
- **Wrapper-based apps:** A BOINC-supplied `wrapper` program runs an unmodified executable. The wrapper handles the BOINC API calls on behalf of the legacy application.
- **VM-based apps:** A `vboxwrapper` runs applications inside a VirtualBox VM, providing a strong isolation sandbox. Used for untrusted or complex-dependency applications.

#### 2.3.3 BOINC Manager (GUI)

An optional graphical interface that allows volunteers to:
- Attach/detach projects.
- View task status and statistics.
- Configure resource usage preferences (CPU percentage, disk space, network bandwidth).
- Communicate with the core client via a **local GUI RPC protocol** (XML over TCP on localhost, port 31416).

### 2.4 Communication Model

All communication in BOINC is **HTTP-based and client-initiated** (pull model):

| Communication | Protocol | Direction | Format |
|--------------|----------|-----------|--------|
| Scheduler RPC | HTTP POST/reply | Client -> Server -> Client | XML |
| File download | HTTP GET | Client <- Server | Binary |
| File upload | HTTP POST (CGI) | Client -> Server | Binary |
| GUI RPC | TCP (localhost) | Manager <-> Core Client | XML |

**Key property:** The server never initiates connections to clients. This is essential for volunteer computing, where clients are behind NATs and firewalls. The client-initiated model means the server cannot push urgent messages or cancel running tasks in real time; it must wait for the client's next RPC.

**Scheduler RPC request** (simplified XML structure):
```xml
<scheduler_request>
  <platform_name>x86_64-pc-linux-gnu</platform_name>
  <core_client_major_version>7</core_client_major_version>
  <work_req_seconds>86400</work_req_seconds>
  <result_ack>  <!-- report completed results -->
    <name>wu_12345_0</name>
  </result_ack>
  <result>  <!-- completed results to report -->
    <name>wu_12345_1</name>
    <outcome>1</outcome>  <!-- SUCCESS -->
    <cpu_time>3600.5</cpu_time>
    <exit_status>0</exit_status>
  </result>
</scheduler_request>
```

**Scheduler RPC reply** (simplified):
```xml
<scheduler_reply>
  <result>  <!-- new work assignment -->
    <name>wu_67890_0</name>
    <wu_name>wu_67890</wu_name>
    <app_version_num>100</app_version_num>
    <report_deadline>1711555200</report_deadline>
  </result>
  <file_info>  <!-- files to download -->
    <name>input_67890.dat</name>
    <url>https://project.example.com/download/input_67890.dat</url>
    <md5_cksum>abc123...</md5_cksum>
    <nbytes>1048576</nbytes>
  </file_info>
</scheduler_reply>
```

---

## 3. Key Mechanisms

### 3.1 Redundant Computation and Quorum-Based Validation

**Problem:** In volunteer computing, clients are untrusted. A result may be incorrect due to: malicious intent, faulty hardware (overclocked CPUs, bad RAM), software bugs on specific platforms, or silent data corruption.

**Solution:** BOINC sends each workunit to multiple clients (replicas). The validator compares the results. If a quorum of `min_quorum` results agree (by project-defined comparison), the result is accepted as the "canonical result."

**Parameters per workunit:**
- `target_nresults`: Initial number of replicas to create (typically 2-3).
- `min_quorum`: Minimum matching results for consensus (typically 2).
- `max_error_results`: Maximum allowed errors before marking workunit as failed.
- `max_total_results`: Absolute maximum replicas before giving up.

**Adaptive replication:** To reduce the overhead of redundancy, BOINC tracks per-host reliability. For each `(host, app_version)` pair, it maintains a counter `CV` of consecutive validated results. Once `CV >= 10`, the host is considered "trusted" and its jobs are replicated with decreasing probability. This reduces redundancy overhead to as low as 5-10% of total CPU time for projects with reliable volunteer bases.

**Homogeneous redundancy:** For applications sensitive to floating-point non-determinism (different results on different architectures), BOINC groups hosts into "equivalence classes" (~15 to ~80 classes) and only compares results from hosts in the same class.

### 3.2 Workunit Lifecycle (State Machine)

The complete lifecycle of a workunit:

```
Work Generator creates workunit in DB
         |
         v
   [WORKUNIT CREATED]
         |
    Transitioner generates N result records (target_nresults)
         |
         v
   [RESULTS: UNSENT]
         |
    Feeder loads results into shared-memory cache
         |
    Scheduler picks results from cache, sends to clients
         |
         v
   [RESULTS: IN_PROGRESS]
         |
    +----+----+
    |         |
    v         v
 Client     Deadline
 reports    expires
 result     (NO_REPLY)
    |         |
    v         v
 [RESULT:  [RESULT:
  OVER      OVER
 SUCCESS]  NO_REPLY]
    |         |
    |    Transitioner creates replacement result
    |         |
    v         v
   Validator compares results when quorum is available
         |
    +----+----+
    |         |
    v         v
 [VALID]  [INVALID]
    |         |
    |    Generate more replicas or fail workunit
    |
    v
 Canonical result selected
    |
    v
 Assimilator processes canonical result
    |
    v
 File Deleter removes input/output files
    |
    v
   [WORKUNIT COMPLETE]
```

### 3.3 Security Model

BOINC addresses security from two perspectives: **protecting the volunteer** (from malicious project code) and **protecting the project** (from malicious volunteers).

**Protecting the volunteer:**
1. **Code signing:** Every project has an RSA key pair. The private key is kept offline (on a physically secure, network-disconnected machine). All application binaries and input files are signed with the private key. The client verifies signatures using the project's public key before executing anything. Even if the project server is compromised, attackers cannot distribute malicious code without the private key.
2. **Sandboxing (account-based):** On supported platforms, BOINC applications run under a dedicated low-privilege account (`boinc_project`). The core client uses a setuid helper (`switcher`) to drop privileges before executing science apps. This limits filesystem and network access.
3. **VM isolation:** For applications using VirtualBox, the VM provides a strong sandbox boundary. The science app runs inside the VM and cannot access the host filesystem directly.

**Protecting the project:**
1. **Redundant computation + validation:** As described in Section 3.1.
2. **Deadline enforcement:** If a client does not report by the deadline, the result is marked `NO_REPLY` and a replacement is generated.
3. **Adaptive replication:** Reduces cost while maintaining statistical confidence in result correctness.

### 3.4 Fault Tolerance Mechanisms

**Server-side resilience:**
- The multi-daemon architecture provides process-level fault isolation. If one daemon crashes (e.g., the assimilator due to an external database failure), other daemons continue operating. Work for the failed daemon accumulates in the MySQL database and is processed when the daemon restarts.
- The feeder/scheduler separation via shared memory means that a scheduler crash does not affect the feeder, and vice versa.

**Client-side resilience:**
- **Checkpointing:** Applications can periodically write checkpoint files. If the client crashes or the machine reboots, computation resumes from the last checkpoint.
- **Persistent file transfers:** Interrupted uploads/downloads resume from the last successful byte offset.
- **Exponential backoff:** If the server is unreachable, the client backs off exponentially, preventing thundering herd on server recovery.

**Work-level resilience:**
- **Deadline + replacement:** The transitioner automatically creates replacement results for timed-out jobs.
- **Error limits:** Workunits have configurable limits (`max_error_results`, `max_total_results`) to prevent infinite retry loops.

### 3.5 Scalability Architecture

BOINC's scalability comes from several architectural choices:

1. **Shared-memory feeder:** Eliminates per-RPC database queries for the critical dispatch path.
2. **Stateless scheduler:** The scheduler is a CGI program with no in-process state. Horizontal scaling is achieved by running more Apache/CGI processes (or using FastCGI).
3. **Asynchronous daemons:** The transitioner, validator, assimilator, and file deleter run independently and asynchronously, processing batches of records from the database.
4. **Multi-host deployment:** The web server, database server, and backend daemons can run on separate machines. The upload/download servers can be separated from the scheduler server.
5. **Hierarchical file storage:** Files are distributed across 1024 subdirectories to avoid filesystem bottlenecks.

---

## 4. Comparison with Relativist's Context

### 4.1 Fundamental Model Difference

| Dimension | BOINC | Relativist |
|-----------|-------|------------|
| **Computation model** | Bag-of-tasks (independent jobs) | Iterative synchronous graph reduction |
| **Inter-task communication** | None (jobs are independent) | Every round: partition -> distribute -> reduce -> collect -> merge -> resolve borders (SPEC-05) |
| **Client trust** | Untrusted (anonymous volunteers) | Trusted (controlled lab environment, SPEC-07 R44) |
| **Availability** | Intermittent (volunteers disconnect freely) | Stable (dedicated machines for experiment duration) |
| **Scale target** | Millions of hosts | 8 physical machines (SPEC-07, SPEC-09) |
| **Result verification** | Redundant computation + quorum validation | Deterministic by construction (strong confluence, SPEC-01) |
| **Network topology** | Star (all clients talk to server only) | Star (all workers talk to coordinator only) |
| **Communication frequency** | Low (RPCs every minutes/hours) | High (every round, potentially sub-second, SPEC-06) |
| **State management** | MySQL database (persistent, durable) | In-memory (coordinator holds Net in RAM, SPEC-02) |
| **Failure model** | Expected and handled (redundancy, deadlines) | Not tolerated in v1 (SPEC-07 R44, DISC-007 v2) |

### 4.2 Network Topology Similarity

Both BOINC and Relativist use a **centralized coordinator/server** that mediates all computation. Workers/clients never communicate directly with each other. This star topology simplifies reasoning about state consistency but creates a potential bottleneck at the center.

In BOINC, this is acceptable because: (a) jobs are independent, so the server only needs to dispatch and collect, not coordinate; (b) the shared-memory feeder architecture handles high dispatch rates.

In Relativist, the coordinator is a tighter bottleneck because: (a) it must perform merge and border-redex resolution between rounds (SPEC-05); (b) every round requires a full serialize -> send -> reduce -> receive -> deserialize cycle for all partitions (SPEC-06); (c) the coordinator holds the entire Net in memory and performs the partitioning step.

### 4.3 Fault Tolerance: Opposite Ends of the Spectrum

BOINC is designed for the most adversarial fault model in distributed computing: untrusted, unreliable, heterogeneous, intermittent clients. Its entire architecture (redundancy, validation, deadlines, checkpointing, code signing) addresses this.

Relativist explicitly excludes fault tolerance from v1 scope (SPEC-07 R44, DISC-007 v2). The environment is a controlled lab with 8 trusted machines running for the duration of a benchmark. This is a valid scope decision for a TCC prototype, but it means Relativist's architecture is fragile: a single worker crash halts the entire computation (DISC-007 v2, Section 1.2, failure mode F-a).

### 4.4 Communication Protocol

| Aspect | BOINC | Relativist |
|--------|-------|------------|
| Transport | HTTP (TCP underneath) | Raw TCP with persistent connections (SPEC-06) |
| Framing | HTTP content-length | 8-byte header: 4B length + 4B CRC32 (SPEC-06 R6) |
| Serialization | XML (human-readable) | bincode (binary, compact) (SPEC-06 R4) |
| Initiation | Client-initiated (pull) | Coordinator-initiated (push) |
| Connection lifetime | Per-RPC (HTTP request/response) | Persistent for entire grid loop (SPEC-06 R21) |
| Integrity | MD5 checksum on files | CRC32 per frame (SPEC-06 R6) |
| Encryption | Optional HTTPS | None in v1 (SPEC-07 R44) |

BOINC's HTTP/XML protocol prioritizes universality (any HTTP client can interact) and firewall traversal (port 80/443). Relativist's raw TCP/bincode protocol prioritizes low latency and high throughput for the tight reduce-merge loop.

### 4.5 Work Distribution Strategy

**BOINC:** Pull-based. Clients request work when they have capacity. The scheduler selects work based on platform, reliability, and deadline feasibility. Work is pre-generated and stored in the database.

**Relativist:** Push-based. The coordinator partitions the current Net (SPEC-04) and pushes partitions to workers at the start of each round (SPEC-06, Section 4.6). Workers cannot request work; they receive exactly one partition per round.

The pull model is necessary for BOINC because clients are heterogeneous and intermittent. The push model is appropriate for Relativist because: (a) all workers are identical and always available; (b) the computation is synchronous (all workers must finish before the next round); (c) the coordinator knows the entire graph and can make optimal partitioning decisions.

---

## 5. Lessons for Relativist (ADOPT / ADAPT / REJECT)

### L1. Shared-Memory Feeder for Scheduler Performance -- REJECT

**BOINC mechanism:** A feeder daemon pre-loads work items into shared memory. Scheduler CGI instances read from this cache instead of querying the database.

**Relevance to Relativist:** None. Relativist has a single coordinator process (not multiple CGI instances), no database, and the coordinator already holds the Net in memory. The problem this solves (database query bottleneck under concurrent CGI load) does not exist in Relativist's architecture.

**Verdict: REJECT.** The problem domain does not apply.

### L2. Multi-Daemon Architecture with Process Isolation -- ADAPT

**BOINC mechanism:** Separate daemons (feeder, transitioner, validator, assimilator, file deleter) communicate via the database. If one daemon crashes, others continue. Work accumulates and is processed when the failed daemon recovers.

**Relevance to Relativist:** Relativist v1 is a monolithic single-binary design (SPEC-07 R1). This is correct for the TCC scope: 8 machines, no fault tolerance, simplicity is paramount. However, the principle of separating concerns into independent processing stages is sound.

**Adaptation:** In future versions (post-TCC), if Relativist grows to handle larger grids or fault tolerance, the coordinator could be decomposed into separate stages (partitioner, dispatcher, merger, result processor) communicating via channels. For v1, keep the monolith but structure the code as clearly separated modules (SPEC-13 candidate).

**Verdict: ADAPT for v2+.** Keep monolith for v1, but ensure clean module boundaries that could later become process boundaries.

### L3. Redundant Computation for Untrusted Environments -- REJECT

**BOINC mechanism:** Send each job to multiple clients. Compare results. Accept only when a quorum agrees. Use adaptive replication to reduce overhead for trusted hosts.

**Relevance to Relativist:** None for v1. Relativist operates in a trusted environment (SPEC-07 R44). Furthermore, Interaction Combinators have **strong confluence** (SPEC-01): the reduction result is deterministic regardless of reduction order. There is no need to verify correctness by redundancy because correctness is guaranteed by the mathematical properties of the system.

**Important nuance:** Strong confluence guarantees determinism of the *reduction* but not of the *implementation*. A bug in the reduction engine could produce wrong results that would go undetected. BOINC's redundancy would catch such bugs. However, for a TCC prototype, the test strategy (SPEC-08) with round-trip property verification is the appropriate mechanism, not runtime redundancy.

**Verdict: REJECT.** Strong confluence makes runtime redundancy unnecessary. Implementation bugs are caught by the test suite (SPEC-08), not by redundant execution.

### L4. Deadline-Based Timeout with Replacement -- ADAPT

**BOINC mechanism:** Each result has a `report_deadline`. If the client does not report by the deadline, the transitioner marks it `NO_REPLY` and generates a replacement result. This handles client crashes, disconnections, and stragglers.

**Relevance to Relativist:** Relativist v1 has timeouts (SPEC-06 R30: `collect_timeout` of 600 seconds) but no recovery mechanism. If a worker times out, the coordinator reports an error and the computation fails (DISC-007 v2, failure mode F-a).

**Adaptation:** For v2+, Relativist could implement a deadline + reassignment mechanism: if a worker does not respond within `collect_timeout`, the coordinator could reassign that partition to another worker (or reduce it locally). This is feasible because the coordinator retains the original partition data until it receives the result. Strong confluence guarantees that re-reducing the same partition on a different worker produces the same result.

**Verdict: ADAPT for v2+.** The mechanism is sound and compatible with IC properties. For v1, the existing timeout-and-fail behavior (SPEC-06 R30) is sufficient.

### L5. Code Signing for Application Integrity -- REJECT

**BOINC mechanism:** All application binaries are signed with an offline private key. Clients verify signatures before execution.

**Relevance to Relativist:** None. Relativist is a single binary deployed manually by the researcher (SPEC-07 R41) or via Docker (SPEC-07 R37). The binary is compiled from source by the user. There is no concept of downloading and executing untrusted code.

**Verdict: REJECT.** The threat model does not apply.

### L6. Sandboxing for Client Protection -- REJECT

**BOINC mechanism:** Applications run under a low-privilege account, with optional VM isolation.

**Relevance to Relativist:** Workers in Relativist execute the reduction engine, which is pure graph manipulation (SPEC-03). The worker does not execute arbitrary code; it only applies the 6 IC reduction rules. There is no sandboxing concern.

**Verdict: REJECT.** The threat model does not apply.

### L7. Exponential Backoff for Connection Retry -- ADOPT

**BOINC mechanism:** Clients use exponential backoff when the server is unreachable, preventing thundering herd on recovery.

**Relevance to Relativist:** Already adopted. SPEC-06 R23 specifies exponential backoff for worker connection retry (`base_delay * 2^attempt`, capped at 30 seconds, maximum 10 attempts). This was inspired by standard distributed systems practice and is consistent with BOINC's approach.

**Verdict: ADOPT (already implemented in spec).** SPEC-06 R23 already specifies this.

### L8. Persistent File Transfers with Resume -- REJECT

**BOINC mechanism:** File transfers support resume from the last byte offset.

**Relevance to Relativist:** Relativist does not transfer files during the grid loop. It transfers serialized partitions over persistent TCP connections (SPEC-06). If a connection fails mid-transfer, the entire round fails (no partial message recovery). Given the small partition sizes expected (SPEC-07, Section 4.6: ~5 KB to ~5 MB) and the trusted network environment, resume is unnecessary overhead.

**Verdict: REJECT.** Message sizes are small, connections are persistent, and the trusted LAN environment makes this unnecessary.

### L9. Asynchronous Backend Processing -- ADAPT

**BOINC mechanism:** Backend daemons (transitioner, validator, assimilator) process work asynchronously. They scan the database for work that needs attention, process it, and sleep briefly before scanning again. This decouples processing stages.

**Relevance to Relativist:** Relativist v1 is synchronous: the coordinator waits for all workers before proceeding to merge (SPEC-05, SPEC-06). This is correct for the BSP-like computation model. However, within the coordinator's merge phase, individual steps (deserializing partition results, rebuilding the free port index, resolving border redexes) could potentially be pipelined.

**Adaptation:** The coordinator could begin deserializing worker results as they arrive (instead of waiting for all workers to finish before processing any result). This is a minor optimization for v1 but becomes significant with more workers.

**Verdict: ADAPT for v1 (minor).** Consider processing worker results as they arrive in `collect` phase. The coordinator already uses tokio (SPEC-07), so this is architecturally feasible.

### L10. Hierarchical File Storage with Hashing -- REJECT

**BOINC mechanism:** Files distributed across 1024 subdirectories hashed by filename.

**Relevance to Relativist:** Relativist does not manage a file hierarchy during operation. Input/output is a single `.bin` file (SPEC-07, Section 4.6). The `generate` subcommand produces one file. No filesystem scalability concern exists.

**Verdict: REJECT.** Not applicable.

### L11. Workunit State Machine with Clear Lifecycle -- ADOPT

**BOINC mechanism:** Every workunit and result has a well-defined state machine with explicit states (`UNSENT`, `IN_PROGRESS`, `OVER`) and transitions managed by a dedicated component (transitioner).

**Relevance to Relativist:** Relativist's coordinator and worker already have implicit state machines described in SPEC-06 (Section 4.3: Coordinator FSM, Section 4.4: Worker FSM). The lesson from BOINC is to make these state machines explicit and formally defined, with clear transition triggers and invariants.

**Adaptation:** SPEC-13 (System Architecture) should formalize the coordinator and worker FSMs with named states, transition conditions, and error states. SPEC-06 already has the foundation; SPEC-13 should refine it.

**Verdict: ADOPT.** Formalize FSMs in SPEC-13.

### L12. XML-Based Protocol -- REJECT

**BOINC mechanism:** All communication uses XML for serialization.

**Relevance to Relativist:** Relativist uses bincode (SPEC-06 R4), which is 5-10x more compact and faster to serialize/deserialize than XML. Given the tight reduce-merge loop with sub-second round targets (SPEC-09), binary serialization is essential.

**Verdict: REJECT.** bincode is the correct choice for Relativist's latency-sensitive, high-frequency communication pattern.

---

## 6. Comparison Table (BOINC vs Relativist)

| Dimension | BOINC | Relativist | Notes |
|-----------|-------|------------|-------|
| **Year / Maturity** | 2002-present, production | 2026, TCC prototype | Different maturity levels |
| **Language** | C++ (server + client) | Rust (single binary) | |
| **Computation model** | Bag-of-tasks | Iterative synchronous graph reduction | Fundamentally different |
| **Network topology** | Star (server-centric) | Star (coordinator-centric) | Same topology |
| **Communication protocol** | HTTP + XML | TCP + bincode | Relativist optimizes for latency |
| **Connection pattern** | Stateless per-RPC | Persistent TCP | Relativist avoids reconnection cost |
| **Work distribution** | Pull (client-initiated) | Push (coordinator-initiated) | Different trust/availability models |
| **Trust model** | Untrusted clients | Trusted workers | Biggest architectural difference |
| **Fault tolerance** | Comprehensive (redundancy, deadlines, checkpoints) | None in v1 | BOINC's primary design concern |
| **Result verification** | Quorum-based validation | Guaranteed by strong confluence | IC properties eliminate need |
| **Security** | Code signing + sandboxing + VM | None in v1 | Different threat models |
| **Scale** | Millions of hosts | 8 machines | Different orders of magnitude |
| **State persistence** | MySQL database | In-memory only | Relativist: no durability needed |
| **Scheduling** | Complex (platform, reliability, deadlines) | Simple round-robin partitioning | Relativist: homogeneous environment |
| **Inter-job dependency** | None (independent jobs) | Total (every round depends on previous) | Critical difference for design |
| **Communication frequency** | Low (~1 RPC per minutes/hours) | High (~1 exchange per round, sub-second) | Drives protocol design choices |
| **Serialization format** | XML | bincode | bincode: ~10x faster, ~5x smaller |
| **Data integrity** | MD5 on files | CRC32 per frame | Both adequate for their context |
| **Configuration** | Config files + web interface | CLI only (SPEC-07 R10) | Relativist: simplicity for v1 |
| **Deployment** | Multi-server, complex setup | Single binary + Docker (SPEC-07) | Relativist: minimal operational burden |

---

## 7. Sources

### Academic Papers

- Anderson, D.P. (2019). "BOINC: A Platform for Volunteer Computing." *Journal of Grid Computing*, 18, 99-122. [arXiv:1903.01699](https://arxiv.org/pdf/1903.01699)
- Anderson, D.P., Korpela, E., Walton, R. (2005). "High-Performance Task Distribution for Volunteer Computing." *Proceedings of the First International Conference on e-Science and Grid Computing*, pp. 196-203, IEEE. [PDF](https://boinc.berkeley.edu/boinc_papers/server_perf/server_perf.pdf)
- Anderson, D.P. (2010). "Volunteer Computing: The Ultimate Cloud." *ACM Crossroads*, 16(3), 7-10. [PDF](https://boinc.berkeley.edu/boinc_papers/crossroads.pdf)

### BOINC Official Documentation

- [BOINC Server Introduction (Wiki)](https://github.com/BOINC/boinc/wiki/ServerIntro)
- [Backend State Machine](https://boinc.berkeley.edu/trac/wiki/BackendState)
- [Backend Logic (Transitions)](https://boinc.berkeley.edu/trac/wiki/BackendLogic)
- [Backend Programs (Daemons)](https://boinc.berkeley.edu/trac/wiki/BackendPrograms)
- [Job Replication and Validation](https://boinc.berkeley.edu/trac/wiki/JobReplication)
- [Adaptive Replication](https://boinc.berkeley.edu/trac/wiki/AdaptiveReplication)
- [Homogeneous Redundancy](https://github.com/BOINC/boinc/wiki/Homogeneous-Redundancy)
- [Validation Introduction](https://boinc.berkeley.edu/trac/wiki/ValidationIntro)
- [BOINC Security](https://github.com/BOINC/boinc/wiki/BOINC_Security)
- [Code Signing](https://github.com/BOINC/boinc/wiki/CodeSigning)
- [Sandbox Design](https://boinc.berkeley.edu/sandbox_design.php)
- [Scheduler RPC Protocol](https://github.com/BOINC/boinc/wiki/RpcProtocol)
- [GUI RPC Protocol](https://github.com/BOINC/boinc/wiki/GuiRpcProtocol)
- [File Upload Protocol](https://github.com/BOINC/boinc/wiki/FileUpload)
- [BOINC Apps Introduction](https://github.com/BOINC/boinc/wiki/BOINC-apps-(introduction))
- [BOINC Client Overview](https://github.com/BOINC/boinc/wiki/BOINC-Client)

### BOINC Source Code

- [BOINC GitHub Repository](https://github.com/BOINC/boinc)
- [Transitioner Source (C++)](https://github.com/BOINC/boinc/blob/master/sched/transitioner.cpp)

### Wikipedia

- [BOINC Client-Server Technology](https://en.wikipedia.org/wiki/BOINC_client%E2%80%93server_technology)
- [Berkeley Open Infrastructure for Network Computing](https://en.wikipedia.org/wiki/Berkeley_Open_Infrastructure_for_Network_Computing)
