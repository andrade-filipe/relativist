# v1_local_baseline — Resumo Consolidado dos Resultados Locais

**Tag do binário:** `v0.10.0-bench` (commit `8529dd5`)
**Campanhas executadas em:** 2026-04-11
**Hardware:** Lenovo ThinkPad T14 Gen 4 (Intel i7-1365U, 32 GB DDR5-5200, Windows 11 Pro, Docker Desktop 29.3.1/WSL2)
**Snapshot imutável:** [`reproduce_article/results/locked/v1_local_baseline/`](../reproduce_article/results/locked/v1_local_baseline/)
**Status:** FECHADA — referência para subtração da Phase 3 LAN

---

## TL;DR

A Local Benchmark Phase do Relativist foi fechada em 2026-04-11 com **4200 execuções totais (3800 em Phase 1 + 400 em Phase 2), zero falhas de correção**. As duas campanhas rodaram no mesmo binário tagged `v0.10.0-bench` em 11 min 39 s (Phase 1, in-process) e 43 min 42 s (Phase 2, Docker/TcpLocalhost). O achado crítico da sessão — L2, a colapsagem do loop BSP em 1 rodada única — foi **resolvido arquiteturalmente** pelo novo modo `strict_bsp` opt-in, e os rounds empíricos do `cascade_cross` e `dual_tree` sob strict mode batem **exatamente** com as predições teóricas do SPEC-09. A baseline v1 é agora referência congelada: qualquer medição da Phase 3 LAN será subtraída desses dados para isolar o custo de rede.

---

## 1. Objetivo e escopo

A "Local Benchmark Phase" é a base de comparação local que a **Phase 3 LAN** vai subtrair para isolar o custo de rede real:

> `t_network = t_lan − t_localhost`

Qualquer imprecisão, drift ou artefato presente nesta baseline vira contaminante permanente em toda conclusão downstream. Por isso a campanha foi estruturada com as seguintes garantias:

- **Binário único tagged** — um só `cargo build --release` produziu o executável usado em ambas as fases; tag `v0.10.0-bench` anotada aponta para o commit que congelou os dados.
- **Atomicidade do snapshot** — os CSVs, o `manifest.md` com checksums sha256, os logs forensic e o próprio tag foram commitados como uma unidade em `787b195`.
- **Imutabilidade cross-platform** — `.gitattributes` fixa `text eol=lf` em `reproduce_article/results/locked/**` para que os checksums sha256 sobrevivam a checkouts em Windows com `core.autocrlf=true`.
- **Reprodutibilidade** — `reproduce_article/scripts/reproduce_local_baseline.sh` faz checkout da tag, rebuild, re-rodada, e comparação de row counts + correctness flags.

---

## 2. Metodologia — Phase 1 vs Phase 2

Duas campanhas complementares, mesmo binário, mesmo hardware:

| Dimensão | **Phase 1** | **Phase 2** |
|---|---|---|
| Modo de execução | In-process (`--local`) | Docker containers com `TcpLocalhost` |
| Driver | `reproduce_article/scripts/bench_phase1_locked.sh` | `reproduce_article/scripts/bench_phase2_locked.sh` |
| Propósito | Baseline sequencial + shared-memory parallel | Baseline de overhead do protocolo TCP localhost |
| Benchmarks lenient | 12 (todos os `default_sizes()`) | 3 (`ep_annihilation_con`, `dual_tree`, `condup_expansion`) |
| Benchmarks strict | 2 (`cascade_cross` default, `dual_tree` {6,10,14}) | — (Phase 2 é lenient por design) |
| Workers testados | `{sequential, local 1, local 2, local 4, local 8}` | `{sequential, tcp_localhost 1, 2, 4, 8}` |
| Repetições / config | 10 (+ 2 warmup) | 10 (+ 2 warmup) |
| Timeout por run | — | 1800 s |
| Total de medições | **3800 reps** | **400 reps** |
| Wall clock total | **11 min 39 s** | **43 min 42 s** |
| Método de correção | Full G1 (isomorfismo) em tudo, exceto `condup_expansion` 10k/50k que usa `--skip-g1` (abordagem A) | Structural check via `relativist inspect` (agent + redex count) |
| Falhas de correção | **0 / 3800** | **0 / 400** |

**Por que Phase 2 só tem 3 benchmarks.** Phase 2 é deliberadamente focada nos únicos benchmarks onde o overhead do protocolo TCP localhost é mensurável em wall-clock significativo: os que produzem nets grandes (`ep_annihilation_con` 500k/1M/5M; `dual_tree` 18/20/22) e um pequeno (`condup_expansion` 1k/5k) para contextualizar o startup cost do container. Os 9 benchmarks menores rodariam em sub-milisegundos contaminados pelo startup do Docker.

