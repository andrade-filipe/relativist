# 09 · Streaming Generation (`--chunk-size`)

Guia para a **geracao e particionamento em streaming**, especificada em [SPEC-21](../../docs/specs/SPEC-21-streaming-generation.md). SPEC-21 substitui o pipeline eager de v1 (`gerar rede inteira -> particionar -> dispatch`) por um pipeline incremental (`gerar chunk -> particionar chunk -> dispatch chunk -> repetir`), limitando o pico de memoria do coordinator a **O(chunk_size + border_state)** em vez de O(total_agents).

> **Status:** ativo por default em `coordinator` e `local` desde v0.20-pre. O parametro `--chunk-size` tem default `10000`. Para forcar o caminho eager v1-equivalente, use `--chunk-size 4294967295` (`u32::MAX`) ou — no `bench` — omita a flag. A correctness depende de SPEC-22 R10b/c (free-list recycle precisa estar protegida sob streaming); ver §6.

## 1. Quando usar

Streaming compensa quando:

- A rede tem **>10M agentes** e voce nao quer alocar `~64 bytes * total_agents` no coordinator antes do primeiro dispatch.
- O gerador e **naturalmente incremental** (ex.: `ep_annihilation` emite pares ERA-ERA independentes; `dual_tree` tem dependencias forward referenciadas mas resolviveis batch-a-batch).
- Voce esta rodando o `coordinator` em uma maquina com pouca RAM e workers em maquinas mais robustas (a memoria peak deveria ficar nos workers, nao no orquestrador).

Para redes pequenas (<100k agentes) o ganho e inexistente e o overhead de gerencia de chunks pesa mais que a economia de memoria. Default `--chunk-size 10000` ja e razoavel para esse threshold.

## 2. Ideia central

### v1 (eager)

```
gerador.make_net(size) -> Net completo (peak: O(total_agents))
   -> split(Net, num_workers) -> Vec<Partition>
   -> dispatch
```

A `Net` materializada vive no coordinator inteira ate o split terminar. Para 100M agentes (~6 GB), isso simplesmente nao cabe em laptops/CI.

### v2 streaming (default)

```
gerador.make_net_stream(size, chunk_size) -> Iterator<AgentBatch>

para cada batch:
   strategy.allocate_batch(batch, num_workers)
       -> mapeamento AgentId -> WorkerId
   para cada agente do batch:
       insere no PartitionAccumulator do worker correspondente
   para cada conexao resolvida:
       wire interno (mesmo worker) ou border (workers diferentes)
   para cada conexao pendente (forward ref):
       buffer no pending store, resolve quando o target aparecer
finalize: cada PartitionAccumulator vira uma Partition concreta
```

Pico de memoria do coord: `O(chunk_size) + O(borders) + O(|pending|)` — bounded por `--max-pending-lifetime`.

## 3. Como ativar

### Default (todos os subcomandos com workers)

`coordinator`, `local` e `bench` aceitam `--chunk-size` com default `10000`. Em `coordinator`/`local`, basta deixar o default. Em `bench`, omitir o flag desativa streaming (path eager); passar o flag ativa.

```bash
# coordinator: streaming implicito (default chunk_size=10000)
relativist coordinator --workers 4 --port 9000 -i big_net.bin -o out.bin

# local: idem
relativist local --workers 4 -i big_net.bin -o out.bin

# bench: opt-in via flag explicito
relativist bench --benchmark ep_annihilation --sizes 5000000 \
    --workers 1,2,4,8 --mode tcp_localhost \
    --chunk-size 10000 \
    --csv-summary results/streaming_summary.csv
```

### Forcar caminho eager (v1-equivalente)

```bash
# coordinator/local: chunk_size = u32::MAX desativa streaming
relativist coordinator --workers 4 --port 9000 \
    --chunk-size 4294967295 -i net.bin -o out.bin

# bench: simplesmente nao passe --chunk-size
relativist bench --benchmark ep_annihilation --sizes 1000000
```

