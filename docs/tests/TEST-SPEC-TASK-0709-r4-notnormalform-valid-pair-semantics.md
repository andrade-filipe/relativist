# TEST-SPEC-TASK-0709: Tests for TASK-0709 — `NotNormalForm.redexes` valid-pair semantics + I4 prune helper

**Task:** TASK-0709
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R4 (NotNormalForm semantics tied to SPEC-01 I4 — SC-005), R5, R6
**Test IDs (from SPEC-27 v3 §7.1):** Indirectly supports T1, T2 (decode contract validation); the helper itself is foundational and has UT-only coverage.
**Inviolable invariants asserted:** SPEC-01 I4 (stale-entry pruning of `redex_queue`), T1-T7 (net invariants).

---

## Scope

This task adds the canonical helper `count_valid_active_pairs(net: &Net) -> usize` that prunes stale `redex_queue` entries per SPEC-01 I4 (an entry `(a, b)` is valid iff both `a` and `b` are live agents AND their principal ports are mutually connected at port 0). It is a **prerequisite** for TASK-0712 (`decode_biguint`) and TASK-0715 (HornerCodec decoder) — both must use this helper to populate `DecodeError::NotNormalForm.redexes` rather than `net.redex_queue.len()`.

Decision per Round 2 closure SC-005: NotNormalForm is "valid active pairs after stale pruning per SPEC-01 I4", NOT `redex_queue.len()`. This TEST-SPEC enforces the distinction.

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0709-01 | unit (in-module) | none | `relativist-core/src/reduction/mod.rs` (or wherever helper lives) | ~25 |
| UT-0709-02 | unit (in-module) | none | same | ~20 |
| UT-0709-03 | unit (in-module) | none | same | ~25 |
| UT-0709-04 | unit (in-module) | none | same | ~25 |
| UT-0709-05 | unit (in-module) | none | same | ~30 |

## Test floor delta (from TASK-0709 acceptance criteria)

- default: **+5** → ≥ 1824
- zero-copy: **+5** → ≥ 1868
- streaming-no-recycle: **+5** → ≥ 1815
- release: **+5** → ≥ 1766

---

## Unit Tests

### UT-0709-01: `count_valid_pairs_zero_on_empty_queue`

**Purpose:** Sanity check — a net with `redex_queue.len() == 0` must return 0.

**Preconditions:** Fresh `Net::new()` with no agents added.

**Input:**
```rust
let net = Net::new();
assert!(net.redex_queue.is_empty());
let n = count_valid_active_pairs(&net);
```

**Expected output:** `n == 0`.

**Edge cases:**
- (EC-1) Default-constructed `Net` has empty `redex_queue` regardless of platform.

---

### UT-0709-02: `count_valid_pairs_includes_live_redex`

**Purpose:** Verify a fresh net containing exactly one true active pair (two live agents connected principal-to-principal) returns 1.

**Preconditions:** Build a net with two CON agents A, B; wire `AgentPort(A, 0) <-> AgentPort(B, 0)`; push `(A, B)` to `redex_queue`.

**Input:**
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Con);
let b = net.add_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.redex_queue.push_back((a, b));

let n = count_valid_active_pairs(&net);
```

**Expected output:** `n == 1`.

**Edge cases:**
- (EC-1) Order of `(a, b)` in queue (i.e., `(b, a)` instead) MUST also count as valid — the helper is order-agnostic.
- (EC-2) Symbol pair (Con/Con vs Con/Dup vs Era/Era) does NOT affect validity — any two principal ports connected mutually count as one redex.

---

### UT-0709-03: `count_valid_pairs_excludes_stale_after_remove_agent`

**Purpose:** Verify the helper prunes stale entries — the canonical SC-005 case. After one of the two agents in a queued redex is removed (made non-live), the helper MUST return 0 even though `redex_queue.len() == 1`.

**Preconditions:** Net with one queued redex (UT-0709-02 setup), then mark agent `a` as not-live (or remove it).

**Input:**
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Con);
let b = net.add_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.redex_queue.push_back((a, b));

// Make `a` no longer live (mechanism is implementation-dependent; e.g.,
// `net.remove_agent(a)` if available, or mark via SparseNet free-list).
net.remove_agent(a);

let n_pruned = count_valid_active_pairs(&net);
let n_raw    = net.redex_queue.len();
```

**Expected output:**
- `n_pruned == 0`.
- `n_raw == 1` (the queue itself is NOT mutated by the helper — pruning is read-only / functional).

