# TEST-SPEC-TASK-0715: Tests for TASK-0715 — HornerCodec decoder + `Codec` impl + T11 / T13

**Task:** TASK-0715
**Spec:** SPEC-27 v3
**Bundle:** D-015 (SPEC-27 Encoder/Decoder API + HornerCodec — Topic 2)
**Requirements covered:** R3 (HornerCodec implements `Codec`), R14' (decoder uses `decode_biguint`), R15' (output schema `{value, bit_length}`), R16' (decode-side edge cases)
**Test IDs (from SPEC-27 v3 §7.3):** T7 (decode side), T8 (decode side), T9, T9b, T10 (NotNormalForm row), T11 (property test ≥100 valid + ≥30 negative), T13 (distributed equivalence — empirical demonstration of ARG-001 G1).
**Inviolable invariants asserted:** ARG-001 G1 (Fundamental Property — `seq_value == inproc_value` for `W ∈ {2,4,8}`); R4 NotNormalForm semantics tied to SPEC-01 I4 (via TASK-0709 helper); SPEC-04 R25 fallback to centralized partition for non-decomposable codecs.

---

## Scope

This task closes Phase 3c of SPEC-27 v3 §6 — the v1 codec is complete after this. It contains the **most consequential** test of the bundle: T13, the empirical demonstration of ARG-001 G1 (Fundamental Property) for HornerCodec.

### Round 2 closure decisions honored
- **SC-007:** Negative cross-check (`EncodeError` family vs `OracleError` family on the same input) is part of T11 with ≥30 cases.
- **SC-009:** R13' rationale and T13 cite **G1 (Fundamental Property)** with **P1** as engine + **P3 + P4** as distribution-side preconditions, NOT P3 alone.
- **SC-010:** T13 in-process MUST for `cargo test`; Docker TCP SHOULD `#[ignore]` permitted; partition strategy = round-robin per SPEC-07 R3 default; decoder stage explicit.

### Phase 3c deliverables
1. `impl Decoder for HornerCodec` — wraps `decode_biguint` (TASK-0712) and serializes to R15' schema.
2. `impl Codec for HornerCodec { fn description(&self) -> &str }`.
3. T11 property test (≥100 valid + ≥30 negative).
4. T9 / T9b BigUint-range pipeline tests.
5. T13 distributed equivalence test (in-process MUST; Docker TCP SHOULD).

## Test category & location

| # | Category | Cfg gating | File | LoC |
|---|----------|-----------|------|-----|
| UT-0715-01 | unit (in-module) | none | `relativist-core/src/encoding/horner.rs` | ~25 |
| UT-0715-02 | unit (in-module) | none | same | ~25 |
| UT-0715-03 | unit (in-module) | none | same | ~30 |
| UT-0715-04 | unit (in-module) | none | same | ~30 |
| UT-0715-05 | unit (in-module) | none | same | ~25 |
| PT-0715-06 | property test (proptest) | none | same | ~50 |
| PT-0715-07 | property test (proptest) | none | same | ~40 |
| IT-0715-08 | integration (in-process distributed) | none | `relativist-core/tests/horner_distributed_g1.rs` | ~80 |
| IT-0715-09 | integration (Docker TCP) | `#[ignore]` (CI-only) | `relativist-core/tests/horner_distributed_tcp.rs` | ~80 |

## Test floor delta (from TASK-0715 acceptance criteria)

- default: **+8** (5 unit + 2 proptest + 1 integration in-process; `#[ignore]` Docker test does NOT count toward `cargo test` default floor) → ≥ 1867
- zero-copy: **+8** → ≥ 1911
- streaming-no-recycle: **+8** → ≥ 1858
- release: **+8** → ≥ 1809

(IT-0715-09 is `#[ignore]` for `cargo test`; runs in CI integration suite via cicd agent follow-up.)

---

## Unit Tests

### UT-0715-01: `horner_decode_canonical_case_matches_oracle` (T7 decode side)

**Purpose:** End-to-end pipeline `encode → reduce_all → decode`. Output schema MUST be `{"value": "<base-10>", "bit_length": <usize>}`.

