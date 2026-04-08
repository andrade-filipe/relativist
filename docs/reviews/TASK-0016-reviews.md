# Reviews -- TASK-0016: Define BorderMap type alias

**Date:** 2026-04-08

---

## Code Cleaner: **PASS** -- Single-line type alias with comprehensive doc comment explaining purpose (FreePort reverse lookup), ownership (external to Net, maintained by SPEC-04/SPEC-05), and usage context. Naming follows Rust convention (PascalCase). No dead code.
## Architecture: **PASS** -- SPEC-02 R23 satisfied. Defined in `src/net/types.rs` alongside related types (PortRef, AgentId). Publicly exported via `pub use types::*` in `src/net/mod.rs`, accessible as `crate::net::BorderMap`. Type alias correctly defers to `HashMap<u32, PortRef>`, matching SPEC-04 expectations. Not a field of Net (correct per spec).
## QA: **PASS** -- Type alias is compile-time verified (any use of `BorderMap` fails to compile if definition is wrong). No runtime logic, no panics possible. `BorderMap` is used downstream in `src/net/debug.rs` doc comments and will be used by partition/merge modules (SPEC-04/SPEC-05).