### Estrategia de particionamento

```bash
# Round-robin (default; assigna agente i ao worker i % num_workers)
relativist local --workers 4 --streaming-strategy round-robin -i net.bin

# Fennel (heuristic, agente vai para o worker com mais vizinhos ja assignados)
relativist local --workers 4 --streaming-strategy fennel \
    --fennel-alpha 1.5 -i net.bin
```

Round-robin tem zero state e match exato com `ContiguousIdStrategy` (SPEC-04). Fennel mantem cache `AgentId -> WorkerId` (~8 bytes/agente — 8x menor que materializar a `Net`) e melhora locality quando o gerador produz vizinhos no mesmo batch.

## 4. Eager vs streaming: profiles `bench-tcp` e `bench-tcp-eager`

O `docker-compose.yml` expoe **dois services** que diferenciam os caminhos para evitar regressao silenciosa:

```yaml
bench-tcp:                          # streaming explicito (CHUNK_SIZE default = 100, agressivo)
  command:
    - bench
    - --chunk-size=${CHUNK_SIZE:-100}
    - --max-pending-lifetime=${MAX_PENDING_LIFETIME:-16}

bench-tcp-eager:                    # eager (--chunk-size NAO e passado)
  command:
    - bench
    - --max-pending-lifetime=${MAX_PENDING_LIFETIME:-16}
    # nota: --chunk-size omitido => BenchmarkSuiteConfig.chunk_size = None
    #       => harness toma o branch eager em build_input_net_from_suite
```

CI roda **os dois** apos cada PR (QA-D011-004): se uma regressao silencia o caminho streaming (ex.: gerador caindo no eager por engano), o `bench-tcp-eager` continuaria verde e o smoke `bench-tcp` falharia, capturando o drift.

```bash
# Streaming
CHUNK_SIZE=100 docker compose --profile bench-tcp run --rm bench-tcp

# Eager (sem CHUNK_SIZE; o flag nao chega ao bench)
docker compose --profile bench-tcp-eager run --rm bench-tcp-eager
```

## 5. Forward references e `--max-pending-lifetime`

Geradores como `dual_tree` produzem agentes em ordem onde a *raiz* aparece num batch e os *filhos* em batches posteriores. SPEC-21 representa essas conexoes via `PendingConnection { source_agent_id, source_port, target_agent_id, target_port }`. O pipeline:

1. Quando um batch chega com pending, armazena no `pending_store`.
2. Quando um batch posterior cria o target, resolve a pending: vira wire interno ou border.

Se uma pending fica nao-resolvida por mais que `--max-pending-lifetime` batches (default 16), o pipeline aborta com erro — isso evita memory leak quando o gerador esquece de emitir um agente referenciado.

```bash
# Tornar o pipeline mais tolerante a forward refs longos
relativist local --workers 4 --max-pending-lifetime 64 -i tree_grande.bin

# Mais conservador (forca geradores a emitirem dependencias rapido)
relativist local --workers 4 --max-pending-lifetime 4 -i net.bin
```

## 6. Interacao com free-list recycle (SPEC-22 R10b/c)

Streaming **expande o threat model do free-list**. Em delta mode, recyclar um `AgentId` que o coordinator ainda referencia em sua `BorderGraph` quebra G1 — SPEC-22 R10b proibe. Sob streaming, o coordinator mantem `border_map` no pipeline (mesmo sem delta mode), expondo a mesma vulnerabilidade. SPEC-21 §3.8 A6 amplia o trigger de R10b/c para `(delta_mode || streaming_active) && id in border_referenced_set`.

Implementacao tem **duas estrategias normativas**:

