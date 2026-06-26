# 10 · Arena Management (`--recycle-policy`, `--representation`)

Guia para o **arena management** especificado em [SPEC-22](../../docs/specs/SPEC-22-arena-management.md). SPEC-22 introduz dois mecanismos independentes para evitar que o consumo de memoria cresca alem do necessario durante reducao distribuida: a **free-list** que reaproveita slots de agentes consumidos, e a **SparseNet** que evita tombstones quando a arena densa ficaria cheia de buracos.

> **Status:** ativo por default desde v0.20-pre. As decisoes de free-list e dense vs sparse acontecem **automaticamente** com defaults seguros (`disable-under-delta` para recycle, `dense` para representation com fallback para sparse acima do threshold). Os flags existem para benchmarks/experimentos; o usuario tipico nao precisa toca-los.

## 1. Quando importa

Memoria de arena vira problema quando:

- A reducao tem **fase de expansao pesada** (CON-DUP commutation: 2 agentes consumidos -> 4 criados). Sem reuso, `next_id` cresce monotonicamente e a arena densa segue junto.
- A rede e **grande** (M5: 100M agentes) e o coordinator tem **memoria limitada** (laptops: 16-32 GiB). Um `vec![None; 100M]` por particao ja ocupa ~800 MB com 0 dados uteis.
- Voce esta rodando **delta mode** ou **streaming**, onde o coordinator mantem refs longas para `AgentId`s especificos (border map). Nesse caso, recyclar errado *quebra G1*.

Para redes pequenas (<1M agentes) e workloads sem expansao agressiva (puro `ep_annihilation`), o impacto de memoria e marginal e os defaults bastam.

## 2. Ideia central

### Free-list (SPEC-22 §3.1)

Cada `Net` carrega `free_list: Vec<AgentId>`. Quando `remove_agent(id)` e chamado:

1. Marca `agents[id] = None` e desconecta as portas (SPEC-02 R12).
2. **Push** `id` no free-list.

Quando `create_agent(symbol)` e chamado:

1. Se o free-list nao esta vazio: **pop** um ID, reusa o slot, NAO incrementa `next_id`.
2. Senao: aloca novo slot via `next_id` (comportamento v1).

LIFO maximiza temporal locality (slots quentes em cache). A invariant key e que **um ID na free-list nao pode ser referenciado por nenhum `PortRef::AgentPort`** (SPEC-22 R7) — caso contrario, recyclar quebra os ponteiros.

### SparseNet (SPEC-22 §3.2)

Estrutura alternativa baseada em `HashMap<AgentId, Agent>` + `HashMap<(AgentId, PortId), PortRef>`. Memoria **estritamente proporcional aos agentes vivos** — sem tombstones. Trade: O(1) amortizado vs O(1) garantido da arena densa, e ~5-10x cache misses no hot path da reducao.

Por isso a regra de design: **SparseNet para construcao/particionamento; Net densa para o loop de reducao**. SPEC-22 R23 mantem isso via lint de CI (`src/reduction/**` nao pode importar `SparseNet`).

## 3. Decisao de recycle: `--recycle-policy`

A flag controla o que acontece quando o worker quer popar do free-list mas existe risco de quebrar a `BorderGraph` (delta mode) ou o `border_map` do streaming pipeline.

| Valor                       | Comportamento                                                                          | Default? |
|-----------------------------|----------------------------------------------------------------------------------------|----------|
| `disable-under-delta`       | Worker NAO popa do free-list enquanto `(delta_mode \|\| streaming_active)`. Strategy A.  | sim      |
| `border-clean`              | Worker popa, mas valida que o ID nao esta em `border_referenced_set`. Strategy B.       | nao      |
| `disable`                   | Recycle desligado em todo cenario (free-list so acumula pushes; nunca popa).            | nao      |

```bash
# Default — safe, zero risco de quebrar G1 sob delta/streaming
relativist coordinator --workers 4 --port 9000 -i net.bin

# Opt-in para medir o ganho de recycle sob delta mode
relativist bench --benchmark con_dup_expansion --sizes 5000 \
    --workers 1,2,4 --recycle-policy border-clean

# Conservadorismo absoluto (debugging, repro de bugs antigos)
relativist bench --benchmark ep_annihilation --sizes 1000000 \
    --recycle-policy disable
```

