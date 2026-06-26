# Encoder/Decoder API + Horner Distributed Evaluation — Design

**Data:** 2026-05-06
**Branch:** `feature/stress-and-encoder` (mesma do Topic 1; ambos topics convergem aqui)
**Tópico:** 2 de 2 (Topic 1 = Stress Curve, design doc separado)
**Companheiro:** `2026-05-06-horner-method-explainer.md` (foundation explainer; este design cita)
**Política de merge:** branch → `main` direto após aprovação explícita do usuário (mesma política do Topic 1)

---

## 1. Contexto

SPEC-14 entregou Church arithmetic (`encode_nat`, `build_add`, `build_mul`, `build_exp`, `decode_nat`) + comando `compute` hardcoded para esse caso. SPEC-27 está em **Draft** especificando uma API genérica (`Encoder`/`Decoder`/`Codec`/`EncoderRegistry`) com LambdaEncoder POC. Pré-requisito SPEC-26 R1-R7 (workspace restructure) já satisfeito.

Topic 2 implementa a API SPEC-27 com **Horner como o primeiro codec real**, demonstrando que avaliação polinomial — algoritmo classicamente sequencial — pode ser executada distribuídamente entre workers preservando correctness via confluência (Lafont 1997, ARG-001 P3). LambdaEncoder fica como Future Work na SPEC-27 revisada.

A motivação completa do Horner está no explainer companheiro (`2026-05-06-horner-method-explainer.md`). TL;DR: é o exemplo canônico de "algoritmo intrinsecamente sequencial" em literatura de algoritmos paralelos, e esse status faz dele a demo perfeita pra ilustrar empiricamente o que ARG-001 P3 prova formalmente.

## 2. Decisões fechadas no brainstorming

| # | Decisão | Valor |
|---|---|---|
| Problema | Math problem alvo | Avaliação polinomial via Horner |
| API | Escopo | (B) SPEC-27 traits + Registry + HornerCodec; LambdaEncoder fica em "Future Work" |
| Range | Decoder | `BigUint` (`num-bigint`); resultado sem teto |
| Demo | Validação distribuída | (II) Integration tests + `reproduce_article/scripts/horner_demo.sh` + diretório locked |
| Spec | Atualização SPEC-27 | (α) Revisão ANTES da implementação via handoff brief → ESPECIALISTA EM SPECS |
| Branch | Onde mora | `feature/stress-and-encoder` (mesma do Topic 1) |
| Merge | Destino | `main` direto após aprovação |
| Merge bundling | Topic 1 + Topic 2 | Recomendado juntos (1 PR); decisão final no momento |

## 3. Arquitetura

Tudo novo vive em `relativist-core/src/encoding/`. Não toca `net/`, `reduction/`, `partition/`, `merge/`, ou wire protocol. SPEC-14 (`encoding/church.rs`, `encoding/arithmetic.rs`) **fica intacta** — HornerCodec compõe via `build_add` + `build_mul` existentes.

### 3.1 Reaproveitado (zero código novo)

- `encode_nat`, `decode_nat`, `build_add`, `build_mul` (SPEC-14)
- `Net`, `AgentId`, `PortRef`, `Symbol` (SPEC-02)
- Pipeline distribuído (SPEC-03..06)
- `bench/suite.rs`, lock-and-manifest pattern, métricas D-012
- `serde_json` (já dependência do projeto), `thiserror` (idem)

### 3.2 Construído novo (~845 LoC distribuídos em 9 arquivos Rust + 2 scripts + 1 doc)

