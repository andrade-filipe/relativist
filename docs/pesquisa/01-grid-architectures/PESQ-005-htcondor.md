---
pesq_id: PESQ-005
title: "HTCondor: High-Throughput Computing Architecture"
category: Grid Computing Architectures
date_created: 2026-03-25
status: Complete
---

# PESQ-005: HTCondor -- High-Throughput Computing Architecture

**Category:** Grid Computing Architectures
**Status:** Complete
**Cross-references:**
- Specs: SPEC-10 (security/auth), SPEC-07 (deployment), SPEC-06 (wire protocol)
- References: REF-017 (Foster 2001), REF-007 (Casanova 2002)
- Discussions: DISC-007 v2 (fault tolerance), DISC-005 v2 (cross-boundary protocol)

---

## 1. Subject Overview

HTCondor is an open-source high-throughput computing (HTC) software framework developed at the University of Wisconsin-Madison, led by Miron Livny since 1988 (originally named "Condor"; renamed in 2012 to resolve a trademark dispute). It is designed for coarse-grained distributed parallelization of computationally intensive tasks across opportunistically harvested resources -- workstations, clusters, and cloud VMs that may be shared with other uses and are not guaranteed to be continuously available.

**Scale:** HTCondor is used by major scientific collaborations (LIGO, CMS at CERN, ATLAS) and manages pools ranging from hundreds to hundreds of thousands of execution slots. The Open Science Pool (OSPool), operated by the OSG Consortium, federates resources from dozens of institutions using HTCondor.

**Computation model:** HTCondor implements a **high-throughput computing** model, which the HTCondor team distinguishes from high-performance computing (HPC). HPC maximizes FLOPS over short periods; HTC maximizes total floating-point operations over months or years by efficiently utilizing all available cycles, including idle workstations. Jobs are independent, batch-oriented tasks with no inter-job communication during execution. This is fundamentally different from Relativist's **iterative synchronous graph reduction** (SPEC-05), where workers reduce partitions of a single shared interaction net and must synchronize every round.

**Key design constraints that shaped HTCondor:**
1. **Distributed ownership:** Resources belong to different owners with different policies (e.g., "run jobs only when idle").
2. **Heterogeneous and opportunistic resources:** Machines vary in OS, CPU, memory, and availability.
3. **Long-running jobs on preemptable resources:** Jobs may be evicted at any time; checkpoint/restart is essential.
4. **No inter-job communication:** Jobs are independent batch processes.
5. **Multi-pool federation:** Resources from different administrative domains must be sharable.

**Primary references:**
- Thain, D., Tannenbaum, T., Livny, M. (2005). "Distributed Computing in Practice: The Condor Experience." *Concurrency and Computation: Practice and Experience*, 17(2-4), 323-356.
- Litzkow, M., Livny, M., Mutka, M. (1988). "Condor: A Hunter of Idle Workstations." *Proceedings of the 8th International Conference on Distributed Computing Systems*, pp. 104-111.

---

## 2. Architecture / Design

### 2.1 High-Level Architecture

HTCondor uses a **three-role architecture**: Central Manager, Submit Nodes (Access Points), and Execute Nodes (Execution Points). Each role runs specific daemons. A single machine may serve multiple roles.

```
+---------------------------------------------------+
|               CENTRAL MANAGER                      |
|                                                    |
|  +-------------+       +---------------+           |
|  | Collector   |<------| Negotiator    |           |
|  | (ClassAd    |       | (matchmaking  |           |
|  |  aggregator)|       |  + priority)  |           |
|  +------^------+       +-------+-------+           |
|         |                      |                   |
+---------|---------+------------|-------------------+
          |         |            |
   periodic ads   periodic ads  match notification
          |         |            |
+---------|---------+------------|-------------------+
|         |  SUBMIT NODE         |                   |
|  +------+------+        +-----v-----+             |
|  | Schedd      |<------>| Shadow    |  (per-job)  |
|  | (job queue) |        | (monitor) |             |
|  +------+------+        +-----------+             |
|         |                                          |
+---------|------------------------------------------+
          | claim
          v
+---------------------------------------------------+
|         EXECUTE NODE                               |
|                                                    |
|  +-------------+       +---------------+           |
|  | Startd      |------>| Starter       | (per-job) |
|  | (resource   |       | (sandbox +    |           |
|  |  manager)   |       |  execution)   |           |
|  +-------------+       +---------------+           |
|                                                    |
+---------------------------------------------------+
```

