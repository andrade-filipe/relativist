# Architecture Review — TASK-0001

**Task:** Convert net module to directory structure
**Reviewer:** Architecture Reviewer (Stage 5)
**Date:** 2026-04-06

---

## Spec Compliance (SPEC-02, SPEC-13)

### SPEC-13 R5 — Module Structure

SPEC-13 R5 specifies:
```
src/
├── net/
│   ├── mod.rs
│   ├── agent.rs
│   ├── wire.rs
│   └── port.rs
```

**Current implementation:**
```
src/
├── net/
│   ├── mod.rs
│   ├── types.rs    (will contain Symbol, AgentId, PortId, PortRef, Agent)
│   ├── core.rs     (will contain Net struct + operations)
│   └── debug.rs    (will contain debug assertions)
```

**Assessment:** The sub-module naming differs from SPEC-13's suggestion (`types.rs` vs `agent.rs/wire.rs/port.rs`, `core.rs` vs no equivalent). This is acceptable because:

1. TASK-0001.md explicitly states: "The sub-module names are a suggestion. The key constraint is that types [...] live in one file, Net struct + operations in another, and debug assertions in a third."
2. `core.rs` was renamed from `net.rs` to avoid clippy's `module_inception` lint — a valid technical reason.
3. Grouping all types in `types.rs` is simpler than splitting across 3 files for a project of this size. Can be split later if the file grows too large.

**Verdict:** COMPLIANT (naming is a suggestion, structure is correct)

### Dependency Direction

- `net/` is a Core Layer module (SPEC-13 R6). It has no dependencies on tokio, async, or I/O.
- **Verdict:** COMPLIANT

## Architecture Issues

None.

## Pattern Review

- Re-export pattern (`pub use types::*; pub use core::*;`) is idiomatic Rust for module facades.
- Debug module separated from production code — follows separation of concerns.

## Verdict

**PASS** — Module structure is compliant with SPEC-13 R5 (naming flexibility acknowledged) and R6 (core layer, no async/IO dependencies).