A enum wire-level se chama `RecyclePolicy::DisableUnderDelta` por compatibilidade — mesmo apos SPEC-21 §3.8 A6 generalizar o trigger para `(delta_mode || streaming_active)`. Internamente o nome conceitual e "DisableUnderBorderTracking" mas o nome publico nao mudou.

### Alternativa de compile-time

```bash
# Desabilita recycle outright durante streaming/delta, sem ler RecyclePolicy
cargo build --release --features streaming-no-recycle
```

Essa feature foi prevista em SPEC-21 R37b como "valid one-liner closure" — substitui a logica runtime por um `#[cfg]` que sempre cai no `next_id` quando `is_in_delta_round` e true. Util para testes onde voce quer descartar variavel de configuracao.

## 4. Decisao dense vs sparse: `--representation`

A flag aplica-se ao **bench harness**: ela controla qual representacao e usada na fase de construcao das particoes (`build_subnet`).

| Valor      | Comportamento                                                                  | Default? |
|------------|--------------------------------------------------------------------------------|----------|
| `dense`    | Usa o `Net` denso para construcao.                                             | sim      |
| `sparse`   | Usa `SparseNet` para construcao; converte para denso antes da reducao via `to_dense`. | nao  |

```bash
# Status quo — usa dense path; o threshold automatico (R22) decide quando precisa cair pra sparse
relativist bench --benchmark dual_tree --sizes 18,20,22 \
    --workers 1,2,4,8 --representation dense

# Forca sparse path — util para o D-011 Phase D micro-bench (acceptance gate exige
# sparse < 0.8x dense em dual_tree, SPEC-22 R22a)
relativist bench --benchmark dual_tree --sizes 22 \
    --workers 1,2,4,8 --representation sparse \
    --csv-sparse results/sparse_construction_memory.csv
```

Em `coordinator`/`local`, a representacao e decidida automaticamente pelo `effective_arena_size` threshold (proximo paragrafo) — nao ha flag direta, e isso e proposital: o usuario pediria sparse "porque ouviu falar" e quebraria performance em healthy workloads.

## 5. O threshold `effective_arena_size` (D-011 fix)

A regra automatica para escolher dense vs sparse mora em SPEC-22 R22:

```
effective_arena_size := max_live_id + 1
                     // == max(worker_agents) + 1, com max() sobre o slice de IDs vivos

partition.live_agent_count := worker_agents.len()

se effective_arena_size > 4 * live_agent_count:
    use SparseNet (apoia M5 e ID-fragmented workloads)
senao:
    use Net densa (status quo, otima para healthy nets)
```

### Antes do D-011 (bug)

A formula original era `id_range > 4 * live_agent_count`, onde `id_range = id_range.end - id_range.start` vinha do **planning range** alocado por `compute_id_ranges` (`base_next_id * 10` por particao). Em healthy workloads (agentes densamente empacotados em IDs baixos), o planning range era 5-800x o live count e roteava **toda particao** pelo branch sparse, produzindo +83% de wall-clock regression em `ep_annihilation_con 5M w=2` (12 s -> 22 s, comprovado por bisect 7-pontos).

### Depois do D-011 fix (v2.4)

Substituiu-se `id_range_size` por `effective_arena_size = max_live_id + 1`, que mede a *arena que o caminho denso realmente alocaria* (`vec![None; max_live_id + 1]`) em vez do upper bound do planejamento. O fator `4x` ficou identico — apenas a metrica que multiplica mudou.

Sob workloads "M5-like" (CON-DUP-dominado, free-list disabled, fragmentacao de `next_id`), o threshold fira corretamente: `live_count = 10M`, `max_live_id = 100M` -> `effective_arena_size = 100M > 40M` -> sparse path, evita os 800 MB de tombstones.

A regra e **build-time only** (R22 closes SC-023): ela e avaliada **uma vez** na entrada de `build_subnet`. Crescimento de arena durante a reducao local nao re-checa o threshold; isso e governado por SPEC-02 R11 + SPEC-22 R3/R4 (free-list-first).

