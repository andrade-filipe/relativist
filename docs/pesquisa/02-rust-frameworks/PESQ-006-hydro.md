---
pesq_id: PESQ-006
title: "Hydro: Dataflow Framework for Distributed Systems in Rust"
category: Rust Distributed Frameworks
date_created: 2026-03-25
status: Complete
---

# PESQ-006: Hydro -- Dataflow Framework for Distributed Systems in Rust

**Category:** Rust Distributed Frameworks
**Status:** Complete
**Cross-references:**
- Specs: SPEC-13 (system architecture), SPEC-11 (observability), SPEC-06 (wire protocol), SPEC-08 (test strategy)
- References: REF-002 (Lafont 1997 -- strong confluence), REF-003 (Taelin 2024 -- HVM2)
- Discussions: DISC-003 v2 (confluence local to distributed), DISC-005 v2 (cross-boundary protocol), DISC-008 v2 (shared memory to distributed)

---

## 1. Subject Overview

Hydro is a high-level distributed programming framework for Rust, born from UC Berkeley's research on programmable distributed systems. The project is led by Joseph M. Hellerstein and Alvin Cheung, and is now co-led by a team at Berkeley and AWS. Hydro is the first production framework to implement **location-oriented programming**, where a single function can encapsulate logic spanning multiple machines, eliminating the traditional pattern of fragmenting distributed logic across per-service codebases.

**Intellectual lineage:** Hydro grows from two decades of Berkeley research on declarative networking and the **CALM Theorem** (Consistency As Logical Monotonicity), which proves that programs with consistent, coordination-free distributed implementations are exactly the programs expressible in monotonic logic (Hellerstein 2020). This theoretical foundation drives Hydro's emphasis on compile-time correctness: if the type system can verify monotonicity and location safety, many classes of distributed bugs are eliminated before runtime.

**Scale of the project:** The Hydro monorepo (`hydro-project/hydro` on GitHub) contains approximately 15 crates including `hydro_lang`, `dfir_rs`, `dfir_lang`, `hydro_deploy`, `hydro_std`, `hydro_test`, and `lattices`. The `dfir_rs` crate (formerly `hydroflow`) reports ~15,000 downloads/month on crates.io. The framework is backed by 15+ peer-reviewed papers at venues including POPL 2025, SIGMOD 2024, OOPSLA 2023, and CIDR 2021.

**Computation model:** Hydro implements a **streaming dataflow** model. Developers write programs as compositions of stream operators (map, filter, fold, join, cross_product, etc.) over typed, location-annotated streams. A global Hydro program is compiled ("hydrolyzed") into per-node single-threaded DFIR programs, which are then compiled to native Rust binaries. This is fundamentally a **continuous stream processing** paradigm, orthogonal to Relativist's **iterative synchronous graph reduction** model where the computation terminates when no active pairs remain.

**Key design constraints that shaped Hydro:**
1. **Correctness by construction:** Distributed bugs (out-of-order messages, duplicates, cross-node reference violations) should be caught at compile time, not runtime.
2. **Zero-overhead abstractions:** The high-level API compiles to bare-metal binaries via DFIR and Rust+LLVM, with performance matching handwritten systems.
3. **Single-program, multi-node:** A distributed protocol should be expressible as a single composable function, not scattered across per-service implementations.
4. **Academic rigor:** Every layer of the stack has formal semantics backed by peer-reviewed publications.
5. **Deployment flexibility:** Programs should run identically in simulation, local testing, and multi-cloud production.