### 2.2 Central Manager Components

#### 2.2.1 Collector (condor_collector)

The Collector is the information aggregator for the entire pool. Every daemon in the pool periodically sends ClassAd updates to the Collector (typically every 5 minutes). The Collector maintains an in-memory database of all current ClassAds: machine ClassAds (from Startds), job queue summaries (from Schedds), and daemon status ads. It is a read-heavy, write-periodic service. Tools like `condor_status` query the Collector to display pool state.

#### 2.2.2 Negotiator (condor_negotiator)

The Negotiator runs the **matchmaking cycle** periodically (default: every 60 seconds). In each cycle it:
1. Fetches all machine ClassAds from the Collector.
2. Fetches all Schedd ClassAds (summaries of waiting jobs) from the Collector.
3. Sorts submitters by fair-share priority (users who have consumed more resources get lower priority).
4. For each submitter in priority order, contacts the Schedd to get individual job ClassAds, then attempts to match each job against available machine ClassAds.
5. A match occurs when both the job's `Requirements` expression evaluates to true in the context of the machine ad, AND the machine's `Requirements` expression evaluates to true in the context of the job ad.
6. Sends match notifications to both the Schedd and the Startd.

After notification, the Schedd's Shadow daemon contacts the Startd directly to claim the resource and begin execution. The Negotiator is not in the data path after matchmaking.

### 2.3 Submit Node Components

#### 2.3.1 Schedd (condor_schedd)

The Schedd manages the **persistent job queue** for a submit node. It stores job ClassAds in a transaction log on disk (not a database -- a sequential append log with periodic snapshots). The Schedd:
- Accepts job submissions (`condor_submit`).
- Advertises queue summaries to the Collector.
- Participates in the negotiation cycle by presenting job ClassAds to the Negotiator.
- Spawns a Shadow process for each running job to manage its execution remotely.

#### 2.3.2 Shadow (condor_shadow)

