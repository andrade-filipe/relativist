# Reviews -- TASK-0018: Verify PartialEq and Eq for Net

**Date:** 2026-04-08

---

## Code Cleaner: **PASS** -- No manual implementation needed; `PartialEq` and `Eq` are derived on the `Net` struct (line 18 of `src/net/core.rs`). All inner types (`Vec<Option<Agent>>`, `Vec<PortRef>`, `VecDeque<(u32, u32)>`, `u32`, `Option<PortRef>`) implement both traits, so derive is correct. Tests are focused and minimal.
## Architecture: **PASS** -- SPEC-02 R26 (round-trip identity) and R26a (derive PartialEq + Eq) satisfied. Structural equality (field-by-field) is the correct choice here; isomorphic equality is a separate concern (SPEC-08 `nets_isomorphic`). Derives are co-located with the struct definition in TASK-0008, making this a verification/testing task only.
## QA: **PASS** -- 4 tests: T1 (different next_id not equal), T2 (different agents not equal), T3 (different ports not equal), T4 (serde round-trip structural equality). All acceptance criteria covered. The derive-based approach guarantees no field is accidentally excluded from comparison. No panics possible.
