# Handoff Brief — SPEC-27 Stage 2 (test-generator)

**Para:** `sdd-pipeline` agent (Camada 2, Relativist)
**Origem:** Sessão TCC-root `feature/stress-and-encoder` em 2026-05-06
**Branch:** `feature/stress-and-encoder` (Relativist subdir)
**Stage atual:** Stage 1 (task-splitter) **DONE** → próximo é Stage 2 (test-generator)

---

## 1. Contexto resumido

Topic 2 do roadmap atual (HornerCodec promovido a v1 codec; LambdaCodec deferido para §5 Future Work) percorreu o pipeline SDD até Stage 1:

| Etapa | Agente | Estado | Artefato principal |
|-------|--------|--------|--------------------|
| Spec authoring (v2) | `especialista-specs` | DONE | `specs/SPEC-27-encoder-decoder-api.md` v2 |
| Round 1 critic | `spec-critic` | DONE | `docs/spec-reviews/SPEC-27-round1-critic.md` (NEEDS REVISION; 4 HIGH + 7 MEDIUM + 2 LOW) |
| Round 2 response | `especialista-specs` | DONE | `docs/spec-reviews/SPEC-27-v2-round2-response.md` (todas 13 issues ACCEPTED com fixes) → spec promovida a **v3** |
| Stage 1 splitting | `task-splitter` | DONE | 11 TASKs em `docs/backlog/TASK-0709..0719` |
| **Stage 2 tests** | **`test-generator`** | **NEXT** | TEST-SPECs em `docs/tests/` |

## 2. Briefing direto para o sdd-pipeline

Cole o bloco abaixo ao invocar o `sdd-pipeline`:

> SPEC-27 v3 fechou Round 2 spec-critic em 2026-05-06. Stage 1 (task-splitter) criou TASK-0709..0719 (11 tasks atomicas, todas <200 LoC, sem L/XL). BACKLOG.md ganhou secao "SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)". Avance para Stage 2 (test-generator) — gerar TEST-SPECs para as 11 tasks em `docs/tests/`, citando T1-T23 da §7 do SPEC-27 v3. Caminho critico: 0711 → 0714 → 0715 → 0716. Foundationals paralelizaveis: 0709, 0711, 0713.

