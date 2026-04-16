# SPEC-27: Encoder/Decoder Trait API and Problem Registry

**Status:** Draft
**Depends on:** SPEC-01 (Invariants), SPEC-02 (Net Representation), SPEC-04 (Partitioning), SPEC-06 (Wire Protocol), SPEC-14 (Encoding), SPEC-25 (Recipe-Based Generation), SPEC-26 (GUI Application — workspace restructure R1-R7 only)
**ROADMAP items:** 2.41 (Encoder/Decoder API and Problem Registry)
**References consumed:** REF-005 (Mackie & Pinto 2002, Theorems 5.2 and 6.2: encoding linear logic with ICs), REF-002 (Lafont 1997: universality)
**Arguments consumed:** ARG-001 (P1-P6: confluence preserves determinism)
**Briefings consumed:** BRIEF-20260415-disc012-job-submission (6 reference systems, encode→reduce→decode contract, HVM compatibility analysis)
**Discussions consumed:** DISC-012 v2 (Job Submission, Encoding, and Decoding — 8 options analyzed, 2-round adversarial debate, Layers 0-3 selected for v2)

---

## 1. Purpose

This spec defines the Encoder/Decoder trait API that enables third-party problem encodings without forking the Relativist codebase. It covers three concerns:

1. **Trait definitions** for `Encoder`, `Decoder`, and `RecipeEncoder` — the extension points that domain-specific code implements.
2. **Encoder Registry** — a runtime-discoverable collection of named encoders, selectable via CLI.
3. **LambdaEncoder** — a proof-of-concept encoder for pure lambda-calculus terms, based on the formally verified Mackie/Pinto pipeline (REF-005).

The current Relativist has exactly one end-to-end pipeline: Church numeral arithmetic (`encoding/church.rs` + `encoding/arithmetic.rs`, ~500 LoC). This spec adds a second pipeline (lambda-calculus) and makes the extension mechanism explicit so that future encoders are plug-and-play.

**No IC-theoretic invariants are changed.** Encoders produce standard IC nets (CON/DUP/ERA, 3 ports per agent). The reducer is unchanged. This spec is entirely about the API boundary between "problem domain" and "IC infrastructure".

**Prerequisite:** SPEC-26 R1-R7 (Cargo workspace restructure) MUST be completed first. The traits are defined in the `relativist-core` library crate, which only exists after the workspace split.

---

## 2. Definitions

Terms defined in SPEC-00, SPEC-01, SPEC-02, SPEC-14, and SPEC-25 are used without redefinition. Terms introduced in this spec:

