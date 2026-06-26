# TEST-SPEC-TASK-0711: Tests for TASK-0711 — `wire_add_into` / `wire_mul_into` R13a' obligation validation (Phase 3a promotion)

**Task:** TASK-0711
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R13a' (composable arithmetic helpers obligation set: T1-T7 preservation, reduction equivalence, `pub(crate)` privacy)
**Test IDs (from SPEC-27 v3 §7):** Foundational — supports T6, T7, T11, T13 (HornerCodec correctness depends on these helpers).
**Inviolable invariants asserted:** SPEC-01 T1-T7 (net invariants — preserved across helper calls).

---

## Scope

Per Round 2 closure SC-013 / Q5: `wire_add_into` and `wire_mul_into` (PortRef-based, `pub(crate)`) **already exist** in `relativist-core/src/encoding/arithmetic.rs:92,224` (introduced for SPEC-09 R17d `church_sum_of_squares`). Phase 3a is **promotion-and-validation**, NOT new construction. This TEST-SPEC adds the **direct-helper coverage** that R13a' demands — separate from the `build_add` / `build_mul` round-trips already tested at `arithmetic.rs:737, 745`.

Three obligations from R13a' are tested:
1. **T1-T7 preservation** — net validates before AND after the helper call.
2. **Reduction equivalence** — `m_port` rooted at Church(m), `n_port` at Church(n) → result reduces to Church(m+n) (resp. Church(m·n)).
3. **Privacy** — `wire_add_into` / `wire_mul_into` are NOT in the `relativist-core::encoding` public re-export list; external integration test that imports them MUST fail to compile.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0711-01 | unit (in-module) | none | `relativist-core/src/encoding/arithmetic.rs` | ~25 |
| UT-0711-02 | unit (in-module) | none | same | ~35 |
| UT-0711-03 | unit (in-module) | none | same | ~25 |
| UT-0711-04 | unit (in-module) | none | same | ~35 |
| CT-0711-05 | compile-fail doc-test | none | rustdoc on helpers in same file | ~10 |

## Test floor delta (from TASK-0711 acceptance criteria)

- default: **+4** → ≥ 1833 (compile-fail doc-test does NOT count toward `cargo test --lib` floor; it runs as a doc-test)
- zero-copy: **+4** → ≥ 1877
- streaming-no-recycle: **+4** → ≥ 1824
- release: **+4** → ≥ 1775

(CT-0711-05 contributes to `cargo test --doc` only; it is not part of the unit-test floor.)

---

## Unit Tests

### UT-0711-01: `wire_add_into_preserves_t1_t7_for_distinct_subnets`

**Purpose:** Obligation 1 — T1-T7 preserved across `wire_add_into`. Use `validate_encoded_net` before AND after the helper call.

**Preconditions:** Build a single net containing two Church-numeral sub-nets (via `encode_church_into(net, m)` then `encode_church_into(net, n)`); both sub-nets are well-formed (T1-T7 hold) before the helper call.

**Input:**
```rust
let mut net = Net::new();
let m_id = encode_church_into(&mut net, 7);
let n_id = encode_church_into(&mut net, 9);

// Pre-condition: net is valid.
validate_encoded_net(&net).expect("pre-call net must satisfy T1-T7");

let m_port = PortRef::AgentPort(m_id, 0);
let n_port = PortRef::AgentPort(n_id, 0);
let _result_id = wire_add_into(&mut net, m_port, n_port);

// Post-condition: net is still valid.
validate_encoded_net(&net).expect("post-call net must satisfy T1-T7");
```

**Expected output:** Both `validate_encoded_net` calls return `Ok(())`.

