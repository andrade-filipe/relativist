---
pesq_id: PESQ-022
title: "Property-Based Testing for Distributed Systems"
category: Testing Distributed Systems
date: 2026-03-26
status: Complete
cross_references:
  specs: [SPEC-08, SPEC-01, SPEC-13]
  pesqs: [PESQ-020, PESQ-021]
  discs: [DISC-005, DISC-008]
---

# PESQ-022: Property-Based Testing for Distributed Systems

**Category:** Testing Distributed Systems
**Status:** Complete

---

## 1. Subject Overview

Property-based testing (PBT) generates random inputs and verifies that properties (invariants) hold for all of them. When a property fails, the framework **shrinks** the input to find the minimal counterexample.

### 1.1 Rust Frameworks

| Framework | Style | Shrinking | Recommended |
|-----------|-------|-----------|-------------|
| `proptest` | Strategy-based (per-value) | Automatic, composable | **Yes** |
| `quickcheck` | Arbitrary trait (per-type) | Automatic | Alternative |
| `bolero` | Unified PBT + fuzzing | Via engines | Advanced |

**Recommendation: `proptest`** — more flexible strategy composition, better shrinking, Hypothesis-inspired API.

---

## 2. Properties for Relativist

### 2.1 Core IC Properties (from SPEC-01)

These are the invariants that MUST hold under all inputs:

| Property | Description | Test Strategy |
|----------|-------------|---------------|
| **P1 (Confluence)** | Different reduction orders produce same normal form | Generate net, reduce with different orders, compare |
| **P2 (Split/Merge Integrity)** | `merge(split(net, k)) ≈ net` | Generate net, split into k, merge back, verify structural equality |
| **P3 (Border Completeness)** | All border redexes resolved after merge | Generate net with cross-partition edges, merge, check no border redexes |
| **T1-T7 (Type invariants)** | Port counts, connectivity, no self-loops | Generate arbitrary nets, verify invariants hold after every operation |
| **D1-D6 (Distribution invariants)** | Partition ID consistency, no shared agents | Generate splits, verify each partition independently |

### 2.2 Protocol Properties (from SPEC-06)

| Property | Description | Test Strategy |
|----------|-------------|---------------|
| **Serialization roundtrip** | `deserialize(serialize(msg)) == msg` | Generate random messages, verify roundtrip |
| **Frame integrity** | CRC32 matches payload | Generate payloads, compute CRC32, verify |
| **Message ordering** | Response follows request | Generate message sequences, verify protocol FSM |

### 2.3 Grid Cycle Properties (from SPEC-05)

| Property | Description | Test Strategy |
|----------|-------------|---------------|
| **Termination** | Grid cycle eventually terminates | Generate small nets, run full cycle, verify termination |
| **Correctness** | Distributed result equals local result | Generate net, reduce locally AND via grid, compare |
| **Monotonic progress** | Each round reduces total redexes or border redexes | Track metrics per round, verify non-increasing |

---

## 3. proptest Strategies for IC Nets

### 3.1 Net Generation

```rust
use proptest::prelude::*;

fn arb_agent_type() -> impl Strategy<Value = AgentType> {
    prop_oneof![
        Just(AgentType::CON),
        Just(AgentType::DUP),
        Just(AgentType::ERA),
    ]
}

fn arb_net(max_agents: usize) -> impl Strategy<Value = Net> {
    (1..=max_agents)
        .prop_flat_map(|n| {
            prop::collection::vec(arb_agent_type(), n)
                .prop_map(|types| build_random_net(types))
        })
}

proptest! {
    #[test]
    fn confluence_holds(net in arb_net(50)) {
        let result_a = reduce_order_a(net.clone());
        let result_b = reduce_order_b(net);
        prop_assert_eq!(result_a, result_b);
    }

    #[test]
    fn split_merge_identity(net in arb_net(100), k in 2..8usize) {
        let partitions = split(&net, k);
        let merged = merge(partitions);
        prop_assert!(structurally_equal(&net, &merged));
    }

    #[test]
    fn serialization_roundtrip(msg in arb_message()) {
        let bytes = serialize(&msg);
        let decoded = deserialize(&bytes);
        prop_assert_eq!(msg, decoded);
    }
}
```

### 3.2 Shrinking Benefits

When `confluence_holds` fails, proptest will:
1. Find the failing net (e.g., 47 agents)
2. Shrink to find minimal counterexample (e.g., 3 agents, specific topology)
3. Report the minimal failing case

This is enormously valuable for debugging: instead of "fails on a 1000-agent net," you get "fails on CON(0)—DUP(1)—ERA(2) with specific wiring."

---

## 4. Integration with SPEC-08 Test Catalog

SPEC-08 catalogs ~130 tests. Property-based tests complement, not replace, example-based tests:

| Test Layer | Example-Based | Property-Based |
|-----------|---------------|---------------|
| Net structure (T1-T7) | Specific known nets | Random nets, verify invariants |
| Reduction (RE1-RE21) | Each rule individually | Random nets, verify P1 (confluence) |
| Partition (P1-P21) | Known topologies | Random nets, verify P2 (split/merge) |
| Protocol (SPEC-06) | Specific message sequences | Random messages, verify roundtrip |
| Grid cycle (I1-I11) | Known workload profiles | Small random nets, verify termination |

**Rule of thumb:** Example-based tests for known edge cases and regression. Property-based tests for invariant verification across the input space.

---

## 5. Lessons for Relativist

### L1: proptest for All Invariant Tests [ADOPT]
Use proptest to verify P1, P2, P3, T1-T7, D1-D6. These properties are the mathematical core of Relativist's correctness — they should hold for ALL valid nets, not just specific examples.
→ Informs: SPEC-08

### L2: Custom Net Generators [ADOPT]
Build proptest strategies that generate valid IC nets (respecting port constraints, connectivity rules). This is the foundation for all property tests.
→ Informs: SPEC-08

### L3: Shrinking is Critical for Debugging [ADOPT]
proptest's shrinking finds minimal counterexamples. For complex IC nets, this is the difference between "fails on a huge net" and "fails on 3 agents with this specific wiring."
→ Informs: SPEC-08

### L4: Complement, Don't Replace Example Tests [ADOPT]
Property tests verify broad invariants. Example tests verify specific known behaviors (each of the 6 rules, specific edge cases). Both are needed.
→ Informs: SPEC-08

### L5: Keep Generated Nets Small [ADOPT]
Property tests should use nets of 1-100 agents. Larger nets make tests slow and shrinking expensive. Use benchmarks (SPEC-09) for large nets.
→ Informs: SPEC-08, SPEC-09

---

## 6. Sources

| Source | URL | Accessed |
|--------|-----|----------|
| proptest GitHub | https://github.com/proptest-rs/proptest | 2026-03-26 |
| proptest crates.io | https://crates.io/crates/proptest | 2026-03-26 |
| Property-Based Testing intro (Palmieri) | https://www.lpalmieri.com/posts/an-introduction-to-property-based-testing-in-rust/ | 2026-03-26 |
| PBT in Rust (LogRocket) | https://blog.logrocket.com/property-based-testing-in-rust-with-proptest/ | 2026-03-26 |
| PBT examples (LambdaClass) | https://blog.lambdaclass.com/what-is-property-based-testing/ | 2026-03-26 |
| Rust Testing Strategies 2026 | https://dasroot.net/posts/2026/03/rust-testing-strategies-unit-integration-property-tests/ | 2026-03-26 |