| Componente | Caminho | LoC | Papel |
|---|---|---|---|
| Trait `Encoder` + `EncodeError` | `relativist-core/src/encoding/traits/encoder.rs` (novo) | ~60 | SPEC-27 R1, R4 |
| Trait `Decoder` + `DecodeError` | `relativist-core/src/encoding/traits/decoder.rs` (novo) | ~50 | SPEC-27 R2, R4 |
| Trait `Codec` (Encoder + Decoder + descrição) | `relativist-core/src/encoding/traits/codec.rs` (novo) | ~25 | SPEC-27 R3 |
| `EncodeContract` validator (E1-E2) | `relativist-core/src/encoding/traits/contract.rs` (novo) | ~80 | SPEC-27 R5-R6: valida T1-T7 + presença de redex pré-redução |
| `EncoderRegistry` | `relativist-core/src/encoding/registry.rs` (novo) | ~120 | SPEC-27 R12: collection nomeada, dispatcher por nome, validação centralizada |
| `ChurchArithmeticCodec` (wrapper sobre SPEC-14) | `relativist-core/src/encoding/church_codec.rs` (novo) | ~100 | SPEC-27 R3 exige "all built-ins implement Codec"; wrapper expõe `add`/`mul`/`exp` de SPEC-14 via trait Codec; input JSON: `{"op":"add","a":3,"b":5}` |
| `HornerCodec` (encode + decode) | `relativist-core/src/encoding/horner.rs` (novo) | ~250 | Tradução `(coeffs, x) → Net` em árvore de `build_add`/`build_mul`; decode via BigUint readback |
| `BigUintDecoder` (helper) | `relativist-core/src/encoding/biguint_readback.rs` (novo) | ~80 | Estende `decode_nat` para BigUint quando resultado excede u64 |
| CLI `compute --codec X --input '{...}'` | `relativist-cli/src/compute.rs` (extensão) | ~80 | **Back-compat:** subcomandos legados `compute add 3 5` continuam funcionando como antes (SPEC-14 paths preservados); novo `compute --codec X --input '{...}'` despacha pro registry. Exato shape final ratificado na revisão SPEC-27. |
| Demo script | `reproduce_article/scripts/horner_demo.sh` | ~120 | Roda matrix `cases × env × W`; produz CSV + figura |
| Plot generator | `scripts/plot_horner_demo.py` | ~100 | 2 PDFs IEEE-ready (correctness table + walltime) |
| Docs metodologia | `docs/encoders/horner.md` | ~200 md | Tutorial reproduzível: input format, exemplos, validação |

**Adições de Cargo deps:** `num-bigint = "0.4"` em `relativist-core/Cargo.toml`. Sem outras dependências novas.

### 3.3 Interfaces críticas

```rust
// relativist-core/src/encoding/traits/encoder.rs
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("encoding produced invalid net: {0}")]
    InvalidNet(String),
    #[error("input too large: {size} exceeds limit {limit}")]
    InputTooLarge { size: usize, limit: usize },
}

pub trait Encoder: Send + Sync {
    fn name(&self) -> &str;
    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError>;
}

// relativist-core/src/encoding/traits/decoder.rs
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("net is not in normal form (has {redexes} redexes)")]
    NotNormalForm { redexes: usize },
    #[error("unrecognized net structure: {0}")]
    UnrecognizedStructure(String),
    #[error("decode failed: {0}")]
    DecodeFailed(String),
}

pub trait Decoder: Send + Sync {
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError>;
}

// relativist-core/src/encoding/traits/codec.rs
pub trait Codec: Encoder + Decoder {
    fn description(&self) -> &str;
}

// relativist-core/src/encoding/registry.rs
pub struct EncoderRegistry { /* HashMap<&'static str, Arc<dyn Codec>> */ }

impl EncoderRegistry {
    pub fn new_with_builtins() -> Self;  // registra "church_arithmetic" e "horner"
    pub fn register(&mut self, codec: Arc<dyn Codec>) -> Result<(), RegistryError>;
    pub fn get(&self, name: &str) -> Option<Arc<dyn Codec>>;
    pub fn list(&self) -> Vec<&'static str>;
    /// SPEC-27 R5-R6: valida E1 (T1-T7) + E2 (≥1 redex) antes de devolver Net.
    pub fn encode_validated(&self, codec_name: &str, input: &[u8])
        -> Result<Net, EncodeError>;
}

// relativist-core/src/encoding/horner.rs
pub struct HornerCodec;

#[derive(Serialize, Deserialize)]
pub struct HornerInput {
    pub coeffs: Vec<u64>,   // a_0, a_1, ..., a_n; cada a_i ≤ 10_000 (SPEC-14 cap)
    pub x: u64,             // x ≤ 10_000 (idem)
}

#[derive(Serialize, Deserialize)]
pub struct HornerOutput {
    pub value: String,      // BigUint serializado em base-10
    pub bit_length: usize,
}

impl Codec for HornerCodec {
    fn description(&self) -> &str { "Polynomial evaluation via Horner's method" }
}

impl Encoder for HornerCodec {
    fn name(&self) -> &str { "horner" }
    fn encode(&self, input: &[u8]) -> Result<Net, EncodeError> { /* árvore de Horner */ }
}

impl Decoder for HornerCodec {
    fn decode(&self, net: &Net) -> Result<serde_json::Value, DecodeError> { /* BigUint readback */ }
}

/// Oracle: avaliação serial em Rust nativo, usada por testes e validação cruzada.
pub fn horner_serial(coeffs: &[u64], x: u64) -> num_bigint::BigUint;
```