**Input:**
```rust
let codec = HornerCodec::new();
let coeffs = [3u64, 2, 5, 1];
let x = 2u64;
let expected = horner_serial(&coeffs, x).unwrap();

let net = codec.encode(br#"{"coeffs":[3,2,5,1],"x":2}"#).unwrap();
let mut n = net; reduce_all(&mut n);
let out = codec.decode(&n).unwrap();
```

**Expected output:**
```rust
assert_eq!(out["value"].as_str().unwrap(), expected.to_string());
assert_eq!(out["value"].as_str().unwrap(), "35");
assert_eq!(out["bit_length"].as_u64().unwrap() as usize, expected.bits() as usize);
assert_eq!(out["bit_length"].as_u64().unwrap(), 6);  // 35 = 0b100011 → 6 bits
```

**Edge cases:**
- (EC-1) JSON object has exactly two top-level keys: `value` and `bit_length` (extra keys would indicate schema drift).
- (EC-2) `value` is a string (NOT integer); `bit_length` is a non-negative integer.

---

### UT-0715-02: `horner_decode_sparse_coefficients_match_oracle` (T8 decode side)

**Purpose:** Sparse case `[1, 0, 0, 0, 0, 1] @ 10 = 100_001`.

**Input:**
```rust
let codec = HornerCodec::new();
let net = codec.encode(br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#).unwrap();
let mut n = net; reduce_all(&mut n);
let out = codec.decode(&n).unwrap();
```

**Expected output:**
```rust
let expected = horner_serial(&[1,0,0,0,0,1], 10).unwrap();
assert_eq!(out["value"].as_str().unwrap(), expected.to_string());
assert_eq!(out["value"].as_str().unwrap(), "100001");
assert_eq!(out["bit_length"].as_u64().unwrap() as usize, expected.bits() as usize);
```

**Edge cases:**
- (EC-1) `bit_length` for `100_001 = 0b11000011010100001` = 17 bits (assert).
- (EC-2) `BigUint::from(0u64).bits() == 0` — for `[0;k] @ x`, `bit_length == 0`.

---

### UT-0715-03: `horner_pipeline_biguint_range_25_coeffs` (T9)

**Purpose:** SC-006 closure — verifies `bit_length > 64` (strictly exceeds u64 range) AND exact equality to `horner_serial`.

**Input:**
```rust
let codec = HornerCodec::new();
let coeffs = vec![1u64; 25];
let json = br#"{"coeffs":[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1],"x":10}"#;

let expected = horner_serial(&coeffs, 10).unwrap();

let net = codec.encode(json).unwrap();
let mut n = net; reduce_all(&mut n);
let out = codec.decode(&n).unwrap();
```

**Expected output:**
```rust
let value = out["value"].as_str().unwrap();
let bit_length = out["bit_length"].as_u64().unwrap() as usize;
assert_eq!(value, expected.to_string());
assert_eq!(value, "1111111111111111111111111");  // 25 '1's
assert!(bit_length > 64, "T9 BigUint witness: bit_length must strictly exceed u64");
assert_eq!(bit_length, expected.bits() as usize);
```

**Edge cases:**
- (EC-1) Reduction time: this test may take several seconds; document expected wall-clock as informative (not asserted).
- (EC-2) Memory: 25-coeff Horner constructs ~25 mul + 25 add scaffolds + 26 Church(_) sub-nets. Exact agent count is not asserted (implementation-dependent), but SHOULD be tractable on dev hardware.

---

### UT-0715-04: `horner_pipeline_boundary_max_inputs` (T9b)

**Purpose:** SC-006 — boundary value `10_000` for both coeff and x, AND BigUint range. Single test exercises both R16' boundary AND R14' BigUint readback.

**Input:**
```rust
let codec = HornerCodec::new();
let coeffs = [10_000u64, 10_000, 10_000, 10_000, 10_000];
let expected = horner_serial(&coeffs, 10_000).unwrap();

let net = codec.encode(br#"{"coeffs":[10000,10000,10000,10000,10000],"x":10000}"#).unwrap();
let mut n = net; reduce_all(&mut n);
let out = codec.decode(&n).unwrap();
```