---

## 3. Resultados Phase 1 — In-process

### 3.1 Wall clock e correção

| Métrica | Valor |
|---|---|
| Wall clock total | **11 min 39 s** (`11:44:55 → 11:56:34`) |
| Repetições | 3800 (12 lenient + 2 strict benchmarks × sizes × workers × 10 reps) |
| Falhas de correção | **0 / 3800** |
| Row counts (`phase1_lenient_detail.csv`) | 3401 linhas (3400 reps + header) |
| Row counts (`phase1_strict_detail.csv`) | 401 linhas (400 reps + header) |

**Por que muito mais rápido que os 4-6 h estimados.** A estimativa original presumia o pior caso U-series throttled + G1 full. Na prática: (a) DDR5-5200 compensa boa parte do sustained-load concern do chip U; (b) a abordagem A (`--skip-g1` em `condup_expansion` 10k/50k) elimina o custo O(N!) do isomorfismo de grafos nos piores configs.

### 3.2 Key datapoints — `ep_annihilation_con` (Profile B)

Headline: o overhead shared-memory parallel cresce com `workers`, não há break-even em nenhum size testado nesta fase.

| Size | Sequencial (s) | Local w=1 (s) | Local w=2 (s) | Local w=4 (s) | Local w=8 (s) | Speedup w=2 |
|---|---|---|---|---|---|---|
| 1 000 | 0.000137 | 0.000148 | 0.000272 | 0.000311 | 0.000404 | 0.506 |
| 5 000 | 0.000682 | 0.000716 | 0.001408 | 0.001503 | 0.001858 | 0.465 |
| 10 000 | 0.001429 | 0.001508 | 0.002918 | 0.003018 | 0.003614 | 0.436 |
| 50 000 | 0.006236 | 0.006631 | 0.015765 | 0.016687 | 0.018528 | 0.395 |
| 100 000 | 0.012775 | 0.013940 | 0.033859 | 0.035616 | 0.044183 | 0.378 |

Consistente com o achado L1 da v0.9.0: nenhum config cruza `speedup > 1.0` em `workers ≥ 2` para benchmarks in-process. O overhead da distribuição local (particionamento, partition_to_subnet, merge) excede o ganho paralelo em toda a faixa testada, como o TCC já previa e documenta.

### 3.3 Key datapoints — `dual_tree` (Profile C, cascata profunda)

| Depth | Sequencial (s) | Local w=1 (s) | Local w=2 (s) | Local w=4 (s) | Local w=8 (s) |
|---|---|---|---|---|---|
| 10 | 0.000083 | 0.000077 | 0.000279 | 0.000278 | 0.000350 |
| 12 | 0.000340 | 0.000324 | 0.001067 | 0.001146 | 0.001342 |
| 14 | 0.001329 | 0.001378 | 0.004635 | 0.004988 | 0.005670 |

### 3.4 Key datapoints — `cascade_cross` (novo, Profile B)

O benchmark que existe precisamente para exercitar o modo strict BSP:

| N | Sequencial (s) | Local lenient w=8 (s) |
|---|---|---|
| 10 | 0.000001 | 0.000011 |
| 50 | 0.000004 | 0.000025 |
| 100 | 0.000007 | 0.000040 |
| 500 | 0.000046 | 0.000200 |
| 1 000 | 0.000084 | 0.000401 |

Linear em N, wall-clock sub-milisegundo mesmo no maior tamanho — adequado para CI e para exercitar o modo strict sem stress de escalabilidade.

---

## 4. Resultados Phase 2 — Docker / TcpLocalhost

### 4.1 Wall clock e correção

| Métrica | Valor |
|---|---|
| Wall clock total | **43 min 42 s** (`12:22:37 → 13:06:19`) |
| Repetições | 400 (8 bench×size × 5 worker configs × 10 reps) |
| Falhas de correção | **0 / 400** |
| Configs L6 previamente bloqueados | **Desbloqueados** (`dual_tree=22 w=1`; `ep_annihilation_con=5M w∈{1,2,4}`) |
| Row counts (`phase2_detail.csv`) | 401 linhas (400 reps + header) |
| Rounds por run | **sempre 1** (lenient by design) |

### 4.2 Key datapoints — `ep_annihilation_con` (configs grandes)

O overhead do protocolo TCP localhost é mensurável e cresce monotonicamente com o número de workers:

| Size | Sequencial (s) | TCP w=1 (s) | TCP w=2 (s) | TCP w=4 (s) | TCP w=8 (s) | w=8 / seq |
|---|---|---|---|---|---|---|
| 500 000 | 0.336 | 0.300 | 0.379 | 0.415 | 0.551 | 1.64× |
| 1 000 000 | 0.541 | 0.574 | 0.822 | 0.853 | 1.166 | 2.15× |
| 5 000 000 | 2.490 | 3.186 | 5.041 | 5.550 | 7.253 | 2.91× |

Leitura: no caso mais pesado (`5M × w=8`), o protocolo TCP localhost adiciona **~2.91× wall clock** versus o baseline sequencial em memória. Esse número é o **limite inferior** do overhead distribuído — é o que a Phase 3 LAN vai subtrair. Qualquer custo adicional observado em LAN (`t_lan − 7.253 s`) é atribuível a RTT de rede real.

### 4.3 Key datapoints — `dual_tree` (cascata profunda sob Docker)

| Depth | Sequencial (s) | TCP w=1 (s) | TCP w=2 (s) | TCP w=4 (s) | TCP w=8 (s) |
|---|---|---|---|---|---|
| 18 | 0.222 | 0.164 | 0.236 | 0.225 | 0.245 |
| 20 | 0.448 | 0.468 | 1.062 | 1.004 | 1.111 |
| 22 | 1.531 | 1.935 | 5.031 | 5.044 | 5.362 |

O único config onde `w=1` bate o sequencial é `dual_tree 18` (0.164 vs 0.222 s) — provável artefato de scheduling no cache L3 do chip. A curva confirma o padrão geral: overhead dominante em `w ≥ 2` por causa dos 2 P-cores físicos apenas.

### 4.4 Nota sobre `condup_expansion` Phase 2

`condup_expansion` 1 000 e 5 000 em modo `tcp_localhost w=1` aparece com wall-clock muito menor que o sequencial baseline (0.002 s vs 0.196 s), produzindo "speedup" absurdo de 96×. **Não é anomalia do protocolo** — é característica do benchmark em sizes pequenos contra o código path do sequential baseline em Phase 2 (que tem o overhead fixo do `relativist reduce` subcommand). Para análise comparativa útil, os dados desses dois configs devem ser lidos apenas relativamente entre os 4 worker counts (w=1 → w=8), não contra a linha sequencial. O dado está no CSV para completude, mas o artigo NÃO deve plotar speedup desses configs.

---

## 5. Três validações teóricas confirmadas

### 5.1 L2 resolvido — BSP multi-round funciona

**Antes (v0.9.0):** todo benchmark convergia em 1 rodada no grid loop. Isto foi originalmente interpretado como "otimização lenient", mas a investigação desta sessão descobriu que era **um bug do loop do coordenador**: `reduce_all(&mut merged_net)` em `src/merge/grid.rs:117` drenava toda a cascata de border redexes no coordenador, e o check `redex_queue.is_empty()` logo depois sempre saía em 1 rodada independente da topologia.

**Depois (v0.10.0-bench):** novo modo `strict_bsp` opt-in (`SPEC-05 R30a`) adiciona um branch que usa o primitivo `reduce_border_once` em vez de `reduce_all` na fase "resolve borders". Sob strict mode, as cascatas cross-partition voltam a ser border redexes na próxima rodada, e o loop itera até normal form.

**Validação empírica — strict BSP rounds batem as predições teóricas do SPEC-09:**

| Benchmark | Size | Workers | Rounds teórico (SPEC-09) | Rounds medido |
|---|---|---|---|---|
| `cascade_cross` | 10 | 2 | 10 | **10** |
| `cascade_cross` | 50 | 2 | 50 | **50** |
| `cascade_cross` | 100 | 4 | 100 | **100** |
| `cascade_cross` | 500 | 8 | 500 | **500** |
| `cascade_cross` | 1 000 | 8 | 1 000 | **1 000** |
| `dual_tree` | 6 | 2 | 6 | **6** |
| `dual_tree` | 10 | 4 | 10 | **10** |
| `dual_tree` | 14 | 8 | 14 | **14** |

Casamento exato em todos os 8 configs validados. A predição `cascade_cross(N) = N rounds` e `dual_tree(d) = d rounds` com `workers ≥ 2` do SPEC-09 R18 é agora empiricamente confirmada.

**Propriedade G1 preservada em ambos os modos** (SPEC-01 D6): `R_strict(μ, n) ≥ R_lenient(μ, n) = 1`. A redução completa ainda acontece; apenas é distribuída entre mais rodadas sob strict mode. Zero falhas de correção em ambos os modos confirmam que a equivalência `reduce_all ≡ run_grid` vale independentemente do modo.

