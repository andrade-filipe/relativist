# TEST-SPEC-0043: Define PartitionStrategy trait

**Task:** TASK-0043
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: Trait is object-safe
Declare a variable `let _: &dyn PartitionStrategy;` -- this must compile, confirming the trait is object-safe.

### T2: Mock implementation compiles and is callable
Implement a `MockStrategy` struct that returns a fixed `HashMap<AgentId, WorkerId>`. Call `strategy.allocate(&net, 2)` and verify the returned map matches the expected fixed output.

### T3: Trait method signature matches spec
`PartitionStrategy::allocate(&self, net: &Net, num_workers: u32) -> HashMap<AgentId, WorkerId>` must be the exact signature. Verify by implementing a struct that uses all three parameters (`&self`, `&Net`, `u32`) and returns `HashMap<AgentId, WorkerId>`.

### T4: Trait is publicly exported from partition module
`use relativist::partition::PartitionStrategy;` must compile.

## Edge Cases

### E1: Mock strategy returning empty map for empty net
A mock that returns `HashMap::new()` for a net with 0 agents is valid and does not panic.

### E2: Mock strategy with num_workers=0
Calling `allocate(&net, 0)` with a mock that returns an empty map does not panic. The trait itself imposes no constraint on num_workers; callers enforce preconditions.
