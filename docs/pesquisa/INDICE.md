# Research Index -- Relativist Architecture & System Design

**Last updated:** 2026-03-26 (ALL COMPLETE: PESQ-001 to PESQ-024)
**Purpose:** Research library for Relativist's end-to-end specs (SPEC-10 through SPEC-13). Each PESQ document analyzes a system, framework, pattern, or technology relevant to Relativist's architecture decisions. Cataloged using bibliographic techniques for cross-referencing with existing specs, references, and discussions.

---

## Document Catalog

### Category 1: Grid Computing Architectures

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-001 | BOINC Volunteer Computing Architecture | **Complete** | SPEC-13, SPEC-10 |
| PESQ-002 | Apache Ignite Compute Grid | **Complete** | SPEC-13, SPEC-12 |
| PESQ-003 | Ray Distributed AI Framework | **Complete** | SPEC-13, SPEC-11 |
| PESQ-004 | Dask Distributed Scheduler | **Complete** | SPEC-13, SPEC-06 |
| PESQ-005 | HTCondor High-Throughput Computing | **Complete** | SPEC-10, SPEC-07 |

### Category 2: Rust Distributed Frameworks

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-006 | Hydro Dataflow Framework | **Complete** | SPEC-13, SPEC-11, SPEC-08 |
| PESQ-007 | Paladin Declarative Distributed | **Complete** | SPEC-05, SPEC-06, SPEC-08, SPEC-13 |
| PESQ-008 | Constellation Actor Model | **Complete** | SPEC-06, SPEC-13 |
| PESQ-009 | Other Rust Distributed Crates | **Complete** | SPEC-05, SPEC-06, SPEC-08, SPEC-13 |

### Category 3: System Design Patterns

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-010 | Coordinator-Worker Pattern | **Complete** | SPEC-05, SPEC-06, SPEC-13 |
| PESQ-011 | Work-Stealing Patterns | **Complete** | SPEC-05, SPEC-13 |
| PESQ-012 | MapReduce, Dataflow, and BSP | **Complete** | SPEC-04, SPEC-05, SPEC-09, SPEC-13 |
| PESQ-013 | State Machines in Distributed Systems | **Complete** | SPEC-06, SPEC-08, SPEC-11, SPEC-13 |

### Category 4: Observability & Tracing

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-014 | OpenTelemetry for Rust (2025) | **Complete** | SPEC-11, SPEC-13 |
| PESQ-015 | tracing Crate Ecosystem | **Complete** | SPEC-11, SPEC-13 |
| PESQ-016 | Prometheus Metrics Exposition | **Complete** | SPEC-11, SPEC-13 |

### Category 5: Security in Distributed Systems

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-017 | TLS 1.3 / rustls / mTLS | **Complete** | SPEC-10, SPEC-13 |
| PESQ-018 | Token Authentication & Trust Models | **Complete** | SPEC-06, SPEC-10, SPEC-13 |
| PESQ-019 | Security Lessons from CVEs | **Complete** | SPEC-06, SPEC-10, SPEC-13 |

### Category 6: Testing Distributed Systems

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-020 | Deterministic Simulation Testing Concepts | **Complete** | SPEC-08, SPEC-13 |
| PESQ-021 | Turmoil and MadSim | **Complete** | SPEC-08, SPEC-13 |
| PESQ-022 | Property-Based Testing for Distributed Systems | **Complete** | SPEC-01, SPEC-08 |

### Category 7: Synthesis

| ID | Title | Status | Informs |
|----|-------|--------|---------|
| PESQ-023 | Decision Matrix | **Complete** | All SPEC-10 to SPEC-13 |
| PESQ-024 | Architecture Recommendations | **Complete** | SPEC-13 (primary input) |

---

## Cross-Reference: Open Decisions -> PESQ Documents

| Open Decision | Primary PESQs | Target Spec |
|---------------|---------------|-------------|
| Workspace structure (single vs multi-crate) | PESQ-007, PESQ-008, PESQ-024 | SPEC-13 |
| Error handling (thiserror vs anyhow) | PESQ-024, PESQ-009 | SPEC-13 |
| Feature flags (tls, metrics) | PESQ-017, PESQ-016, PESQ-024 | SPEC-13 |
| Security model (TLS, auth, trust) | PESQ-005, PESQ-017, PESQ-018, PESQ-019 | SPEC-10 |
| Observability (metrics, traces, health) | PESQ-003, PESQ-014, PESQ-015, PESQ-016 | SPEC-11 |
| User I/O (DSL, formats, inspect) | PESQ-002, PESQ-012, PESQ-024 | SPEC-12 |
| Deterministic testing strategy | PESQ-006, PESQ-020, PESQ-021, PESQ-022 | SPEC-08 |
| In-memory testing runtime | PESQ-007, PESQ-020, PESQ-021 | SPEC-08 |
| System architecture (modules, FSM, deps) | PESQ-004, PESQ-006, PESQ-007, PESQ-010, PESQ-012, PESQ-013, PESQ-024 | SPEC-13 |