| Estrategia                                | Comportamento                                                            | Quando                                |
|-------------------------------------------|--------------------------------------------------------------------------|---------------------------------------|
| `--recycle-policy disable-under-delta`    | Worker NAO popa do free-list enquanto `streaming || delta` ativo. Default (safe). | Producao, baseline canonico.          |
| `--recycle-policy border-clean`           | Worker popa do free-list, mas valida que o ID nao esta em borda local.   | Benchmarks que querem medir o ganho de recycle. |
| `--recycle-policy disable`                | Recycle sempre off (mais conservador ainda).                             | Smoke tests, repro de bugs antigos.   |

Alternativa de compile-time:

```bash
# Disable recycle outright via cargo feature
cargo build --release --features streaming-no-recycle
```

Essa feature satisfaz R37b "trivialmente" — o `create_agent` cai sempre no `next_id` quando ha streaming/delta ativo, sem ler o `RecyclePolicy` runtime.

Detalhes em [10-arena-management.md](10-arena-management.md) §3.

## 7. Trade-offs

- **Memoria do coord:** caiu de O(total_agents) para O(chunk_size). Ganho real em redes grandes.
- **Quality de particionamento:** round-robin streaming **iguala** o round-robin eager para geradores sequenciais. Fennel streaming pode perder ~5-10% de locality vs Fennel eager (heuristic so ve dados ate o batch atual).
- **CPU do coord:** overhead leve de gerenciamento de pending store + `border_accumulator` incremental. Fica < 5% do wall clock para `chunk_size=10000`.
- **Wire format:** PROTOCOL_VERSION foi para 5 (SPEC-21 R37c) ao adicionar `RequestWork` e `NoMoreWork` ao `Message` enum (necessarios para pull dispatch). Workers e coordinator precisam ter versao compativel; mismatch rejeita o handshake (SPEC-22 R10b precondition).

## 8. Caminho eager continua valido

Para reproducibility com baselines v1 + bisect cirurgico, o caminho eager esta intacto. Sintaxe:

```bash
# bench: omita --chunk-size
relativist bench --benchmark ep_annihilation --sizes 1000000 \
    --workers 1,2,4,8 --csv-summary v1_compat_summary.csv

# coordinator/local: passe u32::MAX
relativist coordinator --workers 4 --port 9000 \
    --chunk-size 4294967295 -i net.bin -o out.bin
```

O baseline canonico v2 ([`v2_post_d012_baseline_2026-05-05`](../../reproduce_article/results/locked/v2_post_d012_baseline_2026-05-05/)) usa `CHUNK_SIZE=10000` — i.e., streaming ON. Para comparar com `v1_local_baseline`, use o eager path.

## 9. Limitacoes

- **Default chunk_size = 10000 nao foi calibrado para Phase 3 LAN.** Comentario inline em `relativist-core/src/config.rs` marca isso como `SC-024 follow-up`. Apos Phase 3, sera ajustado.
- **Fennel ainda nao aproveita topology hints.** A versao atual e o Fennel basico de SPEC-21 R5; uma variacao topology-aware esta em ROADMAP 2.29.
- **Streaming + elastic departure:** funcional mas nao stress-testado. A `retained_partition` (guia [08](08-elastic-grid.md)) precisa preservar o snapshot do `border_accumulator` no momento do snapshot.
- **`generate` subcommand ainda materializa.** O `relativist generate ... -o file.bin` escreve a `Net` completa em disco — streaming so se aplica ao **dispatch** (do disco para os workers). Para evitar o pico em disco voce precisaria de um pipe direto generator -> coordinator, o que esta em ROADMAP mas nao implementado.

## 10. Proximo passo

- [SPEC-21](../../docs/specs/SPEC-21-streaming-generation.md) — especificacao formal (R1-R37g, pull/push dispatch, FENNEL, forward references, BorderGraph interaction).
- [10-arena-management.md](10-arena-management.md) — SPEC-22 arena, R10b/c free-list e a interacao critica com streaming.
- [06-delta-protocol.md](06-delta-protocol.md) — SPEC-19 delta protocol, complementar (delta reduz numero de mensagens; streaming reduz o pico de memoria do coord).
