# Handoff Brief — SPEC-27 Revision for Topic 2 (Horner Codec)

**Para:** ESPECIALISTA EM SPECS (a ser disparado pelo usuário em sessão separada)
**Origin:** Sessão de design `feature/stress-and-encoder` (Topic 1 + Topic 2 brainstorm + design)
**Data:** 2026-05-06
**Branch:** `feature/stress-and-encoder` (Relativist subdir)
**Spec target:** `specs/SPEC-27-encoder-decoder-api.md`

---

## 1. Objetivo desta revisão

Substituir **LambdaCodec** (SPEC-27 R10-R16) por **HornerCodec** como o segundo codec de produção (primeiro = ChurchArithmeticCodec, já em R7-R9). LambdaCodec é demoted para a seção **Future Work** com ponteiro para o trabalho futuro detalhado.

A justificativa completa está em `docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md` (Topic 2 design doc) e na conversa de brainstorming associada (commits `033d8f8`, `ba26e55`, `8a44077` na branch).

**TL;DR da decisão:** o usuário priorizou um codec demonstrativo simples (Horner) que gera narrativa direta para o TCC ("algoritmo classicamente sequencial executando distribuído via confluência IC"). LambdaCodec via Mackie/Pinto é um POC ambicioso que excede o escopo de "encoder simples" + "secundário ao TCC" estipulado pelo usuário.

## 2. Critérios de aceitação para a revisão

A SPEC-27 revisada DEVE:

1. **Manter** R1-R6 inalterados (traits + EncodeContract; arquitetura núcleo)
2. **Manter** R7-R9 inalterados (ChurchArithmeticCodec wrapper)
3. **Substituir** R10-R16 (LambdaCodec) por novos requisitos para HornerCodec (ver §3 abaixo)
4. **Atualizar** R19 (default_registry built-ins): trocar `"lambda"` por `"horner"`
5. **Manter** R17, R18, R20 (registry mecânica)
6. **Manter** R21-R23 (CLI integration) — aceitar tanto `--encoder` quanto `--codec` como flag (alias) para compatibilidade com o uso de "codec" no design doc do Topic 2
7. **Manter** R24-R28 (RecipeEncoder generalization) — não-bloqueante, fica como infraestrutura disponível
8. **Adicionar** seção **§5 Future Work** consolidando LambdaCodec demoted (ver §4 abaixo)
9. **Manter** §4 Non-Goals com NG2 atualizado para refletir que LambdaCodec não está em v1
10. **Atualizar** o cabeçalho do spec: "Status: Revised v2 (Topic 2 alignment)" + bumped revision history

A SPEC-27 revisada NÃO DEVE:

- Modificar qualquer outra spec (SPEC-14, SPEC-25, etc.). HornerCodec compõe via SPEC-14 sem alterá-la.
- Adicionar dependências de rede ou protocol além das já presentes em R24-R28.
- Introduzir requisitos de performance (este codec é demo de correctness, não de throughput).
- Especificar implementação interna do HornerCodec (a árvore de Horner é uma escolha de implementação, não de spec).

## 3. Requisitos novos a adicionar (substituem R10-R16)

### 3.4 Horner Polynomial Codec (substitui §3.4 Lambda Calculus Codec)

**R10' (substitui R10).** A `HornerCodec` MUST be implemented in `relativist-core::encoding::horner` that encodes polynomial-evaluation problems using Horner's method as IC nets, composed entirely from `build_add` and `build_mul` primitives provided by SPEC-14. **(MUST)**

**R11' (substitui R11).** The `HornerCodec` encoder MUST accept the following input schema:

```json
{
  "coeffs": [<u64>, <u64>, ..., <u64>],
  "x": <u64>
}
```

Where `coeffs[i]` represents the coefficient `a_i` of `x^i`, with `coeffs[0]` being the constant term and `coeffs[coeffs.len() - 1]` being the leading coefficient. **(MUST)**

**R12' (substitui R12).** The encoder MUST enforce the following input bounds (inherited from SPEC-14 R4):

- `coeffs.len() >= 1` (no empty coefficient list)
- For each `coeffs[i]`: `coeffs[i] <= 10_000`
- `x <= 10_000`

Violations MUST return `EncodeError::InvalidInput` with a descriptive message. **(MUST)**

**R13' (substitui R13).** The encoder MUST construct the IC net by composing `build_add` and `build_mul` (from SPEC-14) following Horner's recurrence:

```
acc <- encode_church_into(net, coeffs[n])
for k in (n-1 .. 0):
    x_node <- encode_church_into(net, x)
    prod <- build_mul(net, acc, x_node)
    coef_node <- encode_church_into(net, coeffs[k])
    acc <- build_add(net, prod, coef_node)
net.set_root(acc)
```

The resulting net, when reduced to Normal Form via SPEC-03 `reduce_all` (or distributed equivalent in SPEC-05), MUST produce a Church numeral whose decoded value equals `p(x) = sum(coeffs[i] * x^i for i in 0..=n)`. **(MUST)**