### 3.4 Algoritmo do encoder (sketch)

```text
fn encode_horner(coeffs: &[u64], x: u64) -> Net:
    if coeffs.is_empty():
        return Err(EncodeError::InvalidInput("empty coeffs"))

    let mut net = Net::new()
    let n = coeffs.len() - 1   // grau

    // acc <- a_n
    let mut acc = encode_church_into(&mut net, coeffs[n])

    for k in (0..n).rev():
        // acc <- acc * x + a_k
        let x_node = encode_church_into(&mut net, x)
        let prod = build_mul(&mut net, acc, x_node)
        let coef_node = encode_church_into(&mut net, coeffs[k])
        acc = build_add(&mut net, prod, coef_node)

    net.set_root(acc)
    Ok(net)
```

Cada iteração adiciona ~constante de agentes (CON/DUP/ERA do Church para `x`, `a_k`, e os combinadores `mul`/`add`). Total de agentes O(n × c) onde c é o custo Church por op (~6-12 dependendo do tamanho dos números).

### 3.5 Algoritmo do decoder (sketch)

```text
fn decode_biguint(net: &Net) -> Result<BigUint, DecodeError>:
    // 1. Verifica que net está em Normal Form (zero redexes)
    if net.count_redexes() > 0:
        return Err(NotNormalForm { redexes: net.count_redexes() })

    // 2. Faz readback do Church numeral via traversal estruturado
    //    (mesma lógica de SPEC-14 decode_nat, mas acumulando em BigUint
    //    em vez de u64 para não overflow)
    let count = readback_church_count_biguint(net, net.root())
    Ok(count)
```

O readback BigUint compartilha a estrutura de SPEC-14 `decode_nat`: percorre a cadeia `f^n(x)` contando aplicações. A única diferença é o tipo do acumulador (`BigUint` vs `u64`), evitando overflow para `n > 2⁶⁴`.

## 4. Procedimento de implementação (e demo)

### Fase 0 — Spec revision

Eu produzo um **handoff brief** em `docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md` com:
- Diff conceitual: o que muda em SPEC-27 (LambdaEncoder R7-R10 → seção "Future Work"; HornerCodec adicionado como R7'-R10' codec v1)
- Justificativa baseada nas decisões deste design doc
- Critério de aceitação para a revisão (testabilidade, completude, consistência com SPEC-14)
- Texto sugerido para a seção "Future Work" sobre LambdaEncoder

Você dispara o **ESPECIALISTA EM SPECS** de outra sessão. Saída: SPEC-27 revisada + closure log em `docs/spec-reviews/`. Spec passa por **spec-critic Round 1** + **ESPECIALISTA EM SPECS Round 2** antes de fechar (padrão CLAUDE.md / WORKFLOWS.md).

### Fase 1 — Implementação SDD

Pipeline padrão de 6 stages do CLAUDE.md aplicado contra SPEC-27 revisada:
- task-splitter quebra em ~10 TASKs atômicas (3 traits, contract, registry, horner, biguint helper, CLI, scripts, docs)
- test-generator escreve TEST-SPECs por TASK
- developer implementa TDD (RED → GREEN → REFACTOR)
- reviewer (quality + arquitetura)
- qa adversarial (foca em encoder edge cases: overflow, malformed JSON, registry abuse, double-register)
- developer aplica fixes