Detalhes empiricos no closure log [`docs/_archive/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`](../_archive/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md).

### Forcar caminho dense quando o threshold pediria sparse

```bash
# bench: PartitionConfig.sparse_build = false; se threshold disparar, retorna PartitionError
relativist bench --benchmark ep_annihilation --sizes 100000000 \
    --representation dense
# -> falha cedo com DenseAllocationExceedsThreshold em vez de OOM
```

## 6. Defaults seguros — quando nao mexer

A combinacao default cobre 100% dos benchmarks travados de v2:

```
--recycle-policy disable-under-delta
--representation dense   (com fallback automatico para sparse acima do threshold)
```

Esses defaults preservam G1, evitam o regression de +83% identificado pelo D-011, e mantem zero crashes nos 32/32 slots distribuidos do baseline canonico ([`v2_post_d012_baseline_2026-05-05/`](../../reproduce_article/results/locked/v2_post_d012_baseline_2026-05-05/)).

Mude apenas se voce esta:

1. Medindo deliberadamente o overhead/ganho de uma alternativa (paper, micro-bench, pre-release validation).
2. Reproduzindo um bug antigo num branch onde a logica era diferente.
3. Rodando uma rede tao grande que `dense` sempre vai bater no threshold (M5+) — ai nao e questao de mexer flag, e questao de aceitar que o sparse path vai ser usado.

## 7. Wire compatibility

A introducao do `free_list: Vec<AgentId>` no `Net` serializado **quebrou compatibilidade de wire**: workers/coordinator pre-SPEC-22 nao deserializam `Net` com free-list nao-vazio. SPEC-22 R9a (referenciado em SPEC-18 §3.8 A9) bumpou `PROTOCOL_VERSION` de 2 para 3.

Persisted v1/v2 `.bin` files (ex.: `reproduce_article/results/locked/v1_local_baseline/*.bin`) ficam **ilegiveis para binarios v3+**. Aceitavel — esses arquivos sao baselines congelados que nao alimentam codigo v2/v3. Para regerar inputs com schema novo, basta rodar `relativist generate` com o binario atual.

## 8. Limitacoes

- **`SparseNet` nao e usada no hot path.** Por design (R23). O lint do CI (`cargo clippy`) reforca via grep de imports em `src/reduction/`. Mudar isso exige mudar o spec.
- **Free-list LIFO unico.** SPEC-22 R5 fixa LIFO; nao ha plano de oferecer FIFO/random como flag. Justificativa: temporal locality vence locality espacial em todos os micro-benches medidos (REF-014 Kahl).
- **`SparseNet::to_dense(None)` aloca `vec![None; max_id + 1]`.** Significa que se voce converter um sparse com `max_id = 100M` e so 10M vivos, o resultado denso ainda tem 90M tombstones. Use `to_dense(Some(id_range))` para limitar o range alocado (SPEC-22 R20 contract).
- **Acceptance gate em D-011 Phase D so cobre `dual_tree`.** Outros geradores nao tem o threshold validado por micro-bench dedicado; foram validados pelo gate de regressao de ±5% (CONTRIBUTING.md).
- **Free-list em workers nao reusa entre particoes.** Cada worker tem seu free-list local; quando o merge agrega particoes, o free-list do net mergeado e reconciliado por SPEC-05 R12 (so IDs que ainda batem em `None` no merged arena permanecem). Cross-partition reuse nao e otimizado.

## 9. Proximo passo

- [SPEC-22 v2.4](../../docs/specs/SPEC-22-arena-management.md) — especificacao completa com todas as amendments do D-011 (R22 + R22a + R30 + §3.8 A9-A11).
- [09-streaming-generation.md](09-streaming-generation.md) — SPEC-21 streaming, complementar: a interacao R10b/c com streaming e o que torna o threshold relevante na pratica.
- [08-elastic-grid.md](08-elastic-grid.md) — SPEC-20 elastic grid, complementar: ARG-006 mixed-trace recoverability sob departure depende de `RecyclePolicy::DisableUnderDelta` para fechar o caso delta-mode.
- [`docs/_archive/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md`](../_archive/spec-reviews/SPEC-22-amendment-2026-05-04-d011-blocker.md) — closure log do D-011 fix do threshold.
