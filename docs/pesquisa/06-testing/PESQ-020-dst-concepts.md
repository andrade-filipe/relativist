---
pesq_id: PESQ-020
title: "Deterministic Simulation Testing (DST) Concepts"
category: Testing Distributed Systems
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-08, SPEC-13]
  pesqs: [PESQ-006, PESQ-021, PESQ-022]
  discs: [DISC-007]
---

# PESQ-020: Deterministic Simulation Testing (DST) Concepts

**Category:** Testing Distributed Systems
**Status:** Complete

---

## 1. Subject Overview

Deterministic Simulation Testing (DST) is a testing methodology where the entire distributed system runs in a single process with simulated time, network, and I/O. By controlling all sources of non-determinism, failures become reproducible and the system can be subjected to chaos injection.

**Origin:** FoundationDB (Apple) pioneered DST, attributing their system's reliability to "millions of hours of simulated testing." The technique has since been adopted by TigerBeetle, RisingWave, and others.

### 1.1 Core Principles

1. **Deterministic execution:** Same seed → same behavior. All randomness, time, and scheduling controlled.
2. **Simulated environment:** Network, disk, clock are fake. No real I/O.
3. **Chaos injection:** Drop messages, reorder, partition network, crash nodes — all controlled.
4. **Reproducibility:** Failed test can be replayed with the same seed.
5. **Time compression:** Simulated time can advance instantly (no real waiting).

### 1.2 What DST Replaces

| Traditional Testing | DST Equivalent |
|--------------------|---------------|
| Unit tests | Still needed (pure logic) |
| Integration tests (real network) | Simulated network tests |
| Chaos engineering (Chaos Monkey) | Controlled fault injection |
| Soak tests (run for hours) | Compressed simulation (seconds) |
| Manual failure scenarios | Automated, exhaustive |

---

## 2. How DST Works

### 2.1 Architecture

```
┌─────────────────────────────────────────────┐
│  Single OS Process                          │
│                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │  Node 1  │  │  Node 2  │  │  Node 3  │  │
│  │(Coord.)  │  │(Worker)  │  │(Worker)  │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  │
│       │              │              │        │
│  ┌────┴──────────────┴──────────────┴────┐  │
│  │     Simulated Network Layer           │  │
│  │  (message queue, latency, drops)      │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │     Simulated Clock                   │  │
│  │  (instant advance, no real waiting)   │  │
│  └───────────────────────────────────────┘  │
│                                             │
│  ┌───────────────────────────────────────┐  │
│  │     Deterministic RNG (seeded)        │  │
│  └───────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

### 2.2 Fault Injection Capabilities

| Fault Type | Description | Relativist Relevance |
|-----------|-------------|---------------------|
| Message drop | Silently discard messages | Worker partition return lost |
| Message delay | Add arbitrary latency | Heartbeat timeout triggers |
| Message reorder | Deliver out of order | Protocol robustness |
| Network partition | Isolate nodes | Worker unreachable during round |
| Node crash | Kill and optionally restart | Worker crash mid-reduction |
| Clock skew | Desynchronize clocks | Timer-based decisions |
| Slow node | Throttle processing speed | Straggler detection |

---

## 3. Relevance to Relativist

### 3.1 Why DST is Valuable

Relativist's correctness depends on:
- P1 (confluence): Deterministic regardless of reduction order
- P3 (border completeness): All border redexes resolved at merge
- P5 (termination): Grid cycle eventually terminates

DST can verify these properties under fault conditions:
- Does the system produce the same result when workers are slow/crash/disconnect?
- Does the coordinator correctly handle partial round completion?
- Does the merge produce correct results when partitions arrive out of order?

### 3.2 Why Full DST is Too Heavy for v1

Full DST requires:
- Abstracting ALL I/O behind traits (network, time, random)
- Building or integrating a simulation runtime
- Significant development effort (~30-50% of codebase for simulation layer)

For a research project, this is disproportionate effort. However, **partial DST** is achievable:

### 3.3 Partial DST Strategy (Recommended for v1)

Instead of full simulation, use an **in-memory grid mode**:

1. Coordinator and workers run in the same process
2. Communication via `tokio::sync::mpsc` channels (not TCP)
3. Time is real (not simulated) but tests are fast (no network latency)
4. No chaos injection (save for v2)

This gives:
- Fast integration tests (milliseconds, not seconds)
- No port allocation / network setup
- Deterministic execution (single process, controlled scheduling)
- Ability to test the full grid cycle without Docker

```rust
// In-memory grid for testing
async fn test_grid_reduces_correctly() {
    let net = generate_test_net(1000);
    let config = GridConfig { workers: 4, rounds_max: 100 };
    let result = run_in_memory_grid(net, config).await;
    assert_eq!(result.border_redexes, 0);
    assert!(result.is_normal_form());
}
```

---

## 4. Lessons for Relativist

### L1: In-Memory Grid Mode for Testing [ADOPT]
Build an in-memory grid mode where coordinator and workers communicate via channels. This is the most valuable testing investment for v1.
→ Informs: SPEC-08, SPEC-13

### L2: Trait-Abstract Network I/O [ADOPT]
Abstract the network layer behind a trait: `trait Transport { async fn send(...); async fn recv(...); }`. Implement `TcpTransport` for production and `ChannelTransport` for testing. This is the foundation for future DST.
→ Informs: SPEC-06, SPEC-13

### L3: Full DST as v2 Goal [ADAPT]
Full deterministic simulation (Turmoil/MadSim integration) is a v2 goal. The trait abstraction from L2 makes this migration straightforward.
→ Informs: SPEC-08 (roadmap)

### L4: Seeded RNG for Reproducibility [ADOPT]
Any randomness in partition assignment or test data generation should use a seeded RNG. Log the seed; replay with same seed for reproducibility.
→ Informs: SPEC-08

---

## 5. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| DST for Async Rust (s2.dev) | https://s2.dev/blog/dst | 2026-03-26 |
| DST in Rust: Theater of State Machines | https://www.polarsignals.com/blog/posts/2025/07/08/dst-rust | 2026-03-26 |
| RisingWave DST blog (Part 1) | https://www.risingwave.com/blog/deterministic-simulation-a-new-era-of-distributed-system-testing/ | 2026-03-26 |
| RisingWave DST blog (Part 2) | https://risingwave.com/blog/applying-deterministic-simulation-the-risingwave-story-part-2-of-2/ | 2026-03-26 |
| Awesome DST resources | https://github.com/ivanyu/awesome-deterministic-simulation-testing | 2026-03-26 |
| FoundationDB testing | https://apple.github.io/foundationdb/testing.html | 2026-03-26 |
