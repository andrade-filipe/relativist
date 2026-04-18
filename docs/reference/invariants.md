# Invariantes — G1, D1-D6, T1-T7, I1-I5

Tabela de referencia rapida. A fonte de verdade formal e [SPEC-01 Invariants](../../specs/SPEC-01-invariantes.md); amendments do bundle 2.26 (delta protocol) estao em [SPEC-19 §3.5](../../specs/SPEC-19-delta-protocol.md#35-invariant-amendments).

## G1 — Propriedade Fundamental

**G1 (v1).** Para toda rede terminante `N` e todo numero de workers `n >= 1`:

```
reduce_all(N)  ≅  run_grid(N, n)
```

onde `≅` denota isomorfismo estrutural modulo renomeacao de IDs. Esta e a propriedade central do Relativist — distribuicao nao altera o resultado.

**G1 (amendment SPEC-19 R38).** Para `delta_mode=true`:

```
reduce_all(N)  ≅  run_grid_delta(N, n)
```

A correcao da decomposicao apoia-se em strong confluence (T4): em qualquer ponto da BSP delta, o estado distribuido `(BorderGraph, worker_partitions)` reconstroi (modulo isomorfismo) o estado sequencial `mu_k` apos k interacoes.

Verificacao operacional: `nets_isomorphic(seq, dist)` ou (quando decode e possivel) `decode_nat_or_shared(seq) == decode_nat_or_shared(dist)`.

---

## T1-T7 — Teoremas de Lafont e premissas do TCC

| Tag | Propriedade                       | Fonte              |
|-----|-----------------------------------|--------------------|
| T1  | Universalidade de {CON, DUP, ERA} | Lafont 1997        |
| T2  | Confluencia local (local)         | Lafont 1997        |
| T3  | Church-Rosser                     | Lafont 1997        |
| T4  | **Strong confluence**             | Lafont 1997        |
| T5  | Terminacao em subconjuntos        | P6 (TCC, qualificada) |
| T6  | Linearidade de regras              | 6 regras fixas      |
| T7  | Forma normal unica (terminante)   | T3 + T4 + T5       |

T4 e o alicerce operacional: **dois redexes nao-sobrepostos podem ser reduzidos em qualquer ordem com o mesmo resultado**. Isso e o que permite workers reduzirem em paralelo sem coordenacao.

---

## D1-D6 — Protocolo de Distribuicao

Invariantes sobre `split()`, `merge()` e o ciclo BSP.

| Tag | Propriedade                                     | v1                  | v2 (amendment)       |
|-----|-------------------------------------------------|---------------------|----------------------|
| D1  | Agent preservation: `|split(N)| = |N|`          | SPEC-04             | (inalterado)         |
| D2  | Wire preservation: toda wire de `N` esta em `split(N)` | SPEC-04         | (inalterado)         |
| D3  | **Border completeness**                         | SPEC-04 R12         | **R39** (incremental via BorderGraph) |
| D4  | Commutation preservation (CON-DUP cross-border) | SPEC-05 R8          | (inalterado)         |
| D5  | ID consistency                                  | SPEC-04 R7          | (inalterado)         |
| D6  | **Protocol termination**                        | SPEC-05 R30         | **R40** (3-conjunct, DC-C5) |

### D3 (v1) — Border completeness

Apos `merge(split(N))`, toda redex cross-border de `N` e detectada pela fase RESOLVE BORDERS.

### D3 (amendment R39) — Incremental via BorderGraph

- **D3a (amended):** `BorderGraph.detect_border_redexes()` usa o flag `is_redex` por borda. O flag e recomputado a cada `apply_deltas` (R11).
- **D3c (unchanged):** Redexes emergentes via CON-DUP commutation sao reportados pelo worker no proximo delta.
- **D3d (amended):** Equivalencia formal: para todo border redex detectavel por merge completo, o BorderGraph detecta o mesmo redex na mesma rodada (dado correct delta reporting).

### D6 (v1) — Termination via fila vazia

O protocolo termina quando o net mergeado tem fila de redexes vazia.

### D6 (amendment R40) — Three-conjunct convergence (DC-C5)

O protocolo delta termina quando **os tres** abaixo sao verdadeiros na mesma rodada:

1. Todos os workers reportam `zero local redexes`.
2. O `BorderGraph` tem `zero active pairs`.
3. Nenhum delta novo foi reportado.

Equivalente ao v1 sem reconstruir o net.

---

## I1-I5 — Invariantes de Implementacao

Invariantes estruturais do codigo (SPEC-02, SPEC-03).

| Tag | Invariante                                              |
|-----|---------------------------------------------------------|
| I1  | Arity: cada simbolo tem aridade fixa (CON=3, DUP=3, ERA=1) |
| I2  | Port uniqueness: cada porta ocupada por no maximo uma wire |
| I3  | Symmetric wires: `wire(a, b)` <=> `wire(b, a)`          |
| I4  | No dangling: toda porta ou e wire ou e FreePort         |
| I5  | Redex detection: `is_redex(a, b) <=> a.principal <-> b.principal` |

---

## Como esses invariantes sao testados

- **G1:** cada datapoint de benchmark roda `nets_isomorphic(seq, dist)`. Toda campanha congelada (v1_local_baseline, v1_stress) teve **zero `correct=false`** em 4490 execucoes.
- **D3/D6:** cobertos por testes property-based sobre redes geradas (SPEC-08 test strategy).
- **I1-I5:** invariant checks ativos sob `debug_assertions`; podem ser desativados em release por configuracao (feature-gated).
- **G1 (amendment):** testes novos da SPEC-19 cobrem delta_mode=true contra delta_mode=false como oracle.

Para detalhes formais, a prova de D3d (equivalencia delta vs merge completo) esta marcada como *pending formal proof* na [SPEC-19 §8](../../specs/SPEC-19-delta-protocol.md).

## Ver tambem

- [SPEC-01](../../specs/SPEC-01-invariantes.md) — definicoes formais completas
- [SPEC-19 §3.5](../../specs/SPEC-19-delta-protocol.md) — amendments G1/D3/D6
- [../guides/06-delta-protocol.md](../guides/06-delta-protocol.md) — guia didatico do delta protocol
