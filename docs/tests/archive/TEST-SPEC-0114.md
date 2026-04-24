# TEST-SPEC-0114: Implement run_generate_command (workload generator entry point)

**Task:** TASK-0114
**Spec:** SPEC-07
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Generate produces a deserializable .bin file

**Type:** Integration test
**Input:**
```
let args = GenerateArgs {
    example: ExampleNet::EpAnnihilation,
    n: 10,
    output: tmp_dir.path().join("gen.bin"),
};
run_generate_command(args).unwrap();
let net = load_net_from_file(&tmp_dir.path().join("gen.bin")).unwrap();
```
**Expected:** `Ok(())`; file exists; net has agents (count > 0)
**Verifies:** R8 -- generate creates a valid .bin file

### T2: Generated net has expected agent count

**Type:** Integration test
**Input:** Generate with `n = 20` for a known workload
**Expected:** Net agent count is proportional to n (e.g., for ep-annihilation: ~20 agents)
**Verifies:** R33 -- size parameter controls network size

### T3: Deterministic generation

**Type:** Unit test
**Input:** Generate the same workload with `n = 10` twice
**Expected:** Both nets are byte-identical after serialization
**Verifies:** R33 -- same parameters produce same net

### T4: Function is synchronous

**Type:** Compilation test
**Input:** `run_generate_command(args)` -- no `.await`
**Expected:** Compiles without async
**Verifies:** Generate command uses synchronous I/O

### T5: Info-level logging of generated net

**Type:** Integration test
**Input:** Generate a net; capture tracing output
**Expected:** Log contains "generated network" with agent count and example name
**Verifies:** Acceptance criteria: info-level logging

---

## Edge Cases

### E1: n = 0 produces a minimal or empty net

**Verify:** `GenerateArgs { n: 0, .. }` either produces an empty net or returns an error.
**Why:** Edge case: zero-size workload should be handled gracefully.

### E2: Unknown workload returns error with available list

**Verify:** If using string-based dispatch (not value_enum), an unknown workload name returns `Err(RelativistError::Config(_))` with a message listing available workloads. If using value_enum, clap rejects it at parse time.
**Why:** User should know which workloads are available.