| Term | Definition |
|------|-----------|
| **Encoder** | A Rust trait that converts a domain-specific problem description into an IC net with redexes. The net, when reduced to normal form, encodes the solution. |
| **Decoder** | A Rust trait that interprets an IC net in normal form and extracts a domain-specific answer. The decoder is the semantic inverse of its paired encoder. |
| **Codec** | A combined Encoder+Decoder pair for a single problem domain. Codecs are the primary unit of registration. |
| **EncoderRegistry** | A runtime collection of named Codecs. The CLI dispatches to a Codec by name. |
| **LambdaEncoder** | A Codec that encodes pure lambda-calculus terms (Lambda, Application, Variable, Erasure) as IC nets via the Mackie/Pinto pipeline (REF-005) and decodes results via port-directed readback (inspired by Bend's `net_to_term`, AC-013). |
| **RecipeEncoder** | An extension of Encoder (from SPEC-25) that can produce a compact recipe instead of the full net. Generalizes SPEC-25's built-in recipe generation to user-defined encoders. |
| **Encode Contract** | The set of invariants (E1-E5, from DISC-012) that every encoder must satisfy for correctness: valid net (T1-T7), productive redexes, invertible decoding. |

---

## 3. Requirements

### 3.1 Trait Definitions

**R1.** The `Encoder` trait MUST be defined in `relativist-core::encoding::traits` with the following signature:

```rust
pub trait Encoder: Send + Sync {
    /// Human-readable name (e.g., "church_add", "lambda").
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
    #[error("net is not in normal form (has {redexes} redexes)")]
    NotNormalForm { redexes: usize },
    #[error("unrecognized net structure: {0}")]
    UnrecognizedStructure(String),
    #[error("decode failed: {0}")]
    DecodeFailed(String),
}
```

**(MUST)**

### 3.2 Encode Contract (Validation)

**R5.** Every `Encoder::encode()` output MUST be validated before reduction. Validation checks:
- **E1.** Net satisfies T1-T7 from SPEC-01 (valid net invariants).
- **E2.** Net has at least one redex (otherwise there is nothing to reduce).

Validation MUST be performed by the registry (R12), not by individual encoders. **(MUST)**

**R6.** If validation fails, the error MUST include which invariant was violated and a human-readable description. **(MUST)**

### 3.3 Church Numeral Codec (Refactoring)

**R7.** The existing Church numeral encoding (`encoding/church.rs`, `encoding/arithmetic.rs`) MUST be refactored to implement the `Codec` trait. The refactoring MUST NOT change any existing public API signatures — only add trait implementations. **(MUST)**

**R8.** The `ChurchArithmeticCodec` MUST support the following input schema:

```json
{
  "op": "add" | "mul" | "exp" | "sum_of_squares",
  "a": <u64>,
  "b": <u64>       // optional for sum_of_squares
}
```

The output schema MUST be:

```json
{
  "result": <u64>,
  "interactions": <u64>
}
```

**(MUST)**

**R9.** All 690 existing tests MUST pass after the Church refactoring. Zero behavioral changes. **(MUST)**

### 3.4 Lambda Calculus Codec

**R10.** A `LambdaCodec` MUST be implemented in `relativist-core::encoding::lambda` that encodes pure lambda-calculus terms as IC nets. **(MUST)**

**R11.** The `LambdaCodec` encoder MUST support the following term grammar:

```
Term ::= Var(name)           -- variable reference
       | Lam(name, Term)     -- lambda abstraction
       | App(Term, Term)     -- application
```

No numerals, no let-bindings, no types. This is the minimal lambda-calculus. **(MUST)**

**R12.** The input schema for `LambdaCodec` MUST be:

```json
{
  "term": "(λx. x)"          // string representation
}
```

OR the equivalent JSON AST:

```json
{
  "ast": { "Lam": ["x", { "Var": "x" }] }
}
```

Both formats MUST be accepted. **(MUST)**

**R13.** The encoding MUST follow the Mackie/Pinto mapping (REF-005, Section 5):
- Lambda abstraction → 1 CON agent (p0=up, p1=variable, p2=body)
- Application → 1 CON agent (p0=function, p1=result, p2=argument)
- Variable → bidirectional link (linear: each variable used exactly once)
- Free variables → FreePort connections
- Erased variables → 1 ERA agent connected to the binding port

**(MUST)**

**R14.** The decoder MUST implement port-directed readback:
- Enter CON via p0 → Lambda
- Enter CON via p1 → Variable
- Enter CON via p2 → Application
- Encounter ERA → erased term
- Encounter DUP → duplication (readback both branches)

This follows the same principle as Bend's `net_to_term` (AC-013, Section "Net -> Term Readback") but for the minimal lambda-calculus only. **(MUST)**

**R15.** The output schema for `LambdaCodec` MUST be:

```json
{
  "term": "λx. x",
  "agents": 2,
  "interactions": 3
}
```

**(MUST)**

**R16.** The `LambdaCodec` MUST handle the following edge cases:
- Identity: `(λx. x)` → normal form is `λx. x` (0 interactions)
- Beta-reduction: `(λf. λx. f x) (λy. y)` → `λx. x` (2 interactions)
- Erasure: `(λx. λy. y) (λz. z)` → `λy. y` (1 interaction, ERA erases argument)
- Duplication: `(λx. λy. x x) (λz. z)` → requires DUP, tests DUP-CON commutation
**(MUST)**

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
- `"church_add"` — Church numeral addition
- `"church_mul"` — Church numeral multiplication
- `"church_exp"` — Church numeral exponentiation
- `"church_sum_of_squares"` — Sum of squares
- `"lambda"` — Pure lambda-calculus

**(MUST)**

**R20.** Attempting to register a codec with a name that already exists MUST return `RegistryError::DuplicateName`. **(MUST)**

### 3.6 CLI Integration

**R21.** The `compute` subcommand MUST accept an `--encoder` flag:

```
relativist compute --encoder <name> --input '<json>'
```

If `--encoder` is omitted, the current behavior (positional args for Church arithmetic) MUST be preserved for backward compatibility. **(MUST)**

**R22.** A new `encoders` subcommand MUST list available encoders:

```
$ relativist encoders list
Available encoders:
  church_add            Church numeral addition (a + b)
  church_mul            Church numeral multiplication (a × b)
  church_exp            Church numeral exponentiation (a ^ b)
  church_sum_of_squares Sum of squares (1² + 2² + ... + n²)
  lambda                Pure lambda-calculus (encode, reduce, readback)
```

**(MUST)**

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

**NG2.** HVM/Bend compatibility. The `LambdaCodec` uses Lafont's pure ICs (3 symbols, no labels). It does not support Bend's extended ICs (7 symbols + labels). HVM compatibility requires ROADMAP 2.42 (label support), which is a separate decision.

**NG3.** DSL or custom language. No new syntax or parser beyond the lambda-calculus term format in R12.

**NG4.** Dynamic plugin loading. Encoders are registered at compile time via `default_registry()`. Runtime plugin discovery (e.g., loading .so/.dll files) is out of scope.

**NG5.** Distributed decode. Decoding always happens on the coordinator after merge. Worker-side decode is not supported.

---

## 5. Implementation Phases

| Phase | Deliverable | LoC | Depends on |
|-------|-------------|-----|-----------|
| **1. Traits** | `Encoder`, `Decoder`, `Codec`, error types, encode contract validation | ~100 | SPEC-26 R1-R7 |
| **2. Church refactoring** | `ChurchArithmeticCodec` implementing `Codec`, backward-compatible | ~100 | Phase 1 |
| **3. LambdaCodec** | Encoder (REF-005 mapping) + Decoder (port-directed readback) + tests | ~250 | Phase 1 |
| **4. Registry** | `EncoderRegistry`, `default_registry()`, `encoders list` CLI | ~200 | Phases 2-3 |
| **5. CLI integration** | `compute --encoder` dispatch, backward-compatible | ~100 | Phase 4 |
| **6. RecipeEncoder** | Trait, refactor SPEC-25, generalize AssignRecipe | ~150 | Phase 4 + SPEC-25 |

**Total estimated:** ~900 LoC (excluding workspace restructure, which is SPEC-26).

---

## 6. Test Strategy

### 6.1 Trait and Validation Tests

**T1. Encode contract validation catches invalid nets.**
- Create an encoder that produces a net with a disconnected port (violates T1). Verify `encode_and_validate` returns `EncodeError::InvalidNet`.

**T2. Encode contract validation catches empty nets.**
- Create an encoder that produces a net with 0 redexes. Verify validation rejects it (E2: must have at least one redex).

### 6.2 Church Codec Tests

**T3. ChurchArithmeticCodec round-trip (backward compatibility).**
- For each operation (add, mul, exp): encode via JSON input, reduce, decode. Verify result matches `compute_arithmetic()` output.
- `{"op":"add","a":3,"b":5}` → result: 8.
- `{"op":"mul","a":4,"b":7}` → result: 28.

**T4. All 690 existing tests pass unchanged.**

### 6.3 Lambda Codec Tests

**T5. Identity term.**
- Input: `{"term":"λx. x"}`. Encode → 2 agents (lam_f CON, lam_x CON), 0 redexes. Net is already in normal form. Decode → `"λx. x"`.

**T6. Single beta-reduction.**
- Input: `{"term":"(λx. x) (λy. y)"}`. Encode → 4 agents, 1 redex (CON-CON annihilation). Reduce → 2 agents. Decode → `"λy. y"`.

**T7. Nested application.**
- Input: `{"term":"(λf. λx. f (f x)) (λy. y)"}`. Encode, reduce, decode → `"λx. x"` (double identity reduces to identity).

**T8. Erasure.**
- Input: `{"term":"(λx. λy. y) (λz. z)"}`. Encode → includes ERA for unused x. Reduce → ERA-CON interaction erases argument. Decode → `"λy. y"`.

**T9. Duplication.**
- Input: `{"term":"(λx. x x) (λy. y)"}`. Encode → includes DUP for shared x. Reduce → DUP-CON commutation + annihilation. Decode → `"λy. y"` (self-application of identity).

### 6.4 Registry Tests

**T10. default_registry() contains all 5 codecs.**
- Verify `list()` returns 5 entries with correct names.

**T11. Duplicate registration fails.**
- Register "lambda" twice → `RegistryError::DuplicateName`.

**T12. Unknown encoder returns None.**
- `get("nonexistent")` → None.

### 6.5 CLI Integration Tests

**T13. Backward compatibility.**
- `relativist compute add 3 5` (old syntax) still works.

**T14. New encoder flag.**
- `relativist compute --encoder lambda --input '{"term":"(λx. x) (λy. y)"}'` → outputs JSON with `"term": "λy. y"`.

**T15. Encoder list.**
- `relativist encoders list` → outputs 5 encoders.

### 6.6 RecipeEncoder Tests

**T16. Built-in generators still work via RecipeEncoder.**
- `ep_annihilation` via `RecipeEncoder::make_recipes()` → same result as SPEC-25 `compute_recipes()`.

**T17. Non-decomposable codec falls back to centralized.**
- `LambdaCodec` does not implement `RecipeEncoder`. Grid reduction uses centralized partition. Verify result matches sequential.

---

## 7. Open Questions

**Q1. Trait object vs generic.** The registry uses `Box<dyn Codec>`, which requires object safety. The `RecipeEncoder` trait has an associated type (`Recipe`), which is NOT object-safe. Resolution: `RecipeEncoder` is a separate trait checked via `Any` downcast, not part of the `Codec` trait object. Codecs that also implement `RecipeEncoder` are registered separately.

**Q2. Input format.** R1 specifies JSON bytes as `&[u8]`. An alternative is `serde_json::Value`. JSON bytes are more flexible (the encoder can use any serde format internally), but `Value` is more ergonomic. Decision: use `&[u8]` for the trait, provide a helper `fn parse_input<T: DeserializeOwned>(input: &[u8]) -> Result<T, EncodeError>`.

**Q3. Lambda term parser.** The LambdaCodec needs a parser for `"(λx. x) (λy. y)"` syntax. Options: (a) hand-written recursive descent (~80 LoC), (b) use `nom` parser combinator (~60 LoC + dependency), (c) only accept JSON AST (no string syntax). Recommendation: hand-written parser to avoid new dependency.

**Q4. Variable linearity.** Pure lambda-calculus in ICs requires linear variable usage (each variable used exactly once). Non-linear terms (e.g., `λx. x x`) require DUP agents. The encoder MUST handle this automatically (insert DUPs for shared variables, ERAs for unused variables). This is the same mechanism as Bend's `term_to_net` (AC-013).