The Shadow is a per-job process spawned by the Schedd. It acts as the submit-side agent for a running job: it provides the execution node with input files (via HTCondor's file transfer mechanism), receives output files when the job completes, handles checkpoint file transfers, and logs job events. If a job is evicted, the Shadow handles the checkpoint retrieval and re-queues the job.

### 2.4 Execute Node Components

#### 2.4.1 Startd (condor_startd)

The Startd manages compute resources on an execute node. It:
- Advertises machine ClassAds to the Collector (CPU count, memory, disk, OS, architecture, load average, keyboard idle time, owner-defined policy expressions).
- Enforces the **resource owner's policy**: when to accept jobs (e.g., only when keyboard idle > 15 min), when to suspend them, when to evict them (e.g., when the owner returns).
- Manages **slots**: a machine can be divided into multiple slots (static or dynamic/partitionable), each advertised independently.
- Accepts claims from Schedds and spawns a Starter for each claimed slot.

#### 2.4.2 Starter (condor_starter)

The Starter is a per-job process spawned by the Startd. It creates a sandbox directory, stages input files, launches the user's executable, monitors resource usage (CPU, memory, disk), enforces resource limits, and stages output files back to the Shadow upon completion. The Starter is the process isolation boundary for the job.

### 2.5 Job States

HTCondor defines these job states:

| State | Code | Meaning |
|-------|------|---------|
| Idle | 1 | Waiting for a match (in Schedd queue) |
| Running | 2 | Executing on an execute node |
| Removed | 3 | Cancelled by user (`condor_rm`) |
| Completed | 4 | Finished execution successfully |
| Held | 5 | Paused due to error or policy; requires release |
| Transferring Output | 6 | Job finished, output files being transferred |
| Suspended | 7 | Running but temporarily suspended (owner activity) |

---

## 3. Key Mechanisms

### 3.1 ClassAds: Matchmaking Language

ClassAds (Classified Advertisements) are HTCondor's declarative, semi-structured data language. A ClassAd is a set of named attribute-expression pairs. Expressions can reference attributes from a counterpart ad, enabling **bilateral matching**: both the job and the machine must agree to the match.

**Example machine ClassAd (simplified):**
```
MyType = "Machine"
Name = "slot1@worker01.lab.edu"
Arch = "X86_64"
OpSys = "LINUX"
Memory = 32768
Cpus = 8
TotalDisk = 500000000
LoadAvg = 0.05
KeyboardIdle = 86400
Requirements = (TARGET.RequestMemory <= Memory) && (TARGET.RequestDisk <= TotalDisk)
Rank = TARGET.ImageSize
```

**Example job ClassAd (simplified):**
```
MyType = "Job"
Owner = "researcher"
Cmd = "/home/researcher/simulate"
RequestMemory = 4096
RequestDisk = 10000000
RequestCpus = 1
Requirements = (TARGET.Arch == "X86_64") && (TARGET.OpSys == "LINUX") && (TARGET.Memory >= 4096)
Rank = TARGET.Memory
```

A match requires: `job.Requirements` evaluates to `true` given the machine ad as `TARGET`, AND `machine.Requirements` evaluates to `true` given the job ad as `TARGET`. The `Rank` attribute is used to prefer one match over another when multiple matches exist.

**Key properties of ClassAds:**
- **Symmetric evaluation:** Both sides have Requirements and Rank.
- **Late binding:** Expressions are evaluated at match time, not submission time.
- **Extensible:** Administrators and users can add arbitrary attributes.
- **Expression language:** Supports arithmetic, string operations, conditionals, regex matching, list operations.

### 3.2 Universes

HTCondor "universes" determine the execution runtime for a job:

| Universe | Description | Use Case |
|----------|-------------|----------|
| **vanilla** | Default. File transfer, no checkpoint support from HTCondor itself. | Most jobs |
| **docker** | Runs job inside a Docker container on the execute node. | Reproducible environments |
| **container** | Generalized container support (Docker, Singularity/Apptainer). | HPC + HTC convergence |
| **vm** | Runs a full virtual machine image (KVM, Xen). | Legacy OS, strong isolation |
| **grid** | Submits to external batch systems (SLURM, PBS, ARC, another HTCondor). | Multi-system federation |
| **local** | Runs on the submit node itself. | Lightweight pre/post processing |
| **scheduler** | Runs as a Schedd-managed process (used by DAGMan). | Workflow meta-schedulers |
| **standard** | (Deprecated) Provided transparent checkpointing via modified binaries. | Historical |

### 3.3 Checkpoint/Restart

HTCondor has evolved through multiple checkpoint strategies:

**1. Standard Universe (deprecated):** Required relinking the user's executable with a special HTCondor library. The library intercepted system calls and could capture the complete process state (registers, memory, open files) to produce a checkpoint image. This was transparent to the application but fragile: it required specific compilers, did not support threads, and was limited to specific platforms. Deprecated and removed in HTCondor 23.0.

**2. Self-Checkpointing (current approach):** The application itself is responsible for writing checkpoint files. HTCondor provides infrastructure to manage these files:
- **`checkpoint_exit_code`:** The application exits with a designated exit code to signal "I have written a checkpoint." HTCondor transfers the checkpoint files to the submit node for safekeeping, then immediately restarts the application in the same sandbox. If the job is later evicted and rescheduled, HTCondor transfers the checkpoint files to the new execute node before starting.
- **`+WantCheckpointSignal = true`:** HTCondor sends a configurable signal (e.g., SIGUSR2) to the application before eviction, giving it time to write a checkpoint. The application must handle the signal and write its state within the configured `MaxVacateTime`.
- **Periodic checkpointing:** HTCondor can be configured to periodically signal the application or trigger a checkpoint-exit cycle at regular intervals, providing fault tolerance against unexpected failures.

**3. Container/VM checkpoint:** For docker/container universe jobs, HTCondor can leverage container runtime checkpoint mechanisms (e.g., CRIU for Linux containers).

### 3.4 Security Model

HTCondor's security has evolved significantly, transitioning from GSI (Grid Security Infrastructure, X.509 certificates) to a modern token-based model:

**Authentication methods (current, as of v25.x):**

| Method | Description | Status |
|--------|-------------|--------|
| **IDTOKENS** | JWT-based tokens signed by a pool-specific signing key. Easy to issue and manage. Recommended for new installations. | Primary |
| **SSL** | TLS/SSL with X.509 certificates for daemon-to-daemon authentication. Uses CA-signed or self-signed certs. | Supported |
| **SCITOKENS** | OAuth2-based tokens for federated authorization. Used in scientific computing (CMS, LIGO). Tokens carry capability-based authorizations. | Federated use |
| **Kerberos** | Kerberos v5 tickets for authentication. Common in enterprise/university environments with existing Kerberos infrastructure. | Supported |
| **FS/FS_REMOTE** | Filesystem-based authentication (checks file ownership). Used for local connections. | Local only |
| **GSI** | X.509 proxy certificates (Grid Security Infrastructure). Being phased out in favor of tokens. | Deprecated |
| **PASSWORD** | Shared secret (pool password). Simple but limited. | Simple setups |
| **MUNGE** | HPC-oriented authentication using the MUNGE credential service. | HPC integration |
| **CLAIMTOBE** | No actual authentication; the peer simply claims an identity. Only for testing. | Testing only |

**Authorization model:** After authentication, HTCondor checks authorization using an Access Control List (ACL) based on security levels:

| Level | Meaning |
|-------|---------|
| READ | Query pool state (condor_status, condor_q) |
| WRITE | Submit jobs, update ClassAds |
| ADMINISTRATOR | Reconfigure daemons, drain nodes |
| DAEMON | Inter-daemon communication |
| NEGOTIATOR | Match-making operations |
| CONFIG | Remote configuration changes |

**Key security architecture patterns:**
- **Layered authentication:** Multiple methods can be configured in priority order; the first mutually supported method is used.
- **Per-daemon configuration:** Each daemon type can require different authentication/authorization levels.
- **Token signing keys:** For IDTOKENS, a signing key is generated once and shared among all daemons in the pool. Tokens are JWTs with claims for identity and authorization.
- **Encryption:** All daemon-to-daemon communication can be encrypted (TLS). Encryption is negotiable per-connection.

### 3.5 Flocking: Inter-Pool Resource Sharing

Flocking allows jobs from one HTCondor pool to execute on resources in another pool. This is HTCondor's mechanism for **federation without merging administrative domains**.

**How flocking works:**
1. A Schedd is configured with a list of remote Collectors (`FLOCK_TO`).
2. The Schedd advertises its queue to remote Collectors in addition to its home Collector.
3. Remote Negotiators include the flocked Schedd in their matchmaking cycles, typically at lower priority than local submitters.
4. If a match is made, the Schedd's Shadow contacts the remote Startd directly to claim the resource.

**Key properties:**
- **Unidirectional or bidirectional:** Pool A can flock to Pool B without B flocking to A.
- **Priority-aware:** Remote (flocked) jobs get lower priority than local jobs by default.
- **Transparent to users:** Users submit jobs normally; flocking happens automatically if local resources are insufficient.
- **Security boundary:** Each pool maintains its own authentication/authorization; cross-pool trust must be explicitly configured.

### 3.6 DAGMan: Workflow Management

DAGMan (Directed Acyclic Graph Manager) is HTCondor's built-in workflow engine. It expresses job dependencies as a DAG and manages execution order.

**Key properties:**
- DAGMan itself runs as an HTCondor job (scheduler universe), providing fault tolerance for the workflow manager.
- Nodes can be individual jobs or sub-DAGs (recursive composition).
- Supports PRE/POST scripts for each node (setup/teardown around the actual job).
- Supports RETRY with configurable limits per node.
- Writes a rescue DAG on failure, allowing the workflow to be restarted from the point of failure.
- Throttling: limits on concurrent running nodes to prevent resource exhaustion.

**DAG file syntax (example):**
```
JOB  A  a.sub
JOB  B  b.sub
JOB  C  c.sub
JOB  D  d.sub

PARENT A CHILD B C
PARENT B C CHILD D
```

This expresses: A runs first; B and C run in parallel after A completes; D runs after both B and C complete.

---

## 4. Comparison with Relativist's Context

### 4.1 Fundamental Model Difference

| Dimension | HTCondor | Relativist |
|-----------|----------|------------|
| **Computation model** | Batch HTC (independent jobs) | Iterative synchronous graph reduction |
| **Inter-job communication** | None during execution | Every round: partition -> reduce -> merge (SPEC-05) |
| **Resource ownership** | Distributed (different owners, policies) | Centralized (researcher controls all 8 machines) |
| **Job duration** | Minutes to days | Sub-second to seconds per round |
| **Scheduling frequency** | Every 60 seconds (negotiation cycle) | Every round (potentially sub-second, SPEC-09) |
| **Trust model** | Semi-trusted (known users, policy enforcement) | Fully trusted (controlled lab, SPEC-07 R44) |
| **Scale target** | Hundreds to hundreds of thousands of slots | 8 machines (SPEC-07, SPEC-09) |
| **Network topology** | Star (Collector-centric) + direct Schedd-Startd | Star (coordinator-centric, SPEC-06) |
| **State management** | Persistent transaction log (Schedd) | In-memory (coordinator, SPEC-02) |
| **Failure model** | Expected (eviction, preemption, crashes) | Not tolerated in v1 (SPEC-07 R44) |

### 4.2 Communication Architecture

| Aspect | HTCondor | Relativist |
|--------|----------|------------|
| **Transport** | TCP with HTCondor-specific protocol (Cedar/ReliSock) | TCP with length-prefixed bincode (SPEC-06) |
| **Serialization** | ClassAds (text) + custom binary for file transfer | bincode (binary, compact, SPEC-06 R4) |
| **Connection pattern** | Per-operation (daemons connect as needed) | Persistent TCP for entire grid loop (SPEC-06 R21) |
| **Communication direction** | Multi-directional (Schedd<->Collector, Schedd<->Startd, etc.) | Bidirectional coordinator<->worker only |
| **Matchmaking** | Declarative bilateral matching (ClassAds) | None; coordinator assigns partitions directly |
| **Security** | Layered (IDTOKENS, SSL, SCITOKENS, etc.) | None in v1 (SPEC-07 R44) |

### 4.3 Scheduling Philosophy

HTCondor's matchmaking is a **declarative, bilateral, priority-aware** process: jobs express requirements, machines express requirements, and the Negotiator finds mutually compatible matches weighted by fair-share priority. This is necessary because resources have diverse owners with diverse policies.

Relativist has no matchmaking. The coordinator knows all workers, all workers are identical, and all workers are always available. Partitioning (SPEC-04) determines which portion of the net each worker receives. The assignment is a direct push, not a negotiated match.

### 4.4 Checkpoint/Restart vs Relativist's Model

HTCondor's checkpoint mechanisms address **long-running jobs on preemptable resources**. Jobs may run for hours or days, and eviction can happen at any time. Without checkpointing, all progress is lost.

Relativist's "jobs" are reduction rounds lasting sub-second to seconds. If a worker fails mid-round, re-executing the entire round is cheap (DISC-007 v2). The coordinator retains the original partition data until results are collected. There is no need for application-level checkpointing in v1. For v2+, if Relativist were to support larger nets with longer reduction times per round, the concept of periodic state snapshots (inspired by HTCondor's self-checkpoint model) could become relevant.

