# Invariantes — G1, D1-D6, T1-T7, I1-I5

Tabela de referencia rapida. A fonte de verdade formal e [SPEC-01 Invariants](../../docs/specs/SPEC-01-invariantes.md); amendments do bundle 2.26 (delta protocol) estao em [SPEC-19 §3.5](../../docs/specs/SPEC-19-delta-protocol.md#35-invariant-amendments).

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

Para detalhes formais, a prova de D3d (equivalencia delta vs merge completo) esta marcada como *pending formal proof* na [SPEC-19 §8](../../docs/specs/SPEC-19-delta-protocol.md).

## v2 Amendments (post-SPEC-19)

Alem dos amendments G1/D3/D6 ja descritos acima (bundle 2.26 do delta protocol), v2 acumulou tres mudancas adicionais nos invariantes que merecem destaque na referencia rapida.

### I3 -> I3' — Uniqueness em vez de Monotonicity (D-009 amendment, SPEC-22 §3.1)

**I3 (v1).** AgentIds sao alocados por `next_id` incremental, monotono crescente. Um ID nunca e reusado.

**I3' (v2).** AgentIds sao **unicos** mas **nao necessariamente monotonos**. Quando o free-list de SPEC-22 pop um slot, o ID retorna ao mundo dos agentes vivos — quebrando monotonicity, mas preservando uniqueness (R6: o free-list nao contem duplicatas; R7: um ID na free-list nao e referenciado por nenhum `PortRef::AgentPort`).

| Tag  | v1 (Monotonicity)                            | v2 (I3' — Uniqueness)                                                  |
|------|----------------------------------------------|------------------------------------------------------------------------|
| I3   | Cada novo agente tem ID > todos anteriores. | Em qualquer instante, a relacao `id -> agent` e funcional (sem conflito). |

A relaxacao e formal: SPEC-22 §3.1 amenda SPEC-01 I3, SPEC-02 R2 ("never reused" -> "uniqueness via free-list"), SPEC-02 R10 ("incremented by k" -> "incremented by the count of non-recycle creations") e SPEC-03 §4.3 (debug-assertion language reformulada). SPEC-21 R15 mantem monotonicity como contrato **de geracao** (estritamente mais forte que I3'); apenas codigo pos-dispatch precisa assumir I3' em vez de I3.

### R10b/c — Free-list recycle preconditions (SPEC-22, ampliado por SPEC-21 §3.8 A6)

A correcao de G1 sob recycle exige que slots referenciados por estruturas de tracking de borda **nao sejam recyclados**. SPEC-22 R10b/c formaliza:

- **R10b (Strategy A — disable):** quando `(delta_mode || streaming_active)`, o worker NAO popa do free-list dentro de uma rodada. `create_agent` cai em `next_id`. O free-list ainda acumula pushes; e drenado no proximo clean boundary (`reconstruct` apos `FinalStateRequest`).
- **R10b (Strategy B — border-clean):** o worker popa apenas se o ID nao esta no `border_referenced_set` da particao (locally inspectable em O(1) via `HashSet<AgentId>` shadow). Se o ID e border-referenced, re-push para a free-list e aloca novo via `next_id`.
- **R10c (protected tombstones):** quando `remove_agent(id)` e chamado e `id` esta no border_referenced_set, as portas sao desconectadas, o slot vira `None`, **mas o ID NAO e pushed para o free-list** — o slot vira protected tombstone ate o proximo `reconstruct`.

A escolha A vs B e o `GridConfig.recycle_under_delta: RecyclePolicy` (default `DisableUnderDelta`). SPEC-21 §3.8 A6 amplia o trigger de R10b/c de `delta_mode == true` para `(delta_mode || streaming_active)` — o `border_map` do streaming pipeline expoe a mesma vulnerabilidade que a `BorderGraph` do delta protocol.

A feature flag `streaming-no-recycle` (`Cargo.toml [features]`) implementa Strategy A em compile-time, eliminando a necessidade de ler `RecyclePolicy` runtime.

### `effective_arena_size` — D-011 metric correction (SPEC-22 v2.4)

SPEC-22 R22 governa quando `build_subnet` cai no caminho `SparseNet`. A formula original:

```
id_range > 4 * partition.live_agent_count
```

usava `id_range = id_range.end - id_range.start` (vindo de `compute_id_ranges`, alocado como `max(100_000, base_next_id × 10)` por particao). Em healthy workloads (agentes densamente empacotados em IDs baixos), o planning range era 5-800x o live count e roteava **toda** particao pelo branch sparse, produzindo +83% de wall-clock regression em `ep_annihilation_con 5M w=2`.

A correcao do D-011 amendment 2026-05-04 substitui a metrica:

```
effective_arena_size := max_live_id + 1
                     // == max(worker_agents) + 1

se effective_arena_size > 4 * live_agent_count: SparseNet
senao:                                          Net densa
```

`effective_arena_size` mede a arena que o caminho denso **realmente alocaria** (`vec![None; max_live_id + 1]`), nao o upper bound do planejamento. Empiricamente: o regression desapareceu sem reintroduzir o pathology M5 que motivou o threshold (R22a documenta o mecanismo de fragmentacao `next_id` que ainda dispara o threshold corretamente).

Closure log: [`docs/_archive/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`](../_archive/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md). Consumidor: [guia 10-arena-management.md](../guides/10-arena-management.md) §5.

## Ver tambem

- [SPEC-01](../../docs/specs/SPEC-01-invariantes.md) — definicoes formais completas
- [SPEC-19 §3.5](../../docs/specs/SPEC-19-delta-protocol.md) — amendments G1/D3/D6
- [SPEC-22 §3.1 / §3.8](../../docs/specs/SPEC-22-arena-management.md) — I3' uniqueness, R10b/c free-list, R22 effective_arena_size
- [SPEC-21 §3.7 / §3.8](../../docs/specs/SPEC-21-streaming-generation.md) — A6 trigger broadening para R10b/c, R37b feature gate
- [../guides/06-delta-protocol.md](../guides/06-delta-protocol.md) — guia didatico do delta protocol
- [../guides/09-streaming-generation.md](../guides/09-streaming-generation.md) — guia didatico do streaming
- [../guides/10-arena-management.md](../guides/10-arena-management.md) — guia didatico de free-list + dense/sparse