**Expected output:**
```rust
assert_eq!(out["value"].as_str().unwrap(), expected.to_string());
assert!(expected.bits() > 64);  // sanity that the test exercises BigUint range
assert_eq!(out["bit_length"].as_u64().unwrap() as usize, expected.bits() as usize);
```

**Edge cases:**
- (EC-1) Reduction time: this test exercises the Church(10_000) construction (the largest Church-numeral cap allowed by SPEC-14 R4) plus 4 mul + 4 add scaffolds. Document expected wall-clock as informative.
- (EC-2) `expected.bits() > 64` — direct BigUint witness via the oracle; the codec must produce a value with the same bit count.

---

### UT-0715-05: `horner_decode_rejects_non_nf` (R4 + R14')

**Purpose:** R14' E1 — decoder rejects non-NF nets via `count_valid_active_pairs` (TASK-0709). Returns `DecodeError::NotNormalForm { redexes }` with valid-pair count, NOT raw `redex_queue.len()`.

**Input:**
```rust
let mut net = Net::new();
let a = net.add_agent(Symbol::Con);
let b = net.add_agent(Symbol::Con);
net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
net.redex_queue.push_back((a, b));
// Note: net.set_root(...) intentionally not set OR set to a placeholder; the decoder MUST reject on NotNormalForm BEFORE structural checks.

let codec = HornerCodec::new();
let result = codec.decode(&net);
```

**Expected output:**
```rust
match result {
    Err(DecodeError::NotNormalForm { redexes }) => assert_eq!(redexes, 1),
    other => panic!("expected NotNormalForm{{1}}, got {:?}", other),
}
```

**Edge cases:**
- (EC-1) Stale-only queue: queue has one entry but agent has been removed → `redexes == 0` → decoder DOES NOT return NotNormalForm; it proceeds to structural decode.
- (EC-2) Multiple valid redexes: `redexes` reports the actual count.

---

## Property Tests

### PT-0715-06: `horner_property_test_oracle_agreement` (T11 positive, ≥100 cases)

**Property:** For randomly sampled `(coeffs, x)` within SPEC-14 R4 caps, `decode(reduce_all(encode((coeffs, x)))).value == horner_serial(coeffs, x).unwrap().to_string()`.

**Generator strategy:**
```text
arb_coeffs: Vec<u64> with len in [1, 15], each element in [0, 10_000]
arb_x: u64 in [0, 10_000]
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]
    #[test]
    fn horner_property_test_oracle_agreement(
        coeffs in proptest::collection::vec(0u64..=10_000, 1..=15),
        x in 0u64..=10_000,
    ) {
        let codec = HornerCodec::new();
        let expected = horner_serial(&coeffs, x).unwrap();

        let json = serde_json::to_vec(&serde_json::json!({"coeffs": coeffs, "x": x})).unwrap();
        let net = codec.encode(&json).unwrap();
        let mut n = net; reduce_all(&mut n);
        let out = codec.decode(&n).unwrap();

        prop_assert_eq!(out["value"].as_str().unwrap(), expected.to_string());
        prop_assert_eq!(out["bit_length"].as_u64().unwrap() as usize, expected.bits() as usize);
    }
}
```

**Sample size:** ≥100 internal cases (per T11).