---

## 5. Lessons for Relativist (ADOPT / ADAPT / REJECT)

### L1. ClassAds Bilateral Matchmaking -- REJECT

**HTCondor mechanism:** A declarative expression language where both job and machine express requirements and preferences. The Negotiator evaluates expressions bilaterally to find compatible matches.

**Relevance to Relativist:** None. Relativist's workers are homogeneous and fully controlled by the coordinator. There is no concept of "machine requirements" or "job preferences" -- the coordinator directly assigns partitions based on net topology (SPEC-04). Introducing matchmaking would add complexity with no benefit in a homogeneous, controlled environment.

**Verdict: REJECT.** The problem ClassAds solves (heterogeneous resources with distributed ownership) does not exist in Relativist's deployment model.

### L2. Layered, Pluggable Security Authentication -- ADAPT (for SPEC-10)

**HTCondor mechanism:** Multiple authentication methods (IDTOKENS, SSL, SCITOKENS, Kerberos, etc.) are configured in priority order. Daemons negotiate the strongest mutually supported method. Authorization is separate from authentication (ACL-based levels: READ, WRITE, DAEMON, ADMIN).

**Relevance to Relativist:** Relativist v1 has no security (SPEC-07 R44). For SPEC-10 (security/auth), HTCondor's layered model offers a useful architectural pattern:
- **Separation of authentication and authorization.** Even in a simple system, distinguishing "who are you" from "what can you do" is good practice.
- **IDTOKENS as a model.** HTCondor's IDTOKENS (JWTs signed by a pool signing key) are lightweight and easy to implement. For Relativist, a similar approach could work: the coordinator generates a session token at startup, shares it with workers, and workers present it on connection. This is simpler than full TLS/mTLS but provides basic identity verification.
- **Optional TLS.** HTCondor makes encryption negotiable per-connection. Relativist could make TLS a compile-time feature flag (`--features tls`), disabled by default for v1 benchmarks but available for production use.

