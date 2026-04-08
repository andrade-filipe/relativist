# TEST-SPEC-0042: Define PartitionPlan struct

**Task:** TASK-0042
**Spec:** SPEC-04
**Generated:** 2026-04-07 (retroactive)

---

## Unit Tests

### T1: PartitionPlan has required fields
Construct `PartitionPlan { partitions: vec![], borders: HashMap::new() }`. Verify both fields are accessible and have the expected types (`Vec<Partition>` and `HashMap<u32, (PortRef, PortRef)>`).

### T2: PartitionPlan derives Debug, Clone, Serialize, Deserialize
Create a `PartitionPlan` with 2 empty partitions and an empty border map. Call `format!("{:?}", plan)`, `plan.clone()`, and round-trip through `bincode::serialize`/`bincode::deserialize`. All must succeed without panic.

### T3: PartitionPlan with 2 partitions and empty border map
Construct a `PartitionPlan` with 2 `Partition` entries (both with empty subnets). Assert `plan.partitions.len() == 2` and `plan.borders.is_empty()`.

### T4: Insert and retrieve border entry
Create a `PartitionPlan` with an empty partitions vec. Insert `borders.insert(7, (AgentPort(0, 0), AgentPort(1, 0)))`. Assert `plan.borders.get(&7) == Some(&(AgentPort(0, 0), AgentPort(1, 0)))`.

### T5: PartitionPlan is publicly exported from partition module
`use relativist::partition::PartitionPlan;` must compile.

## Edge Cases

### E1: PartitionPlan with no partitions and no borders
`PartitionPlan { partitions: vec![], borders: HashMap::new() }` is valid construction, no panic.

### E2: PartitionPlan with large border map
Insert 1000 border entries with sequential IDs `(0..1000)`. Verify all 1000 entries are retrievable and `plan.borders.len() == 1000`.