**R14' (substitui R14).** The decoder MUST implement BigUint readback:

- Verify the net is in Normal Form (zero redexes); otherwise return `DecodeError::NotNormalForm`
- Traverse the Church numeral structure rooted at `net.root()`, accumulating the application count in `num_bigint::BigUint` (NOT `u64`)
- Return the value as a JSON-serializable string (base-10) plus its bit length

This avoids overflow when `p(x)` exceeds `u64::MAX`, which occurs commonly for moderate polynomial degrees. **(MUST)**

**R15' (substitui R15).** The output schema for `HornerCodec` MUST be:

```json
{
  "value": "<base-10 BigUint string>",
  "bit_length": <usize>
}
```

`bit_length` is the bit-length of the resulting BigUint (`BigUint::bits()`). **(MUST)**

**R16' (substitui R16).** The `HornerCodec` MUST handle the following edge cases correctly:

- **Empty coeffs:** `coeffs.len() == 0` → `EncodeError::InvalidInput("empty coeffs")`
- **Constant polynomial:** `coeffs.len() == 1` → encode-time skip the Horner loop; net is just `encode_church_into(net, coeffs[0])`; result is `coeffs[0]`
- **Evaluation at zero:** `x == 0` → result is `coeffs[0]` (mathematically correct; reducer handles via mul-by-zero ⇒ zero, add-with-zero ⇒ identity)
- **All-zero coefficients:** `coeffs == [0, 0, ..., 0]` → result is `0`
- **Maximum coefficient:** any `coeffs[i] == 10_000` MUST be accepted (boundary inclusive)
- **Coefficient overflow:** `coeffs[i] > 10_000` MUST return `EncodeError::InvalidInput`

**(MUST)**

**R16a' (novo).** A pure-Rust oracle function MUST be exposed for testing purposes:

```rust
pub fn horner_serial(coeffs: &[u64], x: u64) -> num_bigint::BigUint;
```

This function MUST compute the same value as `decode(reduce_all(encode((coeffs, x))))` using native Horner evaluation in Rust. Property tests MUST cross-check against this oracle. **(MUST)**

**R16b' (novo).** The `BigUintDecoder` (or equivalent helper) MUST live in `relativist-core::encoding::biguint_readback` and MUST be cross-checked against SPEC-14's `decode_nat` for `n <= u64::MAX`:

```rust
// Property: for any net N where decode_nat(N) is Ok(n) and n <= u64::MAX,
// decode_biguint(N) == BigUint::from(n)
```

This invariant MUST be tested. **(MUST)**

### 3.5 Encoder Registry (R17-R20: ajustes pontuais)

**R19 (atualizar).** A `default_registry()` function MUST return a registry pre-populated with:

- `"church_add"` — Church numeral addition (existente, R7-R9)
- `"church_mul"` — Church numeral multiplication (existente)
- `"church_exp"` — Church numeral exponentiation (existente)
- `"church_sum_of_squares"` — Sum of squares (existente)
- `"horner"` — Polynomial evaluation via Horner's method (novo, R10'-R16b')

`"lambda"` MUST NOT be in the default registry; it is documented as future work in §5. **(MUST)**

### 3.6 CLI Integration (R21-R23: ajuste opcional)

