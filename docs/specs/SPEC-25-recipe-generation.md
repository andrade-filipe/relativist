# SPEC-25: Recipe-Based Distributed Generation

**Status:** Draft
**Depends on:** SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-06 (Wire Protocol), SPEC-09 (Benchmark Suite), SPEC-13 (System Architecture)
**ROADMAP items:** 2.29 (Recipe-Based Distributed Generation)
**References consumed:** REF-002 (Lafont 1997, p.70-73: net structure), REF-001 (Lafont 1990: locality)
**Arguments consumed:** ARG-001 (P1-P6), ARG-002 (C1-C3 partitioning correctness)
**Briefings consumed:** BRIEF-20260415-v2-tier5-teorica (Section 2.29: ARG-006, per-generator decomposability), BRIEF-20260415-v2-tier5-codebase (Section 2.29: generator analysis, IdRange infrastructure)

---

## 1. Purpose

This spec defines recipe-based distributed generation: instead of the coordinator generating the full net, partitioning it, and shipping partitions to workers, the coordinator sends a lightweight recipe to each worker, and each worker generates its own partition locally. The coordinator never holds the full net — its memory is O(K × recipe_size), where K is the worker count and recipe_size is typically a few hundred bytes.

This is the most memory-efficient generation strategy: it eliminates all four coordinator memory peaks (generation, partitioning, dispatch buffer, merge) for benchmarks with decomposable topology. It complements SPEC-21 (streaming generation), which reduces coordinator memory but does not eliminate it. For benchmarks that cannot be decomposed (Church numerals, `tree_sum`), SPEC-21 streaming remains the only option.

**Formal foundation (ARG-006):** The correctness of recipe-based generation rests on proving that `merge(gen_local(recipe_1), ..., gen_local(recipe_K)) ≅ gen_centralized(bench, size)`. This is a per-generator property, not a generic one: each benchmark's decomposition must be individually verified. Section 4 formalizes this requirement.

---

## 2. Definitions

Terms defined in SPEC-00, SPEC-01, SPEC-02, SPEC-04, SPEC-06, SPEC-09, and SPEC-13 are used without redefinition. Terms introduced in this spec:

| Term | Definition |
|------|-----------|
| **GenerationRecipe** | A serializable descriptor telling a worker how to generate its local partition: benchmark type, global size, worker ID, ID range, and border specification. Approximately 100-200 bytes per worker. |
| **Recipe Decomposition** | The process of computing K `GenerationRecipe` values from a benchmark type, global size, and worker count. A benchmark is recipe-decomposable if a correct decomposition exists. |
| **Border Specification** | A list of `(local_port, border_id)` pairs included in the recipe that tells the worker which of its locally-generated ports must be connected to ports on other workers. For independent-pair benchmarks, this list is empty. |
| **Local Partition Generation** | The process by which a worker creates a `Partition` from a `GenerationRecipe` without coordinator involvement. The worker pre-sets `net.next_id = recipe.id_range.start` and generates agents with IDs in the assigned range. |
| **Recipe-Decomposable Benchmark** | A benchmark whose net topology can be partitioned into K independent (or minimally-connected) fragments, each describable by a recipe. Independent-pair benchmarks are trivially decomposable; tree structures are partially decomposable. |
| **Centralized Fallback** | For non-decomposable benchmarks, the coordinator falls back to the standard generate→split→dispatch pipeline (SPEC-04) or streaming pipeline (SPEC-21). |

---

## 3. Requirements

### 3.1 GenerationRecipe Type

**R1.** Relativist MUST define a `GenerationRecipe` struct in `src/io/recipe.rs`:
```rust
pub struct GenerationRecipe {
    pub benchmark: ExampleNet,
    pub global_size: u32,
    pub num_workers: u32,
    pub worker_id: WorkerId,
    pub id_range: (AgentId, AgentId),  // [start, end) exclusive
    pub borders: Vec<BorderSpec>,
}
```
**(MUST)**

**R2.** `GenerationRecipe` MUST implement `Debug`, `Clone`, `Serialize`, `Deserialize`. **(MUST)**

**R3.** The `id_range` field MUST specify a half-open range `[start, end)` of agent IDs allocated to this worker. ID ranges across all K workers MUST be disjoint and cover the full ID space needed by the benchmark: `union(id_range_w for w in 0..K) = [0, total_agents)`. **(MUST)**

**R4.** `BorderSpec` MUST be defined as:
```rust
pub struct BorderSpec {
    pub local_agent_id: AgentId,
    pub local_port: PortId,
    pub border_id: u32,
}
```
This tells the worker: "after generating your partition, set port `(local_agent_id, local_port)` to `FreePort(border_id)` to create a border connection." **(MUST)**

