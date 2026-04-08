# TEST-SPEC-0016: Define BorderMap type alias

**Task:** TASK-0016
**Spec:** SPEC-04 (via SPEC-02 R23)
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: BorderMap type is usable as HashMap<u32, PortRef>
Instantiate `BorderMap::new()`, insert `(0, AgentPort(1, 0))`, verify lookup returns correct value.

### T2: BorderMap is publicly accessible
Import `BorderMap` from the `net` module re-exports. Compiles without error.

## Edge Cases

### E1: Empty BorderMap
`BorderMap::new().is_empty()` returns `true`.

### E2: Multiple entries with distinct keys
Insert 3 entries with different FreePort IDs. All 3 retrievable.