**R21 (manter; flag aliasing opcional).** The `compute` subcommand MUST accept an `--encoder` flag. The flag `--codec` MAY be accepted as an alias to `--encoder` (clap's `aliases` macro), since the design doc of Topic 2 uses "codec" terminology. The exact wording is editorial — both terms are common in the IC literature; "codec" emphasizes the symmetry of encode + decode while "encoder" reflects the historical R1 wording. The ESPECIALISTA EM SPECS decides. **(MUST for one form; MAY for both)**

## 4. Seção §5 Future Work (nova)

Adicionar uma seção §5 ao SPEC-27 contendo:

```markdown
## 5. Future Work

### 5.1 LambdaCodec (deferred from v1)

A `LambdaCodec` for pure lambda-calculus terms (Var, Lam, App) following the
Mackie/Pinto pipeline (REF-005, Section 5) was specified in earlier drafts of
this document and remains a high-value future codec. It is deferred from v1
on the grounds that:

1. The TCC's empirical-validation needs are met by HornerCodec, which is a
   simpler codec demonstrating the same theoretical point (confluence preserves
   correctness under arbitrary reduction order).
2. Mackie/Pinto encoding is non-trivial to implement and validate (port-directed
   readback is subtle; DUP-CON commutation edge cases require careful testing).
3. The trait API (R1-R3) is designed to accommodate LambdaCodec without
   modification when it is later implemented.

Future implementation work (Roadmap candidate D-NN+):
- `relativist-core::encoding::lambda` module
- LamCalc term grammar parser (string + JSON AST)
- Mackie/Pinto encode pipeline
- Port-directed readback decoder
- Edge cases: identity, beta-reduction, erasure, duplication
- Property tests against a reference lambda interpreter

References:
- REF-005 (Mackie & Pinto 2002, Theorems 5.2 and 6.2)
- AC-013 (HVM/Bend `net_to_term` readback)
- DISC-012 v2 (Layer 3 lambda calculus discussion)

### 5.2 Other deferred codecs

Additional candidate codecs documented in DISC-012 v2 / ROADMAP §2.41 that
are NOT in v1 scope:
- FactorialCodec (`factorial(n)` via repeated mul)
- FibonacciCodec (`F(n)` via Y combinator or unrolled DUP)
- MatMulCodec (`A · B` for small matrices)
- PolynomialMultiEvalCodec (evaluate at K points sharing the polynomial structure)
```

## 5. Atualizações em §4 Non-Goals

**NG2 (atualizar).** HVM/Bend compatibility. *No codec in v1 uses* labeled IC symbols.
HornerCodec uses only Lafont's 3-symbol set (CON/DUP/ERA). LambdaCodec (future, §5.1)
will also use only Lafont's set. HVM compatibility requires ROADMAP 2.42 (label support),
which is a separate decision.

(Original NG2 text mentioned LambdaCodec specifically; update to reflect that HornerCodec
is now the v1 codec subject to this constraint.)

## 6. Histórico de revisão a adicionar no cabeçalho

```markdown
**Revision history:**
- v1 (initial Draft): full proposal including LambdaCodec POC
- v2 (Topic 2 alignment, 2026-05-06): LambdaCodec demoted to §5 Future Work;
  HornerCodec promoted to v1 codec; default_registry updated; supersedes earlier
  R10-R16 with R10'-R16b' for Horner. See `docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md`.
```

## 7. Validação esperada

Após a revisão, a SPEC-27 v2 deve satisfazer:

1. **Coerência interna:** R1-R3 (traits) ainda válidos; R7-R9 (ChurchArithmetic) intactos; R10'-R16b' (Horner) substituem R10-R16; R17-R20 (Registry) atualizados em R19; R21-R23 mantidos com flag alias opcional; R24-R28 (RecipeEncoder) intactos.
2. **Coerência com SPEC-14:** caps de input (`<= 10_000`) batem com SPEC-14 R4.
3. **Coerência com ARG-001:** o teste empírico de HornerCodec ilustra P3 (confluência preserva determinismo sob qualquer ordem de redução).
4. **Testabilidade:** todos os requisitos R10'-R16b' são verificáveis por teste (cross-check com oracle, property tests, edge cases enumerados).
5. **Completude:** o spec contempla input schema, output schema, encode contract, edge cases, oracle, BigUint readback, e relação com Future Work.

## 8. Pipeline de revisão sugerido

```
1. ESPECIALISTA EM SPECS aplica o diff conceitual deste brief no SPEC-27 →
   produz SPEC-27 v2 + closure log

2. spec-critic Round 1 → review adversarial (consistência com predecessores
   SPEC-14, SPEC-25; testabilidade; completude; preservação de invariantes)

3. ESPECIALISTA EM SPECS Round 2 → addressa achados do Round 1, revisa SPEC-27 v2,
   produz closure log final em docs/spec-reviews/

4. Spec-critic Round 3 (se necessário) ou closure direta
```

## 9. Após a revisão

Quando SPEC-27 v2 for aprovada (closure log final em `docs/spec-reviews/`), o pipeline SDD do Topic 2 destrava:

```
1. SPLITTING — task-splitter quebra SPEC-27 v2 em ~10 TASKs atômicas (em docs/backlog/)
2. TESTS    — test-generator escreve TEST-SPECs por TASK
3. DEV      — developer implementa TDD
4. REVIEW   — reviewer
5. QA       — qa adversarial
6. REFACTOR — developer aplica fixes
```

Em paralelo, **Topic 1 (Stress Curve Campaign)** pode avançar SDD imediatamente — não tem spec novo a revisar (é metodologia de bench, não feature de sistema).

## 10. Referências

- **Design doc Topic 2:** `docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md` (commit `8a44077`)
- **Explainer Horner:** `docs/superpowers/specs/2026-05-06-horner-method-explainer.md` (commit `ba26e55`)
- **Design doc Topic 1:** `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` (commit `033d8f8`)
- **SPEC-27 atual:** `specs/SPEC-27-encoder-decoder-api.md` (Draft, status pre-revision)
- **SPEC-14 (intacta):** `specs/SPEC-14-encoding.md`
- **SPEC-25 (intacta):** `specs/SPEC-25-recipe-generation.md`
- **ROADMAP §2.41:** Encoder/Decoder API and Problem Registry
- **REF-005:** Mackie & Pinto 2002 (LambdaCodec referência futura)
- **REF-002:** Lafont 1997 (confluência)
- **ARG-001 P1-P6:** confluência preserva determinismo (DEBATEDOR)

---

**Pergunta do usuário ao receber esta brief:** dispare o agente `especialista-specs` com o input "execute esta revisão de SPEC-27 conforme docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md, aplicando o diff conceitual em §3-§6 do brief".