**Shrinking note:** On failure, proptest minimizes both `coeffs` (toward shorter, smaller values) and `x` (toward 0). The minimal counterexample identifies which encoder construction or reducer rule diverges from the oracle. Common failure shapes:
- Coefficient ordering bug (R11' violated): minimal `(coeffs, x)` distinguishes `coeffs[0]` vs `coeffs[len-1]` semantics.
- Mul-by-zero reducer bug: minimal counterexample has `0` somewhere in `coeffs` or `x == 0`.
- Boundary handling: minimal counterexample has a `10_000` or large coeff.

**Boundary cases (auto-included via proptest):**
- `coeffs.len() == 1` (constant polynomial path).
- `x == 0`.
- `coeffs == [0; k]` (all zero).
- `coeffs[i] == 10_000` (boundary).

---

### PT-0715-07: `horner_property_test_negative_cross_check` (T11 negative, ≥30 cases)

**Property:** For randomly sampled out-of-range inputs, encoder returns `EncodeError::InvalidInput` AND oracle returns matching `OracleError` family. Closes SC-007 negative cross-check.

**Generator strategy:**
```text
arb_negative_input: union of three families:
  (a) empty coeffs: coeffs = [], any x
  (b) coefficient overflow: coeffs has at least one element > 10_000
  (c) x overflow: x > 10_000 (regardless of coeffs validity)
```

**Assertion:**
```rust
proptest! {
    #![proptest_config(ProptestConfig { cases: 30, .. ProptestConfig::default() })]
    #[test]
    fn horner_property_test_negative_cross_check(
        case in arb_negative_horner_input(),
    ) {
        let codec = HornerCodec::new();
        let (coeffs, x) = case.unpack();

        let json = serde_json::to_vec(&serde_json::json!({"coeffs": coeffs, "x": x})).unwrap();
        let codec_result = codec.encode(&json);
        let oracle_result = horner_serial(&coeffs, x);

        // Both MUST be Err.
        prop_assert!(codec_result.is_err(), "encoder MUST reject out-of-range input");
        prop_assert!(oracle_result.is_err(), "oracle MUST reject out-of-range input");

        // Family matching: empty coeffs ⇔ EmptyCoeffs; coeff overflow ⇔ CoefficientOverflow; x overflow ⇔ XOverflow.
        match (codec_result, oracle_result) {
            (Err(EncodeError::InvalidInput(msg)), Err(OracleError::EmptyCoeffs)) => {
                prop_assert!(msg.to_lowercase().contains("empty"));
            }
            (Err(EncodeError::InvalidInput(msg)), Err(OracleError::CoefficientOverflow { idx, value, max })) => {
                prop_assert!(msg.contains(&idx.to_string()) || msg.contains(&value.to_string()) || msg.to_lowercase().contains("coeff"));
                prop_assert_eq!(max, 10_000);
            }
            (Err(EncodeError::InvalidInput(msg)), Err(OracleError::XOverflow { value, max })) => {
                prop_assert!(msg.contains(&value.to_string()) || msg.to_lowercase().contains("x"));
                prop_assert_eq!(max, 10_000);
            }
            (codec_err, oracle_err) => prop_assert!(false, "mismatched error families: {:?} vs {:?}", codec_err, oracle_err),
        }
    }
}
```

**Sample size:** ≥30 internal cases (per T11 negative).

**Note on check-order coupling:** When multiple violations apply simultaneously (e.g., empty coeffs AND `x > 10_000`), both encoder and oracle MUST report the **first** violation per the documented order: empty → coeff overflow → x overflow. The test uses a strict family-match arm, so divergence in check order surfaces as `prop_assert!(false, ...)` failure with both error values printed.

---

## Integration Tests

### IT-0715-08: `horner_distributed_g1_in_process` (T13 in-process MUST)

**Purpose:** Empirical demonstration of **ARG-001 G1 (Fundamental Property)**: for any terminating net, sequential `reduce_all` and distributed `run_grid` produce isomorphic Normal Forms. T13 specializes to HornerCodec by asserting **decoded values** are equal.

**Round 2 closure honored:**
- **SC-009:** Cite **G1 with P1 as engine + P3 + P4 as distribution-side preconditions** (NOT P3 alone).
- **SC-010:** In-process MUST for `cargo test`; partition strategy = round-robin per SPEC-07 R3 default; HornerCodec is NOT a `RecipeEncoder` so SPEC-04 R25 fallback applies (coordinator generates full net + partitions centrally); decoder stage explicit (decoding occurs on coordinator's merged net per NG5).

**Preconditions:** `run_grid(net, num_workers, partition_strategy)` from `relativist-core::merge` is available; HornerCodec is registered (TASK-0716 dependency).

**Inputs:** Five Horner cases (T6, T7, T8, T9, T9b).

**Algorithm:**
```rust
fn t13_distributed_pipeline(input_json: &[u8], workers: u32) -> serde_json::Value {
    let codec = HornerCodec::new();
    let net = codec.encode(input_json).unwrap();

    // SPEC-04 R25 fallback: coordinator generates full net, partitions centrally,
    // ships partitions to W workers via in-process channel. Round-robin partition
    // per SPEC-07 R3 default --strategy round-robin.
    let merged_net = run_grid(net, workers, PartitionStrategy::RoundRobin);

    // NG5: decoding always happens on coordinator after merge.
    codec.decode(&merged_net).unwrap()
}

#[test]
fn horner_distributed_g1_in_process() {
    let inputs: &[(&str, &[u8])] = &[
        ("T6 [1;5] @ 2",       br#"{"coeffs":[1,1,1,1,1],"x":2}"#),
        ("T7 [3,2,5,1] @ 2",   br#"{"coeffs":[3,2,5,1],"x":2}"#),
        ("T8 [1,0,0,0,0,1] @ 10", br#"{"coeffs":[1,0,0,0,0,1],"x":10}"#),
        ("T9 [1;25] @ 10",      &gen_t9_json()),
        ("T9b [10000;5] @ 10000", br#"{"coeffs":[10000,10000,10000,10000,10000],"x":10000}"#),
    ];

    for (label, input) in inputs {
        // Sequential baseline (MUST per T13 spec).
        let codec = HornerCodec::new();
        let mut net_seq = codec.encode(input).unwrap();
        reduce_all(&mut net_seq);
        let seq_value = codec.decode(&net_seq).unwrap();

        // In-process distributed (MUST for cargo test) for W ∈ {2, 4, 8}.
        for &w in &[2u32, 4, 8] {
            let inproc_value = t13_distributed_pipeline(input, w);
            assert_eq!(
                inproc_value, seq_value,
                "G1 violation [{}] W={}: seq={:?} != inproc={:?}",
                label, w, seq_value, inproc_value
            );
        }
    }
}
```

**Expected output:** All `assert_eq!` succeed. ARG-001 G1 holds empirically for HornerCodec.

**Edge cases:**
- (EC-1) `W = 1` (degenerate single-worker grid) — value MUST equal seq_value (trivial; technically not tested but follows by construction).
- (EC-2) `W > num_agents`: implementation may reject or normalize; out of scope for T13.
- (EC-3) Round-robin partition with skewed agent distribution: still produces same NF (G1).
- (EC-4) Partition strategy other than round-robin (e.g., random): still produces same NF (P3 + P4 are partition-strategy-agnostic). Optional secondary test if implementation exposes alternate strategies.

**Failure interpretation:** If this test fails, ONE of P1, P3, P4 is violated:
- P1 violation: confluence broken — same input reduces to different NFs. Highly unlikely; would indicate reducer bug.
- P3 violation: border redex completeness broken — distribution misses cross-partition redexes.
- P4 violation: ID consistency broken — merge step produces malformed net.

---

### IT-0715-09: `horner_distributed_g1_docker_tcp` (T13 Docker SHOULD `#[ignore]`)

**Purpose:** Same as IT-0715-08 but via the Docker Compose `docker-local` deploy target (SPEC-07 §3.6) — exercises the wire protocol end-to-end.

**Cfg gating:** `#[ignore]` (per SC-010: MUST run in CI integration suite via cicd agent follow-up; MAY be skipped by default `cargo test` to keep runtime bounded).

**Preconditions:**
- Docker Compose `docker-local` available.
- Coordinator + W workers spun up via the standard recipe.
- HornerCodec registered in worker registry (TASK-0719 R28).

**Inputs:** Same five inputs as IT-0715-08.

**Algorithm:** Same as IT-0715-08 but `run_grid` replaced with the equivalent end-to-end TCP pipeline (coordinator binds, workers connect, partitions ship via wire, results merge, decoded on coordinator).

**Expected output:** `tcp_value == seq_value` for all 5 inputs and W ∈ {2, 4, 8}.

**Edge cases:**
- (EC-1) Network jitter / packet reorder: NF MUST still match (P3 + P4 robust to ordering).
- (EC-2) Worker disconnect mid-job: out of scope for T13 (fault tolerance is ARG-006 territory).
- (EC-3) Wire protocol version mismatch: test fails fast with explicit error message.

**Cfg attribute:**
```rust
#[test]
#[ignore = "T13 Docker TCP — SC-010: SHOULD run in CI integration suite, not default cargo test"]
fn horner_distributed_g1_docker_tcp() { /* ... */ }
```

---

## Edge Cases Catalog

| # | Scenario | Expected Behavior | Test |
|---|----------|-------------------|------|
| EC-001 | Canonical T7 case | `value="35"`, `bit_length=6` | UT-0715-01 |
| EC-002 | Sparse T8 case | `value="100001"`, `bit_length=17` | UT-0715-02 |
| EC-003 | T9 BigUint witness | `bit_length > 64` | UT-0715-03 |
| EC-004 | T9b boundary BigUint | `value` = oracle, `bit_length > 64` | UT-0715-04 |
| EC-005 | NotNormalForm with valid redex | `redexes == 1` (not raw queue len) | UT-0715-05 |
| EC-006 | NotNormalForm with stale-only queue | Decodes successfully | UT-0715-05 EC-1 |
| EC-007 | Property: 100 valid (coeffs, x) cases agree with oracle | All agree | PT-0715-06 |
| EC-008 | Property: 30 negative cases — error families match | All match | PT-0715-07 |
| EC-009 | T13 in-process G1 W∈{2,4,8} for 5 inputs | seq_value == inproc_value | IT-0715-08 |
| EC-010 | T13 Docker TCP G1 W∈{2,4,8} for 5 inputs | seq_value == tcp_value | IT-0715-09 |
| EC-011 | Output schema has exactly 2 keys | `value`, `bit_length` only | UT-0715-01 EC-1 |
| EC-012 | `value` is base-10 string, NOT integer | Type assertion via serde_json | UT-0715-01 EC-2 |

## Mapping to SPEC-27 v3 §7

| SPEC-27 v3 Test ID | Coverage |
|---|---|
| T7 (canonical decode) | UT-0715-01 |
| T8 (sparse decode) | UT-0715-02 |
| T9 (BigUint range, 25 coeffs) | UT-0715-03 |
| T9b (boundary BigUint) | UT-0715-04 |
| T10 (decode NotNormalForm row) | UT-0715-05 |
| T11 (property test ≥100 valid + ≥30 negative) | PT-0715-06 + PT-0715-07 |
| T13 (in-process MUST + Docker SHOULD) | IT-0715-08 + IT-0715-09 |

## Dependencies Context

- `decode_biguint` from TASK-0712.
- `horner_serial` and `OracleError` from TASK-0713.
- `HornerCodec` and its `Encoder` impl from TASK-0714.
- `run_grid(net, workers, partition_strategy) -> Net` from `relativist-core::merge` (existing).
- `PartitionStrategy::RoundRobin` from SPEC-04 R25 / SPEC-07 R3.
- `Codec`, `Decoder` traits from `traits.rs`.
- `count_valid_active_pairs` from TASK-0709 (used inside `decode_biguint`).
- `proptest` (existing in `[dev-dependencies]`).
- `serde_json::json!` (existing).

## Notes

- **T13 is the empirical demonstration of ARG-001 G1 for HornerCodec** — the central scientific outcome of this bundle. Failure of T13 is a critical scientific finding (would falsify the v1 codec's claim of grid-correctness) and MUST trigger an immediate sdd-pipeline halt and root-cause investigation (P1, P3, or P4 violation).
- **T13 is NOT the only G1 demonstration in Relativist** — DPC's existing distributed reduction tests also empirically exercise G1. T13 is HornerCodec-specific: the first user-defined Codec-level G1 illustration.
- **HornerCodec is NOT a `RecipeEncoder`** (Q4): the coordinator generates the full net centrally and partitions via SPEC-04 R25 fallback. TASK-0719 verifies this fallback path explicitly via T23.
- After this task lands, `HornerCodec` satisfies `Codec` and is registrable via `EncoderRegistry`; TASK-0716 performs that registration.
- Test floor delta: **+8** total (5 unit + 2 proptest + 1 in-process integration; Docker TCP is `#[ignore]`).