### Fase 2 — Demo distribuído

`reproduce_article/scripts/horner_demo.sh` roda casos representativos cobrindo grau e magnitude:

| Case | Coeffs | x | Grau | Resultado esperado | Razão de incluir |
|---|---|---|---|---|---|
| C1 | `[1, 1, 1, 1, 1]` | 2 | 4 | 31 | Smallest non-trivial; sanity |
| C2 | `[3, 2, 5, 1]` | 2 | 3 | 43 | Caso do explainer; canônico |
| C3 | `[1, 0, 0, 0, 0, 1]` | 10 | 5 | 100_001 | Coeffs esparsos (zeros) |
| C4 | `[1] * 10` (10 uns) | 10 | 9 | 1_111_111_111 | Beira u32; testa escala |
| C5 | `[1] * 20` | 10 | 19 | ~10¹⁹ (BigUint) | Excede u64; valida BigUint path |
| C6 | `[7, 13, 5, 11, 3]` | 100 | 4 | BigUint não-trivial | Coeffs primos não-triviais |
| C7 | `[1] * 50` | 5 | 49 | ~10³⁵ (BigUint) | Grau alto; rede grande |

Cada caso roda em `W ∈ {1, 2, 4, 8}` × `env ∈ {in_process, docker_tcp}` = 8 configurações. Total: **7 × 8 = 56 runs** (1 rep cada — demo de correctness, não de variância). ~10-15 min total no Docker; ~3 min in-process.

Output do script:

```
reproduce_article/results/locked/v2_horner_demo_<YYYY-MM-DD>/
├── MANIFEST.md                         # provenance D-012 pattern
├── README.md
├── raw/
│   └── horner_results.csv              # case_id, coeffs, x, env, W, wall_time_ms,
│                                       # decoded_value_base10, oracle_match, encoded_agents,
│                                       # reduction_rounds
├── figures/
│   ├── horner_correctness.pdf          # tabela visual: 7 cases × 8 configs, marca ✓ se oracle_match
│   └── horner_walltime.pdf             # wall time por case por W; mostra speedup quando aplicável
└── checksums.sha256
```

## 5. Outputs e atualizações de docs

| Arquivo | Mudança |
|---|---|
| `docs/specs/SPEC-27-encoder-decoder-api.md` | **Revisão pelo ESPECIALISTA EM SPECS (Fase 0)** — única edição em `specs/` |
| `docs/encoders/horner.md` | Novo. Tutorial: input format, exemplos, validação por oracle |
| `docs/INDEX.md` | Nova entrada "Encoders > Horner Polynomial Evaluation" |
| `docs/ROADMAP.md` §2.41 | Status: "**[DONE — Topic 2]** trait API + EncoderRegistry shipped; HornerCodec is the v1 codec; LambdaEncoder remains as documented future work" |
| `docs/next-steps.md` | Bundle "D-015 Encoder/Decoder API + Horner" (D-014 = Topic 1 Stress Curve) |
| `CHANGELOG.md` | Entrada em `[Unreleased]` ou nova `[v0.21.0-pre]` |
| `relativist-cli/src/compute.rs` | Extensão `--codec X` (default `church_arithmetic` para back-compat com SPEC-14) |

**Handoffs para TCC root** (sessão separada, pós-demo):
- REDATOR: incorpora explainer (`2026-05-06-horner-method-explainer.md`) + figura `horner_correctness.pdf` na Seção 5; cita ARG-001 P3 explicitamente
- DEBATEDOR: pode atualizar argumentos sobre demonstrações empíricas

## 6. Riscos & limitações

**Riscos de execução (mitigados):**