### 3.2 Recipe Computation

**R5.** Relativist MUST provide a function `compute_recipes` in `src/io/recipe.rs`:
```rust
pub fn compute_recipes(
    benchmark: ExampleNet,
    size: u32,
    num_workers: u32,
) -> Result<Vec<GenerationRecipe>, RecipeError>
```
that computes K recipes for the given benchmark. If the benchmark is not recipe-decomposable, this function MUST return `Err(RecipeError::NotDecomposable)`. **(MUST)**

**R6.** `compute_recipes` MUST use `compute_id_ranges()` from `src/partition/helpers.rs` to allocate disjoint ID ranges. **(MUST)**

**R7.** `compute_recipes` MUST support the following benchmarks at minimum:
- `EpAnnihilation`: each worker generates `size/K` ERA-ERA pairs. No borders. **(MUST)**
- `EpAnnihilationCon`: each worker generates `size/K` CON-CON pairs. No borders. **(MUST)**
- `EpAnnihilationDup`: each worker generates `size/K` DUP-DUP pairs. No borders. **(MUST)**
- `ConDupExpansion`: each worker generates `size/K` CON-DUP pairs. No borders. **(MUST)**
- `MixedRules`: each worker generates `size/K` mixed pairs (thirds of ERA-ERA, CON-CON, CON-DUP). No borders. **(MUST)**

**R8.** `compute_recipes` SHOULD support `DualTree` and `ErasurePropagation` with border specifications. For `DualTree(D)`, the recipe SHOULD split the tree at level boundaries, with border wires at the cut points. **(SHOULD)**

**R9.** `compute_recipes` MUST return `Err(RecipeError::NotDecomposable)` for `TreeSum` and `SumOfSquares`. These benchmarks have sequential DUP-chain structure that cannot be partitioned without the full chain context. **(MUST)**

### 3.3 Local Partition Generation

**R10.** Each generator in `src/io/generators.rs` that is recipe-decomposable MUST gain a new function:
```rust
pub fn generate_partition(recipe: &GenerationRecipe) -> Partition
```
This function creates a `Net` with `next_id = recipe.id_range.0`, generates agents with IDs in the assigned range, applies border specifications, and wraps the net in a `Partition`. **(MUST)**

**R11.** `generate_partition` MUST set `net.next_id = recipe.id_range.0` before generating any agents, ensuring all created agent IDs fall within `[id_range.0, id_range.1)`. **(MUST)**

**R12.** After generating internal connections, `generate_partition` MUST iterate `recipe.borders` and for each `BorderSpec`, set `net.ports[port_index(local_agent_id, local_port)] = FreePort(border_id)`. This creates the border connections that `merge()` (SPEC-05) will resolve. **(MUST)**

**R13.** `generate_partition` MUST verify that the number of created agents equals `recipe.id_range.1 - recipe.id_range.0`. If not, it MUST return an error. **(MUST)**

**R14.** The generated partition MUST satisfy C1, C2, and C3 (SPEC-04, ARG-002) when combined with all other workers' partitions:
- C1: every agent ID in the global range appears in exactly one partition.
- C2: every wire in the original topology is either internal to one partition or represented as a border.
- C3: every border ID appears in exactly 2 partitions.
**(MUST)**

### 3.4 Protocol Integration

**R15.** The `Message` enum (SPEC-06, `protocol/types.rs`) MUST gain a new variant:
```rust
AssignRecipe {
    round: u32,
    recipe: GenerationRecipe,
}
```
Appended after the last discriminant to preserve bincode discriminant stability (SPEC-06 R5). **(MUST)**

**R16.** The coordinator MUST support a new dispatch path: if `compute_recipes` succeeds for the given benchmark, the coordinator sends `AssignRecipe` to each worker instead of `AssignPartition`. **(MUST)**

**R17.** The worker MUST handle `AssignRecipe` by calling `generate_partition(recipe)`, then proceeding with local reduction as if it had received `AssignPartition`. The subsequent `PartitionResult` message is identical. **(MUST)**

**R18.** If the benchmark is not recipe-decomposable (`RecipeError::NotDecomposable`), the coordinator MUST fall back to the standard generate→split→dispatch pipeline (SPEC-04) or streaming pipeline (SPEC-21). This fallback MUST be transparent to the worker. **(MUST)**

**R19.** The `GridConfig` struct MUST gain a field `generation_mode: GenerationMode` where:
```rust
pub enum GenerationMode {
    Centralized,  // default: coordinator generates, splits, dispatches
    Streaming,    // SPEC-21: chunked pipeline
    Recipe,       // SPEC-25: recipe-based distributed generation
    Auto,         // tries Recipe first, falls back to Streaming then Centralized
}
```
**(MUST)**

