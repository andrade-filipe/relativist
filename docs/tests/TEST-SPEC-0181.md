# TEST-SPEC-0181: BenchmarkId, Mode, and core enums

**Task:** TASK-0181
**Spec:** SPEC-09
**Requirements verified:** R1, R2, R6, R8, R26

---

## Tests

### T1: BenchmarkId Display (snake_case)

**Type:** Unit
**Input:** Each `BenchmarkId` variant.
**Expected output:**
- `EPAnnihilation` -> `"ep_annihilation"`
- `EPAnnihilationCon` -> `"ep_annihilation_con"`
- `EPAnnihilationDup` -> `"ep_annihilation_dup"`
- `ConDupExpansion` -> `"condup_expansion"`
- `DualTree` -> `"dual_tree"`
- `TreeSum` -> `"tree_sum"`
- `TreeSumBalanced` -> `"tree_sum_balanced"`
- `MixedNet` -> `"mixed_net"`
- `ErasurePropagation` -> `"erasure_propagation"`
- `ChurchAdd` -> `"church_add"`
- `ChurchMul` -> `"church_mul"`

### T2: Mode Display

**Type:** Unit
**Input:** Each `Mode` variant.
**Expected output:**
- `Sequential` -> `"sequential"`
- `Local` -> `"local"`
- `TcpLocalhost` -> `"tcp_localhost"`
- `TcpNetwork` -> `"tcp_network"`

### T3: BenchmarkId is Copy

**Type:** Unit
**Input:** `let a = BenchmarkId::DualTree; let b = a;`
**Expected:** Both `a` and `b` are usable (no move).

### T4: BenchmarkId serde round-trip

**Type:** Unit
**Input:** Serialize `BenchmarkId::TreeSum` to JSON, deserialize back.
**Expected:** Deserialized value == original.

### T5: BenchmarkId variant count

**Type:** Unit
**Verification:** There are exactly 11 variants (R8 requires at least 10).
**Method:** Create a Vec with all variants, assert len == 11.

## Edge Cases

1. **Display is not Debug:** Verify `format!("{}", id)` produces snake_case, NOT the Debug `EPAnnihilation` format.
2. **Hash consistency:** Two equal BenchmarkId values produce the same hash (required for HashMap keys).