| Risco | Mitigação |
|---|---|
| Coeffs ou `x` excedem cap SPEC-14 (10_000) | `EncodeError::InvalidInput` com msg explícita; demo cases respeitam |
| Resultado BigUint enorme inflama logs/CSV | CSV armazena base-10 string (compacta); MANIFEST documenta caso peor |
| Rede IC do Horner com grau alto explode em agentes (~6-12 por op × 2n ops) | Documentado no MANIFEST; demo case C7 (grau 49) é o peor; ainda cabe em RAM razoável |
| Reducer demora muito em casos de grau 50 (Church mul é caro) | Timeout por case = 10 min; falha vira `oracle_match=null` + linha de erro |
| Decoder BigUint readback diverge do Church readback de SPEC-14 | Property test cruzado: para `n ≤ u64::MAX`, `decode_biguint(net) == BigUint::from(decode_nat(net))` |
| Spec critic rejeita revisão de SPEC-27 em Round 1 | Re-handoff; iteração SDD padrão. Sua decisão se valeu a pena ou se simplifica escopo. |
| HornerCodec assume `coeffs.len() ≥ 1` | `EncodeError::InvalidInput("empty coeffs")`; testes cobrem |
| `coeffs.len() == 1` (polinômio constante p(x) = a_0) | Encoder pula loop; só `encode_nat(a_0)`; teste explícito |
| `x = 0` (avalia em zero — só `a_0` importa) | Reduz corretamente por construção (mul por zero = zero); teste explícito |
| Registry duplicate-name registrado por engano | `Result<(), RegistryError>` no `register`; teste explícito |

**Limitações estruturais (declaradas no TCC):**

1. **Performance: nada vence Horner nativo.** Church arithmetic é várias ordens de grandeza mais lenta. O ponto é correctness sob distribuição, não tempo absoluto.
2. **Apenas inteiros não-negativos.** Coeffs e `x` são `u64`. Polinômios com coeficientes negativos / racionais / floats ficam fora — exigiriam encoders separados (Future Work).
3. **Apenas avaliação em um ponto.** Não calculamos `p(x)` para múltiplos `x` num único net (extensão trivial mas fora de escopo).
4. **Apenas Horner.** Outros algoritmos paralelos clássicos sequenciais (parallel prefix scan, gather/scatter) ficam como future codecs.
5. **Caps herdados do SPEC-14.** `coeffs[i] ≤ 10_000` e `x ≤ 10_000`. Pode ser relaxado se SPEC-14 evoluir, mas fora deste escopo.

## 7. Estratégia de testes

| Camada | Quantidade aprox | O que valida |
|---|---|---|
| Unit (`encoding/horner.rs`) | ~10 | `build_horner` em casos: empty (erro), degree 0 (`p=a_0`), degree 1, x=0, x=1, all zeros, max coeff (=10_000), coeffs > cap (erro), coeffs.len() == 1 |
| Unit (`encoding/registry.rs`) | ~6 | register/get/list; `encode_validated` rejeita net inválido (E1); `encode_validated` rejeita net sem redex (E2); duplicate-name rejected; lookup case-sensitive; built-ins (`church_arithmetic` + `horner`) presentes após `new_with_builtins()` |
| Unit (`encoding/church_codec.rs`) | ~4 | Wrapper despacha corretamente para SPEC-14 add/mul/exp; rejeita JSON malformado; cross-check produz mesmo Net que chamada direta a `build_add`/`build_mul`/`build_exp` |
| Unit (`encoding/biguint_readback.rs`) | ~4 | Decoded value em u64 cap; cross-check com `decode_nat` para `n ≤ u64::MAX`; readback de Normal Form com Church numeral grande; rejeita net não-NF |
| Unit (traits + contract) | ~4 | EncodeContract E1 rejeita T-violation; E2 rejeita zero redex; error display correto; trait object dispatch funciona |
| Integration (`tests/horner_codec.rs`) | ~5 | Roundtrip encode→reduce→decode == oracle, em casos C1-C7 (in-process, W=1) |
| Integration (`tests/horner_distributed.rs`) | ~4 | Mesmo roundtrip mas via pipeline distribuído `W ∈ {1, 2, 4, 8}` em-processo |
| Integration (`tests/horner_cli.rs`) | ~3 | `relativist compute --codec horner --input '{"coeffs":[3,2,5,1],"x":2}'` retorna `{"value":"43","bit_length":6}` |
| Property (`tests/horner_proptest.rs`) | ~2 | Para `(coeffs, x)` aleatórios com bounds (≤100 coeffs, cada ≤ 1000, x ≤ 1000), `decode == horner_serial`; redução determinística (mesmo net → mesmo resultado em runs distintos) |