**R20.** The default `GenerationMode` MUST be `Auto`. **(MUST)**

### 3.5 Correctness (ARG-006)

**R21.** For each recipe-decomposable benchmark, the following equivalence MUST hold:
```
merge(generate_partition(recipe_0), ..., generate_partition(recipe_{K-1}))
  ≅ generate(benchmark, size)
```
where `≅` means isomorphic up to agent ID renumbering. **(MUST)**

**R22.** R21 MUST be verified by an equivalence test for each supported benchmark (see Section 7.2). **(MUST)**

**R23.** For independent-pair benchmarks (zero borders), the proof of R21 is trivial: the union of K disjoint sets of independent pairs equals the full set. No cross-partition wires exist, so merge is a simple union. SPEC-25 MUST document this proof sketch in an appendix or in the test comments. **(MUST)**

**R24.** For benchmarks with borders (e.g., `DualTree`), the proof of R21 requires showing that the border specification correctly encodes all cross-partition wires. SPEC-25 MUST document the border computation algorithm and prove its completeness for each supported benchmark with borders. **(MUST)**

---

## 4. Formal Argument: Recipe Generation Correctness (ARG-006)

### 4.1 Statement

For any recipe-decomposable benchmark B, global size N, and worker count K:

Let `net = gen_centralized(B, N)` (the full net produced by existing generators).
Let `{recipe_w}_{w=0}^{K-1} = compute_recipes(B, N, K)`.
Let `{partition_w}_{w=0}^{K-1}` where `partition_w = generate_partition(recipe_w)`.

Then: `merge(partition_0, ..., partition_{K-1}) ≅ net`.

### 4.2 Proof Structure (Per-Benchmark)

**Independent-Pair Benchmarks (ep_annihilation, ep_annihilation_con, ep_annihilation_dup, con_dup_expansion, mixed_rules):**

1. `gen_centralized(B, N)` produces N pairs `{(a_i, b_i)}_{i=0}^{N-1}` where each pair is internally connected and independent.
2. `compute_recipes(B, N, K)` assigns pairs `[w*chunk, (w+1)*chunk)` to worker w, where `chunk = ceil(N/K)`.
3. Each worker generates `chunk` pairs with IDs in `[id_range.start, id_range.end)`.
4. Since pairs are independent, `union(partition_w) = net` as a set of agents and wires.
5. No borders exist (`borders = []` in every recipe), so `merge = union`.
6. QED: `merge(partition_0, ..., partition_{K-1}) = gen_centralized(B, N)`.

**DualTree(D):**

1. `gen_centralized(DualTree, D)` produces two mirrored complete binary trees of depth D, connected at the root.
2. `compute_recipes(DualTree, D, K)` assigns tree levels to workers. Worker 0 gets levels 0..L, worker 1 gets levels L..2L, etc. (exact split depends on K and D).
3. The cut between level ranges creates border wires: the children of the last level in worker w's range become border connections to the first level in worker (w+1)'s range.
4. Each worker's `borders` field contains `BorderSpec` entries for these cut points.
5. After local generation, `merge()` reconnects borders, reconstructing the original tree.
6. Proof: by induction on D. Base case (D=1): single pair, trivially correct. Inductive step: if level-split is correct for D-1, adding one level either stays within one worker or creates a border at the new level.
7. QED: `merge(partition_0, ..., partition_{K-1}) ≅ gen_centralized(DualTree, D)`.

### 4.3 Non-Decomposable Benchmarks

`TreeSum` and `SumOfSquares` use Church numeral encoding where each number is a chain of DUP agents. The chain must be contiguous because each DUP's auxiliary port connects to the next DUP in sequence. Splitting this chain across workers would require border wires at every cut point, with border count proportional to chain length — no better than centralized generation. Therefore these benchmarks are NOT recipe-decomposable.

---

## 5. Non-Goals

**NG1.** Recipe-based generation for Church-encoded benchmarks (`tree_sum`, `sum_of_squares`). These are inherently sequential and fall back to centralized or streaming generation.

**NG2.** Dynamic recipe recomputation during reduction. The recipe is computed once at job start and does not change. If workers join/depart dynamically (SPEC-20), the coordinator re-partitions using the standard split/merge path, not recipe regeneration.

**NG3.** Recipe compression or caching. Recipes are ~100-200 bytes — compression would save nothing meaningful.

---

## 6. Memory Budget Analysis

### 6.1 Coordinator Memory

| Pipeline | Peak Memory |
|----------|-------------|
| Centralized (v1) | O(total_agents) — full net in memory |
| Streaming (SPEC-21) | O(chunk_size + border_tracking) |
| **Recipe (SPEC-25)** | **O(K × recipe_size) ≈ O(K × 200 bytes)** |

