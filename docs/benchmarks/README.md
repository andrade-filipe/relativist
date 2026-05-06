# Benchmarks & Campanhas

Esta pasta documenta a suite de benchmarks do Relativist e as campanhas cientificas que geraram os dados usados no TCC. Todos os CSVs congelados vivem em `results/locked/v1_local_baseline/` e `results/extended/v1_stress/`.

## Entry points

| Documento | Proposito |
|-----------|-----------|
| [bench suite](#bench--suite-de-benchmarks) (este arquivo) | Como invocar `relativist bench` e interpretar a saida |
| [phase-1-local.md](phase-1-local.md) | Phase 1 — in-process, 3 800 datapoints, ~6 h |
| [phase-2-docker.md](phase-2-docker.md) | Phase 2 — `tcp_localhost` via Docker, 400 datapoints |
| [phase-3-lan.md](phase-3-lan.md) | Phase 3 — `tcp_network` em LAN real (pendente) |
| [limitations.md](limitations.md) | L1–L7: limitacoes historicas e seus status |
| [pipeline.md](pipeline.md) | Pipeline completa `generate → reduce/local → inspect` |
| [campaigns/v1-local-baseline.md](campaigns/v1-local-baseline.md) | Tutorial: reproduzir a baseline congelada |
| [campaigns/v1-stress.md](campaigns/v1-stress.md) | Tutorial: rodar a campanha de stress (sizes maiores) |
| [campaigns/church-sum-of-squares.md](campaigns/church-sum-of-squares.md) | Demo aritmetica `Σ i²` de ponta a ponta |
| [campaigns/stress-curve.md](campaigns/stress-curve.md) | D-014 stress-curve campaign — N up to 10⁹ via `MemoryProbe` + `StopRule` |

## `bench` — Suite de Benchmarks

Executa a suite completa com baseline sequencial, warmup, repeticoes, verificacao de corretude e saida CSV.

```bash
relativist bench [OPCOES]
```

### Opcoes

| Flag                | Padrao      | Descricao                                   |
|---------------------|-------------|---------------------------------------------|
| `--benchmark`       | todos       | Benchmarks a executar (lista separada por `,`) |
| `--sizes`           | por bench   | Tamanhos de problema                        |
| `--workers`         | `1,2,4,8`   | Contagens de workers                        |
| `--mode`            | `local`     | Modo: `sequential`, `local`                 |
| `--warmup`          | `2`         | Rodadas de warmup (descartadas)             |
| `--repetitions`     | `5`         | Repeticoes cronometradas                    |
| `--csv-detail`      | —           | CSV detalhado (1 linha por repeticao)       |
| `--csv-rounds`      | —           | CSV por rodada (overhead por fase)          |
| `--csv-summary`     | —           | CSV resumo (estatisticas agregadas)         |
| `--max-rounds`      | —           | Limite de rodadas do grid                   |
| `--strict-bsp`      | off         | Forca modo strict (ver guia 03)             |
| `--delta-mode`      | off         | Ativa protocolo delta (ver guia 06)         |

### Benchmarks disponiveis

| Nome                   | Perfil | Descricao                                |
|------------------------|--------|------------------------------------------|
| `ep_annihilation`      | A      | ERA-ERA (aniquilacao void)               |
| `ep_annihilation_con`  | A      | CON-CON (aniquilacao cross)              |
| `ep_annihilation_dup`  | A      | DUP-DUP (aniquilacao parallel)           |
| `condup_expansion`     | B      | CON-DUP (expansao + colapso)             |
| `dual_tree`            | C      | Arvores duais (cascata nivel-a-nivel)    |
| `tree_sum`             | A/B    | Soma via Church add                      |
| `tree_sum_balanced`    | A/B    | Soma balanceada                          |
| `mixed_net`            | B      | Todas as 6 regras                        |
| `erasure_propagation`  | C      | Propagacao de erasure em cadeia          |
| `church_add`           | B      | Adicao Church numeral                    |
| `church_mul`           | B      | Multiplicacao Church numeral             |
| `cascade_cross`        | C      | Cascata CON-DUP cross-partition          |
| `church_sum_of_squares`| B      | Demo aritmetica `Σ i²` ([ver campanha](campaigns/church-sum-of-squares.md)) |

## Exemplos de invocacao

### Um benchmark, varias configuracoes

```bash
relativist bench \
  --benchmark ep_annihilation \
  --sizes 100,500,1000 \
  --workers 1,2,4 \
  --warmup 1 \
  --repetitions 3
```

### Suite completa com defaults

```bash
relativist bench --warmup 2 --repetitions 5
```

### Apenas modo sequencial

```bash
relativist bench \
  --benchmark ep_annihilation \
  --sizes 100,1000 \
  --mode sequential \
  --repetitions 5
```

### Exportar CSVs para analise

```bash
relativist bench \
  --benchmark ep_annihilation,condup_expansion,dual_tree \
  --sizes 100,500 \
  --workers 2,4 \
  --warmup 1 \
  --repetitions 5 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv \
  --csv-summary results/summary.csv
```

### Smoke de corretude rapido

```bash
relativist bench \
  --benchmark ep_annihilation,mixed_net,erasure_propagation,church_add \
  --sizes 10 \
  --workers 2 \
  --warmup 0 \
  --repetitions 1
```

## Saida da suite

```
=== Relativist Benchmark Suite ===
Benchmarks:  ep_annihilation
Mode:        local
Workers:     [1, 2, 4]
Warmup:      1
Repetitions: 3

=== Results ===
Benchmark               Size Workers    Time(s)     MIPS  Speedup Efficiency
--------------------------------------------------------------------------
ep_annihilation          100       0   0.000001     83.3   1.0000     1.0000
ep_annihilation          100       1   0.000003     34.1   0.4087     0.4087
ep_annihilation          100       2   0.000022      4.3   0.0520     0.0260
  WARNING: high variance (CV=13.72%) ...

Total datapoints: 18  |  All correct: true
```

## Metricas

- **Time(s)** — mediana do wall-clock.
- **MIPS** — Millions of Interactions Per Second.
- **Speedup** — `t_sequencial / t_grid` (> 1.0 = distribuicao compensa).
- **Efficiency** — `speedup / workers` (1.0 = escala perfeitamente).
- **CV** — coeficiente de variacao (aviso se > 10%).

## Formato dos CSVs

### `detail.csv` — uma linha por execucao

```
benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,
total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,
peak_memory_bytes,bytes_sent,bytes_received,
con_con,dup_dup,era_era,con_dup,con_era,dup_era
```

### `rounds.csv` — uma linha por rodada (apenas grid)

```
benchmark,input_size,workers,mode,repetition,round,
partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,
border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received
```

### `summary.csv` — uma linha por configuracao

```
benchmark,input_size,mode,workers,repetitions,all_correct,
wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,
mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv
```

## Correspondencia entre fases

| Comando base               | Modo no CSV     | Origem do overhead                 |
|----------------------------|-----------------|------------------------------------|
| `relativist reduce`        | `sequential`    | Nenhum (baseline)                  |
| `relativist local -w N`    | `local`         | Particionamento + merge in-process |
| `docker compose up`        | `tcp_localhost` | + serializacao + TCP loopback      |
| `coordinator`/`worker` LAN | `tcp_network`   | + RTT real + jitter                |

Esta decomposicao permite isolar a contribuicao de cada camada no `overhead_ratio` reportado.

## Resultados congelados

**Zero falhas de corretude em 4 200+ execucoes.**

| Campanha           | Reps  | Wall Clock  | Status   |
|--------------------|-------|-------------|----------|
| Phase 1 (in-process) | 3 800 | 11 min 39 s | Completo |
| Phase 2 (Docker)     | 400   | 43 min 42 s | Completo |
| Phase 3 (LAN)        | —     | —           | Pendente |

Cada datapoint e verificado pela propriedade fundamental `reduce_all(net) ≅ run_grid(net, n)` — isomorfismo estrutural (modulo renomeacao de IDs).

Dados: [`results/locked/v1_local_baseline/`](../../results/locked/v1_local_baseline/) com checksums SHA-256 e manifest de provenance.