**Não-regressão (mesmos pisos do Topic 1, sobem com novos testes):**
- `cargo test --release` ≥ 1740 (atual) + ~42 novos = ~1782
- `cargo test` ≥ 1798 + ~42 = ~1840
- `cargo test --features zero-copy` ≥ 1842 + ~42 = ~1884
- `cargo test --features streaming-no-recycle` ≥ 1789 + ~42 = ~1831
- `cargo clippy --all-features -- -D warnings` 0 warnings
- `cargo fmt --check` 0 diff

Pisos finais documentados no commit que fecha Topic 2.

**Sanity checks pós-demo (manual):**
- 56/56 runs com `oracle_match = true` (qualquer falso = bug GRAVE; investiga imediatamente, **não merge**)
- Wall time crescente com grau (esperado)
- Wall time em W=4 ≤ wall time em W=1 nos cases de grau alto (Horner em árvore tem paralelismo inerente; se W=4 ≥ W=1, suspeitar de overhead dominante)
- BigUint values em C5/C6/C7 batem com `horner_serial` byte-a-byte (decoder não pode ter rounding ou truncation)

## 8. Pipeline SDD aplicado

| Stage | Agente | Output |
|---|---|---|
| 0. SPEC HANDOFF | (este design doc) → handoff brief | `docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md` |
| 0a. SPEC REV | ESPECIALISTA EM SPECS (você dispara) | SPEC-27 revisada |
| 0b. SPEC CRITIC | spec-critic Round 1 | Review em `docs/spec-reviews/` |
| 0c. SPEC DEFENDER | ESPECIALISTA EM SPECS Round 2 | Closure log em `docs/spec-reviews/` |
| 1. SPLITTING | task-splitter | ~10 TASKs em `docs/backlog/` |
| 2. TESTS | test-generator | TEST-SPECs em `docs/tests/` |
| 3. DEV | developer | Implementação TDD; só ele escreve código |
| 4. REVIEW | reviewer | Quality + arquitetura |
| 5. QA | qa | Adversarial em encoder edge cases (overflow, malformed input, registry abuse) |
| 6. REFACTOR | developer | Aplica fixes |

Após 6 stages verdes + demo rodada + aprovação do usuário → merge da `feature/stress-and-encoder` direto pra `main` (preferencialmente junto com Topic 1 num único PR).

## 9. Política de commits

Branch: `feature/stress-and-encoder` (mesma do Topic 1; ambos topics convergem aqui).

Ordem de commits cumulativa:

1. ✅ Design doc Topic 1 (commit `033d8f8`)
2. ✅ Explainer Horner (commit `ba26e55`)
3. **(este momento)** Design doc Topic 2 (este arquivo)
4. **(próximo entregável após sua aprovação)** Handoff brief Fase 0 → ESPECIALISTA EM SPECS
5. SPEC-27 revisão (ESPECIALISTA EM SPECS, sessão separada — você dispara)
6. SDD bundle Topic 2:
   - (6a) traits (`encoder.rs`, `decoder.rs`, `codec.rs`) + tests
   - (6b) contract + registry + tests
   - (6c) HornerCodec + BigUintDecoder + tests unit
   - (6d) CLI extension `--codec` + tests
   - (6e) integration tests (`horner_codec.rs`, `horner_distributed.rs`, `horner_cli.rs`) + property tests
   - (6f) demo script + plot generator
   - (6g) docs (`docs/encoders/horner.md`, `docs/INDEX.md`, `docs/ROADMAP.md`, `docs/next-steps.md`, `CHANGELOG.md`)
   - (6h) demo run + diretório locked
7. SDD bundle Topic 1 (paralelizável ou sequencial; sua decisão de ordem)
8. Aprovação explícita do usuário → push branch → merge para `main`