**Adaptation for SPEC-10:**
1. **v1 minimum:** Pre-shared token (coordinator generates, distributed via CLI flag or env var). Workers present token on connect. No encryption.
2. **v1 recommended:** Optional TLS via rustls (feature flag). Coordinator presents a self-signed certificate. Workers verify via pinned certificate hash.
3. **v2+:** mTLS with per-worker certificates for mutual authentication.

**Verdict: ADAPT.** HTCondor's IDTOKENS model informs a lightweight token-based auth for SPEC-10. The layered authentication/authorization separation is a sound architectural pattern.

### L3. Self-Checkpointing Application Pattern -- ADAPT (for DISC-007 v2)

**HTCondor mechanism:** Applications are responsible for writing their own checkpoint files. HTCondor provides infrastructure: a designated exit code triggers checkpoint transfer, signals warn of impending eviction, and periodic checkpointing is configurable.

**Relevance to Relativist:** Relativist v1 does not need checkpointing (rounds are cheap to re-execute). However, the **self-checkpoint pattern** -- where the application defines what constitutes its critical state and writes it explicitly -- is relevant for v2+ fault tolerance design:
- The coordinator could periodically serialize the current Net state to disk (a "coordinator checkpoint").
- If the coordinator crashes and restarts, it resumes from the last checkpoint rather than restarting the entire computation.
- This is exactly the self-checkpoint model: the application (coordinator) knows its state structure and writes it.