For `ep_annihilation_con(50M)` with K=4 workers:
- Centralized: ~6.4 GB (100M agents × 64 bytes)
- Streaming (chunk=10K): ~10 MB
- **Recipe: ~800 bytes (4 × 200)**

### 6.2 Worker Memory

Worker memory is identical across all three pipelines: O(total_agents / K) per worker. The generation strategy only affects the coordinator.

---

## 7. Test Strategy

### 7.1 Unit Tests

**T1. Recipe computation for independent-pair benchmarks.**
- For each of [EpAnnihilation, EpAnnihilationCon, EpAnnihilationDup, ConDupExpansion, MixedRules]:
  `compute_recipes(bench, 100, 4)` returns 4 recipes with disjoint ID ranges covering [0, 200).
  All `borders` lists are empty.

**T2. Recipe computation for non-decomposable benchmarks.**
- `compute_recipes(TreeSum, 10, 4)` returns `Err(RecipeError::NotDecomposable)`.
- `compute_recipes(SumOfSquares, 5, 4)` returns `Err(RecipeError::NotDecomposable)`.

**T3. Local partition generation.**
- For `EpAnnihilation(100)` with 4 workers:
  Generate 4 partitions from 4 recipes. Verify each partition has 50 agents (25 pairs).
  Verify all agent IDs are within the assigned `id_range`.

**T4. Agent count validation.**
- Create a recipe with `id_range = (0, 100)` but generate only 50 agents.
  Verify `generate_partition` returns an error.

### 7.2 Equivalence Tests (ARG-006 Verification)

**T5. Recipe vs. centralized equivalence (independent pairs).**
- For each decomposable benchmark at size=100, K=4:
  1. `net_centralized = generate(bench, 100)`
  2. `recipes = compute_recipes(bench, 100, 4)`
  3. `partitions = [generate_partition(r) for r in recipes]`
  4. `net_recipe = merge(partitions)`
  5. Verify `net_centralized ≅ net_recipe` (isomorphic modulo ID renumbering).

**T6. Recipe vs. centralized reduction equivalence.**
- For each decomposable benchmark at size=100:
  1. `result_seq = reduce_all(generate(bench, 100))`
  2. Run `run_grid` with `GenerationMode::Recipe`.
  3. Verify both reach the same normal form (isomorphic results, identical interaction counts per T7).

**T7. Border specification correctness (DualTree).**
- If DualTree recipe is implemented:
  1. `recipes = compute_recipes(DualTree, 4, 2)` (depth 4, 2 workers)
  2. Verify `borders` lists are non-empty.
  3. Verify merged result is isomorphic to centralized DualTree(4).

### 7.3 Property-Based Tests

**T8. Worker count independence.**
- For a fixed `(bench, size)`, vary K from 1 to 8.
  Verify merged result is isomorphic to centralized generation for all K.

**T9. ID range disjointness.**
- For random `(bench, size, K)` triples:
  Verify `compute_recipes` produces non-overlapping ID ranges that cover [0, total_agents).

### 7.4 Integration Tests

**T10. Full grid cycle with recipe mode.**
- Run `run_grid` with `GenerationMode::Recipe` for `ep_annihilation_con(1000)`, K=2.
  Verify G1: result matches sequential baseline.

**T11. Auto mode fallback.**
- Run with `GenerationMode::Auto` for `TreeSum(5)`.
  Verify the system falls back to centralized generation.
  Verify the result is correct.

---

## 8. Open Questions

**Q1. DualTree border computation complexity.** For `DualTree(D)` with K workers, the number of border wires at each level cut is `2^level`. For deep trees split across many workers, the border specification could grow to O(2^D / K) entries. This is still much smaller than the net itself (O(2^D) agents) but should be benchmarked.

**Q2. ErasurePropagation decomposition.** The erasure propagation chain (N CON agents connected in series with an ERA at the head) can be split at K-1 cut points, creating K-1 border wires. Each worker generates a contiguous segment of the chain. The border specification is simple: each worker's last CON connects to the next worker's first CON. This should be straightforward but needs careful ID range management.

**Q3. Interaction with delta protocol.** Under the delta protocol (SPEC-19), if workers generate their own partitions via recipe, the `InitialPartition` message (SPEC-19 R3) is replaced by `AssignRecipe`. The coordinator never holds the full partition state. The delta protocol's `RoundStart`/`RoundResult` messages work identically regardless of how the initial partition was created.

**Q4. Recipe versioning.** If the generator implementation changes (e.g., a new agent layout for `ep_annihilation`), old recipes become invalid. Should recipes include a version field? For the TCC, generator implementations are frozen with v1, so this is not an immediate concern.