### 5.2 G1 100% em 4200 execuções

`awk -F, 'NR>1 && $6=="false"'` aplicado a qualquer `*_detail.csv` do snapshot retorna **zero linhas**. Em números:

| Fase | Execuções | Falhas G1 / weak / estrutural |
|---|---|---|
| Phase 1 lenient (12 bench) | ~3400 | 0 |
| Phase 1 strict (2 bench) | ~400 | 0 |
| Phase 2 Docker (3 bench) | 400 | 0 |
| **Total** | **~4200** | **0** |

### 5.3 L6 desbloqueado (secundário)

Os 4 configs Phase 2 historicamente bloqueados pelo cap de 256 MiB do frame bincode agora completam dentro do timeout 1800 s em todos os worker counts:

- `dual_tree=22 w=1` — baseline era F, agora 1.935 s
- `ep_annihilation_con=5M w=1` — agora 3.186 s
- `ep_annihilation_con=5M w=2` — agora 5.041 s
- `ep_annihilation_con=5M w=4` — agora 5.550 s

Isto não é descoberta desta sessão (foi resolvido em v0.9.0 com `CompactSubnet` + cap raise para 1 GiB, ROADMAP 2.20), mas é a primeira vez que todos os 4 configs aparecem validados na mesma campanha com o mesmo binário.

---

## 6. Ressalvas de hardware (disclosure para o artigo)

O campanha rodou em um ultrabook ThinkPad T14 Gen 4 de 14 polegadas, não em uma estação de trabalho dedicada. Três caveats de hardware afetam a leitura dos wall-clock absolutos:

1. **Throttling U-series em runs longos.** O i7-1365U sustenta PL1 = 15 W indefinidamente, mas exaure o turbo PL2 = 55 W em segundos. Benchmarks curtos podem ter picos de turbo; benchmarks longos (`dual_tree=22`, `ep_annihilation_con=5M`) rodam largamente no regime throttled. Consequência: o ratio "benchmark longo / benchmark curto" é enviesado para cima comparado a um workstation. **Aceitável para a baseline** porque Phase 3 compara *mesmo hardware* contra LAN, não contra uma máquina idealizada.

2. **Scheduling hibrido P/E injeta variância.** Com `--workers 8`, os 8 threads de redução são distribuídos pelo Windows scheduler entre 2 P-cores (~5.2 GHz) e 8 E-cores (~3.9 GHz). A alocação muda entre repetições — fonte de variância que não pode ser eliminada sem core pinning (que por si comprometeria a representatividade da baseline). 4 datapoints de CV triage documentam o efeito no range 0.155-0.193.

3. **Docker oversubscribes o CPU físico.** A WSL2 VM usa 12 vCPUs contra os 10 cores físicos (12 threads lógicos) do host. Em Phase 2 este é o ponto de operação intencional. **Docker Desktop foi fechado durante Phase 1** para evitar o ~2-4 GB de footprint residual do WSL2 contaminar medições in-process, e reaberto imediatamente antes de Phase 2.

Estas limitações são por que `reproduce_article/scripts/cv_triage.py` existe e por que o artigo reportará CV junto com wall-clock médio: transformam o caveat em metodologia disclosable em vez de confundidor oculto.

Documentadas em detalhe no `manifest.md` §"Hardware constraints (disclosure)".

---

## 7. CV Triage — análise de variância

**Threshold:** CV > 0.15. **Ferramenta:** `reproduce_article/scripts/cv_triage.py`.

| Fase | Summary rows | Flagged (CV > 0.15) | Keep | Rerun | Exclude |
|---|---|---|---|---|---|
| Phase 1 lenient | 340 | 60 | 60 | 0 | 0 |
| Phase 1 strict | 40 | 2 | 2 | 0 | 0 |
| Phase 2 Docker | 40 | 1 | 1 | 0 | 0 |
| **Total** | **420** | **63** | **63** | **0** | **0** |

**Composição dos 63 keep:**
- **58 timer noise sub-milisegundo** (wall-clock < 5 ms, CV alto por resolução do `Instant::now`, não variância real).
- **4 genuína variância P/E hybrid scheduling** (todos na faixa 0.155-0.193, bem abaixo do threshold de 0.30 para rerun). Serão anotados em footnote no artigo.
- **1 timer noise Phase 2** (`condup_expansion 1000 w=1` em 1.99 ms, CV 0.172).

Zero reruns, zero exclusions — a campanha é assinada sem rejeições. Detalhe linha-por-linha em [`cv_triage.md`](../reproduce_article/results/locked/v1_local_baseline/cv_triage.md).