**Key insight from HTCondor:** The `checkpoint_exit_code` pattern (application exits with a special code to trigger checkpoint management) is elegant but not directly applicable. More relevant is the **periodic checkpoint signal** pattern: an external timer triggers the application to write state. For Relativist, this could be a configurable `checkpoint_interval_rounds` parameter.

**Verdict: ADAPT for v2+.** The self-checkpoint pattern informs future coordinator state persistence. Not needed for v1.

### L4. Negotiation Cycle (Periodic Batch Matching) -- REJECT

**HTCondor mechanism:** The Negotiator runs a complete matching cycle every ~60 seconds, re-evaluating all jobs against all machines.

**Relevance to Relativist:** Relativist's "scheduling" (partitioning + assignment) happens at the start of every reduction round, which may be sub-second. There is no periodic cycle; the coordinator directly assigns work as part of the synchronous loop. The batch-matching approach is designed for a system with thousands of independent jobs and resources changing dynamically -- neither applies to Relativist.

**Verdict: REJECT.** The scheduling model does not apply.

### L5. Flocking (Inter-Pool Federation) -- REJECT

**HTCondor mechanism:** Schedds advertise to remote Collectors, allowing jobs to run on resources in other administrative domains.

**Relevance to Relativist:** Relativist operates a single pool of 8 machines under single administrative control. There is no concept of multiple pools or federation. All workers connect to a single coordinator.

**Verdict: REJECT.** Single-pool architecture; federation is out of scope.

### L6. DAGMan Workflow Dependencies -- REJECT

**HTCondor mechanism:** DAGMan manages job dependencies as a directed acyclic graph, with retry, pre/post scripts, and rescue DAGs.

**Relevance to Relativist:** Relativist does not have job dependencies in the HTCondor sense. The computation is a single iterative loop (partition -> reduce -> merge), not a DAG of independent jobs. The "dependency" is temporal and implicit: round N+1 depends on round N, enforced by the synchronous loop structure (SPEC-05). DAGMan solves a different problem.

**Verdict: REJECT.** The computation model is iterative, not DAG-structured.

### L7. Resource Owner Policy (Startd Policy Expressions) -- REJECT

**HTCondor mechanism:** Machine owners define ClassAd expressions that control when jobs can run, be suspended, or be evicted (e.g., "only when keyboard idle > 15 minutes").

**Relevance to Relativist:** Workers are dedicated machines for the experiment duration. There is no concept of resource sharing with other users or preemption by owners. Workers accept all work from the coordinator unconditionally.

**Verdict: REJECT.** The problem does not exist in Relativist's deployment model.

### L8. Persistent Job Queue (Transaction Log) -- ADAPT (minor)

