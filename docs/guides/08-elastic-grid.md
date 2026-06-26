# 08 · Elastic Grid (`--hybrid`, `--elastic-join`, `--elastic-departure`)

Guia para o **Elastic Grid**, especificado em [SPEC-20](../../docs/specs/SPEC-20-elastic-grid.md). SPEC-20 introduz tres recursos que tornam o conjunto de nos participantes **dinamico** durante uma reducao distribuida: o coordinator pode atuar como worker, novos workers podem entrar entre rodadas BSP, e workers podem sair (graceful ou por timeout) sem matar o run.

> **Status:** parcialmente entregue. As flags da CLI estao no parser e os campos do `GridConfig` ja sao consumidos pelo coordinator. A semantica completa de SPEC-20 (recovery sob delta mode, retain-partitions, M5 elastico) ainda esta sendo coberta por testes de integracao na v2; o uso fora do `local` mode deve ser tratado como **experimental** ate o fechamento de [SPEC-20 §3.3 ARG-005](../../docs/specs/SPEC-20-elastic-grid.md). Default continua o comportamento estatico de v1 (zero regressao).

## 1. Quando usar

O grid elastico e util quando:

- Voce esta em uma **maquina unica** (laptop, desktop) e quer aproveitar o core do coordinator: ative `--hybrid` para que ele tambem reduza uma particao em vez de ficar idle entre rodadas.
- Voce tem um pool de **maquinas heterogeneas/intermitentes** (laboratorio universitario, BOINC-style) onde os clientes se conectam quando podem: `--elastic-join` permite que workers entrem em rodadas subsequentes sem reiniciar o run.
- Voce precisa de **resiliencia minima**: `--elastic-departure` reaproveita o trabalho de um worker que caiu, redistribuindo a particao retida entre os sobreviventes.

Se a sua malha e **fixa** e dimensionada antes do run (Phase 2 docker, Phase 3 LAN com `--workers N` igual a contagem real de workers), as flags elasticas adicionam complexidade sem ganho — mantenha o default.

## 2. Ideia central

### v1 (default) — malha estatica

```
coord espera N workers conectarem -> entrega particoes -> recebe -> merge -> repete
                       |
                       v
              malha fixa por todo o run
```

Se um worker cai, o run aborta. Se o coordinator tem CPU sobrando, ele fica ocioso entre as fases de merge.

### v2 elastica (opt-in)

```
coord cria K_eff = K + 1 slots (em modo hybrid)
   -> 1 self-partition no proprio coord (in-process via ChannelTransport)
   -> K particoes entregues a workers remotos

entre rodadas:
   - drena conexoes pendentes (join window)
   - aplica LeaveRequest pendentes
   - se um worker silenciou alem do timeout, redispatcha sua retained_partition
```

Tres componentes:

- **Hybrid Coordinator (2.1):** o coordinator instancia um worker in-process. Toda a logica de partition/merge segue identica; o self-worker aparece para o FSM como mais uma conexao via `ChannelTransport` (SPEC-17 R15).
- **Dynamic Joining (2.2):** entre `merge` e o proximo `partition`, o coord drena `accept()` pendentes (limites `join_window_min_ms` / `join_window_max_ms`) e os workers novos recebem particoes na rodada seguinte.
- **Dynamic Departure (2.3):** workers podem mandar `LeaveRequest{kind: AfterResult|Urgent}` ou simplesmente parar de responder. O coord retem a `retained_partition` (formato depende de v1 vs delta mode) e a redispatcha entre os sobreviventes.

## 3. Componentes

### Hybrid mode

`WorkerId = 0` e reservado **permanentemente** para o self-partition do coordinator quando `--hybrid` esta ativo. Workers remotos comecam em `WorkerId = 1`. Em modo nao-hybrid, `WorkerId = 0` continua designado ao primeiro worker remoto, por compatibilidade com testes de v1.

`K_eff = K + 1` substitui `K` em todos os calculos de slot e ID range (SPEC-04 R16-R19). Ou seja: a particao do coord nao "come" o slot de um worker — ela e um slot adicional.

### Solo mode

Em hybrid mode, se nenhum worker conectar dentro de `--initial-wait-timeout` (default 30 s), o coord entra em `SoloReducing`: roda `reduce_n(net, solo_budget)` em loop, polando o event loop async entre batches para detectar joins. Default `--solo-budget = 10000` interacoes por batch — trade entre overhead de poll e responsividade de join.

### Retained partitions

Quando `--elastic-departure` esta ativo, o coord mantem dois snapshots por worker:

- `retained_initial`: particao da rodada 0 (usada se o worker cair antes de mandar qualquer resultado).
- `retained_last_acked`: o resultado mais recente comitado (em delta mode, par `(BorderGraph snapshot, last RoundResult)`; em v1 mode, ultimo `PartitionResult.partition`).

Em departure, o coord redispatcha `retained_last_acked` (ou `retained_initial` se nada foi recebido ainda) para outro worker.

## 4. Uso na CLI

> **Nota de status.** As flags abaixo sao parseadas e propagadas para `GridConfig`. A entrega completa SPEC-20 sob `coordinator`/`worker` real (TCP) ainda esta endurecendo — para experimentos academicos, use `local --hybrid` que cobre 100% das transicoes em-processo via `ChannelTransport`. O modo `coordinator` aceita as flags hoje, mas casos extremos (catastrophic departure mid-round, M5-scale recovery) sao cobertos por testes mas nao por benchmarks travados.

### Modo `local` (in-process; mais coberto)

