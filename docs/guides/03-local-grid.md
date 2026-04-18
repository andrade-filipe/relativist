# 3 — Grid Local (BSP in-process)

O subcomando `local` executa o **ciclo BSP completo** (Bulk Synchronous Parallel) em um unico processo, sem rede. E o jeito mais rapido de provar que a distribuicao preserva o resultado (G1) e de medir o overhead algoritmico do protocolo, isoladamente do custo de TCP.

Ciclo BSP:

```
particionar  →  reduzir localmente  →  merge  →  resolver borda  →  repetir
```

> Este guia assume que voce ja sabe gerar e inspecionar redes ([02 — Primeira Reducao](02-first-reduction.md)).

## 3.1 Uso

```bash
relativist local -w <WORKERS> -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>]
```

### Opcoes principais

| Flag               | Descricao                                         |
|--------------------|---------------------------------------------------|
| `-w, --workers N`  | Numero de workers simulados (`>= 1`)              |
| `-i, --input`      | Arquivo da rede de entrada (`.bin` ou `.ic`)      |
| `-o, --output`     | Salvar rede reduzida                              |
| `-m, --metrics`    | Salvar metricas em JSON                           |
| `--max-rounds N`   | Limite de rodadas BSP (sem limite por padrao)     |
| `--strategy`       | Estrategia de particionamento (`round-robin`)     |
| `--log-format`     | Formato de log: `text` ou `json`                  |
| `--strict-bsp`     | Uma rodada BSP genuina por fila (ver Secao 3.4)   |
| `--delta-mode`     | Ativa o protocolo delta (v2 — [guia 06](06-delta-protocol.md)) |

Todas as flags estao listadas em [reference/cli.md](../reference/cli.md).

## 3.2 Exemplo — smoke test em 4 workers

```bash
relativist generate ep-annihilation -n 500 -o ep500.bin
relativist local -w 4 -i ep500.bin
```

Saida:

```
=== Relativist Execution Summary ===
Converged:          yes
Rounds:             1
Total interactions: 500
Total time:         0.000s
Final agents:       0
Avg round time:     0.000s
Local interactions: 500
Border interactions:0
```

Leitura:

- **Converged: yes** → rede atingiu a forma normal (SPEC-01 G1).
- **Rounds: 1** → nao houve cascata cross-partition (Profile A).
- **Border interactions: 0** → nenhuma reacao precisou de merge — o particionamento round-robin distribuiu redexes independentes.

## 3.3 Salvar resultado e metricas

```bash
relativist generate mixed-rules -n 5 -o mixed5.bin
relativist local -w 2 -i mixed5.bin -o mixed5_grid.bin -m metrics.json
```

O `metrics.json` tem a lista `rounds` com, por rodada:

- `partition_time_secs`, `compute_time_secs`, `merge_time_secs`, `network_time_secs`
- `border_redexes`, `border_ratio`, `agents_at_start`
- `bytes_sent`, `bytes_received` (no modo in-process, zero)

Use esse arquivo para plotar overhead por fase em cada rodada.

## 3.4 Strict BSP — forcar rodadas reais

Por padrao (`--strict-bsp=false`) o Relativist opera em **lenient mode**: apos o merge, `reduce_all` esgota a fila inteira — incluindo cascatas cross-partition geradas por resolver borda, produzindo resultado correto em **uma unica rodada** para a maioria dos benchmarks.

Para observar o custo real de cada rodada (e medir RTT no modo TCP):

```bash
relativist generate cascade-cross -n 100 -o cc100.bin
relativist local -w 2 -i cc100.bin --strict-bsp -m strict_metrics.json
```

Em strict mode a fila e processada exatamente **uma vez** por rodada; novos redexes ficam enfileirados para a proxima. Para `cascade_cross(N)` com `workers >= 2`, a rede termina em **exatamente N rodadas** — confirma a previsao teorica.

Contexto historico e motivacao em [benchmarks/limitations.md](../benchmarks/limitations.md) (item **L2**).

## 3.5 Limite de rodadas e log JSON

```bash
# Interrompe apos 5 rodadas
relativist generate dual-tree -n 10 -o dual10.bin
relativist local -w 4 -i dual10.bin --max-rounds 5

# Log estruturado (util para pipelines de dados)
relativist generate con-dup-expansion -n 50 -o condup50.bin
relativist local -w 2 -i condup50.bin --log-format json
```

## 3.6 Quando NAO usar `local`

- Para medir **custo de rede real**: use `coordinator` + `worker` ([guia 05](05-distributed-tcp.md)).
- Para medir **apenas algoritmo sequencial**: use `reduce` ([guia 02](02-first-reduction.md)).
- Para **expressar contas aritmeticas** em alto nivel: use `compute` ([guia 04](04-church-arithmetic.md)).

---

**Proximo guia →** [04 — Aritmetica Church](04-church-arithmetic.md): `add`/`mul`/`exp` codificados em IC.