**Primary references:**
- Laddad, S., Cheung, A., Hellerstein, J.M., Milano, M. (2025). "Flo: a Semantic Foundation for Progressive Stream Processing." *POPL 2025*. [arXiv:2411.08274](https://arxiv.org/abs/2411.08274)
- Laddad, S., Cheung, A., Hellerstein, J.M. (2024). "Suki: Choreographed Distributed Dataflow in Rust." *CP 2024*. [arXiv:2406.14733](https://arxiv.org/abs/2406.14733)
- Samuel, M., Cheung, A., Hellerstein, J.M. (2023). "Hydroflow: A Compiler Target for Fast, Correct Distributed Programs." *OOPSLA 2023 (SPLASH 2023)*.
- Cheung, A., Crooks, N., Hellerstein, J.M., Milano, M. (2021). "New Directions in Cloud Programming." *CIDR 2021*.

---

## 2. Architecture / Design

### 2.1 The Hydro Stack

Hydro is organized as a **layered compiler stack**, explicitly modeled after LLVM's multi-level IR approach. Each layer has a distinct responsibility and can be programmed independently:

```
+------------------------------------------------------------------+
|                        DEVELOPER                                  |
|                                                                   |
|  +----------------------------------------------------------+    |
|  |  HYDRO (hydro_lang)                                      |    |
|  |  Global, multi-process dataflow specification             |    |
|  |  Location-oriented programming                            |    |
|  |  Stream types with compile-time distributed safety        |    |
|  +----------------------------------------------------------+    |
|                          |                                        |
|                    Hydrolysis                                      |
|                    (compile phase)                                 |
|                          |                                        |
|  +----------------------------------------------------------+    |
|  |  DFIR (dfir_rs)                                           |    |
|  |  Local, single-process dataflow IR                        |    |
|  |  Single-threaded microbatch execution                     |    |
|  |  Rust code generation via proc macros                     |    |
|  +----------------------------------------------------------+    |
|                          |                                        |
|                    rustc + LLVM                                    |
|                          |                                        |
|  +----------------------------------------------------------+    |
|  |  NATIVE BINARIES                                          |    |
|  |  One per-node binary, monomorphized                       |    |
|  +----------------------------------------------------------+    |
|                          |                                        |
|  +----------------------------------------------------------+    |
|  |  HYDRO DEPLOY (hydro_deploy)                              |    |
|  |  Deployment to localhost, Docker, AWS, GCP, Azure         |    |
|  |  Automatic provisioning and port forwarding               |    |
|  +----------------------------------------------------------+    |
+------------------------------------------------------------------+
```

### 2.2 Hydro Language Layer (hydro_lang)

The top layer provides the developer-facing API. Key abstractions:

#### Location Types

Every piece of data in a Hydro program is tagged with a **location** specifying where it physically resides:

- **`Process<'a, Tag>`**: Represents exactly one machine instance. The `Tag` is a zero-sized marker type (e.g., `Leader`, `Worker`) that statically distinguishes different roles.
- **`Cluster<'a, Tag>`**: Represents a set of machines running the same code in SIMD-like fashion. Each member has a runtime `ClusterId`.
- **`ExternalProcess<'a, Tag>`**: Represents an external client that connects to the Hydro program.

The lifetime parameter `'a` ties all locations to the same `FlowBuilder`, enforcing that streams from different programs cannot be mixed.

#### Live Collections (Stream Types)

Data flows through typed **live collections** that update asynchronously over time:

- **`Stream<T, Loc>`**: An unbounded sequence of `T` values materialized at location `Loc`. The most common type.
- **`Singleton<T, Loc>`**: A single value that may be updated over time.
- **`Optional<T, Loc>`**: Zero or one value, similar to `Option<T>` but asynchronous.

These types carry additional **order annotations** (`Unbounded`, `Unordered`, `ExactlyOnce`, etc.) that make the Rust type checker enforce distributed correctness constraints:

```rust
// Type system prevents accessing data from mismatched locations:
fn process_on_leader<'a>(
    data: Stream<usize, Cluster<'a, Worker>>,  // data is on Workers
    leader: &Process<'a, Leader>,
) -> Singleton<usize, Process<'a, Leader>> {
    // Must explicitly send to the leader -- the type system enforces this
    data.send(leader, TCP.fail_stop().bincode())
        .map(q!(|v| v.1))
        .fold(q!(0), q!(|acc, v| *acc += v))
}
```

#### The `q!` Macro (Code Quoting)

All closures and values passed to stream operators must be wrapped in the `q!()` macro. This is Hydro's staged programming mechanism: the `q!` macro captures Rust code tokens (not values) for later emission into per-node DFIR programs. At the first stage (running on the developer's laptop), the Hydro program constructs the global dataflow graph. At the second stage, Hydrolysis slices this graph by location and emits the captured code into per-node Rust source files.

```rust
// Stage 1 (developer laptop): builds the graph
let transformed = input.filter(q!(|v| v > 2)).map(q!(|v| v * 2));

// Stage 2 (Hydrolysis): emits per-node code containing |v| v > 2 and |v| v * 2
```

This is the key mechanism enabling "single-program, multi-node" semantics: the developer writes one Rust function, and the compiler splits it into per-node executables.

### 2.3 Hydrolysis (Compiler Phase)

Hydrolysis is the compiler that translates a global Hydro specification into multiple single-threaded DFIR programs. Currently embedded in the Hydro codebase, it is planned to evolve into a standalone optimizing compiler "inspired by database query optimizers and e-graphs."

Hydrolysis performs:
1. **Location slicing:** Partitions the global dataflow graph at `send()` boundaries. Each location gets the subgraph of operators that execute on it.
2. **Network materialization:** Inserts serialization/deserialization and TCP send/receive operators at cut points.
3. **Optimization:** Applies rewrite rules (e.g., predicate pushdown, communication batching). The SIGMOD 2024 paper demonstrated 5x throughput improvement for 2PC and 3x for Paxos via rule-driven rewrites.

### 2.4 DFIR Layer (dfir_rs)

DFIR (Dataflow Intermediate Representation) is the low-level local runtime. It is a **single-threaded microbatch dataflow engine** that executes on each node.

**Key properties of the DFIR runtime:**

1. **Single-threaded execution:** Each DFIR program runs on a single thread. Parallelism comes from running multiple DFIR programs on different machines or processes, not from multi-threading within a single node.

2. **Microbatch processing:** Rather than processing one item at a time (like actors) or waiting for full batches (like MapReduce), DFIR processes **microbatches**: small groups of items that arrived since the last scheduling tick. This enables:
   - **Automatic vectorization:** Operations on microbatches can be vectorized by LLVM.
   - **Amortized scheduling overhead:** The per-item cost of scheduling is amortized across the microbatch.
   - **Low latency:** Microbatches can be as small as one item, so latency is not sacrificed for throughput.

3. **Rust monomorphization:** DFIR flow syntax is embedded in Rust via proc macros. The Rust compiler monomorphizes all generic operators, eliminating virtual dispatch. Combined with LLVM optimization, this produces binaries competitive with handwritten C++ (the Hydroflow OOPSLA 2023 paper showed throughput in the same order of magnitude as C++ Anna KVS).

4. **Semilattice-based state:** DFIR uses a semilattice formalism for stateful operators. This means that state updates are commutative, associative, and idempotent -- properties that enable correct program transformations (e.g., replication, sharding) without changing semantics.

5. **Rich profiling:** DFIR emits profiling information and can automatically generate Mermaid diagrams of the dataflow graph.

### 2.5 Hydro Deploy (hydro_deploy)

Hydro Deploy handles the mapping from logical locations to physical infrastructure:

```rust
// Local deployment (testing)
let mut deployment = Deployment::new();
flow.with_process(&process, deployment.Localhost())
    .deploy(&mut deployment);

// Cloud deployment (production)
flow.with_process(&process,
    deployment.GcpComputeEngineHost()
        .project("my-project")
        .machine_type("e2-micro")
        .image("debian-cloud/debian-11")
        .region("us-west1-a")
        .add()
).deploy(&mut deployment);
```

The deployment model works in three phases:
1. **Plan generation:** The Hydro program runs on the developer's laptop and produces a deployment plan specifying which binaries go where.
2. **Binary compilation:** DFIR programs are compiled to native Rust binaries for each target platform.
3. **Provisioning and launch:** Hydro Deploy provisions cloud resources (AWS, GCP, Azure), uploads binaries, and starts processes. It automatically handles port forwarding to the developer's machine.

### 2.6 Deterministic Simulation Testing

Hydro includes a **deterministic simulator** that runs the entire distributed system in a single thread with controlled scheduling:

```rust
#[test]
fn test_echo_capitalize() {
    let mut flow = FlowBuilder::new();
    let process = flow.process();
    let (in_port, requests) = process.sim_input();
    let responses = super::echo_capitalize(requests);
    let out_port = responses.sim_output();

    flow.sim().exhaustive(async || {
        in_port.send("hello".to_owned());
        in_port.send("world".to_owned());
        out_port.assert_yields_only(["HELLO", "WORLD"]).await;
    });
}
```

Key properties:
- **Deterministic:** Same seed produces same execution. Failures are always reproducible.
- **Exhaustive exploration:** The `.exhaustive()` method explores all possible distributed interleavings.
- **Standard Rust tests:** No special test harness required -- uses `#[test]` with `cargo test`.
- **Virtual time:** Simulated time progresses without real-world delays, enabling fast exploration of timeout-dependent logic.
- **Fuzzing integration:** Compatible with coverage-guided fuzzers for automated edge-case discovery.

### 2.7 Networking Model

Communication between locations is expressed through typed `send()` operators:

```rust
// Send with serialization and failure semantics specified in the type
data.send(leader, TCP.fail_stop().bincode())
```

The send operator is parameterized by:
- **Transport:** TCP (persistent connections with backpressure).
- **Failure mode:** `fail_stop()` (messages may be lost on crash), `exactly_once()` (retransmission with deduplication).
- **Serialization:** `bincode()`, `json()`, or custom codecs.

These parameters are reflected in the stream's type, so the Rust compiler enforces that downstream operators handle the declared failure semantics. For example, a stream marked `Unordered` cannot be passed to an operator that assumes ordered input without an explicit `sort()` or `assert_ordered()`.

---

## 3. Key Mechanisms

### 3.1 Location-Oriented Programming and Choreographic Compilation

**Problem:** Traditional distributed systems require developers to write separate programs for each role (client, server, coordinator, worker), manually implement serialization and networking between them, and reason about distributed correctness across multiple codebases.

**Solution:** Hydro's location-oriented programming (formalized as "Suki" in CP 2024) allows a single function to express computation across multiple machines. The Hydrolysis compiler automatically splits this into per-node programs.

**How it works:**
1. Developer writes a function parameterized by `Process` and `Cluster` types.
2. Stream operators chain together, with `send()` marking cross-machine boundaries.
3. Hydrolysis slices the dataflow graph at `send()` points.
4. Each slice becomes a separate DFIR program.
5. Network operators are automatically inserted at cut points.

**Impact:** A Paxos implementation in Hydro takes ~100 lines (single function) vs. ~1000+ lines in traditional per-role implementations. The Suki paper reported >50 kops/s for Paxos out of the box.

### 3.2 Compile-Time Distributed Safety via Stream Types

**Problem:** Distributed bugs (sending data to the wrong node, ignoring message reordering, accessing remote data without communication) are caught only at runtime in traditional frameworks.

**Solution:** Hydro's type system encodes location and ordering information in stream types. The Rust compiler rejects programs that violate distributed safety:

- **Location mismatch:** `Stream<T, Process<A>>` cannot be combined with `Stream<T, Process<B>>` without an intervening `send()`.
- **Order violation:** A stream annotated `Unordered` cannot be passed to an operator that requires ordered input.
- **Duplicate hazard:** Streams that may contain duplicates (e.g., from retransmission) are typed differently from deduplicated streams.

These constraints have **zero runtime overhead** -- they exist purely in the type system and are erased at compilation.

### 3.3 Flo Formal Semantics (POPL 2025)

**Problem:** Stream processing systems make implicit assumptions about progress and eagerness that are rarely formalized, leading to subtle correctness bugs.

**Solution:** Flo defines two formal semantic properties for streaming systems:

1. **Streaming progress:** Outputs advance monotonically with respect to inputs. A system satisfying streaming progress will not "get stuck" or delay outputs unnecessarily.
2. **Eager execution:** Data is processed as soon as possible, not deferred or batched unnecessarily.

Together, these properties guarantee that streaming outputs are **deterministic and kept fresh** with respect to streaming inputs.

Flo introduces a lightweight type system distinguishing **bounded streams** (which terminate) from **unbounded streams** (which are infinite). Operators on bounded streams may block on termination (e.g., sort), while operators on unbounded streams must process data incrementally.

The paper demonstrates that Flink, LVars, and DBSP can all be modeled as instances of Flo's parameterized framework, establishing Flo as a unifying foundation.

### 3.4 CALM-Based Coordination Avoidance

**Problem:** Distributed coordination (locks, barriers, consensus) is expensive. Not all programs need it, but traditional frameworks cannot determine when it is safe to skip.

**Solution:** The CALM Theorem (Hellerstein 2020) proves that **monotonic** computations -- those where adding new input never invalidates previous output -- can be executed without coordination and still produce consistent results.

Hydro leverages this by:
1. Tracking monotonicity through the type system (the `lattices` crate provides semilattice-based data types).
2. Allowing the compiler to automatically identify coordination-free subgraphs.
3. Inserting coordination (barriers, consensus) only where non-monotonic operations occur.

This is conceptually related to Relativist's reliance on **strong confluence** (SPEC-01): both systems exploit mathematical properties of the computation to avoid coordination overhead. However, they exploit different properties: CALM exploits monotonicity of lattice operations, while Relativist exploits strong confluence of Interaction Combinator reductions.

### 3.5 Query Optimization Rewrites (SIGMOD 2024)

**Problem:** Distributed protocols like 2PC and Paxos have well-known but manually applied scalability optimizations (compartmentalization, sharding, batching).

**Solution:** The Hydro team showed that these optimizations can be expressed as **dataflow rewrite rules**, analogous to query optimization in databases. Applied automatically by the compiler, these rewrites improved:
- 2PC throughput by **5x**
- Paxos throughput by **3x**

This is possible because the DFIR intermediate representation is a dataflow graph, amenable to the same algebraic transformations used in database query planners.

---

## 4. Comparison with Relativist's Context

### 4.1 Fundamental Model Difference

| Dimension | Hydro | Relativist |
|-----------|-------|------------|
| **Computation model** | Continuous streaming dataflow | Iterative synchronous graph reduction |
| **Program lifetime** | Long-running (services) | Finite (reduce until no active pairs) |
| **Data model** | Typed streams (unbounded sequences) | Interaction net (agent graph with ports) |
| **Distribution model** | Compile-time: global program -> per-node DFIR | Runtime: coordinator partitions net -> workers reduce |
| **Parallelism source** | Multiple DFIR processes on different nodes | Multiple workers reducing disjoint graph partitions |
| **Per-node execution** | Single-threaded DFIR dataflow engine | Single-threaded reduction engine (SPEC-03) |
| **Correctness basis** | CALM theorem (monotonicity) + stream types | Strong confluence (Lafont 1997) |
| **Network role** | First-class: typed send() operators | Infrastructure: coordinator pushes/collects partitions |
| **Formal foundation** | Flo semantics (POPL 2025) | IC reduction rules (Lafont 1997) |
| **Academic backing** | 15+ papers (POPL, SIGMOD, OOPSLA) | 1 TCC (this work) |

### 4.2 Distribution Strategy: Compile-Time vs Runtime

Hydro's most distinctive property is **compile-time distribution**. The developer writes one program; the Hydrolysis compiler splits it into per-node binaries before any code runs. The distribution topology is fixed at compile time: which locations exist, how they communicate, and what serialization format they use are all determined before deployment.

Relativist uses **runtime distribution**. The coordinator receives a complete interaction net, partitions it at runtime based on the graph's current structure (SPEC-04), sends partitions to workers, collects results, merges them (SPEC-05), and repeats. The distribution topology is fixed (star, coordinator-centric), but the **work assignment** (which agents go to which worker) changes every round.

**Why this difference matters:**

| Property | Hydro (compile-time) | Relativist (runtime) |
|----------|---------------------|---------------------|
| Optimization opportunity | Global program analysis, cross-location rewrites | Limited to per-round partitioning heuristics |
| Flexibility | Topology fixed at compile time | Work assignment adapts to graph structure |
| Overhead | Zero runtime distribution overhead (compiled away) | Per-round partition + serialize + deserialize cost |
| Applicability | Services with known communication patterns | Graph reduction with unpredictable structure changes |

Relativist's runtime distribution is **necessary** because IC reduction changes the graph structure unpredictably. After one round of reductions, the net may have a completely different topology, requiring re-partitioning. Hydro's compile-time approach assumes the communication pattern is known statically, which is true for services but not for dynamic graph reduction.

### 4.3 Single-Threaded Per-Node: Shared Design Decision

Both Hydro and Relativist use **single-threaded execution on each node**:
- Hydro: Each DFIR program is single-threaded, using microbatch scheduling.
- Relativist: Each worker runs a single-threaded reduction engine (SPEC-03).

Both avoid the complexity of shared-state concurrency within a single node, relying instead on distribution across nodes for parallelism. This is a significant architectural alignment.

However, the single-threaded models serve different purposes:
- Hydro's single-threaded DFIR enables **deterministic scheduling** within a node, which is essential for the deterministic simulation testing framework.
- Relativist's single-threaded worker avoids **data races on the agent arena** during reduction, where multiple rules might try to rewire the same ports simultaneously.

### 4.4 Type-Level Correctness vs Mathematical Correctness

Hydro ensures distributed correctness through the **Rust type system**: location types prevent cross-node data access, stream type annotations prevent ordering violations, and the CALM theorem identifies coordination-free subgraphs.

Relativist ensures distributed correctness through **mathematical properties of the computation model**: strong confluence (SPEC-01) guarantees that the reduction result is deterministic regardless of reduction order or distribution. The type system plays no role in distributed correctness; the property is inherent to Interaction Combinators.

| Aspect | Hydro | Relativist |
|--------|-------|------------|
| **What ensures correctness** | Type system + CALM theorem | Strong confluence of IC |
| **When correctness is checked** | Compile time | By construction (mathematical proof) |
| **What the developer must do** | Use correct types and annotations | Implement the 6 rules correctly (SPEC-03) |
| **What can go wrong** | Logic errors within correctly-typed code | Implementation bugs in rules or merge protocol |
| **Testing strategy** | Deterministic simulation + fuzzing | Round-trip property: `reduce_all(net) == extract_result(run_grid(net, n))` (SPEC-08) |

### 4.5 Streaming vs Graph Reduction

Hydro processes **streams of data items** flowing through operators. The data items are independent values (tuples, records, messages). The topology of the dataflow graph is static.

Relativist processes a **single interaction net** -- a graph where agents are connected through ports. The graph structure changes with every reduction step. There are no independent data items flowing through a pipeline; instead, the entire graph is the state, and reductions mutate it in place.

This is the deepest incompatibility between the two models. Hydro's streaming model assumes data independence between items in a stream, enabling operators like `filter`, `map`, and `fold` to process items without knowledge of each other. Relativist's graph reduction model has total data dependence: rewiring one agent's ports during a reduction may create or destroy active pairs involving other agents.

### 4.6 Deployment Comparison

| Aspect | Hydro | Relativist |
|--------|-------|------------|
| **Deployment tool** | Hydro Deploy (automatic provisioning) | Manual deploy or Docker Compose (SPEC-07) |
| **Target platforms** | Localhost, Docker, AWS, GCP, Azure | 8 physical machines + Docker (SPEC-07, SPEC-09) |
| **Binary generation** | Per-node binaries from Hydrolysis | Single binary, all nodes identical (SPEC-07 R1) |
| **Configuration** | Rust code (deployment script) | CLI arguments (SPEC-07 R10) |
| **Port management** | Automatic with forwarding | Manual or Docker-mapped (SPEC-07) |

Hydro Deploy's automated provisioning is sophisticated but targets a different use case (long-running cloud services). Relativist's simpler deployment model is appropriate for a TCC experiment with 8 known machines.

---

## 5. Lessons for Relativist (ADOPT / ADAPT / REJECT)

### L1. Location-Oriented Programming -- REJECT

**Hydro mechanism:** Write a single function spanning multiple machines. The compiler splits it into per-node binaries. Location types prevent cross-node data access without explicit `send()`.

**Relevance to Relativist:** None. Relativist's computation is not a static dataflow graph. The coordinator and workers run fundamentally different code: the coordinator partitions, dispatches, and merges; workers reduce. These roles cannot be expressed as a single dataflow function because the communication pattern (which partition goes where) is determined at runtime by the graph structure.

Furthermore, Relativist's communication is simple (coordinator sends partitions, workers return results) and does not benefit from the type-level safety that Hydro provides for complex multi-party protocols like Paxos.

**Verdict: REJECT.** The computation model is fundamentally incompatible. Relativist's runtime-determined distribution cannot be expressed as a compile-time-fixed dataflow graph.

### L2. Stream Types for Distributed Safety -- REJECT

**Hydro mechanism:** Stream type annotations (location, ordering, deduplication) catch distributed bugs at compile time. Zero runtime overhead.

**Relevance to Relativist:** Relativist does not have streams. It has a single interaction net that is partitioned, distributed, reduced, and merged in a synchronous loop. The communication pattern is fixed (coordinator-to-worker and back), so there is no risk of accidentally sending data to the wrong location or ignoring message ordering.

The distributed correctness property that Relativist needs -- deterministic results regardless of distribution -- is guaranteed by strong confluence (SPEC-01), not by type-level constraints.

**Verdict: REJECT.** Stream types solve problems that do not exist in Relativist's architecture.

### L3. DFIR Single-Threaded Microbatch Architecture -- ADAPT

**Hydro mechanism:** Each DFIR program is a single-threaded microbatch dataflow engine. Microbatches amortize scheduling overhead while maintaining low latency. Rust monomorphization eliminates virtual dispatch.

**Relevance to Relativist:** Relativist workers are already single-threaded (SPEC-03), aligning with DFIR's design. However, Relativist workers do not process microbatches of independent items; they iterate over a redex queue and apply reduction rules to the agent graph.

**Adaptation:** The microbatch concept could inform how the coordinator processes returning partition results. Instead of waiting for all workers to return before processing any result (full-barrier BSP), the coordinator could process results in **microbatches** as they arrive: deserialize and rebuild the free port index for each incoming result while other workers are still reducing. This is already noted as a minor optimization in PESQ-001 L9.

More directly, the DFIR design validates Relativist's single-threaded worker choice: Hydro's research shows that single-threaded per-node execution, combined with distribution across nodes, achieves competitive performance even against multi-threaded designs. This is reassuring for Relativist's architecture.

**Verdict: ADAPT (minor).** Validates single-threaded worker design. Consider incremental processing of returning partition results in the coordinator.

### L4. Deterministic Simulation Testing -- ADAPT

**Hydro mechanism:** A deterministic simulator runs the entire distributed system in a single thread with controlled scheduling. Same seed produces same execution. Supports exhaustive interleaving exploration and fuzzing integration. Uses virtual time to avoid real-world delays.

**Relevance to Relativist:** This is directly relevant to SPEC-08 (test strategy). Relativist's grid loop involves multiple asynchronous workers communicating with a coordinator. Testing this is difficult because network timing introduces non-determinism.

**Adaptation:** Relativist should adopt the principle of deterministic testing but with a different implementation strategy, because Hydro's simulator is tightly integrated with its stream-based programming model and cannot be used directly for graph-reduction programs.

Options for Relativist:
1. **Turmoil or MadSim:** Use an existing Rust deterministic simulation framework (PESQ-021 will analyze these). These frameworks intercept tokio's async runtime to provide deterministic scheduling and simulated networking.
2. **In-process grid:** Run the coordinator and workers in the same process on separate tokio tasks, with a simulated network layer that introduces controllable delays and reordering. This is simpler than a full simulation framework but provides deterministic testing.
3. **Property-based testing with seeds:** Use proptest or quickcheck to generate random nets, partition them with different worker counts, reduce locally and distributed, and assert the fundamental property (`reduce_all(net) == extract_result(run_grid(net, n))`). The seed makes failures reproducible.

**Verdict: ADAPT for SPEC-08.** The principle of deterministic simulation testing is highly valuable. Implementation strategy should be analyzed in PESQ-020 and PESQ-021, and applied in SPEC-08.

### L5. Automated Deployment with Hydro Deploy -- REJECT

**Hydro mechanism:** Hydro Deploy automatically provisions cloud resources, compiles per-node binaries, uploads them, and starts the distributed system.

**Relevance to Relativist:** Relativist is a TCC prototype targeting 8 known physical machines (SPEC-07, SPEC-09). The deployment model is simple: one binary, compiled once, copied to all machines. Docker Compose handles local testing (SPEC-07 R37). Automated cloud provisioning is unnecessary overhead.

**Verdict: REJECT.** Over-engineered for Relativist's scope. Docker Compose and manual deployment (SPEC-07) are sufficient.

### L6. CALM Theorem for Coordination Avoidance -- ADAPT (Conceptual)

**Hydro mechanism:** Use the CALM theorem to identify monotonic (coordination-free) subgraphs. Only insert coordination where non-monotonic operations occur.

**Relevance to Relativist:** The CALM theorem itself does not directly apply to Relativist because IC reduction is not expressed as a monotonic lattice computation. However, the **underlying insight** is deeply relevant: both CALM and strong confluence identify classes of computations where the result is deterministic regardless of execution order, and therefore coordination can be minimized.

Relativist already exploits this insight: strong confluence means workers can reduce their local partitions without coordinating with each other (SPEC-03, SPEC-05). The coordinator synchronizes only between rounds (BSP barrier), not within rounds.

**Adaptation:** The TCC paper's related work section (Section 2: Fundamentacao Teorica) should discuss the conceptual parallel between CALM and strong confluence as independent but analogous approaches to determinism in distributed systems. Both provide formal guarantees that enable coordination avoidance, but for different computational models (dataflow vs graph rewriting).

**Verdict: ADAPT (conceptual, for the TCC paper).** The CALM/strong-confluence parallel is a valuable intellectual contribution. Does not affect Relativist's implementation.

### L7. Query Optimization Rewrites for Distributed Protocols -- REJECT

**Hydro mechanism:** Express distributed protocol optimizations as dataflow rewrite rules (e.g., predicate pushdown, sharding, compartmentalization). Apply automatically via the compiler.

**Relevance to Relativist:** Relativist's protocol is simple: partition, distribute, reduce, collect, merge. There is no complex multi-party consensus protocol to optimize. The main performance bottleneck is the serialize/deserialize cost per round (SPEC-06), not protocol inefficiency.

**Verdict: REJECT.** Relativist's protocol is too simple to benefit from automated rewrite optimization.

### L8. Staged Programming with `q!` Macro -- REJECT

**Hydro mechanism:** Use a `q!()` macro to capture code tokens at the first stage (graph construction) for emission into per-node programs at the second stage (compilation).

**Relevance to Relativist:** Relativist does not have a two-stage compilation model. The same binary runs on all nodes; the difference between coordinator and worker is a runtime decision based on CLI arguments (SPEC-07 R10). There is no need for code capture or per-node code generation.

**Verdict: REJECT.** Relativist's single-binary model does not require staged programming.

### L9. Semilattice-Based State Management -- REJECT

**Hydro mechanism:** Use semilattice-based data types (commutative, associative, idempotent merge) for stateful operators. This enables safe replication and sharding.

**Relevance to Relativist:** IC reduction is not a lattice operation. The reduction rules (SPEC-03) involve graph rewiring -- creating and destroying agents and reconnecting ports. These operations are neither commutative in general (two different active pairs may conflict on shared ports) nor idempotent (reducing the same active pair twice would corrupt the graph). Correctness comes from strong confluence (the *result* is the same regardless of *order*, even though individual steps are not commutative with respect to graph state).

**Verdict: REJECT.** The semilattice model does not fit IC graph reduction.

### L10. Composable Distributed Modules (hydro_std) -- ADAPT (Conceptual)

**Hydro mechanism:** Common distributed patterns (quorum counting, 2PC, leader election) are packaged as reusable single-function modules in `hydro_std`.

**Relevance to Relativist:** Relativist's core protocol (partition-distribute-reduce-merge) is specific enough that generic distributed primitives from `hydro_std` are not directly useful. However, the **principle** of composable, well-tested protocol building blocks is sound.

**Adaptation:** Relativist's code should be organized so that the partition, reduce, merge, and wire protocol components are cleanly separated modules (SPEC-13 candidate). This enables testing each component independently (SPEC-08) and replacing individual components in future versions without affecting others.

**Verdict: ADAPT (architectural principle).** Inform SPEC-13 module boundaries.

### L11. Rust-Native Design with Zero-Overhead Abstractions -- ADOPT (Validation)

**Hydro mechanism:** The entire stack (Hydro, DFIR, Hydro Deploy) is written in Rust. Hydro leverages Rust's type system, ownership model, proc macros, and LLVM codegen to deliver zero-overhead abstractions.

**Relevance to Relativist:** This validates Relativist's choice of Rust as the implementation language (SPEC-07). Hydro demonstrates that a research-grade distributed system can be built entirely in Rust with competitive performance. Specific Rust patterns used by Hydro that Relativist also uses or should use:
- **serde + bincode** for efficient serialization (Hydro supports bincode as a serialization format; Relativist uses it as the primary format in SPEC-06).
- **tokio** for async networking (Hydro uses tokio under DFIR; Relativist uses it for the coordinator/worker loop in SPEC-06).
- **proc macros** for code generation (Hydro uses them extensively for DFIR flow syntax; Relativist does not need them but could use derive macros for serde).

**Verdict: ADOPT (validation).** Hydro's successful use of the same Rust ecosystem (serde, bincode, tokio) validates Relativist's technology choices.

### L12. Rich Profiling and Diagram Generation -- ADAPT

**Hydro mechanism:** DFIR emits rich profiling information and can automatically generate Mermaid diagrams of the dataflow graph.

**Relevance to Relativist:** Automatic visualization of the interaction net at various stages (pre-partition, post-partition, post-reduce, post-merge) would be valuable for debugging and for the TCC paper's figures.

**Adaptation:** SPEC-11 (observability) should consider emitting Graphviz (DOT) or Mermaid diagrams of the interaction net at configurable stages. This is a low-priority "nice to have" but could significantly aid debugging of the merge protocol (SPEC-05) and partition algorithm (SPEC-04).

**Verdict: ADAPT for SPEC-11 (low priority).** Graph visualization for debugging and paper figures.

---

## 6. Comparison Table (Hydro vs Relativist)

| Dimension | Hydro | Relativist | Notes |
|-----------|-------|------------|-------|
| **Year / Maturity** | 2021-present, active research + production preview | 2026, TCC prototype | Different maturity levels |
| **Language** | Rust (multi-crate monorepo, ~15 crates) | Rust (single binary) | Both Rust-native |
| **Computation model** | Continuous streaming dataflow | Iterative synchronous graph reduction | Fundamentally different |
| **Distribution model** | Compile-time (Hydrolysis splits global program) | Runtime (coordinator partitions net per round) | Key architectural divergence |
| **Per-node execution** | Single-threaded DFIR (microbatch) | Single-threaded reduction engine | Same design choice |
| **Correctness guarantee** | CALM theorem + type system (compile-time) | Strong confluence (mathematical, by construction) | Both avoid unnecessary coordination |
| **Formal foundation** | Flo (POPL 2025) + CALM (CACM 2020) | Lafont (1997) | Both have strong theoretical backing |
| **Network abstraction** | First-class: typed send() with failure semantics | Infrastructure: TCP frames with bincode (SPEC-06) | Hydro: network is part of the program |
| **Serialization** | Configurable (bincode, json, custom) | bincode only (SPEC-06 R4) | Both support bincode |
| **Async runtime** | tokio (under DFIR) | tokio (SPEC-06, SPEC-07) | Same runtime |
| **Deployment** | Hydro Deploy (auto-provisioning, multi-cloud) | Docker Compose + manual (SPEC-07) | Hydro: production-grade |
| **Testing** | Deterministic simulation + fuzzing | Round-trip property testing (SPEC-08) | Hydro: more sophisticated |
| **State model** | Semilattice-based (CRDT-inspired) | Agent arena with port graph (SPEC-02) | Incompatible models |
| **Communication pattern** | Arbitrary (typed by location graph) | Star (coordinator <-> workers only) | Relativist: simpler |
| **Communication frequency** | Continuous (streaming) | Per-round (BSP-like barriers) | Different cadences |
| **Optimization** | Automatic dataflow rewrites (SIGMOD 2024) | None (manual partitioning heuristics) | Hydro: compiler-driven |
| **Scale target** | Cloud-scale services | 8 physical machines (SPEC-09) | Orders of magnitude apart |
| **Academic papers** | 15+ (POPL, SIGMOD, OOPSLA, VLDB) | 1 TCC | Different research scale |
| **Code organization** | Multi-crate workspace | Single crate (SPEC-13 candidate) | Hydro: complex, Relativist: simple |
| **Open source** | Apache 2.0 | TBD (SPEC-13 candidate) | Both open |

---

## 7. Sources

### Academic Papers (Hydro Project)

- Laddad, S., Cheung, A., Hellerstein, J.M., Milano, M. (2025). "Flo: a Semantic Foundation for Progressive Stream Processing." *Proceedings of the ACM on Programming Languages*, 9(POPL). [arXiv:2411.08274](https://arxiv.org/abs/2411.08274). [ACM DL](https://dl.acm.org/doi/10.1145/3704845)
- Laddad, S. (2025). "Programming Models for Correct and Modular Distributed Systems." *PhD Dissertation, UC Berkeley*. [EECS-2025-85](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2025/EECS-2025-85.html)
- Power, C., Koutris, P., Hellerstein, J.M. (2025). "The Free Termination Property of Queries Over Time." *ICDT 2025*.
- Laddad, S., Cheung, A., Hellerstein, J.M. (2024). "Suki: Choreographed Distributed Dataflow in Rust." *CP 2024*. [arXiv:2406.14733](https://arxiv.org/abs/2406.14733)
- Chu, D., Panchapakesan, R., Laddad, S., Katahanas, L., Liu, C., Shivakumar, K., Crooks, N., Hellerstein, J.M., Howard, H. (2024). "Optimizing Distributed Protocols with Query Rewrites." *SIGMOD 2024*.
- Chu, D., Liu, C., Crooks, N., Hellerstein, J.M., Howard, H. (2024). "Bigger, not Badder: Safely Scaling BFT Protocols." *PaPoC 2024*.
- Power, C., Achalla, S., Cottone, R., Macasaet, N., Hellerstein, J.M. (2024). "Wrapping Rings in Lattices: An Algebraic Symbiosis of Incremental View Maintenance and Eventual Consistency." *PaPoC 2024*.
- Samuel, M., Cheung, A., Hellerstein, J.M. (2023). "Hydroflow: A Compiler Target for Fast, Correct Distributed Programs." *OOPSLA 2023 (SPLASH 2023)*. [SPLASH proceedings](https://2023.splashcon.org/details/splash-2023-oopsla/112/Hydroflow-A-Compiler-Target-for-Fast-Correct-Distributed-Programs)
- Laddad, S., Power, C., Milano, M., Cheung, A., Crooks, N., Hellerstein, J.M. (2023). "Keep CALM and CRDT On." *VLDB 2023*.
- Hellerstein, J.M., Laddad, S., Milano, M., Power, C., Samuel, M. (2023). "Initial Steps Toward a Compiler for Distributed Programs." *ApPLIED 2023*.
- Laddad, S., Power, C., Milano, M., Cheung, A., Hellerstein, J.M. (2022). "Katara: Synthesizing CRDTs with Verified Lifting." *OOPSLA 2022*.
- Samuel, M., Cheung, A., Hellerstein, J.M. (2021). "Hydroflow: A Model and Runtime for Distributed Systems Programming." *Technical Report, UC Berkeley*.
- Cheung, A., Crooks, N., Hellerstein, J.M., Milano, M. (2021). "New Directions in Cloud Programming." *CIDR 2021*.
- Hellerstein, J.M. (2020). "Keeping CALM: When Distributed Consistency is Easy." *Communications of the ACM*, 63(9). [arXiv:1901.01930](https://arxiv.org/abs/1901.01930)

### Hydro Official Documentation

- [Hydro Main Website](https://hydro.run/)
- [Hydro Research Publications](https://hydro.run/research/)
- [Hydro Introduction (hydro_lang)](https://hydro.run/docs/hydro/)
- [Hydro Quickstart](https://hydro.run/docs/hydro/learn/quickstart/)
- [Hydro API Reference](https://hydro.run/docs/hydro/reference/)
- [DFIR Introduction](https://hydro.run/docs/dfir/)
- [The Hydro Ecosystem](https://hydro.run/docs/dfir/ecosystem/)
- [hydro_lang Rustdoc](https://hydro.run/rustdoc/hydro_lang/)

### Hydro Source Code

- [Hydro GitHub Repository](https://github.com/hydro-project/hydro)
- [Hydro Archive (older version)](https://github.com/hydro-project/hydro-archive)

### Crates.io

- [hydro_lang on crates.io](https://crates.io/crates/hydro_lang)
- [dfir_rs on lib.rs](https://lib.rs/crates/dfir_rs)
- [hydroflow on crates.io (legacy)](https://crates.io/crates/hydroflow)

### Third-Party Analysis

- [The Guide to Distributed Programming Frameworks for Rust: Hydro & Beyond (2025)](https://www.blog.brightcoding.dev/2025/12/23/the-guide-to-distributed-programming-frameworks-for-rust-hydro-beyond-2025/)
- [Hacker News Discussion: Hydro Distributed Programming Framework](https://news.ycombinator.com/item?id=42885087)
