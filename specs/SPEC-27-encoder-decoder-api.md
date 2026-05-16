# SPEC-27: Encoder/Decoder Trait API and Problem Registry

**Status:** Revised v3 (Round 2 spec-critic response, 2026-05-06)
**Depends on:** SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-06 (Wire Protocol), SPEC-14 (Encoding), SPEC-25 (Recipe-Based Generation), SPEC-26 (GUI Application — workspace restructure R1-R7 only)
**ROADMAP items:** 2.41 (Encoder/Decoder API and Problem Registry)
**References consumed:** REF-005 (Mackie & Pinto 2002, Theorems 5.2 and 6.2: encoding linear logic with ICs — referenced for future LambdaCodec, §5.1), REF-002 (Lafont 1997: universality)
**Arguments consumed:** ARG-001 (P1-P6 framework: confluence preserves determinism — HornerCodec is the v1 empirical illustration of G1, with P1 as the engine and P3+P4 as distribution-side preconditions, see R13' rationale)
**Briefings consumed:** BRIEF-20260415-disc012-job-submission (6 reference systems, encode→reduce→decode contract, HVM compatibility analysis)
**Discussions consumed:** DISC-012 v2 (Job Submission, Encoding, and Decoding — 8 options analyzed, 2-round adversarial debate, Layers 0-3 selected for v2)
**Handoffs consumed:** `docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md` (Topic 2 alignment, v2 revision brief)
**Design docs consumed:** `docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md`, `docs/superpowers/specs/2026-05-06-horner-method-explainer.md`
**Spec-critic reviews consumed:** `docs/spec-reviews/SPEC-27-round1-critic.md` (Round 1, NEEDS REVISION — 4 HIGH + 7 MEDIUM + 2 LOW), `docs/spec-reviews/SPEC-27-v2-round2-response.md` (Round 2 closure log)

**Revision history:**
- v1 (initial Draft): full proposal including LambdaCodec POC.
- v2 (Topic 2 alignment, 2026-05-06): LambdaCodec demoted to §5 Future Work; HornerCodec promoted to v1 codec; default_registry updated; supersedes earlier R10-R16 with R10'-R16b' for Horner; NG2 generalized; §5 Future Work added. See `docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md`.
- v3 (Round 2 spec-critic response, 2026-05-06): addresses 13 issues from `docs/spec-reviews/SPEC-27-round1-critic.md`. §2 expanded with six locally-defined terms (SC-001); R12' citation chain made explicit (SC-002); §2 ChurchArithmeticCodec and R8/R19 cross-references repaired (SC-003); R14' BigUint readback algorithm inlined (SC-004); R4 NotNormalForm semantics tied to SPEC-01 I4 (SC-005); T9 polynomial widened so the result truly exceeds `u64::MAX` and a new T9b boundary case added (SC-006); R16a' oracle returns `Result<BigUint, OracleError>` mirroring encoder bounds (SC-007); R21 reformulated with explicit clap `conflicts_with` mechanics (SC-008); R13' rationale re-anchored to G1 + P1 + P3 + P4 instead of P3 alone, T13 retargeted accordingly (SC-009); T13 in-process/Docker MUST/SHOULD split with explicit decoder-stage protocol (SC-010); §5.1 LambdaCodec sketch tagged informative (SC-011); §6 Phase 3 split into 3a/3b/3c (SC-012); new R13a' specifies `wire_add_into` / `wire_mul_into` PortRef-based composable helpers required by R13' pseudocode (SC-013, Caminho A — SPEC-14 NOT amended; helpers live as `pub(crate)` in `relativist-core::encoding::arithmetic` and are reused by HornerCodec in the same crate). R1-R9, R17-R28 unchanged.

---

## 1. Purpose

This spec defines the Encoder/Decoder trait API that enables third-party problem encodings without forking the Relativist codebase. It covers three concerns:

1. **Trait definitions** for `Encoder`, `Decoder`, `Codec`, and `RecipeEncoder` — the extension points that domain-specific code implements.
2. **Encoder Registry** — a runtime-discoverable collection of named encoders, selectable via CLI.
3. **HornerCodec** — the v1 codec for distributed polynomial evaluation via Horner's method, composed entirely on top of SPEC-14's Church arithmetic primitives (`build_add`, `build_mul`) and the composable helpers `wire_add_into` / `wire_mul_into` (R13a'). HornerCodec serves as the empirical illustration of ARG-001 G1 (the Fundamental Property: for any terminating net, sequential `reduce_all` and distributed `run_grid` produce isomorphic Normal Forms), with P1 (strong confluence) as the engine and P3 + P4 as distribution-side preconditions: a classically sequential algorithm executed correctly across an arbitrary number of distributed workers (cf. R13' rationale).

The current Relativist has exactly one end-to-end pipeline: Church numeral arithmetic (`encoding/church.rs` + `encoding/arithmetic.rs`, ~500 LoC). This spec wraps that pipeline as a `ChurchArithmeticCodec`, adds HornerCodec as a second built-in codec, and makes the extension mechanism explicit so that future encoders are plug-and-play.

**No IC-theoretic invariants are changed.** Encoders produce standard IC nets (CON/DUP/ERA, 3 ports per agent). The reducer is unchanged. This spec is entirely about the API boundary between "problem domain" and "IC infrastructure".

**Prerequisite:** SPEC-26 R1-R7 (Cargo workspace restructure) MUST be completed first. The traits are defined in the `relativist-core` library crate, which only exists after the workspace split.

**Future codec work** (LambdaCodec via Mackie/Pinto and other candidate codecs) is documented in §5 Future Work; it is intentionally out of v1 scope.

---

## 2. Definitions

Terms defined in SPEC-00, SPEC-01, SPEC-02, SPEC-14, and SPEC-25 are used without redefinition. Terms introduced in this spec are tagged **(Relativist, this spec)** to indicate that SPEC-27 is the canonical source until and unless they are absorbed into SPEC-00 §8b or §8c by a future amendment (out of scope of this revision; see Round 2 closure log SC-001).

| Term | Definition |
|------|-----------|
| **Encoder** | **(Relativist, this spec)** A Rust trait (R1) that converts a domain-specific problem description (opaque JSON bytes) into an IC net with redexes. Distinct from SPEC-00 §8b.2 *Encoding* (the activity); SPEC-27's `Encoder` is the trait type. The net, when reduced to Normal Form, encodes the solution. |
| **Decoder** | **(Relativist, this spec)** A Rust trait (R2) that interprets an IC net in Normal Form and extracts a domain-specific answer (`serde_json::Value`). Distinct from SPEC-00 §8b.3 *Decoding (Readback)* (the activity); SPEC-27's `Decoder` is the trait type. The decoder is the semantic inverse of its paired encoder. |
| **Codec** | **(Relativist, this spec)** A Rust trait (R3) that combines `Encoder` + `Decoder` for a single problem domain. Codecs are the primary unit of registration in the `EncoderRegistry`. Object-safe (`dyn Codec` is permitted). |
| **EncoderRegistry** | **(Relativist, this spec)** A runtime collection of named Codecs (R17-R20). The CLI dispatches to a Codec by name. Static registration only (no plugin loading; NG4). |
| **HornerCodec** | **(Relativist, this spec)** The v1 Codec implemented in `relativist-core::encoding::horner` (R10') that encodes a polynomial-evaluation problem `(coeffs, x)` as an IC net via Horner's recurrence, composed on top of `wire_add_into` and `wire_mul_into` (R13a') over Church numeral sub-nets created by `encode_church_into` (SPEC-14 R4b), and decodes the resulting Normal Form Church numeral as a `BigUint` (R14'). Empirical illustration of ARG-001 G1 — see R13' rationale. |
| **ChurchArithmeticCodec** | **(Relativist, this spec)** The Codec that wraps the `compute` CLI subcommand pipeline (SPEC-14 §3.6 R22-R25), which dispatches over `build_add` / `build_mul` / `build_exp` (SPEC-14 R15-R17) and `build_sum_of_squares` (which exists in `relativist-core::encoding::arithmetic` per SPEC-09 R17d but is not a SPEC-14-listed primitive). Backward-compatible with the legacy positional `compute add 3 5` form (R21 fallback). |
| **RecipeEncoder** | **(Relativist, this spec)** A Rust trait (R24) that extends Encoder by adding `make_recipes` and `generate_partition` for distributed generation. Generalizes SPEC-25's built-in recipe generation to user-defined encoders. NOT object-safe (associated type `Recipe`); registered separately from `Codec` (Q1). |
| **Encode Contract** | **(Relativist, this spec)** The validation invariants (E1-E2, R5) that every encoder output must satisfy before reduction: E1 — net satisfies T1-T7 of SPEC-01; E2 — net has at least one redex. Enforced centrally by the registry (R18 `encode_and_validate`). |
| **Horner's method** | **(Relativist, this spec)** The recurrence `p(x) = (((a_n · x + a_{n-1}) · x + a_{n-2}) · x + ... + a_1) · x + a_0`, which evaluates a degree-n polynomial in `n` multiplications and `n` additions. **Coefficient ordering convention:** `coeffs[0]` is the constant term `a_0`; `coeffs[len-1]` is the leading coefficient `a_n` (R11', §3.4). Classically sequential (each step depends on the previous accumulator). Used here as the canonical empirical demonstration that confluence enables correct distributed reduction of an inherently sequential algorithm (R13' rationale). |
| **BigUint** | **(Relativist, this spec)** The arbitrary-precision unsigned integer type `num_bigint::BigUint` from the `num-bigint` crate (version `^0.4`, MIT/Apache-2.0 license). Used for HornerCodec's decode output to avoid overflow when `p(x) > u64::MAX`. **Bit-length semantics:** `BigUint::bits()` returns the number of bits needed to represent the value in base 2 (e.g., `BigUint::from(0u64).bits() == 0`; `BigUint::from(u64::MAX).bits() == 64`). |
| **BigUint readback** | **(Relativist, this spec)** The decoding algorithm of R14' that traverses a Church numeral IC net in Normal Form and returns its value as a `BigUint` instead of `u64`. Implemented in `relativist-core::encoding::biguint_readback` (R16b'). Structurally identical to SPEC-14 §4.4 `decode_nat` but with a `BigUint` accumulator; see R14' algorithm block. |
| **NotNormalForm (operational)** | **(Relativist, this spec)** The decoder error returned when the net presented to `Decoder::decode` has at least one valid (non-stale per SPEC-01 I4) active pair in `redex_queue`. Operational refinement of SPEC-00 §5.5 *Normal Form*: a net with a non-empty `redex_queue` is NOT necessarily out of Normal Form (queue may contain stale entries per I4); the decoder must distinguish the two cases (R4 + R14'). |

---

## 3. Requirements

### 3.1 Trait Definitions

**R1.** The `Encoder` trait MUST be defined in `relativist-core::encoding::traits` with the following signature:

```rust
pub trait Encoder: Send + Sync {
    /// Human-readable name (e.g., "church_add", "horner").
    fn name(&self) -> &str;

    /// Encode a problem (JSON bytes) into an IC net with redexes.
    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError>;
}
```

The `input` parameter is opaque JSON bytes. Each encoder defines its own input schema. **(MUST)**

**R2.** The `Decoder` trait MUST be defined alongside `Encoder`:

```rust
pub trait Decoder: Send + Sync {
    /// Decode an IC net in normal form into a JSON-serializable answer.
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError>;
}
```

The return type is `serde_json::Value` for maximum flexibility. **(MUST)**

**R3.** The `Codec` trait MUST combine `Encoder` and `Decoder`:

```rust
pub trait Codec: Encoder + Decoder {
    /// Short description of what this codec encodes/decodes.
    fn description(&self) -> &str;
}
```

All built-in encoders MUST implement `Codec`. **(MUST)**

**R4.** Error types MUST be defined:

```rust
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("encoding produced invalid net: {0}")]
    InvalidNet(String),
    #[error("input too large: {size} exceeds limit {limit}")]
    InputTooLarge { size: usize, limit: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("net is not in normal form (has {redexes} valid active pair(s))")]
    NotNormalForm { redexes: usize },
    #[error("unrecognized net structure: {0}")]
    UnrecognizedStructure(String),
    #[error("decode failed: {0}")]
    DecodeFailed(String),
}
```

**Semantics of `NotNormalForm.redexes`.** The `redexes` field MUST report the count of **valid** active pairs in the net — i.e., active pairs that survive stale-entry pruning per SPEC-01 I4 (the redex queue may contain stale entries that no longer correspond to live active pairs after reductions). Concretely, the decoder MUST NOT emit `NotNormalForm` solely because `net.redex_queue.len() > 0`; it MUST first prune stale entries (or use the standard valid-redex detector) and emit the error only if at least one valid active pair remains. This avoids false positives in the distributed pipeline (T13) where a freshly merged net's queue may carry stale entries from cross-partition merges (SPEC-05) that are invariants-compatible with Normal Form. Implementers MAY reuse the same valid-redex-detection helper used by `reduce_all` (SPEC-03). **(MUST)**

### 3.2 Encode Contract (Validation)

**R5.** Every `Encoder::encode()` output MUST be validated before reduction. Validation checks:
- **E1.** Net satisfies T1-T7 from SPEC-01 (valid net invariants).
- **E2.** Net has at least one redex (otherwise there is nothing to reduce).

Validation MUST be performed by the registry (R12), not by individual encoders. **(MUST)**

**R6.** If validation fails, the error MUST include which invariant was violated and a human-readable description. **(MUST)**

### 3.3 Church Numeral Codec (Refactoring)

**R7.** The existing Church numeral encoding (`encoding/church.rs`, `encoding/arithmetic.rs`) MUST be refactored to implement the `Codec` trait. The refactoring MUST NOT change any existing **SPEC-14 R3 public function signatures** (`encode_nat`, `decode_nat`, `build_add`, `build_mul`, `build_exp` — and `build_sum_of_squares` per SPEC-09 R17d). The codec layer adds (a) new `Codec` trait `impl`s on top of the existing primitives and (b) a new JSON-dispatch surface (R8); both are additive and do not modify SPEC-14's exports. **(MUST)**

**R8.** The `ChurchArithmeticCodec` MUST support the following input schema:

```json
{
  "op": "add" | "mul" | "exp" | "sum_of_squares",
  "a": <u64>,
  "b": <u64>       // optional for sum_of_squares
}
```

**Operand semantics:**
- For `op = "add"` or `op = "mul"`: `a` and `b` are the two operands; the codec invokes `build_add(a, b)` / `build_mul(a, b)` (SPEC-14 R15-R16).
- For `op = "exp"`: `a` is the **base** and `b` is the **exponent**, matching SPEC-14 R17 ordering `build_exp(base, exp) -> Net` (i.e., the codec invokes `build_exp(a, b)` so the result is `a^b`).
- For `op = "sum_of_squares"`: `a` is the upper bound `n`; `b` is ignored (`b` MAY be omitted in the JSON object, and SHOULD be omitted in normative usage). The codec invokes `build_sum_of_squares(a)` (defined in `relativist-core::encoding::arithmetic` per SPEC-09 R17d), which produces `1^2 + 2^2 + ... + n^2`. `build_sum_of_squares` is *not* a SPEC-14 R3 export — it is a SPEC-09-derived helper that lives alongside `build_add`/`build_mul`/`build_exp` in the same module.

The output schema MUST be:

```json
{
  "result": <u64>,
  "interactions": <u64>
}
```

**(MUST)**

**R9.** All 690 existing tests (v1 floor) MUST pass after the Church refactoring. Zero behavioral changes to any SPEC-14 public function. **(MUST)**

### 3.4 Horner Polynomial Codec

The `HornerCodec` is the v1 codec illustrating ARG-001 G1 empirically (with P1 as
the engine and P3+P4 as distribution-side preconditions): a classically sequential
algorithm (Horner's recurrence) executed correctly under distributed reduction
with arbitrary worker count `W` and arbitrary BSP scheduling. It composes
exclusively on top of SPEC-14's Church arithmetic via two `pub(crate)` helpers
(`wire_add_into`, `wire_mul_into`) declared in R13a' — SPEC-14's public R3
export list is NOT modified.

**R10' (substitutes R10).** A `HornerCodec` MUST be implemented in `relativist-core::encoding::horner` that encodes polynomial-evaluation problems using Horner's method as IC nets, composed on top of (a) `encode_church_into` (SPEC-14 R4b) for Church numeral construction inside an existing net, and (b) `wire_add_into` / `wire_mul_into` (R13a' below) for incremental composition of additions and multiplications inside a shared net. **(MUST)**

**R11' (substitutes R11).** The `HornerCodec` encoder MUST accept the following input schema:

```json
{
  "coeffs": [<u64>, <u64>, ..., <u64>],
  "x": <u64>
}
```

Where `coeffs[i]` represents the coefficient `a_i` of `x^i`, with `coeffs[0]` being the constant term and `coeffs[coeffs.len() - 1]` being the leading coefficient. **(MUST)**

**R12' (substitutes R12).** The encoder MUST reject inputs that would exceed the SPEC-14 caps on `encode_church_into(net, n)`. SPEC-14 R4 caps `encode_nat(n)` at `n <= 10_000`, and SPEC-14 R4b extends the same cap (10_000) to `encode_church_into(net, n)` since the latter shares the construction logic. Each value `v` in `coeffs ∪ {x}` is passed to `encode_church_into` (transitively, via `wire_add_into` / `wire_mul_into` operands as well as direct calls in R13'); exceeding the cap triggers a panic in `encode_church_into`, which the encoder MUST trap before it happens.

Concretely, the encoder MUST validate, **before any call to `encode_church_into`**:

- `coeffs.len() >= 1` (no empty coefficient list — see R16').
- For each `v` in `coeffs ∪ {x}`: `v <= 10_000`.

Violations MUST return `EncodeError::InvalidInput` with a descriptive message identifying which bound was violated. The cap value `10_000` is dynamic: if SPEC-14 R4 / R4b ever raise the cap, this requirement automatically inherits the new value via the citation chain (the implementer MUST NOT hardcode `10_000` as a literal divorced from the SPEC-14 R4 cap; a single shared constant `relativist_core::encoding::church::MAX_CHURCH_NAT` SHOULD be the single source of truth). **(MUST)**

**R13a' (new — composable arithmetic helpers).** The HornerCodec implementation depends on **composable** variants of `build_add` and `build_mul` that operate on an existing net (mirroring SPEC-14 R4b's `encode_church_into`). SPEC-14 R15-R16 expose only the non-composable signatures `build_add(a: u64, b: u64) -> Net` and `build_mul(a: u64, b: u64) -> Net`, which create a fresh `Net` and accept `u64` arguments — these are unsuitable for composing intermediate accumulators across many Horner iterations.

The SPEC-27 v3 baseline assumption is that the `relativist-core::encoding::arithmetic` module (the same module that hosts `build_add` / `build_mul` per SPEC-14) provides two `pub(crate)` helpers with the following signatures:

```rust
/// Wire `m_port` and `n_port` (the principal ports of two Church-numeral
/// sub-nets already present in `net`) through the `add` combinator construction
/// of SPEC-14 §4.3.1, returning the AgentId of the resulting sub-net's root
/// (the outer lambda of the addition's closure).
pub(crate) fn wire_add_into(
    net: &mut Net,
    m_port: PortRef,
    n_port: PortRef,
) -> AgentId;

/// Wire `m_port` and `n_port` (the principal ports of two Church-numeral
/// sub-nets already present in `net`) through the `mul` combinator construction
/// of SPEC-14 §4.3.2, returning the AgentId of the resulting sub-net's root.
pub(crate) fn wire_mul_into(
    net: &mut Net,
    m_port: PortRef,
    n_port: PortRef,
) -> AgentId;
```

These helpers MUST satisfy the following obligations:

- **Invariant preservation:** If `net` satisfies T1-T7 of SPEC-01 before the call, and if `m_port` and `n_port` are the principal ports of two Church-numeral sub-nets already in `net` (in their own Normal Forms), then `net` MUST satisfy T1-T7 after the call. The proof argument is the same as SPEC-14 §4.3.1 / §4.3.2 for `build_add` / `build_mul`, with composition replacing the `Net::new` step.
- **Reduction equivalence:** For `m_port` and `n_port` rooted at Church numeral encodings of `m` and `n` respectively, the resulting sub-net MUST reduce (under SPEC-03 `reduce_all`, applied either to the whole net or to any prefix that includes the helper's added agents) to the Church encoding of `m + n` (resp. `m * n`).
- **Privacy:** The helpers are `pub(crate)` (internal to `relativist-core`). They are NOT part of SPEC-14's public R3 export list. HornerCodec, residing in the same crate (`relativist-core::encoding::horner`), can call them directly. If a future codec is implemented in a separate crate, SPEC-14 will be amended (separate task, out of scope of SPEC-27 v3) to expose `wire_add_into` / `wire_mul_into` as part of its public surface.

This requirement closes Round 1 SC-013 (the original R13' pseudocode invoked `build_add(net, acc, x_node)` with an AgentId-based composable signature that did not exist in SPEC-14). The PortRef-based signatures above are the canonical ones; AgentIds are converted to PortRefs at call sites via `PortRef::AgentPort(id, 0)` (the principal port of a Church numeral root). **(MUST)**

**R13' (substitutes R13).** The encoder MUST construct the IC net by composing `wire_add_into` and `wire_mul_into` (R13a') following Horner's recurrence. Let `n = coeffs.len() - 1`. The construction proceeds as follows (pseudocode):

```text
let mut net = Net::new();
let acc_id = encode_church_into(&mut net, coeffs[n]);   // SPEC-14 R4b
let mut acc_port = PortRef::AgentPort(acc_id, 0);

for k in (0..n).rev() {                                  // k = n-1, n-2, ..., 0
    // 1. Encode a fresh Church(x) inside the same net.
    let x_id = encode_church_into(&mut net, x);          // SPEC-14 R4b
    let x_port = PortRef::AgentPort(x_id, 0);

    // 2. Multiply the current accumulator by x:  prod = acc * x.
    let prod_id = wire_mul_into(&mut net, acc_port, x_port); // R13a'
    let prod_port = PortRef::AgentPort(prod_id, 0);

    // 3. Encode a fresh Church(coeffs[k]).
    let coef_id = encode_church_into(&mut net, coeffs[k]);
    let coef_port = PortRef::AgentPort(coef_id, 0);

    // 4. Add the coefficient:  acc' = prod + coeffs[k].
    let new_acc_id = wire_add_into(&mut net, prod_port, coef_port); // R13a'
    acc_port = PortRef::AgentPort(new_acc_id, 0);
}

net.set_root(acc_port);
```

The resulting net, when reduced to Normal Form via SPEC-03 `reduce_all` (or any distributed equivalent that respects the BSP cycle of SPEC-05), MUST produce a Church numeral whose decoded value equals `p(x) = sum(coeffs[i] * x^i for i in 0..=n)`. **(MUST)**

**Empirical illustration of ARG-001 G1 (rationale, informative).** This requirement is the v1 empirical illustration of ARG-001's central thesis (G1, the Fundamental Property): for any terminating net, sequential `reduce_all` and distributed `run_grid` produce isomorphic Normal Forms. **P1** (strong confluence, Lafont's Proposition 1, REF-002 p. 73) is the engine that guarantees the value is invariant under reduction order. **P3** (border redex completeness) and **P4** (ID consistency) are the distribution-side preconditions that lift P1's local guarantee to the BSP grid pipeline (SPEC-05). T13 (§7.3) specifically targets G1 by asserting `seq_value == inproc_value == tcp_value` across `W ∈ {2, 4, 8}` workers.

Round 1 SC-009 (this round 2 response) replaced the previous "P3 alone" framing with the correct G1 + P1 + P3 + P4 framing.

**R14' (substitutes R14 — BigUint readback algorithm).** The decoder MUST implement BigUint readback as follows. The algorithm MUST mirror SPEC-14 §4.4 `decode_nat` exactly in topology and traversal, replacing the `count: u64` accumulator with `count: BigUint`.

**Pseudocode (normative for control flow; informative for syntax):**

```rust
use num_bigint::BigUint;
use crate::net::{DISCONNECTED, Net, PortRef, Symbol};

pub fn decode_biguint(net: &Net) -> Result<BigUint, DecodeError> {
    // E1. Validate Normal Form per R4 semantics (valid active pairs only,
    // not stale queue entries — see R4 NotNormalForm.redexes definition).
    let valid_redexes = count_valid_active_pairs(net);  // helper: prune I4-stale entries
    if valid_redexes > 0 {
        return Err(DecodeError::NotNormalForm { redexes: valid_redexes });
    }

    // E2. Find outer lambda (lambda f) from net.root.
    let root = net.root.ok_or_else(|| DecodeError::DecodeFailed("no root".into()))?;
    let lam_f = match root {
        PortRef::AgentPort(id, 0) => id,
        _ => return Err(DecodeError::UnrecognizedStructure("root not an AgentPort(_, 0)".into())),
    };
    let lam_f_agent = get_agent(net, lam_f)
        .ok_or_else(|| DecodeError::UnrecognizedStructure("lambda_f missing".into()))?;
    if lam_f_agent.symbol != Symbol::Con {
        return Err(DecodeError::UnrecognizedStructure("lambda_f not CON".into()));
    }

    // E3. Find inner lambda (lambda x).
    let lam_f_p2 = net.get_target(PortRef::AgentPort(lam_f, 2));
    if lam_f_p2 == DISCONNECTED {
        return Err(DecodeError::UnrecognizedStructure("lambda_f.p2 disconnected".into()));
    }
    let lam_x = match lam_f_p2 {
        PortRef::AgentPort(id, 0) => id,
        _ => return Err(DecodeError::UnrecognizedStructure("lambda_f.p2 not AgentPort(_,0)".into())),
    };
    let lam_x_agent = get_agent(net, lam_x)
        .ok_or_else(|| DecodeError::UnrecognizedStructure("lambda_x missing".into()))?;
    if lam_x_agent.symbol != Symbol::Con {
        return Err(DecodeError::UnrecognizedStructure("lambda_x not CON".into()));
    }

    // E4. Detect the n=0 case.
    // Church(0): lambda_f.p1 connects to ERA(p0); lambda_x.p1 self-loops with lambda_x.p2.
    let f_target = net.get_target(PortRef::AgentPort(lam_f, 1));
    let x_bind   = net.get_target(PortRef::AgentPort(lam_x, 1));
    let x_body   = net.get_target(PortRef::AgentPort(lam_x, 2));
    if f_target == DISCONNECTED || x_bind == DISCONNECTED || x_body == DISCONNECTED {
        return Err(DecodeError::UnrecognizedStructure("malformed Church frame".into()));
    }
    if x_bind == PortRef::AgentPort(lam_x, 2) && x_body == PortRef::AgentPort(lam_x, 1) {
        // Self-loop on auxiliaries; verify ERA on lambda_f.p1.
        if let PortRef::AgentPort(era_id, 0) = f_target {
            let era_agent = get_agent(net, era_id)
                .ok_or_else(|| DecodeError::UnrecognizedStructure("era agent missing".into()))?;
            if era_agent.symbol == Symbol::Era {
                return Ok(BigUint::from(0u64));
            }
        }
        return Err(DecodeError::UnrecognizedStructure("Church(0) frame missing ERA".into()));
    }

    // E5. Walk the application chain from lambda_x.p2.
    // Each application is a CON agent; we count one BigUint increment per application.
    let mut count: BigUint = BigUint::from(0u64);
    let one: BigUint = BigUint::from(1u64);
    let mut current = PortRef::AgentPort(lam_x, 2);

    loop {
        let target = net.get_target(current);
        if target == DISCONNECTED {
            return Err(DecodeError::UnrecognizedStructure("application chain broken".into()));
        }
        match target {
            PortRef::AgentPort(app_id, 2) => {
                let agent = get_agent(net, app_id)
                    .ok_or_else(|| DecodeError::UnrecognizedStructure("app agent missing".into()))?;
                if agent.symbol != Symbol::Con {
                    return Err(DecodeError::UnrecognizedStructure("non-CON in app chain".into()));
                }
                count += &one;
                current = PortRef::AgentPort(app_id, 1);
            }
            PortRef::AgentPort(id, port) if id == lam_x && port == 1 => {
                // Reached x variable binding — end of chain.
                break;
            }
            _ => return Err(DecodeError::UnrecognizedStructure("unexpected port in chain".into())),
        }
    }

    Ok(count)
}
```

**Independence from `decode_nat`.** `decode_biguint` MUST be a **standalone** implementation, not a wrapper over `decode_nat` (otherwise the cross-check in R16b' is tautological). The two functions share only their structural traversal logic; the accumulator type and final return are different (`u64` vs `BigUint`). The shared traversal logic MAY be factored into a private generic helper (e.g., `walk_church<A: Counter>(net: &Net) -> Result<A, DecodeError>` with `Counter` implementations for `u64` and `BigUint`), in which case both `decode_nat` and `decode_biguint` instantiate that helper — but the helper itself MUST live in `relativist-core::encoding::biguint_readback` (R16b') so that the cross-check (R16b') compares two independent type instantiations rather than the same code path.

**Decoder return shape.** The `Decoder::decode` impl on `HornerCodec` returns `serde_json::Value` per R2; internally it calls `decode_biguint` and serializes per R15' (base-10 string + bit length).

**(MUST)**

**R15' (substitutes R15).** The output schema for `HornerCodec` MUST be:

```json
{
  "value": "<base-10 BigUint string>",
  "bit_length": <usize>
}
```

`bit_length` is the bit-length of the resulting BigUint per `BigUint::bits()` semantics defined in §2: the number of bits needed to represent the value in base 2 (e.g., `BigUint::from(0u64).bits() == 0`; `BigUint::from(1u64).bits() == 1`; `BigUint::from(u64::MAX).bits() == 64`). **(MUST)**

**R16' (substitutes R16).** The `HornerCodec` MUST handle the following edge cases correctly:

- **Empty coeffs:** `coeffs.len() == 0` → `EncodeError::InvalidInput("empty coeffs")`.
- **Constant polynomial:** `coeffs.len() == 1` → encoder skips the Horner loop; the resulting net is just `encode_church_into(net, coeffs[0])` plus `set_root`; the decoded result is `coeffs[0]`.
- **Evaluation at zero:** `x == 0` → result is `coeffs[0]` (mathematically correct; the reducer obtains this via mul-by-zero collapsing to zero and add-with-zero acting as identity).
- **All-zero coefficients:** `coeffs == [0, 0, ..., 0]` → result is `0`.
- **Maximum coefficient:** any `coeffs[i] == 10_000` MUST be accepted (boundary inclusive per R12'/SPEC-14 R4).
- **Coefficient overflow:** any `coeffs[i] > 10_000` MUST return `EncodeError::InvalidInput`.
- **Maximum x:** `x == 10_000` MUST be accepted (boundary inclusive per R12'/SPEC-14 R4).
- **x overflow:** `x > 10_000` MUST return `EncodeError::InvalidInput`.

**(MUST)**

**R16a' (new — pure-Rust oracle, signature with explicit error model).** A pure-Rust oracle function MUST be exposed for testing purposes with the following signature:

```rust
use num_bigint::BigUint;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OracleError {
    #[error("empty coeffs")]
    EmptyCoeffs,
    #[error("coefficient at index {idx} = {value} exceeds cap (max {max})")]
    CoefficientOverflow { idx: usize, value: u64, max: u64 },
    #[error("x = {value} exceeds cap (max {max})")]
    XOverflow { value: u64, max: u64 },
}

pub fn horner_serial(coeffs: &[u64], x: u64) -> Result<BigUint, OracleError>;
```

The oracle MUST enforce **the same input bounds as the encoder (R12')**: `coeffs.len() >= 1`, `coeffs[i] <= 10_000` for every `i`, and `x <= 10_000`. Violations MUST return the matching `OracleError` variant. Valid inputs MUST return `Ok(value)` where `value = sum(coeffs[i] * x^i for i in 0..coeffs.len())` computed via a straight-line `BigUint` accumulator loop (no IC reduction). The cap value MUST be sourced from the same `MAX_CHURCH_NAT` constant used by R12' (single source of truth).

Property tests (T11) MUST sample inputs from the **valid** range and assert
`horner_serial(c, x).unwrap() == decode(reduce_all(encode((c, x))))` on agreement,
and MUST also assert that the oracle and the codec produce **the same `EncodeError` / `OracleError` family** on the same out-of-range input (negative cross-check). **(MUST)**

**R16b' (new — BigUint readback module + decode_nat cross-check).** The BigUint readback function MUST live in `relativist-core::encoding::biguint_readback` and MUST be cross-checked against SPEC-14's `decode_nat` for nets whose decoded value fits in `u64`:

```rust
// Property (T12 in §7.3):
// for any net N produced by encode_nat(n) with n <= u64::MAX,
//     decode_biguint(N) == Ok(BigUint::from(decode_nat(N).unwrap()))
```

The cross-check is meaningful only because `decode_biguint` and `decode_nat` are independent code paths (R14' "Independence from `decode_nat`" clause). If a shared `walk_church<Counter>` helper is used, the property test instantiates the helper twice — once for `u64` (matching `decode_nat`) and once for `BigUint` (matching `decode_biguint`) — and compares the results.

This invariant MUST be tested as a property test over the range of `decode_nat`-decodable nets (T12). **(MUST)**

### 3.5 Encoder Registry

**R17.** An `EncoderRegistry` MUST be defined in `relativist-core::encoding::registry`:

```rust
pub struct EncoderRegistry {
    codecs: HashMap<String, Box<dyn Codec>>,
}
```

**(MUST)**

**R18.** The registry MUST support the following operations:

```rust
impl EncoderRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, codec: Box<dyn Codec>) -> Result<(), RegistryError>;
    pub fn get(&self, name: &str) -> Option<&dyn Codec>;
    pub fn list(&self) -> Vec<(&str, &str)>; // (name, description)
    pub fn encode_and_validate(&self, name: &str, input: &[u8]) -> Result<Net, Error>;
    pub fn decode(&self, name: &str, net: &Net) -> Result<serde_json::Value, Error>;
}
```

`encode_and_validate` MUST call `encode()` then validate the net (R5). **(MUST)**

**R19.** A `default_registry()` function MUST return a registry pre-populated with:
- `"church_add"` — Church numeral addition (R7-R9, ChurchArithmeticCodec)
- `"church_mul"` — Church numeral multiplication (R7-R9, ChurchArithmeticCodec)
- `"church_exp"` — Church numeral exponentiation (R7-R9, ChurchArithmeticCodec)
- `"church_sum_of_squares"` — Sum of squares (R7-R9, ChurchArithmeticCodec)
- `"horner"` — Polynomial evaluation via Horner's method (R10'-R16b', HornerCodec)

`"lambda"` MUST NOT appear in the default registry; it is documented as future work in §5.1. **(MUST)**

**R20.** Attempting to register a codec with a name that already exists MUST return `RegistryError::DuplicateName`. **(MUST)**

### 3.6 CLI Integration

**R21 (reformulated v3 — explicit clap mechanics).** The `compute` subcommand MUST accept the codec name via either `--encoder <name>` or `--codec <name>`. The dual naming reflects the two terminologies common in the IC literature: "encoder" emphasizes the SPEC-27 R1 trait name; "codec" emphasizes the symmetric encode+decode pair (R3) and is the term used in the Topic 2 design doc. **(MUST)**

The implementation MUST use the **`conflicts_with`** clap pattern, NOT the `aliases(...)` pattern (round 1 SC-008 documented that `aliases(...)` makes two names refer to a single argument and silently keeps the last value when both are passed, which does not satisfy the mutual-exclusion requirement). The canonical pattern is:

```rust
#[derive(clap::Args)]
pub struct ComputeFlags {
    /// Codec name (preferred, matches SPEC-27 R1 trait name).
    #[arg(long = "encoder", value_name = "NAME", conflicts_with = "codec")]
    pub encoder: Option<String>,

    /// Codec name (alternate spelling; same registry entry as --encoder).
    #[arg(long = "codec", value_name = "NAME", conflicts_with = "encoder")]
    pub codec: Option<String>,
    // ... --input <json>, etc.
}
```

After parsing, the application logic MUST coalesce the two `Option<String>` fields into a single `Option<String>` codec name (e.g., `flags.encoder.or(flags.codec)`). The clap-generated help output MUST list both `--encoder` and `--codec` as separate flag entries (not as a single flag with aliases), so the user discovers both spellings; the help text for `--codec` SHOULD reference `--encoder` as the primary name (e.g., `"alternate spelling of --encoder; mutually exclusive"`). **(MUST)**

**Behavior matrix:**

| Invocation | Result |
|------------|--------|
| `--encoder horner --input '<json>'` | OK — codec = `"horner"` |
| `--codec horner --input '<json>'` | OK — codec = `"horner"`, identical pipeline |
| `--encoder horner --codec horner` | clap conflict error (T20) |
| (neither flag, positional fallback) | Legacy positional `compute add 3 5` form preserved (R21 fallback) |

If neither `--encoder` nor `--codec` is given, the current behavior (positional args for Church arithmetic, e.g. `compute add 3 5`) MUST be preserved for backward compatibility (R7 / SPEC-14 R22-R25 invariant). **(MUST)**

**Pattern note for SPEC-07.** SPEC-27 R21 introduces the "dual-form flag" CLI pattern (two clap arguments with `conflicts_with` cross-references); SPEC-07 R3-R10 do not currently document any such precedent. A separate task SHOULD update SPEC-07 to register this pattern as a project-wide convention, so future codecs/subcommands can reuse it. (This is a follow-up; SPEC-27 v3 does NOT amend SPEC-07.)

**R22.** A new `encoders` subcommand MUST list available encoders:

```
$ relativist encoders list
Available encoders:
  church_add            Church numeral addition (a + b)
  church_mul            Church numeral multiplication (a × b)
  church_exp            Church numeral exponentiation (a ^ b)
  church_sum_of_squares Sum of squares (1² + 2² + ... + n²)
  horner                Polynomial evaluation via Horner's method
```

The subcommand MAY also be invoked as `relativist codecs list` (clap alias of the `encoders` subcommand) for terminological symmetry with R21. **(MUST for `encoders list`; MAY for `codecs list` alias)**

**R23.** The `compute --encoder` pipeline MUST be: `encode → validate → reduce_all → decode → print JSON`. **(MUST)**

### 3.7 RecipeEncoder Generalization

**R24.** The `RecipeEncoder` trait MUST be defined as an extension of `Encoder`:

```rust
pub trait RecipeEncoder: Encoder {
    type Recipe: Serialize + DeserializeOwned + Send + Sync;

    /// Whether this encoder supports recipe-based distributed generation.
    fn is_decomposable(&self) -> bool;

    /// Produce K recipes from the problem description and worker count.
    fn make_recipes(&self, input: &[u8], num_workers: u32) -> Result<Vec<Self::Recipe>, EncodeError>;

    /// Generate a local partition from a single recipe.
    fn generate_partition(&self, recipe: &Self::Recipe) -> Result<Partition, EncodeError>;
}
```

**(MUST)**

**R25.** `RecipeEncoder` MUST NOT be required for Codec registration. Codecs that do not implement `RecipeEncoder` fall back to centralized generation (coordinator generates full net, partitions via SPEC-04, ships partitions). **(MUST)**

**R26.** The existing SPEC-25 `GenerationRecipe` struct and `compute_recipes()` function MUST be refactored to implement `RecipeEncoder` for the built-in benchmark generators. Existing behavior MUST be preserved. **(MUST)**

**R27.** The wire protocol `AssignRecipe` message variant (SPEC-25 R15-R17) MUST be generalized to carry recipes from any `RecipeEncoder` in the registry, not only built-in generators. The message MUST include the encoder name so the worker can look up the correct `RecipeEncoder` implementation. **(MUST)**

**R28.** Workers MUST have access to the same `EncoderRegistry` as the coordinator. The registry is compiled in at build time (static registration). Dynamic plugin loading is out of scope. **(MUST)**

---

## 4. Non-Goals

**NG1.** Multi-language support. This spec defines Rust-only traits. FFI bindings (Python/PyO3), WASM plugins, and REST API are deferred to v2.x/v3 (DISC-012, Layer 4+).

**NG2.** HVM/Bend compatibility. No codec in v1 uses labeled IC symbols. `HornerCodec` (the v1 codec, §3.4) uses only Lafont's 3-symbol set (CON/DUP/ERA) and composes Church numerals defined in SPEC-14, which also use only Lafont's symbols. The future `LambdaCodec` (§5.1) will likewise use only Lafont's set. HVM compatibility requires ROADMAP 2.42 (label support), which is a separate decision and is out of scope for this spec.

**NG3.** DSL or custom language. No new syntax or parser is introduced in v1: HornerCodec consumes plain JSON only (R11'). Any future codec that requires a string surface syntax (e.g., the deferred LambdaCodec, §5.1) will introduce its parser under its own spec extension, not under SPEC-27.

**NG4.** Dynamic plugin loading. Encoders are registered at compile time via `default_registry()`. Runtime plugin discovery (e.g., loading .so/.dll files) is out of scope.

**NG5.** Distributed decode. Decoding always happens on the coordinator after merge. Worker-side decode is not supported.

---

## 5. Future Work

### 5.1 LambdaCodec (deferred from v1)

> **Informative scope note (v3).** The bullets in §5.1 are *informative*
> sketches of a future LambdaCodec design — they are NOT v2 normative
> requirements and MUST NOT be read as such. Normative requirements for
> LambdaCodec MUST be authored in a separate spec (e.g., a future SPEC-28
> or successor) when LambdaCodec is admitted into v2 scope.

A `LambdaCodec` for pure lambda-calculus terms (Var, Lam, App) following the
Mackie/Pinto pipeline (REF-005, Section 5) was specified in earlier drafts of
this document and remains a high-value future codec. It is deferred from v1
on the grounds that:

1. The TCC's empirical-validation needs are met by `HornerCodec`, which is a
   simpler codec demonstrating the same theoretical point: confluence preserves
   correctness under arbitrary reduction order (ARG-001 G1, the Fundamental
   Property; see R13' rationale). HornerCodec runs
   on top of SPEC-14 primitives (`build_add`, `build_mul`) without introducing
   any new IC encoding subtlety, while LambdaCodec via Mackie/Pinto introduces
   a non-trivial readback problem.
2. Mackie/Pinto encoding is non-trivial to implement and validate: port-directed
   readback is subtle, and DUP-CON commutation edge cases (variable sharing,
   erasure of unused bindings) require careful testing that exceeds the
   "secondary-to-TCC, simple codec" scope chosen for v1.
3. The trait API (R1-R6) is designed to accommodate LambdaCodec without
   modification when it is later implemented; only the codec module and a
   registry entry need to be added.

Future implementation work (Roadmap candidate, slot TBD — to be assigned by sdd-pipeline once a follow-on D-cycle is opened for codec extensions):
- `relativist-core::encoding::lambda` module
- LamCalc term grammar parser (string + JSON AST)
- Mackie/Pinto encode pipeline (REF-005 Section 5 mapping):
  - Lambda abstraction → 1 CON agent (p0=up, p1=variable, p2=body)
  - Application → 1 CON agent (p0=function, p1=result, p2=argument)
  - Variable → bidirectional link (linear: each variable used exactly once)
  - Free variables → FreePort connections
  - Erased variables → 1 ERA agent connected to the binding port
  - Shared variables → DUP agents inserted at branch points
- Port-directed readback decoder (Bend's `net_to_term` analogue, AC-013)
- Edge cases: identity, beta-reduction, erasure, duplication
- Property tests against a reference lambda interpreter

References:
- REF-005 (Mackie & Pinto 2002, Theorems 5.2 and 6.2)
- AC-013 (HVM/Bend `net_to_term` readback technique)
- DISC-012 v2 (Layer 3 lambda-calculus discussion that originally motivated R10-R16 in the v1 draft of this spec)

### 5.2 Other deferred codecs

Additional candidate codecs documented in DISC-012 v2 / ROADMAP §2.41 that
are NOT in v1 scope:
- `FactorialCodec` — `factorial(n)` via repeated `build_mul` (close cousin of HornerCodec; deferred to keep v1 minimal).
- `FibonacciCodec` — `F(n)` via Y combinator or unrolled DUP; useful as a structural-recursion demo.
- `MatMulCodec` — `A · B` for small matrices (composes Church arithmetic per cell; demonstrates 2-D parallelism).
- `PolynomialMultiEvalCodec` — evaluate the same polynomial at K points sharing the polynomial structure (natural extension of HornerCodec; demonstrates DUP-driven sharing of redex sub-trees).

All four are pure compositions of SPEC-14 primitives plus their own input-schema decoder; none would require changes to SPEC-01 invariants, SPEC-02 net structure, or the wire protocol.

---

## 6. Implementation Phases

| Phase | Deliverable | LoC | Depends on |
|-------|-------------|-----|-----------|
| **1. Traits** | `Encoder`, `Decoder`, `Codec`, error types, encode contract validation | ~100 | SPEC-26 R1-R7 |
| **2. Church refactoring** | `ChurchArithmeticCodec` implementing `Codec`, backward-compatible (R7-R9) | ~100 | Phase 1 |
| **3a. Horner encoder + composable helpers** | `HornerCodec::encode` (R10'-R13'), `wire_add_into` / `wire_mul_into` PortRef-based helpers (R13a' — promoted to `pub(crate)` from existing private helpers in `arithmetic.rs`; minor refactor, NOT new construction logic), input validation (R12'), edge cases (R16') | ~150 | Phases 1-2 |
| **3b. BigUint readback module** | `relativist-core::encoding::biguint_readback`: `decode_biguint(net) -> Result<BigUint, DecodeError>` (R14') and shared `walk_church<Counter>` helper if used; cross-check with `decode_nat` (R16b') | ~80 | Phase 3a |
| **3c. Oracle + Horner tests** | `horner_serial(coeffs, x) -> Result<BigUint, OracleError>` oracle (R16a'), `HornerCodec::decode` impl (calls Phase 3b), property tests T5-T13 | ~120 | Phases 3a + 3b |
| **4. Registry** | `EncoderRegistry`, `default_registry()`, `encoders list` CLI (R17-R20, R22) | ~200 | Phases 2-3c |
| **5. CLI integration** | `compute --encoder` / `--codec` dispatch with `conflicts_with` (R21), backward-compatible positional fallback (R23) | ~100 | Phase 4 |
| **6. RecipeEncoder** | Trait, refactor SPEC-25, generalize AssignRecipe (R24-R28) | ~150 | Phase 4 + SPEC-25 |

**Total estimated:** ~900-1000 LoC (excluding workspace restructure, which is SPEC-26).

The breakdown above is indicative for the SDD task-splitter (Stage 1 of the Relativist development pipeline). It does not constrain implementation order beyond the explicit `Depends on` column. The Phase 3 split into 3a/3b/3c keeps each TASK below the SDD <200 LoC atomicity rule.

---

## 7. Test Strategy

### 7.1 Trait and Validation Tests

**T1. Encode contract validation catches invalid nets.**
- Create a stub encoder that produces a net with a disconnected port (violates T1 of SPEC-01). Verify `encode_and_validate` returns `EncodeError::InvalidNet`.

**T2. Encode contract validation catches empty nets.**
- Create a stub encoder that produces a net with 0 redexes. Verify validation rejects it (E2: must have at least one redex).

### 7.2 Church Codec Tests (R7-R9)

**T3. ChurchArithmeticCodec round-trip (backward compatibility).**
- For each operation (add, mul, exp, sum_of_squares): encode via JSON input, reduce, decode. Verify result matches the existing `compute` CLI subcommand pipeline output (SPEC-14 §3.6 R22-R25, dispatching over `build_add` / `build_mul` / `build_exp` / `build_sum_of_squares`).
- `{"op":"add","a":3,"b":5}` → result: 8 (matches `compute add 3 5` legacy output).
- `{"op":"mul","a":4,"b":7}` → result: 28 (matches `compute mul 4 7` legacy output).
- `{"op":"exp","a":2,"b":3}` → result: 8 (matches `compute exp 2 3` legacy output; `a` is base, `b` is exponent per R8).
- `{"op":"sum_of_squares","a":3}` → result: 14 (= `1^2 + 2^2 + 3^2`; matches `build_sum_of_squares(3)` per SPEC-09 R17d).

**T4. All previously-passing arithmetic tests pass unchanged.**
- The Church refactoring is required to be behavior-preserving (R7, R9).

### 7.3 Horner Codec Tests (R10'-R16b')

**T5. Constant polynomial.**
- Input `{"coeffs":[42],"x":0}` → encoder skips Horner loop; decoded value is `"42"`. Covers R16' constant case.
- Input `{"coeffs":[42],"x":7}` → also `"42"` (constant polynomial is independent of x).

**T6. Smallest non-trivial Horner recurrence.**
- Input `{"coeffs":[1,1,1,1,1],"x":2}` → expected `"31"` (1+2+4+8+16). One representative invocation per worker count `W ∈ {1, 2, 4, 8}` MUST yield the same `"31"`, illustrating R13' (NF invariance under reduction order).

**T7. Canonical Horner case from the explainer doc.**
- Input `{"coeffs":[3,2,5,1],"x":2}`. Under R11' coefficient ordering (`coeffs[0]` = constant term, `coeffs[len-1]` = leading coefficient), `p(x) = 3 + 2·2 + 5·4 + 1·8 = 35`. The expected output value MUST be computed via `horner_serial(coeffs, x).unwrap()` (R16a') and not hard-coded as a string literal in the test fixture, to avoid drift between the two paths.

**T8. Sparse coefficients.**
- Input `{"coeffs":[1,0,0,0,0,1],"x":10}` → expected `"100001"`. Tests that zero coefficients reduce correctly via mul-by-zero ⇒ zero, add-with-zero ⇒ identity (R16' edge case). Expected value MUST also be cross-matched against `horner_serial`.

**T9. BigUint range (must strictly exceed `u64::MAX`).** Round 1 SC-006 documented that the v2 T9 input `[1; 20] @ x = 10` only produces `(10^20 - 1)/9 ≈ 1.11 × 10^19`, which is **less than** `u64::MAX = 1.844 × 10^19` and therefore does NOT exercise BigUint range. The v3 T9 MUST be:

- Input `{"coeffs":[1,1,...,1],"x":10}` with `coeffs.len() == 25` → result `(10^25 - 1)/9 ≈ 1.11 × 10^24`, which **strictly exceeds** `u64::MAX`. The test MUST verify (a) `bit_length > 64`, and (b) exact equality to `horner_serial(coeffs, 10).unwrap().to_string()`. The expected `bit_length` is derivable from `horner_serial(...).unwrap().bits()`; tests MUST NOT hard-code the bit count.

**T9b (new). BigUint range with coefficient boundary inclusion.**
- Input `{"coeffs":[10000, 10000, 10000, 10000, 10000], "x":10000}` → result `sum(10000 * 10000^i, i=0..4) = 10000 * (10000^5 - 1) / 9999 ≈ 10001000100010001 × ... ` (well over `u64::MAX`). MUST verify `bit_length > 64` and exact equality to `horner_serial`. This single test exercises both the boundary value `10_000` (R16') and BigUint range (R14') — see Round 1 SC-006 suggested resolution (b).

**T10. Edge cases enumerated in R16'.**
- `{"coeffs":[],"x":0}` → `EncodeError::InvalidInput` ("empty coeffs"); `horner_serial([], 0)` → `OracleError::EmptyCoeffs` (negative cross-check).
- `{"coeffs":[10000],"x":10000}` → boundary acceptance, decoded value matches `horner_serial([10000], 10000).unwrap()`.
- `{"coeffs":[10001],"x":0}` → `EncodeError::InvalidInput` (coefficient overflow); `horner_serial([10001], 0)` → `OracleError::CoefficientOverflow { idx: 0, value: 10001, max: 10000 }`.
- `{"coeffs":[1],"x":10001}` → `EncodeError::InvalidInput` (x overflow); `horner_serial([1], 10001)` → `OracleError::XOverflow { value: 10001, max: 10000 }`.
- `{"coeffs":[0,0,0,0],"x":7}` → `"0"` (all-zero coefficients).

**T11. Property test against oracle (R16a').**
- For randomly sampled `(coeffs, x)` within the SPEC-14 R4 caps, verify `decode(reduce_all(encode((coeffs, x)))) == horner_serial(coeffs, x).unwrap().to_string()`. At least 100 cases.
- **Negative cross-check (new in v3):** For randomly sampled out-of-range inputs (one or more `coeffs[i] > 10_000`, OR `x > 10_000`, OR `coeffs.len() == 0`), verify that the encoder returns `EncodeError::InvalidInput` AND the oracle returns the matching `OracleError` family (per R16a' negative cross-check clause). At least 30 cases.

**T12. BigUint readback agrees with `decode_nat` for u64-range nets (R16b').**
- Property test: for any net produced by `encode_nat(n)` with `n <= u64::MAX`, `decode_biguint(net) == Ok(BigUint::from(decode_nat(net).unwrap()))`. At least 100 sampled nets, including boundary `n = u64::MAX` (subject to the SPEC-14 R4 cap of `10_000` — this means the sampling range is `[0, 10_000]`; the property still holds tightly because both decoders return the same value on the same net).

**T13 (revised v3 — explicit decoder-stage protocol; in-process MUST, Docker TCP SHOULD).** For inputs T6, T7, T8, T9, T9b (and any other Horner test that produces a non-trivial NF):

- **Sequential baseline:** compute `seq_value = decode(reduce_all(encode(input))).unwrap()`. **(MUST)**

- **In-process distributed:** for each `W ∈ {2, 4, 8}`:
  - Compute `inproc_value = decode(extract_result(run_grid(encode(input), W, partition_strategy))).unwrap()`, where `partition_strategy` is the SPEC-04 default (round-robin, per SPEC-07 R3 default `--strategy round-robin`).
  - HornerCodec is NOT a `RecipeEncoder` (Q4); the coordinator therefore generates the full net centrally and partitions via SPEC-04 R25 fallback.
  - Decoding occurs on the coordinator's merged net (NG5: "Decoding always happens on the coordinator after merge").
  - **(MUST for `cargo test`.)**

- **Docker TCP distributed:** for each `W ∈ {2, 4, 8}`:
  - Compute `tcp_value` via the equivalent end-to-end pipeline launched against the Docker Compose `docker-local` deploy target (SPEC-07 §3.6).
  - **(SHOULD; MAY be marked `#[ignore]` to keep the default `cargo test` runtime bounded; MUST run in the CI integration suite — to be configured by the cicd agent in a follow-up task).**

- **Assertion:** `seq_value == inproc_value` for every `W`, and (when present) `seq_value == tcp_value`.

**This test is the empirical demonstration of ARG-001 G1 for HornerCodec** (cf. R13' rationale). G1 = "for any terminating net, sequential `reduce_all` and distributed `run_grid` produce isomorphic Normal Forms"; T13 specializes G1 to HornerCodec inputs by asserting that the **decoded** values are equal (which is a strictly weaker but TCC-relevant projection of structural isomorphism). P1 (strong confluence) is the engine; P3 (border redex completeness) and P4 (ID consistency) are the distribution-side preconditions.

### 7.4 Registry Tests

**T14. default_registry() contains the 5 v1 codecs.**
- Verify `list()` returns exactly 5 entries: `church_add`, `church_mul`, `church_exp`, `church_sum_of_squares`, `horner` (R19).

**T15. Duplicate registration fails.**
- Register `"horner"` twice → `RegistryError::DuplicateName` (R20).

**T16. Unknown encoder returns None.**
- `get("nonexistent")` → `None`. `get("lambda")` → `None` (not in v1 default registry per R19).

### 7.5 CLI Integration Tests

**T17. Backward compatibility.**
- `relativist compute add 3 5` (old positional syntax, R21 fallback) still produces the same JSON output as before this spec.

**T18. New `--encoder` flag.**
- `relativist compute --encoder horner --input '{"coeffs":[3,2,5,1],"x":2}'` → outputs JSON with `"value"` matching `horner_serial`.

**T19. `--codec` alias.**
- `relativist compute --codec horner --input '{"coeffs":[1,1],"x":3}'` → identical output to passing `--encoder horner`. (R21 alias.)

**T20. Mutually exclusive flags (R21 `conflicts_with` mechanics).**
- `relativist compute --encoder horner --codec horner --input '...'` → CLI rejects with the standard clap conflict error message produced by `conflicts_with` (NOT the silent last-value-wins behavior of `aliases(...)`). The error MUST mention both flag names. The exit code MUST be the clap default for `ErrorKind::ArgumentConflict` (typically 2 on most platforms).

**T21. Encoder list.**
- `relativist encoders list` → outputs the 5 v1 codecs (R22).
- `relativist codecs list` (alias) → identical output (R22 MAY).

### 7.6 RecipeEncoder Tests

**T22. Built-in generators still work via RecipeEncoder.**
- `ep_annihilation` via `RecipeEncoder::make_recipes()` → same result as SPEC-25 `compute_recipes()` (R26).

**T23. Non-decomposable codec falls back to centralized.**
- `HornerCodec` does not implement `RecipeEncoder` (Horner is sequential by construction; recipe-based decomposition is future work, §5.2 PolynomialMultiEvalCodec). Grid reduction MUST fall back to centralized partition via SPEC-04 (R25). Verify the distributed result matches sequential `reduce_all`.

---

## 8. Open Questions

**Q1. Trait object vs generic.** The registry uses `Box<dyn Codec>` (or `Arc<dyn Codec>`), which requires object safety. The `RecipeEncoder` trait has an associated type (`Recipe`), which is NOT object-safe. Resolution: `RecipeEncoder` is a separate trait checked via `Any` downcast or via a parallel registry, not part of the `Codec` trait object. Codecs that also implement `RecipeEncoder` are registered separately. Final implementation choice between `Box` and `Arc` is left to the developer; both satisfy R17.

**Q2. Input format.** R1 specifies JSON bytes as `&[u8]`. An alternative is `serde_json::Value`. JSON bytes are more flexible (the encoder can use any serde format internally), but `Value` is more ergonomic. Decision: use `&[u8]` for the trait, provide a helper `fn parse_input<T: DeserializeOwned>(input: &[u8]) -> Result<T, EncodeError>` in `encoding/traits`. HornerCodec consumes via this helper.

**Q3. Decimal representation of BigUint output.** R15' specifies base-10 string for `value`. Hex or base-256 byte arrays are alternatives but base-10 is the most user-friendly for the demo and trivially comparable against `horner_serial` output. Decision: base-10 only in v1; format extension can be added by a new optional field without breaking R15'.

**Q4. Decomposability of HornerCodec for distributed generation.** Horner's recurrence is sequential (the accumulator at step `k` depends on the accumulator at step `k+1`), so HornerCodec is NOT a `RecipeEncoder` — the coordinator generates the full net and partitions it via SPEC-04 (R25 fallback). A future `PolynomialMultiEvalCodec` (§5.2) that evaluates the same polynomial at K independent points IS a natural `RecipeEncoder` candidate (one recipe per evaluation point). This is intentional and out of v1 scope.

**Q5 (new v3). Pre-existence of `wire_add_into` / `wire_mul_into`.** As of `v0.20.0-pre` (REF-019), the helpers required by R13a' already exist as `pub(crate)` functions in `relativist-core::encoding::arithmetic` (introduced for SPEC-09 R17d `church_sum_of_squares`). Their signatures already match R13a' (PortRef-based). Phase 3a of §6 therefore consists of **promoting** the existing helpers to be reused by `HornerCodec` (no new construction logic), validating that the obligation set in R13a' (T1-T7 preservation, reduction equivalence, privacy) is met by inspection of the existing implementation, and adding any missing test coverage (e.g., tests that invoke `wire_*_into` directly with synthetic Church sub-nets, separate from `build_add` / `build_mul` round-trips). If a future implementation review (Stage 4 reviewer agent) finds that the existing helpers do NOT satisfy R13a''s obligations as stated, the implementation MUST add new helpers under different names; SPEC-27 v3 does NOT assume the existing helpers are correct without test validation.