O `sdd-pipeline` deve:
1. Validar `docs/pipeline-state.md` e marcar Stage 1 como concluído.
2. Verificar que as 11 task files existem e têm "Test Expectations" preenchidos.
3. Despachar `test-generator` com o briefing correto (uma TEST-SPEC por TASK, mapeamento R# ↔ T# preservado).

## 3. Tasks a cobrir (TASK-0709..0719)

| ID | Título | Complex | Test IDs SPEC-27 v3 §7 |
|----|--------|---------|------------------------|
| 0709 | R4 NotNormalForm valid-pair semantics + I4 prune helper | S | T1 |
| 0710 | R7-R9 ChurchArithmeticCodec audit + R8 operand semantics | S | T3, T4 |
| 0711 | R13a' wire_*_into obligation validation (Phase 3a promotion) | S | (validation tests for `wire_add_into`/`wire_mul_into`) |
| 0712 | R14'/R16b' biguint_readback module (`decode_biguint`) | M | T5, T6, T11 (cross-check vs SPEC-14 `decode_nat`) |
| 0713 | R16a' `horner_serial` oracle + `OracleError` + `MAX_CHURCH_NAT` | S | T11, T12 |
| 0714 | R10'-R13', R16' HornerCodec encoder | M | T5, T7, T8, T9, T9b, T10 |
| 0715 | R14', R15', R16' HornerCodec decoder + Codec impl | S–M | T6, T11, T12, T13 |
| 0716 | R19, R20 default_registry: drop lambda, add horner | S | T14, T15 |
| 0717 | R21, R23 CLI `--encoder`/`--codec` `conflicts_with` | S–M | T19, T20 |
| 0718 | R22 `encoders list` + `codecs list` alias | S | T21 |
| 0719 | R24-R28 RecipeEncoder audit + AssignRecipe encoder_name | M | T22, T23 |

## 4. Mapeamento R# → TASK# (cobertura completa)

| R# | TASK | R# | TASK |
|----|------|----|------|
| R1, R2, R3 | (já shipped; auditado em 0710) | R15' | 0715 |
| R4 (v3 update) | **0709** | R16' | 0714, 0715 |
| R5, R6 | 0709 (audit) | R16a' | **0713** |
| R7 | 0710 | R16b' | 0712 |
| R8 | **0710** | R17, R18 | (já shipped) |
| R9 | 0710 (CI gate) | R19 | **0716** |
| R10' | 0714 | R20 | 0716 (regression test) |
| R11' | 0714 | R21 | **0717** |
| R12' | 0714 | R22 | **0718** |
| R13a' | **0711** | R23 | 0717 |
| R13' | 0714 | R24 | 0719 |
| R14' | 0712, 0715 | R25, R26, R27, R28 | 0719 |

## 5. DAG de dependências

```
   0709    0711    0713      (foundationals paralelizaveis)
    |       |       |
   0710   0712      |
            \      /
             0714 (encoder)
              |
             0715 (decoder + Codec impl)
              |
             0716 (registry)
            / | \
       0717 0718 0719
```

**Caminho crítico:** 0711 → 0714 → 0715 → 0716 → {0717, 0718, 0719} (5 hops).

## 6. Decisões estruturais relevantes para test-generator

Do closure log Round 2 (`docs/spec-reviews/SPEC-27-v2-round2-response.md`):

- **SC-013 / Caminho A:** R13a' especifica `wire_add_into(net: &mut Net, m: AgentId, n: AgentId) -> AgentId` e `wire_mul_into` como `pub(crate)` em `relativist-core::encoding::arithmetic`. **Helpers já existem em HEAD** (`arithmetic.rs:92,224`, introduzidos para SPEC-09 R17d). Phase 3a é *promotion-and-validation*, não new-construction. TASK-0711 deve gerar TEST-SPEC validando que as signatures atuais batem com R13a' e cobrem os contratos do SPEC-27 v3.
- **SC-006 / T9 BigUint range:** T9 foi bumpado para `coeffs.len()=25` (resultado ~1.11×10^24, **strictly exceeds** `u64::MAX`). T9b adicional cobre boundary `[10000;5] @ x=10000`. TASK-0714 deve incluir ambos.
- **SC-008 / R21 clap:** `--encoder` e `--codec` usam `conflicts_with` (NÃO `aliases(...)` — aliases silently keeps last value). TASK-0717 deve testar `clap::ErrorKind::ArgumentConflict` quando ambos são passados.
- **SC-009 / G1 vs P3:** R13' rationale e T13 foram corrigidos para citar **G1** (Fundamental Property) com P1 como engine + P3+P4 como preconditions, NÃO P3 isoladamente. TASK-0715 (decoder + Codec impl) e qualquer TEST-SPEC que invoque ARG-001 deve seguir essa correção.
- **SC-007 / Oracle:** `horner_serial` retorna `Result<BigUint, OracleError>`. T11 ganhou negative cross-check (≥30 cases). TASK-0713 deve refletir isso na assinatura e nos testes.
- **SC-005 / NotNormalForm:** semântica é "valid active pairs após stale pruning per SPEC-01 I4", NÃO `redex_queue.len()`. TASK-0709 deve produzir helper `prune_stale_redexes` e TEST-SPEC que separe os dois conceitos.
- **SC-010 / T13 distributed:** in-process MUST + Docker TCP SHOULD `#[ignore]`; partition strategy `round-robin` per SPEC-07 R3; decoder stage explícito. TASK-0715 carrega T13.

## 7. Insight do splitting (auditoria de HEAD)

5 dos 8 módulos previstos pela SPEC-27 já existem em HEAD:
- `traits.rs` (Encoder + Codec + EncodeContract + EncodeError + DecodeError) — auditar em TASK-0710
- `codec_church.rs` (`ChurchArithmeticCodec`) — auditar em TASK-0710
- `registry.rs` (`EncoderRegistry`, `default_registry`) — modificar em TASK-0716
- `recipe.rs` (`RecipeEncoder`) — auditar em TASK-0719
- `arithmetic.rs` com `wire_add_into`/`wire_mul_into` (linhas 92, 224) — promover em TASK-0711

**LambdaCodec ainda registrado no `default_registry` em HEAD** — a TASK-0716 substitui por HornerCodec.

Como consequência, várias tasks são audit-and-document/promotion-and-validation com baixo LoC, e a Phase 3a do SPEC-27 §6 (~150 LoC originalmente estimados) reduz-se a 0711 (audit/promote) + 0714 (encoder ~190 LoC) — sem nova construção dos helpers aritméticos.

## 8. Critérios de aceitação para Stage 2

O test-generator deve produzir, em `codigo/relativist/docs/tests/`:

1. Uma TEST-SPEC por TASK (11 arquivos, naming `TEST-SPEC-TASK-NNNN-titulo.md`)
2. Cada TEST-SPEC mapeia explicitamente para os IDs T1-T23 da §7 do SPEC-27 v3
3. Cobertura: todo MUST do SPEC-27 v3 mapeia para >= 1 caso de teste
4. Edge cases enumerados (R16' inteiro: empty coeffs, constant, x=0, all-zero, max coef boundary, overflow)
5. Property tests especificados quando aplicável (R10' QuickCheck-style; cross-check vs `horner_serial` oracle)
6. Distinção clara entre testes unitários, integration, property, e distributed (T13 sub-types)
7. Cada TEST-SPEC referencia o TASK correspondente e os requisitos R# que verifica
8. Atualizar `docs/pipeline-state.md` para refletir Stage 2 em progresso/concluído

## 9. Próximos stages (após test-generator)

```
Stage 3 — DEV       (developer agent, TDD RED→GREEN→REFACTOR, escreve src/ e tests/)
Stage 4 — REVIEW    (reviewer: code quality + architecture)
Stage 5 — QA        (qa: adversarial bug hunting)
Stage 6 — REFACTOR  (developer aplica fixes, verifica todos testes passam)
```

Topic 1 (Stress Curve, TASK-0700..0708) pode avançar Stages 2-6 em paralelo — não compartilha código com Topic 2.

## 10. Referências cruzadas

- **SPEC-27 v3:** `codigo/relativist/specs/SPEC-27-encoder-decoder-api.md`
- **Closure log Round 2:** `codigo/relativist/docs/spec-reviews/SPEC-27-v2-round2-response.md`
- **Round 1 critic:** `codigo/relativist/docs/spec-reviews/SPEC-27-round1-critic.md`
- **Closure log v2 inicial:** `codigo/relativist/docs/spec-reviews/SPEC-27-v2-closure-2026-05-06.md`
- **Handoff brief original Topic 2:** `codigo/relativist/docs/handoffs/2026-05-06-spec27-horner-revision-handoff.md`
- **Design doc Topic 2:** `codigo/relativist/docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md`
- **Explainer matemático:** `codigo/relativist/docs/superpowers/specs/2026-05-06-horner-method-explainer.md`
- **BACKLOG.md:** `codigo/relativist/docs/backlog/BACKLOG.md` (seção "SPEC-27 Encoder/Decoder API + HornerCodec (Topic 2)")
- **Predecessores (read-only):** SPEC-00, SPEC-01, SPEC-02, SPEC-03, SPEC-07, SPEC-14, SPEC-25
- **ARG-001:** `discussoes/argumentos/ARG-001-confluencia-preserva-determinismo.md` (G1 com P1 engine + P3/P4 preconditions, conforme SC-009)

---

**Pergunta do usuário ao receber esta brief:** abrir Claude Code rooted em `codigo/relativist/`, invocar o `sdd-pipeline` agent, e colar o briefing da §2 desta página.