**Edge cases:**
- (EC-1) `m == 0` and `n == 0`: helper still preserves T1-T7.
- (EC-2) `m_port` and `n_port` chosen at the principal port slot 0 (mandatory per R13a' — Church numeral roots are AgentPort(_, 0)).

---

### UT-0711-02: `wire_add_into_reduces_to_church_sum_for_5_pairs`

**Purpose:** Obligation 2 — `wire_add_into` produces a sub-net that reduces to `Church(m + n)`. 5 distinct pairs from TASK-0711 brief.

**Input:** For each `(m, n) ∈ {(0,0), (1,1), (7,9), (0,5), (5,0)}` with expected sum `0, 2, 16, 5, 5`:
```rust
let mut net = Net::new();
let m_id = encode_church_into(&mut net, m);
let n_id = encode_church_into(&mut net, n);
let m_port = PortRef::AgentPort(m_id, 0);
let n_port = PortRef::AgentPort(n_id, 0);
let result_id = wire_add_into(&mut net, m_port, n_port);
net.set_root(PortRef::AgentPort(result_id, 0));

reduce_all(&mut net);
let value = decode_nat(&net).expect("decodable Church numeral after reduction");
```

**Expected output:** `value == m + n` for every test pair.

**Edge cases:**
- (EC-1) `(0, 0)` (additive identity, both zero) — explicit pair.
- (EC-2) Asymmetric `(0, 5)` AND `(5, 0)` — must both yield 5; verifies commutativity-via-construction.
- (EC-3) `(7, 9)` — generic non-trivial.
- (EC-4) `(1, 1)` — smallest non-zero (regression check on Church-1 wiring).

---

### UT-0711-03: `wire_mul_into_preserves_t1_t7_for_distinct_subnets`

**Purpose:** Obligation 1 — T1-T7 preserved across `wire_mul_into`. Symmetric to UT-0711-01.

**Input:**
```rust
let mut net = Net::new();
let m_id = encode_church_into(&mut net, 3);
let n_id = encode_church_into(&mut net, 4);
validate_encoded_net(&net).expect("pre-call");

let m_port = PortRef::AgentPort(m_id, 0);
let n_port = PortRef::AgentPort(n_id, 0);
let _result_id = wire_mul_into(&mut net, m_port, n_port);

validate_encoded_net(&net).expect("post-call");
```

**Expected output:** Both `validate_encoded_net` return `Ok(())`.

**Edge cases:**
- (EC-1) `m_port` and `n_port` are AgentPort slot-0 (Church root convention).
- (EC-2) Order `wire_mul_into(net, n, m)` vs `(m, n)` MUST both preserve T1-T7 (test optionally swaps args in a separate iteration).

---

### UT-0711-04: `wire_mul_into_reduces_to_church_product_for_5_pairs`

**Purpose:** Obligation 2 — `wire_mul_into` produces a sub-net that reduces to `Church(m * n)`. 5 distinct pairs from TASK-0711 brief.

**Input:** For each `(m, n) ∈ {(0,0), (1,1), (3,4), (0,7), (7,0)}` with expected product `0, 1, 12, 0, 0`:
```rust
let mut net = Net::new();
let m_id = encode_church_into(&mut net, m);
let n_id = encode_church_into(&mut net, n);
let m_port = PortRef::AgentPort(m_id, 0);
let n_port = PortRef::AgentPort(n_id, 0);
let result_id = wire_mul_into(&mut net, m_port, n_port);
net.set_root(PortRef::AgentPort(result_id, 0));

reduce_all(&mut net);
let value = decode_nat(&net).expect("decodable Church numeral after reduction");
```

**Expected output:** `value == m * n` for every test pair.

**Edge cases:**
- (EC-1) `(0, 0)` — zero × zero = 0.
- (EC-2) Both asymmetric zero pairs `(0, 7)` AND `(7, 0)` — both yield 0; checks zero-absorption symmetry.
- (EC-3) `(3, 4) = 12` — generic non-trivial.
- (EC-4) `(1, 1) = 1` — multiplicative identity.

---

### CT-0711-05: `wire_helpers_are_pub_crate_only` (compile-fail doc-test)

**Purpose:** Obligation 3 — `wire_add_into` / `wire_mul_into` are NOT publicly exported. Compile-fail doc-test on the helper rustdoc.

**Input (in rustdoc on `wire_add_into` and `wire_mul_into`):**
```rust
/// ```compile_fail
/// // SPEC-27 v3 R13a' privacy obligation: must NOT compile externally.
/// use relativist_core::encoding::arithmetic::wire_add_into;
/// fn _f() {
///     let mut net = relativist_core::net::Net::new();
///     let _ = wire_add_into;
/// }
/// ```
```

**Expected output:** `cargo test --doc` reports the doc-test as `compile_fail` PASS (i.e., the snippet failed to compile, which is the success criterion for `compile_fail`).

**Edge cases:**
- (EC-1) Verify the doc-test name is unique across the crate to avoid name collisions.
- (EC-2) MUST also test `wire_mul_into` privacy in a parallel doc-test (so privacy holds for both helpers symmetrically).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | T1-T7 holds before `wire_add_into` | `validate_encoded_net` Ok | UT-0711-01 |
| EC-002 | T1-T7 holds AFTER `wire_add_into` (composability) | `validate_encoded_net` Ok | UT-0711-01 |
| EC-003 | `(0,0)` add — both operands zero | reduces to Church(0) | UT-0711-02 |
| EC-004 | `(0, n)` and `(n, 0)` add | both reduce to Church(n) | UT-0711-02 |
| EC-005 | T1-T7 holds before/after `wire_mul_into` | `validate_encoded_net` Ok twice | UT-0711-03 |
| EC-006 | `(0, n)` mul | reduces to Church(0) (zero absorption) | UT-0711-04 |
| EC-007 | `(1, 1)` mul | reduces to Church(1) (identity) | UT-0711-04 |
| EC-008 | External crate import fails to compile | doc-test `compile_fail` passes | CT-0711-05 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T6, T7, T11, T13 | These HornerCodec tests rely on `wire_*_into` correctness; UT-0711-{01..04} provide the foundation. |

## Notes

- This task adds **zero production LoC** (helpers already exist; only rustdoc citations to R13a' / R17d are added). Stage 4 reviewer agent confirms by inspection.
- The existing `test_wire_add_into_port_based_preserves_build_add` at `arithmetic.rs:737` and `test_wire_mul_into_port_based_preserves_build_mul` at line 745 confirm helper-vs-`build_*` parity. These NEW tests (UT-0711-{01..04}) confirm helper behavior **directly** (R13a' obligation set).
- Compile-fail doc-test (CT-0711-05) is in `cargo test --doc`, NOT `cargo test --lib`. Its delta is +1 doc-test per helper (=2 total) but does NOT contribute to the unit-test floor.
- If Stage 4 reviewer or Stage 5 QA finds the existing helpers fail obligation 1 or 2, developer files a follow-up task to add new helpers under different names — SPEC-27 v3 explicitly does NOT amend SPEC-14 in either case.
- Test floor delta: **+4 unit tests** (default + features); 2 doc-tests in `cargo test --doc`.