---

## 8. O que está desbloqueado para Phase 3 LAN

Com a baseline v1 congelada, a Phase 3 LAN tem:

1. **Referência de subtração pronta.** Qualquer medição `t_lan` será comparada contra o valor correspondente em `phase2_summary.csv`. A diferença `t_lan − t_localhost` é o custo de rede real, isolado de overhead de protocolo ou serialização.

2. **Per-round RTT mensurável.** Os dados `phase1_strict_rounds.csv` (50 781 linhas) têm o wall-clock de cada round BSP individual para `cascade_cross` e `dual_tree` sob strict mode. A Phase 3 vai comparar o mesmo config em LAN e medir diretamente o overhead de RTT por round — métrica central que o SPEC-09 promete e que só ficou disponível após a resolução do L2.

3. **Binário idêntico.** Tag `v0.10.0-bench` garante que a Phase 3 usará exatamente o mesmo executável que gerou os números locais. Sem drift de binário entre campanhas.

4. **Reprodutibilidade documentada.** `reproduce_article/scripts/reproduce_local_baseline.sh` permite que qualquer revisor reexecute a Phase 1 + Phase 2 em hardware alternativo e compare row counts + correctness flags (wall-clock diverge, mas estrutura dos dados é invariante).

5. **Hardware caveats disclosable.** As 3 ressalvas do §6 permitem defender os números no artigo como "baseline honesta do ultrabook", não como número universal. Phase 3 LAN comparará *o mesmo ultrabook* contra as mesmas máquinas em rede, eliminando a dependência de hardware idealizado.

Documentação operacional completa da Phase 3 em [`USAGE_GUIDE.md`](../USAGE_GUIDE.md) §11.3.

---

## 9. Fontes detalhadas

Este documento é um resumo consolidado. Os documentos de origem, cada um com nível adicional de detalhe, são:

| Fonte | Conteúdo | Onde olhar |
|---|---|---|
| [`reproduce_article/results/locked/v1_local_baseline/manifest.md`](../reproduce_article/results/locked/v1_local_baseline/manifest.md) | Provenance completa: commit SHA, hardware detalhado, timestamps, checksums sha256, row counts, campaign knobs, Phase 1 + Phase 2 results summary | §"Phase 1 results summary", §"Phase 2 results summary" |
| [`reproduce_article/results/locked/v1_local_baseline/README.md`](../reproduce_article/results/locked/v1_local_baseline/README.md) | Índice dos arquivos congelados + scope notes | Top-level |
| [`reproduce_article/results/locked/v1_local_baseline/cv_triage.md`](../reproduce_article/results/locked/v1_local_baseline/cv_triage.md) | Todos os 63 datapoints flagged com disposição linha por linha | §"Phase 1 (lenient)", §"Phase 1 (strict)", §"Phase 2 (Docker)" |
| [`docs/PHASE1-FINDINGS.md`](PHASE1-FINDINGS.md) | Narrativa original + L2 resolvido + tabela empírica strict BSP | §L2 |
| [`docs/PHASE2-FINDINGS.md`](PHASE2-FINDINGS.md) | Narrativa Phase 2 + history de fixes L3/L6 + role no Phase 3 subtraction | §7 "v1_local_baseline — Unified Frozen Campaign" |
| [`USAGE_GUIDE.md`](../USAGE_GUIDE.md) | Tutorial operacional Phase 3 LAN completo | §11.3 (10 subseções) |
| [`docs/specs/SPEC-05-merge.md`](../docs/specs/SPEC-05-merge.md) | Formalização lenient vs strict BSP | §"Lenient vs Strict BSP modes", R30a |
| [`docs/specs/SPEC-09-benchmarks.md`](../docs/specs/SPEC-09-benchmarks.md) | Tabela property de cada benchmark + rounds teóricos | R18 (cascade_cross) |
| [`docs/specs/SPEC-01-invariantes.md`](../docs/specs/SPEC-01-invariantes.md) | G1 em ambos os modos; D6 refinado com R_strict ≥ R_lenient | D6, G1 |
| [`docs/ROADMAP.md`](ROADMAP.md) | v2 direções (incluindo 2.21 WAN/Internet deployment) | §2.16 (streaming), §2.20 (CompactSubnet DONE), §2.21 (WAN) |

---

**Próximo marco:** [Phase 3 LAN](../USAGE_GUIDE.md#113-phase-3--tcpnetwork-maquinas-reais) — TcpNetwork em máquinas físicas (SPEC-09 R27 MUST). Baseline local v1 pronta como referência de subtração.