**Edge cases:**
- (EC-1) Both agents removed: same result, `n_pruned == 0`.
- (EC-2) Only `b` removed instead of `a`: `n_pruned == 0` (symmetric).
- (EC-3) Helper MUST NOT mutate `net.redex_queue` (assert via the `n_raw` check above; this is the read-only contract).

---

### UT-0709-04: `count_valid_pairs_excludes_stale_after_disconnect`

**Purpose:** Verify pruning when agents are still live but their principal ports are no longer mutually connected (they were rewired by some prior reduction).

**Preconditions:** Two agents queued as a redex, but rewire `AgentPort(a, 0)` to a third agent's port instead of `AgentPort(b, 0)`.

**Input:**
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Con);
let b = net.add_agent(Symbol::Con);
let c = net.add_agent(Symbol::Era);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(c, 0));
// Note: `b.p0` is NOT connected to `a.p0` anymore — stale.
net.redex_queue.push_back((a, b));

let n = count_valid_active_pairs(&net);
```

**Expected output:** `n == 0` (live agents, but principals no longer mutually connected → stale entry).

**Edge cases:**
- (EC-1) `a.p0 <-> c.p0` AND `b.p0` connected somewhere else (e.g., to a free port) → still stale; `n == 0`.
- (EC-2) Re-pushing `(a, c)` (the new genuine redex) MUST yield `n == 1` — the helper handles `redex_queue` containing both stale `(a, b)` and live `(a, c)` correctly.

---

### UT-0709-05: `count_valid_pairs_zero_on_normal_form`

**Purpose:** End-to-end sanity — reduce a small net to NF via `reduce_all`, then assert helper returns 0.

**Preconditions:** A net constructed via `build_add(2, 3)` (or any small Church-arithmetic net) reduced via `reduce_all`.

**Input:**
```rust
let mut net = build_add(2, 3);
reduce_all(&mut net);
let n = count_valid_active_pairs(&net);
```

**Expected output:** `n == 0` (NF reached; no valid active pairs remain).

**Edge cases:**
- (EC-1) `build_add(0, 0)` → also `n == 0` after reduction.
- (EC-2) `build_mul(7, 9)` → `n == 0` after reduction.
- (EC-3) **Critical:** if `reduce_all` leaves stale entries in `redex_queue` (which is permitted by SPEC-01 I4 — pruning is the consumer's responsibility), the helper MUST still return 0. Optionally assert `net.redex_queue.len() >= 0` is consistent (informational).

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Empty net (0 agents, 0 queue entries) | Return 0 | UT-0709-01 |
| EC-002 | Single live redex queued | Return 1 | UT-0709-02 |
| EC-003 | Queued redex but one agent removed | Return 0 (stale) | UT-0709-03 |
| EC-004 | Queued redex but principals no longer mutually connected | Return 0 (stale) | UT-0709-04 |
| EC-005 | Net reduced to NF via `reduce_all` | Return 0 | UT-0709-05 |
| EC-006 | Mixed queue (one stale + one live) | Return 1 (just the live one) | UT-0709-04 EC-2 |
| EC-007 | Helper does NOT mutate `redex_queue` | `redex_queue.len()` unchanged after call | UT-0709-03 |
| EC-008 | Order `(a, b)` vs `(b, a)` in queue | Both count as same redex (deduped if helper dedups; otherwise count both as valid) | UT-0709-02 EC-1 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T1 (encode contract validation catches invalid nets) | Uses `validate_encoded_net` not this helper directly; helper is consumed by T2 path indirectly |
| T2 (encode contract validation catches empty nets — 0 redexes) | Uses redex count via standard detector; this helper is the canonical detector for the **decode** side |

## Notes

- This helper is the foundation of R4 / SC-005 closure. Without it, every decoder downstream (TASK-0712, TASK-0715) is at risk of false-positive `NotNormalForm` errors when distributed merges leave stale queue entries.
- The helper MUST be `pub(crate)` (R13a' privacy convention; HornerCodec lives in the same crate).
- Implementation MAY reuse the existing valid-redex detector used by `reduce_all` (developer choice).
- The `Decoder` trait error variant signature (`#[error("net is not in normal form (has {redexes} valid active pair(s))")]`) is unchanged; this task only adds the helper that populates `redexes`.
- Test floor delta is **+5** unit tests, in-module on the helper's home file.
