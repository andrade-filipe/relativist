---
pesq_id: PESQ-021
title: "Turmoil and MadSim: DST Frameworks for Rust"
category: Testing Distributed Systems
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-08, SPEC-13]
  pesqs: [PESQ-020, PESQ-022]
  discs: []
---

# PESQ-021: Turmoil and MadSim — DST Frameworks for Rust

**Category:** Testing Distributed Systems
**Status:** Complete

---

## 1. Subject Overview

Two main DST frameworks exist for Rust:

| Framework | Approach | Maturity | Used by |
|-----------|----------|----------|---------|
| **Turmoil** | Simulated tokio runtime with network control | Experimental (tokio team) | Community projects |
| **MadSim** | Drop-in tokio replacement via API compatibility | Production | RisingWave |

---

## 2. Turmoil

**Repository:** https://github.com/tokio-rs/turmoil
**Maintainer:** Tokio team
**Approach:** Provides a simulated network where you build your distributed system as "hosts" in a single process.

### 2.1 How It Works

```rust
use turmoil::Builder;

#[test]
fn test_coordinator_worker() {
    let mut sim = Builder::new().build();

    // Add coordinator host
    sim.host("coordinator", || async {
        let listener = turmoil::net::TcpListener::bind("0.0.0.0:9000").await?;
        // ... coordinator logic using turmoil's TCP
        Ok(())
    });

    // Add worker host
    sim.host("worker-1", || async {
        let stream = turmoil::net::TcpStream::connect("coordinator:9000").await?;
        // ... worker logic using turmoil's TCP
        Ok(())
    });

    // Run simulation with fault injection
    sim.run();
}
```

### 2.2 Capabilities

- Simulated TCP (TcpListener, TcpStream)
- Network partition: `sim.partition("coordinator", "worker-1")`
- Network repair: `sim.repair("coordinator", "worker-1")`
- Message latency control
- Deterministic scheduling (single-threaded)
- Time control: `sim.run_until(Duration::from_secs(10))`

### 2.3 Limitations

- Requires using `turmoil::net` instead of `tokio::net` — code must be generic over transport
- Experimental status — API may change
- No disk simulation (only network)
- No libc interception (unlike MadSim)

---

## 3. MadSim

**Repository:** https://github.com/madsim-rs/madsim
**Maintainer:** Community (originally from RisingWave team)
**Approach:** Drop-in replacement for tokio — same API, deterministic execution.

### 3.1 How It Works

MadSim intercepts system calls (gettimeofday, clock_gettime, getrandom) at the libc level, providing deterministic time and randomness. Application code uses standard tokio APIs — the difference is compile-time: `madsim` replaces tokio.

```toml
# Cargo.toml
[target.'cfg(madsim)'.dependencies]
tokio = { version = "1", package = "madsim-tokio" }
```

```rust
// Application code is UNCHANGED — uses tokio::net, tokio::time, etc.
// When compiled with --cfg madsim, it uses the simulator instead.
```

### 3.2 Capabilities

- Drop-in tokio replacement (TcpListener, TcpStream, sleep, spawn)
- Deterministic RNG (getrandom intercepted)
- Deterministic time (clock_gettime intercepted)
- Network fault injection (partition, delay, drop)
- Node crash/restart simulation
- Seed-based reproducibility

### 3.3 Limitations

- Linux only (libc interception)
- Requires cfg-based conditional compilation
- Some tokio features not supported
- Adds complexity to build system

---

## 4. Comparison

| Dimension | Turmoil | MadSim | Relativist Need |
|-----------|---------|--------|----------------|
| API approach | Custom turmoil::net | Drop-in tokio replacement | MadSim is less invasive |
| Platform | Cross-platform | Linux only | Cross-platform needed |
| Code changes | Must use turmoil types | Conditional compilation | MadSim preferred |
| Network simulation | Yes | Yes | Essential |
| Disk simulation | No | Yes | Not needed |
| Time control | Yes | Yes (libc intercept) | Useful |
| Maturity | Experimental | Production (RisingWave) | MadSim more proven |
| Maintenance | Tokio team | Community | Both active |

---

## 5. Recommendation for Relativist

### 5.1 v1: Neither (Use In-Memory Grid)

Both Turmoil and MadSim add significant complexity. For v1:
- Use the in-memory grid mode (PESQ-020 L1)
- Abstract network behind `trait Transport` (PESQ-020 L2)
- Standard `#[tokio::test]` for unit/integration tests

### 5.2 v2: Turmoil Preferred

If DST is added in v2, **Turmoil** is preferred because:
1. Cross-platform (MadSim is Linux-only)
2. Maintained by the tokio team
3. The `trait Transport` abstraction makes Turmoil integration straightforward
4. Relativist doesn't need disk simulation (only network)

The migration path:
```rust
// Production: TcpTransport (real tokio::net)
// In-memory test: ChannelTransport (tokio::sync::mpsc)
// DST test: TurmoilTransport (turmoil::net)
```

All three implement the same `Transport` trait.

---

## 6. Lessons for Relativist

### L1: Skip DST Frameworks in v1 [REJECT for v1]
Both Turmoil and MadSim add too much complexity for v1. The in-memory grid mode is sufficient.
→ Informs: SPEC-08

### L2: Turmoil Over MadSim for v2 [ADAPT]
If DST is added later, prefer Turmoil (cross-platform, tokio team) over MadSim (Linux-only, libc hacks).
→ Informs: SPEC-08 (roadmap)

### L3: Transport Trait is the Foundation [ADOPT]
The `trait Transport` abstraction enables all three modes (TCP, channel, simulation) without changing application logic. This is the single most important architectural decision for testability.
→ Informs: SPEC-08, SPEC-13

---

## 7. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| Turmoil announcement | https://tokio.rs/blog/2023-01-03-announcing-turmoil | 2026-03-26 |
| Turmoil GitHub | https://github.com/tokio-rs/turmoil | 2026-03-26 |
| MadSim GitHub | https://github.com/madsim-rs/madsim | 2026-03-26 |
| MadSim lib.rs | https://lib.rs/crates/madsim | 2026-03-26 |
| DST for Async Rust (s2.dev) | https://s2.dev/blog/dst | 2026-03-26 |