```bash
# Hybrid: 4 workers logicos no total = 1 self-partition + 3 in-process workers
relativist local --workers 3 --hybrid -i net.bin -o out.bin

# Solo + join: comecar sem workers, deixar workers entrarem (so faz sentido com loop de teste)
relativist local --workers 0 --hybrid --elastic-join \
    --initial-wait-timeout 30 --solo-budget 10000 -i net.bin

# Hybrid + departure recovery
relativist local --workers 4 --hybrid --elastic-departure -i net.bin
```

### Modo `coordinator` (TCP real)

```bash
relativist coordinator --workers 4 --port 9000 --hybrid \
    --elastic-join --elastic-departure \
    --join-window-min-ms 50 --join-window-max-ms 500 \
    -i net.bin -o out.bin
```

Workers remotos nao precisam de flag — o coordinator anuncia o modo no handshake.

### Flags relacionadas

| Flag                          | Default | Descricao                                                                  |
|-------------------------------|---------|----------------------------------------------------------------------------|
| `--hybrid`                    | off     | Ativa self-partition no coordinator. Reserva `WorkerId = 0`.               |
| `--elastic-join`              | off     | Drena conexoes pendentes entre rodadas. Auto-ativada com `--hybrid` ou `--elastic-departure`. |
| `--elastic-departure`         | off     | Recupera particoes de workers que caem. Auto-ativa `--retain-partitions`.  |
| `--retain-partitions`         | off     | Forca o coord a guardar `retained_initial` + `retained_last_acked`.         |
| `--checkpoint-partitions`     | off     | Persiste retained partitions em disco (planejado; flag aceita).             |
| `--initial-wait-timeout`      | 30 (s)  | Janela inicial antes de entrar em solo (so com `--hybrid`).                 |
| `--join-window-min-ms`        | 50      | Janela minima para drenar joins entre rodadas (ms).                         |
| `--join-window-max-ms`        | 500     | Janela maxima.                                                              |
| `--solo-budget`               | 10000   | Interacoes por batch no `SoloReducing`. `u32::MAX` desativa polling.        |

## 5. Exemplo guiado — worker entra mid-run (modo `local`)

Como o `local` mode mantem tudo in-process, da para demonstrar o ciclo elastico em um unico terminal.

```bash
# 1. Gere uma rede pequena para o experimento
relativist generate dual-tree -d 10 -o tree.bin

# 2. Rode em hybrid + join, com janela inicial curta para forcar SoloReducing primeiro
#    O scheduler fica esperando workers; passada a janela, o coord entra em solo.
relativist local --workers 0 --hybrid --elastic-join \
    --initial-wait-timeout 5 --solo-budget 1000 \
    -i tree.bin -o out.bin -m metrics.json

# 3. Inspecione metrics.json:
#    - "rounds[0].mode" = "Solo"  (entrou em solo no startup)
#    - eventos de join sao registrados em "elastic_events" (quando joins ocorrem em-processo via spawn)
```

Em `coordinator` real, a mesma sequencia se faz com dois terminais: o coord sobe vazio e entra em solo; um worker e lancado depois e o coord o aceita na proxima janela de join.

## 6. Invariantes e correctness

SPEC-20 nao introduz teoremas novos — toda a recoverability e justificada por strong confluence (T4) + os argumentos:

- **ARG-001** (P1-P6): confluence preserva determinismo, base de tudo.
- **ARG-006** (mixed-trace recoverability): proof formal de que reduzir uma `retained_partition` em outro worker, na rodada N+1, produz isomorfismo com o resultado sequencial. CLOSED para v1 mode; CONDITIONAL em delta mode (depende de ARG-005, com fallback conservador via `RecyclePolicy::DisableUnderDelta`, ver [guia 10](10-arena-management.md)).

G1 e D6 valem identicos; o que muda e a complexidade do FSM do coordinator (estados `SoloReducing`, `Departing`, `Joining`).

## 7. Limitacoes conhecidas

- **Phase 3 LAN ainda nao mediu o ganho do hybrid.** Em localhost (Phase 2 docker), o ganho do hybrid e teoricamente ~1/(K+1) extra de paralelismo, mas a maioria dos slots de v1 ja sao **negative-strong-scaling** ([analise D-011 §RF-06](../analysis/D011-final-baseline-analysis-2026-05-04.md)) — entao a feature precisa de Phase 3 LAN para mostrar valor real.
- **Mode immutability per run (R0c).** `delta_mode` e `strict_bsp` ficam fixos no startup. Mudar mid-run e proibido — a forma do `retained_state` muda entre v1 e delta.
- **`--checkpoint-partitions` e placeholder.** A flag e parseada mas a persistencia em disco esta em backlog (TASK-0560+). Use apenas em-processo por ora.
- **WAN partitions nao sao cobertas.** Recovery de particionamento de rede (split-brain) esta fora de escopo SPEC-20; cobertura prevista em SPEC-24 (WAN).

## 8. Proximo passo

- [SPEC-20](../../docs/specs/SPEC-20-elastic-grid.md) — especificacao formal completa (R1-R45, FSM diagrams, mode matrix v1/delta × lenient/strict, ARG-006 mixed-trace recoverability).
- [09-streaming-generation.md](09-streaming-generation.md) — SPEC-21 streaming pipeline (complementar: o elastic grid permite que workers entrem; o streaming permite que a malha receba uma rede grande sem materializa-la).
- [10-arena-management.md](10-arena-management.md) — SPEC-22 arena/free-list (interage com elastic departure: `RecyclePolicy::DisableUnderDelta` e o fallback que torna ARG-006 valido em delta mode).