**HTCondor mechanism:** The Schedd stores the job queue as a transaction log on disk -- an append-only log with periodic compaction/snapshots. This provides durability across Schedd restarts without the overhead of a full database.

**Relevance to Relativist:** Relativist v1 is purely in-memory (SPEC-02). There is no persistent job queue. However, the **transaction log pattern** (append-only log + periodic snapshots) is the standard approach for durable state in distributed systems and could inform v2+ coordinator state persistence (related to L3 above).

**Adaptation:** If coordinator state persistence is added (v2+), an append-only log of completed rounds (recording which round completed, the Net state hash, and timing) would be lightweight and provide crash recovery. This is not a job queue but an execution log.

**Verdict: ADAPT for v2+.** The append-only log pattern is generally useful for state persistence. Not needed for v1.

### L9. Slot-Based Resource Division (Startd Slots) -- REJECT

**HTCondor mechanism:** A single machine can be divided into multiple slots (static or partitionable), each advertised and matched independently.

**Relevance to Relativist:** Relativist assigns one partition per worker (SPEC-04). A worker is the unit of parallelism. If a machine has 8 cores, the question is whether to run 1 worker (using all cores for local parallelism within the reduction engine) or 8 workers (each getting a smaller partition). This is a deployment decision (SPEC-07), not a slot-management mechanism. The coordinator does not need to understand sub-machine resource topology.

**Verdict: REJECT.** Worker-per-machine granularity is sufficient for v1.

### L10. Separation of Concerns: Collector/Negotiator/Schedd/Startd -- ADAPT (minor)

**HTCondor mechanism:** Clear separation of roles into distinct daemons: information aggregation (Collector), matchmaking (Negotiator), queue management (Schedd), resource management (Startd), per-job monitoring (Shadow/Starter). Each daemon has a single responsibility.

**Relevance to Relativist:** Relativist v1 is a single binary (coordinator or worker mode, SPEC-07 R1). However, the **conceptual separation of concerns** is valuable for code organization (SPEC-13). The coordinator process handles: (a) connection management, (b) partitioning, (c) dispatching, (d) collection, (e) merging, (f) cycle orchestration. These should be clearly separated modules even within a single binary.

**Adaptation:** Already captured in PESQ-001 L2 (BOINC multi-daemon) and PESQ-004 L1 (Dask stimulus-response FSM). HTCondor reinforces the principle: even in a monolith, maintain clear role boundaries.

**Verdict: ADAPT (already captured).** Reinforces existing recommendation for clean module boundaries in SPEC-13.

---

## 6. Comparison Table (HTCondor vs Relativist)

| Dimension | HTCondor | Relativist | Notes |
|-----------|----------|------------|-------|
| **Year / Maturity** | 1988-present, production | 2026, TCC prototype | 38 years of production use vs prototype |
| **Language** | C++ (daemons) | Rust (single binary) | |
| **Computation model** | Batch HTC (independent jobs) | Iterative synchronous graph reduction | Fundamentally different |
| **Network topology** | Star + direct Schedd-Startd claims | Star (coordinator-centric) | Both centrally mediated |
| **Communication** | Custom TCP protocol (Cedar) | TCP + bincode (SPEC-06) | Both custom protocols |
| **Scheduling** | Declarative bilateral matchmaking (ClassAds) | Direct coordinator assignment (SPEC-04) | HTCondor far more complex |
| **Security** | Layered (IDTOKENS, SSL, SCITOKENS, etc.) | None in v1; token-based for SPEC-10 | HTCondor's token model informs SPEC-10 |
| **Fault tolerance** | Comprehensive (checkpoint, eviction, retry) | None in v1 | Different requirements |
| **Checkpointing** | Self-checkpoint (application-managed) | Not needed (rounds are cheap) | HTCondor's pattern informs v2+ |
| **State persistence** | Transaction log (Schedd) | In-memory only | Different durability needs |
| **Resource ownership** | Distributed (multiple owners, policies) | Centralized (single researcher) | Biggest architectural difference |
| **Job duration** | Minutes to days | Sub-second to seconds per round | Orders of magnitude different |
| **Inter-job dependency** | DAGMan (explicit DAG) | Implicit (synchronous loop) | Different paradigms |
| **Scale** | Hundreds of thousands of slots | 8 machines | Orders of magnitude different |
| **Federation** | Flocking (inter-pool) | Single pool | Not applicable |
| **Result verification** | None (trusted users, unlike BOINC) | Guaranteed by strong confluence (SPEC-01) | Different assurance models |
| **Deployment** | Multi-daemon, multi-machine | Single binary + Docker (SPEC-07) | HTCondor operationally complex |