Tópicos 1 e 2 podem mergear juntos (1 PR) ou separados (2 PRs sequenciais). **Recomendado:** juntos, porque (a) compartilham branch, (b) compartilham CHANGELOG entry "[v0.21.0-pre]", (c) menos overhead de revisão.

## 10. Fora de escopo

- LambdaEncoder (LamCalc → IC via Mackie/Pinto) — Future Work em SPEC-27 revisada
- Outros codecs (Fatorial, Fibonacci, MatMul, Polynomial Multi-Eval) — design doc separado se forem entrar
- Coeficientes negativos / racionais / floats
- Múltiplos pontos de avaliação no mesmo net
- Recipe-based encoder (SPEC-25 Draft, fora; HornerCodec usa o pipeline normal de generation)
- GUI (SPEC-26 Draft, fora deste escopo)
- Streaming reduction (Topic 1 também não shipa; ROADMAP §2.16)
- Comparação de performance com Horner nativo C/Rust (off-topic; ponto é correctness)
- Editar `OBJETIVO_TCC.md`, `artigo/tcc_pt_br.tex`, `discussoes/...` (delegado para REDATOR/DEBATEDOR no TCC root)
- Tag de release durante esta entrega
- Editar `specs/` diretamente (Fase 0a delega ao ESPECIALISTA EM SPECS)

## 11. Verificação end-to-end

Após implementação e antes de aprovar o demo:

```bash
cd codigo/relativist

# 1. Pisos de teste verdes (post-Topic 2)
cargo test --release             # ≥1782 (1740 + ~42)
cargo test                       # ≥1840 (1798 + ~42)
cargo test --features zero-copy  # ≥1884 (1842 + ~42)
cargo test --features streaming-no-recycle  # ≥1831 (1789 + ~42)
cargo clippy --all-features -- -D warnings  # 0
cargo fmt --check                            # 0 diff

# 2. CLI smoke (caso C2 do explainer)
cargo run --release -- compute --codec horner \
    --input '{"coeffs":[3,2,5,1],"x":2}'
# Espera: {"value":"43","bit_length":6}

# 3. Lista de codecs registrados
cargo run --release -- compute --list-codecs
# Espera: church_arithmetic, horner

# 4. Demo distribuído completo
reproduce_article/scripts/horner_demo.sh           # ~10-15 min

# 5. Inspeção do output
ls reproduce_article/results/locked/v2_horner_demo_<YYYY-MM-DD>/figures/
# Espera: horner_correctness.pdf, horner_walltime.pdf
cat reproduce_article/results/locked/v2_horner_demo_<YYYY-MM-DD>/MANIFEST.md
# Espera: provenance completa + 56/56 oracle_match=true

# 6. Sanity checks manuais (conforme tabela na seção 7)
# Aprovação do usuário → merge para main
```

## 12. Próximos passos

1. Você revisa este doc e aprova ou pede revisões.
2. Eu produzo o **handoff brief** (`docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md`) para Fase 0a.
3. Você dispara o **ESPECIALISTA EM SPECS** de outra sessão para revisar SPEC-27.
4. Após SPEC-27 revisada e aprovada → eu (ou agente Relativist próprio) executa o bundle SDD do Topic 2 (~10 TASKs).
5. Topic 1 SDD acontece em paralelo ou sequencial à sua escolha.
6. Aprovação final + merge para `main`.

## 13. Companheiros

- **Foundation explainer:** `2026-05-06-horner-method-explainer.md` — o que é Horner, pra que serve, por que é a demo perfeita para confluência.
- **Topic 1 design:** `2026-05-05-stress-test-large-nets-design.md` — sub-projeto irmão (curva de stress test).
- **SPEC-27 (a revisar):** `docs/specs/SPEC-27-encoder-decoder-api.md` — Draft atual com LambdaEncoder; revisado na Fase 0a para HornerCodec como codec v1.
- **SPEC-14 (intacta):** `docs/specs/SPEC-14-encoding.md` — Church arithmetic já em produção; HornerCodec compõe por cima.
- **ROADMAP §2.41:** Encoder/Decoder API and Problem Registry — atualizado para `[DONE — Topic 2]` ao fechar.