---

## 7. Sources

### Academic Papers

- Thain, D., Tannenbaum, T., Livny, M. (2005). "Distributed Computing in Practice: The Condor Experience." *Concurrency and Computation: Practice and Experience*, 17(2-4), 323-356. [PDF](https://research.cs.wisc.edu/htcondor/doc/condor-practice.pdf)
- Litzkow, M., Livny, M., Mutka, M. (1988). "Condor: A Hunter of Idle Workstations." *Proceedings of the 8th International Conference on Distributed Computing Systems*, pp. 104-111.
- Thain, D., Tannenbaum, T., Livny, M. (2003). "Condor and the Grid." In *Grid Computing: Making the Global Infrastructure a Reality*, Wiley. [PDF](https://research.cs.wisc.edu/htcondor/doc/condorgrid.pdf)
- Epema, D.H.J., Livny, M., et al. (1996). "A Worldwide Flock of Condors: Load Sharing among Workstation Clusters." *Future Generation Computer Systems*, 12(1), 53-65. [ResearchGate](https://www.researchgate.net/publication/2812679_Condor_Flocking_Load_Sharing_Between_Pools_of_Workstations)
- CMS Collaboration (2024). "Adoption of a token-based authentication model for the CMS Submission Infrastructure." [arXiv:2405.14644](https://arxiv.org/html/2405.14644)

### HTCondor Official Documentation (v25.x)

- [Introduction to HTCondor Administration](https://htcondor.readthedocs.io/en/latest/admin-manual/introduction-admin-manual.html)
- [Central Manager Configuration](https://htcondor.readthedocs.io/en/latest/admin-manual/cm-configuration.html)
- [Security](https://htcondor.readthedocs.io/en/latest/admin-manual/security.html)
- [Matchmaking with ClassAds](https://htcondor.readthedocs.io/en/v8_8/users-manual/matchmaking-with-classads.html)
- [HTCondor's ClassAd Mechanism](https://htcondor.readthedocs.io/en/latest/classads/classad-mechanism.html)
- [Job ClassAd Attributes](https://htcondor.readthedocs.io/en/latest/classad-attributes/job-classad-attributes.html)
- [Choosing an HTCondor Universe](https://htcondor.readthedocs.io/en/latest/users-manual/choosing-an-htcondor-universe.html)
- [Self-Checkpointing Applications](https://htcondor.readthedocs.io/en/latest/users-manual/self-checkpointing-applications.html)
- [Connecting Pools with Flocking](https://htcondor.readthedocs.io/en/latest/grid-computing/connecting-pools-with-flocking.html)
- [DAGMan Introduction](https://htcondor.readthedocs.io/en/latest/automated-workflows/dagman-introduction.html)
- [Managing a Job](https://htcondor.readthedocs.io/en/latest/users-manual/managing-a-job.html)
- [High-Throughput Computing Requirements](https://htcondor.readthedocs.io/en/latest/overview/high-throughput-computing-requirements.html)
- [ClassAd Language Reference Manual v2.2](https://htcondor.org/classad/refman.V2.2/refman.pdf)

### Presentations

- [HTCondor System Architecture and Administration Introduction (CERN, 2024)](https://indico.cern.ch/event/1386170/contributions/6127903/attachments/2934905/5154630/HTCondor%20System%20Administration%20Introduction.pdf)
- [HTCondor Administration Basics (Condor Week Barcelona, 2016)](https://indico.cern.ch/event/467075/contributions/1143813/attachments/1236271/1815391/Admin_Basics_Condor_Week_Barcelona_2016.pdf)

### Other

- [HTCondor Wikipedia](https://en.wikipedia.org/wiki/HTCondor)
- [HTCondor Official Website](https://htcondor.org/)
- [HTCondor GitHub Repository](https://github.com/htcondor/htcondor)
- [HTCondor Checkpointing Overview](https://htcondor.org/checkpointing.html)
- [HTCSS Vulnerability Reports](https://htcondor.org/security/vulnerabilities/)
